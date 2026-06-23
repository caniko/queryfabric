# Scenario: Data Portability and GDPR Export

**Who this is for:** You run a QueryFabric instance that stores personal or
sensitive data. Users have the right to access, rectify, and export their data
under GDPR Articles 15, 16, and 20. You need a mechanism to produce portable
export bundles with cryptographic provenance.

**What you'll end up with:** An API endpoint that accepts a user identity and
returns a signed, content-addressed bundle containing all data the system holds
about that user — plus a DOI and a citation.

## How it works

```
Request ──► Host application ──► queryfabric-access (find resources)
                │
                ▼
         queryfabric-portability (build bundle, mint DOI)
                │
                ▼
         S3/MinIO (store bundle)
                │
                ▼
         Return download URL + DOI
```

## Step 1: Implement `AccessPolicy` for your resources

The `queryfabric-access` crate defines the contract:

```rust
use queryfabric_access::{AccessDecision, AccessOutcome, Subject};
use queryfabric_contract::AccessPolicy;

struct MyAccessPolicy;

impl AccessDecision for MyAccessPolicy {
    fn evaluate(&self, subject: &Subject, policy: &AccessPolicy) -> AccessOutcome {
        match policy {
            AccessPolicy::Open => AccessOutcome::Allow,
            AccessPolicy::Registered if subject.registered => AccessOutcome::Allow,
            _ => AccessOutcome::Deny {
                reason: "access denied".into(),
            },
        }
    }
}
```

## Step 2: Collect resources for a subject

```rust
use queryfabric_access::{OwnershipSnapshot, ResourceRef};
use queryfabric_contract::Subject;

async fn collect_user_data(subject: &Subject) -> Vec<ResourceRef> {
    // Query your database for all resources owned by this subject.
    // Return their identifiers so the portability layer can bundle them.
}
```

## Step 3: Build an export bundle

```rust
use queryfabric_portality::{Bundle, BundleEntry, BundleManifest};
use std::time::SystemTime;

let bundle = Bundle::builder()
    .subject(subject.clone())
    .entries(vec![
        BundleEntry::json("measurements", &measurements)?,
        BundleEntry::json("profile", &profile)?,
    ])
    .build()?;

// The bundle is content-addressed: its ID is a BLAKE3 hash of its contents.
let manifest: BundleManifest = bundle.manifest();
println!("Bundle ID: {}", manifest.content_hash);
```

## Step 4: Mint a DOI

```rust
use queryfabric_portality::DoiMinter;

let doi = DoiMinter::new("https://api.datacite.org", credentials)
    .mint(
        &manifest,
        format!("Export for user {}", subject.id),
    )
    .await?;

println!("DOI: {}", doi);
```

## Step 5: Return the bundle

```rust
let url = store.upload(&bundle).await?;

Ok(ExportResponse {
    download_url: url.to_string(),
    doi: doi.to_string(),
    expires_at: SystemTime::now() + Duration::from_days(30),
})
```

## The export bundle structure

The bundle is a JSON-LD document with:

```json
{
    "@context": "https://w3id.org/queryfabric/export/v1",
    "id": "blake3:a1b2c3...",
    "subject": { "id": "uuid:...", "registered": true },
    "created": "2026-06-23T12:00:00Z",
    "entries": [
        {
            "path": "measurements.json",
            "content_type": "application/json",
            "content_hash": "blake3:...",
            "size": 1234
        }
    ],
    "provenance": {
        "compiler_version": "0.2.0",
        "catalog_snapshot": "snapshot-2026-06-23"
    }
}
```

## Legal note

This scenario implements the technical mechanism for GDPR data portability.
It does not provide legal advice. Consult your organisation's data protection
officer to verify compliance with your specific regulatory requirements.
