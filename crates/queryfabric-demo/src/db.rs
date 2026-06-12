//! Postgres access for the demonstrator host.
//!
//! QueryFabric compiles queries; executing them is host work (DECISIONS.md
//! D003), and the demonstrator is its own host. Connections are opened per
//! operation: the demonstrator favours self-healing across database
//! restarts over connection-pool throughput.

use chrono::{DateTime, Duration, NaiveDate, NaiveDateTime, Utc};
use serde_json::{Map, Value};
use tokio_postgres::types::Type;
use tokio_postgres::{Client, NoTls, Row};

use crate::dataset::{READINGS_PER_STATION, SEED_EPOCH_MS, STATIONS, readings_for};

/// Database errors, kept close to the failing statement.
#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("postgres connection failed: {0}")]
    Connect(#[source] tokio_postgres::Error),
    #[error("postgres statement failed: {0}")]
    Statement(#[from] tokio_postgres::Error),
}

/// Handle on the metadata/query database.
#[derive(Clone)]
pub struct Database {
    url: String,
}

impl Database {
    #[must_use]
    pub fn new(url: String) -> Self {
        Self { url }
    }

    async fn client(&self) -> Result<Client, DbError> {
        let (client, connection) = tokio_postgres::connect(&self.url, NoTls)
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
        let client = self.client().await?;
        client.simple_query("SELECT 1").await?;
        Ok(())
    }

    /// Create the demonstration schema and seed it. Idempotent: tables use
    /// `IF NOT EXISTS`, rows use deterministic ids with `ON CONFLICT DO
    /// NOTHING`.
    pub async fn seed(&self) -> Result<(), DbError> {
        let client = self.client().await?;
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

        let insert_station = client
            .prepare(
                "INSERT INTO stations (station_id, code, name, city, latitude, longitude)
                 VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT (station_id) DO NOTHING",
            )
            .await?;
        let insert_reading = client
            .prepare(
                "INSERT INTO readings (reading_id, station_id, measured_at, pm25, no2, ozone)
                 VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT (reading_id) DO NOTHING",
            )
            .await?;

        let epoch = seed_epoch_naive();
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

    /// Execute backend SQL and return column names plus JSON rows.
    pub async fn execute(&self, sql: &str) -> Result<(Vec<String>, Vec<Value>), DbError> {
        let client = self.client().await?;
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
        Type::TIMESTAMP => row
            .try_get::<_, Option<NaiveDateTime>>(index)
            .map(|cell| {
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
