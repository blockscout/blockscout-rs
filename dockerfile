FROM rust:1 as build

WORKDIR /build_app

# cache dependencies
RUN cargo init
COPY ./Cargo.toml ./
COPY ./Cargo.lock ./
RUN cargo build --release
RUN rm -rf ./src

# build
COPY ./ ./
RUN cargo build --release

FROM ubuntu:20.04 as run

RUN apt-get update && apt-get install -y libssl1.1 libssl-dev ca-certificates

WORKDIR /app
COPY --from=build /build_app/target/release/verification /app/verification

ENTRYPOINT ["/app/verification"]