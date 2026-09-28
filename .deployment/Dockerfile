# syntax=docker/dockerfile:1

# ---- builder ---------------------------------------------------------------
# Compiles the `agent-api-services` binary in release mode. No database is
# needed at build time: queries are checked at runtime, and the migrations are
# embedded into the binary by `sqlx::migrate!` at compile time.
#
# The build context is the workspace root (see docker-compose.yml `context: ..`),
# which is required: the crate inherits `[lints]`, edition, and rust-version from
# the workspace Cargo.toml and is pinned by the root Cargo.lock.
FROM rust:1-slim-bookworm AS builder
WORKDIR /build

# Copy the whole workspace so Cargo can resolve it, then build only the API
# crate (sibling members are not compiled unless they are dependencies).
COPY . .
RUN cargo build --release -p agent-api-services

# ---- runtime ---------------------------------------------------------------
# Minimal image containing just the binary and the certs it may need.
FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --no-create-home --uid 10001 appuser

COPY --from=builder /build/target/release/agent-api-services /usr/local/bin/agent-api-services

USER appuser
EXPOSE 3000

# `run()` applies any pending migrations on startup, then serves.
CMD ["agent-api-services"]
