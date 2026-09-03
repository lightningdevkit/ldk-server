# Nix deployment

The flake builds `ldk-server` and `ldk-server-cli`. It also provides a NixOS
module for the daemon.

## Run without installing

Build and run the server from this repository:

```bash
nix run . -- /path/to/config.toml
```

Run the command-line client:

```bash
nix shell .#ldk-server -c ldk-server-cli --help
```

## Deploy on NixOS

Add the flake to your system inputs:

```nix
{
  inputs.ldk-server.url = "github:lightningdevkit/ldk-server";

  outputs = { nixpkgs, ldk-server, ... }: {
    nixosConfigurations.my-host = nixpkgs.lib.nixosSystem {
      system = "x86_64-linux";
      modules = [
        ldk-server.nixosModules.default
        {
          services.ldk-server.instances = {
            mainnet = {
              enable = true;
              openFirewall = true;
              lightningPort = 9735;
              settings = {
                node = {
                  network = "bitcoin";
                  listening_addresses = [ "0.0.0.0:9735" ];
                  grpc_service_address = "127.0.0.1:3536";
                };
                esplora.server_url = "https://mempool.space/api";
                log = {
                  level = "Info";
                  log_to_file = false;
                };
              };
            };

            signet = {
              enable = true;
              lightningPort = 19735;
              grpcPort = 13536;
              settings = {
                node = {
                  network = "signet";
                  listening_addresses = [ "127.0.0.1:19735" ];
                  grpc_service_address = "127.0.0.1:13536";
                };
                esplora.server_url = "https://mutinynet.com/api";
              };
            };
          };
        }
      ];
    };
  };
}
```

By default, each instance gets a separate service, user, configuration, and
data directory. For example, `mainnet` uses `ldk-server-mainnet.service` and
stores data in `/var/lib/ldk-server/mainnet`.

Use different Lightning and gRPC addresses for each instance. The module sets
each data path even if the TOML file contains a different storage path.

The example exposes the Lightning port but keeps the gRPC API on loopback.
Before you expose gRPC, configure its certificate and client access as
described in [Operations - TLS](operations.md#tls).

The `settings` option writes values to the Nix store. Do not put passwords or
other secrets in this option. Use `environmentFiles` for secrets:

```nix
services.ldk-server.instances.mainnet.environmentFiles = [
  "/run/secrets/ldk-server-mainnet"
];
```

The file can override supported settings with environment variables:

```text
LDK_SERVER_BITCOIND_RPC_USER=rpc-user
LDK_SERVER_BITCOIND_RPC_PASSWORD=rpc-password
```

You can also set an instance's `configFile` to a complete TOML file. You
cannot use `configFile` and `settings` on the same instance.

After deployment, inspect the service with this command:

```bash
systemctl status ldk-server-mainnet
```
