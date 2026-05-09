from __future__ import annotations

import json

import queryfabric


def catalog() -> queryfabric.MemoryCatalog:
    catalog = queryfabric.MemoryCatalog()
    catalog.set_snapshot_id("python-test-catalog")
    catalog.register_relation(
        queryfabric.RelationSchema(
            "neurons",
            [
                queryfabric.ColumnSchema(
                    "neuron_id", queryfabric.DataType.uuid(), nullable=False
                ),
                queryfabric.ColumnSchema(
                    "cable_length", queryfabric.DataType.float64()
                ),
                queryfabric.ColumnSchema("species", queryfabric.DataType.utf8()),
            ],
            kind=queryfabric.RelationKind.table(),
        )
    )
    return catalog


def test_parse_syql_summary_matches_expected_shape() -> None:
    parsed = queryfabric.parse_syql(
        "SELECT neuron_id FROM neurons WHERE cable_length > 100 LIMIT 5"
    )
    summary = parsed.summary()
    assert summary["primary_relation"] == "neurons"
    assert summary["projected_columns"] == ["neuron_id"]
    assert summary["predicate_count"] == 1
    assert summary["row_limit"] == 5
    assert summary["scope"] == "local"
    assert summary["output_format"] == "arrow"


def test_inspect_parameters_reports_positional_and_named_placeholders() -> None:
    parsed = queryfabric.parse_sql(
        "SELECT neuron_id FROM neurons WHERE cable_length > $1 AND species = :species"
    )
    summary = queryfabric.inspect_parameters(parsed)
    assert summary.positional_count == 1
    assert summary.named_params == ["species"]


def test_bind_analyze_and_emit_clickhouse() -> None:
    parsed = queryfabric.parse_syql(
        "SELECT neuron_id FROM neurons WHERE cable_length > 100 LIMIT 3"
    )
    bound = queryfabric.bind_and_validate(parsed, catalog())
    analysis = queryfabric.analyze_clickhouse(bound, catalog())
    assert analysis.supported is True
    artifact = queryfabric.emit_clickhouse_sql(bound, catalog())
    payload = artifact.to_dict()
    assert payload["dialect"] == "clickhouse"
    assert "SELECT" in payload["text"]
    assert "neurons" in payload["text"]


def test_memory_catalog_document_roundtrip() -> None:
    original = catalog()
    payload = original.to_dict()
    roundtrip = queryfabric.MemoryCatalog.from_dict(payload)

    assert roundtrip.to_dict() == payload
    assert roundtrip.to_json() == original.to_json()


def test_memory_catalog_from_json_roundtrip() -> None:
    payload = catalog().to_json()
    roundtrip = queryfabric.MemoryCatalog.from_json(payload)

    assert roundtrip.to_json() == payload


def test_emit_postgres_sql() -> None:
    parsed = queryfabric.parse_sql("SELECT neuron_id FROM neurons LIMIT 2")
    bound = queryfabric.bind_and_validate(parsed, catalog())
    artifact = queryfabric.emit_postgres_sql(bound, catalog())
    assert artifact.dialect == "postgres"
    assert artifact.text == "SELECT neurons.neuron_id FROM neurons LIMIT 2"


def test_query_parameters_and_json_roundtrip() -> None:
    params = queryfabric.QueryParameters()
    params.insert_positional(1, 42.0)
    params.insert_named("species", "mouse")
    parsed = queryfabric.parse_sql(
        "SELECT neuron_id FROM neurons WHERE cable_length > $1 AND species = :species"
    )
    bound = queryfabric.bind_and_validate(parsed, catalog(), params)
    payload = bound.to_dict()
    assert payload["parameters"][0]["schema"]["reference"]["Positional"] == 1
    assert payload["parameters"][1]["schema"]["reference"]["Named"] == "species"
    assert "neurons" in bound.to_json()


def test_query_parameters_accept_json_object_values() -> None:
    params = queryfabric.QueryParameters()
    params.insert_named("filters", {"species": "mouse", "count": 2})
    payload = params.to_dict()
    assert json.loads(payload["named"]["filters"]["Json"]) == {
        "species": "mouse",
        "count": 2,
    }
