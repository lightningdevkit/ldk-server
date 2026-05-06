# ldk-server

The main LDK Server daemon. This is a Lightning Network node that exposes a gRPC API over
HTTP/2 with TLS, built on [LDK Node](https://github.com/lightningdevkit/ldk-node).

## Running

```bash
cargo run --release --bin ldk-server /path/to/config.toml
```

See the [Getting Started](../docs/getting-started.md) guide for a full walkthrough.

## Configuration

A fully annotated config template is provided at
[ldk-server-config.toml](../contrib/ldk-server-config.toml). See
[Configuration](../docs/configuration.md) for details on all options, environment variables,
and Bitcoin backend choices.

## Documentation

- [Getting Started](../docs/getting-started.md): build, configure, and run your first node
- [API Guide](../docs/api-guide.md): gRPC transport, authentication, and endpoint reference
- [Operations](../docs/operations.md): production deployment, backups, and monitoring
