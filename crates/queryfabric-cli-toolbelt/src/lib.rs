//! CLI ergonomics: subprocess runner, repo-root finder, tracing init,
//! HTTP client builders, and auth token management.

#![warn(missing_docs)]

/// Auth token storage and retrieval.
pub mod auth;
/// ClickHouse connection arguments.
pub mod clickhouse;
/// Arrow Flight client (requires `flight` feature).
#[cfg(feature = "flight")]
pub mod flight;
/// Shared HTTP client construction helpers.
pub mod http;
/// Kubernetes resource types and kubectl helpers.
pub mod k8s;
/// Tracing/logging initialization helpers for CLI and MCP binaries.
pub mod logging;
/// Subprocess execution helpers with miette-friendly errors.
pub mod process;
/// Repository-root discovery helpers.
pub mod repo;
