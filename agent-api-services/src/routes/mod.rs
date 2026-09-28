//! HTTP routing: assembles the public, admin, and client route groups.

mod admin;
mod keys;

use axum::routing::{get, post};
use axum::{Json, Router, middleware};
use serde_json::{Value, json};
use tower_http::trace::TraceLayer;

use crate::auth;
use crate::state::AppState;

/// Builds the complete application router.
///
/// Layout:
/// - `GET /health` — public.
/// - `/admin/*` — behind the admin-auth middleware (env `ADMIN_API_KEY`).
/// - `/me`, `/workflows` — behind the API-key-auth middleware (DB-stored keys).
///
/// A [`TraceLayer`] wraps everything for request logging.
pub fn build_router(state: AppState) -> Router {
    let admin = Router::new()
        .route("/keys", post(admin::create_key).get(admin::list_keys))
        .route(
            "/keys/{id}",
            get(admin::get_key).patch(admin::update_key).delete(admin::delete_key),
        )
        .layer(middleware::from_fn_with_state(state.clone(), auth::admin_auth));

    let authed = Router::new()
        .route("/me", get(keys::me))
        .route("/workflows", post(keys::run_workflow))
        .layer(middleware::from_fn_with_state(state.clone(), auth::api_key_auth));

    Router::new()
        .route("/health", get(health))
        .nest("/admin", admin)
        .merge(authed)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// `GET /health` — liveness probe.
async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}
