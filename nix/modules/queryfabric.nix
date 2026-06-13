{
  defaultPackage ? null,
}:
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.services.queryfabric;

  mkMaybeDefault = value: if value == null then { } else { default = value; };

  mkCommonOptions =
    {
      enableDefault,
      enableDescription,
      packageDefault ? null,
    }:
    {
      enable = lib.mkOption {
        type = lib.types.bool;
        default = enableDefault;
        description = enableDescription;
      };

      package = lib.mkOption (
        {
          type = lib.types.package;
          description = "queryfabric-demo package to run.";
        }
        // mkMaybeDefault packageDefault
      );

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
          type = lib.types.enum [
            "memory"
            "s3"
          ];
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
          default = [ ];
          example = [ "/dns4/hub.example.org/tcp/4001" ];
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

  instanceModule =
    { ... }:
    {
      options = mkCommonOptions {
        enableDefault = true;
        enableDescription = "Enable this QueryFabric instance.";
        packageDefault = defaultPackage;
      };
    };

  legacyInstance = {
    inherit (cfg)
      package
      listenAddress
      port
      publicBaseUrl
      logLevel
      openFirewall
      ;
    database = cfg.database;
    store = cfg.store;
    federation = cfg.federation;
  };

  configuredInstances =
    cfg.instances
    // lib.optionalAttrs cfg.enable {
      default = legacyInstance;
    };

  enabledInstances = lib.filterAttrs (_: instanceCfg: instanceCfg.enable) configuredInstances;
  namedInstances = if cfg.enable then lib.removeAttrs cfg.instances [ "default" ] else cfg.instances;
  enabledNamedInstances = lib.filterAttrs (_: instanceCfg: instanceCfg.enable) namedInstances;
  legacyEnabled = cfg.enable;

  unitName = name: if name == "default" then "queryfabric" else "queryfabric-${name}";

  stateDirectoryName = name: "queryfabric-${name}";

  credentialsFor =
    instanceCfg:
    lib.optional (instanceCfg.database.urlFile != null) "db-url:${instanceCfg.database.urlFile}"
    ++ lib.optional (
      instanceCfg.store.credentialsFile != null
    ) "store-creds:${instanceCfg.store.credentialsFile}";

  mkUnit = name: instanceCfg: {
    ${unitName name} = {
      description = "QueryFabric self-host demonstrator";
      wantedBy = [ "multi-user.target" ];
      after = [
        "network-online.target"
        "postgresql.service"
        "minio.service"
      ];
      wants = [ "network-online.target" ];

      environment = {
        RUST_LOG = instanceCfg.logLevel;
        QFDEMO_LISTEN_ADDR = "${instanceCfg.listenAddress}:${toString instanceCfg.port}";
        QFDEMO_STORE_BACKEND = instanceCfg.store.backend;
        QFDEMO_FEDERATION_ENABLE = if instanceCfg.federation.enable then "true" else "false";
        QFDEMO_FEDERATION_NODE_NAME = instanceCfg.federation.nodeName;
        QFDEMO_FEDERATION_FLIGHT_PORT = toString instanceCfg.federation.flightPort;
      }
      // lib.optionalAttrs (instanceCfg.publicBaseUrl != null) {
        QFDEMO_PUBLIC_BASE_URL = instanceCfg.publicBaseUrl;
      }
      // lib.optionalAttrs (instanceCfg.database.urlFile != null) {
        QFDEMO_DATABASE_URL_FILE = "%d/db-url";
      }
      // lib.optionalAttrs (instanceCfg.database.urlFile == null && instanceCfg.database.url != null) {
        QFDEMO_DATABASE_URL = instanceCfg.database.url;
      }
      // lib.optionalAttrs (instanceCfg.store.backend == "s3") {
        QFDEMO_STORE_ENDPOINT = instanceCfg.store.endpoint;
        QFDEMO_STORE_BUCKET = instanceCfg.store.bucket;
        QFDEMO_STORE_REGION = instanceCfg.store.region;
        QFDEMO_STORE_CREDENTIALS_FILE = "%d/store-creds";
      }
      // lib.optionalAttrs (instanceCfg.federation.hubMultiaddrs != [ ]) {
        QFDEMO_FEDERATION_HUB_MULTIADDRS = lib.concatStringsSep "," instanceCfg.federation.hubMultiaddrs;
      };

      serviceConfig = {
        ExecStart = lib.getExe instanceCfg.package;
        LoadCredential = credentialsFor instanceCfg;
        Restart = "on-failure";
        RestartSec = 2;
        StateDirectory = stateDirectoryName name;

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
        RestrictAddressFamilies = [
          "AF_INET"
          "AF_INET6"
          "AF_UNIX"
        ];
        RestrictNamespaces = true;
        RestrictRealtime = true;
        RestrictSUIDSGID = true;
        LockPersonality = true;
        MemoryDenyWriteExecute = true;
        SystemCallArchitectures = "native";
        SystemCallFilter = [
          "@system-service"
          "~@privileged"
        ];
        CapabilityBoundingSet = "";
        AmbientCapabilities = "";
        UMask = "0077";
      };
    };
  };

  openFirewallPorts = lib.unique (
    lib.concatMap (instanceCfg: lib.optional instanceCfg.openFirewall instanceCfg.port) (
      lib.attrValues enabledInstances
    )
  );

  listenPorts = map (instanceCfg: instanceCfg.port) (lib.attrValues enabledInstances);
  flightPorts = map (instanceCfg: instanceCfg.federation.flightPort) (
    lib.attrValues enabledInstances
  );
  federationNodeNames = map (instanceCfg: instanceCfg.federation.nodeName) (
    lib.filter (instanceCfg: instanceCfg.federation.enable) (lib.attrValues enabledInstances)
  );

  hasUniqueCount = values: builtins.length values == builtins.length (lib.unique values);

  mkInstanceAssertions = optionPath: instanceCfg: [
    {
      assertion = instanceCfg.database.url != null || instanceCfg.database.urlFile != null;
      message = "${optionPath} needs database.url or database.urlFile.";
    }
    {
      assertion =
        instanceCfg.store.backend != "s3"
        || (
          instanceCfg.store.endpoint != null
          && instanceCfg.store.bucket != null
          && instanceCfg.store.credentialsFile != null
        );
      message = "${optionPath}: store.backend = \"s3\" needs store.endpoint, store.bucket, and store.credentialsFile.";
    }
  ];
in
{
  options.services.queryfabric =
    mkCommonOptions {
      enableDefault = false;
      enableDescription = "Enable the legacy default QueryFabric instance.";
    }
    // {
      instances = lib.mkOption {
        type = lib.types.attrsOf (lib.types.submodule instanceModule);
        default = { };
        description = "Named QueryFabric instances to deploy on this host.";
      };
    };

  config = {
    assertions = [
      {
        assertion = !(cfg.enable && cfg.instances ? default);
        message = "services.queryfabric.enable and services.queryfabric.instances.default are mutually exclusive.";
      }
    ]
    ++ lib.optionals legacyEnabled (mkInstanceAssertions "services.queryfabric" legacyInstance)
    ++ lib.concatLists (
      lib.mapAttrsToList (
        name: instanceCfg: mkInstanceAssertions "services.queryfabric.instances.${name}" instanceCfg
      ) enabledNamedInstances
    )
    ++ [
      {
        assertion = hasUniqueCount listenPorts;
        message = "services.queryfabric instance listen ports must be unique.";
      }
      {
        assertion = hasUniqueCount flightPorts;
        message = "services.queryfabric federation.flightPort values must be unique across enabled instances.";
      }
      {
        assertion = hasUniqueCount federationNodeNames;
        message = "services.queryfabric federation.enable instances must use unique federation.nodeName values.";
      }
    ];

    networking.firewall.allowedTCPPorts = openFirewallPorts;

    systemd.services = lib.mkMerge (lib.mapAttrsToList mkUnit enabledInstances);
  };
}
