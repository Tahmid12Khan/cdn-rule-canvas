#!/usr/bin/env bash
# Boot the full RRE stack (postgres, backend, frontend, proxy, demo-upstream,
# adminer), wait for the backend to become healthy, then seed the demo feature.
#
# Usage:  ./scripts/up.sh        (or `make up`)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COMPOSE_FILE="$ROOT/infra/docker-compose.yml"
ENV_FILE="$ROOT/infra/.env"
ENV_EXAMPLE="$ROOT/infra/.env.example"

# Seed a local .env from the template on first run.
if [[ ! -f "$ENV_FILE" ]]; then
  echo "[up] infra/.env not found — copying from .env.example"
  cp "$ENV_EXAMPLE" "$ENV_FILE"
fi

compose() {
  docker compose --env-file "$ENV_FILE" -f "$COMPOSE_FILE" "$@"
}

echo "[up] building and starting the stack..."
compose up --build -d

echo "[up] waiting for the backend to become healthy..."
for _ in $(seq 1 60); do
  status="$(compose ps backend --format '{{.Health}}' 2>/dev/null || true)"
  if [[ "$status" == "healthy" ]]; then
    break
  fi
  sleep 2
done

echo "[up] seeding demo data (idempotent)..."
compose exec -T backend seed_demo

cat <<'EOF'

[up] stack is up:
  frontend       http://localhost:3000
  backend (API)  http://localhost:8000        (docs: /docs)
  proxy (demo)   http://localhost:9000/article.html
  adminer        http://localhost:8080
  demo-upstream  http://localhost:9001/article.html

Tear down with:  ./scripts/down.sh   (or `make down`)
EOF
