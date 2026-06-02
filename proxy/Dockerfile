# Multi-stage build for the RRE proxy.
#
# The build context MUST be the repo root (the proxy depends on ../core/* via
# path dependencies), e.g.:
#   docker build -f proxy/Dockerfile -t rre-proxy .
#
# Base images pin the major version only (never :latest).
FROM rust:1-slim AS builder
WORKDIR /src

# build-essential + cmake: the C toolchain for QuickJS (rquickjs) and the rustls
# crypto backend (ring / aws-lc-rs) compiled transitively via zen-engine.
RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config build-essential cmake \
    && rm -rf /var/lib/apt/lists/*

# The core/* crates inherit their deps from the root workspace, so the root
# Cargo.toml must be present for them to resolve. proxy/ carries its own empty
# [workspace] table, so it stays a standalone crate detached from this root
# workspace.
COPY Cargo.toml ./Cargo.toml

# zen path deps the proxy builds against + the proxy crate itself.
COPY core ./core
COPY proxy ./proxy

WORKDIR /src/proxy
RUN cargo build --release --bin rre-proxy

# ----------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime
WORKDIR /app

# ca-certificates for outbound TLS; curl for the container healthcheck.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10002 --user-group --no-create-home rre

COPY --from=builder /src/proxy/target/release/rre-proxy /usr/local/bin/rre-proxy
# Runtime config: layered JSON settings (default.json + {APP_ENV}.json) plus the
# feature map + sanitizer allow-list YAML. Resolved relative to the working dir
# (/app/proxy/config), so preserve that layout.
COPY proxy/config ./proxy/config

ENV PROXY_BIND_ADDR=0.0.0.0:9000
EXPOSE 9000

USER rre

HEALTHCHECK --interval=10s --timeout=3s --start-period=15s --retries=5 \
    CMD curl -fsS http://localhost:9000/health || exit 1

CMD ["rre-proxy"]
