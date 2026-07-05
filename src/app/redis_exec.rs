use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use redis::Value as RedisValue;
use redis::aio::ConnectionManager;
use tokio::time::timeout;
use tracing::{error, warn};

use crate::metrics::Metrics;
use crate::security::RedisCommand;

use super::error::ApiError;
use super::redis_error::{redis_api_error, redis_error_message};
use super::state::RedisTarget;

pub(crate) enum RedisResponseItem {
    Result(RedisValue),
    Error(String),
}

pub(crate) async fn execute_command(
    target: Arc<RedisTarget>,
    command: RedisCommand,
    request_timeout: Duration,
    acquire_timeout: Duration,
    metrics: Metrics,
) -> Result<RedisValue, ApiError> {
    execute_operation(
        target,
        "command",
        "Redis command timed out",
        request_timeout,
        acquire_timeout,
        metrics,
        move |mut connection| async move {
            let mut redis_command = redis::cmd(command.name.as_str());

            for arg in command.args {
                redis_command.arg(arg.as_slice());
            }

            let result: redis::RedisResult<RedisValue> =
                redis_command.query_async(&mut connection).await;

            result.map_err(redis_api_error)
        },
    )
    .await
}

pub(crate) async fn execute_pipeline(
    target: Arc<RedisTarget>,
    commands: Vec<RedisCommand>,
    request_timeout: Duration,
    acquire_timeout: Duration,
    metrics: Metrics,
) -> Result<Vec<RedisResponseItem>, ApiError> {
    execute_operation(
        target,
        "pipeline",
        "Redis pipeline timed out",
        request_timeout,
        acquire_timeout,
        metrics,
        move |mut connection| async move {
            let mut pipe = redis::pipe();
            append_commands(&mut pipe, commands);

            let result: redis::RedisResult<Vec<redis::RedisResult<RedisValue>>> =
                pipe.ignore_errors().query_async(&mut connection).await;

            result
                .map(|items| {
                    items
                        .into_iter()
                        .map(|item| match item {
                            Ok(value) => RedisResponseItem::Result(value),
                            Err(error) => RedisResponseItem::Error(redis_error_message(&error)),
                        })
                        .collect()
                })
                .map_err(redis_api_error)
        },
    )
    .await
}

pub(crate) async fn execute_transaction(
    target: Arc<RedisTarget>,
    commands: Vec<RedisCommand>,
    request_timeout: Duration,
    acquire_timeout: Duration,
    metrics: Metrics,
) -> Result<Vec<RedisValue>, ApiError> {
    execute_operation(
        target,
        "multi_exec",
        "Redis transaction timed out",
        request_timeout,
        acquire_timeout,
        metrics,
        move |mut connection| async move {
            let mut pipe = redis::pipe();
            pipe.atomic();
            append_commands(&mut pipe, commands);

            let result: redis::RedisResult<Vec<RedisValue>> =
                pipe.query_async(&mut connection).await;

            result.map_err(redis_api_error)
        },
    )
    .await
}

async fn execute_operation<T, F, Fut>(
    target: Arc<RedisTarget>,
    operation_name: &'static str,
    timeout_message: &'static str,
    request_timeout: Duration,
    acquire_timeout: Duration,
    metrics: Metrics,
    operation: F,
) -> Result<T, ApiError>
where
    F: FnOnce(ConnectionManager) -> Fut,
    Fut: Future<Output = Result<T, ApiError>>,
{
    let target_id = target.id().to_owned();
    let task_id = target_id.clone();

    let operation_metrics = metrics.begin_operation(target_id.clone(), operation_name);

    let result = timeout(request_timeout, async move {
        let _permit = timeout(acquire_timeout, target.acquire_operation())
            .await
            .map_err(|_| {
                warn!(
                    target = %task_id,
                    timeout_ms = acquire_timeout.as_millis(),
                    "Redis operation limiter saturated"
                );
                ApiError::too_many_requests("Redis operation capacity exhausted")
            })?
            .map_err(|error| {
                error!(%error, target = %task_id, "Redis operation limiter closed");
                ApiError::unavailable("Redis backend unavailable")
            })?;

        let connection = target.connection().await.map_err(|error| {
            error!(%error, target = %task_id, "Redis connection failed");
            ApiError::unavailable("Redis backend unavailable")
        })?;

        operation(connection).await
    })
    .await;

    match result {
        Ok(Ok(value)) => {
            operation_metrics.success();
            Ok(value)
        }
        Ok(Err(error)) => {
            operation_metrics.error();
            Err(error)
        }
        Err(_) => {
            operation_metrics.timeout();

            warn!(
                target = %target_id,
                timeout_ms = request_timeout.as_millis(),
                "{}",
                timeout_message
            );

            Err(ApiError::timeout(timeout_message))
        }
    }
}

fn append_commands(pipe: &mut redis::Pipeline, commands: Vec<RedisCommand>) {
    for command in commands {
        pipe.cmd(command.name.as_str());

        for arg in command.args {
            pipe.arg(arg.as_slice());
        }
    }
}
