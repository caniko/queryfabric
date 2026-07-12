//! Axum routes: portable query API plus the sovereignty endpoints.

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header::AUTHORIZATION};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use queryfabric::{
    EmitArtifact, GenericSqlDialect, MemoryCatalog, PostgresAdapter, QueryCompiler,
    QueryFabricError, build_query_parameters,
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
use crate::db::{Database, ImportCommit};
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
    #[error("authentication required: {0}")]
    Unauthorized(String),
    #[error("import replay conflicts with the existing receipt: {0}")]
    Conflict(String),
}

impl ApiError {
    fn status(&self) -> StatusCode {
        match self {
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
            Self::Dependency(_) => StatusCode::BAD_GATEWAY,
            Self::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            Self::Conflict(_) => StatusCode::CONFLICT,
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
        match error {
            crate::db::DbError::ConflictingReplay(message) => Self::Conflict(message),
            other => Self::Dependency(other.to_string()),
        }
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

impl From<queryfabric_portability::ImportError> for ApiError {
    fn from(error: queryfabric_portability::ImportError) -> Self {
        Self::BadRequest(error.to_string())
    }
}

fn authenticated_subject(
    state: &AppState,
    headers: &HeaderMap,
    required_role: &str,
) -> Result<Subject, ApiError> {
    let header = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| ApiError::Unauthorized("missing Authorization header".to_owned()))?;
    let token = queryfabric_paseto::extract_bearer_token(header)
        .ok_or_else(|| ApiError::Unauthorized("expected a Bearer token".to_owned()))?;
    let user = queryfabric_paseto::validate_paseto_token(token, &state.config.auth_secret)
        .map_err(|_| ApiError::Unauthorized("invalid bearer credential".to_owned()))?;
    if !user.is_active || !user.is_verified {
        return Err(ApiError::Forbidden(
            "the authenticated account is inactive or unverified".to_owned(),
        ));
    }
    if !user.is_superuser && !user.has_role(required_role) {
        return Err(ApiError::Forbidden(format!(
            "authenticated subject lacks required role '{required_role}'"
        )));
    }
    let mut attributes = std::collections::BTreeMap::new();
    attributes.insert("email".to_owned(), user.email.as_str().to_owned());
    Ok(Subject {
        id: user.id,
        registered: true,
        attributes,
    })
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
        .route("/imports/dry-run", post(import_dry_run))
        .route("/imports/apply", post(import_apply))
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
#[serde(rename_all = "camelCase")]
struct QueryRequest {
    sql: String,
    #[serde(default)]
    dialect: Option<String>,
    #[serde(default)]
    positional: Option<Vec<Value>>,
    #[serde(default)]
    named: Option<HashMap<String, Value>>,
    #[serde(default)]
    expected_catalog_snapshot_id: Option<String>,
    #[serde(default)]
    requested_backend: Option<String>,
}

const MAX_QUERY_ROWS: usize = 1_000;
const MAX_QUERY_BYTES: usize = 4 * 1024 * 1024;

/// Compile a portable query against the catalog, emit Postgres SQL, execute
/// it, and return the rows. The compiler validates against the catalog, so
/// only the two demo relations are reachable.
async fn query(
    State(state): State<SharedState>,
    Json(request): Json<QueryRequest>,
) -> Result<Json<Value>, ApiError> {
    let dialect = request.dialect.as_deref().unwrap_or("sql");
    if dialect != "sql" {
        return Err(ApiError::BadRequest(format!(
            "unsupported query dialect '{dialect}'; this host supports 'sql'"
        )));
    }
    if let Some(expected) = &request.expected_catalog_snapshot_id
        && expected != dataset::SNAPSHOT_ID
    {
        return Err(ApiError::Conflict(format!(
            "catalog snapshot mismatch: expected '{expected}', current '{}'",
            dataset::SNAPSHOT_ID
        )));
    }
    if let Some(requested) = &request.requested_backend
        && requested != "postgres"
    {
        return Err(ApiError::BadRequest(format!(
            "requested backend '{requested}' is unavailable; this host supports 'postgres'"
        )));
    }
    let compiler = QueryCompiler::new();
    let parsed = compiler.parse(&GenericSqlDialect, &request.sql)?;
    let parameters = build_query_parameters(request.positional.as_deref(), request.named.as_ref())?;
    let bound = compiler.bind_and_validate(&parsed, &state.catalog, &parameters)?;
    let artifact = compiler.emit(&bound, &PostgresAdapter, &state.catalog)?;
    let EmitArtifact::Sql(sql) = artifact else {
        return Err(ApiError::Dependency(
            "postgres adapter emitted a non-SQL artifact".to_owned(),
        ));
    };
    let (columns, rows, truncated) = state
        .db
        .execute_query(
            &sql.text,
            bound.parameters(),
            MAX_QUERY_ROWS,
            MAX_QUERY_BYTES,
        )
        .await?;
    Ok(Json(json!({
        "contractVersion": "queryfabric.query/1",
        "requestedSql": request.sql,
        "backendDialect": sql.dialect,
        "backend": "postgres",
        "backendSql": sql.text,
        "parameterSchema": sql.parameters,
        "resultSchema": bound.result_schema(),
        "provenance": sql.provenance,
        "snapshotId": dataset::SNAPSHOT_ID,
        "limits": { "maxRows": MAX_QUERY_ROWS, "maxBytes": MAX_QUERY_BYTES },
        "columns": columns,
        "rowCount": rows.len(),
        "truncated": truncated,
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

async fn export(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let subject = authenticated_subject(&state, &headers, "operator")?;
    let station = lookup_station(&id)?;
    let readings_sql = format!(
        "SELECT to_char(measured_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS measured_at, pm25, no2, ozone FROM readings WHERE station_id = '{}' ORDER BY measured_at",
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
            actor: Some(subject),
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportRequest {
    /// Canonical bundle JSON transferred by the operator; never a URI to fetch.
    bundle: String,
    /// Profile-1 CSV bytes represented as UTF-8 text for this HTTP MVP.
    artifact: String,
    expected_bundle_digest: String,
    target: String,
    /// The plan digest returned by dry-run. Required by apply.
    #[serde(default)]
    plan_digest: Option<String>,
    /// The immutable staging object returned by dry-run. Required by apply.
    #[serde(default)]
    staged_object: Option<String>,
}

async fn prepare_import(
    state: &AppState,
    request: &ImportRequest,
) -> Result<
    (
        queryfabric_portability::ValidatedBundle,
        Vec<Vec<String>>,
        queryfabric_portability::ImportPlan,
        &'static StationSpec,
        String,
    ),
    ApiError,
> {
    let station = lookup_station(&request.target)?;
    let validated = queryfabric_portability::validate_import_bundle(
        request.bundle.as_bytes(),
        &request.expected_bundle_digest,
        queryfabric_portability::ImportLimits::default(),
    )?;
    let artifact_bytes = request.artifact.as_bytes();
    let plan = queryfabric_portability::plan_tabular_import(
        &validated,
        artifact_bytes,
        queryfabric_portability::PlanTarget {
            target_resource: station.resource(),
            relation: "readings".to_owned(),
            target_revision: dataset::SNAPSHOT_ID.to_owned(),
            expected_schema: sovereignty::readings_schema(),
            local_owner: state.operator,
        },
        queryfabric_portability::ImportLimits::default(),
    )?;
    let rows = queryfabric_portability::decode_tabular_csv(
        artifact_bytes,
        &sovereignty::readings_schema(),
        queryfabric_portability::ImportLimits::default(),
    )?;
    let staged_path = format!(
        "imports/staging/{}/artifact.csv",
        plan.artifact_digest.replace(':', "-")
    );
    Ok((validated, rows, plan, station, staged_path))
}

async fn import_dry_run(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(request): Json<ImportRequest>,
) -> Result<Json<Value>, ApiError> {
    authenticated_subject(&state, &headers, "import")?;
    let (validated, rows, plan, station, staged_path) = prepare_import(&state, &request).await?;
    if let Ok(existing) = state.store.get(&staged_path).await {
        if existing != request.artifact.as_bytes() {
            return Err(ApiError::Conflict(
                "content-addressed staging object already contains different bytes".to_owned(),
            ));
        }
    } else {
        state
            .store
            .put(&staged_path, request.artifact.as_bytes().to_vec())
            .await?;
    }
    Ok(Json(json!({
        "contractVersion": "queryfabric.import/1",
        "mode": "dry-run",
        "bundleDigest": validated.bundle_digest,
        "sourceResource": validated.source_resource,
        "targetResource": station.resource(),
        "targetRevision": plan.target.target_revision,
        "stagedObject": staged_path,
        "planDigest": plan.plan_digest,
        "rowCount": rows.len(),
        "plan": plan,
        "diagnostics": [],
        "policy": "source licence/restriction are carried evidence; target policy is assigned locally",
    })))
}

async fn import_apply(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(request): Json<ImportRequest>,
) -> Result<Json<Value>, ApiError> {
    authenticated_subject(&state, &headers, "import")?;
    let expected_plan_digest = request.plan_digest.as_deref().ok_or_else(|| {
        ApiError::BadRequest("apply requires the planDigest returned by dry-run".to_owned())
    })?;
    let expected_staged_object = request.staged_object.as_deref().ok_or_else(|| {
        ApiError::BadRequest("apply requires the stagedObject returned by dry-run".to_owned())
    })?;
    let (validated, rows, plan, station, staged_path) = prepare_import(&state, &request).await?;
    if expected_plan_digest != plan.plan_digest {
        return Err(ApiError::Conflict(
            "planDigest does not match the current validated import plan".to_owned(),
        ));
    }
    if expected_staged_object != staged_path {
        return Err(ApiError::Conflict(
            "stagedObject does not match the content-addressed artifact".to_owned(),
        ));
    }
    let staged_bytes = state.store.get(&staged_path).await.map_err(|_| {
        ApiError::BadRequest("the dry-run staging object is no longer available".to_owned())
    })?;
    if staged_bytes != request.artifact.as_bytes() {
        return Err(ApiError::Conflict(
            "staged bytes differ from the apply request".to_owned(),
        ));
    }
    let source_resource = serde_json::to_value(validated.source_resource)
        .map_err(|error| ApiError::Dependency(error.to_string()))?;
    let target_resource = serde_json::to_value(station.resource())
        .map_err(|error| ApiError::Dependency(error.to_string()))?;
    let source_evidence = serde_json::to_value(&validated.bundle.provenance)
        .map_err(|error| ApiError::Dependency(error.to_string()))?;
    let mapping = serde_json::to_value(&plan.column_mapping)
        .map_err(|error| ApiError::Dependency(error.to_string()))?;
    let local_policy = serde_json::to_value(station_policy())
        .map_err(|error| ApiError::Dependency(error.to_string()))?;
    let (receipt, replayed) = state
        .db
        .apply_import(ImportCommit {
            station_id: station.id(),
            rows: &rows,
            bundle_digest: &validated.bundle_digest,
            plan_digest: &plan.plan_digest,
            target_revision: &plan.target.target_revision,
            source_resource: &source_resource,
            target_resource: &target_resource,
            local_owner: state.operator,
            local_policy: &local_policy,
            source_evidence: &source_evidence,
            mapping: &mapping,
            byte_count: request.artifact.len() as u64,
        })
        .await?;
    Ok(Json(json!({
        "contractVersion": "queryfabric.import/1",
        "mode": "apply",
        "receipt": receipt,
        "stagedObject": staged_path,
        "replayed": replayed,
    })))
}

/// GDPR Article 15: structured access export (policy + full audit trail).
async fn access_export(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let subject = authenticated_subject(&state, &headers, "operator")?;
    let station = lookup_station(&id)?;
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
}

/// GDPR Article 17: erasure is owner-only and soft (audit trail survives).
async fn erase(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<EraseRequest>,
) -> Result<Json<Value>, ApiError> {
    let subject = authenticated_subject(&state, &headers, "operator")?;
    let station = lookup_station(&id)?;
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
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    authenticated_subject(&state, &headers, "operator")?;
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

    fn test_state() -> SharedState {
        let operator = AccountIds::from_str_key("operator");
        Arc::new(AppState {
            config: DemoConfig {
                listen_addr: "127.0.0.1:8780".parse().expect("address"),
                database_migration_url: "postgres://invalid".to_owned(),
                database_query_url: "postgres://invalid".to_owned(),
                database_import_url: "postgres://invalid".to_owned(),
                auth_secret: "qf-demo-auth-secret-2026-operator-000000".to_owned(),
                db_wait_secs: 1,
                public_base_url: "http://127.0.0.1:8780".to_owned(),
                seed_demo_data: false,
                store: crate::config::StoreConfig::Memory,
                federation: crate::config::FederationConfig {
                    enable: false,
                    node_name: "queryfabric-demo".to_owned(),
                    hub_multiaddrs: Vec::new(),
                    flight_port: 50051,
                },
            },
            db: crate::db::Database::new_with_roles(
                "postgres://invalid".to_owned(),
                "postgres://invalid".to_owned(),
                "postgres://invalid".to_owned(),
            ),
            store: ObjectStore::memory(),
            catalog: dataset::build_catalog(),
            provenance: VecProvenanceStore::new(),
            ownership: InMemoryOwnership::new(),
            operator,
        })
    }

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

    #[tokio::test]
    async fn apply_requires_dry_run_identity() {
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            axum::http::HeaderValue::from_static(
                "Bearer v4.local.7YoCIGisuMEE_g46oSO_uTRGiZbR_d96apYfYQWAGzXQ07T517-vONyS7-pRrLRO7a9Uf7Or2wvHyrvDm4T2IdG98EDF91T58R_bCdGEblRVHXe0JuMp9EjereFJOEigiO6ZuwvFyUtR9DMQ3ZdxtVhFqsPQzS4qYeQ64Q3rIdVcL3hHqmfhV-_5gn_LTkX6ebRBATWsbeQBwItpw67kotTTAsOWmPE4NoCQG0vmNdF482Ml4SOSxlQVtuQ6jzcOFzpW0t6espbI7iwsQWt2Gui85b61VpCozOXamqe4IlLSmfN0nrtzMKs2yRdRsR4yrl2cRaq9FtRo6_6m29dcw2Yj-RWKfK5OFukgJK2z516DEI3fhcbJB8K5DoT_w1lEFJ2eU2sm6kr3bRgHHUybgySGWWRXaayO_AsG5yiyNdc6seRabPOBkwI",
            ),
        );
        let result = import_apply(
            State(test_state()),
            headers,
            Json(ImportRequest {
                bundle: String::new(),
                artifact: String::new(),
                expected_bundle_digest: String::new(),
                target: "lis-baixa".to_owned(),
                plan_digest: None,
                staged_object: None,
            }),
        )
        .await;
        assert!(matches!(
            result,
            Err(ApiError::BadRequest(message))
                if message.contains("planDigest")
        ));
    }

    #[test]
    fn protected_routes_reject_missing_credentials() {
        let result = authenticated_subject(&test_state(), &HeaderMap::new(), "operator");
        assert!(matches!(
            result,
            Err(ApiError::Unauthorized(message)) if message.contains("Authorization")
        ));
    }

    #[tokio::test]
    async fn query_contract_rejects_unsupported_dialect_backend_and_snapshot() {
        let base = || QueryRequest {
            sql: "SELECT 1".to_owned(),
            dialect: None,
            positional: None,
            named: None,
            expected_catalog_snapshot_id: None,
            requested_backend: None,
        };

        let mut request = base();
        request.dialect = Some("syql".to_owned());
        assert!(matches!(
            query(State(test_state()), Json(request)).await,
            Err(ApiError::BadRequest(message)) if message.contains("dialect")
        ));

        let mut request = base();
        request.requested_backend = Some("clickhouse".to_owned());
        assert!(matches!(
            query(State(test_state()), Json(request)).await,
            Err(ApiError::BadRequest(message)) if message.contains("backend")
        ));

        let mut request = base();
        request.expected_catalog_snapshot_id = Some("stale-snapshot".to_owned());
        assert!(matches!(
            query(State(test_state()), Json(request)).await,
            Err(ApiError::Conflict(message)) if message.contains("snapshot")
        ));
    }
}
