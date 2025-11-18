# ---- Build stage ----
FROM rust:1.89 as builder

WORKDIR /usr/src/ens-rpc-patch-proxy

# copy manifests first to leverage caching
COPY Cargo.toml Cargo.lock ./
# ensure deps are downloaded before code copy
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm -f target/release/deps/ens_rpc_patch_proxy*

# copy real sources
COPY src ./src

# build optimized binary
RUN cargo build --release

# ---- Runtime stage ----
FROM gcr.io/distroless/cc-debian12

WORKDIR /app
COPY --from=builder /usr/src/ens-rpc-patch-proxy/target/release/ens-rpc-patch-proxy /usr/local/bin/ens-rpc-patch-proxy

EXPOSE 8080

ENTRYPOINT ["/usr/local/bin/ens-rpc-patch-proxy"]
