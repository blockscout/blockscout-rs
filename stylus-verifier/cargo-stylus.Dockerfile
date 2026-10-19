# Every supported cargo-stylus version and Rust toolchain, each toolchain with the wasm target
# and rust-src: everything a Stylus project needs to be built offline, in the stylus-verifier's
# sandbox. A build picks its cargo-stylus version with CARGO_STYLUS_VERSION and its toolchain with
# the project's rust-toolchain.toml. Builds are reproducible on one architecture only, so the
# image is linux/amd64.
#
# The verifier's `sandbox.verifier.images` entry for an image lists the two lists below.

# 0.10.1 was yanked.
ARG CARGO_STYLUS_VERSIONS="0.5.12 0.6.0 0.6.1 0.6.2 0.6.3 0.10.0 0.10.2 0.10.3 0.10.4 0.10.5 0.10.6 0.10.7 0.10.8 0.10.9 0.10.10"
ARG RUST_TOOLCHAINS="1.80.0 1.81.0 1.88.0 1.91.0 1.97.1"
# cargo-stylus 0.10 needs Rust 1.91 to build.
ARG BUILDER_IMAGE=rust:1.91.1-bookworm@sha256:c1e5f19e773b7878c3f7a805dd00a495e747acbdc76fb2337a4ebf0418896b33
ARG RUNTIME_IMAGE=debian:bookworm-slim@sha256:7c7b2c966bc9ee8cedfeef67e0e279108992c77681fa595db4a9d65c06ccc587

FROM ${BUILDER_IMAGE} AS cargo-stylus
ARG CARGO_STYLUS_VERSIONS
# One target directory for all versions, so the dependencies they share are compiled once.
RUN set -eu; \
    for version in ${CARGO_STYLUS_VERSIONS}; do \
        cargo install cargo-stylus --version "=${version}" --locked \
            --target-dir /tmp/target --root "/opt/cargo-stylus/${version}"; \
    done; \
    rm -rf /tmp/target

FROM ${RUNTIME_IMAGE}
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
ARG RUSTUP_VERSION=1.28.2
ARG RUSTUP_SHA256=20a06e644b0d9bd2fbdbfd52d42540bdde820ea7df86e92e533c073da0cdd43c
RUN set -eu; \
    apt-get update; \
    # cargo-stylus before 0.10 links OpenSSL.
    apt-get install -y --no-install-recommends ca-certificates curl gcc git libc6-dev libssl3; \
    rm -rf /var/lib/apt/lists/*; \
    curl -fsSL --proto '=https' --tlsv1.2 -o /tmp/rustup-init \
        "https://static.rust-lang.org/rustup/archive/${RUSTUP_VERSION}/x86_64-unknown-linux-gnu/rustup-init"; \
    echo "${RUSTUP_SHA256} /tmp/rustup-init" | sha256sum -c -; \
    chmod +x /tmp/rustup-init; \
    /tmp/rustup-init -y --no-modify-path --profile minimal --default-toolchain none; \
    rm /tmp/rustup-init

COPY --from=cargo-stylus /opt/cargo-stylus /opt/cargo-stylus
COPY --chmod=0755 <<'EOF' /usr/local/cargo/bin/cargo-stylus
#!/bin/sh
# Runs the cargo-stylus version named by CARGO_STYLUS_VERSION.
set -eu
version="${CARGO_STYLUS_VERSION:-}"
case "${version}" in
    "" | *[!0-9.]*)
        echo "CARGO_STYLUS_VERSION must name a cargo-stylus version, got '${version}'" >&2
        exit 2
        ;;
esac
binary="/opt/cargo-stylus/${version}/bin/cargo-stylus"
if [ ! -x "${binary}" ]; then
    echo "cargo-stylus ${version} is not in this image" >&2
    exit 2
fi
exec "${binary}" "$@"
EOF

ARG RUST_TOOLCHAINS
RUN set -eu; \
    for version in ${RUST_TOOLCHAINS}; do \
        toolchain="${version}-x86_64-unknown-linux-gnu"; \
        rustup toolchain install "${toolchain}" --profile minimal \
            --target wasm32-unknown-unknown --component rust-src; \
        rustup default "${toolchain}"; \
    done; \
    chmod -R a+rX "${RUSTUP_HOME}" "${CARGO_HOME}"
