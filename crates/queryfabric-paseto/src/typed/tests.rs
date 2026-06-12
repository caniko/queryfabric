use super::*;
use rusty_paseto::prelude::*;

const TEST_SECRET: &str = "test_secret_key_1234567890abcdef";

fn test_uuid() -> Uuid {
    Uuid::parse_str("01234567-89ab-7def-8123-456789abcdef").unwrap()
}

/// Build a PASETO v4.local token with the given custom claims.
fn build_token(claims: &[(&str, &str)]) -> String {
    let key = PasetoSymmetricKey::<V4, Local>::from(Key::from(
        TEST_SECRET.as_bytes().first_chunk::<32>().unwrap(),
    ));
    let mut builder = PasetoBuilder::<V4, Local>::default();
    builder.set_claim(ExpirationClaim::try_from("2099-01-01T00:00:00+00:00").unwrap());
    for &(key_name, value) in claims {
        match key_name {
            "sub" => {
                builder.set_claim(SubjectClaim::from(value));
            }
            _ => {
                builder.set_claim(CustomClaim::try_from((key_name, value)).unwrap());
            }
        }
    }
    builder.build(&key).unwrap()
}

/// Build a token with a specific secret (not TEST_SECRET).
fn build_token_with_secret(secret: &str, claims: &[(&str, &str)]) -> String {
    let key = PasetoSymmetricKey::<V4, Local>::from(Key::from(
        secret.as_bytes().first_chunk::<32>().unwrap(),
    ));
    let mut builder = PasetoBuilder::<V4, Local>::default();
    builder.set_claim(ExpirationClaim::try_from("2099-01-01T00:00:00+00:00").unwrap());
    for &(key_name, value) in claims {
        match key_name {
            "sub" => {
                builder.set_claim(SubjectClaim::from(value));
            }
            _ => {
                builder.set_claim(CustomClaim::try_from((key_name, value)).unwrap());
            }
        }
    }
    builder.build(&key).unwrap()
}

// -----------------------------------------------------------------
// Happy path: full token with all boolean claim combinations
// -----------------------------------------------------------------

#[test]
fn valid_token_all_claims() {
    let uid = test_uuid();
    for (active, superuser, verified) in [
        ("true", "false", "true"),
        ("true", "true", "true"),
        ("false", "false", "false"),
    ] {
        let token = build_token(&[
            ("sub", &uid.to_string()),
            ("email", "test@example.com"),
            ("is_active", active),
            ("is_superuser", superuser),
            ("is_verified", verified),
        ]);
        let user = validate_paseto_token(&token, TEST_SECRET).unwrap();
        assert_eq!(user.id, uid);
        assert_eq!(user.email.as_str(), "test@example.com");
        assert_eq!(user.is_active, active == "true");
        assert_eq!(user.is_superuser, superuser == "true");
        assert_eq!(user.is_verified, verified == "true");
    }
}

#[test]
fn defaults_when_boolean_claims_missing() {
    let uid = test_uuid();
    let token = build_token(&[("sub", &uid.to_string()), ("email", "test@example.com")]);
    let user = validate_paseto_token(&token, TEST_SECRET).unwrap();
    assert!(user.is_active); // default true
    assert!(!user.is_superuser); // default false
    assert!(!user.is_verified); // default false
}

#[test]
fn non_boolean_is_active_treated_as_false() {
    let uid = test_uuid();
    let token = build_token(&[
        ("sub", &uid.to_string()),
        ("email", "test@example.com"),
        ("is_active", "not_a_bool"),
    ]);
    let user = validate_paseto_token(&token, TEST_SECRET).unwrap();
    assert!(!user.is_active);
}

#[test]
fn extra_claims_ignored() {
    let uid = test_uuid();
    let token = build_token(&[
        ("sub", &uid.to_string()),
        ("email", "extra@test.com"),
        ("custom_field", "custom_value"),
    ]);
    let user = validate_paseto_token(&token, TEST_SECRET).unwrap();
    assert_eq!(user.email.as_str(), "extra@test.com");
}

// -----------------------------------------------------------------
// Error paths
// -----------------------------------------------------------------

#[test]
fn missing_sub_claim() {
    let token = build_token(&[("email", "test@example.com")]);
    assert!(matches!(
        validate_paseto_token(&token, TEST_SECRET).unwrap_err(),
        AuthError::MissingSub
    ));
}

#[test]
fn missing_email_claim() {
    let uid = test_uuid();
    let token = build_token(&[("sub", &uid.to_string())]);
    assert!(matches!(
        validate_paseto_token(&token, TEST_SECRET).unwrap_err(),
        AuthError::MissingEmail
    ));
}

#[test]
fn invalid_uuid_in_sub() {
    let token = build_token(&[("sub", "not-a-uuid"), ("email", "test@example.com")]);
    assert!(matches!(
        validate_paseto_token(&token, TEST_SECRET).unwrap_err(),
        AuthError::InvalidUuid(_)
    ));
}

#[test]
fn secret_length_edge_cases() {
    // Too short
    for short in ["", "short", "abcdefghijklmnopqrstuvwxyz01234"] {
        assert!(
            matches!(
                validate_paseto_token("v4.local.fake", short).unwrap_err(),
                AuthError::SecretTooShort
            ),
            "secret len={} should be rejected",
            short.len()
        );
    }
    // Exactly 32, 33, and longer all work
    for secret in [
        "abcdefghijklmnopqrstuvwxyz012345",
        "abcdefghijklmnopqrstuvwxyz0123456",
        "abcdefghijklmnopqrstuvwxyz0123456789abcdefghijklmnop",
    ] {
        let uid = test_uuid();
        let token = build_token_with_secret(
            secret,
            &[("sub", &uid.to_string()), ("email", "test@example.com")],
        );
        let user = validate_paseto_token(&token, secret).unwrap();
        assert_eq!(user.id, uid);
    }
}

#[test]
fn corrupted_token() {
    assert!(matches!(
        validate_paseto_token("garbage-data", TEST_SECRET).unwrap_err(),
        AuthError::TokenParse(_)
    ));
}

#[test]
fn wrong_secret() {
    let uid = test_uuid();
    let token = build_token(&[("sub", &uid.to_string()), ("email", "test@example.com")]);
    let wrong = "wrong_secret_key_1234567890abcdef";
    assert!(matches!(
        validate_paseto_token(&token, wrong).unwrap_err(),
        AuthError::TokenParse(_)
    ));
}

#[test]
fn expired_token() {
    let uid = test_uuid();
    let key = PasetoSymmetricKey::<V4, Local>::from(Key::from(
        TEST_SECRET.as_bytes().first_chunk::<32>().unwrap(),
    ));
    let uid_str = uid.to_string();
    let token = PasetoBuilder::<V4, Local>::default()
        .set_claim(ExpirationClaim::try_from("2000-01-01T00:00:00+00:00").unwrap())
        .set_claim(SubjectClaim::from(uid_str.as_str()))
        .set_claim(CustomClaim::try_from(("email", "test@example.com")).unwrap())
        .build(&key)
        .unwrap();
    assert!(matches!(
        validate_paseto_token(&token, TEST_SECRET).unwrap_err(),
        AuthError::TokenParse(_)
    ));
}

// -----------------------------------------------------------------
// AuthError display
// -----------------------------------------------------------------

#[test]
fn auth_error_display_all_variants() {
    let cases: Vec<(AuthError, &str)> = vec![
        (AuthError::SecretTooShort, "32 bytes"),
        (AuthError::TokenParse("bad".into()), "bad"),
        (AuthError::MissingSub, "sub"),
        (AuthError::MissingEmail, "email"),
        (AuthError::InvalidEmail("bad@".into()), "bad@"),
        (
            AuthError::InvalidUuid(Uuid::parse_str("xyz").unwrap_err()),
            "Invalid UUID",
        ),
    ];
    for (err, expected_substr) in &cases {
        assert!(
            err.to_string().contains(expected_substr),
            "{err:?} should contain '{expected_substr}'"
        );
    }
}

#[test]
fn auth_error_source_chain() {
    // InvalidUuid has #[from] so it has a source
    let uuid_err = Uuid::parse_str("invalid").unwrap_err();
    let auth_err = AuthError::InvalidUuid(uuid_err);
    assert!(std::error::Error::source(&auth_err).is_some());

    // Others do not
    for err in [
        AuthError::SecretTooShort,
        AuthError::MissingSub,
        AuthError::MissingEmail,
    ] {
        assert!(std::error::Error::source(&err).is_none());
    }
}

// -----------------------------------------------------------------
// AuthUser serde
// -----------------------------------------------------------------

#[test]
fn auth_user_serde_roundtrip() {
    let user = AuthUser {
        id: test_uuid(),
        email: Email::new_unchecked("test@example.com"),
        is_active: false,
        is_superuser: true,
        is_verified: false,
        user_type: UserType::default(),
        roles: Vec::new(),
    };
    let json = serde_json::to_string(&user).unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["email"], "test@example.com");
    assert_eq!(v["is_active"], false);
    assert_eq!(v["is_superuser"], true);
    assert!(v["id"].is_string());

    let back: AuthUser = serde_json::from_str(&json).unwrap();
    assert_eq!(back.id, user.id);
    assert_eq!(back.email.as_str(), user.email.as_str());
    assert_eq!(back.is_active, user.is_active);
    assert_eq!(back.is_superuser, user.is_superuser);
    assert_eq!(back.is_verified, user.is_verified);
}

#[test]
fn auth_user_clone_independence() {
    let user = AuthUser {
        id: test_uuid(),
        email: Email::new_unchecked("clone@test.com"),
        is_active: true,
        is_superuser: false,
        is_verified: true,
        user_type: UserType::default(),
        roles: Vec::new(),
    };
    let mut cloned = user.clone();
    cloned.is_active = false;
    assert!(user.is_active);
    assert!(!cloned.is_active);
}

#[path = "tests/int_delegation.rs"]
mod delegation;
