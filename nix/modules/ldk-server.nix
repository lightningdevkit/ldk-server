{
  pkgs,
  config,
  lib,
  ...
}:
let
  cfg = config.services.ldk-server;
  format = pkgs.formats.toml { };
  configFile = format.generate "ldk-server.toml" cfg;

  ldk-server-cli = pkgs.writeShellScriptBin "ldk-server-cli" ''
    ${lib.getExe pkgs.local.ldk-server-cli}  -b ${cfg.node.rest_service_address} $1
  '';
in
{
  options.services.ldk-server = {
    enable = lib.mkEnableOption "LDK Server";
    node = {
      network = lib.mkOption {
        type = lib.types.str;
        default = "regtest";
      };
      listening_address = lib.mkOption {
        type = lib.types.str;
        default = "127.0.0.1:3001";
      };
      rest_service_address = lib.mkOption {
        type = lib.types.str;
        default = "127.0.0.1:3002";
      };
    };
    storage.disk.dir_path = lib.mkOption {
      type = lib.types.str;
      default = "/var/lib/ldk-server";
    };
    bitcoind = {
      rpc_address = lib.mkOption {
        type = lib.types.str;
        default = "127.0.0.1:18444";
      };
      rpc_user = lib.mkOption {
        type = lib.types.str;
        default = "polaruser";
      };
      rpc_password = lib.mkOption {
        type = lib.types.str;
        default = "polarpass";
      };
    };
    rabbitmq = {
      connection_string = lib.mkOption {
        type = lib.types.str;
        default = "";
      };
      exchange_name = lib.mkOption {
        type = lib.types.str;
        default = "";
      };
    };

    liquidity = {
      lsps2_service = {
        advertise_service = lib.mkOption {
          type = lib.types.bool;
          default = false;
        };
        channel_opening_fee_ppm = lib.mkOption {
          type = lib.types.int;
          default = 1000;
        };
        channel_over_provisioning_ppm = lib.mkOption {
          type = lib.types.int;
          default = 500000;
        };
        min_channel_opening_fee_msat = lib.mkOption {
          type = lib.types.int;
          default = 10000000;
        };
        min_channel_lifetime = lib.mkOption {
          type = lib.types.int;
          default = 8320;
        };
        max_client_to_self_delay = lib.mkOption {
          type = lib.types.int;
          default = 1440;
        };
        min_payment_size_msat = lib.mkOption {
          type = lib.types.int;
          default = 10000000;
        };
        max_payment_size_msat = lib.mkOption {
          type = lib.types.int;
          default = 25000000000;
        };
      };
    };
  };

  config = lib.mkIf cfg.enable {
    users.users.ldk-server = {
      isSystemUser = true;
      group = "ldk-server";
    };
    users.groups.ldk-server = { };

    systemd.services.ldk-server = {
      wantedBy = [ "multi-user.target" ];
      after = [ "network.target" ];
      serviceConfig = {
        ExecStart = "${lib.getExe pkgs.local.ldk-server} ${configFile}";
        User = "ldk-server";
        Group = "ldk-server";
        StateDirectory = "ldk-server";

        UMask = "0640";
        Restart = "on-failure";
        RestartSec = "10";
      };
    };
    environment.systemPackages = [
      ldk-server-cli
    ];
  };
}
