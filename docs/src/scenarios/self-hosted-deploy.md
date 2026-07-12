# Scenario: Deploy a Self-Hosted Instance

**Who this is for:** You want to run the QueryFabric self-hosted demonstrator on
your own infrastructure — a single-node service that accepts portable queries,
stores data in PostgreSQL, and exports bundles to S3.

**What you'll end up with:** A running QueryFabric instance accessible at
`https://queryfabric.example.com` with a web UI, a SyQL query editor, and a
REST API.

## Architecture

```text
Browser ──► queryfabric-demo ──► PostgreSQL (metadata, catalog)
                  │
                  └──► S3/MinIO (export bundles, provenance)
```

## Option A: NixOS (recommended)

The project ships a NixOS module that wires everything together:

```nix
{
  imports = [ inputs.queryfabric.nixosModules.default ];

  services.queryfabric = {
    enable = true;
    domain = "queryfabric.example.com";
    database.url = "postgres://user:pass@localhost:5432/queryfabric";
    store.endpoint = "https://s3.example.com";
    store.bucket = "queryfabric-exports";
    federation.enable = false;  # single-node mode
  };
}
```

Apply and deploy:

```bash
nixos-rebuild switch --flake .#hostname
```

The module sets up:

- systemd service with hardening
- PostgreSQL database and user
- TLS via ACME (Let's Encrypt)
- Firewall rules
- Health check endpoint

See [Self-hosting on NixOS](../deployment/self-hosting-nixos.md) for the full
reference.

## Option B: Docker / Podman

```yaml
# docker-compose.yml
services:
  postgres:
    image: postgres:17-alpine
    environment:
      POSTGRES_DB: queryfabric
      POSTGRES_PASSWORD: changeme
    volumes:
      - pgdata:/var/lib/postgresql/data

  queryfabric:
    image: codeberg.org/caniko/queryfabric-demo:latest
    ports:
      - "8780:8780"
    environment:
      QF_DATABASE_URL: postgres://postgres:changeme@postgres:5432/queryfabric
      QF_S3_ENDPOINT: http://minio:9000
      QF_DOMAIN: localhost:8780
    depends_on:
      - postgres

volumes:
  pgdata:
```

```bash
docker compose up -d
```

## Step 1: Register a table

```bash
curl -X POST https://queryfabric.example.com/api/v1/catalog \
  -H "Content-Type: application/json" \
  -d '{
    "name": "measurements",
    "columns": [
      {"name": "sample_id", "type": "uuid", "nullable": false},
      {"name": "value", "type": "float64", "nullable": true}
    ]
  }'
```

## Step 2: Insert data

```bash
curl -X POST https://queryfabric.example.com/api/v1/ingest/measurements \
  -H "Content-Type: application/json" \
  -d '[{"sample_id": "a1b2c3...", "value": 42.5}]'
```

## Step 3: Run a query

```bash
curl -X POST https://queryfabric.example.com/api/v1/query \
  -H "Content-Type: application/json" \
  -d '{"syql": "FROM measurements WHERE value > 10"}'
```

## Troubleshooting

| Symptom | Likely cause |
|---------|--------------|
| "Connection refused" | PostgreSQL not started or wrong URL. Check `services.queryfabric.database.url`. |
| "Bucket not found" | S3 bucket does not exist. Create it: `mc mb minio/queryfabric-exports`. |
| "TLS handshake error" | ACME certificate not provisioned yet. Wait 30s and retry. |
