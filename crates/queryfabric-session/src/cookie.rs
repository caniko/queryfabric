//! SynDB session cookie helpers.
//!
//! Thin wrappers around [`crate::session_cookie_value`] and
//! [`crate::clear_session_cookie_value`] that hard-code the SynDB cookie name
//! so call sites don't have to.

use crate::{CookieSameSite, clear_session_cookie_value, session_cookie_value};

/// Browser session cookie used by the SynDB UI and API.
pub const SESSION_COOKIE_NAME: &str = "syndb_session";

/// Build the browser session cookie value shared by the API and UI.
#[must_use]
pub fn web_session_cookie_value(
    session: Option<&str>,
    max_age: i64,
    same_site: CookieSameSite,
    secure: bool,
) -> String {
    session_cookie_value(SESSION_COOKIE_NAME, session, max_age, same_site, secure)
}

/// Build a browser session cookie value that clears the session.
#[must_use]
pub fn clear_web_session_cookie_value(same_site: CookieSameSite, secure: bool) -> String {
    clear_session_cookie_value(SESSION_COOKIE_NAME, same_site, secure)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_cookie_value_uses_syndb_name() {
        assert_eq!(
            web_session_cookie_value(Some("abc"), 60, CookieSameSite::Lax, false),
            "syndb_session=abc; Path=/; HttpOnly; SameSite=lax; Max-Age=60"
        );
        assert_eq!(
            clear_web_session_cookie_value(CookieSameSite::None, true),
            "syndb_session=; Path=/; HttpOnly; SameSite=none; Max-Age=0; Secure"
        );
    }
}
