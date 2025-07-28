{ inputs, self, ... }:
{
  perSystem =
    { pkgs, config, ... }:
    {
      checks = {
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
