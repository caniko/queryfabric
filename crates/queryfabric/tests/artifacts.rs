use std::path::PathBuf;

use queryfabric::builtin_capability_manifest;
use serde_json::Value;

mod support;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/")
        .parent()
        .expect("repo root")
        .to_path_buf()
}

#[test]
fn builtin_capability_manifest_matches_runtime_contract() {
    let manifest_path = repo_root().join("capabilities/builtin-capability-manifest.json");
    let raw = std::fs::read_to_string(manifest_path).expect("manifest");
    let json: Value = serde_json::from_str(&raw).expect("valid json");
    let expected = serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "backends": builtin_capability_manifest()
            .into_iter()
            .map(|entry| serde_json::json!({
                "backend": entry.backend,
                "features": entry
                    .capabilities
                    .features
                    .into_iter()
                    .map(|feature| format!("{feature:?}"))
                    .collect::<Vec<_>>(),
            }))
            .collect::<Vec<_>>(),
    });
    assert_eq!(json, expected);
}

#[test]
fn portable_subset_conformance_corpus_matches_seeded_cases() {
    let corpus_path = repo_root().join("conformance/portable-subset.json");
    let raw = std::fs::read_to_string(corpus_path).expect("corpus");
    let json: Value = serde_json::from_str(&raw).expect("valid json");
    let expected = support::portable_subset_seed_json();
    assert_eq!(json, expected);
}
