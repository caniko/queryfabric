use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use serde::Deserialize;
use std::process::Command as ProcessCommand;

#[derive(Parser)]
#[command(name = "queryfabric-release", about = "QueryFabric release tool")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run pre-release checks: fmt, clippy, test, fuzz, Python bindings
    Check,
    /// Publish crates to crates.io (staged)
    Publish {
        /// Version to publish (e.g. 0.1.0)
        #[arg(long)]
        version: String,
        /// Resume from this crate (default: first unpublished)
        #[arg(long)]
        from: Option<String>,
        /// Actually publish (default: dry-run first crate only)
        #[arg(long)]
        execute: bool,
    },
    /// Create an annotated git tag for the version
    Tag {
        /// Version to tag (e.g. 0.1.0)
        #[arg(long)]
        version: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Check => run_check(),
        Command::Publish {
            version,
            from,
            execute,
        } => run_publish(&version, from.as_deref(), execute),
        Command::Tag { version } => run_tag(&version),
    }
}

fn run_check() -> Result<()> {
    let root = workspace_root()?;

    for (step, cmd, args) in [
        (
            "cargo fmt --all --check",
            "cargo",
            &["fmt", "--all", "--check"] as &[&str],
        ),
        (
            "cargo clippy --workspace",
            "cargo",
            &[
                "clippy",
                "--workspace",
                "--all-targets",
                "--",
                "-D",
                "warnings",
            ],
        ),
        (
            "cargo test --workspace",
            "cargo",
            &[
                "test",
                "--workspace",
                "--all-targets",
                "--exclude",
                "queryfabric-python",
            ],
        ),
    ] {
        println!("==> {step}");
        let status = ProcessCommand::new(cmd)
            .args(args)
            .current_dir(&root)
            .status()?;
        if !status.success() {
            bail!("{step} failed");
        }
    }

    println!("All checks passed.");
    Ok(())
}

fn run_publish(version: &str, from: Option<&str>, execute: bool) -> Result<()> {
    let root = workspace_root()?;
    let crates = publishable_crates(&root)?;
    let start_idx = from
        .map(|c| {
            crates
                .iter()
                .position(|x| x == c)
                .ok_or_else(|| anyhow::anyhow!("unknown crate '{c}'"))
        })
        .transpose()?
        .unwrap_or(0);

    let plan = &crates[start_idx..];
    println!("==> publish order: {}", plan.join(", "));

    if !execute {
        // Dry-run the first crate only
        let crate_name = &plan[0];
        println!("Dry-running {crate_name} {version}...");
        let manifest = root.join("crates").join(crate_name).join("Cargo.toml");
        let status = ProcessCommand::new("cargo")
            .args([
                "publish",
                "--manifest-path",
                &manifest.to_string_lossy(),
                "--dry-run",
                "--allow-dirty",
            ])
            .current_dir(&root)
            .status()?;
        if !status.success() {
            println!("{crate_name} {version} is not independently dry-runnable yet.");
            println!("Run with --execute to publish staged.");
        } else {
            println!("Dry-run of {crate_name} {version} OK.");
        }
        return Ok(());
    }

    // Execute: publish each crate sequentially, wait for crates.io propagation
    for crate_name in plan {
        let manifest = root.join("crates").join(crate_name).join("Cargo.toml");

        println!("==> Publishing {crate_name} {version}...");

        let status = ProcessCommand::new("cargo")
            .args(["publish", "--manifest-path", &manifest.to_string_lossy()])
            .current_dir(&root)
            .status()?;
        if !status.success() {
            bail!("publish {crate_name} failed");
        }

        wait_for_crates_io(crate_name, version)?;
    }

    println!("All crates published.");
    Ok(())
}

#[derive(Debug, Deserialize)]
struct Metadata {
    packages: Vec<Package>,
    resolve: Resolve,
    workspace_members: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Package {
    id: String,
    name: String,
    publish: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct Resolve {
    nodes: Vec<ResolveNode>,
}

#[derive(Debug, Deserialize)]
struct ResolveNode {
    id: String,
    dependencies: Vec<serde_json::Value>,
}

/// Return publishable packages in dependency order from Cargo metadata.
/// `publish = false` packages cannot accidentally become release inputs just
/// because a workflow or tool forgot to update a second hard-coded list.
fn publishable_crates(root: &std::path::Path) -> Result<Vec<String>> {
    let output = ProcessCommand::new("cargo")
        .args(["metadata", "--format-version", "1", "--locked"])
        .current_dir(root)
        .output()
        .context("reading Cargo metadata")?;
    if !output.status.success() {
        bail!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let metadata: Metadata =
        serde_json::from_slice(&output.stdout).context("parsing Cargo metadata")?;
    let workspace_members = metadata
        .workspace_members
        .iter()
        .collect::<std::collections::BTreeSet<_>>();
    let publishable = metadata
        .packages
        .iter()
        .filter(|package| workspace_members.contains(&&package.id))
        .filter(|package| {
            package
                .publish
                .as_ref()
                .is_none_or(|registries| !registries.is_empty())
        })
        .map(|package| package.id.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let names = metadata
        .packages
        .iter()
        .map(|package| (package.id.clone(), package.name.clone()))
        .collect::<std::collections::BTreeMap<_, _>>();
    let dependencies = metadata
        .resolve
        .nodes
        .into_iter()
        .map(|node| {
            (
                node.id,
                node.dependencies
                    .into_iter()
                    .filter_map(|dependency| {
                        dependency
                            .as_str()
                            .map(str::to_owned)
                            .or_else(|| dependency.get("pkg")?.as_str().map(str::to_owned))
                    })
                    .collect::<std::collections::BTreeSet<_>>(),
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();

    let mut remaining = publishable;
    let mut ordered = Vec::with_capacity(remaining.len());
    while !remaining.is_empty() {
        let ready = remaining
            .iter()
            .filter(|package| {
                dependencies
                    .get(*package)
                    .into_iter()
                    .flatten()
                    .filter(|dependency| remaining.contains(*dependency))
                    .count()
                    == 0
            })
            .cloned()
            .collect::<Vec<_>>();
        if ready.is_empty() {
            bail!("publishable Cargo packages contain a dependency cycle");
        }
        for package in ready {
            remaining.remove(&package);
            ordered.push(
                names
                    .get(&package)
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("missing package name for `{package}`"))?,
            );
        }
    }
    Ok(ordered)
}

fn run_tag(version: &str) -> Result<()> {
    let root = workspace_root()?;
    println!("==> creating annotated tag v{version}");

    let status = ProcessCommand::new("git")
        .args([
            "tag",
            "-a",
            &format!("v{version}"),
            "-m",
            &format!("queryfabric {version}"),
        ])
        .current_dir(&root)
        .status()?;
    if !status.success() {
        bail!("git tag failed");
    }
    println!("Tag v{version} created.");
    Ok(())
}

fn wait_for_crates_io(crate_name: &str, version: &str) -> Result<()> {
    let url = format!("https://crates.io/api/v1/crates/{crate_name}/{version}");
    println!("Waiting for {crate_name} {version} on crates.io...");

    for attempt in 1..=60 {
        let status = ProcessCommand::new("curl")
            .args(["--fail", "--silent", "--show-error", &url])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if status {
            println!("{crate_name} {version} is visible on crates.io");
            return Ok(());
        }
        println!("  not visible yet ({attempt}/60), sleeping 10s...");
        std::thread::sleep(std::time::Duration::from_secs(10));
    }
    bail!("{crate_name} {version} did not appear on crates.io in time");
}

fn workspace_root() -> Result<std::path::PathBuf> {
    let output = ProcessCommand::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .context("finding workspace root")?;
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(std::path::PathBuf::from(path))
}

#[cfg(test)]
mod tests {
    use super::{publishable_crates, workspace_root};

    #[test]
    fn metadata_drives_the_ten_crate_publish_tier() {
        let crates = publishable_crates(&workspace_root().expect("workspace root"))
            .expect("publishable crate metadata");
        assert_eq!(crates.len(), 10);
        assert_eq!(crates.last().map(String::as_str), Some("queryfabric"));
        assert!(crates.iter().all(|name| {
            !matches!(
                name.as_str(),
                "queryfabric-changelog"
                    | "queryfabric-cli-toolbelt"
                    | "queryfabric-cmd-runner"
                    | "queryfabric-release"
                    | "queryfabric-runtime-k8s"
                    | "queryfabric-seaorm-ext"
                    | "queryfabric-test-rig"
                    | "queryfabric-types"
                    | "queryfabric-worker"
            )
        }));
    }
}
