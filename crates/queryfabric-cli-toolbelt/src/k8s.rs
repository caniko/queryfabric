#![allow(missing_docs)]
use serde::Deserialize;

/// Minimal Kubernetes Job representation for `kubectl get -o json` output.
#[derive(Debug, Clone, Deserialize)]
pub struct Job {
    pub metadata: Metadata,
    pub status: Option<JobStatus>,
}

/// Minimal Pod representation.
#[derive(Debug, Clone, Deserialize)]
pub struct Pod {
    pub metadata: Metadata,
    pub status: Option<PodStatus>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Metadata {
    pub name: Option<String>,
    pub namespace: Option<String>,
    pub labels: Option<std::collections::BTreeMap<String, String>>,
    pub uid: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JobStatus {
    pub active: Option<i32>,
    pub succeeded: Option<i32>,
    pub failed: Option<i32>,
    pub conditions: Option<Vec<JobCondition>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JobCondition {
    #[serde(rename = "type")]
    pub type_: String,
    pub status: String,
    pub reason: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PodStatus {
    pub phase: Option<String>,
    pub container_statuses: Option<Vec<ContainerStatus>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ContainerStatus {
    pub name: String,
    pub ready: bool,
    pub state: ContainerStateValue,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ContainerStateValue {
    pub running: Option<serde_json::Value>,
    pub terminated: Option<ContainerTerminated>,
    pub waiting: Option<ContainerWaiting>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ContainerTerminated {
    pub exit_code: i32,
    pub reason: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ContainerWaiting {
    pub reason: Option<String>,
    pub message: Option<String>,
}

/// A Kubernetes Secret resource (minimal representation for `kubectl get -o json`).
#[derive(Debug, Clone, Deserialize)]
pub struct Secret {
    /// Base64-encoded key-value pairs.
    pub data: Option<std::collections::BTreeMap<String, String>>,
}

/// Run kubectl and return raw JSON output.
pub fn kubectl_json(args: &[&str]) -> Result<serde_json::Value, String> {
    let output = std::process::Command::new("kubectl")
        .args(args)
        .output()
        .map_err(|e| format!("kubectl not found: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("kubectl {} failed: {stderr}", args.join(" ")));
    }
    serde_json::from_slice(&output.stdout).map_err(|e| format!("kubectl JSON parse error: {e}"))
}

/// List Jobs by label selector.
pub fn list_jobs(namespace: &str, label_selector: &str) -> Result<Vec<Job>, String> {
    let list = kubectl_json(&[
        "get",
        "jobs",
        "-n",
        namespace,
        "-l",
        label_selector,
        "-o",
        "json",
    ])?;
    Ok(serde_json::from_value(list["items"].clone()).unwrap_or_default())
}

/// Delete Jobs by label selector.
pub fn delete_jobs(namespace: &str, label_selector: &str) -> Result<(), String> {
    kubectl_json(&[
        "delete",
        "jobs",
        "-n",
        namespace,
        "-l",
        label_selector,
        "--ignore-not-found",
    ])?;
    Ok(())
}

/// List Pods for a Job.
pub fn list_pods(namespace: &str, job_name: &str) -> Result<Vec<Pod>, String> {
    let list = kubectl_json(&[
        "get",
        "pods",
        "-n",
        namespace,
        "-l",
        &format!("job-name={job_name}"),
        "-o",
        "json",
    ])?;
    Ok(serde_json::from_value(list["items"].clone()).unwrap_or_default())
}

/// Read a Secret field value (base64 decoded).
pub fn read_secret(namespace: &str, name: &str, key: &str) -> Result<String, String> {
    let value: Secret = serde_json::from_value(kubectl_json(&[
        "get", "secret", "-n", namespace, name, "-o", "json",
    ])?)
    .map_err(|e| format!("Secret JSON parse error: {e}"))?;
    let data = value
        .data
        .ok_or_else(|| format!("Secret {name} has no data"))?;
    let encoded = data
        .get(key)
        .ok_or_else(|| format!("Secret {name} has no key {key}"))?;
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|e| format!("base64 decode error: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("UTF-8 decode error: {e}"))
}

/// Parse a Kubernetes resource quantity string into millicores/mebibytes.
pub fn parse_quantity(value: &str) -> Option<u64> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    let (num_str, suffix) = split_quantity_suffix(value);
    let num: f64 = num_str.parse().ok()?;
    let multiplier = match suffix {
        "Ki" => 1024_f64,
        "Mi" => 1024_f64 * 1024_f64,
        "Gi" => 1024_f64 * 1024_f64 * 1024_f64,
        "Ti" => 1024_f64 * 1024_f64 * 1024_f64 * 1024_f64,
        "m" => 0.001_f64,
        "K" => 1000_f64,
        "M" => 1_000_000_f64,
        "G" => 1_000_000_000_f64,
        "T" => 1_000_000_000_000_f64,
        "" => 1_f64,
        _ => return None,
    };
    Some((num * multiplier) as u64)
}

fn split_quantity_suffix(value: &str) -> (&str, &str) {
    let suffixes = ["Ti", "Gi", "Mi", "Ki", "m", "T", "G", "M", "K"];
    for suffix in &suffixes {
        if let Some(rest) = value.strip_suffix(suffix) {
            return (rest, suffix);
        }
    }
    (value, "")
}
