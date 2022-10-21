FROM lukemathwalker/cargo-chef:0.1.39-rust-1.63-buster as chef
WORKDIR /app

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

FROM ubuntu:20.04 as run
RUN apt-get update && apt-get install -y libssl1.1 libssl-dev ca-certificates

WORKDIR /app
ENV APP_USER=app
RUN groupadd $APP_USER \
    && useradd -g $APP_USER $APP_USER

COPY --from=build /app/target/release/smart-contract-verifier-http /app/smart-contract-verifier-http
RUN chown -R $APP_USER:$APP_USER /app
USER app

CMD ["./smart-contract-verifier-http"]
