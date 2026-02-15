use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use utoipa::ToSchema;

/// Standard error response body.
#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorResponse {
    /// Human-readable error message
    pub error: String,
}

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("External service error: {0}")]
    ExternalServiceError(String),

    #[error("Internal error: {0}")]
    InternalError(String),

    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::ExternalServiceError(msg) => {
                tracing::error!("External service error: {}", msg);
                (
                    StatusCode::BAD_GATEWAY,
                    "External service unavailable".to_string(),
                )
            }
            AppError::InternalError(msg) => {
                tracing::error!("Internal error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
            AppError::DatabaseError(err) => {
                tracing::error!("Database error: {:?}", err);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal database error".to_string(),
                )
            }
        };

        (status, axum::Json(ErrorResponse { error: message })).into_response()
    }
}

impl From<crate::services::gpx::GpxError> for AppError {
    fn from(err: crate::services::gpx::GpxError) -> Self {
        AppError::InternalError(format!("GPX parsing error: {}", err))
    }
}
