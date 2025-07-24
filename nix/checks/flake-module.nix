{ inputs, self, ... }:
{
  perSystem =
    { pkgs, config, ... }:
    let
      advisory-db = inputs.advisory-db;
    in
    {
      checks = {
        default = config.packages.ldk-server;
        cargo-audit = pkgs.craneLib.cargoAudit {
          src = ../../.;
          inherit advisory-db;
        };
        intTest = pkgs.callPackage ./integration.nix {
          inherit
            self
            inputs
            ;
          formatting = config.treefmt.build.check self;
        };
      };
    };
}
