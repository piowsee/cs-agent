//! API-key authentication service.
//!
//! A small [Axum] HTTP API backed by PostgreSQL via [SQLx] (raw SQL, no ORM).
//! A single administrator — configured entirely through the `ADMIN_API_KEY`
//! environment variable, with no user table — can create, read, update, and
//! delete API keys. Each key carries a consumable *workflow quota* and an
//! associated *vector store URL*.
//!
//! # Layout
//!
//! - [`config`] — environment configuration.
//! - [`models`] — request/response payloads and the database row type.
//! - [`error`] — the [`error::ApiError`] type and its HTTP mapping.
//! - [`build_router`] — assembles the router; [`run`] is the full entry point.
//!
//! # Authentication
//!
//! Two [middleware] layers guard the routes: the admin secret is compared in
//! constant time, and client keys are looked up by SHA-256 hash. Keys are never
//! stored in plaintext.
//!
//! [Axum]: https://docs.rs/axum
//! [SQLx]: https://docs.rs/sqlx
//! [middleware]: axum::middleware

pub mod config;
pub mod error;
pub mod models;

mod auth;
mod db;
mod key;
mod routes;
mod state;

use sqlx::postgres::PgPoolOptions;

pub use crate::routes::build_router;
pub use crate::state::AppState;

use crate::config::Config;

/// Runs the service: loads configuration, connects to PostgreSQL, and serves
/// HTTP until shutdown.
///
/// Migrations are **not** applied automatically. Apply pending migrations by
/// hand (`sqlx migrate run`) before starting the service — see the crate
/// README, "Database migrations".
///
/// # Errors
///
/// Returns an error if configuration is invalid, the database is unreachable,
/// or the listener cannot bind.
pub async fn run() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    init_tracing();

    let config = Config::from_env()?;

    let pool = PgPoolOptions::new()
        .max_connections(config.db_max_connections)
        .connect(&config.database_url)
        .await?;

    // Migrations are applied manually (see the README), not on startup, so the
    // app never takes the session-level advisory lock that `sqlx::migrate!`
    // would — which is unreliable through a transaction-mode pooler anyway.
    let state = AppState::new(pool, &config.admin_api_key);
    let listener = tokio::net::TcpListener::bind(&config.bind_addr).await?;
    tracing::info!(addr = %config.bind_addr, "server listening");
    axum::serve(listener, build_router(state)).await?;

    Ok(())
}

/// Initializes tracing once, honoring `RUST_LOG` and defaulting to `info`.
///
/// Uses `try_init` so repeated calls (e.g. across tests) are harmless.
fn init_tracing() {
    use tracing_subscriber::EnvFilter;

    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}
