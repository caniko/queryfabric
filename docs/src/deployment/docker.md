# Docker / Podman

QueryFabric ships a self-host demonstrator (`queryfabric-demo`) as a Docker
image. This page covers running it with Docker Compose or Podman.

## Quick start with Docker Compose

```yaml
# compose.yaml
services:
  postgres:
    image: postgres:17-alpine
    environment:
      POSTGRES_DB: queryfabric
      POSTGRES_PASSWORD: changeme
      POSTGRES_USER: queryfabric
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U queryfabric"]
      interval: 5s
      timeout: 5s
      retries: 5
    volumes:
      - pgdata:/var/lib/postgresql/data

  minio:
    image: minio/minio:latest
    command: server /data --console-address ":9001"
    environment:
      MINIO_ROOT_USER: minioadmin
      MINIO_ROOT_PASSWORD: minioadmin
    healthcheck:
      test: ["CMD", "mc", "ready", "local"]
      interval: 5s
      timeout: 5s
      retries: 5
    volumes:
      - minio_data:/data

  queryfabric:
    image: codeberg.org/caniko/queryfabric-demo:latest
    ports:
      - "8780:8780"
    environment:
      QF_DOMAIN: localhost:8780
      QF_DATABASE_URL: postgres://queryfabric:changeme@postgres:5432/queryfabric
      QF_S3_ENDPOINT: http://minio:9000
      QF_S3_ACCESS_KEY: minioadmin
      QF_S3_SECRET_KEY: minioadmin
      QF_S3_BUCKET: queryfabric-exports
      RUST_LOG: info
    depends_on:
      postgres:
        condition: service_healthy
      minio:
        condition: service_healthy

volumes:
  pgdata:
  minio_data:
```

```bash
docker compose up -d
# Open http://localhost:8780
```

## Podman (rootless)

The same compose file works with Podman:

```bash
podman compose up -d
```

If you use rootless Podman, ensure the `postgres` volume is writable by the
container user:

```bash
podman unshare chown 999:999 pgdata  # postgres UID inside container
```

## Configuration reference

| Variable | Default | Description |
|----------|---------|-------------|
| `QF_DOMAIN` | `localhost:8780` | Public-facing domain for CORS and redirects |
| `QF_DATABASE_URL` | — | PostgreSQL connection string |
| `QF_S3_ENDPOINT` | — | S3-compatible endpoint URL |
| `QF_S3_ACCESS_KEY` | — | S3 access key |
| `QF_S3_SECRET_KEY` | — | S3 secret key |
| `QF_S3_BUCKET` | `queryfabric-exports` | S3 bucket for export bundles |
| `RUST_LOG` | `info` | Logging level |

## Building the image locally

```bash
nix build .#oci-image
docker load < result
docker tag queryfabric-demo:latest your-registry/queryfabric-demo:latest
```

Or with plain cargo:

```bash
cargo build --release -p queryfabric-demo
```
