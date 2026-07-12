# Self-hosting on NixOS

QueryFabric ships a NixOS module and a demonstrator service so a single
host can run a portable query API with the full data-sovereignty surface —
export bundles, GDPR access/erase, DOI minting — backed by Postgres and any
S3-compatible object store (Garage, MinIO, AWS S3). Mutation and import routes
require a host-issued PASETO bearer credential.

## Quick start

Add the flake input and import the module:

```nix
{
  inputs.queryfabric.url = "git+https://codeberg.org/caniko/queryfabric";

  outputs = {nixpkgs, queryfabric, ...}: {
    nixosConfigurations.datahost = nixpkgs.lib.nixosSystem {
      system = "x86_64-linux";
      modules = [
        queryfabric.nixosModules.default
        ./configuration.nix
      ];
    };
  };
}
```

Then enable the service. The legacy shorthand keeps the original
single-instance shape:

```nix
{config, ...}: {
  services.queryfabric = {
    enable = true;
    listenAddress = "127.0.0.1";
    port = 8780;

    # Connection URL with credentials lives in a root-only file, loaded
    # via systemd LoadCredential — never the world-readable Nix store.
    database.urlFile = "/run/secrets/queryfabric-db-url";
    auth.secretFile = "/run/secrets/queryfabric-auth-secret";

    store = {
      backend = "s3";
      endpoint = "http://127.0.0.1:9000";
      bucket = "queryfabric";
      credentialsFile = "/run/secrets/queryfabric-store-creds";
    };

    federation.enable = true; # optional: announce a federation identity
  };

  # Companion services on the same host.
  services.postgresql = {
    enable = true;
    enableTCPIP = true;
  };
  services.minio = {
    enable = true;
    rootCredentialsFile = "/run/secrets/minio-root";
  };
}
```

For multi-tenant hosts, use the instance API instead. Each instance gets
its own service unit, runtime state directory, credentials, port, and
federation identity:

```nix
{config, ...}: {
  services.queryfabric.instances.alpha = {
    listenAddress = "127.0.0.1";
    port = 8780;
    database.urlFile = "/run/secrets/queryfabric-alpha-db-url";
    auth.secretFile = "/run/secrets/queryfabric-alpha-auth-secret";
    store = {
      backend = "s3";
      endpoint = "http://127.0.0.1:9000";
      bucket = "queryfabric-alpha";
      credentialsFile = "/run/secrets/queryfabric-alpha-store-creds";
    };
    federation = {
      enable = true;
      nodeName = "queryfabric-alpha";
      flightPort = 50051;
    };
  };

  services.queryfabric.instances.beta = {
    listenAddress = "127.0.0.1";
    port = 8781;
    database.urlFile = "/run/secrets/queryfabric-beta-db-url";
    auth.secretFile = "/run/secrets/queryfabric-beta-auth-secret";
    store = {
      backend = "s3";
      endpoint = "http://127.0.0.1:9000";
      bucket = "queryfabric-beta";
      credentialsFile = "/run/secrets/queryfabric-beta-store-creds";
    };
    federation = {
      enable = true;
      nodeName = "queryfabric-beta";
      flightPort = 50052;
    };
  };
}
```

```text
# /run/secrets/queryfabric-auth-secret
at-least-32-random-bytes-from-the-host-secret-manager
```

You can mix the new API with the legacy shorthand only if you do not also
define `services.queryfabric.instances.default`. The old top-level options
still map to a virtual default instance, and the module rejects a config
that defines both surfaces at once.

The secret files referenced above follow the same pattern per instance:

```text
# /run/secrets/queryfabric-db-url
postgres://queryfabric:CHANGE-ME@127.0.0.1:5432/queryfabric
```

```text
# /run/secrets/queryfabric-store-creds
QFDEMO_STORE_ACCESS_KEY=CHANGE-ME
QFDEMO_STORE_SECRET_KEY=CHANGE-ME
```

Provision them with your secret manager of choice (agenix, sops-nix, or
plain root-owned files with mode `0600`). The module hands them to the
service through `LoadCredential`, so they are readable only by the service
at runtime and never enter the Nix store.

## What you get

Once the unit is up (`systemctl status queryfabric` for the legacy path or
`systemctl status queryfabric-alpha` for a named instance), the service
seeds a generic urban air-quality dataset and exposes:

| Endpoint | Purpose |
|---|---|
| `GET /healthz` | readiness probe |
| `GET /catalog` | queryable relations and snapshot id |
| `GET /resources` | resources with their access policy |
| `POST /query` | portable SQL, compiled and validated against the catalog, executed on Postgres |
| `POST /resources/{id}/export` | build the content-addressed export bundle and store it |
| `GET /resources/{id}/bundle` | read the sealed bundle back from the object store |
| `GET /resources/{id}/access-export` | GDPR Art. 15 structured access export |
| `POST /resources/{id}/erase` | GDPR Art. 17 soft erasure (owner-only, audited) |
| `POST /resources/{id}/doi` | mint a demonstration DOI (DataCite test prefix) |
| `POST /imports/dry-run` | validate and stage a profile-1 artifact, returning plan/staging identities |
| `POST /imports/apply` | revalidate the dry-run identities and persist the import receipt |
| `GET /federation/status` | federation node identity |

Try a query:

```console
$ curl -s -X POST http://127.0.0.1:8780/query \
    -H 'content-type: application/json' \
    -d '{"sql": "SELECT city, avg(pm25) FROM readings JOIN stations ON readings.station_id = stations.station_id GROUP BY city"}'
```

Replace `$QF_TOKEN` in mutation/import requests with a valid bearer token
issued by the host identity service:

```console
$ curl -s -X POST http://127.0.0.1:8780/resources/lis-baixa/export \
    -H "authorization: Bearer $QF_TOKEN" | jq .contentHash
```

## Module options

All options live under `services.queryfabric`. The same per-instance option
set is available under `services.queryfabric.instances.<name>`:

| Option | Default | Description |
|---|---|---|
| `enable` | `false` | enable the legacy default instance |
| `package` | the flake's `queryfabric-demo` | binary to run |
| `listenAddress` / `port` | `127.0.0.1` / `8780` | HTTP bind |
| `publicBaseUrl` | listen address | external URL used in citations/DOIs |
| `logLevel` | `info` | `RUST_LOG` filter |
| `database.url` | – | inline connection URL (passwordless URLs only — it lands in the Nix store) |
| `database.urlFile` | – | secret file with the connection URL (preferred) |
| `database.migrationUrl` / `migrationUrlFile` | `database.url` | migration-admin URL or secret file |
| `database.queryUrl` / `queryUrlFile` | `database.url` | read-only query URL or secret file |
| `database.importUrl` / `importUrlFile` | `database.url` | narrow import-writer URL or secret file |
| `auth.secret` | – | inline PASETO validation secret (development only) |
| `auth.secretFile` | – | secret file with the PASETO validation secret (preferred) |
| `store.backend` | `memory` | `memory` (evaluation only) or `s3` |
| `store.endpoint` / `store.bucket` / `store.region` | – / – / `us-east-1` | S3 connection facts |
| `store.credentialsFile` | – | secret file with `QFDEMO_STORE_ACCESS_KEY` / `QFDEMO_STORE_SECRET_KEY` |
| `federation.enable` | `false` | announce a federation node identity |
| `federation.nodeName` | `queryfabric-demo` | announced node name |
| `federation.hubMultiaddrs` | `[]` | federation hub multiaddrs |
| `federation.flightPort` | `50051` | announced Arrow Flight port |
| `openFirewall` | `false` | open the HTTP port |

Named instances default to enabled; set `enable = false` to stage one
without starting it.

Namespacing rules:

- `services.queryfabric.instances.<name>` creates `queryfabric-<name>.service`.
- Each instance gets `StateDirectory=queryfabric-<name>`.
- `database.urlFile` and `store.credentialsFile` are passed through
  per-unit `LoadCredential` entries, so the credentials stay out of the Nix
  store.
- `listenAddress`, `port`, `federation.flightPort`, and
  `federation.nodeName` must be unique across enabled instances when the
  corresponding features are enabled.
- `openFirewall` adds the instance port to the host firewall; multiple
  instances are aggregated.

## Security posture

- Database, object-store, and PASETO secrets travel exclusively through systemd `LoadCredential`; the unit in
  the Nix store contains no credential material. The end-to-end VM test
  asserts this by grepping the unit's store path for the test secrets.
- The service runs as a `DynamicUser` with `NoNewPrivileges`,
  `ProtectSystem=strict`, `PrivateTmp`, `PrivateDevices`, syscall
  filtering (`@system-service`, minus `@privileged`), and an empty
  capability bounding set.
- Use distinct `migrationUrlFile`, `queryUrlFile`, and `importUrlFile`
  credentials in production. The migration role owns schema setup, the query
  role is read-only, and the import role is limited to target rows and receipt
  tables; the migration VM asserts the query and import denials.

## Testing the stack

The repository carries the full VM test:

```console
$ nix build .#checks.x86_64-linux.selfhost
```

It boots a NixOS VM with Postgres, Garage, and the module, then drives a
portable query, an authenticated export/import round-trip through Garage, the
GDPR access/erase flow, DOI minting, stale-plan rejection, and the
secret-hygiene assertion.
