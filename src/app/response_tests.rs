use axum::http::StatusCode;
use redis::Value as RedisValue;

use super::redis_response::CommandResponse;
use super::response::serialized_response;

#[test]
fn allow_size_limit() {
    let result = RedisValue::BulkString(b"ok".to_vec());

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
    let result = RedisValue::BulkString(b"this response is too large".to_vec());

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
