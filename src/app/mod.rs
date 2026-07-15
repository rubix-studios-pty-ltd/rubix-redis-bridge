mod error;
mod handlers;
mod realtime;
mod response;
mod state;

pub use handlers::{command, healthz, metrics, multi_exec, pipeline, readyz, root};
pub use realtime::subscribe;

pub(crate) use error::ApiError;
pub(crate) use state::{AppState, AuthRoute, RedisTarget};

#[cfg(test)]
pub(crate) use {realtime::validate_channel, response::serialized_response};
