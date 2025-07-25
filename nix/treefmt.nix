{ ... }:
{
  perSystem =
    { pkgs, lib, ... }:
    {
      treefmt = {
        projectRootFile = "flake.nix";
        programs = {
          nixfmt.enable = true;
          rustfmt.enable = true;
        };
        settings = {
          formatter.rustfmt.options = [
            "--config-path"
            "./rustfmt.toml"
          ];
        };
      };
    };
}
