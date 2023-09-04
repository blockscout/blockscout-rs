FROM lukemathwalker/cargo-chef:0.1.62-rust-1.72-buster as chef
RUN apt-get update && apt-get install -y curl wget unzip
WORKDIR /app

# Install protoc
RUN wget https://github.com/protocolbuffers/protobuf/releases/download/v21.12/protoc-21.12-linux-x86_64.zip -O ./protoc.zip \
    && unzip protoc.zip \
    && mv ./include/* /usr/include/ \
    && mv ./bin/protoc /usr/bin/protoc

# Install protoc-gen-openapiv2
RUN wget https://github.com/grpc-ecosystem/grpc-gateway/releases/download/v2.15.0/protoc-gen-openapiv2-v2.15.0-linux-x86_64 -O ./protoc-gen-openapiv2 \
        && chmod +x protoc-gen-openapiv2 \
        && mv ./protoc-gen-openapiv2 /usr/bin/protoc-gen-openapiv2


FROM chef AS plan
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS build
COPY --from=plan /app/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook --release --recipe-path recipe.json
# Build application
COPY . .
RUN cargo build --release

FROM node:16-bullseye-slim
WORKDIR /usr/src/
# sol2uml needed phantom which installation needed bzip2
RUN apt-get update && apt-get install bzip2 \
    && npm install phantom \
    && npm link sol2uml@2.1 --only=production

COPY --from=build /app/target/release/visualizer-server ./
ENTRYPOINT ["./visualizer-server"]
