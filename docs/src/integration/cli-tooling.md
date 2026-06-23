# CLI and Test Tooling

QueryFabric ships several utility crates that are useful both inside the
workspace and for external host projects.

## queryfabric-cli-toolbelt

Reusable CLI utilities that don't depend on any specific QueryFabric backend:

- **`flight`**: Arrow Flight client for `do_get` operations (feature-gated).
- **`auth`**: Auth token storage (`AuthStore`, `load_auth`, `save_auth`) with
  configurable app name and env prefix.
- **`clickhouse`**: ClickHouse connection args with env-based configuration.
- **`k8s`**: Kubernetes resource types (`Job`, `Pod`, `Secret`) and kubectl
  helpers. `parse_quantity` handles K8s resource quantity strings.
- **`process`**: Subprocess execution helpers with miette-friendly errors.
- **`http`**: Shared HTTP client builder.
- **`logging`**: Tracing/logging initialisation for CLI binaries.

```rust
use queryfabric_cli_toolbelt::auth::{AuthStore, load_auth, save_auth};

let store = AuthStore {
    token: "my-token".into(),
    email: "user@example.com".into(),
    refresh_token: None,
};
save_auth("my-app", &store)?;
let loaded = load_auth("my-app")?;
```

## queryfabric-cmd-runner

Async subprocess runner with combined stdout/stderr capture, tail truncation,
and MCP integration:

```rust
use queryfabric_cmd_runner::run_cmd;

let result = run_cmd("cargo", &["check"]).await?;
println!("{}", result.output);  // last 200 lines
```

With the `mcp` feature, results can be formatted as MCP `CallToolResult`:

```rust
use queryfabric_cmd_runner::mcp::format_result;

let mcp_result = format_result("cargo check", result);
```

## queryfabric-test-rig

Docker/Podman integration test harness:

```rust
use queryfabric_test_rig::{
    connect_docker, ensure_image, ensure_network, start_container_with_ports,
    wait_for_port, probe::wait_for_tcp_port,
};
```

Pre-built service definitions (`PostgresService`, `ClickHouseService`,
`MinioService`) are available through the `TestRigBuilder`.

### Port probing

```rust
use queryfabric_test_rig::probe::{wait_for_tcp_port, WaitConfig};

let config = WaitConfig {
    poll_interval: Duration::from_millis(500),
    max_attempts: 30,
};
wait_for_tcp_port("localhost:5432", &config)?;
```

### Docker registry auth

```rust
use queryfabric_test_rig::docker_auth::resolve_registry_auth;

if let Some(auth) = resolve_registry_auth("docker.io", "MYAPP")? {
    match auth {
        Auth::Basic { user, password } => { /* use */ }
        Auth::Bearer(token) => { /* use */ }
    }
}
```
