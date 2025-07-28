{
  description = "LDK Server is a fully-functional Lightning node in daemon form, built on top of LDK Node, which itself provides a powerful abstraction over the Lightning Development Kit (LDK) and uses a built-in Bitcoin Development Kit (BDK) wallet.";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.05";

    flake-parts.url = "github:hercules-ci/flake-parts";

    crane.url = "github:ipetkov/crane";

    treefmt-nix.url = "github:numtide/treefmt-nix";

    advisory-db = {
      url = "github:rustsec/advisory-db";
      flake = false;
    };
  };

  outputs =
    inputs@{
      self,
      nixpkgs,
      flake-parts,
      ...
    }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = nixpkgs.lib.systems.flakeExposed;
      imports = [
        inputs.treefmt-nix.flakeModule
        ./nix/modules/flake-module.nix
        ./nix/checks/flake-module.nix
        ./nix/crane.nix
        ./nix/shells.nix
        ./nix/treefmt.nix
      ];
      perSystem =
        {
          config,
          pkgs,
          self',
          system,
          ...
        }:
        {
          _module.args.pkgs = import inputs.nixpkgs {
            inherit system;
            overlays = [
              (final: prev: {
                craneLib = (inputs.crane.mkLib pkgs);
                local = config.packages;
              })
            ];
          };
        };
    };
}
