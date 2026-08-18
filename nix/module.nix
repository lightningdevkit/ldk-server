{
  config,
  lib,
  pkgs,
  ...
}:

let
  inherit (lib) mkOption types;
  cfg = config.services.ldk-server;
  toml = pkgs.formats.toml { };

  isAbsolutePath = path: lib.hasPrefix "/" path;
  isAbsoluteEnvironmentFile = path: isAbsolutePath (lib.removePrefix "-" path);

  instanceType = types.submodule (
    { name, ... }:
    {
      options = {
        enable = mkOption {
          type = types.bool;
          default = false;
          description = "Start this LDK Server instance.";
        };

        settings = mkOption {
          inherit (toml) type;
          default = { };
          example = lib.literalExpression ''
            {
              node = {
                network = "bitcoin";
                listening_addresses = [ "0.0.0.0:9735" ];
              };
              esplora.server_url = "https://mempool.space/api";
            }
          '';
          description = ''
            Settings for the generated TOML file. Do not put secrets in this
            option because the Nix store is readable by all local users.
          '';
        };

        configFile = mkOption {
          type = types.nullOr types.str;
          default = null;
          description = ''
            An existing TOML configuration file with an absolute path. This
            option and settings cannot be used together.
          '';
        };

        environmentFiles = mkOption {
          type = types.listOf types.str;
          default = [ ];
          example = [ "/run/secrets/ldk-server-${name}" ];
          description = ''
            Files with environment variables for secrets and setting
            overrides. Each file must have an absolute path and use systemd
            EnvironmentFile syntax. Prefix a path with - to make it optional.
          '';
        };

        dataDir = mkOption {
          type = types.str;
          default = "/var/lib/ldk-server/${name}";
          description = ''
            The absolute path of the directory that stores data for this
            instance.
          '';
        };

        user = mkOption {
          type = types.str;
          default = "ldk-server-${name}";
          description = "The user for this instance.";
        };

        group = mkOption {
          type = types.str;
          default = "ldk-server-${name}";
          description = "The group for this instance.";
        };

        openFirewall = mkOption {
          type = types.bool;
          default = false;
          description = "Open the selected ports for this instance.";
        };

        lightningPort = mkOption {
          type = types.nullOr types.port;
          default = null;
          description = ''
            The Lightning port to open. This port must match the instance
            configuration.
          '';
        };

        grpcPort = mkOption {
          type = types.nullOr types.port;
          default = null;
          description = ''
            The gRPC port to open. This port must match the instance
            configuration.
          '';
        };
      };
    }
  );

  enabledInstances = lib.filterAttrs (_: instance: instance.enable) cfg.instances;

  generatedConfigs = lib.mapAttrs (
    name: instance:
    toml.generate "ldk-server-${name}.toml" (
      lib.recursiveUpdate instance.settings {
        storage.disk.dir_path = instance.dataDir;
      }
    )
  ) enabledInstances;

  instanceConfigPath =
    name: instance:
    if instance.configFile == null then generatedConfigs.${name} else instance.configFile;

  defaultUsers = lib.filterAttrs (
    name: instance: instance.user == "ldk-server-${name}"
  ) enabledInstances;
  defaultGroups = lib.filterAttrs (
    name: instance: instance.group == "ldk-server-${name}"
  ) enabledInstances;
in
{
  options.services.ldk-server = {
    package = mkOption {
      type = types.package;
      default = pkgs.callPackage ./package.nix { };
      defaultText = lib.literalExpression "pkgs.callPackage ./nix/package.nix { }";
      description = "The LDK Server package to run.";
    };

    instances = mkOption {
      type = types.attrsOf instanceType;
      default = { };
      description = "Named LDK Server instances.";
    };
  };

  config = {
    assertions =
      lib.concatLists (
        lib.mapAttrsToList (name: instance: [
          {
            assertion = builtins.match "^[a-zA-Z0-9_-]+$" name != null;
            message = "LDK Server instance name '${name}' contains invalid characters";
          }
          {
            assertion = instance.configFile == null || instance.settings == { };
            message = ''
              services.ldk-server.instances.${name}.configFile and settings
              cannot be used together
            '';
          }
          {
            assertion = isAbsolutePath instance.dataDir;
            message = ''
              services.ldk-server.instances.${name}.dataDir must be an
              absolute path
            '';
          }
          {
            assertion = instance.configFile == null || isAbsolutePath instance.configFile;
            message = ''
              services.ldk-server.instances.${name}.configFile must be an
              absolute path
            '';
          }
          {
            assertion = lib.all isAbsoluteEnvironmentFile instance.environmentFiles;
            message = ''
              services.ldk-server.instances.${name}.environmentFiles must
              contain absolute paths
            '';
          }
          {
            assertion = !instance.openFirewall || instance.lightningPort != null || instance.grpcPort != null;
            message = ''
              services.ldk-server.instances.${name}.openFirewall requires at
              least one port
            '';
          }
        ]) enabledInstances
      )
      ++ [
        {
          assertion =
            builtins.length (lib.unique (lib.mapAttrsToList (_: instance: instance.dataDir) enabledInstances))
            == builtins.length (lib.attrNames enabledInstances);
          message = "Enabled LDK Server instances must use different data directories";
        }
      ];

    users.groups = lib.mapAttrs' (_: instance: lib.nameValuePair instance.group { }) defaultGroups;

    users.users = lib.mapAttrs' (
      _: instance:
      lib.nameValuePair instance.user {
        isSystemUser = true;
        group = instance.group;
        home = instance.dataDir;
      }
    ) defaultUsers;

    systemd.tmpfiles.rules = lib.mapAttrsToList (
      _: instance: "d '${instance.dataDir}' 0750 ${instance.user} ${instance.group} - -"
    ) enabledInstances;

    systemd.services = lib.mapAttrs' (
      name: instance:
      lib.nameValuePair "ldk-server-${name}" {
        description = "LDK Server Lightning Node (${name})";
        documentation = [ "https://github.com/lightningdevkit/ldk-server" ];
        wantedBy = [ "multi-user.target" ];
        wants = [ "network-online.target" ];
        after = [ "network-online.target" ];

        serviceConfig = {
          Type = "notify";
          NotifyAccess = "main";
          ExecStart = lib.escapeShellArgs [
            (lib.getExe cfg.package)
            (instanceConfigPath name instance)
            "--storage-dir-path"
            instance.dataDir
          ];
          User = instance.user;
          Group = instance.group;
          EnvironmentFile = instance.environmentFiles;
          Restart = "on-failure";
          RestartSec = 10;

          NoNewPrivileges = true;
          PrivateDevices = true;
          PrivateTmp = true;
          ProtectControlGroups = true;
          ProtectHome = true;
          ProtectKernelModules = true;
          ProtectKernelTunables = true;
          ProtectSystem = "strict";
          ReadWritePaths = [ instance.dataDir ];
          RestrictAddressFamilies = [
            "AF_INET"
            "AF_INET6"
            "AF_UNIX"
          ];
        };
      }
    ) enabledInstances;

    networking.firewall.allowedTCPPorts = lib.concatMap (
      instance:
      lib.optionals instance.openFirewall (
        lib.filter (port: port != null) [
          instance.lightningPort
          instance.grpcPort
        ]
      )
    ) (lib.attrValues enabledInstances);
  };
}
