{ inputs, ... }:
{
  perSystem =
    { pkgs, config, ... }:
    let
      src = pkgs.craneLib.cleanCargoSource ../.;
      commonArgs = {
        inherit src;
        strictDeps = true;
      };
      cargoArtifacts = pkgs.craneLib.buildDepsOnly commonArgs;
      advisory-db = inputs.advisory-db;
      ldk-server = pkgs.craneLib.buildPackage (
        commonArgs
        // {
          pname = "ldk-server";
          inherit cargoArtifacts;
          meta = {
            description = "Lightning Development Kit server";
            mainProgram = "ldk-server";
          };
        }
      );
      ldk-server-cli = pkgs.craneLib.buildPackage (
        commonArgs
        // {
          pname = "ldk-server-cli";
          inherit cargoArtifacts;
          meta = {
            description = "Command line interface for the Lightning Development Kit server";
            mainProgram = "ldk-server-cli";
          };
        }
      );
      ldk-server-client = pkgs.craneLib.buildPackage (
        commonArgs
        // {
          pname = "ldk-server-client";
          inherit cargoArtifacts;
        }
      );
      ldk-server-protos = pkgs.craneLib.buildPackage (
        commonArgs
        // {
          pname = "ldk-server-protos";
          inherit cargoArtifacts;
          nativeBuildInputs = with pkgs; [ protobuf ];
        }
      );
    in
    {
      packages = {
        inherit
          ldk-server
          ldk-server-cli
          ldk-server-client
          ldk-server-protos
          ;
        default = ldk-server;
      };
      checks = {
        cargo-audit = pkgs.craneLib.cargoAudit {
          inherit advisory-db src;
        };
        default = config.packages.ldk-server;
      };
      apps = {
        ldk-server = {
          program = "${ldk-server}/bin/ldk-server";
          meta.description = "Lightning Dev Kit Server";
        };
        ldk-server-cli = {
          program = "${ldk-server-cli}/bin/ldk-server-cli";
          meta.description = "Lightning Dev Kit Server Command Line Interface";
        };
      };
    };
}
