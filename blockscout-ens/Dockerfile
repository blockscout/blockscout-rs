FROM lukemathwalker/cargo-chef:0.1.62-rust-1.72-buster as chef
WORKDIR /app
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
RUN cargo build --release --bin bens-server

FROM ubuntu:20.04 as run
WORKDIR /app
RUN apt-get update && apt-get install -y libssl1.1 libssl-dev ca-certificates
ENV APP_USER=app
RUN groupadd $APP_USER \
    && useradd -g $APP_USER $APP_USER
COPY --from=build /app/target/release/bens-server /usr/local/bin/bens-server
ENTRYPOINT ["/usr/local/bin/bens-server"]
