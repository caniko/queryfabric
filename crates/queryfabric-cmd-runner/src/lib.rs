//! Async subprocess runner with combined stdout/stderr capture.
//!
//! Designed for build-verification tools (lint, test, build) where the
//! caller wants a compact summary plus the last N lines of output.
//!
//! Enable the `mcp` feature for MCP formatting helpers.

#![warn(missing_docs)]

use std::time::Instant;

/// Default number of output lines to retain (from the tail).
pub const DEFAULT_MAX_LINES: usize = 200;

/// Errors produced while launching or waiting for a command.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CmdError {
    /// The current working directory could not be determined.
    #[error("cannot determine working directory")]
    WorkingDirectory {
        /// Underlying filesystem error.
        #[source]
        source: std::io::Error,
    },

    /// The subprocess could not be spawned.
    #[error("failed to spawn `{program}`")]
    Spawn {
        /// Program name that was being launched.
        program: String,
        /// Underlying spawn error.
        #[source]
        source: std::io::Error,
    },

    /// Waiting for the subprocess failed.
    #[error("`{program}` wait failed")]
    Wait {
        /// Program name that was being waited on.
        program: String,
        /// Underlying wait error.
        #[source]
        source: std::io::Error,
    },
}

/// Result of running a shell command.
#[derive(Debug, Clone)]
pub struct CmdResult {
    /// Whether the subprocess exited successfully.
    pub success: bool,
    /// Wall-clock runtime in seconds.
    pub duration_secs: f64,
    /// Combined stdout/stderr, optionally truncated to the retained tail.
    pub output: String,
    /// Whether output lines were dropped from the head.
    pub truncated: bool,
}

/// Run a command and capture combined stdout+stderr.
///
/// Output is truncated to the last [`DEFAULT_MAX_LINES`] lines (errors are
/// usually at the end). Use [`run_cmd_with_limit`] to override.
///
/// # Errors
/// Returns any working-directory, spawn, or wait failure surfaced through
/// [`CmdError`].
pub async fn run_cmd(program: &str, args: &[&str]) -> Result<CmdResult, CmdError> {
    run_cmd_with_limit(program, args, DEFAULT_MAX_LINES).await
}

/// Run a command in the current directory, retaining the last `max_lines`
/// lines of combined output.
///
/// # Errors
/// Returns any working-directory, spawn, or wait failure surfaced through
/// [`CmdError`].
pub async fn run_cmd_with_limit(
    program: &str,
    args: &[&str],
    max_lines: usize,
) -> Result<CmdResult, CmdError> {
    let root = std::env::current_dir().map_err(|source| CmdError::WorkingDirectory { source })?;

    let start = Instant::now();

    let child = tokio::process::Command::new(program)
        .args(args)
        .current_dir(&root)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|source| CmdError::Spawn {
            program: program.to_owned(),
            source,
        })?;

    let output = child
        .wait_with_output()
        .await
        .map_err(|source| CmdError::Wait {
            program: program.to_owned(),
            source,
        })?;

    let duration_secs = start.elapsed().as_secs_f64();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    let combined = if stderr.is_empty() {
        stdout.into_owned()
    } else if stdout.is_empty() {
        stderr.into_owned()
    } else {
        format!("{stdout}\n{stderr}")
    };

    let lines: Vec<&str> = combined.lines().collect();
    let truncated = lines.len() > max_lines;
    let kept = if truncated {
        lines[lines.len() - max_lines..].join("\n")
    } else {
        combined
    };

    Ok(CmdResult {
        success: output.status.success(),
        duration_secs,
        output: kept,
        truncated,
    })
}
pub mod mcp;
