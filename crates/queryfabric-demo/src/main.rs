//! QueryFabric self-host demonstrator.
//!
//! One binary wiring the extracted crates into a runnable service: portable
//! query compilation (`queryfabric` + the Postgres adapter), provenance,
//! access control, GDPR data rights, portable export bundles, DOI minting,
//! object storage, and an optional federation identity — over a generic
//! air-quality dataset, proving none of it is tied to the domain QueryFabric
//! was extracted from.

mod config;
mod dataset;
mod db;
mod http;
mod sovereignty;

use std::sync::Arc;

use queryfabric_namespace_uuid::NamespacedIds;
use queryfabric_store::{ObjectStore, S3Config};
use queryfabric_tenancy::{Account, AccountKind, InMemoryOwnership};
use tracing_subscriber::EnvFilter;

use crate::config::{DemoConfig, StoreConfig, parse_credentials_file};
use crate::dataset::{AccountIds, STATIONS};
use crate::db::Database;
use crate::http::AppState;

fn init_store(config: &StoreConfig) -> Result<ObjectStore, Box<dyn std::error::Error>> {
    match config {
        StoreConfig::Memory => {
            tracing::warn!("store backend 'memory' is not durable; use 's3' in production");
            Ok(ObjectStore::memory())
        }
        StoreConfig::S3 {
            endpoint,
            bucket,
            region,
            credentials_file,
        } => {
            let credentials = parse_credentials_file(credentials_file)?;
            Ok(ObjectStore::s3(S3Config {
                bucket: bucket.clone(),
                endpoint: Some(endpoint.clone()),
                region: Some(region.clone()),
                access_key_id: credentials.access_key_id,
                secret_access_key: credentials.secret_access_key,
                root: None,
            })?)
        }
    }
}

/// Seed the in-memory tenancy registry: one operator account owning every
/// station.
fn seed_ownership() -> (InMemoryOwnership, uuid::Uuid) {
    let ownership = InMemoryOwnership::new();
    let operator = AccountIds::from_str_key("operator");
    ownership.add_account(Account {
        id: operator,
        email: "operator@example.org".to_owned(),
        active: true,
        verified: true,
        kind: AccountKind::Human,
    });
    for station in &STATIONS {
        ownership.set_owner(station.resource(), operator);
    }
    (ownership, operator)
}

async fn wait_for_database(db: &Database, wait_secs: u64) -> Result<(), db::DbError> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(wait_secs);
    loop {
        match db.ping().await {
            Ok(()) => return Ok(()),
            Err(error) if std::time::Instant::now() < deadline => {
                tracing::info!(%error, "waiting for postgres");
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
            Err(error) => return Err(error),
        }
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.ok();
    };
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(_) => std::future::pending().await,
        }
    };
    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
    tracing::info!("shutdown signal received");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let config = DemoConfig::from_env()?;
    tracing::info!(?config, "starting queryfabric demonstrator");

    let db = Database::new(config.database_url.clone());
    wait_for_database(&db, config.db_wait_secs).await?;
    db.seed().await?;

    let store = init_store(&config.store)?;
    let provenance = queryfabric_provenance::VecProvenanceStore::new();
    sovereignty::seed_provenance(&provenance).await?;
    let (ownership, operator) = seed_ownership();

    let listen_addr = config.listen_addr;
    let federation_enabled = config.federation.enable;
    let state = Arc::new(AppState {
        config,
        db,
        store,
        catalog: dataset::build_catalog(),
        provenance,
        ownership,
        operator,
    });

    if federation_enabled {
        tracing::info!("federation identity enabled; see GET /federation/status");
    }

    let listener = tokio::net::TcpListener::bind(listen_addr).await?;
    tracing::info!(%listen_addr, "listening");
    axum::serve(listener, http::router(state))
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}
