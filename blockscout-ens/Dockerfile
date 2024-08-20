FROM ghcr.io/blockscout/services-base:latest as chef

FROM chef AS plan
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef as cache
COPY --from=plan /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

FROM chef AS build
# Include proto common definitions (will be used in a `build-contexts` section)
COPY --from=proto . /proto
COPY . .
COPY --from=cache /app/target target
COPY --from=cache $CARGO_HOME $CARGO_HOME
RUN cargo build --release --bin bens-server

FROM ubuntu:20.04 as run
WORKDIR /app
RUN apt-get update && apt-get install -y libssl1.1 libssl-dev ca-certificates
ENV APP_USER=app
RUN groupadd $APP_USER \
    && useradd -g $APP_USER $APP_USER
COPY --from=build /app/target/release/bens-server /app/bens-server
COPY ./bens-server/config/ /app/config/
ENTRYPOINT ["/app/bens-server"]
