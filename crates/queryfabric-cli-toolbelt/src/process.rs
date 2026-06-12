//! Subprocess helpers for CLI tools.

#![warn(missing_docs)]

use std::path::Path;
use std::process::{Command, Stdio};

use miette::{IntoDiagnostic, Result, miette};

/// Captured failure detail from a subprocess that did not succeed.
#[derive(Debug)]
pub struct CommandFailure {
    /// Human-readable stderr/stdout summary from the failed command.
    pub detail: String,
}

/// Run a command and return an error if it exits unsuccessfully.
///
/// # Errors
/// Returns an error when the command cannot be spawned or exits with a
/// non-zero status.
pub fn run(root: &Path, program: &str, args: &[String]) -> Result<()> {
    let status = Command::new(program)
        .args(args)
        .current_dir(root)
        .status()
        .into_diagnostic()?;
    if !status.success() {
        return Err(miette!(
            "{program} failed with exit code {:?}",
            status.code()
        ));
    }
    Ok(())
}

/// Run a command from borrowed argument slices.
///
/// # Errors
/// Returns an error when the command cannot be spawned or exits with a
/// non-zero status.
pub fn run_refs(root: &Path, program: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(program)
        .args(args)
        .current_dir(root)
        .status()
        .into_diagnostic()?;
    if !status.success() {
        return Err(miette!(
            "{program} failed with exit code {:?}",
            status.code()
        ));
    }
    Ok(())
}

/// Run a command and capture stdout, surfacing stderr/stdout on failure.
pub fn output(
    root: &Path,
    program: &str,
    args: &[String],
) -> std::result::Result<String, CommandFailure> {
    let output = Command::new(program)
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|err| CommandFailure {
            detail: err.to_string(),
        })?;
    if !output.status.success() {
        return Err(CommandFailure {
            detail: output_detail(&output),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Run a command and return UTF-8 stdout.
///
/// # Errors
/// Returns an error when the command cannot be spawned, exits with a
/// non-zero status, or emits non-UTF-8 stdout.
pub fn output_refs(root: &Path, program: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(program)
        .args(args)
        .current_dir(root)
        .output()
        .into_diagnostic()?;
    if !output.status.success() {
        return Err(miette!("{program} failed: {}", output_detail(&output)));
    }
    String::from_utf8(output.stdout).into_diagnostic()
}

/// Run a command while piping `stdin` into it.
///
/// # Errors
/// Returns an error when the command cannot be spawned, stdin cannot be
/// written, or the command exits unsuccessfully.
pub fn run_with_stdin(root: &Path, program: &str, args: &[&str], stdin: &str) -> Result<()> {
    use std::io::Write;

    let mut child = Command::new(program)
        .args(args)
        .current_dir(root)
        .stdin(Stdio::piped())
        .spawn()
        .into_diagnostic()?;
    child
        .stdin
        .as_mut()
        .expect("stdin is piped")
        .write_all(stdin.as_bytes())
        .into_diagnostic()?;
    let status = child.wait().into_diagnostic()?;
    if !status.success() {
        return Err(miette!("{program} failed with {status}"));
    }
    Ok(())
}

fn output_detail(output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        output.status.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_failure_reports_stderr() {
        let tmp = tempfile::tempdir().unwrap();
        let err = output(
            tmp.path(),
            "sh",
            &["-c".to_string(), "echo bad >&2; exit 7".to_string()],
        )
        .unwrap_err();
        assert!(err.detail.contains("bad"));
    }

    #[test]
    fn output_failure_falls_back_to_stdout() {
        let tmp = tempfile::tempdir().unwrap();
        let err = output(
            tmp.path(),
            "sh",
            &["-c".to_string(), "echo bad; exit 7".to_string()],
        )
        .unwrap_err();
        assert!(err.detail.contains("bad"));
    }
}
