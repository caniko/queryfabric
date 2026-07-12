# End-to-end self-host test: one NixOS VM running Postgres + Garage + two
# queryfabric demonstrator instances through the NixOS module. Secrets are
# created at VM runtime and handed to each service via LoadCredential; the
# test asserts they never appear in the unit's store path.
{
  pkgs,
  nixosModule,
}:
pkgs.testers.runNixOSTest {
  name = "queryfabric-selfhost";

  nodes.machine =
    { lib, ... }:
    {
      imports = [ nixosModule ];

      virtualisation.memorySize = 2048;
      virtualisation.cores = 2;

      services.postgresql = {
        enable = true;
        enableTCPIP = true;
      };

      virtualisation.diskSize = 4096;

      services.garage = {
        enable = true;
        package = pkgs.garage_2;
        settings = {
          rpc_bind_addr = "127.0.0.1:3901";
          rpc_public_addr = "127.0.0.1:3901";
          rpc_secret = "5c1915fa04d0b6739675c61bf5907eb0fe3d9c69850c83820f51b4d25d13868c";
          replication_factor = 1;
          consistency_mode = "consistent";
          s3_api = {
            s3_region = "us-east-1";
            api_bind_addr = "127.0.0.1:9000";
            root_domain = ".s3.garage";
          };
        };
      };

      services.queryfabric.instances.alpha = {
        listenAddress = "127.0.0.1";
        port = 8780;
        database.urlFile = "/root/qfalpha-db-url";
        auth.secretFile = "/root/qfalpha-auth-secret";
        store = {
          backend = "s3";
          endpoint = "http://127.0.0.1:9000";
          bucket = "queryfabric-alpha";
          credentialsFile = "/root/qfalpha-store-creds";
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
        database.urlFile = "/root/qfbeta-db-url";
        auth.secretFile = "/root/qfbeta-auth-secret";
        store = {
          backend = "s3";
          endpoint = "http://127.0.0.1:9000";
          bucket = "queryfabric-beta";
          credentialsFile = "/root/qfbeta-store-creds";
        };
        federation = {
          enable = true;
          nodeName = "queryfabric-beta";
          flightPort = 50052;
        };
      };

      # Secrets are written by the test script after boot; do not start the
      # services before they exist.
      systemd.services.queryfabric-alpha.wantedBy = lib.mkForce [ ];
      systemd.services.queryfabric-beta.wantedBy = lib.mkForce [ ];

      environment.systemPackages = [
        pkgs.curl
        pkgs.minio-client
      ];
    };

  testScript = ''
    import json

    JSON_HEADER = " -H 'content-type: application/json'"
    AUTH_TOKEN = "v4.local.7YoCIGisuMEE_g46oSO_uTRGiZbR_d96apYfYQWAGzXQ07T517-vONyS7-pRrLRO7a9Uf7Or2wvHyrvDm4T2IdG98EDF91T58R_bCdGEblRVHXe0JuMp9EjereFJOEigiO6ZuwvFyUtR9DMQ3ZdxtVhFqsPQzS4qYeQ64Q3rIdVcL3hHqmfhV-_5gn_LTkX6ebRBATWsbeQBwItpw67kotTTAsOWmPE4NoCQG0vmNdF482Ml4SOSxlQVtuQ6jzcOFzpW0t6espbI7iwsQWt2Gui85b61VpCozOXamqe4IlLSmfN0nrtzMKs2yRdRsR4yrl2cRaq9FtRo6_6m29dcw2Yj-RWKfK5OFukgJK2z516DEI3fhcbJB8K5DoT_w1lEFJ2eU2sm6kr3bRgHHUybgySGWWRXaayO_AsG5yiyNdc6seRabPOBkwI"
    AUTH_HEADER = " -H 'authorization: Bearer " + AUTH_TOKEN + "'"


    def base(port):
        return "http://127.0.0.1:" + str(port)


    def get_json(port, path):
        return json.loads(machine.succeed("curl -sf " + base(port) + path + AUTH_HEADER))


    def post(port, path):
        return json.loads(
            machine.succeed("curl -sS --fail-with-body -X POST " + base(port) + path + AUTH_HEADER)
        )


    def post_json(port, path, payload):
        body = " -d '" + json.dumps(payload) + "'"
        return json.loads(
            machine.succeed(
                "curl -sS --fail-with-body -X POST "
                + base(port)
                + path
                + JSON_HEADER
                + AUTH_HEADER
                + body
            )
        )


    def post_json_status(port, path, payload):
        body = " -d '" + json.dumps(payload) + "'"
        return machine.succeed(
            "curl -s -o /dev/null -w '%{http_code}' -X POST "
            + base(port) + path + JSON_HEADER + AUTH_HEADER + body
        ).strip()


    def unauth_post_json_status(port, path, payload):
        body = " -d '" + json.dumps(payload) + "'"
        return machine.succeed(
            "curl -s -o /dev/null -w '%{http_code}' -X POST "
            + base(port) + path + JSON_HEADER + body
        ).strip()


    machine.start()
    machine.wait_for_unit("postgresql.service")
    machine.wait_for_unit("garage.service")
    machine.wait_for_open_port(5432)
    machine.wait_for_open_port(9000)

    with subtest("provision databases, Garage buckets, and runtime secrets"):
        setup_sql = (
            "CREATE ROLE qfalpha LOGIN PASSWORD 'qfalpha-pg-secret';\n"
            "CREATE DATABASE qfalpha OWNER qfalpha;\n"
            "CREATE ROLE qfbeta LOGIN PASSWORD 'qfbeta-pg-secret';\n"
            "CREATE DATABASE qfbeta OWNER qfbeta;\n"
        )
        machine.succeed("cat > /tmp/setup.sql << 'EOF'\n" + setup_sql + "EOF")
        machine.succeed('su -s /bin/sh postgres -c "psql -f /tmp/setup.sql"')
        machine.succeed("garage layout assign $(garage node id | cut -d@ -f1) -z qf -c 1G")
        machine.succeed("garage layout apply --version 1")
        key_output = machine.succeed("garage key create qf-queryfabric")

        def key_field(name):
            prefix = name + ":"
            for line in key_output.splitlines():
                if line.startswith(prefix):
                    return line.split(":", 1)[1].strip()
            raise AssertionError("Garage key output did not contain " + name)

        key_id = key_field("Key ID")
        secret_key = key_field("Secret key")
        machine.succeed("garage bucket create queryfabric-alpha")
        machine.succeed("garage bucket create queryfabric-beta")
        machine.succeed(
            "garage bucket allow --read --write queryfabric-alpha --key qf-queryfabric"
        )
        machine.succeed(
            "garage bucket allow --read --write queryfabric-beta --key qf-queryfabric"
        )
        machine.succeed(
            "mc alias set local http://127.0.0.1:9000 " + key_id + " " + secret_key
        )
        machine.succeed(
            "install -m 600 /dev/null /root/qfalpha-db-url && "
            "echo 'postgres://qfalpha:qfalpha-pg-secret@127.0.0.1:5432/qfalpha'"
            " > /root/qfalpha-db-url"
        )
        machine.succeed(
            "install -m 600 /dev/null /root/qfbeta-db-url && "
            "echo 'postgres://qfbeta:qfbeta-pg-secret@127.0.0.1:5432/qfbeta'"
            " > /root/qfbeta-db-url"
        )
        machine.succeed(
            "install -m 600 /dev/null /root/qfalpha-auth-secret && "
            "printf '%s' 'qf-demo-auth-secret-2026-operator-000000' > /root/qfalpha-auth-secret"
        )
        machine.succeed(
            "install -m 600 /dev/null /root/qfbeta-auth-secret && "
            "printf '%s' 'qf-demo-auth-secret-2026-operator-000000' > /root/qfbeta-auth-secret"
        )
        machine.succeed(
            "install -m 600 /dev/null /root/qfalpha-store-creds && "
            "cat > /root/qfalpha-store-creds << 'EOF'\n"
            "QFDEMO_STORE_ACCESS_KEY=" + key_id + "\n"
            "QFDEMO_STORE_SECRET_KEY=" + secret_key + "\n"
            "EOF"
        )
        machine.succeed(
            "install -m 600 /dev/null /root/qfbeta-store-creds && "
            "cat > /root/qfbeta-store-creds << 'EOF'\n"
            "QFDEMO_STORE_ACCESS_KEY=" + key_id + "\n"
            "QFDEMO_STORE_SECRET_KEY=" + secret_key + "\n"
            "EOF"
        )

    with subtest("services start and report healthy"):
        machine.succeed("systemctl start queryfabric-alpha.service")
        machine.succeed("systemctl start queryfabric-beta.service")
        machine.wait_for_open_port(8780)
        machine.wait_for_open_port(8781)
        machine.wait_until_succeeds("curl -sf http://127.0.0.1:8780/healthz")
        machine.wait_until_succeeds("curl -sf http://127.0.0.1:8781/healthz")

    with subtest("alpha and beta answer queries independently"):
        alpha_result = post_json(
            8780,
            "/query",
            {
                "sql": "SELECT city, pm25 FROM readings JOIN stations"
                " ON readings.station_id = stations.station_id LIMIT 5"
            },
        )
        beta_result = post_json(
            8781,
            "/query",
            {
                "sql": "SELECT city, pm25 FROM readings JOIN stations"
                " ON readings.station_id = stations.station_id LIMIT 5"
            },
        )
        assert alpha_result["rowCount"] == 5, (
            f"alpha expected 5 rows, got {alpha_result['rowCount']}"
        )
        assert beta_result["rowCount"] == 5, (
            f"beta expected 5 rows, got {beta_result['rowCount']}"
        )
        assert alpha_result["rows"][0]["city"], "alpha rows must carry a city column"
        assert beta_result["rows"][0]["city"], "beta rows must carry a city column"
        assert "SELECT" in alpha_result["backendSql"], "alpha backend SQL missing"
        assert "SELECT" in beta_result["backendSql"], "beta backend SQL missing"

    with subtest("parameterized query returns typed contract metadata"):
        parameterized = post_json(
            8780,
            "/query",
            {
                "sql": "SELECT city, pm25 FROM readings JOIN stations"
                " ON readings.station_id = stations.station_id"
                " WHERE stations.code = $1 LIMIT $2",
                "dialect": "sql",
                "expectedCatalogSnapshotId": "qfdemo-air-quality-v1",
                "requestedBackend": "postgres",
                "positional": ["lis-baixa", 5],
            },
        )
        assert parameterized["rowCount"] == 5
        assert parameterized["parameterSchema"]
        assert parameterized["resultSchema"]["fields"]
        assert parameterized["provenance"]["query_hash"]
        assert parameterized["limits"]["maxRows"] == 1000
        assert parameterized["backend"] == "postgres"
        assert parameterized["truncated"] is False
        assert post_json_status(
            8780,
            "/query",
            {"sql": "SELECT 1", "expectedCatalogSnapshotId": "stale-snapshot"},
        ) == "409"

    with subtest("federation identities are namespaced"):
        alpha_fed = get_json(8780, "/federation/status")
        beta_fed = get_json(8781, "/federation/status")
        assert alpha_fed["enabled"] is True
        assert beta_fed["enabled"] is True
        assert alpha_fed["identity"]["name"] == "queryfabric-alpha"
        assert beta_fed["identity"]["name"] == "queryfabric-beta"

    with subtest("state directories are distinct"):
        alpha_state = machine.succeed(
            "systemctl show -p StateDirectory --value queryfabric-alpha.service"
        ).strip()
        beta_state = machine.succeed(
            "systemctl show -p StateDirectory --value queryfabric-beta.service"
        ).strip()
        assert alpha_state == "queryfabric-alpha"
        assert beta_state == "queryfabric-beta"
        assert alpha_state != beta_state
        machine.succeed("test -d /var/lib/private/queryfabric-alpha")
        machine.succeed("test -d /var/lib/private/queryfabric-beta")

    with subtest("rejected query fails cleanly"):
        code = post_json_status(8780, "/query", {"sql": "SELECT * FROM secrets"})
        assert code == "400", f"unknown relation must yield 400, got {code}"

    with subtest("export bundle is produced, stored in Garage, and parses"):
        export = post(8780, "/resources/lis-baixa/export")
        assert export["contentHash"], "bundle must be content-addressed"
        assert export["storageBackend"] == "s3"
        machine.succeed("mc stat local/queryfabric-alpha/bundles/lis-baixa.json")
        machine.succeed("mc stat local/queryfabric-alpha/exports/lis-baixa/readings.csv")

        bundle = get_json(8780, "/resources/lis-baixa/bundle")
        assert bundle["exportBundle"]["version"] == "2.0"
        assert bundle["citations"]["bibtex"], "bundle must carry citations"
        assert bundle["citations"]["cff"], "bundle must carry a CFF citation"
        assert bundle["provenance"]["entries"], "bundle must embed provenance"
        assert bundle["license"]["spdxId"] == "CC-BY-4.0"
        assert bundle["artifacts"][0]["rowCount"] == 72

    with subtest("operator-mediated import dry-run and idempotent apply"):
        source_bundle = machine.succeed(
            "curl -sf http://127.0.0.1:8780/resources/lis-baixa/bundle"
        )
        source_artifact = machine.succeed(
            "mc cat local/queryfabric-alpha/exports/lis-baixa/readings.csv"
        )
        import_payload = {
            "bundle": source_bundle,
            "artifact": source_artifact,
            "expectedBundleDigest": export["contentHash"],
            "target": "lis-baixa",
        }
        dry_run = post_json(8781, "/imports/dry-run", import_payload)
        assert dry_run["mode"] == "dry-run"
        assert dry_run["rowCount"] == 72
        assert dry_run["plan"]["planDigest"].startswith("blake3-256:")
        import_payload["planDigest"] = dry_run["planDigest"]
        import_payload["stagedObject"] = dry_run["stagedObject"]

        applied = post_json(8781, "/imports/apply", import_payload)
        assert applied["mode"] == "apply"
        assert applied["replayed"] is False
        assert applied["receipt"]["event"] == "Imported"

        stale = dict(import_payload)
        stale["planDigest"] = "blake3-256:" + ("0" * 64)
        assert post_json_status(8781, "/imports/apply", stale) == "409"

        replay = post_json(8781, "/imports/apply", import_payload)
        assert replay["replayed"] is True
        assert replay["receipt"]["receiptId"] == applied["receipt"]["receiptId"]

        machine.succeed("systemctl restart queryfabric-beta.service")
        machine.wait_until_succeeds("curl -sf http://127.0.0.1:8781/healthz")
        replay_after_restart = post_json(8781, "/imports/apply", import_payload)
        assert replay_after_restart["replayed"] is True
        assert replay_after_restart["receipt"]["receiptId"] == applied["receipt"]["receiptId"]

    with subtest("GDPR access export works"):
        record = get_json(8780, "/resources/lis-baixa/access-export")
        assert record["history"]["entries"], "access export must include history"
        assert record["policy"]["license"] == "CC_BY"

    with subtest("erasure is owner-only and audited"):
        code = unauth_post_json_status(
            8780,
            "/resources/lis-baixa/erase",
            {"reason": "test"},
        )
        assert code == "401", f"missing credentials must yield 401, got {code}"

        deletion = post_json(
            8780,
            "/resources/lis-baixa/erase",
            {"reason": "user requested erasure"},
        )
        assert deletion["reason"] == "user requested erasure"

        record = get_json(8780, "/resources/lis-baixa/access-export")
        tags = {
            entry["activity"].get("activity")
            for entry in record["history"]["entries"]
        }
        assert "deleted" in tags, f"audit trail must record the erasure: {tags}"

    with subtest("DOI minting returns a registered record"):
        doi = post(8780, "/resources/lis-baixa/doi")
        assert doi["record"]["status"] == "registered"
        assert doi["record"]["doi"].startswith("10.5072/")

    with subtest("secrets are absent from the unit's store path"):
        unit_path = machine.succeed(
            "systemctl show -p FragmentPath --value queryfabric-alpha.service"
        ).strip()
        machine.fail(f"grep -q qfalpha-pg-secret {unit_path}")
        machine.fail(f"grep -q {secret_key} {unit_path}")
        env = machine.succeed("systemctl show -p Environment queryfabric-alpha.service")
        assert "qfalpha-pg-secret" not in env
        assert secret_key not in env

        unit_path = machine.succeed(
            "systemctl show -p FragmentPath --value queryfabric-beta.service"
        ).strip()
        machine.fail(f"grep -q qfbeta-pg-secret {unit_path}")
        machine.fail(f"grep -q {secret_key} {unit_path}")
        env = machine.succeed("systemctl show -p Environment queryfabric-beta.service")
        assert "qfbeta-pg-secret" not in env
        assert secret_key not in env
  '';
}
