{ pkgs, ldkServerModule }:

let
  fakeServer = pkgs.writeShellApplication {
    name = "ldk-server";
    runtimeInputs = [
      pkgs.coreutils
      pkgs.systemd
    ];
    text = ''
      config_file="$1"
      test "$2" = "--storage-dir-path"
      data_dir="$3"

      cp --remove-destination "$config_file" "$data_dir/observed-config.toml"
      printf '%s\n' "$@" > "$data_dir/observed-arguments"
      exec systemd-notify --ready --exec ';' sleep infinity
    '';
  };
in
{
  name = "ldk-server-module";

  nodes.machine = {
    imports = [ ldkServerModule ];

    services.ldk-server = {
      package = fakeServer;
      instances = {
        mainnet = {
          enable = true;
          environmentFiles = [ "-/run/secrets/ldk-server-mainnet" ];
          settings.node.network = "bitcoin";
        };
        signet = {
          enable = true;
          dataDir = "/var/lib/ldk-server-signet-test";
          settings.node.network = "signet";
        };
      };
    };
  };

  testScript = ''
    machine.wait_for_unit("ldk-server-mainnet.service")
    machine.wait_for_unit("ldk-server-signet.service")

    machine.succeed(
      "test $(stat -c '%U:%G:%a' /var/lib/ldk-server/mainnet) = "
      "ldk-server-mainnet:ldk-server-mainnet:750"
    )
    machine.succeed(
      "test $(stat -c '%U:%G:%a' /var/lib/ldk-server-signet-test) = "
      "ldk-server-signet:ldk-server-signet:750"
    )

    machine.succeed(
      "grep -F 'network = \"bitcoin\"' "
      "/var/lib/ldk-server/mainnet/observed-config.toml"
    )
    machine.succeed(
      "grep -F 'dir_path = \"/var/lib/ldk-server/mainnet\"' "
      "/var/lib/ldk-server/mainnet/observed-config.toml"
    )
    machine.succeed(
      "grep -F 'network = \"signet\"' "
      "/var/lib/ldk-server-signet-test/observed-config.toml"
    )
    machine.succeed(
      "grep -F 'dir_path = \"/var/lib/ldk-server-signet-test\"' "
      "/var/lib/ldk-server-signet-test/observed-config.toml"
    )

    machine.succeed("systemctl stop ldk-server-mainnet.service")
    machine.fail("systemctl is-active --quiet ldk-server-mainnet.service")
    machine.succeed("systemctl is-active --quiet ldk-server-signet.service")
  '';
}
