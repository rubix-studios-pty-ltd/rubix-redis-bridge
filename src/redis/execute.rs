use redis::Value;
use redis::aio::ConnectionManager;
use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;
use tokio::time::timeout;
use tracing::{error, warn};

use crate::app::{ApiError, RedisTarget};
use crate::metrics::Metrics;
use crate::security::RedisCommand;

use super::error::{redis_api_error, redis_error_message};
use super::response::RedisResponse;

pub(crate) async fn execute_command(
    target: Arc<RedisTarget>,
    command: RedisCommand,
    request_timeout: Duration,
    acquire_timeout: Duration,
    metrics: Metrics,
) -> Result<Value, ApiError> {
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

            let result: redis::RedisResult<Value> =
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
) -> Result<Vec<RedisResponse>, ApiError> {
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

            let result: redis::RedisResult<Vec<redis::RedisResult<Value>>> =
                pipe.ignore_errors().query_async(&mut connection).await;

            result
                .map(|items| {
                    items
                        .into_iter()
                        .map(|item| match item {
                            Ok(value) => RedisResponse::Result(value),
                            Err(error) => RedisResponse::Error(redis_error_message(&error)),
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
) -> Result<Vec<Value>, ApiError> {
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

            let result: redis::RedisResult<Vec<Value>> = pipe.query_async(&mut connection).await;

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

    let used_shard = Arc::new(AtomicUsize::new(usize::MAX));
    let used_generation = Arc::new(AtomicU64::new(0));

    let result = {
        let target = target.clone();
        let used_shard = used_shard.clone();
        let used_generation = used_generation.clone();

        timeout(request_timeout, async move {
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
                    error!(
                        %error,
                        target = %task_id,
                        "Redis operation limiter closed"
                    );

                    ApiError::unavailable("Redis backend unavailable")
                })?;

            let (shard, generation, connection) = target.connection().await.map_err(|error| {
                error!(
                    %error,
                    target = %task_id,
                    "Redis connection failed"
                );

                ApiError::unavailable("Redis backend unavailable")
            })?;

            used_generation.store(generation, Ordering::Relaxed);
            used_shard.store(shard, Ordering::Release);

            operation(connection).await
        })
        .await
    };

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

            let shard = used_shard.load(Ordering::Acquire);

            if shard != usize::MAX {
                let generation = used_generation.load(Ordering::Relaxed);

                if target.invalidate_connection(shard, generation).await {
                    warn!(
                        target = %target_id,
                        shard,
                        generation,
                        "Discarded Redis connection after timeout"
                    );
                }
            }

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
