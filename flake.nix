{
  description = "LDK Server";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      supportedSystems = [
        "aarch64-darwin"
        "aarch64-linux"
        "x86_64-linux"
      ];
      forAllSystems = nixpkgs.lib.genAttrs supportedSystems;
    in
    {
      packages = forAllSystems (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        {
          default = self.packages.${system}.ldk-server;
          ldk-server = pkgs.callPackage ./nix/package.nix {
            gitHash = self.rev or self.dirtyRev or "unknown";
          };
        }
      );

      checks = forAllSystems (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        {
          inherit (self.packages.${system}) ldk-server;
        }
        // nixpkgs.lib.optionalAttrs pkgs.stdenv.hostPlatform.isLinux {
          nixos-module = pkgs.testers.runNixOSTest (
            import ./nix/tests/module.nix {
              inherit pkgs;
              ldkServerModule = self.nixosModules.ldk-server;
            }
          );
        }
      );

      nixosModules = {
        default = self.nixosModules.ldk-server;
        ldk-server =
          { lib, pkgs, ... }:
          {
            imports = [ ./nix/module.nix ];
            services.ldk-server.package =
              lib.mkDefault
                self.packages.${pkgs.stdenv.hostPlatform.system}.ldk-server;
          };
      };

      formatter = forAllSystems (system: nixpkgs.legacyPackages.${system}.nixfmt-tree);
    };
}
