FROM rust:1 AS builder

WORKDIR /build_app

# Cache dependencies
RUN cargo init
COPY ./Cargo.toml ./
RUN cargo build --release
RUN rm -rf ./src

# Build stage
RUN apt update && apt install lld clang -y
COPY . .

RUN cargo build --release

# Runtime stage
FROM debian:bullseye-slim AS runtime

RUN apt-get update && apt-get install -y libssl1.1 libssl-dev ca-certificates

WORKDIR /app
COPY --from=builder /build_app/target/release/multichain-api-gateway multichain-api-gateway

ENTRYPOINT ["./multichain-api-gateway"]
