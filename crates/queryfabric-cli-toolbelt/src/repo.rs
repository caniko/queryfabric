//! Walk upward from a starting directory to find a repository root.

#![warn(missing_docs)]

use std::path::{Path, PathBuf};

use miette::{IntoDiagnostic, Result, miette};

/// Find the repository root by walking upward until every marker exists in the same directory.
///
/// # Errors
/// Returns an error when the current working directory cannot be read or when
/// no ancestor directory contains all required markers.
pub fn find_repo_root(required_markers: &[&str]) -> Result<PathBuf> {
    let cwd = std::env::current_dir().into_diagnostic()?;
    find_repo_root_from(&cwd, required_markers)
}

/// Find the repository root from a specific start directory.
///
/// # Errors
/// Returns an error when no ancestor of `start` contains all required markers.
pub fn find_repo_root_from(start: &Path, required_markers: &[&str]) -> Result<PathBuf> {
    let mut dir = start.to_path_buf();
    loop {
        if required_markers
            .iter()
            .all(|marker| dir.join(marker).exists())
        {
            return Ok(dir);
        }
        if !dir.pop() {
            return Err(miette!(
                "Could not find repository root from {} using markers: {}",
                start.display(),
                required_markers.join(", ")
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_repo_root_from_nested_directory() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("Cargo.toml"), "[workspace]\n").unwrap();
        std::fs::create_dir_all(tmp.path().join("a/b")).unwrap();

        let root = find_repo_root_from(&tmp.path().join("a/b"), &["Cargo.toml"]).unwrap();
        assert_eq!(root, tmp.path());
    }

    #[test]
    fn missing_markers_return_useful_error() {
        let tmp = tempfile::tempdir().unwrap();
        let err = find_repo_root_from(tmp.path(), &["missing.marker"]).unwrap_err();
        assert!(err.to_string().contains("missing.marker"));
    }
}
