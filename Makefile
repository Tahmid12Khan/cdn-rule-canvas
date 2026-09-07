# RRE developer entrypoints. One-command boot:  make up
#
# `make up` builds all images, starts the stack, waits for the backend, and
# seeds the demo feature. `make down` stops it; `make down-clean` also drops the
# postgres volume.

COMPOSE_FILE := infra/docker-compose.yml
ENV_FILE     := infra/.env
COMPOSE      := docker compose --env-file $(ENV_FILE) -f $(COMPOSE_FILE)

.DEFAULT_GOAL := help

.PHONY: help up down down-clean dev seed logs ps restart \
        backend-check proxy-check frontend-check check

help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) \
		| awk 'BEGIN {FS = ":.*?## "} {printf "  \033[36m%-16s\033[0m %s\n", $$1, $$2}'

up: ## Build + start the full stack, then seed demo data (one-command boot)
	@./scripts/up.sh

down: ## Stop the stack (keeps the postgres volume)
	@./scripts/down.sh

down-clean: ## Stop the stack AND drop the postgres volume (rre-pg-data)
	@./scripts/down.sh --volumes

dev: ## Local fast loop: db+upstream in docker, backend/frontend/proxy native (frees ports first; Ctrl-C stops them)
	@./scripts/dev.sh

seed: ## Re-run the idempotent demo seeder against the running backend
	@$(COMPOSE) exec -T backend seed_demo

logs: ## Tail logs for all services
	@$(COMPOSE) logs -f

ps: ## Show service status
	@$(COMPOSE) ps

restart: ## Recreate and restart all services
	@$(COMPOSE) up --build -d

# --- local quality gates ------------------------------------------------------

rre-core-check: ## fmt + clippy + test + wasm build the shared rule crate
	cd rre-core && cargo fmt --check && cargo clippy --all-targets -- -D warnings \
		&& cargo test && cargo build --target wasm32-wasip1

backend-check: ## fmt + clippy + test + build the backend crate
	cd backend && cargo fmt --check && cargo clippy --all-targets -- -D warnings \
		&& cargo test && cargo build --release

proxy-check: ## fmt + clippy + test + build the proxy crate
	cd proxy && cargo fmt --check && cargo clippy --all-targets -- -D warnings \
		&& cargo test && cargo build --release

frontend-check: ## lint + typecheck + test + build the frontend
	cd frontend && npm ci && npm run lint && npm run typecheck \
		&& npm run test && npm run build

check: rre-core-check backend-check proxy-check frontend-check ## Run every service's quality gate
