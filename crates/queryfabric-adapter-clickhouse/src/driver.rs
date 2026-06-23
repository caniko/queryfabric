use std::borrow::Cow;
use std::io::Cursor;
use std::sync::Arc;

use arrow::array::Array;
use arrow::buffer::Buffer;
use arrow::compute::cast;
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use arrow_ipc::reader::{StreamDecoder, StreamReader};
use arrow_ipc::writer::StreamWriter;
use futures::StreamExt;
use reqwest::{Client, Response};
use secrecy::{ExposeSecret, SecretString};
use thiserror::Error;
use tokio::sync::mpsc;
use tracing::debug;

use futures::FutureExt;

/// Configuration for connecting to a ClickHouse HTTP server.
#[derive(Debug, Clone)]
pub struct ClickHouseConfig {
    pub host: String,
    pub fallback_hosts: Vec<String>,
    pub port: u16,
    pub username: String,
    pub password: SecretString,
    pub database: String,
    pub secure: bool,
}

impl Default for ClickHouseConfig {
    fn default() -> Self {
        Self {
            host: "localhost".to_owned(),
            fallback_hosts: Vec::new(),
            port: 8123,
            username: "default".to_owned(),
            password: SecretString::from(String::new()),
            database: "default".to_owned(),
            secure: false,
        }
    }
}

/// Errors from ClickHouse HTTP client operations.
#[derive(Debug, Error)]
pub enum ClickHouseError {
    #[error("ClickHouse client error: {0}")]
    Client(#[from] clickhouse::error::Error),

    #[error("Invalid schema: {0}")]
    InvalidSchema(String),

    #[error("Invalid identifier: {0}")]
    InvalidIdentifier(String),

    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Arrow error: {0}")]
    Arrow(#[from] arrow::error::ArrowError),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, ClickHouseError>;

/// Dynamic ClickHouse client using raw HTTP with Arrow IPC streaming.
#[derive(Debug, Clone)]
pub struct DynamicClient {
    http: Client,
    base_url: String,
    fallback_base_urls: Vec<String>,
    user: String,
    password: SecretString,
    database: String,
}

fn clickhouse_url(scheme: &str, host: &str, port: u16) -> String {
    format!("{scheme}://{host}:{port}")
}

fn select_send_error_is_retryable(error: &reqwest::Error) -> bool {
    error.status().is_none() && !error.is_body() && !error.is_decode()
}

async fn check_response(resp: reqwest::Response, context: &str) -> Result<reqwest::Response> {
    if resp.status().is_success() {
        return Ok(resp);
    }
    let status = resp.status();
    let body = resp.text().await.unwrap_or_else(|e| {
        tracing::warn!(error = %e, "Failed to read ClickHouse error response body");
        String::new()
    });
    tracing::warn!(%status, "{context} failed");
    Err(ClickHouseError::InvalidSchema(format!(
        "{context} ({status}): {body}"
    )))
}

impl ClickHouseConfig {
    /// Build the base URL for the ClickHouse HTTP interface.
    pub fn url(&self) -> String {
        let scheme = if self.secure { "https" } else { "http" };
        format!("{scheme}://{}:{}", self.host, self.port)
    }
}

impl DynamicClient {
    pub fn from_config(config: &ClickHouseConfig) -> Self {
        let scheme = if config.secure { "https" } else { "http" };
        let base_url = clickhouse_url(scheme, &config.host, config.port);
        let fallback_base_urls = config
            .fallback_hosts
            .iter()
            .map(|host| host.trim())
            .filter(|host| !host.is_empty())
            .map(|host| clickhouse_url(scheme, host, config.port))
            .filter(|url| url != &base_url)
            .collect();

        Self {
            http: Client::new(),
            base_url,
            fallback_base_urls,
            user: config.username.clone(),
            password: config.password.clone(),
            database: config.database.clone(),
        }
    }

    pub fn database(&self) -> &str {
        &self.database
    }

    fn select_base_urls(&self) -> Vec<String> {
        let mut urls = Vec::with_capacity(1 + self.fallback_base_urls.len());
        urls.push(self.base_url.clone());
        urls.extend(self.fallback_base_urls.iter().cloned());
        urls
    }

    async fn send_select_to(
        http: &Client,
        base_url: &str,
        user: &str,
        password: &SecretString,
        sql: String,
    ) -> std::result::Result<Response, reqwest::Error> {
        http.post(base_url)
            .basic_auth(user, Some(password.expose_secret()))
            .body(sql)
            .send()
            .await
    }

    pub async fn insert_json_each_row(&self, table_fqn: &str, json_body: &str) -> Result<()> {
        let sql = format!("INSERT INTO {table_fqn} FORMAT JSONEachRow\n{json_body}");
        debug!(
            table = table_fqn,
            bytes = json_body.len(),
            "DynamicClient INSERT"
        );
        let resp = self
            .http
            .post(&self.base_url)
            .basic_auth(&self.user, Some(self.password.expose_secret()))
            .body(sql)
            .send()
            .await?;
        check_response(resp, "INSERT failed").await?;
        Ok(())
    }

    pub async fn insert_arrow(&self, table_fqn: &str, batches: &[RecordBatch]) -> Result<()> {
        if batches.is_empty() {
            return Ok(());
        }
        let batches = downcast_view_types(batches)?;
        let batches: &[RecordBatch] = &batches;
        let schema = batches[0].schema();
        let total_rows: usize = batches.iter().map(|b| b.num_rows()).sum();
        let sql_prefix = format!("INSERT INTO {table_fqn} FORMAT ArrowStream\n");
        let mut body = sql_prefix.into_bytes();
        {
            let mut writer = StreamWriter::try_new(&mut body, &schema)?;
            for batch in batches {
                writer.write(batch)?;
            }
            writer.finish()?;
        }
        debug!(
            table = table_fqn,
            rows = total_rows,
            ipc_bytes = body.len(),
            "DynamicClient INSERT Arrow"
        );
        let resp = self
            .http
            .post(&self.base_url)
            .basic_auth(&self.user, Some(self.password.expose_secret()))
            .header("Content-Type", "application/octet-stream")
            .body(body)
            .send()
            .await?;
        check_response(resp, "INSERT Arrow failed").await?;
        Ok(())
    }

    pub async fn query_arrow(&self, sql: &str) -> Result<Vec<RecordBatch>> {
        let full_sql = format!("{sql} FORMAT ArrowStream");
        debug!(sql = sql, "DynamicClient SELECT Arrow");
        let urls = self.select_base_urls();
        let mut last_error = None;
        let mut resp = None;
        for (index, base_url) in urls.iter().enumerate() {
            match Self::send_select_to(
                &self.http,
                base_url,
                &self.user,
                &self.password,
                full_sql.clone(),
            )
            .await
            {
                Ok(response) => {
                    resp = Some(response);
                    break;
                }
                Err(error) if select_send_error_is_retryable(&error) && index + 1 < urls.len() => {
                    tracing::warn!(error = %error, failed_clickhouse_url = %base_url, next_clickhouse_url = %urls[index + 1], "retrying fallback host");
                    last_error = Some(error);
                }
                Err(error) => return Err(ClickHouseError::Http(error)),
            }
        }
        let resp = match resp {
            Some(resp) => resp,
            None => {
                let error = last_error.map_or_else(
                    || {
                        ClickHouseError::Other(
                            "all ClickHouse URLs exhausted with retryable errors".into(),
                        )
                    },
                    ClickHouseError::Http,
                );
                return Err(error);
            }
        };
        let resp = check_response(resp, "Arrow query failed").await?;
        let bytes = resp.bytes().await?;
        if bytes.is_empty() {
            return Ok(Vec::new());
        }
        let cursor = Cursor::new(bytes);
        let reader = StreamReader::try_new(cursor, None)?;
        let batches: std::result::Result<Vec<_>, _> = reader.collect();
        let batches = batches?;
        debug!(
            batch_count = batches.len(),
            "DynamicClient SELECT Arrow complete"
        );
        Ok(batches)
    }

    pub fn query_arrow_stream(
        &self,
        sql: &str,
    ) -> mpsc::Receiver<std::result::Result<RecordBatch, ClickHouseError>> {
        let full_sql = format!("{sql} FORMAT ArrowStream");
        let http = self.http.clone();
        let base_urls = self.select_base_urls();
        let user = self.user.clone();
        let password = self.password.clone();
        debug!(sql = sql, "DynamicClient SELECT Arrow (streaming)");
        let (tx, rx) = mpsc::channel::<std::result::Result<RecordBatch, ClickHouseError>>(32);

        spawn_traced("clickhouse-arrow-stream", async move {
            let mut last_error = None;
            let mut resp = None;
            for (index, base_url) in base_urls.iter().enumerate() {
                match Self::send_select_to(&http, base_url, &user, &password, full_sql.clone())
                    .await
                {
                    Ok(response) => {
                        resp = Some(response);
                        break;
                    }
                    Err(error)
                        if select_send_error_is_retryable(&error)
                            && index + 1 < base_urls.len() =>
                    {
                        tracing::warn!(error = %error, failed_clickhouse_url = %base_url, next_clickhouse_url = %base_urls[index + 1], "retrying fallback host");
                        last_error = Some(error);
                    }
                    Err(error) => {
                        let _ = tx.send(Err(ClickHouseError::Http(error))).await;
                        return;
                    }
                }
            }
            let Some(resp) = resp else {
                let _ = tx
                    .send(Err(last_error.map_or_else(
                        || {
                            ClickHouseError::Other(
                                "all ClickHouse URLs exhausted with retryable errors".into(),
                            )
                        },
                        ClickHouseError::Http,
                    )))
                    .await;
                return;
            };
            let resp = match check_response(resp, "Arrow query failed").await {
                Ok(r) => r,
                Err(e) => {
                    let _ = tx.send(Err(e)).await;
                    return;
                }
            };
            let mut decoder = StreamDecoder::new();
            let mut byte_stream = resp.bytes_stream();
            while let Some(chunk_result) = byte_stream.next().await {
                let chunk = match chunk_result {
                    Ok(bytes) => bytes,
                    Err(e) => {
                        let _ = tx.send(Err(ClickHouseError::Http(e))).await;
                        return;
                    }
                };
                let mut buf = Buffer::from(chunk);
                while !buf.is_empty() {
                    match decoder.decode(&mut buf) {
                        Ok(Some(batch)) => {
                            if tx.send(Ok(batch)).await.is_err() {
                                return;
                            }
                        }
                        Ok(None) => {}
                        Err(e) => {
                            let _ = tx.send(Err(ClickHouseError::Arrow(e))).await;
                            return;
                        }
                    }
                }
            }
            if let Err(e) = decoder.finish() {
                let _ = tx.send(Err(ClickHouseError::Arrow(e))).await;
            } else {
                debug!("DynamicClient SELECT Arrow streaming complete");
            }
        });

        rx
    }

    #[deprecated(note = "Use query_arrow for better performance")]
    pub async fn query_json(&self, sql: &str) -> Result<String> {
        let full_sql = format!("{sql} FORMAT JSONEachRow");
        debug!(sql = sql, "DynamicClient SELECT");
        let urls = self.select_base_urls();
        let mut last_error = None;
        let mut resp = None;
        for (index, base_url) in urls.iter().enumerate() {
            match Self::send_select_to(
                &self.http,
                base_url,
                &self.user,
                &self.password,
                full_sql.clone(),
            )
            .await
            {
                Ok(response) => {
                    resp = Some(response);
                    break;
                }
                Err(error) if select_send_error_is_retryable(&error) && index + 1 < urls.len() => {
                    tracing::warn!(error = %error, failed_clickhouse_url = %base_url, next_clickhouse_url = %urls[index + 1], "retrying fallback host");
                    last_error = Some(error);
                }
                Err(error) => return Err(ClickHouseError::Http(error)),
            }
        }
        let resp = match resp {
            Some(resp) => resp,
            None => {
                let error = last_error.map_or_else(
                    || {
                        ClickHouseError::Other(
                            "all ClickHouse URLs exhausted with retryable errors".into(),
                        )
                    },
                    ClickHouseError::Http,
                );
                return Err(error);
            }
        };
        let resp = check_response(resp, "Query failed").await?;
        Ok(resp.text().await?)
    }

    pub async fn execute(&self, sql: &str) -> Result<()> {
        debug!(sql = sql, "DynamicClient EXECUTE");
        let resp = self
            .http
            .post(&self.base_url)
            .basic_auth(&self.user, Some(self.password.expose_secret()))
            .body(sql.to_owned())
            .send()
            .await?;
        check_response(resp, "Execute failed").await?;
        Ok(())
    }
}

/// Cast `Utf8View` → `Utf8` and `BinaryView` → `Binary` columns.
pub fn downcast_view_types(
    batches: &[RecordBatch],
) -> std::result::Result<Cow<'_, [RecordBatch]>, arrow::error::ArrowError> {
    let first = match batches.first() {
        Some(b) => b,
        None => return Ok(Cow::Borrowed(batches)),
    };
    let schema = first.schema();
    let cast_targets: Vec<(usize, DataType)> = schema
        .fields()
        .iter()
        .enumerate()
        .filter_map(|(i, f)| match f.data_type() {
            DataType::Utf8View => Some((i, DataType::Utf8)),
            DataType::BinaryView => Some((i, DataType::Binary)),
            _ => None,
        })
        .collect();
    if cast_targets.is_empty() {
        return Ok(Cow::Borrowed(batches));
    }
    let new_fields: Vec<Arc<Field>> = schema
        .fields()
        .iter()
        .enumerate()
        .map(|(i, f)| {
            if let Some((_, target)) = cast_targets.iter().find(|(idx, _)| *idx == i) {
                Arc::new(Field::new(f.name(), target.clone(), f.is_nullable()))
            } else {
                Arc::clone(f)
            }
        })
        .collect();
    let new_schema = Arc::new(Schema::new(new_fields));
    let out = batches
        .iter()
        .map(|batch| {
            let new_columns: Vec<Arc<dyn Array>> = (0..batch.num_columns())
                .map(|i| {
                    if let Some((_, target)) = cast_targets.iter().find(|(idx, _)| *idx == i) {
                        cast(batch.column(i), target)
                    } else {
                        Ok(Arc::clone(batch.column(i)))
                    }
                })
                .collect::<std::result::Result<_, _>>()?;
            RecordBatch::try_new(Arc::clone(&new_schema), new_columns)
        })
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(Cow::Owned(out))
}

fn spawn_traced(
    name: &'static str,
    future: impl std::future::Future<Output = ()> + Send + 'static,
) {
    use std::panic::AssertUnwindSafe;
    tokio::spawn(async move {
        if let Err(payload) = AssertUnwindSafe(future).catch_unwind().await {
            tracing::error!(task = name, panic = %panic_message(payload.as_ref()), "background task panicked");
        }
    });
}

fn panic_message(payload: &(dyn std::any::Any + Send)) -> std::borrow::Cow<'static, str> {
    if let Some(message) = payload.downcast_ref::<&'static str>() {
        std::borrow::Cow::Borrowed(*message)
    } else if let Some(message) = payload.downcast_ref::<String>() {
        std::borrow::Cow::Owned(message.clone())
    } else {
        std::borrow::Cow::Borrowed("<non-string panic payload>")
    }
}
