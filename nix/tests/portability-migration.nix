# Independent alpha -> beta portability proof. Machines have separate
# PostgreSQL databases, Garage buckets, credentials, and service state.
{
  pkgs,
  nixosModule,
}:
let
  mkNode =
    {
      name,
      httpPort,
      database,
      role,
      bucket,
      seedDemoData,
    }:
    { lib, ... }:
    {
      imports = [ nixosModule ];
      virtualisation.memorySize = 2048;
      virtualisation.cores = 2;
      virtualisation.diskSize = 4096;
      services.postgresql = {
        enable = true;
        enableTCPIP = true;
      };
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
      services.queryfabric.instances.${name} = {
        listenAddress = "127.0.0.1";
        port = httpPort;
        inherit seedDemoData;
        database = {
          urlFile = "/root/${name}-db-migration-url";
          migrationUrlFile = "/root/${name}-db-migration-url";
          queryUrlFile = "/root/${name}-db-query-url";
          importUrlFile = "/root/${name}-db-import-url";
        };
        auth.secretFile = "/root/${name}-auth-secret";
        store = {
          backend = "s3";
          endpoint = "http://127.0.0.1:9000";
          inherit bucket;
          credentialsFile = "/root/${name}-store-creds";
        };
        federation.enable = false;
      };
      systemd.services."queryfabric-${name}".wantedBy = lib.mkForce [ ];
      environment.systemPackages = [
        pkgs.curl
        pkgs.minio-client
      ];
    };
in
pkgs.testers.runNixOSTest {
  name = "queryfabric-portability-migration";
  nodes.alpha = mkNode {
    name = "alpha";
    httpPort = 8780;
    database = "qfalpha";
    role = "qfalpha";
    bucket = "queryfabric-alpha";
    seedDemoData = true;
  };
  nodes.beta = mkNode {
    name = "beta";
    httpPort = 8781;
    database = "qfbeta";
    role = "qfbeta";
    bucket = "queryfabric-beta";
    seedDemoData = false;
  };
  testScript = ''
    import json

    AUTH_TOKEN = "v4.local.7YoCIGisuMEE_g46oSO_uTRGiZbR_d96apYfYQWAGzXQ07T517-vONyS7-pRrLRO7a9Uf7Or2wvHyrvDm4T2IdG98EDF91T58R_bCdGEblRVHXe0JuMp9EjereFJOEigiO6ZuwvFyUtR9DMQ3ZdxtVhFqsPQzS4qYeQ64Q3rIdVcL3hHqmfhV-_5gn_LTkX6ebRBATWsbeQBwItpw67kotTTAsOWmPE4NoCQG0vmNdF482Ml4SOSxlQVtuQ6jzcOFzpW0t6espbI7iwsQWt2Gui85b61VpCozOXamqe4IlLSmfN0nrtzMKs2yRdRsR4yrl2cRaq9FtRo6_6m29dcw2Yj-RWKfK5OFukgJK2z516DEI3fhcbJB8K5DoT_w1lEFJ2eU2sm6kr3bRgHHUybgySGWWRXaayO_AsG5yiyNdc6seRabPOBkwI"

    def json_post(machine, port, path, payload):
        body = " -d '" + json.dumps(payload) + "'"
        return json.loads(machine.succeed(
            "curl -sf -X POST http://127.0.0.1:" + str(port) + path
            + " -H 'content-type: application/json' -H 'authorization: Bearer " + AUTH_TOKEN + "'" + body
        ))

    def json_post_status(machine, port, path, payload):
        body = " -d '" + json.dumps(payload) + "'"
        return machine.succeed(
            "curl -s -o /dev/null -w '%{http_code}' -X POST http://127.0.0.1:"
            + str(port) + path + " -H 'content-type: application/json' -H 'authorization: Bearer " + AUTH_TOKEN + "'" + body
        ).strip()

    def field(output, name):
        prefix = name + ":"
        for line in output.splitlines():
            if line.startswith(prefix):
                return line.split(":", 1)[1].strip()
        raise AssertionError("missing Garage key field " + name)

    def provision(machine, name, role, database, bucket, port):
        machine.start()
        machine.wait_for_unit("postgresql.service")
        machine.wait_for_unit("garage.service")
        machine.wait_for_open_port(5432)
        machine.wait_for_open_port(9000)
        migration_role = role + "_migration"
        query_role = role + "_query"
        import_role = role + "_import"
        setup_sql = (
            "CREATE ROLE " + migration_role + " LOGIN PASSWORD '" + migration_role + "-secret';\n"
            "CREATE ROLE " + query_role + " LOGIN PASSWORD '" + query_role + "-secret';\n"
            "CREATE ROLE " + import_role + " LOGIN PASSWORD '" + import_role + "-secret';\n"
            "CREATE DATABASE " + database + " OWNER " + migration_role + ";\n"
        )
        machine.succeed("cat > /tmp/setup.sql << 'EOF'\n" + setup_sql + "EOF")
        machine.succeed('su -s /bin/sh postgres -c "psql -f /tmp/setup.sql"')
        machine.succeed(
            'su -s /bin/sh postgres -c "psql -d ' + database
            + ' -c \'GRANT CONNECT ON DATABASE ' + database + ' TO ' + query_role + ', ' + import_role + ';\'"'
        )
        machine.succeed("garage layout assign $(garage node id | cut -d@ -f1) -z qf -c 1G")
        machine.succeed("garage layout apply --version 1")
        key_output = machine.succeed("garage key create qf-" + name)
        key_id = field(key_output, "Key ID")
        secret_key = field(key_output, "Secret key")
        machine.succeed("garage bucket create " + bucket)
        machine.succeed("garage bucket allow --read --write " + bucket + " --key qf-" + name)
        machine.succeed("mc alias set local http://127.0.0.1:9000 " + key_id + " " + secret_key)
        machine.succeed(
            "install -m 600 /dev/null /root/" + name + "-db-url && "
            "echo 'postgres://" + migration_role + ":" + migration_role + "-secret@127.0.0.1:5432/" + database
            + "' > /root/" + name + "-db-migration-url"
        )
        machine.succeed(
            "install -m 600 /dev/null /root/" + name + "-db-query-url && "
            "echo 'postgres://" + query_role + ":" + query_role + "-secret@127.0.0.1:5432/" + database
            + "' > /root/" + name + "-db-query-url"
        )
        machine.succeed(
            "install -m 600 /dev/null /root/" + name + "-db-import-url && "
            "echo 'postgres://" + import_role + ":" + import_role + "-secret@127.0.0.1:5432/" + database
            + "' > /root/" + name + "-db-import-url"
        )
        machine.succeed(
            "install -m 600 /dev/null /root/" + name + "-auth-secret && "
            "printf '%s' 'qf-demo-auth-secret-2026-operator-000000' > /root/" + name + "-auth-secret"
        )
        machine.succeed(
            "install -m 600 /dev/null /root/" + name + "-store-creds && "
            "printf 'QFDEMO_STORE_ACCESS_KEY=%s\\nQFDEMO_STORE_SECRET_KEY=%s\\n' '"
            + key_id + "' '" + secret_key + "' > /root/" + name + "-store-creds"
        )
        machine.succeed("systemctl start queryfabric-" + name + ".service")
        machine.wait_for_open_port(port)
        machine.wait_until_succeeds("curl -sf http://127.0.0.1:" + str(port) + "/healthz")
        grants_sql = (
            "GRANT CONNECT ON DATABASE " + database + " TO " + query_role + ", " + import_role + ";\n"
            "GRANT USAGE ON SCHEMA public TO " + query_role + ", " + import_role + ";\n"
            "GRANT SELECT ON stations, readings, queryfabric_import_receipts TO " + query_role + ";\n"
            "GRANT SELECT, INSERT ON readings, queryfabric_import_receipts TO " + import_role + ";\n"
        )
        machine.succeed("cat > /tmp/grants.sql << 'EOF'\n" + grants_sql + "EOF")
        machine.succeed('su -s /bin/sh postgres -c "psql -d ' + database + ' -f /tmp/grants.sql"')
        machine.fail(
            "su -s /bin/sh postgres -c \"PGPASSWORD='" + query_role + "-secret' psql -h 127.0.0.1 -U "
            + query_role + " -d " + database + " -c 'INSERT INTO readings SELECT * FROM readings LIMIT 0'\""
        )
        machine.fail(
            "su -s /bin/sh postgres -c \"PGPASSWORD='" + import_role + "-secret' psql -h 127.0.0.1 -U "
            + import_role + " -d " + database + " -c 'CREATE TABLE queryfabric_forbidden_ddl(id integer)'\""
        )

    provision(alpha, "alpha", "qfalpha", "qfalpha", "queryfabric-alpha", 8780)
    provision(beta, "beta", "qfbeta", "qfbeta", "queryfabric-beta", 8781)

    with subtest("export, transfer, dry-run, apply, and replay"):
        export = json_post(alpha, 8780, "/resources/lis-baixa/export", {})
        bundle = alpha.succeed("curl -sf http://127.0.0.1:8780/resources/lis-baixa/bundle")
        artifact = alpha.succeed("mc cat local/queryfabric-alpha/exports/lis-baixa/readings.csv")
        payload = {
            "bundle": bundle,
            "artifact": artifact,
            "expectedBundleDigest": export["contentHash"],
            "target": "lis-baixa",
        }
        dry_run = json_post(beta, 8781, "/imports/dry-run", payload)
        assert dry_run["rowCount"] == 72
        payload["planDigest"] = dry_run["planDigest"]
        payload["stagedObject"] = dry_run["stagedObject"]
        applied = json_post(beta, 8781, "/imports/apply", payload)
        assert applied["replayed"] is False
        stale = dict(payload)
        stale["planDigest"] = "blake3-256:" + ("0" * 64)
        assert json_post_status(beta, 8781, "/imports/apply", stale) == "409"
        replay = json_post(beta, 8781, "/imports/apply", payload)
        assert replay["replayed"] is True
        assert replay["receipt"]["receiptId"] == applied["receipt"]["receiptId"]
        beta.succeed("systemctl restart queryfabric-beta.service")
        beta.wait_until_succeeds("curl -sf http://127.0.0.1:8781/healthz")
        after_restart = json_post(beta, 8781, "/imports/apply", payload)
        assert after_restart["replayed"] is True

    with subtest("tampered artifact is rejected before apply"):
        tampered = dict(payload)
        tampered["artifact"] = tampered["artifact"].replace("11.0", "11.1", 1)
        assert json_post_status(beta, 8781, "/imports/dry-run", tampered) == "400"
  '';
}
