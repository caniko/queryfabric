//! Sovereignty wiring: export bundles, DOI minting, and provenance seeding.
//!
//! This is where the demonstrator exercises the Phase 05 layer end-to-end:
//! it exports a station's readings as a CSV artifact into the object store,
//! assembles the content-addressed export bundle around it, and records the
//! whole flow in provenance.

use async_trait::async_trait;
use queryfabric_access::{DataLicense, ResourcePolicy};
use queryfabric_contract::{Activity, ResourceRef, Subject};
use queryfabric_portability::datacite::{
    DataCiteCreator, DataCiteIdentifier, DataCiteMetadata, DataCiteResourceType, DataCiteTitle,
    IdentifierType, ResourceTypeGeneral,
};
use queryfabric_portability::{
    ArtifactManifest, BundleRequest, CitationInput, DoiError, DoiProvider, DoiRecord, DoiStatus,
    ImportSealedBundle, TABULAR_CSV_PROFILE, TabularColumn, TabularColumnType, TabularSchema,
    build_import_bundle, content_hash_hex, tabular_schema_fingerprint, write_tabular_csv,
};
use queryfabric_provenance::{ProvenanceEntry, ProvenanceError, ProvenanceStore, RecordedActivity};
use queryfabric_store::ObjectStore;
use serde_json::{Value, json};
use uuid::Uuid;

use queryfabric_namespace_uuid::NamespacedIds;

use crate::dataset::{SEED_EPOCH_MS, STATIONS, StationSpec};

/// UUIDv5 namespace for seeded provenance entries: ids must be
/// deterministic so identically seeded hosts produce identical bundles
/// (and therefore identical content hashes).
struct SeedEntryIds;
impl NamespacedIds for SeedEntryIds {
    const NAMESPACE: Uuid = Uuid::from_bytes(*b"qfdemo-airq-prov");
}

/// Demonstration license: every station dataset ships CC-BY 4.0.
pub const DEMO_LICENSE: DataLicense = DataLicense::CcBy;

/// DataCite's reserved test prefix; locally minted demo DOIs live under it.
pub const DEMO_DOI_PREFIX: &str = "10.5072";

/// Current Unix time in milliseconds.
#[must_use]
pub fn now_unix_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or(0)
}

/// The access posture every demo station carries.
#[must_use]
pub fn station_policy() -> ResourcePolicy {
    ResourcePolicy {
        policy: queryfabric_contract::AccessPolicy::Open,
        license: Some(DEMO_LICENSE),
        restriction: None,
    }
}

/// Schema.org JSON-LD metadata for a station dataset.
#[must_use]
pub fn station_metadata_jsonld(station: &StationSpec) -> Value {
    json!({
        "@context": "https://schema.org/",
        "@type": "Dataset",
        "identifier": station.id().to_string(),
        "name": format!("{} air-quality readings", station.name),
        "description": format!(
            "Hourly PM2.5, NO2, and ozone readings from the {} station in {}.",
            station.name, station.city
        ),
        "spatialCoverage": {
            "@type": "Place",
            "geo": {
                "@type": "GeoCoordinates",
                "latitude": station.latitude,
                "longitude": station.longitude,
            },
            "name": station.city,
        },
        "variableMeasured": ["PM2.5", "NO2", "O3"],
        "license": DEMO_LICENSE.rights_uri(),
    })
}

/// Citation facts for a station dataset.
#[must_use]
pub fn station_citation(
    station: &StationSpec,
    base_url: &str,
    doi: Option<String>,
) -> CitationInput {
    CitationInput {
        id: station.id().simple().to_string(),
        title: format!("{} air-quality readings", station.name),
        publisher: "QueryFabric Demonstrator".to_owned(),
        year: "2026".to_owned(),
        url: format!("{base_url}/resources/{}", station.code),
        doi,
        license_spdx: Some(DEMO_LICENSE.spdx_id().to_owned()),
        keywords: vec!["air-quality".to_owned(), station.city.to_lowercase()],
        repository_url: None,
    }
}

/// Render readings rows (as returned by the readings export query) to CSV.
#[must_use]
pub fn readings_csv(rows: &[Value]) -> String {
    let bytes =
        write_tabular_csv(&readings_schema(), rows).expect("demo rows match profile schema");
    String::from_utf8(bytes).expect("normative CSV is UTF-8")
}

/// The one importable table profile implemented by the reference host.
#[must_use]
pub fn readings_schema() -> TabularSchema {
    TabularSchema {
        profile: TABULAR_CSV_PROFILE.to_owned(),
        columns: vec![
            TabularColumn {
                name: "measured_at".to_owned(),
                column_type: TabularColumnType::Timestamp,
            },
            TabularColumn {
                name: "pm25".to_owned(),
                column_type: TabularColumnType::Float64,
            },
            TabularColumn {
                name: "no2".to_owned(),
                column_type: TabularColumnType::Float64,
            },
            TabularColumn {
                name: "ozone".to_owned(),
                column_type: TabularColumnType::Float64,
            },
        ],
    }
}

/// Where a station's exported artifacts live in the object store.
#[must_use]
pub fn export_csv_path(station: &StationSpec) -> String {
    format!("exports/{}/readings.csv", station.code)
}

/// Where a station's sealed bundle lives in the object store.
#[must_use]
pub fn bundle_path(station: &StationSpec) -> String {
    format!("bundles/{}.json", station.code)
}

/// Errors raised while producing an export.
#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error(transparent)]
    Bundle(#[from] queryfabric_portability::BundleError),
    #[error(transparent)]
    Provenance(#[from] ProvenanceError),
    #[error(transparent)]
    Store(#[from] queryfabric_store::StoreError),
}

/// Inputs for [`export_station`].
pub struct ExportRequest<'a> {
    pub station: &'a StationSpec,
    pub csv: String,
    pub row_count: u64,
    pub base_url: &'a str,
    pub actor: Option<Subject>,
    pub now_ms: i64,
}

/// Export one station: write the CSV artifact and its sealed bundle to the
/// object store, recording content hashes in provenance.
pub async fn export_station(
    request: ExportRequest<'_>,
    store: &ObjectStore,
    provenance: &dyn ProvenanceStore,
) -> Result<(ImportSealedBundle, ArtifactManifest), ExportError> {
    let ExportRequest {
        station,
        csv,
        row_count,
        base_url,
        actor,
        now_ms,
    } = request;
    let resource = station.resource();
    let csv_bytes = csv.into_bytes();
    let csv_path = export_csv_path(station);
    let manifest = ArtifactManifest {
        kind: "table_export".to_owned(),
        storage_uri: csv_path.clone(),
        format: "csv".to_owned(),
        schema_fingerprint: tabular_schema_fingerprint(&readings_schema()),
        content_hash: format!("blake3-256:{}", content_hash_hex(&csv_bytes)),
        row_count,
        byte_count: Some(csv_bytes.len() as u64),
        manifest_json: Value::Null,
    };
    store.put(&csv_path, csv_bytes).await?;

    let sealed = build_import_bundle(
        BundleRequest {
            resource,
            exported_at_unix_ms: now_ms,
            metadata_jsonld: station_metadata_jsonld(station),
            citation: station_citation(station, base_url, None),
            policy: station_policy(),
            artifacts: vec![ArtifactManifest {
                manifest_json: serde_json::to_value(readings_schema()).expect("schema serializes"),
                ..manifest.clone()
            }],
        },
        provenance,
    )
    .await?;

    let path = bundle_path(station);
    store
        .put(&path, sealed.canonical_json.clone().into_bytes())
        .await?;

    provenance
        .append(ProvenanceEntry {
            id: Uuid::now_v7(),
            resource,
            actor,
            activity: Activity::BackupAnchor {
                location: path,
                content_hash: sealed.content_hash.clone(),
            }
            .into(),
            description: Some("portable export bundle written to object store".to_owned()),
            occurred_at_unix_ms: now_ms,
        })
        .await?;

    Ok((sealed, manifest))
}

/// Seed `Created` plus a domain-extension activity for every station, once.
pub async fn seed_provenance(provenance: &dyn ProvenanceStore) -> Result<(), ProvenanceError> {
    for station in &STATIONS {
        let resource = station.resource();
        let existing = provenance.history(resource, &Default::default()).await?;
        if !existing.entries.is_empty() {
            continue;
        }
        provenance
            .append(ProvenanceEntry {
                id: SeedEntryIds::from_parts(&[station.code, "created"]),
                resource,
                actor: None,
                activity: Activity::Created.into(),
                description: Some(format!("{} registered in {}", station.name, station.city)),
                occurred_at_unix_ms: SEED_EPOCH_MS,
            })
            .await?;
        // A host-domain activity carried opaquely: the core crates never
        // learn what "sensor_calibration" means.
        provenance
            .append(ProvenanceEntry {
                id: SeedEntryIds::from_parts(&[station.code, "calibration"]),
                resource,
                actor: None,
                activity: RecordedActivity::Domain {
                    kind: "sensor_calibration".to_owned(),
                    payload: json!({
                        "method": "reference-gas",
                        "pollutants": ["pm25", "no2", "ozone"],
                    }),
                },
                description: None,
                occurred_at_unix_ms: SEED_EPOCH_MS + 3_600_000,
            })
            .await?;
    }
    Ok(())
}

/// DataCite 4.5 metadata for a station, used by DOI minting.
#[must_use]
pub fn station_datacite(station: &StationSpec, doi: &str) -> DataCiteMetadata {
    DataCiteMetadata {
        identifier: DataCiteIdentifier {
            identifier: doi.to_owned(),
            identifier_type: IdentifierType::DOI,
        },
        creators: vec![DataCiteCreator {
            name: "QueryFabric Demonstrator".to_owned(),
            given_name: None,
            family_name: None,
            name_identifier: None,
            affiliation: Vec::new(),
        }],
        titles: vec![DataCiteTitle {
            title: format!("{} air-quality readings", station.name),
            title_type: None,
        }],
        publisher: "QueryFabric Demonstrator".to_owned(),
        publication_year: 2026,
        resource_type: DataCiteResourceType {
            resource_type_general: ResourceTypeGeneral::Dataset,
            resource_type: Some("Time series".to_owned()),
        },
        subjects: Vec::new(),
        rights_list: Vec::new(),
        related_identifiers: Vec::new(),
        descriptions: Vec::new(),
        dates: Vec::new(),
        schema_version: "4.5".to_owned(),
    }
}

/// Offline [`DoiProvider`]: fabricates registered records under the DataCite
/// test prefix. The real `DataCiteProvider` ships in
/// `queryfabric-portability`; a self-hoster supplies registrar credentials
/// to use it instead.
#[derive(Debug, Default, Clone, Copy)]
pub struct LocalDoiProvider;

#[async_trait]
impl DoiProvider for LocalDoiProvider {
    fn provider_name(&self) -> &str {
        "demo-local"
    }

    async fn mint(
        &self,
        resource: ResourceRef,
        _metadata: &DataCiteMetadata,
        landing_url: &str,
    ) -> Result<DoiRecord, DoiError> {
        let suffix = resource.id.simple().to_string();
        Ok(DoiRecord {
            resource,
            doi: Some(format!("{DEMO_DOI_PREFIX}/qfdemo.{}", &suffix[..12])),
            provider: self.provider_name().to_owned(),
            status: DoiStatus::Registered,
            response: Some(json!({
                "note": "locally fabricated demonstration DOI (DataCite test prefix); \
                         configure a registrar account for real minting",
                "landingUrl": landing_url,
            })),
            last_error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use queryfabric_provenance::VecProvenanceStore;

    use super::*;

    #[tokio::test]
    async fn export_bundle_round_trips_and_is_content_addressed() {
        let provenance = VecProvenanceStore::new();
        seed_provenance(&provenance).await.expect("seed");
        let store = ObjectStore::memory();
        let station = &STATIONS[0];
        let csv =
            "measured_at,pm25,no2,ozone\r\n2026-01-01T00:00:00Z,11.0,24.0,58.0\r\n".to_owned();

        let (sealed, manifest) = export_station(
            ExportRequest {
                station,
                csv: csv.clone(),
                row_count: 1,
                base_url: "http://127.0.0.1:8780",
                actor: None,
                now_ms: SEED_EPOCH_MS,
            },
            &store,
            &provenance,
        )
        .await
        .expect("export");

        // The stored bundle parses back into the same structure.
        let stored = store.get(&bundle_path(station)).await.expect("get bundle");
        let parsed: queryfabric_portability::ExportBundle =
            serde_json::from_slice(&stored).expect("bundle parses");
        assert_eq!(parsed.export_bundle.resource, station.resource());
        assert_eq!(parsed.artifacts.len(), 1);
        assert!(!parsed.provenance.entries.is_empty());
        assert!(parsed.citations.bibtex.contains("air-quality"));
        assert_eq!(manifest.row_count, 1);

        let validated = queryfabric_portability::validate_import_bundle(
            &stored,
            &sealed.content_hash,
            queryfabric_portability::ImportLimits::default(),
        )
        .expect("2.0 bundle validates");
        let staged = store
            .get(&export_csv_path(station))
            .await
            .expect("get staged CSV");
        let plan = queryfabric_portability::plan_tabular_import(
            &validated,
            &staged,
            queryfabric_portability::PlanTarget {
                target_resource: station.resource(),
                relation: "readings".to_owned(),
                target_revision: "snapshot-1".to_owned(),
                expected_schema: readings_schema(),
                local_owner: Uuid::from_u128(42),
            },
            queryfabric_portability::ImportLimits::default(),
        )
        .expect("tabular import plan");
        assert_eq!(plan.row_count, 1);
        assert!(plan.plan_digest.starts_with("blake3-256:"));

        // Same inputs, same content address: rebuild from an identically
        // seeded store and compare hashes.
        let provenance2 = VecProvenanceStore::new();
        seed_provenance(&provenance2).await.expect("seed");
        let store2 = ObjectStore::memory();
        let (sealed2, _) = export_station(
            ExportRequest {
                station,
                csv,
                row_count: 1,
                base_url: "http://127.0.0.1:8780",
                actor: None,
                now_ms: SEED_EPOCH_MS,
            },
            &store2,
            &provenance2,
        )
        .await
        .expect("export again");
        assert_eq!(sealed.content_hash, sealed2.content_hash);
    }

    #[tokio::test]
    async fn local_doi_provider_mints_under_test_prefix() {
        let station = &STATIONS[1];
        let metadata = station_datacite(station, "10.5072/placeholder");
        let record = LocalDoiProvider
            .mint(station.resource(), &metadata, "http://example.org")
            .await
            .expect("mint");
        assert_eq!(record.status, DoiStatus::Registered);
        assert!(record.doi.expect("doi").starts_with("10.5072/qfdemo."));
    }

    #[test]
    fn csv_rendering_handles_rows() {
        let rows = vec![serde_json::json!({
            "measured_at": "2026-01-01T00:00:00Z",
            "pm25": 11.0,
            "no2": 24.0,
            "ozone": 58.0,
        })];
        let csv = readings_csv(&rows);
        assert!(csv.starts_with("measured_at,"));
        assert!(csv.contains("2026-01-01T00:00:00Z,11"));
    }
}
