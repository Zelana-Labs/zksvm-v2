use axum::{
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde::Serialize;

#[derive(Serialize)]
pub struct JsonErrorResponse {
    error: ErrorBody,
}

#[derive(Serialize)]
pub struct ErrorBody {
    code: &'static str,
    message: String,
}

pub enum ApiError {
    NotFound(String),
    BadRequest(String),
    DatabaseUnavailable(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, error_code, error_message) = match self {
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, "not_found", msg),
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, "bad_request", msg),
            ApiError::DatabaseUnavailable(msg) => {
                (StatusCode::SERVICE_UNAVAILABLE, "db_unavailable", msg)
            }
        };

        let body = Json(JsonErrorResponse {
            error: ErrorBody { code: error_code, message: error_message },
        });

        (status, [("Content-Type", "application/json")], body).into_response()
    }
}

