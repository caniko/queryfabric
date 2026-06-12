# End-to-end self-host test: one NixOS VM running Postgres + MinIO + the
# queryfabric demonstrator through the NixOS module. Secrets are created at
# VM runtime and handed to the service via LoadCredential; the test asserts
# they never appear in the unit's store path.
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

      services.minio = {
        enable = true;
        # Test-harness credentials for the VM's MinIO; the assertion below
        # only concerns the queryfabric unit, whose secrets are runtime files.
        rootCredentialsFile = pkgs.writeText "minio-root-credentials" ''
          MINIO_ROOT_USER=qfminio
          MINIO_ROOT_PASSWORD=qfminio-secret-key
        '';
      };

      services.queryfabric = {
        enable = true;
        listenAddress = "127.0.0.1";
        port = 8780;
        database.urlFile = "/root/qfdemo-db-url";
        store = {
          backend = "s3";
          endpoint = "http://127.0.0.1:9000";
          bucket = "queryfabric";
          credentialsFile = "/root/qfdemo-store-creds";
        };
        federation.enable = true;
      };

      # Secrets are written by the test script after boot; do not start the
      # service before they exist.
      systemd.services.queryfabric.wantedBy = lib.mkForce [ ];

      environment.systemPackages = [
        pkgs.curl
        pkgs.minio-client
      ];
    };

  testScript = ''
    import json

    BASE = "http://127.0.0.1:8780"
    JSON_HEADER = " -H 'content-type: application/json'"


    def get_json(path):
        return json.loads(machine.succeed("curl -sf " + BASE + path))


    def post(path):
        return json.loads(machine.succeed("curl -sf -X POST " + BASE + path))


    def post_json(path, payload):
        body = " -d '" + json.dumps(payload) + "'"
        return json.loads(
            machine.succeed(
                "curl -sf -X POST " + BASE + path + JSON_HEADER + body
            )
        )


    def post_json_status(path, payload):
        body = " -d '" + json.dumps(payload) + "'"
        return machine.succeed(
            "curl -s -o /dev/null -w '%{http_code}' -X POST "
            + BASE + path + JSON_HEADER + body
        ).strip()


    machine.start()
    machine.wait_for_unit("postgresql.service")
    machine.wait_for_unit("minio.service")
    machine.wait_for_open_port(5432)
    machine.wait_for_open_port(9000)

    with subtest("provision database, bucket, and runtime secrets"):
        setup_sql = (
            "CREATE ROLE qfdemo LOGIN PASSWORD 'qfdemo-pg-secret';\n"
            "CREATE DATABASE qfdemo OWNER qfdemo;\n"
        )
        machine.succeed("cat > /tmp/setup.sql << 'EOF'\n" + setup_sql + "EOF")
        machine.succeed('su -s /bin/sh postgres -c "psql -f /tmp/setup.sql"')
        machine.succeed(
            "mc alias set local http://127.0.0.1:9000 qfminio qfminio-secret-key"
        )
        machine.succeed("mc mb local/queryfabric")
        machine.succeed(
            "install -m 600 /dev/null /root/qfdemo-db-url && "
            "echo 'postgres://qfdemo:qfdemo-pg-secret@127.0.0.1:5432/qfdemo'"
            " > /root/qfdemo-db-url"
        )
        store_creds = (
            "QFDEMO_STORE_ACCESS_KEY=qfminio\n"
            "QFDEMO_STORE_SECRET_KEY=qfminio-secret-key\n"
        )
        machine.succeed(
            "install -m 600 /dev/null /root/qfdemo-store-creds && "
            "cat > /root/qfdemo-store-creds << 'EOF'\n" + store_creds + "EOF"
        )

    with subtest("service starts and reports healthy"):
        machine.succeed("systemctl start queryfabric.service")
        machine.wait_for_open_port(8780)
        machine.wait_until_succeeds("curl -sf " + BASE + "/healthz")

    with subtest("portable query returns rows"):
        result = post_json(
            "/query",
            {
                "sql": "SELECT city, pm25 FROM readings JOIN stations"
                " ON readings.station_id = stations.station_id LIMIT 5"
            },
        )
        assert result["rowCount"] == 5, f"expected 5 rows, got {result['rowCount']}"
        assert result["rows"][0]["city"], "rows must carry a city column"
        assert "SELECT" in result["backendSql"], "backend SQL missing"

    with subtest("rejected query fails cleanly"):
        code = post_json_status("/query", {"sql": "SELECT * FROM secrets"})
        assert code == "400", f"unknown relation must yield 400, got {code}"

    with subtest("export bundle is produced, stored in MinIO, and parses"):
        export = post("/resources/lis-baixa/export")
        assert export["contentHash"], "bundle must be content-addressed"
        assert export["storageBackend"] == "s3"
        machine.succeed("mc stat local/queryfabric/bundles/lis-baixa.json")
        machine.succeed("mc stat local/queryfabric/exports/lis-baixa/readings.csv")

        bundle = get_json("/resources/lis-baixa/bundle")
        assert bundle["exportBundle"]["version"] == "1.0"
        assert bundle["citations"]["bibtex"], "bundle must carry citations"
        assert bundle["citations"]["cff"], "bundle must carry a CFF citation"
        assert bundle["provenance"]["entries"], "bundle must embed provenance"
        assert bundle["license"]["spdxId"] == "CC-BY-4.0"
        assert bundle["artifacts"][0]["rowCount"] == 72

    with subtest("GDPR access export works"):
        record = get_json("/resources/lis-baixa/access-export")
        assert record["history"]["entries"], "access export must include history"
        assert record["policy"]["license"] == "CC_BY"

    with subtest("erasure is owner-only and audited"):
        code = post_json_status(
            "/resources/lis-baixa/erase",
            {
                "reason": "test",
                "subject": "00000000-0000-0000-0000-000000000001",
            },
        )
        assert code == "403", f"non-owner erasure must yield 403, got {code}"

        deletion = post_json(
            "/resources/lis-baixa/erase",
            {"reason": "user requested erasure"},
        )
        assert deletion["reason"] == "user requested erasure"

        record = get_json("/resources/lis-baixa/access-export")
        tags = {
            entry["activity"].get("activity")
            for entry in record["history"]["entries"]
        }
        assert "deleted" in tags, f"audit trail must record the erasure: {tags}"

    with subtest("DOI minting returns a registered record"):
        doi = post("/resources/lis-baixa/doi")
        assert doi["record"]["status"] == "registered"
        assert doi["record"]["doi"].startswith("10.5072/")

    with subtest("federation identity is announced"):
        fed = get_json("/federation/status")
        assert fed["enabled"] is True
        assert fed["identity"]["name"] == "queryfabric-demo"

    with subtest("secrets are absent from the unit's store path"):
        unit_path = machine.succeed(
            "systemctl show -p FragmentPath --value queryfabric.service"
        ).strip()
        machine.fail(f"grep -q qfdemo-pg-secret {unit_path}")
        machine.fail(f"grep -q qfdemo-secret-key {unit_path}")
        env = machine.succeed("systemctl show -p Environment queryfabric.service")
        assert "qfdemo-pg-secret" not in env
        assert "qfdemo-secret-key" not in env
  '';
}
