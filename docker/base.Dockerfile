ARG CARGO_CHEF_VERSION=0.1.72-rust-1.88
FROM lukemathwalker/cargo-chef:${CARGO_CHEF_VERSION}

ARG PROTOC_VERSION=31.1
ARG PROTOC_GEN_OPENAPIV2_VERSION=2.27.1

RUN apt-get update && apt-get install -y curl wget unzip

ARG TARGETARCH
RUN case ${TARGETARCH} in \
        "arm64") TARGETARCH=aarch_64 ;; \
        "amd64") TARGETARCH=x86_64 ;; \
    esac \
        && wget https://github.com/protocolbuffers/protobuf/releases/download/v${PROTOC_VERSION}/protoc-${PROTOC_VERSION}-linux-${TARGETARCH}.zip -O ./protoc.zip \
        && unzip protoc.zip \
        && mv ./include/* /usr/include/

RUN case ${TARGETARCH} in \
        "amd64") TARGETARCH=x86_64 ;; \
    esac \
        && wget https://github.com/grpc-ecosystem/grpc-gateway/releases/download/v${PROTOC_GEN_OPENAPIV2_VERSION}/protoc-gen-openapiv2-v${PROTOC_GEN_OPENAPIV2_VERSION}-linux-${TARGETARCH} -O ./protoc-gen-openapiv2 \
        && chmod +x protoc-gen-openapiv2 \
        && mv ./protoc-gen-openapiv2 /usr/bin/protoc-gen-openapiv2

WORKDIR /app
