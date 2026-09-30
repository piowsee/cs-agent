//! Database row types and the request/response payloads exposed by the API.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

/// A row of the `api_keys` table, as decoded by SQLx.
///
/// This mirrors the storage layout and therefore includes `key_hash`, which is
/// never serialized to clients — convert to [`KeyView`] for responses.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ApiKeyRow {
    /// Primary key.
    pub id: Uuid,
    /// Short human-readable prefix of the plaintext key (e.g. `sk_a1b2c3d`).
    pub key_prefix: String,
    /// Hex-encoded SHA-256 of the plaintext key.
    pub key_hash: String,
    /// Optional human label.
    pub name: Option<String>,
    /// Maximum number of workflow consumptions allowed.
    pub workflow_limit: i64,
    /// Number of consumptions used so far.
    pub workflow_used: i64,
    /// Vector store URL associated with the key.
    pub vector_store_url: Option<String>,
    /// Creation timestamp.
    pub created_at: OffsetDateTime,
    /// Last-modified timestamp.
    pub updated_at: OffsetDateTime,
}

/// Public, client-safe view of an API key (never includes the hash).
#[derive(Debug, Clone, Serialize)]
pub struct KeyView {
    /// Primary key.
    pub id: Uuid,
    /// Short human-readable prefix of the plaintext key.
    pub key_prefix: String,
    /// Optional human label.
    pub name: Option<String>,
    /// Maximum number of workflow consumptions allowed.
    pub workflow_limit: i64,
    /// Number of consumptions used so far.
    pub workflow_used: i64,
    /// Vector store URL associated with the key.
    pub vector_store_url: Option<String>,
    /// Creation timestamp, serialized as RFC 3339.
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    /// Last-modified timestamp, serialized as RFC 3339.
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

impl From<ApiKeyRow> for KeyView {
    fn from(row: ApiKeyRow) -> Self {
        Self {
            id: row.id,
            key_prefix: row.key_prefix,
            name: row.name,
            workflow_limit: row.workflow_limit,
            workflow_used: row.workflow_used,
            vector_store_url: row.vector_store_url,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

/// Body of `POST /admin/keys`.
#[derive(Debug, Clone, Deserialize)]
pub struct CreateKeyRequest {
    /// Optional human label.
    pub name: Option<String>,
    /// Initial workflow quota (must be `>= 0`). Defaults to `0`.
    #[serde(default)]
    pub workflow_limit: i64,
    /// Vector store URL to associate with the key.
    pub vector_store_url: Option<String>,
}

/// Body of `PATCH /admin/keys/{id}`.
///
/// Every field is optional; omitted (`null`) fields are left unchanged. Because
/// of this "omit means keep" semantics, `name` and `vector_store_url` cannot be
/// cleared back to `NULL` through this endpoint.
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateKeyRequest {
    /// New human label, if provided.
    pub name: Option<String>,
    /// New workflow quota, if provided (must be `>= 0`).
    pub workflow_limit: Option<i64>,
    /// New usage counter, if provided (must be `>= 0`); useful for resets.
    pub workflow_used: Option<i64>,
    /// New vector store URL, if provided.
    pub vector_store_url: Option<String>,
}

/// Response of `POST /admin/keys`.
///
/// The `api_key` field carries the plaintext key and is returned **only once**,
/// at creation — it cannot be recovered afterward.
#[derive(Debug, Clone, Serialize)]
pub struct CreatedKey {
    /// The plaintext API key. Store it now; it is not retrievable later.
    pub api_key: String,
    /// The persisted key metadata.
    #[serde(flatten)]
    pub key: KeyView,
}

/// Response of `POST /workflows`.
#[derive(Debug, Clone, Serialize)]
pub struct RunWorkflowResponse {
    /// Usage count after this workflow run.
    pub used: i64,
    /// The key's configured limit.
    pub limit: i64,
    /// Remaining workflow runs (`limit - used`).
    pub remaining: i64,
}
