//! Portable export bundles, DOI minting, and artifact manifests.
//!
//! The **export bundle** ([`build_bundle`]) is the "take your data and leave"
//! deliverable: one immutable, content-addressed JSON document embedding a
//! resource's metadata, full provenance history, license + data-use
//! restriction, citations in five formats, and its artifact manifests. The
//! bundle is canonically serialized (sorted keys, stable formatting) so the
//! same content always hashes to the same BLAKE3 digest.
//!
//! **DOI minting** is provider-agnostic: implement [`DoiProvider`] for any
//! registrar. A DataCite REST implementation ([`DataCiteProvider`]) ships
//! here, with its HTTP layer abstracted behind [`HttpTransport`] so hosts
//! inject their own client and tests stay offline.
//!
//! **Artifact manifests** ([`ArtifactManifest`]) carry the FAIR-integrity
//! facts for each stored artifact: schema fingerprint, content hash, row and
//! byte counts, storage URI, and format.
//!
//! Storage is out of scope: the bundle is bytes; hosts persist them via
//! `queryfabric-store` or any sink they like.

mod bundle;
mod canonical;
mod citation;
pub mod datacite;
mod doi;
mod manifest;

pub use bundle::{
    BUNDLE_VERSION, BundleError, BundleHeader, BundleRequest, Citations, ExportBundle,
    LicenseSection, SealedBundle, build_bundle,
};
pub use canonical::canonical_json_string;
pub use citation::{CitationFormat, CitationInput, generate_citation};
pub use doi::{DataCiteProvider, DoiError, DoiProvider, DoiRecord, DoiStatus, HttpTransport};
pub use manifest::{ArtifactManifest, content_hash_hex};

#[cfg(test)]
mod tests;
