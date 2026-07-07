use redis::RedisError;
use tracing::error;

use crate::app::ApiError;

pub(crate) fn redis_api_error(error: RedisError) -> ApiError {
    if backend_unavailable(&error) {
        error!(kind = ?error.kind(), code = ?error.code(), %error, "Redis backend unavailable");
        return ApiError::unavailable("Redis backend unavailable");
    }

    ApiError::bad_request(redis_error_message(&error))
}

fn backend_unavailable(error: &RedisError) -> bool {
    if error.is_io_error()
        || error.is_connection_dropped()
        || error.is_connection_refusal()
        || error.is_timeout()
    {
        return true;
    }

    if matches!(
        error.kind(),
        redis::ErrorKind::AuthenticationFailed | redis::ErrorKind::InvalidClientConfig
    ) {
        return true;
    }

    matches!(
        error.code(),
        Some("LOADING" | "TRYAGAIN" | "CLUSTERDOWN" | "MASTERDOWN")
    )
}

fn strip_response_prefix(message: String) -> String {
    message
        .strip_prefix("ResponseError: ")
        .unwrap_or(message.as_str())
        .to_string()
}

pub(crate) fn redis_error_message(error: &RedisError) -> String {
    if let Some(code) = error.code() {
        let message = strip_response_prefix(error.to_string());

        if message.starts_with(code) {
            return message;
        }

        if let Some((_, detail)) = message.split_once(": ") {
            return format!("{code} {detail}");
        }

        return format!("{code} {message}");
    }

    strip_response_prefix(error.to_string())
}
