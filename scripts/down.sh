#!/usr/bin/env bash
# Tear down the RRE stack. Pass --volumes (or -v) to also drop the postgres
# named volume (rre-pg-data) for a clean slate.
#
# Usage:  ./scripts/down.sh [--volumes]   (or `make down` / `make down-clean`)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COMPOSE_FILE="$ROOT/infra/docker-compose.yml"
ENV_FILE="$ROOT/infra/.env"
ENV_EXAMPLE="$ROOT/infra/.env.example"

# compose needs the env vars to interpolate even on `down`; fall back to the
# template if a local .env has not been created yet.
if [[ ! -f "$ENV_FILE" ]]; then
  ENV_FILE="$ENV_EXAMPLE"
fi

DOWN_ARGS=(--remove-orphans)
if [[ "${1:-}" == "--volumes" || "${1:-}" == "-v" ]]; then
  echo "[down] removing volumes (rre-pg-data will be dropped)"
  DOWN_ARGS+=(--volumes)
fi

docker compose --env-file "$ENV_FILE" -f "$COMPOSE_FILE" down "${DOWN_ARGS[@]}"
echo "[down] stack stopped."
