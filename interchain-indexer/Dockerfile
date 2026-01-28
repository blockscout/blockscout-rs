FROM ghcr.io/blockscout/services-base:latest AS chef

FROM chef AS plan
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS cache
COPY --from=plan /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

FROM chef AS build

COPY --from=proto . /proto
COPY . .
COPY --from=cache /app/target target
COPY --from=cache $CARGO_HOME $CARGO_HOME
#RUN cargo build --release --bin interchain-indexer-server
RUN cargo build --release

FROM ubuntu:24.04 AS run
RUN apt-get update && apt-get install -y libssl3 libssl-dev ca-certificates

WORKDIR /app
ENV APP_USER=app
# Processes in a container should not run as root, so we need to create app user
# https://medium.com/@mccode/processes-in-containers-should-not-run-as-root-2feae3f0df3b
RUN groupadd $APP_USER \
    && useradd -g $APP_USER $APP_USER

COPY --from=build /app/target/release/interchain-indexer-server /app/interchain-indexer-server
# Change directory access for app user
RUN chown -R $APP_USER:$APP_USER /app
USER app

COPY ./config ./config

ENV INTERCHAIN_INDEXER__SWAGGER_PATH=/app/static/interchain-indexer.swagger.yaml
COPY ./interchain-indexer-proto/swagger/v1/interchain-indexer.swagger.yaml $INTERCHAIN_INDEXER__SWAGGER_PATH

CMD ["./interchain-indexer-server"]
