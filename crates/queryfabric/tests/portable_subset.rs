use std::collections::BTreeSet;

use queryfabric::{
    ClickHouseAdapter, GenericSqlDialect, PostgresAdapter, QueryCompiler, QueryDiagnostic,
    bind_and_validate_query,
};

mod support;

#[test]
fn portable_subset_corpus_binds_analyzes_and_emits_as_declared() {
    let compiler = QueryCompiler::default();
    let dialect = GenericSqlDialect;
    let raw =
        std::fs::read_to_string(support::repo_root().join("conformance/portable-subset.json"))
            .expect("corpus");
    let corpus: support::PortableSubsetCorpus = serde_json::from_str(&raw).expect("corpus json");

    for case in corpus.cases {
        let parsed = compiler.parse(&dialect, &case.query).expect("parse");
        let catalog = support::portable_catalog("portable-subset-corpus");
        let bound = bind_and_validate_query(&parsed, &catalog, &case.parameters);

        match bound {
            Ok(bound) => {
                let actual_requirements = bound
                    .capability_requirements()
                    .required()
                    .iter()
                    .map(|requirement| format!("{requirement:?}"))
                    .collect::<BTreeSet<_>>();
                let expected_requirements = case
                    .required_capabilities
                    .iter()
                    .cloned()
                    .collect::<BTreeSet<_>>();
                assert_eq!(
                    actual_requirements, expected_requirements,
                    "case `{}` capability requirements diverged",
                    case.id
                );

                if !case.expected_schema.is_empty() {
                    assert_eq!(
                        bound.result_schema().fields(),
                        case.expected_schema.as_slice(),
                        "case `{}` result schema diverged",
                        case.id
                    );
                }

                for (backend, expected) in &case.expected_backends {
                    let expected_backend_errors = case
                        .expected_backend_error_codes
                        .get(backend)
                        .cloned()
                        .unwrap_or_default();
                    match backend.as_str() {
                        "clickhouse" => {
                            let analysis = compiler.analyze(&bound, &ClickHouseAdapter, &catalog);
                            if expected == "supported" {
                                assert!(
                                    analysis.supported,
                                    "clickhouse should support `{}`",
                                    case.id
                                );
                                assert!(
                                    compiler.emit(&bound, &ClickHouseAdapter, &catalog).is_ok(),
                                    "clickhouse should emit `{}`",
                                    case.id
                                );
                            } else {
                                assert!(
                                    !analysis.supported,
                                    "clickhouse should reject `{}`",
                                    case.id
                                );
                                assert_diagnostic_codes(
                                    &analysis.diagnostics,
                                    &expected_backend_errors,
                                    &case.id,
                                    "clickhouse",
                                );
                                assert!(
                                    compiler.emit(&bound, &ClickHouseAdapter, &catalog).is_err(),
                                    "clickhouse emit should reject `{}`",
                                    case.id
                                );
                            }
                        }
                        "postgres" => {
                            let analysis = compiler.analyze(&bound, &PostgresAdapter, &catalog);
                            if expected == "supported" {
                                assert!(
                                    analysis.supported,
                                    "postgres should support `{}`",
                                    case.id
                                );
                                assert!(
                                    compiler.emit(&bound, &PostgresAdapter, &catalog).is_ok(),
                                    "postgres should emit `{}`",
                                    case.id
                                );
                            } else {
                                assert!(
                                    !analysis.supported,
                                    "postgres should reject `{}`",
                                    case.id
                                );
                                assert_diagnostic_codes(
                                    &analysis.diagnostics,
                                    &expected_backend_errors,
                                    &case.id,
                                    "postgres",
                                );
                                assert!(
                                    compiler.emit(&bound, &PostgresAdapter, &catalog).is_err(),
                                    "postgres emit should reject `{}`",
                                    case.id
                                );
                            }
                        }
                        other => panic!("unknown backend `{other}` in corpus"),
                    }
                }
            }
            Err(error) if error.as_bind().is_some() => {
                let diagnostics = &error.as_bind().expect("bind details").diagnostics;
                assert!(
                    case.expected_backends
                        .values()
                        .all(|status| status == "rejected"),
                    "bind failed for `{}` but not all backends were marked rejected",
                    case.id
                );
                assert_diagnostic_codes(
                    diagnostics,
                    &case.expected_bind_error_codes,
                    &case.id,
                    "bind",
                );
            }
            Err(error) => panic!("unexpected error for `{}`: {error:?}", case.id),
        }
    }
}

fn assert_diagnostic_codes(
    diagnostics: &[QueryDiagnostic],
    expected_codes: &[String],
    case_id: &str,
    stage: &str,
) {
    let actual = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect::<BTreeSet<_>>();
    for code in expected_codes {
        assert!(
            actual.contains(code.as_str()),
            "case `{case_id}` missing {stage} diagnostic `{code}` in {:?}",
            actual
        );
    }
}
