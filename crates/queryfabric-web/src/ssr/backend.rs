use std::sync::Arc;

use http::HeaderMap;
use http::header::{AUTHORIZATION, CONTENT_TYPE, COOKIE};
use reqwest::{Client, RequestBuilder};
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::settings::SsrSettings;

/// Generic SSR-sidecar API client.
///
/// Forwards cookies and request IDs from the incoming SSR request to the
/// backend API. All endpoint paths are resolved through [`SsrSettings`].
#[derive(Clone)]
pub struct ApiClient {
    client: Client,
    settings: Arc<SsrSettings>,
    request_id_header: &'static http::HeaderName,
    form_boundary_prefix: String,
}

impl ApiClient {
    /// Create a new client. `request_id_header` is the header name to forward
    /// (e.g. `"x-request-id"`), and `form_boundary_prefix` is the prefix for
    /// multipart form boundary strings (e.g. `"myapp-archive-"`).
    pub fn new(
        settings: Arc<SsrSettings>,
        user_agent: &str,
        request_id_header: &'static http::HeaderName,
        form_boundary_prefix: &str,
    ) -> Result<Self, reqwest::Error> {
        let client = Client::builder()
            .timeout(settings.api_timeout)
            .user_agent(user_agent)
            .build()?;
        Ok(Self {
            client,
            settings,
            request_id_header,
            form_boundary_prefix: form_boundary_prefix.to_owned(),
        })
    }

    pub async fn get_json<T>(&self, path: &str, headers: &HeaderMap) -> Result<Option<T>, ApiError>
    where
        T: DeserializeOwned,
    {
        let req = self.forward_headers(self.client.get(self.settings.api_endpoint(path)), headers);
        let resp = req.send().await?;
        if resp.status() == reqwest::StatusCode::UNAUTHORIZED
            || resp.status() == reqwest::StatusCode::FORBIDDEN
            || resp.status() == reqwest::StatusCode::NOT_FOUND
        {
            return Ok(None);
        }
        if !resp.status().is_success() {
            return Err(ApiError::Status(resp.status().as_u16()));
        }
        Ok(Some(resp.json::<T>().await?))
    }

    pub async fn get_json_required<T>(&self, path: &str, headers: &HeaderMap) -> Result<T, ApiError>
    where
        T: DeserializeOwned,
    {
        let req = self.forward_headers(self.client.get(self.settings.api_endpoint(path)), headers);
        let resp = req.send().await?;
        if !resp.status().is_success() {
            return Err(ApiError::Status(resp.status().as_u16()));
        }
        Ok(resp.json::<T>().await?)
    }

    pub async fn post_json(
        &self,
        path: &str,
        body: &impl Serialize,
        headers: &HeaderMap,
    ) -> Result<reqwest::Response, ApiError> {
        let req = self.forward_headers(
            self.client
                .post(self.settings.api_endpoint(path))
                .json(body),
            headers,
        );
        Ok(req.send().await?)
    }

    pub async fn post_empty_bearer(
        &self,
        path: &str,
        token: &str,
        headers: &HeaderMap,
    ) -> Result<reqwest::Response, ApiError> {
        let req = self
            .forward_headers(self.client.post(self.settings.api_endpoint(path)), headers)
            .header(AUTHORIZATION, format!("Bearer {token}"));
        Ok(req.send().await?)
    }

    pub async fn post_multipart_bytes(
        &self,
        path: &str,
        field_name: &str,
        filename: &str,
        bytes: Vec<u8>,
        headers: &HeaderMap,
    ) -> Result<reqwest::Response, ApiError> {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let boundary = format!("{}{nonce}", self.form_boundary_prefix);
        let mut body = Vec::new();
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        body.extend_from_slice(
            format!(
                "Content-Disposition: form-data; name=\"{field_name}\"; filename=\"{}\"\r\n",
                filename.replace('"', "_")
            )
            .as_bytes(),
        );
        body.extend_from_slice(b"Content-Type: application/octet-stream\r\n\r\n");
        body.extend_from_slice(&bytes);
        body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

        let req = self
            .forward_headers(self.client.post(self.settings.api_endpoint(path)), headers)
            .header(
                CONTENT_TYPE,
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(body);
        Ok(req.send().await?)
    }

    pub async fn get_raw(
        &self,
        path: &str,
        headers: &HeaderMap,
    ) -> Result<reqwest::Response, ApiError> {
        let req = self.forward_headers(self.client.get(self.settings.api_endpoint(path)), headers);
        Ok(req.send().await?)
    }

    pub async fn api_health(&self) -> Result<reqwest::Response, ApiError> {
        Ok(self
            .client
            .get(self.settings.api_origin_endpoint("/health"))
            .send()
            .await?)
    }

    fn forward_headers(&self, mut req: RequestBuilder, headers: &HeaderMap) -> RequestBuilder {
        if let Some(cookie) = headers.get(COOKIE) {
            req = req.header(COOKIE, cookie);
        }
        if let Some(request_id) = headers.get(self.request_id_header) {
            req = req.header(self.request_id_header, request_id);
        }
        req
    }
}

#[derive(Debug)]
pub enum ApiError {
    Http(reqwest::Error),
    Status(u16),
}

impl From<reqwest::Error> for ApiError {
    fn from(error: reqwest::Error) -> Self {
        Self::Http(error)
    }
}

impl ApiError {
    pub fn user_message(&self) -> String {
        match self {
            Self::Http(error) => format!("API request failed: {error}"),
            Self::Status(status) => format!("API returned HTTP {status}"),
        }
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.user_message())
    }
}

impl std::error::Error for ApiError {}
