FROM ubuntu:24.04

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

RUN useradd --system --create-home --home-dir /app --shell /usr/sbin/nologin dogn3

WORKDIR /app

COPY target/release/dogn3 /usr/local/bin/dogn3
COPY static ./static

RUN mkdir -p /app/images \
    && chown -R dogn3:dogn3 /app

USER dogn3

EXPOSE 3000

HEALTHCHECK --interval=30s --timeout=5s --start-period=20s --retries=3 \
    CMD curl -fsS http://127.0.0.1:3000/api/health >/dev/null || exit 1

CMD ["dogn3"]
