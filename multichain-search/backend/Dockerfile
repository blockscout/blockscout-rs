FROM lukemathwalker/cargo-chef:0.1.62-rust-1.72-buster as chef
WORKDIR /app

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

FROM gcr.io/distroless/cc

ENV RUST_LOG="info"

COPY --from=build /app/target/release/multichain-search /
ENTRYPOINT ["./multichain-search"]
