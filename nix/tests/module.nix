{ pkgs, nixosModule }:
let
  fakePackage = pkgs.writeShellScriptBin "queryfabric-module-test" ''
    set -eu
    test "$QFDEMO_STORE_BACKEND" = memory
    test -n "$QFDEMO_DATABASE_URL"
    test -n "$QFDEMO_AUTH_SECRET"
    trap 'exit 0' TERM INT
    while :; do sleep 3600; done
  '';

  instance = port: flightPort: {
    enable = true;
    package = fakePackage;
    inherit port;
    database.url = "postgres://queryfabric@/queryfabric?host=/run/postgresql";
    auth.secret = "qf-module-test-secret";
    store.backend = "memory";
    federation.enable = false;
    federation.flightPort = flightPort;
  };
in
pkgs.testers.runNixOSTest {
  name = "queryfabric-module";

  nodes.machine = {
    imports = [ nixosModule ];
    services.queryfabric.instances = {
      alpha = instance 8780 50051;
      beta = instance 8781 50052;
    };
  };

  testScript = ''
    machine.start()
    machine.succeed("systemctl start queryfabric-alpha.service queryfabric-beta.service")
    machine.wait_until_succeeds("systemctl is-active queryfabric-alpha.service")
    machine.wait_until_succeeds("systemctl is-active queryfabric-beta.service")
    machine.succeed("systemctl show queryfabric-alpha.service -p DynamicUser --value | grep -qx yes")
    machine.succeed("systemctl show queryfabric-alpha.service -p NoNewPrivileges --value | grep -qx yes")
    machine.succeed("systemctl show queryfabric-beta.service -p DynamicUser --value | grep -qx yes")
  '';
}
