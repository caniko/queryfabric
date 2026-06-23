/// A parsed version bump from a git diff.
#[derive(Debug, Clone)]
pub struct VersionBump {
    pub name: String,
    pub from: String,
    pub to: String,
}

/// Parse Cargo.toml dependency version bumps from a `git diff` string.
#[rustfmt::skip]
pub fn cargo_version_bumps(diff: &str) -> Vec<VersionBump> {
    let mut bumps = Vec::new();
    let pattern = r#"(?-m)^-\s*(?:\w+\.)?(?<name>\w[\w-]*)\s*=\s*"(?<from>[^"]+)"\n\+\s*(?:\w+\.)?\k<name>\s*=\s*"(?<to>[^"]+)""#;
    let re = regex::Regex::new(pattern).expect("cargo bump regex");
    for cap in re.captures_iter(diff) {
        bumps.push(VersionBump {
            name: cap["name"].to_owned(),
            from: cap["from"].to_owned(),
            to: cap["to"].to_owned(),
        });
    }
    bumps
}

/// Parse uv.lock version bumps from a git diff.
#[rustfmt::skip]
pub fn uv_version_bumps(diff: &str) -> Vec<VersionBump> {
    let mut bumps = Vec::new();
    let pattern = r#"(?m)^-version\s*=\s*"(?<from>[^"]+)"\n\+version\s*=\s*"(?<to>[^"]+)""#;
    let re = regex::Regex::new(pattern).expect("uv bump regex");
    for cap in re.captures_iter(diff) {
        bumps.push(VersionBump {
            name: "package".to_owned(),
            from: cap["from"].to_owned(),
            to: cap["to"].to_owned(),
        });
    }
    bumps
}

/// A parsed image tag diff from versions.nix.
#[derive(Debug, Clone)]
pub struct ImageTagDiff {
    pub image: String,
    pub from: String,
    pub to: String,
}

/// Parse image tag changes from a versions.nix diff.
#[rustfmt::skip]
pub fn image_tag_diffs(diff: &str) -> Vec<ImageTagDiff> {
    let mut diffs = Vec::new();
    let pattern = r#"(?m)^-\s*(?:\w+\.)?(?<name>\w+)\s*=\s*"(?<from>[^"]+)"\n\+\s*(?:\w+\.)?\k<name>\s*=\s*"(?<to>[^"]+)""#;
    let re = regex::Regex::new(pattern).expect("image tag regex");
    for cap in re.captures_iter(diff) {
        diffs.push(ImageTagDiff {
            image: cap["name"].to_owned(),
            from: cap["from"].to_owned(),
            to: cap["to"].to_owned(),
        });
    }
    diffs
}
