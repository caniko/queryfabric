use serde_json::Value;

/// RFC 8785 JSON Canonicalization Scheme used by import-ready bundle 2.0.
///
/// Bundle 1.0 deliberately keeps [`canonical_json_string`] for compatibility;
/// callers must opt into this function when producing or verifying 2.0 data.
pub fn canonical_json_string_v2(value: &Value) -> String {
    serde_jcs::to_string(value).expect("serde_json::Value is serializable")
}

/// Serialize a JSON value canonically: object keys sorted lexicographically,
/// no insignificant whitespace, serde_json's standard number and string
/// formatting.
///
/// Two structurally equal values always produce identical bytes, regardless
/// of construction order or whether `serde_json`'s `preserve_order` feature
/// is enabled elsewhere in the dependency graph. This is what makes bundle
/// content addressing deterministic.
#[must_use]
pub fn canonical_json_string(value: &Value) -> String {
    let mut out = String::new();
    write_canonical(value, &mut out);
    out
}

fn write_canonical(value: &Value, out: &mut String) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => out.push_str(&n.to_string()),
        Value::String(s) => {
            // serde_json's escaping is deterministic.
            out.push_str(&Value::String(s.clone()).to_string());
        }
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_canonical(item, out);
            }
            out.push(']');
        }
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort_unstable();
            out.push('{');
            for (i, key) in keys.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&Value::String(key.clone()).to_string());
                out.push(':');
                write_canonical(&map[key], out);
            }
            out.push('}');
        }
    }
}
