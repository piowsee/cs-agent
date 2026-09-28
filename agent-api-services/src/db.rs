//! Repository layer: raw SQL over [`sqlx`], no ORM.
//!
//! Every function takes a `&PgPool` and returns owned rows. Queries use runtime
//! `query_as` with `#[derive(FromRow)]`, so the crate compiles without a live
//! database while keeping the zero-overhead of hand-written SQL.

use uuid::Uuid;

use crate::models::{ApiKeyRow, UpdateKeyRequest};

/// Inserts a new key and returns the stored row.
pub(crate) async fn create(
    pool: &sqlx::PgPool,
    id: Uuid,
    prefix: &str,
    hash: &str,
    name: Option<&str>,
    workflow_limit: i64,
    vector_store_url: Option<&str>,
) -> Result<ApiKeyRow, sqlx::Error> {
    sqlx::query_as::<_, ApiKeyRow>(
        "INSERT INTO api_keys \
             (id, key_prefix, key_hash, name, workflow_limit, vector_store_url) \
         VALUES ($1, $2, $3, $4, $5, $6) \
         RETURNING *",
    )
    .bind(id)
    .bind(prefix)
    .bind(hash)
    .bind(name)
    .bind(workflow_limit)
    .bind(vector_store_url)
    .fetch_one(pool)
    .await
}

/// Returns all keys, newest first.
pub(crate) async fn list(pool: &sqlx::PgPool) -> Result<Vec<ApiKeyRow>, sqlx::Error> {
    sqlx::query_as::<_, ApiKeyRow>("SELECT * FROM api_keys ORDER BY created_at DESC")
        .fetch_all(pool)
        .await
}

/// Returns the key with `id`, if it exists.
pub(crate) async fn get(pool: &sqlx::PgPool, id: Uuid) -> Result<Option<ApiKeyRow>, sqlx::Error> {
    sqlx::query_as::<_, ApiKeyRow>("SELECT * FROM api_keys WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await
}

/// Returns the key matching `hash`, if any. Used by the authentication layer.
pub(crate) async fn find_by_hash(
    pool: &sqlx::PgPool,
    hash: &str,
) -> Result<Option<ApiKeyRow>, sqlx::Error> {
    sqlx::query_as::<_, ApiKeyRow>("SELECT * FROM api_keys WHERE key_hash = $1")
        .bind(hash)
        .fetch_optional(pool)
        .await
}

/// Partially updates a key. Absent (`None`) fields keep their current value via
/// `COALESCE`. Returns the updated row, or `None` if no key has `id`.
pub(crate) async fn update(
    pool: &sqlx::PgPool,
    id: Uuid,
    req: &UpdateKeyRequest,
) -> Result<Option<ApiKeyRow>, sqlx::Error> {
    sqlx::query_as::<_, ApiKeyRow>(
        "UPDATE api_keys SET \
             name             = COALESCE($2, name), \
             workflow_limit   = COALESCE($3, workflow_limit), \
             workflow_used    = COALESCE($4, workflow_used), \
             vector_store_url = COALESCE($5, vector_store_url), \
             updated_at       = now() \
         WHERE id = $1 \
         RETURNING *",
    )
    .bind(id)
    .bind(req.name.as_deref())
    .bind(req.workflow_limit)
    .bind(req.workflow_used)
    .bind(req.vector_store_url.as_deref())
    .fetch_optional(pool)
    .await
}

/// Deletes the key with `id`. Returns `true` if a row was removed.
pub(crate) async fn delete(pool: &sqlx::PgPool, id: Uuid) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM api_keys WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

/// Atomically records one workflow run against the key's quota.
///
/// The single conditional `UPDATE` increments the counter only while
/// `workflow_used < workflow_limit`, so concurrent callers cannot exceed the
/// limit. Returns the updated row, or `None` when the quota is exhausted (the
/// key is assumed to exist, having just been authenticated).
pub(crate) async fn try_run_workflow(
    pool: &sqlx::PgPool,
    id: Uuid,
) -> Result<Option<ApiKeyRow>, sqlx::Error> {
    sqlx::query_as::<_, ApiKeyRow>(
        "UPDATE api_keys \
             SET workflow_used = workflow_used + 1, updated_at = now() \
         WHERE id = $1 AND workflow_used < workflow_limit \
         RETURNING *",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}
