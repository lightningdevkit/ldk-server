{ ... }:
{
  perSystem =
    { pkgs, config, ... }:
    let
      src = pkgs.craneLib.cleanCargoSource ../.;
      common = {
        inherit src;
        strictDeps = true;
      };
    in
    {
      packages = rec {
        ldk-server = pkgs.craneLib.buildPackage (
          common
          // {
            pname = "ldk-server";
            meta = {
              description = "Lightning Development Kit server";
              mainProgram = "ldk-server";
            };
          }
        );
        ldk-server-cli = pkgs.craneLib.buildPackage (
          common
          // {
            pname = "ldk-server-cli";
            meta = {
              description = "Command line interface for the Lightning Development Kit server";
              mainProgram = "ldk-server-cli";
            };
          }
        );
        ldk-server-client = pkgs.craneLib.buildPackage (
          common
          // {
            pname = "ldk-server-client";
          }
        );
        ldk-server-protos = pkgs.craneLib.buildPackage (
          common
          // {
            pname = "ldk-server-protos";
            nativeBuildInputs = with pkgs; [ protobuf ];
          }
        );
        default = ldk-server;
      };
    };
}
