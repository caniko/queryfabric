use queryfabric_portability::{
    ImportLimits, TabularSchema, canonical_json_string_v2, content_hash_hex,
    decode_tabular_csv, tabular_schema_fingerprint, validate_import_bundle,
};
use serde_json::Value;

const BUNDLE_SCHEMA: &str = include_str!("../schema/bundle-2.0.json");
const TABULAR_SCHEMA: &str = include_str!("../schema/tabular-csv-1.json");
const JCS_VECTOR: &str = include_str!("../fixtures/rfc8785-vector.json");
const DUPLICATE_KEY: &[u8] = include_bytes!("../fixtures/invalid-duplicate-key.json");
const VALID_TABULAR_SCHEMA: &str = include_str!("../fixtures/valid-tabular-schema.json");

#[test]
fn published_schemas_are_machine_readable_and_pin_profile_constants() {
    let bundle: Value = serde_json::from_str(BUNDLE_SCHEMA).expect("bundle schema JSON");
    assert_eq!(bundle["$schema"], "https://json-schema.org/draft/2020-12/schema");
    assert_eq!(bundle["properties"]["exportBundle"]["properties"]["version"]["const"], "2.0");
    assert_eq!(bundle["properties"]["artifacts"]["items"]["$ref"], "#/$defs/artifact");

    let tabular: Value = serde_json::from_str(TABULAR_SCHEMA).expect("tabular schema JSON");
    assert_eq!(tabular["properties"]["profile"]["const"], "queryfabric.tabular-csv/1");
    assert_eq!(tabular["properties"]["columns"]["maxItems"], 128);

    let schema: TabularSchema = serde_json::from_str(VALID_TABULAR_SCHEMA).expect("typed schema");
    assert_eq!(schema.profile, "queryfabric.tabular-csv/1");
    assert_eq!(schema.columns.len(), 3);
    assert!(tabular_schema_fingerprint(&schema).starts_with("blake3-256:"));
    let csv = b"id,reading,label\r\n00000000-0000-0000-0000-000000000001,1.5,ok\r\n";
    assert_eq!(decode_tabular_csv(csv, &schema, ImportLimits::default()).unwrap().len(), 1);
}

#[test]
fn public_vector_matches_rfc8785_and_typed_blake3() {
    let vector: Value = serde_json::from_str(JCS_VECTOR).expect("JCS vector JSON");
    let canonical = canonical_json_string_v2(&vector["input"]);
    assert_eq!(canonical, vector["canonical"]);
    let digest = format!("blake3-256:{}", content_hash_hex(canonical.as_bytes()));
    println!("JCS vector digest: {digest}");
    assert_eq!(digest.len(), "blake3-256:".len() + 64);
}

#[test]
fn duplicate_key_fixture_is_rejected_before_bundle_validation() {
    let result = validate_import_bundle(DUPLICATE_KEY, "blake3-256:", ImportLimits::default());
    assert!(matches!(result, Err(queryfabric_portability::ImportError::InvalidJson(_))));
}
