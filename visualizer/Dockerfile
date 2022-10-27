# FROM amd64/rust:1-slim-bullseye as build



# WORKDIR /build_app
# RUN cargo new --lib visualizer && cargo new --lib visualizer-server
# COPY ./visualizer/Cargo.toml ./visualizer/
# COPY ./visualizer-server/Cargo.toml ./visualizer-server/
# COPY ./Cargo.lock ./visualizer-server/
# WORKDIR /build_app/visualizer-server
# RUN cargo build --release


# WORKDIR /build_app
# RUN rm -rf ./smart-contract-verifier/src
# RUN rm -rf ./visualizer-server/src

# # build
# COPY ./visualizer ./visualizer
# COPY ./visualizer-server ./visualizer-server

# WORKDIR /build_app/visualizer-server
# RUN cargo build --release


# # The final base image
# FROM node:16-bullseye-slim

# WORKDIR /usr/src/

# # sol2uml needed phantom which installation needed bzip2
# RUN apt-get update && apt-get install bzip2 \
#     && npm install phantom \
#     && npm link sol2uml@2.1 --only=production

# # Copy from the previous build
# COPY --from=build /visualizer-server/target/release/visualizer-server /usr/src/visualizer-server

# # Run the binary
# ENTRYPOINT ["/usr/src/visualizer-server"]

FROM lukemathwalker/cargo-chef:latest-rust-1.63.0 AS chef
RUN apt-get update && apt-get install -y curl wget unzip
WORKDIR /app

# Install protoc
RUN curl -s https://api.github.com/repos/protocolbuffers/protobuf/releases/latest \ 
          | grep "browser_download_url" \
          | grep "protoc-.*-linux-x86_64" \
          | cut -d : -f 2,3 \
          | tr -d '"' \
          | xargs wget -O ./protoc.zip \
    && unzip protoc.zip \
    && mv ./include/* /usr/include/ \
    && mv ./bin/protoc /usr/bin/protoc

# Install protoc-gen-openapiv2
RUN curl -s https://api.github.com/repos/grpc-ecosystem/grpc-gateway/releases/latest \
          | grep "browser_download_url" \
          | grep "protoc-gen-openapiv2-.*-linux-x86_64" \
          | cut -d : -f 2,3 \
          | tr -d '"' \
          | xargs wget -O ./protoc-gen-openapiv2 \
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

COPY --from=build /app/target/release/visualizer-server /
ENTRYPOINT ["./visualizer-server"]
