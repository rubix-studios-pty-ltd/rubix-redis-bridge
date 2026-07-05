use std::io::{self, Write};

use axum::body::Body;
use axum::extract::Json;
use axum::http::StatusCode;
use axum::http::header::CONTENT_TYPE;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use serde_json::Value;

use super::error::ApiError;

struct LimitBody {
    body: Vec<u8>,
    written: usize,
    max: usize,
    exceeded: bool,
}

impl LimitBody {
    fn new(max: usize) -> Self {
        Self {
            body: Vec::new(),
            written: 0,
            max,
            exceeded: false,
        }
    }

    fn into_body(self) -> Vec<u8> {
        self.body
    }
}

impl Write for LimitBody {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.len() > self.max.saturating_sub(self.written) {
            self.exceeded = true;
            return Err(io::Error::other(
                "serialized Redis response exceeded size limit",
            ));
        }

        self.written += buf.len();
        self.body.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(crate) fn json_response(status: StatusCode, value: Value) -> Response {
    (status, Json(value)).into_response()
}

pub(crate) fn serialized_response<T>(
    status: StatusCode,
    value: &T,
    max_response_bytes: usize,
) -> Result<Response, ApiError>
where
    T: Serialize,
{
    let mut writer = LimitBody::new(max_response_bytes);

    match serde_json::to_writer(&mut writer, value) {
        Ok(()) => Ok((
            status,
            [(CONTENT_TYPE, "application/json")],
            Body::from(writer.into_body()),
        )
            .into_response()),
        Err(_) if writer.exceeded => Err(ApiError::response_too_large(format!(
            "Redis response is too large. Maximum allowed bytes: {max_response_bytes}."
        ))),
        Err(_) => Err(ApiError::unavailable("Failed to encode Redis response")),
    }
}
