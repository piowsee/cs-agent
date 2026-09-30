//! Authentication middleware.
//!
//! In Axum, a "middleware" is a function that runs on every request before the
//! handler. These are wired up with [`axum::middleware::from_fn_with_state`],
//! which — like a [`tower::Layer`] — wraps the inner service.

use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::Response;
use subtle::ConstantTimeEq as _;

use crate::error::ApiError;
use crate::models::ApiKeyRow;
use crate::state::AppState;
use crate::{db, key};

/// The authenticated key, inserted into request extensions by
/// [`api_key_auth`] and read by protected handlers via `Extension`.
#[derive(Debug, Clone)]
pub(crate) struct AuthenticatedKey(pub(crate) ApiKeyRow);

/// Header carrying the administrator secret.
const ADMIN_HEADER: &str = "x-admin-key";
/// Header carrying a client's API key.
const API_KEY_HEADER: &str = "x-api-key";

/// Guards the admin routes.
///
/// Requires an `x-admin-key` header equal to the configured `ADMIN_API_KEY`.
/// The comparison is constant-time to avoid leaking the secret through timing.
///
/// # Errors
///
/// [`ApiError::Unauthorized`] if the header is absent; [`ApiError::Forbidden`]
/// if it is present but does not match.
pub(crate) async fn admin_auth(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let provided = request
        .headers()
        .get(ADMIN_HEADER)
        .and_then(|value| value.to_str().ok())
        .ok_or(ApiError::Unauthorized)?;

    let matches: bool = provided
        .as_bytes()
        .ct_eq(state.admin_api_key.as_bytes())
        .into();
    if !matches {
        return Err(ApiError::Forbidden);
    }

    Ok(next.run(request).await)
}

/// Guards the client routes.
///
/// Requires an `x-api-key` header whose SHA-256 hash matches a stored key. On
/// success the matched [`AuthenticatedKey`] is placed in the request extensions
/// for downstream handlers.
///
/// # Errors
///
/// [`ApiError::Unauthorized`] if the header is missing or unknown;
/// [`ApiError::Database`] if the lookup fails.
pub(crate) async fn api_key_auth(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let provided = request
        .headers()
        .get(API_KEY_HEADER)
        .and_then(|value| value.to_str().ok())
        .ok_or(ApiError::Unauthorized)?;

    let hash = key::hash_key(provided);
    let row = db::find_by_hash(&state.pool, &hash)
        .await?
        .ok_or(ApiError::Unauthorized)?;

    request.extensions_mut().insert(AuthenticatedKey(row));
    Ok(next.run(request).await)
}
