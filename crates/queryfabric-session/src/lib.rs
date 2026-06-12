//! Helpers for building browser session cookie `Set-Cookie` values.
//!
//! Provides the [`CookieSameSite`] enum and the [`session_cookie_value`] /
//! [`clear_session_cookie_value`] builders. The cookie name is supplied by
//! the caller so the crate is reusable across applications.

#![warn(missing_docs)]

use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

/// Cookie SameSite policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, EnumString)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum CookieSameSite {
    /// Send the cookie only for same-site requests.
    Strict,
    /// Send the cookie for same-site requests and top-level cross-site navigations.
    Lax,
    /// Send the cookie in all contexts; browsers typically also require `Secure`.
    None,
}

/// Build a browser session cookie value (`Set-Cookie` body).
///
/// Always sets `Path=/`, `HttpOnly`, the supplied `SameSite`, and `Max-Age`.
/// Adds `Secure` when `secure` is true.
#[must_use]
pub fn session_cookie_value(
    name: &str,
    session: Option<&str>,
    max_age: i64,
    same_site: CookieSameSite,
    secure: bool,
) -> String {
    let mut parts = vec![
        format!("{name}={}", session.unwrap_or("")),
        "Path=/".to_owned(),
        "HttpOnly".to_owned(),
        format!("SameSite={same_site}"),
        format!("Max-Age={max_age}"),
    ];
    if secure {
        parts.push("Secure".to_owned());
    }
    parts.join("; ")
}

/// Build a session cookie value that clears the session (empty value, `Max-Age=0`).
#[must_use]
pub fn clear_session_cookie_value(name: &str, same_site: CookieSameSite, secure: bool) -> String {
    session_cookie_value(name, None, 0, same_site, secure)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn serde_and_strum_roundtrip_all() {
        for (variant, expected) in [
            (CookieSameSite::Strict, "strict"),
            (CookieSameSite::Lax, "lax"),
            (CookieSameSite::None, "none"),
        ] {
            let json = serde_json::to_string(&variant).unwrap();
            assert_eq!(json, format!("\"{expected}\""));
            let back: CookieSameSite = serde_json::from_str(&json).unwrap();
            assert_eq!(back, variant);
            assert_eq!(variant.to_string(), expected);
            assert_eq!(CookieSameSite::from_str(expected).unwrap(), variant);
        }
    }

    #[test]
    fn invalid_inputs() {
        for bad_json in ["\"invalid\"", "\"Strict\"", "\"LAX\"", "\"None\"", "1"] {
            assert!(serde_json::from_str::<CookieSameSite>(bad_json).is_err());
        }
        for bad_str in ["invalid", "", "Strict"] {
            assert!(CookieSameSite::from_str(bad_str).is_err());
        }
    }

    #[test]
    fn session_cookie_value_respects_security_flags() {
        assert_eq!(
            session_cookie_value(
                "queryfabric_session",
                Some("abc"),
                60,
                CookieSameSite::Lax,
                false
            ),
            "queryfabric_session=abc; Path=/; HttpOnly; SameSite=lax; Max-Age=60"
        );
        assert_eq!(
            clear_session_cookie_value("queryfabric_session", CookieSameSite::None, true),
            "queryfabric_session=; Path=/; HttpOnly; SameSite=none; Max-Age=0; Secure"
        );
    }
}
