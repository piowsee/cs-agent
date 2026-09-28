//! End-to-end HTTP tests driven in-process with `Router::oneshot`.
//!
//! Each test uses `#[sqlx::test]`, which provisions an isolated database with
//! the migrations in `./migrations` already applied, then hands us a pool.
//! These tests therefore require a reachable PostgreSQL server (via
//! `DATABASE_URL`). Run with: `cargo test -p agent-api-services`.

use agent_api_services::{AppState, build_router};
use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt as _;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt as _;

/// Admin secret used by the test router.
const ADMIN_KEY: &str = "test-admin-secret";

/// Builds the router under test with a known admin key.
fn app(pool: PgPool) -> Router {
    build_router(AppState::new(pool, ADMIN_KEY))
}

/// Builds a request, optionally attaching auth headers and a JSON body.
fn request(
    method: &str,
    uri: &str,
    admin_key: Option<&str>,
    api_key: Option<&str>,
    body: Option<Value>,
) -> Request<Body> {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(key) = admin_key {
        builder = builder.header("x-admin-key", key);
    }
    if let Some(key) = api_key {
        builder = builder.header("x-api-key", key);
    }

    let body = match body {
        Some(value) => {
            builder = builder.header(header::CONTENT_TYPE, "application/json");
            Body::from(serde_json::to_vec(&value).expect("serialize body"))
        }
        None => Body::empty(),
    };
    builder.body(body).expect("build request")
}

/// Sends a request through a clone of the router and returns status + JSON body.
async fn send(app: &Router, req: Request<Body>) -> (StatusCode, Value) {
    let response = app.clone().oneshot(req).await.expect("router response");
    let status = response.status();
    let bytes = response.into_body().collect().await.expect("read body").to_bytes();
    let json = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).expect("parse json")
    };
    (status, json)
}

/// Creates a key via the admin API and returns `(id, plaintext_api_key)`.
async fn create_key(app: &Router, limit: i64) -> (String, String) {
    let body = json!({ "name": "test", "workflow_limit": limit,
                       "vector_store_url": "https://vec.example/store" });
    let (status, json) =
        send(app, request("POST", "/admin/keys", Some(ADMIN_KEY), None, Some(body))).await;
    assert_eq!(status, StatusCode::CREATED);
    let id = json["id"].as_str().expect("id").to_owned();
    let api_key = json["api_key"].as_str().expect("api_key").to_owned();
    (id, api_key)
}

#[sqlx::test]
async fn health_is_public(pool: PgPool) {
    let app = app(pool);
    let (status, body) = send(&app, request("GET", "/health", None, None, None)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
}

#[sqlx::test]
async fn admin_requires_key(pool: PgPool) {
    let app = app(pool);

    // No admin header -> 401.
    let (status, _) = send(&app, request("GET", "/admin/keys", None, None, None)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // Wrong admin header -> 403.
    let (status, _) =
        send(&app, request("GET", "/admin/keys", Some("nope"), None, None)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Correct admin header -> 200.
    let (status, body) =
        send(&app, request("GET", "/admin/keys", Some(ADMIN_KEY), None, None)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.as_array().expect("array").is_empty());
}

#[sqlx::test]
async fn create_returns_plaintext_once_and_hides_hash(pool: PgPool) {
    let app = app(pool);
    let body = json!({ "workflow_limit": 5, "vector_store_url": "https://v/x" });
    let (status, json) =
        send(&app, request("POST", "/admin/keys", Some(ADMIN_KEY), None, Some(body))).await;

    assert_eq!(status, StatusCode::CREATED);
    assert!(json["api_key"].as_str().expect("api_key").starts_with("sk_"));
    assert!(json["key_prefix"].as_str().expect("prefix").starts_with("sk_"));
    assert_eq!(json["workflow_limit"], 5);
    assert_eq!(json["workflow_used"], 0);
    // The stored hash must never be serialized to clients.
    assert!(json.get("key_hash").is_none());
}

#[sqlx::test]
async fn rejects_negative_limit_on_create(pool: PgPool) {
    let app = app(pool);
    let body = json!({ "workflow_limit": -1 });
    let (status, _) =
        send(&app, request("POST", "/admin/keys", Some(ADMIN_KEY), None, Some(body))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[sqlx::test]
async fn full_crud_lifecycle(pool: PgPool) {
    let app = app(pool);
    let (id, _) = create_key(&app, 10).await;

    // List shows the new key.
    let (status, body) =
        send(&app, request("GET", "/admin/keys", Some(ADMIN_KEY), None, None)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.as_array().expect("array").len(), 1);

    // Read it back.
    let uri = format!("/admin/keys/{id}");
    let (status, body) = send(&app, request("GET", &uri, Some(ADMIN_KEY), None, None)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["id"], id);

    // Update the vector store URL and the limit.
    let patch = json!({ "workflow_limit": 99, "vector_store_url": "https://v/updated" });
    let (status, body) =
        send(&app, request("PATCH", &uri, Some(ADMIN_KEY), None, Some(patch))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["workflow_limit"], 99);
    assert_eq!(body["vector_store_url"], "https://v/updated");

    // Delete, then confirm it is gone.
    let (status, _) = send(&app, request("DELETE", &uri, Some(ADMIN_KEY), None, None)).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = send(&app, request("GET", &uri, Some(ADMIN_KEY), None, None)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn unknown_ids_return_404(pool: PgPool) {
    let app = app(pool);
    let uri = "/admin/keys/00000000-0000-0000-0000-000000000000";

    let (status, _) = send(&app, request("GET", uri, Some(ADMIN_KEY), None, None)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let patch = json!({ "workflow_limit": 1 });
    let (status, _) =
        send(&app, request("PATCH", uri, Some(ADMIN_KEY), None, Some(patch))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, _) = send(&app, request("DELETE", uri, Some(ADMIN_KEY), None, None)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn rejects_negative_values_on_update(pool: PgPool) {
    let app = app(pool);
    let (id, _) = create_key(&app, 10).await;
    let uri = format!("/admin/keys/{id}");

    let patch = json!({ "workflow_used": -5 });
    let (status, _) =
        send(&app, request("PATCH", &uri, Some(ADMIN_KEY), None, Some(patch))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[sqlx::test]
async fn client_auth_is_enforced(pool: PgPool) {
    let app = app(pool);
    let (_, api_key) = create_key(&app, 3).await;

    // Missing key -> 401.
    let (status, _) = send(&app, request("GET", "/me", None, None, None)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // Unknown key -> 401.
    let (status, _) = send(&app, request("GET", "/me", None, Some("sk_bogus"), None)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // Valid key -> 200 with the caller's own metadata.
    let (status, body) =
        send(&app, request("GET", "/me", None, Some(&api_key), None)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["workflow_limit"], 3);
    assert_eq!(body["vector_store_url"], "https://vec.example/store");
    assert!(body.get("key_hash").is_none());
}

#[sqlx::test]
async fn workflows_enforce_quota(pool: PgPool) {
    let app = app(pool);
    let (_, api_key) = create_key(&app, 2).await;

    // First two workflow runs succeed and count up.
    let (status, body) =
        send(&app, request("POST", "/workflows", None, Some(&api_key), None)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["used"], 1);
    assert_eq!(body["remaining"], 1);

    let (status, body) =
        send(&app, request("POST", "/workflows", None, Some(&api_key), None)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["used"], 2);
    assert_eq!(body["remaining"], 0);

    // Third exceeds the quota.
    let (status, _) =
        send(&app, request("POST", "/workflows", None, Some(&api_key), None)).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
}
