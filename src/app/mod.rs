mod error;
mod handlers;
mod response;
mod state;

pub use handlers::{command, healthz, metrics, multi_exec, pipeline, readyz, root};

pub(crate) use error::ApiError;
pub(crate) use state::{AppState, AuthRoute, RedisTarget};

#[cfg(test)]
pub(crate) use response::serialized_response;
