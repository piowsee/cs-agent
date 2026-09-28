//! Shared application state passed to every handler and middleware.

use std::sync::Arc;

use sqlx::PgPool;

/// State cloned into each request by Axum.
///
/// Cloning is cheap: [`PgPool`] is an `Arc` internally, and the admin key is an
/// [`Arc<str>`].
#[derive(Debug, Clone)]
pub struct AppState {
    /// Connection pool used by the repository layer.
    pub pool: PgPool,
    /// Administrator secret, compared in constant time by the admin middleware.
    pub admin_api_key: Arc<str>,
}

impl AppState {
    /// Builds application state from a pool and the administrator secret.
    #[must_use]
    pub fn new(pool: PgPool, admin_api_key: &str) -> Self {
        Self {
            pool,
            admin_api_key: Arc::from(admin_api_key),
        }
    }
}
