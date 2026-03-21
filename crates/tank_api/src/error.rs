use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use tank_core::SimError;

#[derive(Debug, Serialize)]
pub struct ApiError {
    pub error: String,
    #[serde(skip)]
    pub status: StatusCode,
}

impl ApiError {
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self {
            error: msg.into(),
            status: StatusCode::BAD_REQUEST,
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(ApiErrorBody { error: self.error })).into_response()
    }
}

#[derive(Serialize)]
struct ApiErrorBody {
    error: String,
}

impl From<SimError> for ApiError {
    fn from(err: SimError) -> Self {
        let status = match &err {
            SimError::InvariantViolation { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            SimError::Serialization(_) => StatusCode::INTERNAL_SERVER_ERROR,
            _ => StatusCode::BAD_REQUEST,
        };
        Self {
            error: err.to_string(),
            status,
        }
    }
}
