use serde_json::Value;

/// Generic facts a citation is rendered from. No domain fields: hosts fold
/// domain detail into `keywords` or the title.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitationInput {
    /// Stable citation key (e.g. the resource UUID in simple form).
    pub id: String,
    /// Resource title.
    pub title: String,
    /// Publishing platform or organisation.
    pub publisher: String,
    /// Publication year, already formatted (e.g. `"2026"`).
    pub year: String,
    /// Landing URL (the DOI URL when one exists).
    pub url: String,
    /// DOI, when minted.
    pub doi: Option<String>,
    /// SPDX license identifier, when declared.
    pub license_spdx: Option<String>,
    /// Free keywords (CFF `keywords`, RIS `KW`).
    pub keywords: Vec<String>,
    /// Repository URL for CFF, when distinct from `url`.
    pub repository_url: Option<String>,
}

/// Supported citation output formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CitationFormat {
    BibTeX,
    Ris,
    Apa,
    /// CSL-JSON — native import format for Zotero, Mendeley, and most modern
    /// reference managers.
    CslJson,
    /// Citation File Format (CFF) — standard for research data/software
    /// citation, used by forges and archives.
    Cff,
}

impl CitationFormat {
    /// MIME content type for the format.
    #[must_use]
    pub const fn content_type(self) -> &'static str {
        match self {
            Self::BibTeX => "application/x-bibtex",
            Self::Ris => "application/x-research-info-systems",
            Self::Apa => "text/plain",
            Self::CslJson => "application/vnd.citationstyles.csl+json",
            Self::Cff => "text/yaml",
        }
    }

    /// Suggested file extension (without dot).
    #[must_use]
    pub const fn file_extension(self) -> &'static str {
        match self {
            Self::BibTeX => "bib",
            Self::Ris => "ris",
            Self::Apa => "txt",
            Self::CslJson => "json",
            Self::Cff => "cff",
        }
    }
}

/// Render one citation format from generic input.
#[must_use]
pub fn generate_citation(input: &CitationInput, format: CitationFormat) -> String {
    match format {
        CitationFormat::BibTeX => bibtex(input),
        CitationFormat::Ris => ris(input),
        CitationFormat::Apa => apa(input),
        CitationFormat::CslJson => csl_json_string(input),
        CitationFormat::Cff => cff(input),
    }
}

/// CSL-JSON as a parsed value (a single-element array per the spec).
#[must_use]
pub(crate) fn csl_json_value(input: &CitationInput) -> Value {
    let mut item = serde_json::json!({
        "type": "dataset",
        "id": input.id,
        "title": input.title,
        "publisher": input.publisher,
        "URL": input.url,
        "issued": { "date-parts": [[input.year]] },
    });
    if let Some(license) = &input.license_spdx {
        item["license"] = Value::String(license.clone());
    }
    if let Some(doi) = &input.doi {
        item["DOI"] = Value::String(doi.clone());
    }
    Value::Array(vec![item])
}

fn csl_json_string(input: &CitationInput) -> String {
    serde_json::to_string_pretty(&csl_json_value(input))
        .expect("serde_json::json! value is always serializable")
}

fn bibtex(input: &CitationInput) -> String {
    let mut bib = format!(
        "@misc{{{id},\n  title = {{{title}}},\n  publisher = {{{publisher}}},\n  year = {{{year}}},\n  url = {{{url}}}",
        id = input.id,
        title = input.title,
        publisher = input.publisher,
        year = input.year,
        url = input.url,
    );
    if let Some(license) = &input.license_spdx {
        bib.push_str(&format!(",\n  license = {{{license}}}"));
    }
    if let Some(doi) = &input.doi {
        bib.push_str(&format!(",\n  doi = {{{doi}}}"));
    }
    bib.push_str("\n}");
    bib
}

fn ris(input: &CitationInput) -> String {
    let mut ris = format!(
        "TY  - DATA\nTI  - {title}\nPB  - {publisher}\nPY  - {year}\nUR  - {url}",
        title = input.title,
        publisher = input.publisher,
        year = input.year,
        url = input.url,
    );
    for keyword in &input.keywords {
        ris.push_str(&format!("\nKW  - {keyword}"));
    }
    if let Some(doi) = &input.doi {
        ris.push_str(&format!("\nDO  - {doi}"));
    }
    ris.push_str("\nER  - ");
    ris
}

fn apa(input: &CitationInput) -> String {
    let doi_part = input
        .doi
        .as_ref()
        .map(|doi| format!(" https://doi.org/{doi}"))
        .unwrap_or_default();
    format!(
        "{publisher}. ({year}). {title} [Data set]. {publisher}.{doi_part}",
        publisher = input.publisher,
        year = input.year,
        title = input.title,
    )
}

fn cff(input: &CitationInput) -> String {
    let mut cff = format!(
        "cff-version: 1.2.0\n\
         message: \"If you use this dataset, please cite it as below.\"\n\
         title: \"{title}\"\n\
         type: dataset\n",
        title = input.title,
    );
    if let Some(license) = &input.license_spdx {
        cff.push_str(&format!("license: {license}\n"));
    }
    cff.push_str(&format!("url: \"{url}\"\n", url = input.url));
    if let Some(repository) = &input.repository_url {
        cff.push_str(&format!("repository: \"{repository}\"\n"));
    }
    if !input.keywords.is_empty() {
        cff.push_str("keywords:\n");
        for keyword in &input.keywords {
            cff.push_str(&format!("  - {keyword}\n"));
        }
    }
    cff.push_str(&format!(
        "date-released: \"{year}-01-01\"\n",
        year = input.year
    ));
    if let Some(doi) = &input.doi {
        cff.push_str(&format!(
            "identifiers:\n  - type: doi\n    value: \"{doi}\"\n"
        ));
    }
    cff
}
