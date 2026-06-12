use serde::{Deserialize, Serialize};

/// Open data licenses, identified by SPDX id.
///
/// Generic SPDX-ish vocabulary covering the major open licenses used for
/// research data: Creative Commons and Open Data Commons. Carries no domain
/// meaning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum DataLicense {
    #[serde(rename = "CC0")]
    Cc0,
    #[serde(rename = "CC_BY")]
    CcBy,
    #[serde(rename = "CC_BY_SA")]
    CcBySa,
    #[serde(rename = "CC_BY_NC")]
    CcByNc,
    #[serde(rename = "CC_BY_NC_SA")]
    CcByNcSa,
    #[serde(rename = "PDDL")]
    Pddl,
    #[serde(rename = "ODC_BY")]
    OdcBy,
    #[serde(rename = "ODC_ODbL")]
    OdcOdbl,
}

struct LicenseMeta {
    spdx: &'static str,
    uri: &'static str,
    name: &'static str,
}

impl DataLicense {
    const fn meta(self) -> &'static LicenseMeta {
        match self {
            Self::Cc0 => &LicenseMeta {
                spdx: "CC0-1.0",
                uri: "https://creativecommons.org/publicdomain/zero/1.0/",
                name: "Creative Commons Zero v1.0 Universal",
            },
            Self::CcBy => &LicenseMeta {
                spdx: "CC-BY-4.0",
                uri: "https://creativecommons.org/licenses/by/4.0/",
                name: "Creative Commons Attribution 4.0 International",
            },
            Self::CcBySa => &LicenseMeta {
                spdx: "CC-BY-SA-4.0",
                uri: "https://creativecommons.org/licenses/by-sa/4.0/",
                name: "Creative Commons Attribution-ShareAlike 4.0 International",
            },
            Self::CcByNc => &LicenseMeta {
                spdx: "CC-BY-NC-4.0",
                uri: "https://creativecommons.org/licenses/by-nc/4.0/",
                name: "Creative Commons Attribution-NonCommercial 4.0 International",
            },
            Self::CcByNcSa => &LicenseMeta {
                spdx: "CC-BY-NC-SA-4.0",
                uri: "https://creativecommons.org/licenses/by-nc-sa/4.0/",
                name: "Creative Commons Attribution-NonCommercial-ShareAlike 4.0 International",
            },
            Self::Pddl => &LicenseMeta {
                spdx: "PDDL-1.0",
                uri: "https://opendatacommons.org/licenses/pddl/1-0/",
                name: "Open Data Commons Public Domain Dedication and License v1.0",
            },
            Self::OdcBy => &LicenseMeta {
                spdx: "ODC-By-1.0",
                uri: "https://opendatacommons.org/licenses/by/1-0/",
                name: "Open Data Commons Attribution License v1.0",
            },
            Self::OdcOdbl => &LicenseMeta {
                spdx: "ODbL-1.0",
                uri: "https://opendatacommons.org/licenses/odbl/1-0/",
                name: "Open Data Commons Open Database License v1.0",
            },
        }
    }

    /// SPDX license identifier.
    #[must_use]
    pub const fn spdx_id(self) -> &'static str {
        self.meta().spdx
    }

    /// Canonical URL for the license text.
    #[must_use]
    pub const fn rights_uri(self) -> &'static str {
        self.meta().uri
    }

    /// Human-readable license name.
    #[must_use]
    pub const fn display_name(self) -> &'static str {
        self.meta().name
    }
}
