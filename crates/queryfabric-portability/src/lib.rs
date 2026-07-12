//! Portable export bundles, DOI minting, and artifact manifests.
//!
//! The **export bundle** ([`build_bundle`]) is the "take your data and leave"
//! deliverable: one immutable, content-addressed JSON document embedding a
//! resource's metadata, full provenance history, license + data-use
//! restriction, citations in five formats, and its artifact manifests. The
//! Bundle 1.0 keeps its historical sorted-key serializer. Import-ready bundle
//! 2.0 uses RFC 8785 JSON Canonicalization and typed BLAKE3-256 digests.
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
mod import;
mod manifest;

pub use bundle::{
    BUNDLE_VERSION, BundleError, BundleHeader, BundleRequest, Citations, ExportBundle,
    ImportSealedBundle, LicenseSection, SealedBundle, build_bundle, build_import_bundle,
};
pub use canonical::canonical_json_string;
pub use canonical::canonical_json_string_v2;
pub use citation::{CitationFormat, CitationInput, generate_citation};
pub use doi::{DataCiteProvider, DoiError, DoiProvider, DoiRecord, DoiStatus, HttpTransport};
pub use import::{
    IMPORT_BUNDLE_VERSION, ImportError, ImportLimits, ImportPlan, PlanTarget, TABULAR_CSV_PROFILE,
    TabularColumn, TabularColumnType, TabularSchema, ValidatedBundle, decode_tabular_csv,
    plan_tabular_import, tabular_schema_fingerprint, validate_import_bundle, write_tabular_csv,
};
pub use manifest::{ArtifactManifest, content_hash_hex};

#[cfg(test)]
mod schema_fixtures;
#[cfg(test)]
mod tests;
