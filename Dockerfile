FROM rust:1.91.1-slim@sha256:cef0ec962e08d8b5dcba05604189e5751c1bd3ec7d12db0a93e4215468d4ac4a AS builder

ARG BUILD_FEATURES=""

WORKDIR /opt/app

COPY Cargo.* ./
COPY ldk-server/ ldk-server/
COPY ldk-server-cli/ ldk-server-cli/
COPY ldk-server-client/ ldk-server-client/
COPY ldk-server-protos/ ldk-server-protos/
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    if [ -n "$BUILD_FEATURES" ]; then \
      cargo build --release --features "$BUILD_FEATURES"; \
    else \
      cargo build --release; \
    fi

FROM debian:13.1-slim@sha256:1caf1c703c8f7e15dcf2e7769b35000c764e6f50e4d7401c355fb0248f3ddfdb

COPY --from=builder /opt/app/target/release/ldk-server /usr/local/bin/ldk-server
COPY --from=builder /opt/app/target/release/ldk-server-cli /usr/local/bin/ldk-server-cli
COPY --from=builder  /opt/app/ldk-server/ldk-server-config.toml /usr/local/bin/ldk-server-config.toml

EXPOSE 3000 3001

ENTRYPOINT [ "ldk-server", "/usr/local/bin/ldk-server-config.toml" ]
