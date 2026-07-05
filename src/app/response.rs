use std::io::{self, Write};

use axum::extract::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::Value;

use super::error::ApiError;

struct SizeLimitWriter {
    written: usize,
    max: usize,
    exceeded: bool,
}

impl SizeLimitWriter {
    fn new(max: usize) -> Self {
        Self {
            written: 0,
            max,
            exceeded: false,
        }
    }
}

impl Write for SizeLimitWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.len() > self.max.saturating_sub(self.written) {
            self.exceeded = true;
            return Err(io::Error::other(
                "serialized Redis response exceeded size limit",
            ));
        }

        self.written += buf.len();
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(crate) fn json_response(status: StatusCode, value: Value) -> Response {
    (status, Json(value)).into_response()
}

pub(crate) fn limited_json_response(
    status: StatusCode,
    value: Value,
    max_response_bytes: usize,
) -> Result<Response, ApiError> {
    validate_json_response_size(&value, max_response_bytes)?;

    Ok(json_response(status, value))
}

fn validate_json_response_size(value: &Value, max_response_bytes: usize) -> Result<(), ApiError> {
    let mut writer = SizeLimitWriter::new(max_response_bytes);

    match serde_json::to_writer(&mut writer, value) {
        Ok(()) => Ok(()),
        Err(_) if writer.exceeded => Err(ApiError::response_too_large(format!(
            "Redis response is too large. Maximum allowed bytes: {max_response_bytes}."
        ))),
        Err(_) => Err(ApiError::unavailable("Failed to encode Redis response")),
    }
}
