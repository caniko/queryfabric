//! Postgres access for the demonstrator host.
//!
//! QueryFabric compiles queries; executing them is host work (DECISIONS.md
//! D003), and the demonstrator is its own host. Connections are opened per
//! operation: the demonstrator favours self-healing across database
//! restarts over connection-pool throughput.

use chrono::{DateTime, Duration, NaiveDate, NaiveDateTime, Utc};
use queryfabric::{DataType, ParameterBinding, ParameterValue};
use serde_json::{Map, Value, json};
use tokio_postgres::types::{ToSql, Type};
use tokio_postgres::{Client, NoTls, Row};
use uuid::Uuid;

use crate::config::ImportFailureStage;
use crate::dataset::{READINGS_PER_STATION, SEED_EPOCH_MS, STATIONS, readings_for};

/// Database errors, kept close to the failing statement.
#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("postgres connection failed: {0}")]
    Connect(#[source] tokio_postgres::Error),
    #[error("postgres statement failed: {0}")]
    Statement(#[from] tokio_postgres::Error),
    #[error("import row is invalid: {0}")]
    InvalidImport(String),
    #[error("query parameter is invalid: {0}")]
    InvalidQueryParameter(String),
    #[error("query result exceeds {kind} limit ({actual} > {limit})")]
    QueryLimit {
        kind: &'static str,
        actual: usize,
        limit: usize,
    },
    #[error("query result serialization failed: {0}")]
    QuerySerialization(#[source] serde_json::Error),
    #[error("existing import receipt does not match the requested plan ({0})")]
    ConflictingReplay(String),
    #[error("import transaction failure injected at {0}")]
    InjectedFailure(&'static str),
}

/// Handle on the metadata/query database.
#[derive(Clone)]
pub struct Database {
    migration_url: String,
    query_url: String,
    import_url: String,
}

/// Inputs to one atomic import commit.
pub struct ImportCommit<'a> {
    pub station_id: uuid::Uuid,
    pub rows: &'a [Vec<String>],
    pub bundle_digest: &'a str,
    pub plan_digest: &'a str,
    pub target_revision: &'a str,
    pub source_resource: &'a Value,
    pub target_resource: &'a Value,
    pub local_owner: uuid::Uuid,
    pub local_policy: &'a Value,
    pub source_evidence: &'a Value,
    pub mapping: &'a Value,
    pub byte_count: u64,
    pub failure_stage: Option<ImportFailureStage>,
}

impl Database {
    /// Build a database handle with explicit least-privilege connection URLs.
    #[must_use]
    pub fn new_with_roles(migration_url: String, query_url: String, import_url: String) -> Self {
        Self {
            migration_url,
            query_url,
            import_url,
        }
    }

    async fn client_for(url: &str) -> Result<Client, DbError> {
        let (client, connection) = tokio_postgres::connect(url, NoTls)
            .await
            .map_err(DbError::Connect)?;
        tokio::spawn(async move {
            if let Err(error) = connection.await {
                tracing::warn!(%error, "postgres connection task ended");
            }
        });
        Ok(client)
    }

    /// Cheap readiness probe.
    pub async fn ping(&self) -> Result<(), DbError> {
        let client = Self::client_for(&self.query_url).await?;
        client.simple_query("SELECT 1").await?;
        Ok(())
    }

    /// Create the demonstration schema and seed it. Idempotent: tables use
    /// `IF NOT EXISTS`, rows use deterministic ids with `ON CONFLICT DO
    /// NOTHING`.
    pub async fn seed(&self) -> Result<(), DbError> {
        self.seed_schema().await?;
        let client = Self::client_for(&self.migration_url).await?;
        let insert_reading = client
            .prepare(
                "INSERT INTO readings (reading_id, station_id, measured_at, pm25, no2, ozone)
                 VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT (reading_id) DO NOTHING",
            )
            .await?;

        let epoch = seed_epoch_naive();
        for station in &STATIONS {
            for reading in readings_for(station) {
                let measured_at = epoch + Duration::hours(i64::from(reading.hour_offset));
                client
                    .execute(
                        &insert_reading,
                        &[
                            &reading.id,
                            &station.id(),
                            &measured_at,
                            &reading.pm25,
                            &reading.no2,
                            &reading.ozone,
                        ],
                    )
                    .await?;
            }
        }
        tracing::info!(
            stations = STATIONS.len(),
            readings_per_station = READINGS_PER_STATION,
            "demo dataset seeded"
        );
        Ok(())
    }

    /// Create the schema and predeclare station targets without inserting
    /// measurement rows. This is the production-safe target mode used by the
    /// migration proof.
    pub async fn seed_schema(&self) -> Result<(), DbError> {
        let client = Self::client_for(&self.migration_url).await?;
        client
            .batch_execute(
                "CREATE TABLE IF NOT EXISTS stations (
                     station_id UUID PRIMARY KEY,
                     code TEXT NOT NULL UNIQUE,
                     name TEXT NOT NULL,
                     city TEXT NOT NULL,
                     latitude DOUBLE PRECISION NOT NULL,
                     longitude DOUBLE PRECISION NOT NULL
                 );
                 CREATE TABLE IF NOT EXISTS readings (
                     reading_id UUID PRIMARY KEY,
                     station_id UUID NOT NULL REFERENCES stations(station_id),
                     measured_at TIMESTAMP NOT NULL,
                     pm25 DOUBLE PRECISION NOT NULL,
                     no2 DOUBLE PRECISION NOT NULL,
                     ozone DOUBLE PRECISION NOT NULL
                 );",
            )
            .await?;
        self.ensure_import_tables().await?;
        let insert_station = client
            .prepare(
                "INSERT INTO stations (station_id, code, name, city, latitude, longitude)
                 VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT (station_id) DO NOTHING",
            )
            .await?;
        for station in &STATIONS {
            client
                .execute(
                    &insert_station,
                    &[
                        &station.id(),
                        &station.code,
                        &station.name,
                        &station.city,
                        &station.latitude,
                        &station.longitude,
                    ],
                )
                .await?;
        }
        Ok(())
    }

    /// Create the durable import receipt/evidence table.  This migration is
    /// independent of demonstration-data seeding and is safe to run on an
    /// initially empty target.
    pub async fn ensure_import_tables(&self) -> Result<(), DbError> {
        let client = Self::client_for(&self.migration_url).await?;
        client
            .batch_execute(
                "CREATE TABLE IF NOT EXISTS queryfabric_import_receipts (
                     receipt_id UUID PRIMARY KEY,
                     idempotency_key TEXT NOT NULL UNIQUE,
                     bundle_digest TEXT NOT NULL,
                     plan_digest TEXT NOT NULL,
                     source_resource JSONB NOT NULL,
                     target_resource JSONB NOT NULL,
                     target_relation TEXT NOT NULL,
                     target_revision TEXT NOT NULL,
                     local_owner UUID NOT NULL,
                     local_policy JSONB NOT NULL,
                     source_evidence JSONB NOT NULL,
                     mapping JSONB NOT NULL,
                     receipt_json JSONB NOT NULL,
                     row_count BIGINT NOT NULL,
                     byte_count BIGINT NOT NULL,
                     created_at TIMESTAMPTZ NOT NULL DEFAULT now()
                 );",
            )
            .await?;
        Ok(())
    }

    /// Atomically persist imported rows and their local receipt/evidence.
    /// Replaying the same bundle and target mapping returns the original
    /// receipt without inserting another set of rows.
    pub async fn apply_import(&self, request: ImportCommit<'_>) -> Result<(Value, bool), DbError> {
        let ImportCommit {
            station_id,
            rows,
            bundle_digest,
            plan_digest,
            target_revision,
            source_resource,
            target_resource,
            local_owner,
            local_policy,
            source_evidence,
            mapping,
            byte_count,
            failure_stage,
        } = request;
        let idempotency_key = format!("{bundle_digest}:{station_id}:{target_revision}");
        let mut client = Self::client_for(&self.import_url).await?;
        let transaction = client.transaction().await?;
        if let Some(existing) = transaction
            .query_opt(
                "SELECT receipt_json, plan_digest, bundle_digest, local_owner, mapping,
                        byte_count, target_revision
                   FROM queryfabric_import_receipts
                  WHERE idempotency_key = $1",
                &[&idempotency_key],
            )
            .await?
        {
            let existing_plan: String = existing.try_get(1)?;
            let existing_bundle: String = existing.try_get(2)?;
            let existing_owner: Uuid = existing.try_get(3)?;
            let existing_mapping: Value = existing.try_get(4)?;
            let existing_bytes: i64 = existing.try_get(5)?;
            let existing_revision: String = existing.try_get(6)?;
            if existing_plan != plan_digest
                || existing_bundle != bundle_digest
                || existing_owner != local_owner
                || existing_mapping != mapping.clone()
                || existing_bytes != byte_count as i64
                || existing_revision != target_revision
            {
                return Err(DbError::ConflictingReplay(idempotency_key));
            }
            let value: Value = existing.try_get(0)?;
            return Ok((value, true));
        }

        if failure_stage == Some(ImportFailureStage::BeforeRows) {
            return Err(DbError::InjectedFailure("before-rows"));
        }

        let insert = transaction
            .prepare(
                "INSERT INTO readings (reading_id, station_id, measured_at, pm25, no2, ozone)
                 VALUES ($1, $2, $3, $4, $5, $6)
                 ON CONFLICT (reading_id) DO NOTHING",
            )
            .await?;
        for (ordinal, row) in rows.iter().enumerate() {
            if row.len() != 4 {
                return Err(DbError::InvalidImport(
                    "expected four readings fields".to_owned(),
                ));
            }
            let measured_at = DateTime::parse_from_rfc3339(&row[0])
                .map_err(|error| DbError::InvalidImport(format!("timestamp: {error}")))?
                .naive_utc();
            let pm25 = row[1]
                .parse::<f64>()
                .map_err(|error| DbError::InvalidImport(format!("pm25: {error}")))?;
            let no2 = row[2]
                .parse::<f64>()
                .map_err(|error| DbError::InvalidImport(format!("no2: {error}")))?;
            let ozone = row[3]
                .parse::<f64>()
                .map_err(|error| DbError::InvalidImport(format!("ozone: {error}")))?;
            transaction
                .execute(
                    &insert,
                    &[
                        &Uuid::now_v7(),
                        &station_id,
                        &measured_at,
                        &pm25,
                        &no2,
                        &ozone,
                    ],
                )
                .await
                .map_err(|error| {
                    DbError::InvalidImport(format!("row {ordinal} could not be inserted: {error}"))
                })?;
            if failure_stage == Some(ImportFailureStage::DuringRows) && ordinal == 0 {
                return Err(DbError::InjectedFailure("during-rows"));
            }
        }
        let receipt_id = Uuid::now_v7();
        let receipt = json!({
            "receiptId": receipt_id,
            "idempotencyKey": idempotency_key,
            "bundleDigest": bundle_digest,
            "planDigest": plan_digest,
            "sourceResource": source_resource,
            "targetResource": target_resource,
            "targetRelation": "readings",
            "targetRevision": target_revision,
            "localOwner": local_owner,
            "localPolicy": local_policy,
            "sourceEvidence": source_evidence,
            "mapping": mapping,
            "rowCount": rows.len(),
            "byteCount": byte_count,
            "event": "Imported",
        });
        transaction
            .execute(
                "INSERT INTO queryfabric_import_receipts
                 (receipt_id, idempotency_key, bundle_digest, plan_digest,
                  source_resource, target_resource, target_relation, target_revision,
                  local_owner, local_policy, source_evidence, mapping, receipt_json, row_count, byte_count)
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15)",
                &[
                    &receipt_id,
                    &idempotency_key,
                    &bundle_digest,
                    &plan_digest,
                    source_resource,
                    target_resource,
                    &"readings",
                    &target_revision,
                    &local_owner,
                    local_policy,
                    source_evidence,
                    mapping,
                    &receipt,
                    &(rows.len() as i64),
                    &(byte_count as i64),
                ],
            )
            .await?;
        if failure_stage == Some(ImportFailureStage::BeforeCommit) {
            return Err(DbError::InjectedFailure("before-commit"));
        }
        transaction.commit().await?;
        Ok((receipt, false))
    }

    /// Execute backend SQL and return column names plus JSON rows.
    pub async fn execute(&self, sql: &str) -> Result<(Vec<String>, Vec<Value>), DbError> {
        let client = Self::client_for(&self.query_url).await?;
        let statement = client.prepare(sql).await?;
        let columns: Vec<String> = statement
            .columns()
            .iter()
            .map(|column| column.name().to_owned())
            .collect();
        let rows = client.query(&statement, &[]).await?;
        let json_rows = rows
            .iter()
            .map(|row| {
                let mut object = Map::with_capacity(columns.len());
                for (index, name) in columns.iter().enumerate() {
                    object.insert(name.clone(), cell_to_json(row, index));
                }
                Value::Object(object)
            })
            .collect();
        Ok((columns, json_rows))
    }

    /// Execute emitted PostgreSQL SQL with the binder's typed parameter
    /// contract. The outer LIMIT is host policy: a user query cannot bypass
    /// the demonstrator's bounded result surface. One extra row is fetched so
    /// callers can distinguish a complete result from an explicit truncation.
    pub async fn execute_query(
        &self,
        sql: &str,
        parameters: &[ParameterBinding],
        max_rows: usize,
        max_bytes: usize,
    ) -> Result<(Vec<String>, Vec<Value>, bool), DbError> {
        let fetch_rows = max_rows.saturating_add(1);
        let bounded_sql = format!("SELECT * FROM ({sql}) AS queryfabric_result LIMIT {fetch_rows}");
        let client = Self::client_for(&self.query_url).await?;
        let statement = client.prepare(&bounded_sql).await?;
        let owned_parameters = parameters
            .iter()
            .map(parameter_to_postgres)
            .collect::<Result<Vec<_>, _>>()?;
        let references: Vec<&(dyn ToSql + Sync)> = owned_parameters
            .iter()
            .map(|parameter| parameter.as_ref() as &(dyn ToSql + Sync))
            .collect();
        let rows = client.query(&statement, &references).await?;
        let columns: Vec<String> = statement
            .columns()
            .iter()
            .map(|column| column.name().to_owned())
            .collect();
        let mut json_rows = rows
            .iter()
            .map(|row| {
                let mut object = Map::with_capacity(columns.len());
                for (index, name) in columns.iter().enumerate() {
                    object.insert(name.clone(), cell_to_json(row, index));
                }
                Value::Object(object)
            })
            .collect::<Vec<_>>();
        let truncated = json_rows.len() > max_rows;
        if truncated {
            json_rows.truncate(max_rows);
        }
        let encoded = serde_json::to_vec(&json_rows).map_err(DbError::QuerySerialization)?;
        if encoded.len() > max_bytes {
            return Err(DbError::QueryLimit {
                kind: "bytes",
                actual: encoded.len(),
                limit: max_bytes,
            });
        }
        Ok((columns, json_rows, truncated))
    }
}

fn parameter_to_postgres(
    binding: &ParameterBinding,
) -> Result<Box<dyn ToSql + Sync + Send>, DbError> {
    let value = binding.value.as_ref().ok_or_else(|| {
        DbError::InvalidQueryParameter(format!(
            "{} has no supplied value",
            binding.schema.reference
        ))
    })?;
    let data_type = &binding.schema.data_type;
    match (data_type, value) {
        (DataType::Boolean, ParameterValue::Boolean(value)) => Ok(Box::new(*value)),
        (DataType::Int32, ParameterValue::Int64(value)) => {
            Ok(Box::new(i32::try_from(*value).map_err(|_| {
                DbError::InvalidQueryParameter(format!("{} is outside Int32", value))
            })?))
        }
        (DataType::Int64, ParameterValue::Int64(value)) => Ok(Box::new(*value)),
        (DataType::Float64, ParameterValue::Float64(value)) => {
            Ok(Box::new(value.parse::<f64>().map_err(|error| {
                DbError::InvalidQueryParameter(error.to_string())
            })?))
        }
        (DataType::Utf8, ParameterValue::Utf8(value)) => Ok(Box::new(value.clone())),
        (DataType::Uuid, ParameterValue::Uuid(value)) => {
            Ok(Box::new(value.parse::<Uuid>().map_err(|error| {
                DbError::InvalidQueryParameter(error.to_string())
            })?))
        }
        (DataType::Json, ParameterValue::Json(value)) => Ok(Box::new(
            serde_json::from_str::<Value>(value)
                .map_err(|error| DbError::InvalidQueryParameter(error.to_string()))?,
        )),
        (DataType::Date, ParameterValue::Utf8(value)) => {
            Ok(Box::new(value.parse::<NaiveDate>().map_err(|error| {
                DbError::InvalidQueryParameter(error.to_string())
            })?))
        }
        (DataType::Timestamp { timezone: None }, ParameterValue::Utf8(value)) => Ok(Box::new(
            DateTime::parse_from_rfc3339(value)
                .map_err(|error| DbError::InvalidQueryParameter(error.to_string()))?
                .naive_utc(),
        )),
        (DataType::Timestamp { timezone: Some(_) }, ParameterValue::Utf8(value)) => Ok(Box::new(
            DateTime::parse_from_rfc3339(value)
                .map_err(|error| DbError::InvalidQueryParameter(error.to_string()))?
                .with_timezone(&Utc),
        )),
        (DataType::Boolean, ParameterValue::Null) => Ok(Box::new(None::<bool>)),
        (DataType::Int32, ParameterValue::Null) => Ok(Box::new(None::<i32>)),
        (DataType::Int64, ParameterValue::Null) => Ok(Box::new(None::<i64>)),
        (DataType::Float64, ParameterValue::Null) => Ok(Box::new(None::<f64>)),
        (DataType::Utf8, ParameterValue::Null) => Ok(Box::new(None::<String>)),
        (DataType::Uuid, ParameterValue::Null) => Ok(Box::new(None::<Uuid>)),
        (DataType::Json, ParameterValue::Null) => Ok(Box::new(None::<Value>)),
        (DataType::Date, ParameterValue::Null) => Ok(Box::new(None::<NaiveDate>)),
        (DataType::Timestamp { timezone: None }, ParameterValue::Null) => {
            Ok(Box::new(None::<NaiveDateTime>))
        }
        (DataType::Timestamp { timezone: Some(_) }, ParameterValue::Null) => {
            Ok(Box::new(None::<DateTime<Utc>>))
        }
        (
            DataType::List(_) | DataType::Decimal { .. } | DataType::Struct(_) | DataType::Unknown,
            _,
        ) => Err(DbError::InvalidQueryParameter(format!(
            "{} cannot be executed by the demonstrator PostgreSQL adapter",
            binding.schema.reference
        ))),
        (_, value) => Err(DbError::InvalidQueryParameter(format!(
            "{} value {value:?} does not match {}",
            binding.schema.reference,
            data_type_name(data_type)
        ))),
    }
}

fn data_type_name(data_type: &DataType) -> &'static str {
    match data_type {
        DataType::Boolean => "Boolean",
        DataType::Int32 => "Int32",
        DataType::Int64 => "Int64",
        DataType::Float64 => "Float64",
        DataType::Utf8 => "Utf8",
        DataType::Uuid => "Uuid",
        DataType::Json => "Json",
        DataType::Date => "Date",
        DataType::Decimal { .. } => "Decimal",
        DataType::Timestamp { .. } => "Timestamp",
        DataType::List(_) => "List",
        DataType::Struct(_) => "Struct",
        DataType::Unknown => "Unknown",
        _ => "Unknown",
    }
}

fn seed_epoch_naive() -> NaiveDateTime {
    DateTime::<Utc>::from_timestamp_millis(SEED_EPOCH_MS)
        .map(|dt| dt.naive_utc())
        .unwrap_or_else(|| {
            NaiveDate::from_ymd_opt(2026, 1, 1)
                .expect("valid date")
                .and_hms_opt(0, 0, 0)
                .expect("valid time")
        })
}

fn float_to_json(value: f64) -> Value {
    serde_json::Number::from_f64(value).map_or(Value::Null, Value::Number)
}

/// Convert one result cell to JSON, by Postgres type. Types outside the
/// demonstrator's schema map to null rather than failing the whole row.
fn cell_to_json(row: &Row, index: usize) -> Value {
    let ty = row.columns()[index].type_();
    let converted: Result<Value, tokio_postgres::Error> = match *ty {
        Type::BOOL => row
            .try_get::<_, Option<bool>>(index)
            .map(|cell| cell.map_or(Value::Null, Value::Bool)),
        Type::INT2 => row
            .try_get::<_, Option<i16>>(index)
            .map(|cell| cell.map_or(Value::Null, |v| Value::Number(v.into()))),
        Type::INT4 => row
            .try_get::<_, Option<i32>>(index)
            .map(|cell| cell.map_or(Value::Null, |v| Value::Number(v.into()))),
        Type::INT8 => row
            .try_get::<_, Option<i64>>(index)
            .map(|cell| cell.map_or(Value::Null, |v| Value::Number(v.into()))),
        Type::FLOAT4 => row
            .try_get::<_, Option<f32>>(index)
            .map(|cell| cell.map_or(Value::Null, |v| float_to_json(f64::from(v)))),
        Type::FLOAT8 => row
            .try_get::<_, Option<f64>>(index)
            .map(|cell| cell.map_or(Value::Null, float_to_json)),
        Type::UUID => row
            .try_get::<_, Option<uuid::Uuid>>(index)
            .map(|cell| cell.map_or(Value::Null, |v| Value::String(v.to_string()))),
        Type::TIMESTAMP => row.try_get::<_, Option<NaiveDateTime>>(index).map(|cell| {
            cell.map_or(Value::Null, |v| {
                Value::String(v.format("%Y-%m-%dT%H:%M:%S").to_string())
            })
        }),
        Type::TIMESTAMPTZ => row
            .try_get::<_, Option<DateTime<Utc>>>(index)
            .map(|cell| cell.map_or(Value::Null, |v| Value::String(v.to_rfc3339()))),
        Type::DATE => row
            .try_get::<_, Option<NaiveDate>>(index)
            .map(|cell| cell.map_or(Value::Null, |v| Value::String(v.to_string()))),
        Type::JSON | Type::JSONB => row
            .try_get::<_, Option<Value>>(index)
            .map(|cell| cell.unwrap_or(Value::Null)),
        Type::TEXT | Type::VARCHAR | Type::BPCHAR | Type::NAME => row
            .try_get::<_, Option<String>>(index)
            .map(|cell| cell.map_or(Value::Null, Value::String)),
        _ => {
            tracing::debug!(pg_type = %ty, "unmapped result type rendered as null");
            Ok(Value::Null)
        }
    };
    converted.unwrap_or_else(|error| {
        tracing::warn!(%error, pg_type = %ty, "failed to decode result cell");
        Value::Null
    })
}
