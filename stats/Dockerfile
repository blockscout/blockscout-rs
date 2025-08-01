FROM ghcr.io/blockscout/services-base:latest AS chef

FROM chef AS plan
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS cache
COPY --from=plan /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

FROM chef AS build

# Include proto common definitions (`proto` must be passed as build context)
COPY --from=proto . /proto
COPY . .
COPY --from=cache /app/target target
COPY --from=cache $CARGO_HOME $CARGO_HOME
RUN cargo build --release

FROM ubuntu:25.04 AS run
RUN apt-get update && apt-get install -y libssl3 libssl-dev ca-certificates

WORKDIR /app
ENV APP_USER=app
# Processes in a container should not run as root, so we need to create app user
# https://medium.com/@mccode/processes-in-containers-should-not-run-as-root-2feae3f0df3b
RUN groupadd $APP_USER \
    && useradd -g $APP_USER $APP_USER

COPY --from=build /app/target/release/stats-server /app/stats-server
# Change directory access for app user
RUN chown -R $APP_USER:$APP_USER /app
USER app

COPY ./config/charts.json ./config/charts.json
COPY ./config/layout.json ./config/layout.json
COPY ./config/update_groups.json ./config/update_groups.json
COPY ./config/multichain/charts.json ./config/multichain/charts.json
COPY ./config/multichain/layout.json ./config/multichain/layout.json
COPY ./config/multichain/update_groups.json ./config/multichain/update_groups.json

ENV STATS__SWAGGER_PATH=/app/static/stats.swagger.yaml
COPY ./stats-proto/swagger/stats.swagger.yaml $STATS__SWAGGER_PATH

CMD ["./stats-server"]
