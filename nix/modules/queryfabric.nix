# NixOS module for the QueryFabric self-host demonstrator.
#
# Secrets never enter the Nix store: the database URL and the S3
# credentials arrive as root-readable files handed to the service through
# systemd LoadCredential, and the service reads them from
# $CREDENTIALS_DIRECTORY at runtime.
{
  config,
  lib,
  pkgs,
  ...
}: let
  cfg = config.services.queryfabric;

  credentials =
    lib.optional (cfg.database.urlFile != null) "db-url:${cfg.database.urlFile}"
    ++ lib.optional (cfg.store.credentialsFile != null) "store-creds:${cfg.store.credentialsFile}";
in {
  options.services.queryfabric = {
    enable = lib.mkEnableOption "the QueryFabric self-host demonstrator service";

    package = lib.mkOption {
      type = lib.types.package;
      description = "queryfabric-demo package to run.";
    };

    listenAddress = lib.mkOption {
      type = lib.types.str;
      default = "127.0.0.1";
      description = "Address the HTTP API binds to.";
    };

    port = lib.mkOption {
      type = lib.types.port;
      default = 8780;
      description = "Port the HTTP API binds to.";
    };

    publicBaseUrl = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      example = "https://data.example.org";
      description = ''
        External base URL used in citations and DOI landing pages.
        Defaults to the listen address.
      '';
    };

    logLevel = lib.mkOption {
      type = lib.types.str;
      default = "info";
      example = "queryfabric_demo=debug,info";
      description = "RUST_LOG filter for the service.";
    };

    database = {
      url = lib.mkOption {
        type = lib.types.nullOr lib.types.str;
        default = null;
        example = "postgres://queryfabric@/queryfabric?host=/run/postgresql";
        description = ''
          Postgres connection URL. Only safe for URLs without a password
          (for example local socket auth) — anything set here lands in the
          world-readable Nix store. Use {option}`database.urlFile` for URLs
          carrying credentials.
        '';
      };

      urlFile = lib.mkOption {
        type = lib.types.nullOr lib.types.path;
        default = null;
        example = "/run/secrets/queryfabric-db-url";
        description = ''
          File containing the Postgres connection URL, loaded via systemd
          `LoadCredential`. Takes precedence over {option}`database.url`.
        '';
      };
    };

    store = {
      backend = lib.mkOption {
        type = lib.types.enum ["memory" "s3"];
        default = "memory";
        description = ''
          Object-store backend for export bundles and artifacts. `memory`
          is non-durable and only suitable for evaluation; `s3` works with
          any S3-compatible service (AWS S3, MinIO, Garage).
        '';
      };

      endpoint = lib.mkOption {
        type = lib.types.nullOr lib.types.str;
        default = null;
        example = "http://127.0.0.1:9000";
        description = "S3 endpoint URL (required for the s3 backend).";
      };

      bucket = lib.mkOption {
        type = lib.types.nullOr lib.types.str;
        default = null;
        example = "queryfabric";
        description = "S3 bucket name (required for the s3 backend).";
      };

      region = lib.mkOption {
        type = lib.types.str;
        default = "us-east-1";
        description = "S3 region; MinIO and Garage accept the default.";
      };

      credentialsFile = lib.mkOption {
        type = lib.types.nullOr lib.types.path;
        default = null;
        example = "/run/secrets/queryfabric-store-creds";
        description = ''
          File with `QFDEMO_STORE_ACCESS_KEY=...` and
          `QFDEMO_STORE_SECRET_KEY=...` lines, loaded via systemd
          `LoadCredential` (required for the s3 backend).
        '';
      };
    };

    federation = {
      enable = lib.mkEnableOption "announcing a federation node identity";

      nodeName = lib.mkOption {
        type = lib.types.str;
        default = "queryfabric-demo";
        description = "Name this node announces to a federation.";
      };

      hubMultiaddrs = lib.mkOption {
        type = lib.types.listOf lib.types.str;
        default = [];
        example = ["/dns4/hub.example.org/tcp/4001"];
        description = "Multiaddrs of federation hubs to announce.";
      };

      flightPort = lib.mkOption {
        type = lib.types.port;
        default = 50051;
        description = "Arrow Flight port announced to federation peers.";
      };
    };

    openFirewall = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = "Open the HTTP API port in the firewall.";
    };
  };

  config = lib.mkIf cfg.enable {
    assertions = [
      {
        assertion = cfg.database.url != null || cfg.database.urlFile != null;
        message = "services.queryfabric needs database.url or database.urlFile.";
      }
      {
        assertion =
          cfg.store.backend != "s3"
          || (cfg.store.endpoint != null && cfg.store.bucket != null && cfg.store.credentialsFile != null);
        message = "services.queryfabric: store.backend = \"s3\" needs store.endpoint, store.bucket, and store.credentialsFile.";
      }
    ];

    networking.firewall.allowedTCPPorts = lib.mkIf cfg.openFirewall [cfg.port];

    systemd.services.queryfabric = {
      description = "QueryFabric self-host demonstrator";
      wantedBy = ["multi-user.target"];
      after = ["network-online.target" "postgresql.service" "minio.service"];
      wants = ["network-online.target"];

      environment =
        {
          RUST_LOG = cfg.logLevel;
          QFDEMO_LISTEN_ADDR = "${cfg.listenAddress}:${toString cfg.port}";
          QFDEMO_STORE_BACKEND = cfg.store.backend;
          QFDEMO_FEDERATION_ENABLE =
            if cfg.federation.enable
            then "true"
            else "false";
          QFDEMO_FEDERATION_NODE_NAME = cfg.federation.nodeName;
          QFDEMO_FEDERATION_FLIGHT_PORT = toString cfg.federation.flightPort;
        }
        // lib.optionalAttrs (cfg.publicBaseUrl != null) {
          QFDEMO_PUBLIC_BASE_URL = cfg.publicBaseUrl;
        }
        // lib.optionalAttrs (cfg.database.urlFile != null) {
          QFDEMO_DATABASE_URL_FILE = "%d/db-url";
        }
        // lib.optionalAttrs (cfg.database.urlFile == null && cfg.database.url != null) {
          QFDEMO_DATABASE_URL = cfg.database.url;
        }
        // lib.optionalAttrs (cfg.store.backend == "s3") {
          QFDEMO_STORE_ENDPOINT = cfg.store.endpoint;
          QFDEMO_STORE_BUCKET = cfg.store.bucket;
          QFDEMO_STORE_REGION = cfg.store.region;
          QFDEMO_STORE_CREDENTIALS_FILE = "%d/store-creds";
        }
        // lib.optionalAttrs (cfg.federation.hubMultiaddrs != []) {
          QFDEMO_FEDERATION_HUB_MULTIADDRS = lib.concatStringsSep "," cfg.federation.hubMultiaddrs;
        };

      serviceConfig = {
        ExecStart = lib.getExe cfg.package;
        LoadCredential = credentials;
        Restart = "on-failure";
        RestartSec = 2;

        # Hardening: no privileges, no persistent state, minimal kernel
        # surface. The service only needs outbound TCP and its listen
        # socket.
        DynamicUser = true;
        NoNewPrivileges = true;
        ProtectSystem = "strict";
        ProtectHome = true;
        PrivateTmp = true;
        PrivateDevices = true;
        ProtectClock = true;
        ProtectControlGroups = true;
        ProtectKernelLogs = true;
        ProtectKernelModules = true;
        ProtectKernelTunables = true;
        ProtectProc = "invisible";
        ProcSubset = "pid";
        RestrictAddressFamilies = ["AF_INET" "AF_INET6" "AF_UNIX"];
        RestrictNamespaces = true;
        RestrictRealtime = true;
        RestrictSUIDSGID = true;
        LockPersonality = true;
        MemoryDenyWriteExecute = true;
        SystemCallArchitectures = "native";
        SystemCallFilter = ["@system-service" "~@privileged"];
        CapabilityBoundingSet = "";
        AmbientCapabilities = "";
        UMask = "0077";
      };
    };
  };
}
