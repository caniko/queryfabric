//! Retrying parallel HTTP downloader with progress reporting.
//!
//! Provides [`DownloadManager`] which streams large files to disk with
//! exponential-backoff retry on transient HTTP errors (429/5xx) and
//! `buffer_unordered` parallelism over manifest entries.

#![warn(missing_docs)]

use std::path::{Path, PathBuf};
use std::time::Duration;

use futures::stream::StreamExt;
use thiserror::Error;
use tokio::io::AsyncWriteExt;

/// Errors produced while downloading files.
#[derive(Debug, Error)]
pub enum FetchError {
    /// The underlying HTTP client failed.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    /// The transfer failed for a semantic or protocol reason.
    #[error("download failed: {0}")]
    Download(String),
    /// Local filesystem I/O failed.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result type used by this crate.
pub type Result<T> = std::result::Result<T, FetchError>;

/// Describes a file to download as part of a manifest.
#[derive(Debug, Clone)]
pub struct DownloadFile {
    /// Source URL to fetch.
    pub url: String,
    /// Target filename relative to the output directory.
    pub filename: String,
    /// Human-readable progress label.
    pub description: String,
}

const DEFAULT_CONCURRENCY: usize = 4;
const MAX_RETRIES: u32 = 3;
const TRANSIENT_STATUS_CODES: &[u16] = &[429, 500, 502, 503, 504];

/// Streams downloads with a shared HTTP client, retry logic, and concurrency control.
#[derive(Debug, Clone)]
pub struct DownloadManager {
    client: reqwest::Client,
    concurrency: usize,
}

impl Default for DownloadManager {
    fn default() -> Self {
        Self::new(DEFAULT_CONCURRENCY)
    }
}

impl DownloadManager {
    /// Create a new download manager with the given concurrency limit.
    ///
    /// Builds a default `reqwest::Client` with a 1-hour timeout and a
    /// generic user agent. Use [`with_client`](Self::with_client) to supply
    /// your own.
    ///
    /// # Panics
    /// Panics if `concurrency` is 0.
    #[must_use]
    pub fn new(concurrency: usize) -> Self {
        assert!(concurrency > 0, "concurrency must be > 0");
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(3600))
            .user_agent("concurrent-fetch/0.1")
            .build()
            .expect("default reqwest::Client should build");
        Self {
            client,
            concurrency,
        }
    }

    /// Create a download manager from an existing client.
    ///
    /// # Panics
    /// Panics if `concurrency` is 0.
    #[must_use]
    pub fn with_client(client: reqwest::Client, concurrency: usize) -> Self {
        assert!(concurrency > 0, "concurrency must be > 0");
        Self {
            client,
            concurrency,
        }
    }

    /// Download a single file with exponential backoff retry for transient errors.
    ///
    /// Returns the number of bytes written. Skips download if the file
    /// already exists and `overwrite` is false. Rejects responses with a
    /// `text/html` content type (used by upstream login pages).
    ///
    /// # Errors
    /// Returns [`FetchError::Http`] for client/protocol failures,
    /// [`FetchError::Download`] for non-retryable or exhausted HTTP status
    /// failures, and [`FetchError::Io`] for local filesystem writes.
    pub async fn download_file(
        &self,
        url: &str,
        output_path: &Path,
        overwrite: bool,
    ) -> Result<u64> {
        if output_path.exists() && !overwrite {
            let size = std::fs::metadata(output_path)?.len();
            tracing::info!(
                path = %output_path.display(),
                size,
                "File already exists, skipping download"
            );
            return Ok(size);
        }

        tracing::info!(url, path = %output_path.display(), "Downloading");

        let resp = self.send_with_retry(url).await?;

        if let Some(ct) = resp.headers().get(reqwest::header::CONTENT_TYPE)
            && let Ok(ct_str) = ct.to_str()
            && ct_str.contains("text/html")
        {
            return Err(FetchError::Download(format!(
                "Server returned HTML instead of data for {url} (content-type: {ct_str}). \
                 The URL may require authentication or has changed."
            )));
        }

        let total_size = resp.content_length();
        let pb = if let Some(total) = total_size {
            let pb = indicatif::ProgressBar::new(total);
            pb.set_style(
                indicatif::ProgressStyle::default_bar()
                    .template("{spinner:.green} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                    .expect("static progress bar template is valid")
                    .progress_chars("#>-"),
            );
            pb
        } else {
            let pb = indicatif::ProgressBar::new_spinner();
            pb.set_style(
                indicatif::ProgressStyle::default_spinner()
                    .template("{spinner:.green} {bytes} downloaded")
                    .expect("static spinner template is valid"),
            );
            pb
        };

        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut file = tokio::fs::File::create(output_path).await?;
        let mut stream = resp.bytes_stream();
        let mut downloaded: u64 = 0;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            file.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;
            pb.set_position(downloaded);
        }

        file.flush().await?;
        pb.finish_with_message("done");

        tracing::info!(
            url,
            bytes = downloaded,
            path = %output_path.display(),
            "Download complete"
        );
        Ok(downloaded)
    }

    /// Download all files in a manifest concurrently using `buffer_unordered`.
    ///
    /// # Errors
    /// Returns the first download or filesystem error encountered while
    /// materializing the manifest.
    pub async fn download_manifest(
        &self,
        files: &[DownloadFile],
        output_dir: &Path,
        overwrite: bool,
    ) -> Result<Vec<PathBuf>> {
        std::fs::create_dir_all(output_dir)?;

        let results: Vec<Result<PathBuf>> = futures::stream::iter(files.iter().map(|file| {
            let output_path = output_dir.join(&file.filename);
            let description = file.description.clone();
            let filename = file.filename.clone();
            let url = file.url.clone();
            async move {
                tracing::info!(description = %description, "Downloading {}", filename);
                self.download_file(&url, &output_path, overwrite).await?;
                Ok(output_path)
            }
        }))
        .buffer_unordered(self.concurrency)
        .collect()
        .await;

        let mut paths = Vec::with_capacity(files.len());
        for result in results {
            paths.push(result?);
        }
        Ok(paths)
    }

    async fn send_with_retry(&self, url: &str) -> Result<reqwest::Response> {
        for attempt in 0..=MAX_RETRIES {
            let resp = self.client.get(url).send().await?;
            let status = resp.status();

            if status.is_success() {
                return Ok(resp);
            }

            let is_transient = TRANSIENT_STATUS_CODES.contains(&status.as_u16());

            if !is_transient || attempt == MAX_RETRIES {
                return Err(FetchError::Download(format!(
                    "HTTP {status} downloading {url} (after {attempt} retries)"
                )));
            }

            let base_delay = Duration::from_secs(1 << attempt);
            let jitter = Duration::from_millis(rand::random::<u64>() % 1000);
            let delay = base_delay + jitter;

            tracing::warn!(
                url,
                status = %status,
                attempt = attempt + 1,
                max_retries = MAX_RETRIES,
                delay_ms = delay.as_millis() as u64,
                "Transient HTTP error, retrying after backoff"
            );

            tokio::time::sleep(delay).await;
        }

        Err(FetchError::Download(format!(
            "retry loop exhausted unexpectedly for {url}"
        )))
    }
}
