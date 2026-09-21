FROM ubuntu:24.04

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        coreutils \
        libgcc-s1 \
        libssl3 \
        libstdc++6 \
        zlib1g \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 65532 compiler \
    && useradd --uid 65532 --gid 65532 --no-create-home --home-dir /tmp compiler \
    && install -d -m 0755 -o root -g root /job /compiler-tmp /compiler-cache

USER 65532:65532
WORKDIR /job
ENV HOME=/tmp TMPDIR=/compiler-tmp

CMD ["/bin/false"]
