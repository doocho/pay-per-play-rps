use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("validation: {0}")]
    Validation(String),

    #[error("payment required")]
    PaymentRequired,

    #[error("invalid payment: {0}")]
    PaymentInvalid(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("gone: {0}")]
    Gone(String),

    #[error("internal: {0}")]
    Internal(#[from] anyhow::Error),

    #[error("database: {0}")]
    Database(#[from] sqlx::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_code, message) = match &self {
            AppError::Validation(msg) => (StatusCode::BAD_REQUEST, "invalid_request", msg.clone()),
            AppError::PaymentRequired => (
                StatusCode::PAYMENT_REQUIRED,
                "payment_required",
                "Payment is required to play".to_string(),
            ),
            AppError::PaymentInvalid(msg) => {
                (StatusCode::UNAUTHORIZED, "invalid_payment", msg.clone())
            }
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, "not_found", msg.clone()),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, "conflict", msg.clone()),
            AppError::Gone(msg) => (StatusCode::GONE, "game_expired", msg.clone()),
            AppError::Internal(err) => {
                tracing::error!(error = %err, "internal server error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "An unexpected error occurred".to_string(),
                )
            }
            AppError::Database(err) => {
                tracing::error!(error = %err, "database error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "An unexpected error occurred".to_string(),
                )
            }
        };

        let body = json!({
            "error": error_code,
            "message": message,
        });

        (status, axum::Json(body)).into_response()
    }
}
