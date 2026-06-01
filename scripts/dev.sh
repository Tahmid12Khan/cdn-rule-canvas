#!/usr/bin/env bash
# Local fast-debug loop: db + demo-upstream in docker, backend/frontend/proxy
# NATIVELY (cargo run / npm run dev) for fast edit-rebuild cycles.
#
# Re-runnable: frees the native ports (3000/8000/9000) first, so a second run
# kills the previous one. Ctrl-C stops the native services it started; the
# docker db + upstream are left running (stop them with `make down`).
#
# Usage:  ./scripts/dev.sh [--seed]      (or `make dev`)
#   --seed   run the idempotent demo seeder (seed_demo -> feature `dn-article`)
#            once the backend is healthy.
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COMPOSE_FILE="$ROOT/infra/docker-compose.yml"
ENV_FILE="$ROOT/infra/.env"
ENV_EXAMPLE="$ROOT/infra/.env.example"
LOG_DIR="$ROOT/.devlogs"

# cargo is at ~/.cargo/bin but not on PATH in non-interactive shells.
export PATH="$HOME/.cargo/bin:$PATH"

SEED=0
[[ "${1:-}" == "--seed" ]] && SEED=1

compose() {
  docker compose --env-file "$ENV_FILE" -f "$COMPOSE_FILE" "$@"
}

# Free a host port owned by a DOCKER container from ANOTHER project by stopping
# that container. Killing the host PID on a *published* port would hit the
# Docker backend (com.docker.backend), not the container — so we stop the
# container instead. Never stops our own `keep` container.
free_docker_port() {
  local port=$1 keep=${2:-} name
  for name in $(docker ps --filter "publish=$port" --format '{{.Names}}' 2>/dev/null || true); do
    [[ "$name" == "$keep" ]] && continue
    echo "[dev] stopping container '$name' to free port $port"
    docker stop "$name" >/dev/null 2>&1 || true
  done
}

# Wait until a URL answers (any HTTP response = listening), up to `tries` * 2s.
# Returns 0 as soon as the connection succeeds, 1 on timeout.
wait_up() {
  local url=$1 tries=${2:-120} code
  for _ in $(seq 1 "$tries"); do
    code="$(curl -s -o /dev/null -w '%{http_code}' --max-time 2 "$url" 2>/dev/null || echo 000)"
    [[ "$code" != "000" ]] && return 0
    sleep 2
  done
  return 1
}

# Free a TCP port by killing whatever listens on it (graceful, then -9).
kill_port() {
  local port=$1 pids
  pids="$(lsof -ti "tcp:$port" -sTCP:LISTEN 2>/dev/null || true)"
  if [[ -n "$pids" ]]; then
    echo "[dev] freeing port $port (pids: $pids)"
    # shellcheck disable=SC2086
    kill $pids 2>/dev/null || true
    sleep 1
    pids="$(lsof -ti "tcp:$port" -sTCP:LISTEN 2>/dev/null || true)"
    # shellcheck disable=SC2086
    [[ -n "$pids" ]] && kill -9 $pids 2>/dev/null || true
  fi
}

PIDS=()
cleanup() {
  trap - INT TERM EXIT
  echo
  echo "[dev] stopping native services (backend/frontend/proxy)..."
  for pid in "${PIDS[@]:-}"; do
    [[ -n "$pid" ]] && kill "$pid" 2>/dev/null || true
  done
  # cargo/npm wrappers may leave the real listener behind — free the ports too.
  kill_port 8000
  kill_port 3000
  kill_port 9000
  echo "[dev] native services stopped. docker db + upstream still running (\`make down\` to stop)."
}
trap cleanup INT TERM EXIT

# --- env files (seed from templates on first run) -----------------------------
[[ -f "$ENV_FILE" ]]               || { echo "[dev] infra/.env not found — copying from .env.example"; cp "$ENV_EXAMPLE" "$ENV_FILE"; }
[[ -f "$ROOT/backend/.env" ]]      || { echo "[dev] backend/.env not found — copying from .env.example"; cp "$ROOT/backend/.env.example" "$ROOT/backend/.env"; }
[[ -f "$ROOT/frontend/.env.local" ]] || { echo "[dev] frontend/.env.local not found — copying from .env.local.example"; cp "$ROOT/frontend/.env.local.example" "$ROOT/frontend/.env.local"; }

mkdir -p "$LOG_DIR"

# --- free native ports before (re)starting ------------------------------------
echo "[dev] clearing native ports 3000/8000/9000..."
kill_port 8000
kill_port 3000
kill_port 9000

# --- docker: db + demo-upstream -----------------------------------------------
# Free the published ports from any foreign container first, so compose can bind
# them (host 5432 = postgres, host 9001 = demo-upstream).
free_docker_port 5432 rre-postgres-1
free_docker_port 9001 rre-demo-upstream-1

echo "[dev] starting postgres + demo-upstream (docker)..."
if ! compose up -d postgres demo-upstream; then
  echo "[dev] ERROR: compose up failed — a published port is still held. Current owners:"
  docker ps --filter publish=5432 --format '  {{.Names}}  {{.Ports}}' 2>/dev/null || true
  lsof -nP -iTCP:5432 -sTCP:LISTEN 2>/dev/null || true
  exit 1
fi

ALL_OK=1

echo "[dev] waiting for postgres..."
PG_OK=0
for _ in $(seq 1 30); do
  if compose exec -T postgres pg_isready -U rre -d rre >/dev/null 2>&1; then
    PG_OK=1
    break
  fi
  sleep 1
done
[[ "$PG_OK" == 1 ]] || { echo "[dev] postgres did NOT become ready"; ALL_OK=0; }

# --- start the native services (backend+proxy compile concurrently) -----------
# backend: embedded sqlx::migrate! runs at startup.
echo "[dev] starting backend  -> http://localhost:8000  (log: .devlogs/backend.log)"
( cd "$ROOT/backend" && exec cargo run --bin rre-backend ) >"$LOG_DIR/backend.log" 2>&1 &
PIDS+=("$!")

# proxy: the two env overrides are REQUIRED outside compose.
echo "[dev] starting proxy    -> http://localhost:9000/article.html  (log: .devlogs/proxy.log)"
( cd "$ROOT" && exec env UPSTREAM_BASE_URL=http://localhost:9001 BACKEND_BASE_URL=http://localhost:8000 \
    cargo run --manifest-path proxy/Cargo.toml ) >"$LOG_DIR/proxy.log" 2>&1 &
PIDS+=("$!")

echo "[dev] starting frontend -> http://localhost:3000  (log: .devlogs/frontend.log)"
( cd "$ROOT/frontend" && exec npm run dev ) >"$LOG_DIR/frontend.log" 2>&1 &
PIDS+=("$!")

# --- wait for everything to answer (builds happen here; tail .devlogs/*) ------
echo "[dev] waiting for services to come up (first run compiles backend + proxy)..."
wait_up "http://localhost:9001/article.html" 30  || { echo "[dev] demo-upstream not responding (:9001)"; ALL_OK=0; }
wait_up "http://localhost:8000/health"       180 || { echo "[dev] backend not healthy (:8000)";        ALL_OK=0; }

if [[ "$SEED" == "1" && "$ALL_OK" == 1 ]]; then
  echo "[dev] seeding demo data (seed_demo)..."
  ( cd "$ROOT/backend" && cargo run --bin seed_demo ) >"$LOG_DIR/seed.log" 2>&1 \
    && echo "[dev] seed complete (feature dn-article)." \
    || echo "[dev] seed failed — see .devlogs/seed.log"
fi

wait_up "http://localhost:9000/article.html" 180 || { echo "[dev] proxy not responding (:9000)";    ALL_OK=0; }
wait_up "http://localhost:3000"              180 || { echo "[dev] frontend not responding (:3000)"; ALL_OK=0; }

if [[ "$ALL_OK" == 1 ]]; then
  cat <<'EOF'

============================================================
  ✅  All services are up and running!
      Go to  http://localhost:3000/
============================================================
EOF
else
  echo
  echo "[dev] ⚠ NOT all services came up — inspect .devlogs/*.log (tailing below)."
fi

echo "[dev] Tailing logs — Ctrl-C stops backend/frontend/proxy (docker db + upstream stay up)."

# Stream all three native logs until Ctrl-C; cleanup() handles teardown.
tail -n +1 -F "$LOG_DIR/backend.log" "$LOG_DIR/proxy.log" "$LOG_DIR/frontend.log"
