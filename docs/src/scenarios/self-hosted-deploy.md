# Scenario: Deploy a Self-Hosted Instance

**Who this is for:** You want to run the QueryFabric demonstrator on a NixOS
host with your own PostgreSQL database and, for durable export bundles, your
own S3-compatible object store.

**What you will end up with:** One `queryfabric-demo` systemd service listening
on a host-managed address and port. It serves a small static landing page and
the JSON HTTP routes implemented by the demonstrator. It does not provide a
SyQL editor, arbitrary table registration, or a generic ingestion API.

## Current deployment boundary

The QueryFabric NixOS module creates and hardens the systemd service, loads
credentials with systemd `LoadCredential`, and can open the configured HTTP
port. The host operator must separately provide:

- a PostgreSQL database and database role;
- an S3-compatible service and pre-created bucket when `store.backend = "s3"`;
- DNS, a reverse proxy, and TLS termination for a public HTTPS endpoint; and
- root-owned credential files, preferably managed by agenix or sops-nix.

The module does not provision PostgreSQL users or databases, an object store,
ACME certificates, or a reverse proxy.

```text
HTTP client ──► host reverse proxy (optional) ──► queryfabric-demo
                                                     │
                              ┌──────────────────────┴─────────────────────┐
                              ▼                                            ▼
                         PostgreSQL                                S3-compatible store
                    (provided by operator)                         (provided by operator)
```

## Configure the NixOS service

Add the QueryFabric flake input, import `queryfabric.nixosModules.default`, and
configure the service:

```nix
{
  services.queryfabric = {
    enable = true;
    listenAddress = "127.0.0.1";
    port = 8780;
    publicBaseUrl = "https://queryfabric.example.com";

    database.urlFile = "/run/secrets/queryfabric-db-url";
    auth.secretFile = "/run/secrets/queryfabric-auth-secret";

    store = {
      backend = "s3";
      endpoint = "http://127.0.0.1:9000";
      bucket = "queryfabric-exports";
      credentialsFile = "/run/secrets/queryfabric-store-creds";
    };
  };
}
```

`publicBaseUrl` is used in citations and DOI landing URLs; it does not set up
that domain. Keep the service bound to loopback when a reverse proxy on the
same host provides the public endpoint.

The referenced files must already exist on the host. Their formats are
documented in [Self-hosting on NixOS](../deployment/self-hosting-nixos.md),
along with the complete module option reference and the multi-instance form.

Apply the host configuration:

```console
$ sudo nixos-rebuild switch --flake .#hostname
$ systemctl status queryfabric
```

## Verify the current API

The demonstrator seeds its example air-quality resources by default. Verify
the service locally before adding a public reverse proxy:

```console
$ curl --fail http://127.0.0.1:8780/healthz
{"status":"ok"}

$ curl --fail http://127.0.0.1:8780/resources
```

Run a portable SQL query against the fixed demonstration catalog:

```console
$ curl --fail -X POST http://127.0.0.1:8780/query \
    -H 'content-type: application/json' \
    -d '{"sql":"SELECT city, avg(pm25) FROM readings JOIN stations ON readings.station_id = stations.station_id GROUP BY city"}'
```

The current route prefix is `/`, not `/api/v1`. `GET /catalog` describes the
fixed query catalog and `GET /resources` lists the seeded resources. There is
no route for registering arbitrary tables or inserting arbitrary records.
Export, import, erasure, and DOI mutation routes require a host-issued PASETO
bearer credential with the required role. See the
[NixOS deployment reference](../deployment/self-hosting-nixos.md#what-you-get)
for the complete route table.

## Troubleshooting

| Symptom | Check |
|---------|-------|
| Unit does not start | Run `journalctl -u queryfabric`; verify the database and authentication credential files exist and contain valid values. |
| Database connection fails | Confirm that the operator-provided database, role, and network path match `database.urlFile`. |
| S3 configuration is rejected | For the `s3` backend, set `endpoint`, `bucket`, and `credentialsFile`, and create the bucket before starting the service. |
| Public HTTPS endpoint fails | Check the host-managed reverse proxy, DNS, and certificate. The QueryFabric module does not configure them. |
