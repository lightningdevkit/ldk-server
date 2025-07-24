{ self, ... }:
{
  perSystem =
    {
      config,
      pkgs,
      system,
      ...
    }:
    {
      devShells = {
        default = pkgs.craneLib.devShell {
          checks = self.checks.${system};
        };
      };
    };
}
