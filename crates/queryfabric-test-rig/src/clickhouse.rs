#![allow(missing_docs)]
//! Multi-node ClickHouse test helpers.
//!
//! Provides cluster XML configuration, DDL application across shards, and
//! raw HTTP query execution for distributed ClickHouse test scenarios.

use std::net::TcpStream;
use std::time::Duration;

/// Generate ClickHouse cluster XML configuration.
pub fn cluster_xml(cluster: &str, nodes: &[(impl AsRef<str>, u16)]) -> String {
    let mut xml = format!(
        r#"<clickhouse><remote_servers><{}><shard><internal_replication>true</internal_replication>"#,
        cluster
    );
    for (host, port) in nodes {
        xml.push_str(&format!(
            r#"<replica><host>{}</host><port>{}</port></replica>"#,
            host.as_ref(),
            port
        ));
    }
    xml.push_str(&format!(
        "</shard></{}></remote_servers></clickhouse>",
        cluster
    ));
    xml
}

/// Split a ClickHouse DDL script into individual statements.
///
/// Handles multi-line statements and respects quoted strings.
pub fn split_ddl_statements(sql: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut current = String::new();
    let mut in_single_quote = false;
    let mut in_backtick = false;

    for ch in sql.chars() {
        match ch {
            '\'' if !in_backtick => in_single_quote = !in_single_quote,
            '`' if !in_single_quote => in_backtick = !in_backtick,
            ';' if !in_single_quote && !in_backtick => {
                let trimmed = current.trim().to_owned();
                if !trimmed.is_empty() {
                    statements.push(trimmed);
                }
                current.clear();
                continue;
            }
            _ => {}
        }
        current.push(ch);
    }

    let trimmed = current.trim().to_owned();
    if !trimmed.is_empty() {
        statements.push(trimmed);
    }

    statements
}

/// Execute SQL on a ClickHouse node via HTTP interface.
pub async fn execute_ch(port: u16, sql: &str) -> Result<String, String> {
    use std::time::Duration;

    let url = format!("http://localhost:{port}/?query={}", url_encode(sql));
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| format!("HTTP client: {e}"))?;

    let resp = client.post(&url).send().await.map_err(|e| {
        if e.is_connect() {
            format!("ClickHouse not reachable on port {port}")
        } else {
            format!("ClickHouse query failed: {e}")
        }
    })?;

    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();

    if !status.is_success() {
        Err(format!("ClickHouse error ({status}): {body}"))
    } else {
        Ok(body)
    }
}

/// Execute SQL and parse the first column of the first row as u64.
pub async fn query_u64(port: u16, sql: &str) -> Result<u64, String> {
    let body = execute_ch(port, sql).await?;
    body.trim()
        .parse()
        .map_err(|e| format!("Parse u64 from ClickHouse response '{body}': {e}"))
}

fn url_encode(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            ' ' => "%20".into(),
            '\n' => "%0A".into(),
            '\t' => "%09".into(),
            '&' => "%26".into(),
            '=' => "%3D".into(),
            '?' => "%3F".into(),
            ';' => "%3B".into(),
            '#' => "%23".into(),
            _ => c.to_string(),
        })
        .collect()
}

/// Wait for a ClickHouse node to be ready.
pub async fn wait_node(port: u16, timeout_secs: u64) -> Result<(), String> {
    let start = std::time::Instant::now();
    loop {
        if start.elapsed().as_secs() > timeout_secs {
            return Err(format!(
                "ClickHouse node on port {port} not ready within {timeout_secs}s"
            ));
        }
        let addr = format!("127.0.0.1:{port}");
        if TcpStream::connect_timeout(
            &addr.parse().map_err(|e| format!("Socket addr: {e}"))?,
            Duration::from_secs(1),
        )
        .is_ok()
        {
            // Quick liveness check
            if execute_ch(port, "SELECT 1").await.is_ok() {
                return Ok(());
            }
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}
