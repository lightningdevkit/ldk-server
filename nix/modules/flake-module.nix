{ ... }:
{
  flake = {
    nixosModules = {
      ldk-server.imports = [ ./ldk-server.nix ];
    };
  };
}
