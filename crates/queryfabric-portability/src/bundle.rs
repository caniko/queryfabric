use queryfabric_access::{DataUseRestriction, ResourcePolicy};
use queryfabric_contract::ResourceRef;
use queryfabric_provenance::{HistoryFilter, ProvenanceError, ProvenanceHistory, ProvenanceStore};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::canonical::canonical_json_string;
use crate::citation::{CitationFormat, CitationInput, csl_json_value, generate_citation};
use crate::manifest::{ArtifactManifest, content_hash_hex};

/// Export bundle schema version.
pub const BUNDLE_VERSION: &str = "1.0";

/// Errors raised while assembling a bundle.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum BundleError {
    /// The provenance history could not be read.
    #[error("bundle provenance lookup failed: {0}")]
    Provenance(#[from] ProvenanceError),
    /// The bundle could not be serialized.
    #[error("bundle serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// Bundle identity header.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleHeader {
    /// Bundle schema version ([`BUNDLE_VERSION`]).
    pub version: String,
    /// Resource the bundle describes.
    pub resource: ResourceRef,
    /// When the bundle was produced (Unix milliseconds, caller-supplied).
    pub exported_at_unix_ms: i64,
}

/// All five citation renderings plus the citation request message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Citations {
    pub bibtex: String,
    pub ris: String,
    pub csl_json: Value,
    pub cff: String,
    pub apa: String,
    pub message: String,
}

/// License section of the bundle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LicenseSection {
    /// SPDX license identifier.
    pub spdx_id: String,
    /// Canonical URL for the license text.
    pub rights_uri: String,
}

/// The portable export bundle: one immutable JSON document with everything
/// needed to reuse a resource away from the platform that produced it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportBundle {
    /// Identity header.
    pub export_bundle: BundleHeader,
    /// Host-supplied JSON-LD metadata, carried opaquely.
    pub metadata_jsonld: Value,
    /// Citations in five formats.
    pub citations: Citations,
    /// Full ordered provenance history.
    pub provenance: ProvenanceHistory,
    /// Declared license, when any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<LicenseSection>,
    /// Declared data-use restriction, when any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_use_restriction: Option<DataUseRestriction>,
    /// Integrity manifests for the resource's stored artifacts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<ArtifactManifest>,
}

/// Everything [`build_bundle`] needs besides the provenance store.
#[derive(Debug, Clone)]
pub struct BundleRequest {
    /// Resource to bundle.
    pub resource: ResourceRef,
    /// Bundle production time (Unix milliseconds, caller-supplied so builds
    /// are deterministic).
    pub exported_at_unix_ms: i64,
    /// Host metadata as JSON-LD (or any JSON), carried opaquely.
    pub metadata_jsonld: Value,
    /// Citation facts.
    pub citation: CitationInput,
    /// Access posture: license + restriction end up in the bundle.
    pub policy: ResourcePolicy,
    /// Artifact manifests to embed.
    pub artifacts: Vec<ArtifactManifest>,
}

/// A built bundle together with its canonical bytes and content address.
#[derive(Debug, Clone, PartialEq)]
pub struct SealedBundle {
    /// The structured bundle.
    pub bundle: ExportBundle,
    /// Canonical JSON serialization (sorted keys); hash these bytes.
    pub canonical_json: String,
    /// Hex BLAKE3 digest of `canonical_json` — the bundle's content address.
    pub content_hash: String,
}

impl SealedBundle {
    /// Size of the canonical serialization in bytes.
    #[must_use]
    pub fn byte_count(&self) -> u64 {
        self.canonical_json.len() as u64
    }
}

/// Assemble a content-addressed export bundle for a generic resource.
///
/// Reads the resource's full provenance history from `store`, renders all
/// five citation formats, folds in license, restriction, and artifact
/// manifests, and seals the result with a canonical serialization and its
/// BLAKE3 content hash. Two builds from identical inputs produce identical
/// bytes and hashes.
pub async fn build_bundle(
    request: BundleRequest,
    store: &dyn ProvenanceStore,
) -> Result<SealedBundle, BundleError> {
    let provenance = store
        .history(request.resource, &HistoryFilter::default())
        .await?;

    let citations = Citations {
        bibtex: generate_citation(&request.citation, CitationFormat::BibTeX),
        ris: generate_citation(&request.citation, CitationFormat::Ris),
        csl_json: csl_json_value(&request.citation),
        cff: generate_citation(&request.citation, CitationFormat::Cff),
        apa: generate_citation(&request.citation, CitationFormat::Apa),
        message: "Please cite this resource in any publications that use it.".to_owned(),
    };

    let license = request.policy.license.map(|license| LicenseSection {
        spdx_id: license.spdx_id().to_owned(),
        rights_uri: license.rights_uri().to_owned(),
    });

    let bundle = ExportBundle {
        export_bundle: BundleHeader {
            version: BUNDLE_VERSION.to_owned(),
            resource: request.resource,
            exported_at_unix_ms: request.exported_at_unix_ms,
        },
        metadata_jsonld: request.metadata_jsonld,
        citations,
        provenance,
        license,
        data_use_restriction: request.policy.restriction,
        artifacts: request.artifacts,
    };

    let canonical_json = canonical_json_string(&serde_json::to_value(&bundle)?);
    let content_hash = content_hash_hex(canonical_json.as_bytes());
    Ok(SealedBundle {
        bundle,
        canonical_json,
        content_hash,
    })
}
