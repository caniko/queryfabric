use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Hex-encoded BLAKE3 digest of `bytes`.
#[must_use]
pub fn content_hash_hex(bytes: &[u8]) -> String {
    queryfabric_content_hash::hash_bytes(bytes)
}

/// Integrity manifest for one stored artifact of a resource.
///
/// Carries the FAIR-integrity facts a consumer needs to verify and interpret
/// an artifact without fetching it: where it lives, its format, its schema
/// fingerprint, its content hash, and its size.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactManifest {
    /// What kind of artifact this is (e.g. `"table_export"`, `"bundle"`).
    pub kind: String,
    /// Where the artifact is stored (e.g. an S3 URI).
    pub storage_uri: String,
    /// Serialization format (e.g. `"parquet"`, `"json"`).
    pub format: String,
    /// Stable fingerprint of the artifact's schema.
    pub schema_fingerprint: String,
    /// Hex content hash of the artifact bytes (BLAKE3 unless stated
    /// otherwise by the host).
    pub content_hash: String,
    /// Number of rows, where the format has rows.
    pub row_count: u64,
    /// Artifact size in bytes, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub byte_count: Option<u64>,
    /// Host-specific manifest extension, carried opaquely.
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub manifest_json: Value,
}
