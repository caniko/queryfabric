//! Standardized logging initialization for SynDB CLI tools.

#![warn(missing_docs)]

use std::path::Path;
use std::sync::Mutex;

use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer, fmt};

/// Initialize CLI logging with an environment filter.
///
/// Respects `RUST_LOG` env var; falls back to `default_level` (e.g. `"warn"`).
pub fn init_cli(default_level: &str) {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_level)),
        )
        .init();
}

/// Initialize CLI logging with a verbose toggle.
///
/// When `verbose` is true, defaults to `"debug"`; otherwise `"info"`.
/// Always respects `RUST_LOG` if set.
pub fn init_cli_verbose(verbose: bool) {
    let default = if verbose { "debug" } else { "info" };
    init_cli(default);
}

/// Initialize CLI logging with an additional file log filtered to WARN+.
///
/// The stderr layer behaves identically to [`init_cli`]. The file layer
/// writes WARN and ERROR records (no ANSI) to `log_path`, truncating it
/// on each run.
///
/// # Errors
/// Returns any directory-creation or file-creation error while preparing the
/// log sink.
pub fn init_cli_with_file_log(default_level: &str, log_path: &Path) -> std::io::Result<()> {
    if let Some(parent) = log_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::File::create(log_path)?;

    let stderr_layer = fmt::layer().with_filter(
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_level)),
    );

    let file_layer = fmt::layer()
        .with_writer(Mutex::new(file))
        .with_ansi(false)
        .with_filter(EnvFilter::new("warn"));

    tracing_subscriber::registry()
        .with(stderr_layer)
        .with(file_layer)
        .init();

    Ok(())
}

/// Initialize logging for MCP servers.
///
/// Writes to stderr (stdout is the MCP stdio transport), disables ANSI
/// escape codes, and defaults to INFO level.
pub fn init_mcp() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();
}
