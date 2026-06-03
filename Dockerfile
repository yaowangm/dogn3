FROM rust:1-bookworm AS builder
ENV CARGO_LOG=cargo::sources::registry=debug,cargo::util::network=debug
ENV CARGO_TERM_VERBOSE=true
ENV CARGO_NET_RETRY=5
ENV CARGO_NET_TIMEOUT=60

WORKDIR /app

COPY Cargo.toml Cargo.lock ./

COPY src ./src
COPY static ./static
COPY scripts ./scripts

#RUN cargo fetch --config 'source.crates-io.replace-with="tuna"' \
#    --config 'source.tuna.registry="sparse+https://mirrors.tuna.tsinghua.edu.cn/crates.io-index/"'
RUN cargo fetch --config 'source.crates-io.replace-with="ustc"' \
    --config 'source.ustc.registry="sparse+https://mirrors.ustc.edu.cn/crates.io-index/"'
RUN cargo build --release --bin dogn3 --verbose 

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

RUN useradd --system --create-home --home-dir /app --shell /usr/sbin/nologin dogn3

WORKDIR /app

COPY --from=builder /app/target/release/dogn3 /usr/local/bin/dogn3
COPY static ./static

RUN mkdir -p /app/images \
    && chown -R dogn3:dogn3 /app

USER dogn3

EXPOSE 3000

HEALTHCHECK --interval=30s --timeout=5s --start-period=20s --retries=3 \
    CMD curl -fsS http://127.0.0.1:3000/api/health >/dev/null || exit 1

CMD ["dogn3"]
