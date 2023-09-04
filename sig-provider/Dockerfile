FROM lukemathwalker/cargo-chef:0.1.62-rust-1.72-buster as chef
WORKDIR /app

RUN apt-get update && apt-get install -y curl wget unzip
RUN wget https://github.com/protocolbuffers/protobuf/releases/download/v21.12/protoc-21.12-linux-x86_64.zip -O ./protoc.zip \
          && unzip protoc.zip \
          && mv ./include/* /usr/include/ \
          && mv ./bin/protoc /usr/bin/protoc

RUN wget https://github.com/grpc-ecosystem/grpc-gateway/releases/download/v2.15.0/protoc-gen-openapiv2-v2.15.0-linux-x86_64 -O ./protoc-gen-openapiv2 \
          && chmod +x protoc-gen-openapiv2 \
          && mv ./protoc-gen-openapiv2 /usr/bin/protoc-gen-openapiv2

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
# Processes in a container should not run as root, so we need to create app user
# https://medium.com/@mccode/processes-in-containers-should-not-run-as-root-2feae3f0df3b
RUN groupadd $APP_USER \
    && useradd -g $APP_USER $APP_USER

COPY --from=build /app/target/release/sig-provider-server /app/sig-provider-server
# Change directory access for app user
RUN chown -R $APP_USER:$APP_USER /app
USER app

CMD ["./sig-provider-server"]
