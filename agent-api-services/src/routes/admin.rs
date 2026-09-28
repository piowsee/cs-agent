//! Administrator handlers: full CRUD over API keys.
//!
//! Every handler here sits behind [`crate::auth::admin_auth`].

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use uuid::Uuid;

use crate::error::ApiError;
use crate::models::{CreateKeyRequest, CreatedKey, KeyView, UpdateKeyRequest};
use crate::state::AppState;
use crate::{db, key};

/// `POST /admin/keys` — create a key and return its plaintext once.
pub(crate) async fn create_key(
    State(state): State<AppState>,
    Json(body): Json<CreateKeyRequest>,
) -> Result<(StatusCode, Json<CreatedKey>), ApiError> {
    if body.workflow_limit < 0 {
        return Err(ApiError::BadRequest(
            "workflow_limit must be >= 0".to_owned(),
        ));
    }

    let generated = key::generate();
    let row = db::create(
        &state.pool,
        Uuid::new_v4(),
        &generated.prefix,
        &generated.hash,
        body.name.as_deref(),
        body.workflow_limit,
        body.vector_store_url.as_deref(),
    )
    .await
    .map_err(ApiError::from_sqlx)?;

    let created = CreatedKey {
        api_key: generated.plaintext,
        key: row.into(),
    };
    Ok((StatusCode::CREATED, Json(created)))
}

/// `GET /admin/keys` — list all keys.
pub(crate) async fn list_keys(
    State(state): State<AppState>,
) -> Result<Json<Vec<KeyView>>, ApiError> {
    let rows = db::list(&state.pool).await?;
    Ok(Json(rows.into_iter().map(KeyView::from).collect()))
}

/// `GET /admin/keys/{id}` — read a single key.
pub(crate) async fn get_key(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<KeyView>, ApiError> {
    let row = db::get(&state.pool, id).await?.ok_or(ApiError::NotFound)?;
    Ok(Json(row.into()))
}

/// `PATCH /admin/keys/{id}` — update limits, usage, label, or vector store URL.
pub(crate) async fn update_key(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateKeyRequest>,
) -> Result<Json<KeyView>, ApiError> {
    if body.workflow_limit.is_some_and(|value| value < 0)
        || body.workflow_used.is_some_and(|value| value < 0)
    {
        return Err(ApiError::BadRequest(
            "workflow_limit and workflow_used must be >= 0".to_owned(),
        ));
    }

    let row = db::update(&state.pool, id, &body)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(row.into()))
}

/// `DELETE /admin/keys/{id}` — remove a key.
pub(crate) async fn delete_key(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    if db::delete(&state.pool, id).await? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::NotFound)
    }
}
