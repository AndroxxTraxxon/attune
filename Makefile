.PHONY: help build test clean run-api run-executor run-worker run-sensor run-notifier \
        check fmt clippy install-tools db-create db-migrate db-reset docker-build \
        docker-up docker-down docker-cache-warm docker-stop-system-services dev watch generate-agents-index \
        docker-build-workers docker-build-worker-base docker-build-worker-python \
        docker-build-worker-node docker-build-worker-full deny ci-rust ci-web-blocking ci-web-advisory \
        ci-security-blocking ci-security-advisory ci-blocking ci-advisory \
        fmt-check pre-commit install-git-hooks

# Default target
help:
	@echo "Attune Development Commands"
	@echo "==========================="
	@echo ""
	@echo "Building:"
	@echo "  make build          - Build all services"
	@echo "  make build-release  - Build all services in release mode"
	@echo "  make clean          - Clean build artifacts"
	@echo ""
	@echo "Testing:"
	@echo "  make test           - Run all tests"
	@echo "  make test-common    - Run tests for common library"
	@echo "  make test-api       - Run tests for API service"
	@echo "  make test-integration     - Run integration tests (common + API)"
	@echo "  make test-integration-api - Run API integration tests (requires DB)"
	@echo "  make check          - Check code without building"
	@echo ""
	@echo "Code Quality:"
	@echo "  make fmt            - Format all code"
	@echo "  make fmt-check      - Verify formatting without changing files"
	@echo "  make clippy         - Run linter"
	@echo "  make lint           - Run both fmt and clippy"
	@echo "  make deny           - Run cargo-deny checks"
	@echo "  make pre-commit     - Run the git pre-commit checks locally"
	@echo "  make install-git-hooks - Configure git to use the repo hook scripts"
	@echo ""
	@echo "Running Services:"
	@echo "  make run-api        - Run API service"
	@echo "  make run-executor   - Run executor service"
	@echo "  make run-worker     - Run worker service"
	@echo "  make run-sensor     - Run sensor service"
	@echo "  make run-notifier   - Run notifier service"
	@echo "  make dev            - Run all services in development mode"
	@echo ""
	@echo "Database:"
	@echo "  make db-create      - Create database"
	@echo "  make db-migrate     - Run migrations"
	@echo "  make db-reset       - Drop and recreate database"
	@echo "  make db-test-setup  - Setup test database"
	@echo "  make db-test-reset  - Reset test database"
	@echo ""
	@echo "Docker (Port conflicts? Run 'make docker-stop-system-services' first):"
	@echo "  make docker-stop-system-services - Stop system PostgreSQL/RabbitMQ/Redis"
	@echo "  make docker-cache-warm           - Pre-load build cache (prevents races)"
	@echo "  make docker-build                - Build Docker images"
	@echo "  make docker-build-workers        - Build all worker variants"
	@echo "  make docker-build-worker-base    - Build base worker (shell only)"
	@echo "  make docker-build-worker-python  - Build Python worker"
	@echo "  make docker-build-worker-node    - Build Node.js worker"
	@echo "  make docker-build-worker-full    - Build full worker (all runtimes)"
	@echo "  make docker-up                   - Start services with docker compose"
	@echo "  make docker-down                 - Stop services"
	@echo ""
	@echo "Development:"
	@echo "  make watch          - Watch and rebuild on changes"
	@echo "  make install-tools  - Install development tools"
	@echo ""
	@echo "Documentation:"
	@echo "  make generate-agents-index - Generate AGENTS.md index for AI agents"
	@echo ""

# Increase rustc stack size to prevent SIGSEGV during compilation
export RUST_MIN_STACK := 16777216

# Building
build:
	cargo build

build-release:
	cargo build --release

clean:
	cargo clean

# Testing
test:
	cargo test

test-common:
	cargo test -p attune-common

test-api:
	cargo test -p attune-api

test-verbose:
	cargo test -- --nocapture --test-threads=1

test-integration: test-integration-api
	@echo "Setting up test database..."
	@make db-test-setup
	@echo "Running common integration tests..."
	cargo test --test '*' -p attune-common -- --test-threads=1
	@echo "Integration tests complete"

test-integration-api:
	@echo "Running API integration tests..."
	cargo test -p attune-api -- --ignored --test-threads=1
	@echo "API integration tests complete"

test-with-db: db-test-setup test-integration
	@echo "All tests with database complete"

# Code quality
check:
	cargo check --all-features

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all -- --check

clippy:
	cargo clippy --all-features -- -D warnings

lint: fmt clippy

# Running services
run-api:
	cargo run --bin attune-api

run-api-release:
	cargo run --bin attune-api --release

run-executor:
	cargo run --bin attune-executor

run-executor-release:
	cargo run --bin attune-executor --release

run-worker:
	cargo run --bin attune-worker

run-worker-release:
	cargo run --bin attune-worker --release

run-sensor:
	cargo run --bin attune-sensor

run-sensor-release:
	cargo run --bin attune-sensor --release

run-notifier:
	cargo run --bin attune-notifier

run-notifier-release:
	cargo run --bin attune-notifier --release

# Development mode (run all services)
dev:
	@echo "Starting all services in development mode..."
	@echo "Note: Run each service in a separate terminal or use docker compose"
	@echo ""
	@echo "Terminal 1: make run-api"
	@echo "Terminal 2: make run-executor"
	@echo "Terminal 3: make run-worker"
	@echo "Terminal 4: make run-sensor"
	@echo "Terminal 5: make run-notifier"

# Watch for changes and rebuild
watch:
	cargo watch -x check -x test -x build

# Database operations
db-create:
	createdb attune || true

db-migrate:
	sqlx migrate run

db-drop:
	dropdb attune || true

db-reset: db-drop db-create db-migrate
	@echo "Database reset complete"

# Test database operations
db-test-create:
	psql postgresql://postgres:postgres@localhost:5432 -c "CREATE DATABASE attune_test"

db-test-migrate:
	DATABASE_URL=postgresql://postgres:postgres@localhost:5432/attune_test sqlx migrate run

db-test-drop:
	psql postgresql://postgres:postgres@localhost:5432 -c "DROP DATABASE attune_test"

db-test-reset: db-test-drop db-test-create db-test-migrate
	@echo "Test database reset complete"

db-test-setup: db-test-create db-test-migrate
	@echo "Test database setup complete"

# Docker operations

# Stop system services that conflict with Docker Compose
# This resolves "address already in use" errors for PostgreSQL (5432), RabbitMQ (5672), Redis (6379)
docker-stop-system-services:
	@echo "Stopping system services that conflict with Docker..."
	@./scripts/stop-system-services.sh

# Pre-warm the build cache by building one service first
# This prevents race conditions when building multiple services in parallel
# The first build populates the shared cargo registry/git cache
docker-cache-warm:
	@echo "Warming up build cache (building API service first)..."
	@echo "This prevents race conditions during parallel builds."
	docker compose build api
	@echo ""
	@echo "Cache warmed! Now you can safely run 'make docker-build' for parallel builds."

docker-build:
	@echo "Building Docker images..."
	docker compose build

docker-build-api:
	docker compose build api

docker-build-web:
	docker compose build web

# Build worker images
docker-build-workers: docker-build-worker-base docker-build-worker-python docker-build-worker-node docker-build-worker-full
	@echo "✅ All worker images built successfully"

docker-build-worker-base:
	@echo "Building base worker (shell only)..."
	DOCKER_BUILDKIT=1 docker build --target worker-base -t attune-worker:base -f docker/Dockerfile.worker .
	@echo "✅ Base worker image built: attune-worker:base"

docker-build-worker-python:
	@echo "Building Python worker (shell + python)..."
	DOCKER_BUILDKIT=1 docker build --target worker-python -t attune-worker:python -f docker/Dockerfile.worker .
	@echo "✅ Python worker image built: attune-worker:python"

docker-build-worker-node:
	@echo "Building Node.js worker (shell + node)..."
	DOCKER_BUILDKIT=1 docker build --target worker-node -t attune-worker:node -f docker/Dockerfile.worker .
	@echo "✅ Node.js worker image built: attune-worker:node"

docker-build-worker-full:
	@echo "Building full worker (all runtimes)..."
	DOCKER_BUILDKIT=1 docker build --target worker-full -t attune-worker:full -f docker/Dockerfile.worker .
	@echo "✅ Full worker image built: attune-worker:full"

docker-up:
	@echo "Starting all services with Docker Compose..."
	docker compose up -d

docker-down:
	@echo "Stopping all services..."
	docker compose down

docker-down-volumes:
	@echo "Stopping all services and removing volumes (WARNING: deletes data)..."
	docker compose down -v

docker-restart:
	docker compose restart

docker-logs:
	docker compose logs -f

docker-logs-api:
	docker compose logs -f api

docker-ps:
	docker compose ps

docker-shell-api:
	docker compose exec api /bin/sh

docker-shell-db:
	docker compose exec postgres psql -U attune

docker-clean:
	@echo "Cleaning up Docker resources..."
	docker compose down -v --rmi local
	docker system prune -f

# Install development tools
install-tools:
	@echo "Installing development tools..."
	cargo install cargo-watch
	cargo install cargo-expand
	cargo install sqlx-cli --no-default-features --features postgres
	@echo "Tools installed successfully"

# Setup environment
setup: install-tools
	@echo "Setting up development environment..."
	@if [ ! -f .env ]; then \
		echo "Creating .env file from .env.example..."; \
		cp .env.example .env; \
		echo "⚠️  Please edit .env and update configuration values"; \
	fi
	@if [ ! -f .env.test ]; then \
		echo ".env.test already exists"; \
	fi
	@echo "Setup complete! Run 'make db-create && make db-migrate' to initialize the database."
	@echo "For testing, run 'make db-test-setup' to initialize the test database."

# Documentation
docs:
	cargo doc --no-deps --open

# Generate AGENTS.md index
generate-agents-index:
	@echo "Generating AGENTS.md index..."
	python3 scripts/generate_agents_md_index.py
	@echo "✅ AGENTS.md generated successfully"

# Benchmarks
bench:
	cargo bench

# Coverage
coverage:
	cargo tarpaulin --out Html --output-dir coverage

# Update dependencies
update:
	cargo update

# Audit dependencies for security issues (ignores configured in deny.toml)
audit:
	cargo deny check advisories

deny:
	cargo deny check

ci-rust:
	cargo fmt --all -- --check
	cargo clippy --workspace --all-targets --all-features -- -D warnings
	cargo test --workspace --all-features
	cargo deny check

ci-web-blocking:
	cd web && npm ci
	cd web && npm run lint
	cd web && npm run typecheck
	cd web && npm run build

ci-web-pre-commit:
	cd web && npm ci
	cd web && npm run lint
	cd web && npm run typecheck

ci-web-advisory:
	cd web && npm ci
	cd web && npm run knip
	cd web && npm audit --omit=dev

ci-security-blocking:
	mkdir -p $$HOME/bin
	GITLEAKS_VERSION="8.24.2"; \
	ARCH="$$(uname -m)"; \
	case "$$ARCH" in \
		x86_64) ARCH="x64" ;; \
		aarch64|arm64) ARCH="arm64" ;; \
		*) echo "Unsupported architecture: $$ARCH"; exit 1 ;; \
	esac; \
	curl -sSfL \
		-o /tmp/gitleaks.tar.gz \
		"https://github.com/gitleaks/gitleaks/releases/download/v$$GITLEAKS_VERSION/gitleaks_$$GITLEAKS_VERSION"_linux_"$$ARCH".tar.gz; \
	tar -xzf /tmp/gitleaks.tar.gz -C $$HOME/bin gitleaks; \
	chmod +x $$HOME/bin/gitleaks
	$$HOME/bin/gitleaks git --report-format sarif --report-path gitleaks.sarif --config .gitleaks.toml

ci-security-advisory:
	pip install semgrep
	semgrep scan --config p/default --error

ci-blocking: ci-rust ci-web-blocking ci-security-blocking
	@echo "✅ Blocking CI checks passed!"

ci-advisory: ci-web-advisory ci-security-advisory
	@echo "Advisory CI checks complete."

# Check dependency tree
tree:
	cargo tree

# Generate licenses list
licenses:
	cargo license --json > licenses.json
	@echo "License information saved to licenses.json"

# Blocking checks run by the git pre-commit hook after formatting.
# Keep the local web step fast; full production builds stay in CI.
pre-commit: deny ci-web-pre-commit ci-security-blocking
	@echo "✅ Pre-commit checks passed."

install-git-hooks:
	git config core.hooksPath .githooks
	chmod +x .githooks/pre-commit
	@echo "✅ Git hooks configured to use .githooks/"

# CI simulation
ci: ci-blocking ci-advisory
	@echo "✅ CI checks passed!"
