//! DataCite Metadata Schema 4.5 types for DOI minting and metadata export.
//!
//! Pure data types modelling the subset of DataCite properties most relevant
//! to research datasets. Generalised from a host-specific implementation: no
//! domain vocabulary, no HTTP client (see [`crate::DoiProvider`]).

use std::fmt;
use std::str::FromStr;

use queryfabric_access::DataLicense;
use serde::{Deserialize, Serialize};

/// Top-level DataCite metadata record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataCiteMetadata {
    /// DOI identifier (e.g. "10.5281/zenodo.123456").
    pub identifier: DataCiteIdentifier,
    /// Resource creators / authors.
    pub creators: Vec<DataCiteCreator>,
    /// Titles (at least one required).
    pub titles: Vec<DataCiteTitle>,
    /// Publisher name.
    pub publisher: String,
    /// Publication year.
    pub publication_year: i32,
    /// Resource type.
    pub resource_type: DataCiteResourceType,
    /// Subject keywords with optional controlled-vocabulary URIs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subjects: Vec<DataCiteSubject>,
    /// Rights / license information.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rights_list: Vec<DataCiteRights>,
    /// Related identifiers (e.g. publications, parent resources).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related_identifiers: Vec<DataCiteRelatedIdentifier>,
    /// Descriptions (abstract, methods, etc.).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub descriptions: Vec<DataCiteDescription>,
    /// Important dates.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dates: Vec<DataCiteDate>,
    /// Schema version (always "4.5").
    #[serde(default = "schema_version")]
    pub schema_version: String,
}

fn schema_version() -> String {
    "4.5".to_owned()
}

/// Persistent identifier for the resource.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataCiteIdentifier {
    /// Identifier value, such as a DOI string.
    pub identifier: String,
    /// Declared DataCite identifier type.
    pub identifier_type: IdentifierType,
}

/// Supported identifier types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IdentifierType {
    /// Digital Object Identifier.
    DOI,
    /// URL identifier.
    URL,
    /// Uniform Resource Name.
    URN,
}

/// A creator (author) of the resource.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataCiteCreator {
    /// Full creator name as it should appear in the record.
    pub name: String,
    /// Given name component when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub given_name: Option<String>,
    /// Family name component when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub family_name: Option<String>,
    /// ORCID or other name identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_identifier: Option<NameIdentifier>,
    /// Free-text affiliations for the creator.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub affiliation: Vec<String>,
}

/// Name identifier (e.g. ORCID).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NameIdentifier {
    /// Identifier value, such as an ORCID.
    pub name_identifier: String,
    /// Identifier scheme name.
    pub name_identifier_scheme: String,
    /// Optional scheme URI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheme_uri: Option<String>,
}

/// A title for the resource.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataCiteTitle {
    /// Title text.
    pub title: String,
    /// Optional title subtype.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title_type: Option<TitleType>,
}

/// Title type variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TitleType {
    /// Alternative title.
    AlternativeTitle,
    /// Subtitle.
    Subtitle,
    /// Translated title.
    TranslatedTitle,
    /// Other title subtype.
    Other,
}

/// Subject keyword with optional controlled-vocabulary classification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataCiteSubject {
    /// Subject term or keyword.
    pub subject: String,
    /// Subject scheme name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject_scheme: Option<String>,
    /// Subject scheme URI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheme_uri: Option<String>,
    /// URI identifying the subject term itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_uri: Option<String>,
}

/// Rights / license information.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataCiteRights {
    /// Human-readable rights statement.
    pub rights: String,
    /// Canonical rights URI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rights_uri: Option<String>,
    /// Machine-readable rights identifier, such as an SPDX id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rights_identifier: Option<String>,
    /// Scheme for `rights_identifier`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rights_identifier_scheme: Option<String>,
}

impl DataCiteRights {
    /// Build a rights entry from a [`DataLicense`].
    #[must_use]
    pub fn from_license(license: DataLicense) -> Self {
        Self {
            rights: license.display_name().to_owned(),
            rights_uri: Some(license.rights_uri().to_owned()),
            rights_identifier: Some(license.spdx_id().to_owned()),
            rights_identifier_scheme: Some("SPDX".to_owned()),
        }
    }
}

/// Related identifier (e.g. publication DOI).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataCiteRelatedIdentifier {
    /// Related identifier value.
    pub related_identifier: String,
    /// Type of the related identifier.
    pub related_identifier_type: IdentifierType,
    /// Relationship from this resource to the related identifier.
    pub relation_type: RelationType,
}

/// Relationship types between resources.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelationType {
    /// This resource is cited by the related resource.
    IsCitedBy,
    /// This resource cites the related resource.
    Cites,
    /// This resource supplements the related resource.
    IsSupplementTo,
    /// This resource is supplemented by the related resource.
    IsSupplementedBy,
    /// This resource is part of the related resource.
    IsPartOf,
    /// This resource has the related resource as a part.
    HasPart,
    /// This resource is referenced by the related resource.
    IsReferencedBy,
    /// This resource references the related resource.
    References,
    /// This resource is derived from the related resource.
    IsDerivedFrom,
    /// This resource is the source of the related resource.
    IsSourceOf,
    /// This resource is described by the related resource.
    IsDescribedBy,
    /// This resource describes the related resource.
    Describes,
    /// This resource is a version of the related resource.
    IsVersionOf,
    /// This resource has the related resource as a version.
    HasVersion,
}

/// Error returned when parsing a DataCite relation type fails.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum RelationTypeParseError {
    /// The caller supplied an unknown DataCite relation type string.
    #[error("invalid DataCite relation type '{value}'. Allowed: {allowed}")]
    Invalid {
        /// Invalid relation-type string provided by the caller.
        value: String,
        /// Human-readable list of allowed relation-type values.
        allowed: &'static str,
    },
}

impl RelationType {
    const ALLOWED_DISPLAY: &'static str = "IsCitedBy, Cites, IsSupplementTo, IsSupplementedBy, IsPartOf, HasPart, IsReferencedBy, References, IsDerivedFrom, IsSourceOf, IsDescribedBy, Describes, IsVersionOf, HasVersion";

    /// All supported relation-type variants.
    pub const ALL: &'static [Self] = &[
        Self::IsCitedBy,
        Self::Cites,
        Self::IsSupplementTo,
        Self::IsSupplementedBy,
        Self::IsPartOf,
        Self::HasPart,
        Self::IsReferencedBy,
        Self::References,
        Self::IsDerivedFrom,
        Self::IsSourceOf,
        Self::IsDescribedBy,
        Self::Describes,
        Self::IsVersionOf,
        Self::HasVersion,
    ];

    /// Return the canonical DataCite spelling for this relation type.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::IsCitedBy => "IsCitedBy",
            Self::Cites => "Cites",
            Self::IsSupplementTo => "IsSupplementTo",
            Self::IsSupplementedBy => "IsSupplementedBy",
            Self::IsPartOf => "IsPartOf",
            Self::HasPart => "HasPart",
            Self::IsReferencedBy => "IsReferencedBy",
            Self::References => "References",
            Self::IsDerivedFrom => "IsDerivedFrom",
            Self::IsSourceOf => "IsSourceOf",
            Self::IsDescribedBy => "IsDescribedBy",
            Self::Describes => "Describes",
            Self::IsVersionOf => "IsVersionOf",
            Self::HasVersion => "HasVersion",
        }
    }

    /// Return every allowed relation-type string.
    #[must_use]
    pub fn allowed_values() -> Vec<&'static str> {
        Self::ALL.iter().map(|relation| relation.as_str()).collect()
    }
}

impl fmt::Display for RelationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for RelationType {
    type Err = RelationTypeParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .iter()
            .find(|relation| relation.as_str() == value)
            .copied()
            .ok_or_else(|| RelationTypeParseError::Invalid {
                value: value.to_owned(),
                allowed: Self::ALLOWED_DISPLAY,
            })
    }
}

/// Resource type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataCiteResourceType {
    /// Broad DataCite resource category.
    pub resource_type_general: ResourceTypeGeneral,
    /// Optional free-text subtype.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_type: Option<String>,
}

/// General resource type categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceTypeGeneral {
    /// Audiovisual resource.
    Audiovisual,
    /// Collection resource.
    Collection,
    /// Data paper resource.
    DataPaper,
    /// Dataset resource.
    Dataset,
    /// Event resource.
    Event,
    /// Image resource.
    Image,
    /// Model resource.
    Model,
    /// Physical object resource.
    PhysicalObject,
    /// Service resource.
    Service,
    /// Software resource.
    Software,
    /// Sound resource.
    Sound,
    /// Text resource.
    Text,
    /// Workflow resource.
    Workflow,
    /// Other resource type.
    Other,
}

/// Textual description.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataCiteDescription {
    /// Description text.
    pub description: String,
    /// Description subtype.
    pub description_type: DescriptionType,
}

/// Description type categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DescriptionType {
    /// Abstract or summary.
    Abstract,
    /// Methods description.
    Methods,
    /// Series information.
    SeriesInformation,
    /// Table of contents.
    TableOfContents,
    /// Technical information.
    TechnicalInfo,
    /// Other description subtype.
    Other,
}

/// Date with type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataCiteDate {
    /// ISO-8601 date string.
    pub date: String,
    /// Semantic meaning of the date.
    pub date_type: DateType,
}

/// Date type categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DateType {
    /// Date the resource was accepted.
    Accepted,
    /// Date the resource became available.
    Available,
    /// Date the data was collected.
    Collected,
    /// Copyright date.
    Copyrighted,
    /// Creation date.
    Created,
    /// Issue/publication date.
    Issued,
    /// Submission date.
    Submitted,
    /// Update date.
    Updated,
    /// Validity date.
    Valid,
    /// Withdrawal date.
    Withdrawn,
    /// Other date subtype.
    Other,
}
