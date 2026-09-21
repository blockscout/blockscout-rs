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
RUN apt-get update \
    && apt-get install -y --no-install-recommends libssl3 ca-certificates openssh-client \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
ENV APP_USER=app
# Processes in a container should not run as root, so we need to create app user
# https://medium.com/@mccode/processes-in-containers-should-not-run-as-root-2feae3f0df3b
RUN groupadd $APP_USER \
    && useradd --create-home --home-dir /home/$APP_USER -g $APP_USER $APP_USER \
    && install -d -m 0700 -o $APP_USER -g $APP_USER /home/$APP_USER/.ssh

COPY --from=build /app/target/release/smart-contract-verifier-server /app/smart-contract-verifier-server
# Change directory access for app user
RUN chown -R $APP_USER:$APP_USER /app
USER app
ENV HOME=/home/app

CMD ["./smart-contract-verifier-server"]
