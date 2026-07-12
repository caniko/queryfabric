use queryfabric_portability::{
    ImportLimits, TabularSchema, canonical_json_string_v2, content_hash_hex, decode_tabular_csv,
    tabular_schema_fingerprint, validate_import_bundle,
};
use serde_json::Value;

const BUNDLE_SCHEMA: &str = include_str!("../schema/bundle-2.0.json");
const TABULAR_SCHEMA: &str = include_str!("../schema/tabular-csv-1.json");
const JCS_VECTOR: &str = include_str!("../fixtures/rfc8785-vector.json");
const DUPLICATE_KEY: &[u8] = include_bytes!("../fixtures/invalid-duplicate-key.json");
const VALID_TABULAR_SCHEMA: &str = include_str!("../fixtures/valid-tabular-schema.json");
const VALID_BUNDLE: &[u8] = include_bytes!("../fixtures/valid-bundle-2.0.json");

#[test]
fn published_schemas_are_machine_readable_and_pin_profile_constants() {
    let bundle: Value = serde_json::from_str(BUNDLE_SCHEMA).expect("bundle schema JSON");
    assert_eq!(
        bundle["$schema"],
        "https://json-schema.org/draft/2020-12/schema"
    );
    assert_eq!(
        bundle["properties"]["exportBundle"]["properties"]["version"]["const"],
        "2.0"
    );
    assert_eq!(
        bundle["properties"]["artifacts"]["items"]["$ref"],
        "#/$defs/artifact"
    );

    let tabular: Value = serde_json::from_str(TABULAR_SCHEMA).expect("tabular schema JSON");
    assert_eq!(
        tabular["properties"]["profile"]["const"],
        "queryfabric.tabular-csv/1"
    );
    assert_eq!(tabular["properties"]["columns"]["maxItems"], 128);

    let schema: TabularSchema = serde_json::from_str(VALID_TABULAR_SCHEMA).expect("typed schema");
    assert_eq!(schema.profile, "queryfabric.tabular-csv/1");
    assert_eq!(schema.columns.len(), 3);
    println!("schema digest: {}", tabular_schema_fingerprint(&schema));
    let csv = b"id,reading,label\r\n00000000-0000-0000-0000-000000000001,1.5,ok\r\n";
    println!("csv digest: blake3-256:{}", content_hash_hex(csv));
    assert_eq!(
        decode_tabular_csv(csv, &schema, ImportLimits::default())
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn public_vector_matches_rfc8785_and_typed_blake3() {
    let vector: Value = serde_json::from_str(JCS_VECTOR).expect("JCS vector JSON");
    let canonical = canonical_json_string_v2(&vector["input"]);
    assert_eq!(canonical, vector["canonical"]);
    let digest = format!("blake3-256:{}", content_hash_hex(canonical.as_bytes()));
    assert_eq!(digest, vector["typedDigest"]);
}

#[test]
fn duplicate_key_fixture_is_rejected_before_bundle_validation() {
    let result = validate_import_bundle(DUPLICATE_KEY, "blake3-256:", ImportLimits::default());
    assert!(matches!(
        result,
        Err(queryfabric_portability::ImportError::InvalidJson(_))
    ));
}

#[test]
fn valid_bundle_fixture_verifies_against_its_staged_artifact() {
    let value: Value = serde_json::from_slice(VALID_BUNDLE).expect("valid bundle JSON");
    let canonical = canonical_json_string_v2(&value);
    let digest = format!("blake3-256:{}", content_hash_hex(canonical.as_bytes()));
    println!("bundle digest: {digest}");
    assert_eq!(
        digest,
        "blake3-256:4d5083d6acdd6e3fe053c21b380cfa579b751a60565a1dfc37c056b64f32363c"
    );
    let bundle = validate_import_bundle(VALID_BUNDLE, &digest, ImportLimits::default())
        .expect("fixture validates");
    let schema: TabularSchema = serde_json::from_value(bundle.artifacts[0].manifest_json.clone())
        .expect("typed artifact schema");
    let csv = b"id,reading,label\r\n00000000-0000-0000-0000-000000000001,1.5,ok\r\n";
    assert_eq!(
        tabular_schema_fingerprint(&schema),
        bundle.artifacts[0].schema_fingerprint
    );
    assert_eq!(bundle.bundle_digest, digest);
    assert_eq!(bundle.artifacts[0].byte_count, Some(csv.len() as u64));
}
