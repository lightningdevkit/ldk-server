FROM rust:1.85 AS builder

WORKDIR /app

# Copy manifests and lock file first for dependency caching
COPY Cargo.toml Cargo.lock ./
COPY ldk-server/Cargo.toml ldk-server/Cargo.toml
COPY ldk-server-cli/Cargo.toml ldk-server-cli/Cargo.toml
COPY ldk-server-client/Cargo.toml ldk-server-client/Cargo.toml
COPY ldk-server-protos/Cargo.toml ldk-server-protos/Cargo.toml
COPY ldk-server-protos/build.rs ldk-server-protos/build.rs

# Create dummy source files so cargo can resolve and build dependencies
RUN mkdir -p ldk-server/src ldk-server-cli/src ldk-server-client/src ldk-server-protos/src \
    && echo "fn main() {}" > ldk-server/src/main.rs \
    && echo "fn main() {}" > ldk-server-cli/src/main.rs \
    && echo "" > ldk-server-client/src/lib.rs \
    && echo "" > ldk-server-protos/src/lib.rs

# Build dependencies only (this layer is cached unless Cargo.toml/Cargo.lock change)
RUN cargo build --release -p ldk-server --all-features \
    && cargo build --release -p ldk-server-cli

# Copy real source and rebuild
COPY . .
RUN touch ldk-server/src/main.rs ldk-server-cli/src/main.rs \
    ldk-server-client/src/lib.rs ldk-server-protos/src/lib.rs \
    && cargo build --release -p ldk-server --all-features \
    && cargo build --release -p ldk-server-cli

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/ldk-server /usr/local/bin/ldk-server
COPY --from=builder /app/target/release/ldk-server-cli /usr/local/bin/ldk-server-cli

EXPOSE 9735 3002

ENTRYPOINT ["ldk-server"]
