use super::*;

// -----------------------------------------------------------------
// Delegation token tests
// -----------------------------------------------------------------

fn test_user() -> AuthUser {
    AuthUser {
        id: test_uuid(),
        email: Email::new_unchecked("researcher@university.edu"),
        is_active: true,
        is_superuser: false,
        is_verified: true,
        user_type: UserType::default(),
        roles: Vec::new(),
    }
}

#[test]
fn delegation_token_roundtrip() {
    let user = test_user();
    let ids = vec![Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7()];
    let table = 1; // example table id

    let token =
        mint_delegation_token(&user, &ids, table, DelegationOperation::Read, TEST_SECRET).unwrap();
    let claims = validate_delegation_token(&token, TEST_SECRET).unwrap();

    assert_eq!(claims.sub, user.id);
    assert_eq!(claims.email.as_str(), user.email.as_str());
    assert_eq!(claims.dataset_ids, ids);
    assert_eq!(claims.table_id, table);
    assert_eq!(claims.operation, DelegationOperation::Read);
}

#[test]
fn delegation_token_empty_dataset_ids() {
    let user = test_user();
    let token =
        mint_delegation_token(&user, &[], 3, DelegationOperation::Write, TEST_SECRET).unwrap();
    let claims = validate_delegation_token(&token, TEST_SECRET).unwrap();
    assert!(claims.dataset_ids.is_empty());
    assert_eq!(claims.table_id, 3);
    assert_eq!(claims.operation, DelegationOperation::Write);
}

#[test]
fn delegation_token_wrong_secret_rejected() {
    let user = test_user();
    let token = mint_delegation_token(
        &user,
        &[Uuid::now_v7()],
        1,
        DelegationOperation::Read,
        TEST_SECRET,
    )
    .unwrap();
    let wrong = "wrong_secret_key_1234567890abcdef";
    assert!(validate_delegation_token(&token, wrong).is_err());
}

#[test]
fn delegation_token_rejects_regular_token() {
    // A regular user token has no scope claim → should fail validation
    let uid = test_uuid();
    let token = build_token(&[("sub", &uid.to_string()), ("email", "test@example.com")]);
    let err = validate_delegation_token(&token, TEST_SECRET).unwrap_err();
    assert!(matches!(err, AuthError::MissingDelegationClaim(_)));
}

#[test]
fn delegation_token_rejects_wrong_scope() {
    let uid = test_uuid();
    let token = build_token(&[
        ("sub", &uid.to_string()),
        ("email", "test@example.com"),
        ("scope", "user"),
        ("dataset_ids", "[]"),
        ("table_id", "1"),
    ]);
    let err = validate_delegation_token(&token, TEST_SECRET).unwrap_err();
    assert!(matches!(err, AuthError::InvalidScope { .. }));
}

#[test]
fn delegation_token_all_table_ids() {
    let user = test_user();
    let id = Uuid::now_v7();
    for table in 1..=11 {
        let token =
            mint_delegation_token(&user, &[id], table, DelegationOperation::Read, TEST_SECRET)
                .unwrap();
        let claims = validate_delegation_token(&token, TEST_SECRET).unwrap();
        assert_eq!(claims.table_id, table);
    }
}

#[test]
fn delegation_token_short_secret_rejected() {
    let user = test_user();
    for short in ["", "short", "abcdefghijklmnopqrstuvwxyz01234"] {
        assert!(
            matches!(
                mint_delegation_token(
                    &user,
                    &[Uuid::now_v7()],
                    1,
                    DelegationOperation::Read,
                    short,
                )
                .unwrap_err(),
                AuthError::SecretTooShort
            ),
            "mint should reject secret of len {}",
            short.len()
        );
    }
    // validate also rejects short secrets
    assert!(matches!(
        validate_delegation_token("v4.local.fake", "short").unwrap_err(),
        AuthError::SecretTooShort
    ));
}

#[test]
fn delegation_token_many_dataset_ids() {
    let user = test_user();
    let ids: Vec<Uuid> = (0..500).map(|_| Uuid::now_v7()).collect();
    let token =
        mint_delegation_token(&user, &ids, 1, DelegationOperation::Read, TEST_SECRET).unwrap();
    let claims = validate_delegation_token(&token, TEST_SECRET).unwrap();
    assert_eq!(claims.dataset_ids.len(), 500);
    assert_eq!(claims.dataset_ids, ids);
}

#[test]
fn delegation_token_negative_table_id() {
    let user = test_user();
    let token = mint_delegation_token(
        &user,
        &[Uuid::now_v7()],
        -1,
        DelegationOperation::Read,
        TEST_SECRET,
    )
    .unwrap();
    let claims = validate_delegation_token(&token, TEST_SECRET).unwrap();
    assert_eq!(claims.table_id, -1);
}

#[test]
fn delegation_token_preserves_email_exactly() {
    let emails = [
        "researcher@university.edu",
        "a@b.c",
        "user+tag@example.com",
        "UPPER@CASE.ORG",
    ];
    for email in emails {
        let user = AuthUser {
            id: test_uuid(),
            email: Email::new_unchecked(email),
            is_active: true,
            is_superuser: false,
            is_verified: true,
            user_type: UserType::default(),
            roles: Vec::new(),
        };
        let token =
            mint_delegation_token(&user, &[], 1, DelegationOperation::Read, TEST_SECRET).unwrap();
        let claims = validate_delegation_token(&token, TEST_SECRET).unwrap();
        assert_eq!(claims.email.as_str(), email);
    }
}

#[test]
fn delegation_token_missing_dataset_ids_claim() {
    let uid = test_uuid();
    let token = build_token(&[
        ("sub", &uid.to_string()),
        ("email", "test@example.com"),
        ("scope", "delegation"),
        ("table_id", "1"),
        ("operation", "read"),
        // dataset_ids intentionally omitted
    ]);
    let err = validate_delegation_token(&token, TEST_SECRET).unwrap_err();
    assert!(matches!(err, AuthError::MissingDelegationClaim(ref f) if f == "dataset_ids"));
}

#[test]
fn delegation_token_missing_table_id_claim() {
    let uid = test_uuid();
    let token = build_token(&[
        ("sub", &uid.to_string()),
        ("email", "test@example.com"),
        ("scope", "delegation"),
        ("dataset_ids", "[]"),
        ("operation", "read"),
        // table_id intentionally omitted
    ]);
    let err = validate_delegation_token(&token, TEST_SECRET).unwrap_err();
    assert!(matches!(err, AuthError::MissingDelegationClaim(ref f) if f == "table_id"));
}

#[test]
fn delegation_token_non_uuid_in_dataset_ids() {
    let uid = test_uuid();
    let token = build_token(&[
        ("sub", &uid.to_string()),
        ("email", "test@example.com"),
        ("scope", "delegation"),
        ("dataset_ids", r#"["not-a-uuid", "also-bad"]"#),
        ("table_id", "1"),
        ("operation", "read"),
    ]);
    let err = validate_delegation_token(&token, TEST_SECRET).unwrap_err();
    assert!(
        matches!(err, AuthError::InvalidDelegationClaim { ref field, .. } if field == "dataset_ids")
    );
}

#[test]
fn delegation_token_non_integer_table_id() {
    let uid = test_uuid();
    let token = build_token(&[
        ("sub", &uid.to_string()),
        ("email", "test@example.com"),
        ("scope", "delegation"),
        ("dataset_ids", "[]"),
        ("table_id", "not_a_number"),
        ("operation", "read"),
    ]);
    let err = validate_delegation_token(&token, TEST_SECRET).unwrap_err();
    assert!(
        matches!(err, AuthError::InvalidDelegationClaim { ref field, .. } if field == "table_id")
    );
}

#[test]
fn delegation_token_malformed_dataset_ids_json() {
    let uid = test_uuid();
    let token = build_token(&[
        ("sub", &uid.to_string()),
        ("email", "test@example.com"),
        ("scope", "delegation"),
        ("dataset_ids", "not-json-at-all"),
        ("table_id", "1"),
        ("operation", "read"),
    ]);
    let err = validate_delegation_token(&token, TEST_SECRET).unwrap_err();
    assert!(
        matches!(err, AuthError::InvalidDelegationClaim { ref field, .. } if field == "dataset_ids")
    );
}

#[test]
fn delegation_token_missing_operation_rejected() {
    let uid = test_uuid();
    let token = build_token(&[
        ("sub", &uid.to_string()),
        ("email", "test@example.com"),
        ("scope", "delegation"),
        ("dataset_ids", "[]"),
        ("table_id", "1"),
    ]);
    let err = validate_delegation_token(&token, TEST_SECRET).unwrap_err();
    assert!(matches!(err, AuthError::MissingDelegationClaim(ref f) if f == "operation"));
}

#[test]
fn delegation_token_unknown_operation_rejected() {
    let uid = test_uuid();
    let token = build_token(&[
        ("sub", &uid.to_string()),
        ("email", "test@example.com"),
        ("scope", "delegation"),
        ("dataset_ids", "[]"),
        ("table_id", "1"),
        ("operation", "delete"),
    ]);
    let err = validate_delegation_token(&token, TEST_SECRET).unwrap_err();
    assert!(
        matches!(err, AuthError::InvalidDelegationClaim { ref field, .. } if field == "operation")
    );
}

#[test]
fn auth_user_is_service_delegates_to_user_type() {
    let human = AuthUser {
        id: test_uuid(),
        email: Email::new_unchecked("test@example.com"),
        is_active: true,
        is_superuser: false,
        is_verified: true,
        user_type: UserType::Human,
        roles: Vec::new(),
    };
    assert!(!human.is_service());

    let service = AuthUser {
        user_type: UserType::Service,
        ..human
    };
    assert!(service.is_service());
}
