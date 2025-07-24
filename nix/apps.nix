{ ... }:
{
  perSystem =
    {
      config,
      pkgs,
      self',
      system,
      ...
    }:
    {
      apps = {
        ldk-server = {
          program = "${self'.packages.ldk-server}/bin/ldk-server";
          meta.description = "Lightning Dev Kit Server";
        };
        ldk-server-cli = {
          program = "${self'.packages.ldk-server-cli}/bin/ldk-server-cli";
          meta.description = "Lightning Dev Kit Server Command Line Interface";
        };
      };
    };

}
