# Scenario: Data Portability and GDPR Export

QueryFabric keeps the export contract at the host boundary. The reference
demonstrator exposes GDPR access/erasure endpoints and stores an import-ready
tabular bundle in an S3-compatible object store.

## Export bundle versions

Bundle `1.0` is a legacy export-only JSON document. It uses the historical
QueryFabric canonicalizer and a hexadecimal BLAKE3 address. It is not an
authenticated signature.

Bundle `2.0` is the MVP import format. It uses RFC 8785 JSON Canonicalization,
typed `blake3-256:<64 lowercase hex>` digests, and one normative profile:
`queryfabric.tabular-csv/1`. The profile requires UTF-8 CSV, comma delimiters,
CRLF records, exact typed headers, non-nullable Boolean/Int64/Float64/String/
UUID/RFC3339-UTC fields, and bounded validation. A digest is content
addressing, not a signature; expected hashes must arrive through an
authenticated operator channel.

## Host export

The demonstrator's `POST /resources/{id}/export` route queries the host
database, writes the profile-1 CSV and canonical bundle to the configured
object store, and returns the typed bundle digest. `GET
/resources/{id}/bundle` reads the sealed bundle back. The bundle carries
citations, licence/restriction declarations, and source provenance as evidence.
It does not carry a target owner's authorization or a signature.

## Operator-mediated import

The MVP deliberately does not follow bundle `storageUri` values. An operator
transfers the canonical bundle and artifact bytes, then submits them to the
target host:

```json
{
  "bundle": "<canonical bundle JSON>",
  "artifact": "<profile-1 CSV>",
  "expectedBundleDigest": "blake3-256:…",
  "target": "lis-baixa"
}
```

`POST /imports/dry-run` stages the artifact under a content-addressed key,
revalidates the bundle and artifact bytes, checks the exact predeclared
relation schema, and returns a deterministic plan digest and staging object.
Copy `planDigest` and `stagedObject` from that response into the subsequent
`POST /imports/apply` request. Apply repeats the checks against the immutable
staged bytes and atomically commits target rows plus a durable receipt, local
policy/owner, carried source evidence, and source-to-target mapping in
PostgreSQL. Replaying the same plan returns the original receipt. A changed
artifact or stale plan fails before the database transaction.

The reference host currently maps this profile to its predeclared `readings`
relation. Dynamic DDL, arbitrary schemas, URI fetching, and service migration
are intentionally outside the MVP.

## GDPR access export

`GET /resources/{id}/access-export` returns the local policy and audit history.
`POST /resources/{id}/erase` is owner-only and records a soft-deletion event;
the audit trail remains available. These endpoints are host authorization
surfaces, not portability-crate policy decisions.

## DOI and legal boundaries

The demonstrator's DOI provider is local and uses DataCite's reserved test
prefix. Configure a registrar integration before minting production DOIs.
Portability and import code preserve licence/restriction declarations as
mandatory local policy input; consult the organisation's data-protection
officer for legal interpretation.
