FROM rust:1 as build

WORKDIR /build
COPY ./ ./

RUN cargo build --release

FROM ubuntu:20.04 as run

RUN apt-get update && apt-get install -y libssl1.1 libssl-dev ca-certificates

WORKDIR /app
COPY --from=build /build/target/release/verification /app/verification

ENTRYPOINT ["/app/verification"]