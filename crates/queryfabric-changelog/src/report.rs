use std::path::Path;

use crate::diff::{ImageTagDiff, VersionBump};

/// Configuration for changelog compilation.
///
/// Implement this trait to provide environment-specific paths and mappings.
pub trait ChangelogConfig {
    /// Path to a Nix-style version file (e.g. `versions.nix`), if any.
    fn versions_nix_path(&self) -> Option<&Path>;

    /// Map an image tag key to its GitHub repository URL.
    fn image_repo_url(&self, image_key: &str) -> Option<String>;
}

/// Compile a changelog report from git diff between two refs.
pub async fn compile_report(
    config: &dyn ChangelogConfig,
    base_ref: &str,
    head_ref: &str,
) -> Result<String, String> {
    let diff = git_diff(base_ref, head_ref)?;
    let mut sections = Vec::new();

    let cargo_bumps = crate::diff::cargo_version_bumps(&diff);
    if !cargo_bumps.is_empty() {
        sections.push(format_bumps("Cargo", &cargo_bumps));
    }

    let uv_bumps = crate::diff::uv_version_bumps(&diff);
    if !uv_bumps.is_empty() {
        sections.push(format_bumps("uv/Python", &uv_bumps));
    }

    if let Some(versions_path) = config.versions_nix_path() {
        let versions_diff = git_diff_file(base_ref, head_ref, versions_path)?;
        let image_diffs = crate::diff::image_tag_diffs(&versions_diff);
        if !image_diffs.is_empty() {
            sections.push(format_images(&image_diffs, config));
        }
    }

    if sections.is_empty() {
        return Ok("No dependency changes detected.".to_owned());
    }

    Ok(sections.join("\n\n"))
}

fn format_bumps(ecosystem: &str, bumps: &[VersionBump]) -> String {
    let mut lines = vec![format!("## {ecosystem}"), String::new()];
    for bump in bumps {
        lines.push(format!("- `{}`: {} → {}", bump.name, bump.from, bump.to));
    }
    lines.join("\n")
}

fn format_images(diffs: &[ImageTagDiff], config: &dyn ChangelogConfig) -> String {
    let mut lines = vec!["## Infrastructure".to_owned(), String::new()];
    for diff in diffs {
        let repo = config.image_repo_url(&diff.image).unwrap_or_default();
        let repo_note = if repo.is_empty() {
            String::new()
        } else {
            format!(" ({repo})")
        };
        lines.push(format!(
            "- `{}`: {} → {}{}",
            diff.image, diff.from, diff.to, repo_note
        ));
    }
    lines.join("\n")
}

fn git_diff(base_ref: &str, head_ref: &str) -> Result<String, String> {
    let output = std::process::Command::new("git")
        .args(["diff", base_ref, head_ref])
        .output()
        .map_err(|e| format!("git diff failed: {e}"))?;
    String::from_utf8(output.stdout).map_err(|e| format!("git diff output: {e}"))
}

fn git_diff_file(base_ref: &str, head_ref: &str, path: &Path) -> Result<String, String> {
    let output = std::process::Command::new("git")
        .args([
            "diff",
            base_ref,
            head_ref,
            "--",
            path.to_str().unwrap_or_default(),
        ])
        .output()
        .map_err(|e| format!("git diff file failed: {e}"))?;
    String::from_utf8(output.stdout).map_err(|e| format!("git diff file output: {e}"))
}
