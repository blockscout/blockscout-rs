FROM ghcr.io/blockscout/services-base:latest as chef

FROM chef AS plan
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef as cache
COPY --from=plan /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

FROM chef AS build

COPY . .
COPY --from=cache /app/target target
COPY --from=cache $CARGO_HOME $CARGO_HOME
RUN cargo build --release

FROM ubuntu:24.04 as run
RUN apt-get update && apt-get install -y libssl3 libssl-dev ca-certificates

WORKDIR /app
ENV APP_USER=app
# Processes in a container should not run as root, so we need to create app user
# https://medium.com/@mccode/processes-in-containers-should-not-run-as-root-2feae3f0df3b
RUN groupadd $APP_USER \
    && useradd -g $APP_USER $APP_USER

COPY --from=build /app/target/release/tac-operation-lifecycle-server /app/tac-operation-lifecycle-server

ENV TAC_OPERATION_LIFECYCLE__SWAGGER_PATH=/app/static/tac-operation-lifecycle.swagger.yaml
COPY ./tac-operation-lifecycle-proto/swagger/v1/tac-operation-lifecycle.swagger.yaml $TAC_OPERATION_LIFECYCLE__SWAGGER_PATH

# Change directory access for app user
RUN chown -R $APP_USER:$APP_USER /app
USER app

CMD ["./tac-operation-lifecycle-server"]
