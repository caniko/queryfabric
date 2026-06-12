//! Deterministic blake3 content digest over all files in a directory.
//!
//! Combines per-file streaming blake3 hashes with their filenames into a
//! single hex digest. The `metadata_prefix` parameter excludes hash sidecar
//! files from the digest (so storing the result in the directory doesn't
//! invalidate it). For SynDB callers, the prefix is `.syndb-`.

#![warn(missing_docs)]

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use thiserror::Error;

/// Buffer size for streaming file reads (64 KiB).
const BUF_SIZE: usize = 64 * 1024;

/// Errors produced while hashing directories and sidecar files.
#[derive(Debug, Error)]
pub enum HashError {
    /// The requested file or directory did not exist.
    #[error("file not found: {path}: {source}")]
    FileNotFound {
        /// Missing path.
        path: String,
        /// Underlying OS error.
        #[source]
        source: std::io::Error,
    },
    /// A generic filesystem I/O error occurred.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// The target directory contained no hashable files.
    #[error("no files in directory")]
    EmptyDir,
}

/// Result type used by this crate.
pub type Result<T> = std::result::Result<T, HashError>;

/// Hash a single file with blake3 (streaming, constant memory).
///
/// # Errors
/// Returns [`HashError::FileNotFound`] when the file cannot be opened and
/// [`HashError::Io`] for later read failures.
pub fn hash_file(path: &Path) -> Result<blake3::Hash> {
    let mut hasher = blake3::Hasher::new();
    let mut file = fs::File::open(path).map_err(|source| HashError::FileNotFound {
        path: path.display().to_string(),
        source,
    })?;
    let mut buf = vec![0u8; BUF_SIZE];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize())
}

/// Collect all hashable files in `dir`, sorted by filename.
///
/// Skips files whose name begins with `metadata_prefix` (e.g. `.syndb-`).
///
/// # Errors
/// Returns [`HashError::FileNotFound`] when `dir` cannot be read.
pub fn collect_data_files(dir: &Path, metadata_prefix: &str) -> Result<Vec<PathBuf>> {
    let entries = fs::read_dir(dir).map_err(|source| HashError::FileNotFound {
        path: dir.display().to_string(),
        source,
    })?;

    let mut files: Vec<PathBuf> = entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if !path.is_file() {
                return None;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with(metadata_prefix) {
                return None;
            }
            Some(path)
        })
        .collect();

    files.sort();
    Ok(files)
}

/// Hash all data files in `dir` into a single combined blake3 digest.
///
/// Each file's hash and filename are mixed into a final hasher, so renames,
/// adds, removes, and content changes all produce different digests.
/// Files starting with `metadata_prefix` are skipped.
///
/// # Errors
/// Returns [`HashError::EmptyDir`] when no hashable files remain after
/// filtering, plus any file enumeration or hashing error encountered.
pub fn hash_data_dir(dir: &Path, metadata_prefix: &str) -> Result<String> {
    let files = collect_data_files(dir, metadata_prefix)?;
    if files.is_empty() {
        return Err(HashError::EmptyDir);
    }

    let mut combined = blake3::Hasher::new();
    for path in &files {
        if let Some(name) = path.file_name() {
            combined.update(name.as_encoded_bytes());
        }
        let file_hash = hash_file(path)?;
        combined.update(file_hash.as_bytes());
    }

    Ok(combined.finalize().to_hex().to_string())
}

/// Read a stored hash from `dir/<hash_filename>`.
///
/// Returns `None` if the file is missing or doesn't start with `algo_prefix`
/// (e.g. `"blake3:"`).
#[must_use]
pub fn read_stored_hash(dir: &Path, hash_filename: &str, algo_prefix: &str) -> Option<String> {
    let path = dir.join(hash_filename);
    let content = fs::read_to_string(&path).ok()?;
    content
        .trim()
        .strip_prefix(algo_prefix)
        .map(std::borrow::ToOwned::to_owned)
}

/// Write a hash to `dir/<hash_filename>` prefixed with `algo_prefix`.
///
/// # Errors
/// Returns any filesystem write error from the destination sidecar file.
pub fn write_stored_hash(
    dir: &Path,
    hash_filename: &str,
    algo_prefix: &str,
    hash: &str,
) -> Result<()> {
    let path = dir.join(hash_filename);
    fs::write(&path, format!("{algo_prefix}{hash}\n"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    const META: &str = ".test-";
    const HASH_FILE: &str = ".test-content-hash";
    const PREFIX: &str = "blake3:";

    #[test]
    fn hash_file_deterministic() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.csv");
        fs::write(&path, b"a,b,c\n1,2,3\n").unwrap();
        assert_eq!(hash_file(&path).unwrap(), hash_file(&path).unwrap());
    }

    #[test]
    fn collect_data_files_excludes_metadata() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("data.csv"), b"data").unwrap();
        fs::write(dir.path().join(".test-content-hash"), b"x").unwrap();
        let files = collect_data_files(dir.path(), META).unwrap();
        assert_eq!(files.len(), 1);
    }

    #[test]
    fn hash_data_dir_deterministic_and_changes_on_edit() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.csv"), b"x").unwrap();
        fs::write(dir.path().join("b.csv"), b"y").unwrap();
        let h1 = hash_data_dir(dir.path(), META).unwrap();
        let h2 = hash_data_dir(dir.path(), META).unwrap();
        assert_eq!(h1, h2);
        fs::write(dir.path().join("a.csv"), b"z").unwrap();
        assert_ne!(h1, hash_data_dir(dir.path(), META).unwrap());
    }

    #[test]
    fn stored_hash_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        write_stored_hash(dir.path(), HASH_FILE, PREFIX, "abc").unwrap();
        assert_eq!(
            read_stored_hash(dir.path(), HASH_FILE, PREFIX).as_deref(),
            Some("abc")
        );
    }

    #[test]
    fn read_stored_hash_missing_or_malformed() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read_stored_hash(dir.path(), HASH_FILE, PREFIX).is_none());
        fs::write(dir.path().join(HASH_FILE), b"garbage").unwrap();
        assert!(read_stored_hash(dir.path(), HASH_FILE, PREFIX).is_none());
    }

    #[test]
    fn hash_large_file_streaming() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("large.bin");
        let mut f = fs::File::create(&path).unwrap();
        let chunk = vec![0xABu8; BUF_SIZE];
        for _ in 0..4 {
            f.write_all(&chunk).unwrap();
        }
        drop(f);
        assert!(!hash_file(&path).unwrap().to_hex().is_empty());
    }
}
