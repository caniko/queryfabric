//! Bounded validation and planning for the import-ready tabular profile.
//!
//! This module is intentionally transport- and host-neutral.  It verifies the
//! bundle envelope and staged artifact bytes, but never follows a `storageUri`
//! or performs a database write.  Hosts supply the predeclared target and own
//! authorization, persistence, and transaction semantics.

use chrono::DateTime;
use queryfabric_contract::ResourceRef;
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::ExportBundle;
use crate::canonical::canonical_json_string_v2;
use crate::content_hash_hex;
use crate::manifest::ArtifactManifest;

/// Import-ready envelope version.  Version 1.0 remains export-only.
pub const IMPORT_BUNDLE_VERSION: &str = "2.0";
/// The single profile implemented by the reference host MVP.
pub const TABULAR_CSV_PROFILE: &str = "queryfabric.tabular-csv/1";

/// Resource limits applied before any host-visible mutation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportLimits {
    pub max_bundle_bytes: usize,
    pub max_json_depth: usize,
    pub max_string_bytes: usize,
    pub max_artifacts: usize,
    pub max_artifact_bytes: usize,
    pub max_total_artifact_bytes: usize,
    pub max_rows: usize,
    pub max_columns: usize,
    pub max_extension_bytes: usize,
}

impl Default for ImportLimits {
    fn default() -> Self {
        Self {
            max_bundle_bytes: 8 * 1024 * 1024,
            max_json_depth: 32,
            max_string_bytes: 256 * 1024,
            max_artifacts: 8,
            max_artifact_bytes: 512 * 1024 * 1024,
            max_total_artifact_bytes: 512 * 1024 * 1024,
            max_rows: 5_000_000,
            max_columns: 128,
            max_extension_bytes: 512 * 1024,
        }
    }
}

/// Scalar types accepted by `queryfabric.tabular-csv/1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TabularColumnType {
    Boolean,
    Int64,
    Float64,
    String,
    Uuid,
    Timestamp,
}

/// One ordered, non-nullable portable column.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TabularColumn {
    pub name: String,
    #[serde(rename = "type")]
    pub column_type: TabularColumnType,
}

/// Typed schema carried in the table artifact extension.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TabularSchema {
    pub profile: String,
    pub columns: Vec<TabularColumn>,
}

/// The host's predeclared target relation and local authorization decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanTarget {
    pub target_resource: ResourceRef,
    pub relation: String,
    pub target_revision: String,
    pub expected_schema: TabularSchema,
    pub local_owner: Uuid,
}

/// A validated 2.0 bundle.  Artifact bytes are deliberately not retained.
#[derive(Debug, Clone, PartialEq)]
pub struct ValidatedBundle {
    pub bundle: ExportBundle,
    pub canonical_json: String,
    pub bundle_digest: String,
    pub artifacts: Vec<ArtifactManifest>,
    pub source_resource: ResourceRef,
}

/// A deterministic, host-neutral import proposal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPlan {
    pub bundle_digest: String,
    pub artifact_digest: String,
    pub source_resource: ResourceRef,
    pub target: PlanTarget,
    pub column_mapping: Vec<TabularColumn>,
    pub row_count: u64,
    pub byte_count: u64,
    pub plan_digest: String,
}

/// Validation and planning failures.  Messages are stable enough for an API
/// diagnostic, while avoiding raw parser or database internals.
#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("bundle exceeds configured byte limit")]
    BundleTooLarge,
    #[error("artifact exceeds configured byte limit")]
    ArtifactTooLarge,
    #[error("aggregate artifact bytes exceed configured limit")]
    AggregateArtifactTooLarge,
    #[error("bundle JSON is invalid: {0}")]
    InvalidJson(String),
    #[error("bundle JSON exceeds configured nesting or string limits")]
    JsonLimits,
    #[error("unsupported bundle version '{0}'")]
    UnsupportedVersion(String),
    #[error("bundle digest must use blake3-256:<64 lowercase hex>")]
    InvalidDigest,
    #[error("bundle digest does not match expected digest")]
    DigestMismatch,
    #[error("bundle contains too many artifacts")]
    TooManyArtifacts,
    #[error("duplicate or conflicting artifact entry")]
    DuplicateArtifact,
    #[error("artifact manifest is invalid: {0}")]
    InvalidManifest(String),
    #[error("tabular profile is invalid: {0}")]
    InvalidProfile(String),
    #[error("CSV artifact is invalid: {0}")]
    InvalidCsv(String),
    #[error("target mapping is invalid: {0}")]
    InvalidTarget(String),
    #[error("schema fingerprint does not match the typed schema")]
    SchemaMismatch,
    #[error("declared row or byte count does not match the staged artifact")]
    ArtifactMismatch,
}

/// Parse and verify a bundle 2.0 document.  The expected digest is supplied by
/// an authenticated operator channel; it is never taken from `storageUri`.
pub fn validate_import_bundle(
    bytes: &[u8],
    expected_digest: &str,
    limits: ImportLimits,
) -> Result<ValidatedBundle, ImportError> {
    if bytes.len() > limits.max_bundle_bytes {
        return Err(ImportError::BundleTooLarge);
    }
    let value = parse_json_without_duplicate_keys(bytes)?;
    enforce_json_limits(&value, 0, &limits)?;
    let bundle: ExportBundle = serde_json::from_value(value.clone())
        .map_err(|error| ImportError::InvalidJson(error.to_string()))?;
    if bundle.export_bundle.version != IMPORT_BUNDLE_VERSION {
        return Err(ImportError::UnsupportedVersion(
            bundle.export_bundle.version,
        ));
    }
    let canonical_json = canonical_json_string_v2(&value);
    let extension_bytes = serde_json::to_vec(&bundle.metadata_jsonld)
        .map_err(|error| ImportError::InvalidJson(error.to_string()))?
        .len();
    if extension_bytes > limits.max_extension_bytes {
        return Err(ImportError::JsonLimits);
    }
    let digest = typed_digest(canonical_json.as_bytes());
    if !valid_typed_digest(expected_digest) {
        return Err(ImportError::InvalidDigest);
    }
    if !constant_time_eq(digest.as_bytes(), expected_digest.as_bytes()) {
        return Err(ImportError::DigestMismatch);
    }
    if bundle.artifacts.is_empty() || bundle.artifacts.len() > limits.max_artifacts {
        return Err(ImportError::TooManyArtifacts);
    }
    let mut total_bytes = 0usize;
    let mut seen = std::collections::HashSet::new();
    for artifact in &bundle.artifacts {
        if artifact.kind != "table_export" || artifact.format != "csv" {
            return Err(ImportError::InvalidManifest(
                "2.0 accepts exactly one table_export CSV profile".to_owned(),
            ));
        }
        let byte_count = artifact
            .byte_count
            .ok_or_else(|| ImportError::InvalidManifest("byteCount is required".to_owned()))?;
        if byte_count > limits.max_artifact_bytes as u64 {
            return Err(ImportError::ArtifactTooLarge);
        }
        total_bytes = total_bytes
            .checked_add(byte_count as usize)
            .ok_or(ImportError::AggregateArtifactTooLarge)?;
        if total_bytes > limits.max_total_artifact_bytes {
            return Err(ImportError::AggregateArtifactTooLarge);
        }
        if !valid_typed_digest(&artifact.content_hash)
            || !valid_typed_digest(&artifact.schema_fingerprint)
        {
            return Err(ImportError::InvalidManifest(
                "contentHash and schemaFingerprint must be typed blake3-256 digests".to_owned(),
            ));
        }
        if !seen.insert((artifact.kind.clone(), artifact.storage_uri.clone())) {
            return Err(ImportError::DuplicateArtifact);
        }
        if artifact.storage_uri.is_empty()
            || artifact.storage_uri.starts_with('/')
            || artifact.storage_uri.contains("..")
            || artifact.storage_uri.contains("://")
        {
            return Err(ImportError::InvalidManifest(
                "storageUri is metadata only and must be a bounded relative name".to_owned(),
            ));
        }
        let schema: TabularSchema = serde_json::from_value(artifact.manifest_json.clone())
            .map_err(|error| ImportError::InvalidProfile(error.to_string()))?;
        if serde_json::to_vec(&artifact.manifest_json)
            .map_err(|error| ImportError::InvalidJson(error.to_string()))?
            .len()
            > limits.max_extension_bytes
        {
            return Err(ImportError::JsonLimits);
        }
        validate_schema(&schema, limits.max_columns)?;
        if typed_digest(schema_bytes(&schema).as_bytes()) != artifact.schema_fingerprint {
            return Err(ImportError::SchemaMismatch);
        }
    }
    Ok(ValidatedBundle {
        source_resource: bundle.export_bundle.resource,
        artifacts: bundle.artifacts.clone(),
        bundle,
        canonical_json,
        bundle_digest: digest,
    })
}

/// Verify a staged CSV and produce a deterministic target mapping proposal.
pub fn plan_tabular_import(
    bundle: &ValidatedBundle,
    artifact_bytes: &[u8],
    target: PlanTarget,
    limits: ImportLimits,
) -> Result<ImportPlan, ImportError> {
    if bundle.artifacts.len() != 1 {
        return Err(ImportError::InvalidManifest(
            "exactly one table artifact is required".to_owned(),
        ));
    }
    if artifact_bytes.len() > limits.max_artifact_bytes {
        return Err(ImportError::ArtifactTooLarge);
    }
    validate_schema(&target.expected_schema, limits.max_columns)?;
    let manifest = &bundle.artifacts[0];
    let schema: TabularSchema = serde_json::from_value(manifest.manifest_json.clone())
        .map_err(|error| ImportError::InvalidProfile(error.to_string()))?;
    if schema != target.expected_schema {
        return Err(ImportError::InvalidTarget(
            "target schema does not equal the portable schema".to_owned(),
        ));
    }
    let actual_digest = typed_digest(artifact_bytes);
    let actual_bytes = artifact_bytes.len() as u64;
    if actual_digest != manifest.content_hash || Some(actual_bytes) != manifest.byte_count {
        return Err(ImportError::ArtifactMismatch);
    }
    let rows = parse_csv(artifact_bytes, &schema, limits)?;
    if rows.len() as u64 != manifest.row_count {
        return Err(ImportError::ArtifactMismatch);
    }
    if target.relation.trim().is_empty() || target.target_revision.trim().is_empty() {
        return Err(ImportError::InvalidTarget(
            "relation and targetRevision are required".to_owned(),
        ));
    }
    let mut plan = ImportPlan {
        bundle_digest: bundle.bundle_digest.clone(),
        artifact_digest: actual_digest,
        source_resource: bundle.source_resource,
        target,
        column_mapping: schema.columns,
        row_count: rows.len() as u64,
        byte_count: actual_bytes,
        plan_digest: String::new(),
    };
    let plan_value =
        serde_json::to_value(&plan).map_err(|error| ImportError::InvalidJson(error.to_string()))?;
    plan.plan_digest = typed_digest(canonical_json_string_v2(&plan_value).as_bytes());
    Ok(plan)
}

/// Encode JSON result rows using the normative profile-1 CSV rules.  The
/// function is intentionally strict: objects must contain exactly the schema
/// columns, in schema order, and nulls are not representable in profile 1.
pub fn write_tabular_csv(schema: &TabularSchema, rows: &[Value]) -> Result<Vec<u8>, ImportError> {
    validate_schema(schema, ImportLimits::default().max_columns)?;
    if rows.len() > ImportLimits::default().max_rows {
        return Err(ImportError::InvalidCsv("row limit exceeded".to_owned()));
    }
    let mut output = String::new();
    output.push_str(
        &schema
            .columns
            .iter()
            .map(|column| encode_csv_field(&column.name))
            .collect::<Vec<_>>()
            .join(","),
    );
    output.push_str("\r\n");
    for row in rows {
        let object = row
            .as_object()
            .ok_or_else(|| ImportError::InvalidCsv("each row must be a JSON object".to_owned()))?;
        if object.len() != schema.columns.len()
            || schema
                .columns
                .iter()
                .any(|column| !object.contains_key(&column.name))
            || object
                .keys()
                .any(|name| !schema.columns.iter().any(|column| &column.name == name))
        {
            return Err(ImportError::InvalidCsv(
                "row fields do not exactly match schema".to_owned(),
            ));
        }
        let values = schema
            .columns
            .iter()
            .map(|column| value_to_csv_text(object[&column.name].clone(), column.column_type))
            .collect::<Result<Vec<_>, _>>()?;
        validate_values(&values, schema)?;
        output.push_str(
            &values
                .iter()
                .map(|value| encode_csv_field(value))
                .collect::<Vec<_>>()
                .join(","),
        );
        output.push_str("\r\n");
    }
    Ok(output.into_bytes())
}

/// Decode a staged profile-1 CSV into validated string fields. Hosts decide
/// how those fields map to their predeclared relation and transaction.
pub fn decode_tabular_csv(
    bytes: &[u8],
    schema: &TabularSchema,
    limits: ImportLimits,
) -> Result<Vec<Vec<String>>, ImportError> {
    validate_schema(schema, limits.max_columns)?;
    parse_csv(bytes, schema, limits)
}

/// Fingerprint the canonical typed schema document used by profile 1.
#[must_use]
pub fn tabular_schema_fingerprint(schema: &TabularSchema) -> String {
    typed_digest(schema_bytes(schema).as_bytes())
}

fn typed_digest(bytes: &[u8]) -> String {
    format!("blake3-256:{}", content_hash_hex(bytes))
}

fn parse_json_without_duplicate_keys(bytes: &[u8]) -> Result<Value, ImportError> {
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let value = StrictValueSeed
        .deserialize(&mut deserializer)
        .map_err(|error| ImportError::InvalidJson(error.to_string()))?;
    deserializer
        .end()
        .map_err(|error| ImportError::InvalidJson(error.to_string()))?;
    Ok(value)
}

struct StrictValueSeed;

impl<'de> DeserializeSeed<'de> for StrictValueSeed {
    type Value = Value;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(StrictValueVisitor)
    }
}

struct StrictValueVisitor;

impl<'de> Visitor<'de> for StrictValueVisitor {
    type Value = Value;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a JSON value")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
        Ok(Value::Bool(value))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
        Ok(Value::Number(value.into()))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
        Ok(Value::Number(value.into()))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        serde_json::Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom("non-finite JSON number"))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
        Ok(Value::String(value.to_owned()))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(Value::String(value))
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(Value::Null)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(Value::Null)
    }

    fn visit_seq<A>(self, mut access: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::new();
        while let Some(value) = access.next_element_seed(StrictValueSeed)? {
            values.push(value);
        }
        Ok(Value::Array(values))
    }

    fn visit_map<A>(self, mut access: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut values = serde_json::Map::new();
        while let Some(key) = access.next_key::<String>()? {
            if values.contains_key(&key) {
                return Err(de::Error::custom(format!(
                    "duplicate JSON object key '{key}'"
                )));
            }
            let value = access.next_value_seed(StrictValueSeed)?;
            values.insert(key, value);
        }
        Ok(Value::Object(values))
    }
}

fn valid_typed_digest(value: &str) -> bool {
    let Some(hex) = value.strip_prefix("blake3-256:") else {
        return false;
    };
    hex.len() == 64
        && hex
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0u8, |difference, (a, b)| difference | (a ^ b))
        == 0
}

fn enforce_json_limits(
    value: &Value,
    depth: usize,
    limits: &ImportLimits,
) -> Result<(), ImportError> {
    if depth > limits.max_json_depth {
        return Err(ImportError::JsonLimits);
    }
    match value {
        Value::String(text) if text.len() > limits.max_string_bytes => Err(ImportError::JsonLimits),
        Value::Array(items) => items
            .iter()
            .try_for_each(|item| enforce_json_limits(item, depth + 1, limits)),
        Value::Object(map) => map.iter().try_for_each(|(key, item)| {
            if key.len() > limits.max_string_bytes {
                return Err(ImportError::JsonLimits);
            }
            enforce_json_limits(item, depth + 1, limits)
        }),
        _ => Ok(()),
    }
}

fn validate_schema(schema: &TabularSchema, max_columns: usize) -> Result<(), ImportError> {
    if schema.profile != TABULAR_CSV_PROFILE || schema.columns.is_empty() {
        return Err(ImportError::InvalidProfile(
            "profile must be queryfabric.tabular-csv/1 with at least one column".to_owned(),
        ));
    }
    if schema.columns.len() > max_columns {
        return Err(ImportError::InvalidProfile("too many columns".to_owned()));
    }
    let mut names = std::collections::HashSet::new();
    for column in &schema.columns {
        if column.name.is_empty()
            || column.name.len() > 256
            || !column.name.is_char_boundary(column.name.len())
            || !names.insert(column.name.clone())
        {
            return Err(ImportError::InvalidProfile(
                "column names must be unique and bounded UTF-8".to_owned(),
            ));
        }
    }
    Ok(())
}

fn schema_bytes(schema: &TabularSchema) -> String {
    let value = serde_json::to_value(schema).expect("schema serializes");
    canonical_json_string_v2(&value)
}

fn encode_csv_field(value: &str) -> String {
    if value
        .chars()
        .any(|character| matches!(character, ',' | '"' | '\r' | '\n'))
    {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

fn value_to_csv_text(value: Value, column_type: TabularColumnType) -> Result<String, ImportError> {
    match value {
        Value::String(value) => Ok(value),
        Value::Bool(value) if matches!(column_type, TabularColumnType::Boolean) => {
            Ok(value.to_string())
        }
        Value::Number(value)
            if matches!(
                column_type,
                TabularColumnType::Int64 | TabularColumnType::Float64
            ) =>
        {
            Ok(value.to_string())
        }
        Value::Null => Err(ImportError::InvalidCsv("nulls are not allowed".to_owned())),
        _ => Err(ImportError::InvalidCsv(
            "row value type does not match schema".to_owned(),
        )),
    }
}

fn parse_csv(
    bytes: &[u8],
    schema: &TabularSchema,
    limits: ImportLimits,
) -> Result<Vec<Vec<String>>, ImportError> {
    if bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
        return Err(ImportError::InvalidCsv("UTF-8 BOM is forbidden".to_owned()));
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|_| ImportError::InvalidCsv("artifact is not UTF-8".to_owned()))?;
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut field_started = false;
    let mut ended_record = false;
    let mut header_seen = false;
    let chars: Vec<char> = text.chars().collect();
    let mut index = 0;
    while index < chars.len() {
        let character = chars[index];
        if quoted {
            match character {
                '"' if index + 1 < chars.len() && chars[index + 1] == '"' => {
                    field.push('"');
                    index += 2;
                }
                '"' => {
                    quoted = false;
                    index += 1;
                }
                '\r' | '\n' => {
                    return Err(ImportError::InvalidCsv(
                        "embedded newlines are not supported by profile 1".to_owned(),
                    ));
                }
                _ => {
                    field.push(character);
                    index += 1;
                }
            }
            continue;
        }
        match character {
            '"' if !field_started => {
                quoted = true;
                field_started = true;
                index += 1;
            }
            '"' => {
                return Err(ImportError::InvalidCsv(
                    "quote must start a field".to_owned(),
                ));
            }
            ',' => {
                row.push(std::mem::take(&mut field));
                field_started = false;
                index += 1;
            }
            '\r' if index + 1 < chars.len() && chars[index + 1] == '\n' => {
                row.push(std::mem::take(&mut field));
                if row.len() != schema.columns.len() {
                    return Err(ImportError::InvalidCsv(
                        "field count does not match schema".to_owned(),
                    ));
                }
                if !header_seen {
                    if row
                        != schema
                            .columns
                            .iter()
                            .map(|column| column.name.clone())
                            .collect::<Vec<_>>()
                    {
                        return Err(ImportError::InvalidCsv(
                            "header does not exactly match schema".to_owned(),
                        ));
                    }
                    header_seen = true;
                } else {
                    validate_values(&row, schema)?;
                    rows.push(row.clone());
                    if rows.len() > limits.max_rows {
                        return Err(ImportError::InvalidCsv("row limit exceeded".to_owned()));
                    }
                }
                row.clear();
                field_started = false;
                ended_record = true;
                index += 2;
            }
            '\n' => return Err(ImportError::InvalidCsv("records must use CRLF".to_owned())),
            _ => {
                field.push(character);
                field_started = true;
                ended_record = false;
                index += 1;
            }
        }
    }
    if quoted || !ended_record || !row.is_empty() || !field.is_empty() {
        return Err(ImportError::InvalidCsv("CSV must end with CRLF".to_owned()));
    }
    Ok(rows)
}

fn validate_values(row: &[String], schema: &TabularSchema) -> Result<(), ImportError> {
    for (value, column) in row.iter().zip(&schema.columns) {
        let valid = match column.column_type {
            TabularColumnType::Boolean => matches!(value.as_str(), "true" | "false"),
            TabularColumnType::Int64 => valid_integer(value),
            TabularColumnType::Float64 => valid_float(value),
            TabularColumnType::String => true,
            TabularColumnType::Uuid => {
                value.parse::<Uuid>().is_ok() && *value == value.to_ascii_lowercase()
            }
            TabularColumnType::Timestamp => {
                value.ends_with('Z') && DateTime::parse_from_rfc3339(value).is_ok()
            }
        };
        if !valid {
            return Err(ImportError::InvalidCsv(format!(
                "invalid {} value for column '{}'",
                column.column_type.column_type_name(),
                column.name
            )));
        }
    }
    Ok(())
}

fn valid_integer(value: &str) -> bool {
    if value.is_empty() || value.starts_with('+') || value.parse::<i64>().is_err() {
        return false;
    }
    let digits = value.strip_prefix('-').unwrap_or(value);
    (digits == "0" || !digits.starts_with('0')) && digits.bytes().all(|byte| byte.is_ascii_digit())
}

fn valid_float(value: &str) -> bool {
    if value.is_empty() || value.starts_with('+') || value.parse::<f64>().is_err() {
        return false;
    }
    let bytes = value.as_bytes();
    let mut index = usize::from(bytes.first() == Some(&b'-'));
    let integer_start = index;
    while index < bytes.len() && bytes[index].is_ascii_digit() {
        index += 1;
    }
    if index == integer_start || (index - integer_start > 1 && bytes[integer_start] == b'0') {
        return false;
    }
    if index < bytes.len() && bytes[index] == b'.' {
        index += 1;
        let fraction_start = index;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
        if fraction_start == index {
            return false;
        }
    }
    if index < bytes.len() && matches!(bytes[index], b'e' | b'E') {
        index += 1;
        if index < bytes.len() && matches!(bytes[index], b'+' | b'-') {
            index += 1;
        }
        let exponent_start = index;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
        if exponent_start == index {
            return false;
        }
    }
    index == bytes.len()
        && value
            .parse::<f64>()
            .map(|number| number.is_finite())
            .unwrap_or(false)
}

impl TabularColumnType {
    fn column_type_name(self) -> &'static str {
        match self {
            Self::Boolean => "Boolean",
            Self::Int64 => "Int64",
            Self::Float64 => "Float64",
            Self::String => "String",
            Self::Uuid => "Uuid",
            Self::Timestamp => "Timestamp",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn schema() -> TabularSchema {
        TabularSchema {
            profile: TABULAR_CSV_PROFILE.to_owned(),
            columns: vec![
                TabularColumn {
                    name: "id".to_owned(),
                    column_type: TabularColumnType::Uuid,
                },
                TabularColumn {
                    name: "count".to_owned(),
                    column_type: TabularColumnType::Int64,
                },
                TabularColumn {
                    name: "label".to_owned(),
                    column_type: TabularColumnType::String,
                },
            ],
        }
    }

    #[test]
    fn jcs_uses_utf16_key_order_and_typed_digest() {
        let value = json!({"\u{e9}": 1, "a": 2});
        let canonical = canonical_json_string_v2(&value);
        assert_eq!(canonical, r#"{"a":2,"é":1}"#);
        assert!(valid_typed_digest(&typed_digest(canonical.as_bytes())));
    }

    #[test]
    fn csv_profile_rejects_lf_and_accepts_escaped_crlf_records() {
        let schema = schema();
        let header = "id,count,label\r\n";
        let row = "00000000-0000-0000-0000-000000000001,1,\"hello\"\"world\"\r\n";
        let bytes = format!("{header}{row}");
        assert!(parse_csv(bytes.as_bytes(), &schema, ImportLimits::default()).is_ok());
        assert!(parse_csv(b"id,count,label\n", &schema, ImportLimits::default()).is_err());
    }

    #[test]
    fn scalar_lexical_rules_are_strict() {
        assert!(valid_integer("-9223372036854775808"));
        assert!(!valid_integer("01"));
        assert!(!valid_integer("+1"));
        assert!(valid_float("1.5e-2"));
        assert!(!valid_float("NaN"));
        assert!(!valid_float("01.0"));
    }

    #[test]
    fn duplicate_json_keys_are_rejected_before_validation() {
        assert!(parse_json_without_duplicate_keys(br#"{"a":1,"a":2}"#).is_err());
    }
}
