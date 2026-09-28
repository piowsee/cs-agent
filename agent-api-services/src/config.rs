//! Runtime configuration loaded from environment variables.

use anyhow::Context as _;

/// Fully resolved service configuration.
///
/// Construct with [`Config::from_env`], which reads (and validates) the process
/// environment. A `.env` file is loaded by [`crate::run`] before this is called.
#[derive(Debug, Clone)]
pub struct Config {
    /// PostgreSQL connection string (`postgres://user:pass@host/db`).
    pub database_url: String,
    /// Secret presented by the administrator in the `x-admin-key` header.
    pub admin_api_key: String,
    /// Socket address the HTTP server binds to (e.g. `0.0.0.0:3000`).
    pub bind_addr: String,
    /// Maximum number of pooled database connections.
    pub db_max_connections: u32,
}

impl Config {
    /// Reads configuration from the process environment.
    ///
    /// `DATABASE_URL` and `ADMIN_API_KEY` are required; `BIND_ADDR`
    /// (default `0.0.0.0:3000`) and `DB_MAX_CONNECTIONS` (default `5`) are
    /// optional.
    ///
    /// # Errors
    ///
    /// Returns an error if a required variable is missing, if `ADMIN_API_KEY`
    /// is empty, or if `DB_MAX_CONNECTIONS` is set but is not a valid integer.
    pub fn from_env() -> anyhow::Result<Self> {
        Self::from_source(|key| std::env::var(key).ok())
    }

    /// Resolves configuration from an arbitrary lookup function.
    ///
    /// This is the testable core of [`Config::from_env`]; the closure returns
    /// the value for a variable name, or `None` if unset.
    ///
    /// # Errors
    ///
    /// Returns an error if a required variable is missing, if `ADMIN_API_KEY`
    /// is empty (an empty admin secret would let anyone authenticate as the
    /// administrator, so we fail fast), or if `DB_MAX_CONNECTIONS` is set but
    /// not a positive integer.
    fn from_source(get: impl Fn(&str) -> Option<String>) -> anyhow::Result<Self> {
        let database_url = get("DATABASE_URL").context("DATABASE_URL must be set")?;

        let admin_api_key = get("ADMIN_API_KEY").context("ADMIN_API_KEY must be set")?;
        anyhow::ensure!(
            !admin_api_key.trim().is_empty(),
            "ADMIN_API_KEY must not be empty"
        );

        let bind_addr = get("BIND_ADDR").unwrap_or_else(|| "0.0.0.0:3000".to_owned());

        let db_max_connections = match get("DB_MAX_CONNECTIONS") {
            Some(raw) => raw
                .parse()
                .context("DB_MAX_CONNECTIONS must be a positive integer")?,
            None => 5,
        };

        Ok(Self { database_url, admin_api_key, bind_addr, db_max_connections })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::Config;

    /// Builds a lookup closure over a fixed set of key/value pairs.
    fn source(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> =
            pairs.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect();
        move |key| map.get(key).cloned()
    }

    #[test]
    fn resolves_required_fields_and_defaults() {
        let config = Config::from_source(source(&[
            ("DATABASE_URL", "postgres://localhost/db"),
            ("ADMIN_API_KEY", "secret"),
        ]))
        .expect("valid config");

        assert_eq!(config.database_url, "postgres://localhost/db");
        assert_eq!(config.admin_api_key, "secret");
        assert_eq!(config.bind_addr, "0.0.0.0:3000");
        assert_eq!(config.db_max_connections, 5);
    }

    #[test]
    fn honors_optional_overrides() {
        let config = Config::from_source(source(&[
            ("DATABASE_URL", "postgres://localhost/db"),
            ("ADMIN_API_KEY", "secret"),
            ("BIND_ADDR", "127.0.0.1:8080"),
            ("DB_MAX_CONNECTIONS", "20"),
        ]))
        .expect("valid config");

        assert_eq!(config.bind_addr, "127.0.0.1:8080");
        assert_eq!(config.db_max_connections, 20);
    }

    #[test]
    fn rejects_missing_database_url() {
        let err = Config::from_source(source(&[("ADMIN_API_KEY", "secret")]))
            .expect_err("should fail");
        assert!(err.to_string().contains("DATABASE_URL"));
    }

    #[test]
    fn rejects_missing_admin_key() {
        let err = Config::from_source(source(&[("DATABASE_URL", "postgres://x/db")]))
            .expect_err("should fail");
        assert!(err.to_string().contains("ADMIN_API_KEY"));
    }

    #[test]
    fn rejects_blank_admin_key() {
        let err = Config::from_source(source(&[
            ("DATABASE_URL", "postgres://x/db"),
            ("ADMIN_API_KEY", "   "),
        ]))
        .expect_err("should fail");
        assert!(err.to_string().contains("must not be empty"));
    }

    #[test]
    fn rejects_invalid_connection_count() {
        let err = Config::from_source(source(&[
            ("DATABASE_URL", "postgres://x/db"),
            ("ADMIN_API_KEY", "secret"),
            ("DB_MAX_CONNECTIONS", "not-a-number"),
        ]))
        .expect_err("should fail");
        assert!(err.to_string().contains("DB_MAX_CONNECTIONS"));
    }
}
