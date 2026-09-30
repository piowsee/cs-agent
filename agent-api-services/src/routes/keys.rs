//! Client handlers for authenticated API-key holders.
//!
//! Every handler here sits behind [`crate::auth::api_key_auth`], which injects
//! the [`AuthenticatedKey`] these handlers read.

use axum::Extension;
use axum::Json;
use axum::extract::State;

use crate::auth::AuthenticatedKey;
use crate::db;
use crate::error::ApiError;
use crate::models::{KeyView, RunWorkflowResponse};
use crate::state::AppState;

/// `GET /me` — return the calling key's own metadata.
pub(crate) async fn me(Extension(auth): Extension<AuthenticatedKey>) -> Json<KeyView> {
    Json(auth.0.into())
}

/// `POST /workflows` — run one workflow, consuming a unit of the key's quota.
///
/// # Errors
///
/// [`ApiError::LimitExceeded`] (`429`) once the quota is exhausted.
pub(crate) async fn run_workflow(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthenticatedKey>,
) -> Result<Json<RunWorkflowResponse>, ApiError> {
    let row = db::try_run_workflow(&state.pool, auth.0.id)
        .await?
        .ok_or(ApiError::LimitExceeded)?;

    // Quota is now reserved (atomically). Invoke the actual workflow from the
    // other package HERE — the authenticated key is available as `auth.0`, so
    // its vector store URL can be passed through, e.g.:
    //
    //     workflow_crate::run(&auth.0.vector_store_url, /* inputs */).await?;
    //
    // Ordering note: quota is consumed *before* the workflow runs, so a failed
    // run still counts against the limit. To charge only on success, run the
    // workflow first and record usage afterwards — at the cost of the
    // over-limit race protection that `try_run_workflow` gives you here.

    Ok(Json(RunWorkflowResponse {
        used: row.workflow_used,
        limit: row.workflow_limit,
        remaining: row.workflow_limit - row.workflow_used,
    }))
}
