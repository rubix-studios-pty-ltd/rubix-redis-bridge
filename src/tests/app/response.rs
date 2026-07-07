use axum::http::StatusCode;
use redis::Value;

use crate::app::serialized_response;
use crate::redis::CommandResponse;

#[test]
fn allow_size_limit() {
    let result = Value::BulkString(b"ok".to_vec());

    let response = serialized_response(
        StatusCode::OK,
        &CommandResponse {
            result: &result,
            base64_encoding: false,
        },
        1024,
    );

    assert!(response.is_ok());
}

#[test]
fn reject_size_limit() {
    let result = Value::BulkString(b"this response is too large".to_vec());

    let response = serialized_response(
        StatusCode::OK,
        &CommandResponse {
            result: &result,
            base64_encoding: false,
        },
        8,
    );

    assert!(response.is_err());
}
