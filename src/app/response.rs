use axum::extract::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::Value;

pub(crate) fn json_response(status: StatusCode, value: Value) -> Response {
    (status, Json(value)).into_response()
}
