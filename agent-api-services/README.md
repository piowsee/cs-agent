# agent-api-services

An [Axum] HTTP API backed by PostgreSQL via [SQLx] (raw SQL, no ORM). A single
administrator — configured entirely through the `ADMIN_API_KEY` environment
variable, with **no user table** — manages API keys. Each key carries a
consumable *workflow quota* and an associated *vector store URL*.

## Design

- **Keys are hashed.** Only a SHA-256 hash and a short display prefix are stored.
  The plaintext key is returned **once**, at creation, and cannot be recovered.
- **Two middleware layers.** Admin routes require `x-admin-key` (compared to the
  env secret in constant time); client routes require a valid `x-api-key`.
- **Quota is atomic.** `POST /workflows` increments usage in a single conditional
  `UPDATE`, so concurrent callers can never exceed the limit.

## Endpoints

| Method | Path                | Auth        | Purpose                              |
|--------|---------------------|-------------|--------------------------------------|
| GET    | `/health`           | none        | Liveness probe                       |
| POST   | `/admin/keys`       | `x-admin-key` | Create a key (returns plaintext once) |
| GET    | `/admin/keys`       | `x-admin-key` | List keys                          |
| GET    | `/admin/keys/{id}`  | `x-admin-key` | Read a key                         |
| PATCH  | `/admin/keys/{id}`  | `x-admin-key` | Update limit / usage / vector URL  |
| DELETE | `/admin/keys/{id}`  | `x-admin-key` | Delete a key                       |
| GET    | `/me`               | `x-api-key` | The caller's own key metadata        |
| POST   | `/workflows`        | `x-api-key` | Run a workflow, consuming one unit of quota (`429` when exhausted) |

## Quick start (recommended dev loop)

Run **only the database** in Docker and the app on the host. A code change is
then a `cargo run` (seconds) instead of a container rebuild. The dev DB is
defined by [`docker-compose.yml`](./docker-compose.yml) in this crate — run the
Docker commands from the `agent-api-services/` directory.

```bash
# 1. Start just PostgreSQL (this compose file has only the `db` service).
docker compose up -d

# 2. Configure the environment (defaults already match the compose DB).
cp .env.example .env

# 3. Apply pending migrations by hand — the app does NOT migrate on startup.
#    (Installs sqlx-cli once; see "Database migrations" for details.)
sqlx migrate run

# 4. Run the app on the host.
cargo run -p agent-api-services
```

Edit code → `Ctrl-C` → `cargo run` again. The database (and its data) keeps
running in the container the whole time.

**Auto-reload (optional).** To skip the manual `Ctrl-C` / rerun, use
[`cargo-watch`], which recompiles and restarts the app on every save. This works
the same on the host and inside the dev container:

```bash
cargo install cargo-watch                      # once
cargo watch -x 'run -p agent-api-services'     # rebuild + restart on save
```

### Dev container (optional)

A [dev container](../.devcontainer/) is provided for VS Code / Codespaces. It is
a **self-contained** stack — its own PostgreSQL 18-alpine, a PgBouncer pooler, and
a `dev` service carrying the Rust toolchain — independent of the prod stack in
`../.deployment/` and of the db-only compose above. "Dev Containers: Reopen in
Container" brings all three up (Postgres and PgBouncer start in the background,
then `dev`); you then run the app inside with `cargo run -p agent-api-services`
(or `cargo watch`, above). `DATABASE_URL` (pointing at PgBouncer on `6432`, just
like prod), `ADMIN_API_KEY`, and `BIND_ADDR` are preset with hardcoded dev
credentials, so no `.env` setup is needed in the container.

Migrations are manual here too — apply them once against Postgres **directly**
(`db:5432`, not the pooler) before starting the app:

```bash
DATABASE_URL=postgres://postgres:pw@db:5432/agent_api sqlx migrate run
```

Your code is bind-mounted (not copied), so edits on the host appear inside the
container immediately — only the running binary needs a rerun (or `cargo watch`)
to pick them up; the container itself never rebuilds for a code change.

## Database migrations

Migrations live in [`migrations/`](./migrations) and are applied **manually**.
The service does **not** migrate on startup — you must bring the schema up to
date yourself before running the app, otherwise its queries will fail against an
empty database. (Tests are the exception: `#[sqlx::test]` provisions an isolated
database and applies the migrations automatically.)

Use [`sqlx-cli`], run from the `agent-api-services/` crate directory:

```bash
cargo install sqlx-cli --no-default-features --features rustls,postgres   # once
export DATABASE_URL=postgres://postgres:pw@localhost:5432/agent_api

sqlx migrate info     # show applied vs. pending migrations
sqlx migrate run      # apply all pending migrations

# Author a new migration (creates migrations/<timestamp>_add_widgets.sql):
sqlx migrate add add_widgets
```

Migrations are forward-only here (no `.down.sql` files), so `sqlx migrate
revert` is a no-op — add a paired down migration if you need reversibility.

## Docker

There are two compose files, for two purposes:

**Development — database only** ([`docker-compose.yml`](./docker-compose.yml),
this crate). Just PostgreSQL, so you run the app on the host (see Quick start).

```bash
# From agent-api-services/
docker compose up -d           # start the DB
docker compose logs -f db      # tail logs
docker compose down            # stop (data is preserved in the pgdata volume)
docker compose down -v         # stop AND delete the database volume
```

**Full stack — app + database** (`../.deployment/`). Builds the application
image (from `../.deployment/Dockerfile`) and runs it alongside the DB. Run from
the **repository root** so the build context is the workspace.

`POSTGRES_PASSWORD` and `ADMIN_API_KEY` are required (Compose refuses to start
without them). Provide them via a `.deployment/.env` file — Compose loads it
automatically:

```bash
# From the repository root
cp .deployment/.env.example .deployment/.env    # then fill in the two secrets
docker compose -f .deployment/docker-compose.yml up --build

# ...or pass them inline:
POSTGRES_PASSWORD=$(openssl rand -hex 16) ADMIN_API_KEY=$(openssl rand -hex 32) \
  docker compose -f .deployment/docker-compose.yml up --build
```

In the full stack the app connects to **PgBouncer** at `pgbouncer:6432` (which
pools to `postgres:5432`, user/db `postgres`); in dev the app runs on the host
against `localhost:5432` directly (db `agent_api`, per `.env.example`).

### PgBouncer

Both the prod stack and the dev container put a [PgBouncer] pooler in front of
Postgres in **transaction** pooling mode, so a small app-side pool fans out to
many client slots. Two SQLx-specific notes:

- **Prepared statements.** SQLx prepares and caches statements, which do not
  survive transaction-mode pooling on their own. `MAX_PREPARED_STATEMENTS=200`
  (PgBouncer ≥ 1.21) makes PgBouncer track them per client, so this works
  transparently. Without it you would see `prepared statement "…" does not
  exist` at runtime.
- **Migrations.** Migrations are applied by hand (see "Database migrations"), not
  on startup, so the app never takes the session-level advisory lock that
  `sqlx::migrate!` uses — which is unreliable through a transaction-mode pooler
  anyway. When you run `sqlx migrate run`, point `DATABASE_URL` at **Postgres
  directly** (`5432`), not at PgBouncer (`6432`); route only normal application
  queries through the pooler.

## Example usage

```bash
ADMIN=change-me-to-a-long-random-secret

# Create a key with a quota of 100 and a vector store URL.
curl -s -X POST localhost:3000/admin/keys \
  -H "x-admin-key: $ADMIN" -H 'content-type: application/json' \
  -d '{"name":"demo","workflow_limit":100,"vector_store_url":"https://vec/store"}'
# => { "api_key": "sk_...", "id": "...", ... }   # save api_key now!

KEY=sk_...   # the plaintext from above

curl -s localhost:3000/me       -H "x-api-key: $KEY"
curl -s -X POST localhost:3000/workflows -H "x-api-key: $KEY"
```

## Testing & coverage

Tests use `#[sqlx::test]`, which provisions an isolated database per test and
runs the migrations automatically, so a reachable `DATABASE_URL` is required.

```bash
cargo test -p agent-api-services

# Coverage (target >= 90%):
cargo install cargo-llvm-cov   # once
cargo llvm-cov -p agent-api-services
```

## Security notes

- API keys are bearer secrets — deploy behind TLS.
- Keys are stored only as hashes; a database leak does not expose usable keys.
- The admin secret is validated as non-empty at startup and never logged.

[Axum]: https://docs.rs/axum
[SQLx]: https://docs.rs/sqlx
[`sqlx-cli`]: https://crates.io/crates/sqlx-cli
[`cargo-watch`]: https://crates.io/crates/cargo-watch
[PgBouncer]: https://www.pgbouncer.org/
