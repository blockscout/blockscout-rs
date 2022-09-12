FROM rust:1 as build

WORKDIR /build_app

# cache dependencies
RUN cargo new smart-contract-verifier
RUN cargo new smart-contract-verifier-http

COPY ./smart-contract-verifier/Cargo.toml ./smart-contract-verifier/
COPY ./smart-contract-verifier-http/Cargo.toml ./smart-contract-verifier-http/
COPY ./Cargo.lock ./smart-contract-verifier-http/

WORKDIR /build_app/smart-contract-verifier-http
RUN cargo build --release

WORKDIR /build_app
RUN rm -rf ./smart-contract-verifier/src
RUN rm -rf ./smart-contract-verifier-http/src

# build
COPY ./smart-contract-verifier ./smart-contract-verifier
COPY ./smart-contract-verifier-http ./smart-contract-verifier-http

WORKDIR /build_app/smart-contract-verifier-http
RUN cargo build --release

FROM ubuntu:20.04 as run

RUN apt-get update && apt-get install -y libssl1.1 libssl-dev ca-certificates

WORKDIR /app
COPY --from=build /build_app/smart-contract-verifier-http/target/release/smart-contract-verifier-http /app/smart-contract-verifier

ENTRYPOINT ["/app/smart-contract-verifier"]