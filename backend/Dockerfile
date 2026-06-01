# Multi-stage build for the RRE backend.
#
# The build context MUST be the repo root (the backend depends on ../core/engine
# via a path dependency), e.g.:
#   docker build -f backend/Dockerfile -t rre-backend .
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
# Cargo.toml must be present for them to resolve. Trim its `members` to the
# core/* crates actually copied into this context (bindings/* and playground are
# not). backend/ carries its own empty [workspace] table, so it stays a
# standalone crate detached from this root workspace.
COPY Cargo.toml ./Cargo.toml
RUN sed -i '/"bindings\/\*"/d; /"playground"/d' Cargo.toml

# zen path deps the backend builds against + the backend crate itself.
COPY core ./core
COPY backend ./backend

WORKDIR /src/backend
# Build the server + the demo seeder binary (release).
RUN cargo build --release --bin rre-backend --bin seed_demo

# ----------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime
WORKDIR /app

# ca-certificates for outbound TLS; curl for the container healthcheck.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --user-group --no-create-home rre

COPY --from=builder /src/backend/target/release/rre-backend /usr/local/bin/rre-backend
COPY --from=builder /src/backend/target/release/seed_demo /usr/local/bin/seed_demo

# Layered JSON config (config/default.json, config/{APP_ENV}.json) and the
# node-type manifest (config/node_types.json) are read at startup relative to
# the working dir, so the config/ dir must be present in the runtime image.
COPY --from=builder /src/backend/config /app/config

# Migrations are embedded in the binary via sqlx::migrate! and run at startup;
# no migration files are needed on disk at runtime.

ENV BIND_ADDR=0.0.0.0:8000
EXPOSE 8000

USER rre

HEALTHCHECK --interval=10s --timeout=3s --start-period=20s --retries=5 \
    CMD curl -fsS http://localhost:8000/health || exit 1

CMD ["rre-backend"]
