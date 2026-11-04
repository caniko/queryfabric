//! A client that goes away must not leave its query running on the server.
//!
//! Needs a throwaway ClickHouse over HTTP:
//!
//! ```text
//! QF_TEST_CLICKHOUSE_HOST=127.0.0.1 QF_TEST_CLICKHOUSE_PORT=8123 \
//!   cargo test -p queryfabric-adapter-clickhouse --test cancel_on_disconnect -- --ignored --nocapture
//! ```

use std::time::{Duration, Instant};

use queryfabric_adapter_clickhouse::{ClickHouseConfig, DynamicClient};

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

#[allow(deprecated)]
async fn running(client: &DynamicClient, marker: &str) -> u64 {
    let json = client
        .query_json(&format!(
            "SELECT count() AS n FROM system.processes \
             WHERE position(query, '{marker}') > 0 AND query NOT LIKE '%system.processes%'"
        ))
        .await
        .expect("system.processes");
    let value: serde_json::Value = serde_json::from_str(json.lines().next().unwrap()).unwrap();
    value["n"]
        .as_u64()
        .or_else(|| value["n"].as_str()?.parse().ok())
        .unwrap()
}

/// Dropping the stream receiver (what a disconnected DoGet does) must make the
/// server cancel the scan within seconds, not when it would have finished.
#[tokio::test]
#[ignore = "needs a throwaway ClickHouse (QF_TEST_CLICKHOUSE_HOST)"]
async fn dropped_stream_cancels_the_server_side_query() {
    let Some(host) = env("QF_TEST_CLICKHOUSE_HOST") else {
        eprintln!("skipping: QF_TEST_CLICKHOUSE_HOST is not set");
        return;
    };
    let client = DynamicClient::from_config(&ClickHouseConfig {
        host,
        port: env("QF_TEST_CLICKHOUSE_PORT").map_or(8123, |p| p.parse().unwrap()),
        username: env("QF_TEST_CLICKHOUSE_USER").unwrap_or_else(|| "default".to_owned()),
        password: env("QF_TEST_CLICKHOUSE_PASSWORD")
            .unwrap_or_default()
            .into(),
        ..Default::default()
    });
    let marker = format!("qf-cancel-test-{}", std::process::id());
    // Produces no output until the whole aggregate finishes (~100 s), like an
    // audit scan: the stream task has nothing to send, so only a dropped
    // request can stop it.
    let sql = format!(
        "SELECT sum(x) FROM (SELECT sleepEachRow(0.5) AS x FROM numbers(200) \
         SETTINGS max_block_size = 1) /* {marker} */"
    );
    let rx = client.query_arrow_stream(&sql);

    let started = Instant::now();
    while running(&client, &marker).await == 0 {
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "the query never appeared in system.processes"
        );
        tokio::time::sleep(Duration::from_millis(300)).await;
    }

    drop(rx);
    let dropped_at = Instant::now();
    while running(&client, &marker).await != 0 {
        assert!(
            dropped_at.elapsed() < Duration::from_secs(20),
            "the query kept running {:?} after its receiver was dropped",
            dropped_at.elapsed()
        );
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
    eprintln!("cancelled {:?} after the drop", dropped_at.elapsed());
}
