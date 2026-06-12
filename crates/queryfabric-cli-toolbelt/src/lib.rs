//! CLI ergonomics: subprocess runner, repo-root finder, tracing init, and
//! HTTP client builders.

#![warn(missing_docs)]

/// Shared HTTP client construction helpers.
pub mod http;
/// Tracing/logging initialization helpers for CLI and MCP binaries.
pub mod logging;
/// Subprocess execution helpers with miette-friendly errors.
pub mod process;
/// Repository-root discovery helpers.
pub mod repo;
