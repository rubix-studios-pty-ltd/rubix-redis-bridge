mod error;
mod handlers;
mod redis_error;
mod redis_exec;
mod redis_response;
mod redis_value;
mod response;
mod state;

pub use handlers::{command, healthz, metrics, multi_exec, pipeline, readyz, root};

pub(crate) use error::ApiError;
pub(crate) use state::{AppState, RedisTarget};

#[cfg(test)]
mod redis_value_tests;

#[cfg(test)]
mod redis_response_tests;

#[cfg(test)]
mod response_tests;
