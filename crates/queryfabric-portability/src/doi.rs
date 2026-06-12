use async_trait::async_trait;
use queryfabric_contract::ResourceRef;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::datacite::DataCiteMetadata;

/// Lifecycle of a DOI minting attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum DoiStatus {
    /// Minting has been requested but not confirmed.
    Pending,
    /// The registrar registered the DOI.
    Registered,
    /// The registrar rejected the request; see `last_error`.
    Failed,
}

/// Outcome of a DOI minting workflow for one resource.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DoiRecord {
    /// Resource the DOI identifies.
    pub resource: ResourceRef,
    /// The minted DOI, once registered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doi: Option<String>,
    /// Registrar name (e.g. `"datacite"`, `"zenodo"`).
    pub provider: String,
    /// Where the workflow stands.
    pub status: DoiStatus,
    /// Raw registrar response, kept for diagnosis.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response: Option<Value>,
    /// Last registrar error, when `status` is `Failed`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

/// Errors below the registrar protocol: transport failures and malformed
/// replies. Registrar-level rejections are *not* errors — they come back as
/// a [`DoiRecord`] with [`DoiStatus::Failed`].
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum DoiError {
    /// The HTTP layer failed before a registrar verdict was reached.
    #[error("DOI transport failed: {0}")]
    Transport(String),
    /// The registrar replied with a body the provider cannot interpret.
    #[error("DOI registrar response malformed: {0}")]
    MalformedResponse(String),
}

/// Provider-agnostic DOI minting.
#[async_trait]
pub trait DoiProvider: Send + Sync {
    /// Registrar name recorded on minted [`DoiRecord`]s.
    fn provider_name(&self) -> &str;

    /// Mint (register) a DOI for `resource` described by `metadata`, with
    /// `landing_url` as the DOI target.
    async fn mint(
        &self,
        resource: ResourceRef,
        metadata: &DataCiteMetadata,
        landing_url: &str,
    ) -> Result<DoiRecord, DoiError>;
}

/// Minimal HTTP layer a [`DataCiteProvider`] posts through.
///
/// Implement over the host's HTTP client; tests use an offline mock.
#[async_trait]
pub trait HttpTransport: Send + Sync {
    /// POST `body` as JSON to `url` with basic authentication, returning the
    /// response status and parsed JSON body.
    async fn post_json(
        &self,
        url: &str,
        basic_auth: (&str, &str),
        body: &Value,
    ) -> Result<(u16, Value), DoiError>;
}

/// DataCite REST API (`/dois`) implementation of [`DoiProvider`].
pub struct DataCiteProvider<T> {
    /// API base, e.g. `https://api.datacite.org` or
    /// `https://api.test.datacite.org`.
    pub api_base: String,
    /// DataCite repository id used as the basic-auth username.
    pub repository_id: String,
    /// Basic-auth password for the repository.
    pub password: String,
    /// DOI prefix to mint under when the metadata carries no DOI yet.
    pub doi_prefix: String,
    transport: T,
}

impl<T> DataCiteProvider<T> {
    /// Build a provider over the given transport.
    pub fn new(
        api_base: impl Into<String>,
        repository_id: impl Into<String>,
        password: impl Into<String>,
        doi_prefix: impl Into<String>,
        transport: T,
    ) -> Self {
        Self {
            api_base: api_base.into(),
            repository_id: repository_id.into(),
            password: password.into(),
            doi_prefix: doi_prefix.into(),
            transport,
        }
    }

    fn request_body(&self, metadata: &DataCiteMetadata, landing_url: &str) -> Value {
        let mut attributes = serde_json::json!({
            "event": "publish",
            "url": landing_url,
            "creators": metadata.creators,
            "titles": metadata.titles,
            "publisher": metadata.publisher,
            "publicationYear": metadata.publication_year,
            "types": {
                "resourceTypeGeneral": metadata.resource_type.resource_type_general,
                "resourceType": metadata.resource_type.resource_type,
            },
            "subjects": metadata.subjects,
            "rightsList": metadata.rights_list,
            "relatedIdentifiers": metadata.related_identifiers,
            "descriptions": metadata.descriptions,
            "dates": metadata.dates,
            "schemaVersion": format!("http://datacite.org/schema/kernel-{}", metadata.schema_version),
        });
        if metadata.identifier.identifier.is_empty() {
            attributes["prefix"] = Value::String(self.doi_prefix.clone());
        } else {
            attributes["doi"] = Value::String(metadata.identifier.identifier.clone());
        }
        serde_json::json!({
            "data": {
                "type": "dois",
                "attributes": attributes,
            }
        })
    }
}

#[async_trait]
impl<T: HttpTransport> DoiProvider for DataCiteProvider<T> {
    fn provider_name(&self) -> &str {
        "datacite"
    }

    async fn mint(
        &self,
        resource: ResourceRef,
        metadata: &DataCiteMetadata,
        landing_url: &str,
    ) -> Result<DoiRecord, DoiError> {
        let url = format!("{}/dois", self.api_base.trim_end_matches('/'));
        let body = self.request_body(metadata, landing_url);
        let (status, response) = self
            .transport
            .post_json(&url, (&self.repository_id, &self.password), &body)
            .await?;

        if (200..300).contains(&status) {
            let doi = response["data"]["attributes"]["doi"]
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| {
                    DoiError::MalformedResponse(
                        "registrar accepted the DOI but the response carries no \
                         data.attributes.doi"
                            .to_owned(),
                    )
                })?;
            Ok(DoiRecord {
                resource,
                doi: Some(doi),
                provider: self.provider_name().to_owned(),
                status: DoiStatus::Registered,
                response: Some(response),
                last_error: None,
            })
        } else {
            let error = response["errors"][0]["title"]
                .as_str()
                .map_or_else(|| format!("HTTP {status}"), str::to_owned);
            Ok(DoiRecord {
                resource,
                doi: None,
                provider: self.provider_name().to_owned(),
                status: DoiStatus::Failed,
                response: Some(response),
                last_error: Some(error),
            })
        }
    }
}
