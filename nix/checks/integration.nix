{ pkgs, self, ... }:
pkgs.nixosTest {
  name = "IntegrationTest";
  nodes = {
    server = {
      imports = [
        self.nixosModules.ldk-server
      ];
      services.ldk-server = {
        enable = true;
      };
    };
  };
  testScript =
    { nodes }:
    ''
      server.start()
      server.wait_for_unit("ldk-server.service")

      server.succeed("ldk-server-cli help")
      server.succeed("ldk-server-cli get-node-info")
      server.succeed("ldk-server-cli onchain-receive")
    '';
}
