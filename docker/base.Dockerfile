ARG CARGO_CHEF_VERSION=0.1.62-rust-1.75-buster
FROM lukemathwalker/cargo-chef:${CARGO_CHEF_VERSION}

ARG PROTOC_VERSION=25.2
ARG PROTOC_GEN_OPENAPIV2_VERSION=2.19.0

RUN apt-get update && apt-get install -y curl wget unzip
RUN wget https://github.com/protocolbuffers/protobuf/releases/download/v${PROTOC_VERSION}/protoc-${PROTOC_VERSION}-linux-x86_64.zip -O ./protoc.zip \
          && unzip protoc.zip \
          && mv ./include/* /usr/include/ \
          && mv ./bin/protoc /usr/bin/protoc

RUN wget https://github.com/grpc-ecosystem/grpc-gateway/releases/download/v${PROTOC_GEN_OPENAPIV2_VERSION}/protoc-gen-openapiv2-v${PROTOC_GEN_OPENAPIV2_VERSION}-linux-x86_64 -O ./protoc-gen-openapiv2 \
          && chmod +x protoc-gen-openapiv2 \
          && mv ./protoc-gen-openapiv2 /usr/bin/protoc-gen-openapiv2

WORKDIR /app
