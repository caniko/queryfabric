//! Fernet symmetric encryption with env-driven key rotation.
//!
//! [`crate::fernet::load_multi_fernet`] reads a JSON array of base64-encoded 32-byte Fernet
//! keys from a configurable environment variable. The first key is used for
//! encryption; all keys are tried for decryption (so old keys keep working
//! after rotation).

#![warn(missing_docs)]

use fernet::{Fernet, MultiFernet};
use secrecy::SecretString;
use thiserror::Error;

/// Errors raised while loading Fernet keys or encrypting/decrypting payloads.
#[derive(Debug, Error)]
pub enum FernetError {
    /// The configured environment variable was missing.
    #[error("env var {0} is not set")]
    EnvMissing(String),
    /// The environment variable did not contain valid JSON.
    #[error("env var {var} is not valid JSON: {source}")]
    InvalidJson {
        /// Environment variable name.
        var: String,
        /// JSON parser error.
        #[source]
        source: serde_json::Error,
    },
    /// The JSON array was empty.
    #[error("env var {0} must contain at least one key")]
    NoKeys(String),
    /// One array entry was not a valid Fernet key.
    #[error("{var}[{index}] is not a valid Fernet key")]
    InvalidKey {
        /// Environment variable name.
        var: String,
        /// Zero-based array index of the invalid key.
        index: usize,
    },
    /// No supplied Fernet key could decrypt the token.
    #[error("decryption failed (token may be from a rotated-out key)")]
    DecryptionFailed,
    /// Decrypted bytes were not valid UTF-8.
    #[error("decrypted plaintext is not valid UTF-8: {0}")]
    NotUtf8(#[from] std::string::FromUtf8Error),
}

/// Result type used by this module.
pub type Result<T> = std::result::Result<T, FernetError>;

/// Build a [`MultiFernet`] from the named environment variable.
///
/// The variable must hold a JSON array of base64-encoded 32-byte keys.
///
/// # Errors
/// Returns a structured [`FernetError`] when the environment variable is
/// missing, malformed, empty, or contains invalid keys.
pub fn load_multi_fernet(env_var: &str) -> Result<MultiFernet> {
    let keys_json =
        std::env::var(env_var).map_err(|_| FernetError::EnvMissing(env_var.to_owned()))?;

    let keys: Vec<String> =
        serde_json::from_str(&keys_json).map_err(|source| FernetError::InvalidJson {
            var: env_var.to_owned(),
            source,
        })?;

    if keys.is_empty() {
        return Err(FernetError::NoKeys(env_var.to_owned()));
    }

    let fernet_keys: Vec<Fernet> = keys
        .into_iter()
        .enumerate()
        .map(|(i, k)| {
            Fernet::new(&k).ok_or_else(|| FernetError::InvalidKey {
                var: env_var.to_owned(),
                index: i,
            })
        })
        .collect::<Result<_>>()?;

    Ok(MultiFernet::new(fernet_keys))
}

/// Encrypt a UTF-8 string using the keys from `env_var`.
///
/// # Errors
/// Returns any key-loading error from [`crate::fernet::load_multi_fernet`].
pub fn encrypt(env_var: &str, plaintext: &str) -> Result<String> {
    let mf = load_multi_fernet(env_var)?;
    Ok(mf.encrypt(plaintext.as_bytes()))
}

/// Decrypt a Fernet token using the keys from `env_var`. Returns a
/// `SecretString` so the plaintext doesn't appear in `Debug` output.
///
/// # Errors
/// Returns any key-loading error, [`FernetError::DecryptionFailed`] when the
/// token cannot be decrypted, or [`FernetError::NotUtf8`] when plaintext is
/// not valid UTF-8.
pub fn decrypt(env_var: &str, token: &str) -> Result<SecretString> {
    let mf = load_multi_fernet(env_var)?;
    let plaintext = mf
        .decrypt(token)
        .map_err(|_| FernetError::DecryptionFailed)?;
    let s = String::from_utf8(plaintext)?;
    Ok(SecretString::from(s))
}

#[cfg(test)]
mod tests {
    use super::*;
    use secrecy::ExposeSecret;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());
    const TEST_VAR: &str = "TEST_FERNET_ROTATION_KEYS";

    fn set_valid_key() {
        let key = Fernet::generate_key();
        // SAFETY: `set_var` is only unsafe because it is not thread-safe. Every
        // test that touches `TEST_VAR` holds `ENV_LOCK` for its whole body, and
        // cargo-nextest runs each test in its own process, so no other thread
        // observes the environment mid-mutation. Callers hold the lock before
        // invoking this helper.
        unsafe {
            std::env::set_var(TEST_VAR, format!(r#"["{key}"]"#));
        }
    }

    #[test]
    fn roundtrip() {
        let _lock = ENV_LOCK.lock().unwrap();
        set_valid_key();
        let encrypted = encrypt(TEST_VAR, "secret").unwrap();
        let decrypted = decrypt(TEST_VAR, &encrypted).unwrap();
        assert_eq!(decrypted.expose_secret(), "secret");
    }

    #[test]
    fn fresh_iv_per_encrypt() {
        let _lock = ENV_LOCK.lock().unwrap();
        set_valid_key();
        let a = encrypt(TEST_VAR, "x").unwrap();
        let b = encrypt(TEST_VAR, "x").unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn missing_env() {
        let _lock = ENV_LOCK.lock().unwrap();
        // SAFETY: `remove_var` is only unsafe because it is not thread-safe.
        // `ENV_LOCK` is held for the whole test body and nextest isolates each
        // test in its own process, so no concurrent env access can occur.
        unsafe {
            std::env::remove_var(TEST_VAR);
        }
        let err = encrypt(TEST_VAR, "x").unwrap_err();
        assert!(matches!(err, FernetError::EnvMissing(_)));
    }

    #[test]
    fn empty_array() {
        let _lock = ENV_LOCK.lock().unwrap();
        // SAFETY: `set_var` is only unsafe because it is not thread-safe.
        // `ENV_LOCK` is held for the whole test body and nextest isolates each
        // test in its own process, so no concurrent env access can occur.
        unsafe {
            std::env::set_var(TEST_VAR, "[]");
        }
        let err = encrypt(TEST_VAR, "x").unwrap_err();
        assert!(matches!(err, FernetError::NoKeys(_)));
    }

    #[test]
    fn invalid_json() {
        let _lock = ENV_LOCK.lock().unwrap();
        // SAFETY: `set_var` is only unsafe because it is not thread-safe.
        // `ENV_LOCK` is held for the whole test body and nextest isolates each
        // test in its own process, so no concurrent env access can occur.
        unsafe {
            std::env::set_var(TEST_VAR, "not-json");
        }
        let err = encrypt(TEST_VAR, "x").unwrap_err();
        assert!(matches!(err, FernetError::InvalidJson { .. }));
    }
}
