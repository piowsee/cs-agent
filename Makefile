# Makefile for the cs-agent workspace (monorepo).
#
# Thin wrappers over the common cargo workflows, applied across ALL workspace
# members (agent-api-services, agent-workflows, agent-tools) at once. Run
# `make help` to list targets.
#
# Requires `make` and the Rust toolchain. On Windows, run it from the dev
# container, WSL, or Git Bash — the recipes are POSIX sh.

CARGO ?= cargo

# Used by `test` and `migrate`. Override to point at your database, e.g.
#   make test DATABASE_URL=postgres://postgres:pw@localhost:5432/agent_api
DATABASE_URL ?= postgres://postgres:pw@localhost:5432/agent_api
export DATABASE_URL

.DEFAULT_GOAL := help

.PHONY: help
help: ## List the available targets
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) \
		| awk 'BEGIN {FS = ":.*?## "} {printf "  \033[36m%-12s\033[0m %s\n", $$1, $$2}'

.PHONY: deps
deps: ## Fetch every workspace dependency (offline-ready afterwards)
	$(CARGO) fetch --locked

.PHONY: build
build: ## Build all workspace crates (debug, all targets/features)
	$(CARGO) build --workspace --all-targets --all-features

.PHONY: release
release: ## Build all workspace crates (optimized)
	$(CARGO) build --workspace --release

.PHONY: check
check: ## Type-check the whole workspace without emitting binaries
	$(CARGO) check --workspace --all-targets --all-features

.PHONY: fmt
fmt: ## Format all crates
	$(CARGO) fmt --all

.PHONY: fmt-check
fmt-check: ## Verify formatting without changing files (CI-style)
	$(CARGO) fmt --all --check

.PHONY: lint
lint: ## Clippy across the workspace, warnings denied
	$(CARGO) clippy --workspace --all-targets --all-features -- -D warnings

.PHONY: test
test: ## Run the workspace test suite (needs a reachable DATABASE_URL)
	$(CARGO) test --workspace --all-features

.PHONY: doc
doc: ## Build rustdoc for the workspace (no external deps)
	$(CARGO) doc --workspace --no-deps

.PHONY: migrate
migrate: ## Apply pending DB migrations (run against Postgres directly)
	sqlx migrate run --source agent-api-services/migrations

.PHONY: ci
ci: fmt-check lint build test ## Run the full CI sequence locally

.PHONY: clean
clean: ## Remove the target/ build tree
	$(CARGO) clean
