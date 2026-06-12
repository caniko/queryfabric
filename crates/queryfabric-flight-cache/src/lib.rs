//! Parquet cache layer for Arrow Flight queries.
//!
//! Provides deterministic [`cache_path`] derivation (md5-prefixed by a query
//! key) and a tiny streaming parquet writer for `RecordBatch` slices.

#![warn(missing_docs)]

use std::path::{Path, PathBuf};

use arrow::record_batch::RecordBatch;
use md5::{Digest, Md5};
use thiserror::Error;

/// Errors raised while building cache paths or writing parquet data.
#[derive(Debug, Error)]
pub enum CacheError {
    /// Filesystem I/O failed for the given path.
    #[error("I/O error at {path}: {source}")]
    Io {
        /// Path involved in the failed operation.
        path: String,
        /// Underlying filesystem error.
        #[source]
        source: std::io::Error,
    },
    /// Arrow parquet serialization failed.
    #[error("parquet error: {0}")]
    Parquet(#[from] parquet::errors::ParquetError),
    /// The caller provided no record batches to write.
    #[error("no batches to write")]
    Empty,
}

/// Result type used by this crate.
pub type Result<T> = std::result::Result<T, CacheError>;

/// Build a cache path of the form `{cache_dir}/{query_key}_<md5_first12>.parquet`.
///
/// The 12-hex-character md5 prefix ensures uniqueness across queries that
/// happen to share a base name.
pub fn cache_path(cache_dir: &Path, query_key: &str) -> PathBuf {
    let mut hasher = Md5::new();
    hasher.update(query_key.as_bytes());
    let digest = hasher.finalize();
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    let safe = &hex[..12];
    cache_dir.join(format!("{query_key}_{safe}.parquet"))
}

/// Write `batches` to a parquet file, creating parent directories as needed.
///
/// Uses the schema of the first batch; returns [`CacheError::Empty`] if no
/// batches are provided.
///
/// # Errors
/// Returns [`CacheError::Empty`] when `batches` is empty, [`CacheError::Io`]
/// when directory creation or file creation fails, and
/// [`CacheError::Parquet`] when Arrow parquet writing fails.
pub fn write_parquet(batches: &[RecordBatch], path: &Path) -> Result<()> {
    if batches.is_empty() {
        return Err(CacheError::Empty);
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| CacheError::Io {
            path: parent.display().to_string(),
            source: e,
        })?;
    }
    let file = std::fs::File::create(path).map_err(|e| CacheError::Io {
        path: path.display().to_string(),
        source: e,
    })?;
    let schema = batches[0].schema();
    let mut writer = parquet::arrow::ArrowWriter::try_new(file, schema, None)?;
    for batch in batches {
        writer.write(batch)?;
    }
    writer.close()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_path_is_deterministic() {
        let p1 = cache_path(Path::new("/tmp/cache"), "morphometry_celegans");
        let p2 = cache_path(Path::new("/tmp/cache"), "morphometry_celegans");
        assert_eq!(p1, p2);
    }

    #[test]
    fn cache_path_differs_per_query() {
        let p1 = cache_path(Path::new("/tmp"), "a");
        let p2 = cache_path(Path::new("/tmp"), "b");
        assert_ne!(p1, p2);
    }

    #[test]
    fn cache_path_format() {
        let p = cache_path(Path::new("/tmp"), "foo");
        assert!(p.to_string_lossy().ends_with(".parquet"));
        let stem = p.file_stem().unwrap().to_str().unwrap();
        assert!(stem.starts_with("foo_"));
        // 12 hex chars after the underscore.
        assert_eq!(stem.len(), "foo_".len() + 12);
    }
}
