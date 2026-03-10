//! Error handling for API responses

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use std::fmt;
use tracing::error;

#[derive(Debug)]
pub enum ApiError {
    NotImplemented,
    DatabaseConnection(String),
    QueryExecution(String),
    Optimization(String),
    InvalidRequest(String),
    Internal(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            ApiError::NotImplemented => (
                StatusCode::NOT_IMPLEMENTED,
                "NOT_IMPLEMENTED",
                "Endpoint not yet implemented".to_string(),
            ),
            ApiError::DatabaseConnection(msg) => {
                error!("Database connection error: {}", msg);
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    "DB_CONNECTION_ERROR",
                    format!("Database connection failed: {}", msg),
                )
            }
            ApiError::QueryExecution(msg) => {
                error!("Query execution error: {}", msg);
                (
                    StatusCode::BAD_REQUEST,
                    "QUERY_EXECUTION_ERROR",
                    format!("Query execution failed: {}", msg),
                )
            }
            ApiError::Optimization(msg) => {
                error!("Optimization error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "OPTIMIZATION_ERROR",
                    format!("Optimization failed: {}", msg),
                )
            }
            ApiError::InvalidRequest(msg) => (
                StatusCode::BAD_REQUEST,
                "INVALID_REQUEST",
                format!("Invalid request: {}", msg),
            ),
            ApiError::Internal(msg) => {
                error!("Internal server error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTERNAL_ERROR",
                    "Internal server error occurred".to_string(),
                )
            }
        };

        let body = Json(json!({
            "error": message,
            "code": code,
        }));
        (status, body).into_response()
    }
}

pub type ApiResult<T> = Result<T, ApiError>;

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApiError::NotImplemented => write!(f, "Not implemented"),
            ApiError::DatabaseConnection(msg) => write!(f, "Database connection error: {}", msg),
            ApiError::QueryExecution(msg) => write!(f, "Query execution error: {}", msg),
            ApiError::Optimization(msg) => write!(f, "Optimization error: {}", msg),
            ApiError::InvalidRequest(msg) => write!(f, "Invalid request: {}", msg),
            ApiError::Internal(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}
