//! Consistent error envelope (design §6.1): every non-2xx response is
//! `{"error": {"code", "message"}}` — the same shape on 400, 404 and 500.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use utoipa::ToSchema;

/// The error envelope every non-2xx response carries.
#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorBody {
    /// The error itself.
    pub error: ErrorDetail,
}

/// Machine-readable error code plus a human-readable message.
#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorDetail {
    /// Stable machine-readable code (e.g. `invalid_cursor`, `not_found`).
    #[schema(examples("invalid_cursor"))]
    pub code: String,
    /// Human-readable explanation.
    pub message: String,
}

/// Handler-level API failure; renders as the [`ErrorBody`] envelope.
#[derive(Debug)]
pub enum ApiError {
    /// Malformed client input (bad cursor, bad limit, bad query value).
    BadRequest {
        /// Stable machine-readable code.
        code: &'static str,
        /// Human-readable explanation.
        message: String,
    },
    /// The addressed resource does not exist.
    NotFound {
        /// Human-readable explanation.
        message: String,
    },
    /// Anything the client cannot fix; details stay server-side.
    Internal(anyhow::Error),
}

impl ApiError {
    /// Shorthand for a [`ApiError::BadRequest`].
    pub fn bad_request(code: &'static str, message: impl Into<String>) -> Self {
        Self::BadRequest {
            code,
            message: message.into(),
        }
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(err: anyhow::Error) -> Self {
        Self::Internal(err)
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(err: sqlx::Error) -> Self {
        Self::Internal(err.into())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::BadRequest { code, message } => {
                (StatusCode::BAD_REQUEST, code.to_owned(), message)
            }
            Self::NotFound { message } => (StatusCode::NOT_FOUND, "not_found".to_owned(), message),
            Self::Internal(err) => {
                // Details stay server-side; the envelope stays generic.
                eprintln!("internal error: {err:#}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal".to_owned(),
                    "internal server error".to_owned(),
                )
            }
        };
        let body = ErrorBody {
            error: ErrorDetail { code, message },
        };
        (status, Json(body)).into_response()
    }
}
