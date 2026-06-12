use async_trait::async_trait;
use queryfabric_access::{DataLicense, DataRights, DataUseRestriction, ResourcePolicy};
use queryfabric_contract::{AccessPolicy, ResourceRef, Subject};
use queryfabric_provenance::VecProvenanceStore;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::datacite::{
    DataCiteCreator, DataCiteIdentifier, DataCiteMetadata, DataCiteResourceType, DataCiteRights,
    DataCiteTitle, IdentifierType, RelationType, ResourceTypeGeneral,
};
use crate::{
    ArtifactManifest, BUNDLE_VERSION, BundleRequest, CitationFormat, CitationInput,
    DataCiteProvider, DoiError, DoiProvider, DoiStatus, ExportBundle, HttpTransport, build_bundle,
    canonical_json_string, content_hash_hex, generate_citation,
};

fn resource() -> ResourceRef {
    ResourceRef::new(Uuid::from_u128(0xC), Uuid::from_u128(1))
}

fn citation_input() -> CitationInput {
    CitationInput {
        id: "resource_0001".to_owned(),
        title: "Reference Measurements".to_owned(),
        publisher: "ExamplePlatform".to_owned(),
        year: "2026".to_owned(),
        url: "https://doi.org/10.1234/example.1".to_owned(),
        doi: Some("10.1234/example.1".to_owned()),
        license_spdx: Some("CC-BY-4.0".to_owned()),
        keywords: vec!["measurements".to_owned(), "reference".to_owned()],
        repository_url: Some("https://example.org".to_owned()),
    }
}

fn policy() -> ResourcePolicy {
    ResourcePolicy {
        policy: AccessPolicy::Open,
        license: Some(DataLicense::CcBy),
        restriction: Some(DataUseRestriction {
            kind: "DUO:0000042".to_owned(),
            summary: Some("general research use".to_owned()),
            source_url: Some("https://example.org/duo".to_owned()),
        }),
    }
}

fn artifact() -> ArtifactManifest {
    let bytes = b"artifact body";
    ArtifactManifest {
        kind: "table_export".to_owned(),
        storage_uri: "s3://bucket/exports/resource_0001.parquet".to_owned(),
        format: "parquet".to_owned(),
        schema_fingerprint: "fp_v1_ab12".to_owned(),
        content_hash: content_hash_hex(bytes),
        row_count: 128,
        byte_count: Some(bytes.len() as u64),
        manifest_json: json!({"columns": 4}),
    }
}

async fn request_with_history() -> (BundleRequest, VecProvenanceStore) {
    let store = VecProvenanceStore::new();
    let rights = DataRights::new(&store);
    let actor = Subject {
        id: Uuid::from_u128(9),
        registered: true,
        attributes: Default::default(),
    };
    rights
        .rectify(resource(), Some(actor.clone()), "title", 1_000)
        .await
        .expect("seed provenance");
    rights
        .soft_delete(resource(), Some(actor), "superseded", 2_000)
        .await
        .expect("seed provenance");

    let request = BundleRequest {
        resource: resource(),
        exported_at_unix_ms: 3_000,
        metadata_jsonld: json!({
            "@context": {"@vocab": "https://schema.org/"},
            "@type": "Dataset",
            "name": "Reference Measurements",
        }),
        citation: citation_input(),
        policy: policy(),
        artifacts: vec![artifact()],
    };
    (request, store)
}

#[tokio::test]
async fn bundle_round_trips_with_every_section_present() {
    let (request, store) = request_with_history().await;
    let sealed = build_bundle(request, &store).await.expect("build bundle");

    // Parse the canonical bytes back and check every section.
    let parsed: ExportBundle =
        serde_json::from_str(&sealed.canonical_json).expect("canonical JSON parses back");
    assert_eq!(parsed, sealed.bundle);

    assert_eq!(parsed.export_bundle.version, BUNDLE_VERSION);
    assert_eq!(parsed.export_bundle.resource, resource());
    assert_eq!(parsed.export_bundle.exported_at_unix_ms, 3_000);

    assert_eq!(parsed.metadata_jsonld["@type"], "Dataset");

    assert!(parsed.citations.bibtex.starts_with("@misc{resource_0001"));
    assert!(parsed.citations.ris.starts_with("TY  - DATA"));
    assert!(parsed.citations.cff.starts_with("cff-version: 1.2.0"));
    assert!(parsed.citations.apa.contains("[Data set]"));
    assert_eq!(parsed.citations.csl_json[0]["type"], "dataset");

    assert_eq!(parsed.provenance.entries.len(), 2);
    assert_eq!(parsed.provenance.entries[0].activity.tag(), "modified");
    assert_eq!(parsed.provenance.entries[1].activity.tag(), "deleted");

    let license = parsed.license.expect("license section");
    assert_eq!(license.spdx_id, "CC-BY-4.0");
    assert_eq!(
        license.rights_uri,
        "https://creativecommons.org/licenses/by/4.0/"
    );

    let restriction = parsed.data_use_restriction.expect("restriction section");
    assert_eq!(restriction.kind, "DUO:0000042");

    assert_eq!(parsed.artifacts.len(), 1);
    assert_eq!(parsed.artifacts[0].schema_fingerprint, "fp_v1_ab12");
    assert_eq!(parsed.artifacts[0].row_count, 128);
    assert!(!parsed.artifacts[0].content_hash.is_empty());
}

#[tokio::test]
async fn bundle_content_hash_is_deterministic() {
    let (request, store) = request_with_history().await;
    let first = build_bundle(request.clone(), &store)
        .await
        .expect("first build");
    let second = build_bundle(request, &store).await.expect("second build");

    assert_eq!(first.canonical_json, second.canonical_json);
    assert_eq!(first.content_hash, second.content_hash);
    assert_eq!(first.content_hash.len(), 64, "hex blake3 digest");
    assert_eq!(
        first.content_hash,
        content_hash_hex(first.canonical_json.as_bytes())
    );
}

#[test]
fn canonical_json_sorts_keys_at_every_depth() {
    let value = json!({
        "zebra": {"b": 1, "a": [{"y": 2, "x": 3}]},
        "alpha": true,
    });
    assert_eq!(
        canonical_json_string(&value),
        r#"{"alpha":true,"zebra":{"a":[{"x":3,"y":2}],"b":1}}"#
    );
}

#[test]
fn cff_matches_known_good_fixture() {
    let cff = generate_citation(&citation_input(), CitationFormat::Cff);
    let expected = "cff-version: 1.2.0\n\
        message: \"If you use this dataset, please cite it as below.\"\n\
        title: \"Reference Measurements\"\n\
        type: dataset\n\
        license: CC-BY-4.0\n\
        url: \"https://doi.org/10.1234/example.1\"\n\
        repository: \"https://example.org\"\n\
        keywords:\n\
        \x20 - measurements\n\
        \x20 - reference\n\
        date-released: \"2026-01-01\"\n\
        identifiers:\n\
        \x20 - type: doi\n\
        \x20   value: \"10.1234/example.1\"\n";
    assert_eq!(cff, expected);
}

#[test]
fn csl_json_is_a_single_item_array_with_strict_fields() {
    let rendered = generate_citation(&citation_input(), CitationFormat::CslJson);
    let parsed: Value = serde_json::from_str(&rendered).expect("valid JSON");
    let items = parsed.as_array().expect("CSL-JSON is an array");
    assert_eq!(items.len(), 1);
    let item = &items[0];
    assert_eq!(item["type"], "dataset");
    assert_eq!(item["DOI"], "10.1234/example.1");
    assert_eq!(item["issued"]["date-parts"][0][0], "2026");
}

#[test]
fn datacite_relation_type_parses_and_displays() {
    let parsed: RelationType = "IsDerivedFrom".parse().expect("parse");
    assert_eq!(parsed, RelationType::IsDerivedFrom);
    assert_eq!(parsed.to_string(), "IsDerivedFrom");
    assert!("not-a-relation".parse::<RelationType>().is_err());
}

#[test]
fn datacite_rights_from_license() {
    let rights = DataCiteRights::from_license(DataLicense::Cc0);
    assert_eq!(rights.rights_identifier.as_deref(), Some("CC0-1.0"));
    assert_eq!(rights.rights_identifier_scheme.as_deref(), Some("SPDX"));
}

fn datacite_metadata() -> DataCiteMetadata {
    DataCiteMetadata {
        identifier: DataCiteIdentifier {
            identifier: String::new(),
            identifier_type: IdentifierType::DOI,
        },
        creators: vec![DataCiteCreator {
            name: "Doe, Jane".to_owned(),
            given_name: Some("Jane".to_owned()),
            family_name: Some("Doe".to_owned()),
            name_identifier: None,
            affiliation: vec![],
        }],
        titles: vec![DataCiteTitle {
            title: "Reference Measurements".to_owned(),
            title_type: None,
        }],
        publisher: "ExamplePlatform".to_owned(),
        publication_year: 2026,
        resource_type: DataCiteResourceType {
            resource_type_general: ResourceTypeGeneral::Dataset,
            resource_type: None,
        },
        subjects: vec![],
        rights_list: vec![DataCiteRights::from_license(DataLicense::CcBy)],
        related_identifiers: vec![],
        descriptions: vec![],
        dates: vec![],
        schema_version: "4.5".to_owned(),
    }
}

/// Offline transport that records the request and replies like DataCite.
struct MockTransport {
    status: u16,
    reply: Value,
}

#[async_trait]
impl HttpTransport for MockTransport {
    async fn post_json(
        &self,
        url: &str,
        basic_auth: (&str, &str),
        body: &Value,
    ) -> Result<(u16, Value), DoiError> {
        assert_eq!(url, "https://api.test.datacite.org/dois");
        assert_eq!(basic_auth.0, "EXAMPLE.REPO");
        assert_eq!(body["data"]["type"], "dois");
        assert_eq!(body["data"]["attributes"]["event"], "publish");
        assert_eq!(body["data"]["attributes"]["prefix"], "10.1234");
        assert_eq!(
            body["data"]["attributes"]["url"],
            "https://example.org/resources/1"
        );
        Ok((self.status, self.reply.clone()))
    }
}

#[tokio::test]
async fn datacite_provider_mints_a_registered_record() {
    let provider = DataCiteProvider::new(
        "https://api.test.datacite.org",
        "EXAMPLE.REPO",
        "secret",
        "10.1234",
        MockTransport {
            status: 201,
            reply: json!({"data": {"attributes": {"doi": "10.1234/abcd-ef01"}}}),
        },
    );
    let record = provider
        .mint(
            resource(),
            &datacite_metadata(),
            "https://example.org/resources/1",
        )
        .await
        .expect("mint");

    assert_eq!(record.provider, "datacite");
    assert_eq!(record.status, DoiStatus::Registered);
    assert_eq!(record.doi.as_deref(), Some("10.1234/abcd-ef01"));
    assert!(record.last_error.is_none());
    assert_eq!(record.resource, resource());
}

#[tokio::test]
async fn datacite_provider_surfaces_registrar_rejection_as_failed_record() {
    let provider = DataCiteProvider::new(
        "https://api.test.datacite.org",
        "EXAMPLE.REPO",
        "secret",
        "10.1234",
        MockTransport {
            status: 422,
            reply: json!({"errors": [{"title": "This DOI has already been taken"}]}),
        },
    );
    let record = provider
        .mint(
            resource(),
            &datacite_metadata(),
            "https://example.org/resources/1",
        )
        .await
        .expect("registrar rejection is not a transport error");

    assert_eq!(record.status, DoiStatus::Failed);
    assert!(record.doi.is_none());
    assert_eq!(
        record.last_error.as_deref(),
        Some("This DOI has already been taken")
    );
}
