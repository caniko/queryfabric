//! Axum routes: portable query API plus the sovereignty endpoints.

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use queryfabric::{
    EmitArtifact, GenericSqlDialect, MemoryCatalog, PostgresAdapter, QueryCompiler,
    QueryFabricError, QueryParameters,
};
use queryfabric_access::{DataRights, evaluate_access};
use queryfabric_contract::{AccessOutcome, AccessPolicy, Subject};
use queryfabric_federation::ClusterIdentity;
use queryfabric_provenance::VecProvenanceStore;
use queryfabric_store::ObjectStore;
use queryfabric_tenancy::InMemoryOwnership;
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::config::DemoConfig;
use crate::dataset::{self, STATIONS, StationSpec};
use crate::db::Database;
use queryfabric_portability::DoiProvider as _;

use crate::sovereignty::{self, LocalDoiProvider, now_unix_ms, station_policy};

/// Shared service state.
pub struct AppState {
    pub config: DemoConfig,
    pub db: Database,
    pub store: ObjectStore,
    pub catalog: MemoryCatalog,
    pub provenance: VecProvenanceStore,
    pub ownership: InMemoryOwnership,
    pub operator: Uuid,
}

type SharedState = Arc<AppState>;

/// API error → JSON problem response.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("service dependency failed: {0}")]
    Dependency(String),
}

impl ApiError {
    fn status(&self) -> StatusCode {
        match self {
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
            Self::Dependency(_) => StatusCode::BAD_GATEWAY,
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.status();
        let body = Json(json!({
            "status": status.as_u16(),
            "error": self.to_string(),
        }));
        (status, body).into_response()
    }
}

impl From<crate::db::DbError> for ApiError {
    fn from(error: crate::db::DbError) -> Self {
        Self::Dependency(error.to_string())
    }
}

impl From<queryfabric_store::StoreError> for ApiError {
    fn from(error: queryfabric_store::StoreError) -> Self {
        Self::Dependency(error.to_string())
    }
}

impl From<queryfabric_provenance::ProvenanceError> for ApiError {
    fn from(error: queryfabric_provenance::ProvenanceError) -> Self {
        Self::Dependency(error.to_string())
    }
}

impl From<sovereignty::ExportError> for ApiError {
    fn from(error: sovereignty::ExportError) -> Self {
        Self::Dependency(error.to_string())
    }
}

impl From<QueryFabricError> for ApiError {
    fn from(error: QueryFabricError) -> Self {
        Self::BadRequest(error.to_string())
    }
}

/// Build the demonstrator router.
pub fn router(state: SharedState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/healthz", get(healthz))
        .route("/catalog", get(catalog))
        .route("/resources", get(resources))
        .route("/query", post(query))
        .route("/resources/{id}/export", post(export))
        .route("/resources/{id}/bundle", get(bundle))
        .route("/resources/{id}/access-export", get(access_export))
        .route("/resources/{id}/erase", post(erase))
        .route("/resources/{id}/doi", post(mint_doi))
        .route("/federation/status", get(federation_status))
        .with_state(state)
}

async fn index() -> Html<&'static str> {
    Html(include_str!("index.html"))
}

async fn healthz(State(state): State<SharedState>) -> Result<Json<Value>, ApiError> {
    state.db.ping().await?;
    Ok(Json(json!({ "status": "ok" })))
}

async fn catalog(State(state): State<SharedState>) -> Json<Value> {
    Json(serde_json::to_value(state.catalog.to_document()).unwrap_or_else(|_| json!({})))
}

async fn resources() -> Json<Value> {
    let stations: Vec<Value> = STATIONS
        .iter()
        .map(|station| {
            json!({
                "id": station.id().to_string(),
                "code": station.code,
                "name": station.name,
                "city": station.city,
                "resource": station.resource(),
                "policy": station_policy(),
            })
        })
        .collect();
    Json(json!({ "namespace": dataset::DEMO_NAMESPACE.to_string(), "resources": stations }))
}

#[derive(Deserialize)]
struct QueryRequest {
    sql: String,
}

/// Compile a portable query against the catalog, emit Postgres SQL, execute
/// it, and return the rows. The compiler validates against the catalog, so
/// only the two demo relations are reachable.
async fn query(
    State(state): State<SharedState>,
    Json(request): Json<QueryRequest>,
) -> Result<Json<Value>, ApiError> {
    let compiler = QueryCompiler::new();
    let parsed = compiler.parse(&GenericSqlDialect, &request.sql)?;
    let bound = compiler.bind_and_validate(&parsed, &state.catalog, &QueryParameters::default())?;
    let artifact = compiler.emit(&bound, &PostgresAdapter, &state.catalog)?;
    let EmitArtifact::Sql(sql) = artifact else {
        return Err(ApiError::Dependency(
            "postgres adapter emitted a non-SQL artifact".to_owned(),
        ));
    };
    let (columns, rows) = state.db.execute(&sql.text).await?;
    Ok(Json(json!({
        "requestedSql": request.sql,
        "backendDialect": sql.dialect,
        "backendSql": sql.text,
        "snapshotId": dataset::SNAPSHOT_ID,
        "columns": columns,
        "rowCount": rows.len(),
        "rows": rows,
    })))
}

fn lookup_station(id: &str) -> Result<&'static StationSpec, ApiError> {
    dataset::find_station(id).ok_or_else(|| {
        ApiError::NotFound(format!(
            "no station '{id}': use a station UUID or code from GET /resources"
        ))
    })
}

fn subject_for(state: &AppState, id: Option<Uuid>) -> Subject {
    Subject {
        id: id.unwrap_or(state.operator),
        registered: true,
        attributes: Default::default(),
    }
}

async fn export(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let station = lookup_station(&id)?;
    let readings_sql = format!(
        "SELECT to_char(measured_at, 'YYYY-MM-DD\"T\"HH24:MI:SS') AS measured_at, \
         pm25, no2, ozone FROM readings WHERE station_id = '{}' ORDER BY measured_at",
        station.id()
    );
    let (_, rows) = state.db.execute(&readings_sql).await?;
    let csv = sovereignty::readings_csv(&rows);
    let (sealed, manifest) = sovereignty::export_station(
        sovereignty::ExportRequest {
            station,
            csv,
            row_count: rows.len() as u64,
            base_url: &state.config.public_base_url,
            actor: Some(subject_for(&state, None)),
            now_ms: now_unix_ms(),
        },
        &state.store,
        &state.provenance,
    )
    .await?;
    Ok(Json(json!({
        "resource": station.resource(),
        "storageBackend": state.config.store.backend_label(),
        "bundlePath": sovereignty::bundle_path(station),
        "artifact": manifest,
        "contentHash": sealed.content_hash,
        "byteCount": sealed.byte_count(),
        "bundle": sealed.bundle,
    })))
}

/// Read the sealed bundle back from the object store, proving the bytes
/// landed in (and round-trip through) the configured backend.
async fn bundle(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let station = lookup_station(&id)?;
    let path = sovereignty::bundle_path(station);
    let bytes = state.store.get(&path).await.map_err(|_| {
        ApiError::NotFound(format!(
            "no bundle stored for '{}': POST /resources/{}/export first",
            station.code, station.code
        ))
    })?;
    Ok(([("content-type", "application/json")], bytes).into_response())
}

#[derive(Deserialize)]
struct SubjectQuery {
    subject: Option<Uuid>,
}

/// GDPR Article 15: structured access export (policy + full audit trail).
async fn access_export(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    Query(params): Query<SubjectQuery>,
) -> Result<Json<Value>, ApiError> {
    let station = lookup_station(&id)?;
    let subject = subject_for(&state, params.subject);
    let record = DataRights::new(&state.provenance)
        .access_export(
            station.resource(),
            &subject,
            station_policy(),
            now_unix_ms(),
        )
        .await?;
    Ok(Json(serde_json::to_value(record).map_err(|error| {
        ApiError::Dependency(format!("access export serialization failed: {error}"))
    })?))
}

#[derive(Deserialize)]
struct EraseRequest {
    reason: String,
    subject: Option<Uuid>,
}

/// GDPR Article 17: erasure is owner-only and soft (audit trail survives).
async fn erase(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    Json(request): Json<EraseRequest>,
) -> Result<Json<Value>, ApiError> {
    let station = lookup_station(&id)?;
    let subject = subject_for(&state, request.subject);
    let outcome = evaluate_access(
        &subject,
        station.resource(),
        &AccessPolicy::Restricted {
            data_use_restrictions: Vec::new(),
        },
        &state.ownership,
    )
    .await;
    if let AccessOutcome::Deny { reason } = outcome {
        return Err(ApiError::Forbidden(format!(
            "erasure requires resource ownership: {reason}"
        )));
    }
    let deletion = DataRights::new(&state.provenance)
        .soft_delete(
            station.resource(),
            Some(subject),
            &request.reason,
            now_unix_ms(),
        )
        .await?;
    Ok(Json(serde_json::to_value(deletion).map_err(|error| {
        ApiError::Dependency(format!("deletion receipt serialization failed: {error}"))
    })?))
}

async fn mint_doi(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let station = lookup_station(&id)?;
    let landing_url = format!(
        "{}/resources/{}",
        state.config.public_base_url, station.code
    );
    let metadata = sovereignty::station_datacite(
        station,
        &format!("{}/qfdemo.pending", sovereignty::DEMO_DOI_PREFIX),
    );
    let record = LocalDoiProvider
        .mint(station.resource(), &metadata, &landing_url)
        .await
        .map_err(|error| ApiError::Dependency(error.to_string()))?;
    Ok(Json(json!({ "record": record, "datacite": metadata })))
}

async fn federation_status(State(state): State<SharedState>) -> Json<Value> {
    let federation = &state.config.federation;
    if !federation.enable {
        return Json(json!({ "enabled": false }));
    }
    let identity = ClusterIdentity {
        name: federation.node_name.clone(),
        endpoint: state.config.listen_addr.ip().to_string(),
        port: i32::from(federation.flight_port),
        ca_certificate_pem: None,
        description: Some("queryfabric self-host demonstrator".to_owned()),
        institution: None,
        contact_email: None,
    };
    Json(json!({
        "enabled": true,
        "identity": identity,
        "hubMultiaddrs": federation.hub_multiaddrs,
    }))
}

#[cfg(test)]
mod tests {
    use queryfabric_tenancy::{Account, AccountKind};

    use super::*;
    use crate::dataset::AccountIds;
    use queryfabric_namespace_uuid::NamespacedIds;

    /// The erase path must be deny-by-default: a random registered subject
    /// is refused, the seeded operator (owner) is allowed.
    #[tokio::test]
    async fn erasure_is_owner_only() {
        let ownership = InMemoryOwnership::new();
        let operator = AccountIds::from_str_key("operator");
        ownership.add_account(Account {
            id: operator,
            email: "operator@example.org".to_owned(),
            active: true,
            verified: true,
            kind: AccountKind::Human,
        });
        let station = &STATIONS[0];
        ownership.set_owner(station.resource(), operator);

        let policy = AccessPolicy::Restricted {
            data_use_restrictions: Vec::new(),
        };
        let owner_subject = Subject {
            id: operator,
            registered: true,
            attributes: Default::default(),
        };
        let stranger = Subject {
            id: Uuid::now_v7(),
            registered: true,
            attributes: Default::default(),
        };

        let allowed =
            evaluate_access(&owner_subject, station.resource(), &policy, &ownership).await;
        assert!(allowed.is_allowed());
        let denied = evaluate_access(&stranger, station.resource(), &policy, &ownership).await;
        assert!(!denied.is_allowed());
    }
}
