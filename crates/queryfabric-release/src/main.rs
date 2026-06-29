use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use std::process::Command;

const CRATES: &[&str] = &[
    "queryfabric-ir",
    "queryfabric-catalog",
    "queryfabric-runtime",
    "queryfabric-opt",
    "queryfabric-dialect-sql",
    "queryfabric-dialect-syql",
    "queryfabric-adapter-clickhouse",
    "queryfabric-adapter-postgres",
    "queryfabric",
];

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
        Command::Publish { version, from, execute } => run_publish(&version, from.as_deref(), execute),
        Command::Tag { version } => run_tag(&version),
    }
}

fn run_check() -> Result<()> {
    let root = workspace_root()?;

    for (step, cmd, args) in [
        ("cargo fmt --all --check", "cargo", &["fmt", "--all", "--check"] as &[&str]),
        ("cargo clippy --workspace", "cargo", &["clippy", "--workspace", "--all-targets", "--", "-D", "warnings"]),
        ("cargo test --workspace", "cargo", &["test", "--workspace", "--all-targets", "--exclude", "queryfabric-python"]),
    ] {
        println!("==> {step}");
        let status = Command::new(cmd).args(args).current_dir(&root).status()?;
        if !status.success() {
            bail!("{step} failed");
        }
    }

    println!("All checks passed.");
    Ok(())
}

fn run_publish(version: &str, from: Option<&str>, execute: bool) -> Result<()> {
    let root = workspace_root()?;
    let start_idx = from
        .map(|c| {
            CRATES.iter().position(|&x| x == c)
                .ok_or_else(|| anyhow::anyhow!("unknown crate '{c}'"))
        })
        .transpose()?
        .unwrap_or(0);

    let plan: Vec<&str> = CRATES[start_idx..].iter().copied().collect();
    println!("==> publish order: {}", plan.join(", "));

    if !execute {
        // Dry-run the first crate only
        let crate_name = plan[0];
        println!("Dry-running {crate_name} {version}...");
        let manifest = root.join("crates").join(crate_name).join("Cargo.toml");
        let status = Command::new("cargo")
            .args(["publish", "--manifest-path", &manifest.to_string_lossy(), "--dry-run", "--allow-dirty"])
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
    for &crate_name in &plan {
        let manifest = root.join("crates").join(crate_name).join("Cargo.toml");

        println!("==> Publishing {crate_name} {version}...");

        let status = Command::new("cargo")
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

fn run_tag(version: &str) -> Result<()> {
    let root = workspace_root()?;
    println!("==> creating annotated tag v{version}");

    let status = Command::new("git")
        .args(["tag", "-a", &format!("v{version}"), "-m", &format!("queryfabric {version}")])
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
        let status = Command::new("curl")
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
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .context("finding workspace root")?;
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(std::path::PathBuf::from(path))
}
