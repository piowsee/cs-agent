//! The API error type and its HTTP representation.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

/// Every fallible handler and middleware returns this error.
///
/// It implements [`IntoResponse`], mapping each variant to a status code and a
/// JSON body of the shape `{ "error": <code>, "message": <human text> }`.
/// Database internals are logged but never exposed to the client.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// Missing or unrecognized credentials (`401`).
    #[error("authentication required")]
    Unauthorized,
    /// Valid request shape but the caller is not permitted (`403`).
    #[error("access denied")]
    Forbidden,
    /// The requested resource does not exist (`404`).
    #[error("resource not found")]
    NotFound,
    /// The key's workflow quota is exhausted (`429`).
    #[error("workflow limit exceeded")]
    LimitExceeded,
    /// The request body or parameters were invalid (`400`).
    #[error("{0}")]
    BadRequest(String),
    /// A uniqueness constraint was violated (`409`).
    #[error("resource already exists")]
    Conflict,
    /// An unexpected database failure (`500`).
    #[error("internal database error")]
    Database(#[from] sqlx::Error),
}

impl ApiError {
    /// Converts a [`sqlx::Error`], promoting unique-constraint violations to
    /// [`ApiError::Conflict`] and treating everything else as
    /// [`ApiError::Database`].
    pub(crate) fn from_sqlx(error: sqlx::Error) -> Self {
        if let sqlx::Error::Database(ref db_error) = error
            && db_error.is_unique_violation()
        {
            return Self::Conflict;
        }
        Self::Database(error)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code) = match &self {
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized"),
            Self::Forbidden => (StatusCode::FORBIDDEN, "forbidden"),
            Self::NotFound => (StatusCode::NOT_FOUND, "not_found"),
            Self::LimitExceeded => (StatusCode::TOO_MANY_REQUESTS, "limit_exceeded"),
            Self::BadRequest(_) => (StatusCode::BAD_REQUEST, "bad_request"),
            Self::Conflict => (StatusCode::CONFLICT, "conflict"),
            Self::Database(error) => {
                // Log the real cause; return an opaque message to the client.
                tracing::error!(error = %error, "database error");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal_error")
            }
        };

        let message = match &self {
            Self::Database(_) => "internal server error".to_owned(),
            other => other.to_string(),
        };

        (status, Json(json!({ "error": code, "message": message }))).into_response()
    }
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use axum::response::IntoResponse as _;

    use super::ApiError;

    /// Every variant maps to its documented status code, and building the
    /// response exercises both the status and message match arms.
    #[test]
    fn variants_map_to_expected_status_codes() {
        let cases = [
            (ApiError::Unauthorized, StatusCode::UNAUTHORIZED),
            (ApiError::Forbidden, StatusCode::FORBIDDEN),
            (ApiError::NotFound, StatusCode::NOT_FOUND),
            (ApiError::LimitExceeded, StatusCode::TOO_MANY_REQUESTS),
            (
                ApiError::BadRequest("bad".to_owned()),
                StatusCode::BAD_REQUEST,
            ),
            (ApiError::Conflict, StatusCode::CONFLICT),
            (
                ApiError::Database(sqlx::Error::RowNotFound),
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
        ];

        for (error, expected) in cases {
            assert_eq!(error.into_response().status(), expected);
        }
    }

    /// A non-unique-violation `sqlx::Error` stays a `Database` error rather than
    /// being promoted to a `Conflict`.
    #[test]
    fn from_sqlx_keeps_plain_errors_as_database() {
        let error = ApiError::from_sqlx(sqlx::Error::RowNotFound);
        assert!(matches!(error, ApiError::Database(_)));
    }
}
