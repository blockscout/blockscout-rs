FROM lukemathwalker/cargo-chef:0.1.39-rust-1.63-buster as builder
WORKDIR /smart-contract-verifier-http
# RUN git clone -b v0.5.0 https://github.com/blockscout/blockscout-rs \
#     && cd blockscout-rs/smart-contract-verifier-http \
#     && cargo build --release --target-dir /smart-contract-verifier-http --bin smart-contract-verifier-http
RUN cargo install --git https://github.com/blockscout/blockscout-rs --tag smart-contract-verifier/v0.6.0 smart-contract-verifier-http

FROM ubuntu:20.04
ENV APP_USER=root

RUN mkdir -p /smart-contract-verifier

RUN apt update && apt install -y nano sudo net-tools libssl1.1 libssl-dev ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/local/cargo/bin/smart-contract-verifier-http /smart-contract-verifier/smart-contract-verifier-http

# COPY --from=builder /smart-contract-verifier-http/release/smart-contract-verifier-http /smart-contract-verifier/smart-contract-verifier-http

RUN chown -R $APP_USER:$APP_USER /smart-contract-verifier
USER root

EXPOSE 8043

WORKDIR /smart-contract-verifier

CMD ["./smart-contract-verifier-http"]