# Rubix Redis Bridge

Rubix Redis Bridge is a Rust HTTP bridge for Redis.

It provides controlled Redis-over-HTTP access for private infrastructure, internal services, Docker networks, Tailscale networks, serverless workloads, and application integrations that should not connect to Redis over TCP.

Applications can use the supported `@upstash/redis` SDK command flow while the bridge enforces authentication, command policy, runtime limits, and per-target operation controls.

[![CI](https://github.com/rubix-studios-pty-ltd/rubix-redis-bridge/actions/workflows/ci.yml/badge.svg)](https://github.com/rubix-studios-pty-ltd/rubix-redis-bridge/actions/workflows/ci.yml) [![Release](https://github.com/rubix-studios-pty-ltd/rubix-redis-bridge/actions/workflows/release.yml/badge.svg)](https://github.com/rubix-studios-pty-ltd/rubix-redis-bridge/actions/workflows/release.yml) [![Dependabot](https://img.shields.io/badge/Dependabot-enabled-025E8C?logo=dependabot)](https://github.com/rubix-studios-pty-ltd/rubix-redis-bridge/network/updates) [![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)

## Features

Rubix Redis Bridge provides the core components required to expose Redis safely over HTTP for internal platforms, controlled workloads, and applications that require SDK-compatible Redis access.

- Redis over HTTP
- `@upstash/redis` SDK support
- Single commands
- Pipeline requests
- Managed transactions
- Bearer token access
- Command policy controls
- Multi-target file mode
- Runtime limits
- Health checks
- Prometheus metrics
- Docker deployment

## Security

Security controls are applied before requests reach Redis, reducing the risk of unsafe commands, oversized payloads, unauthorised access, or uncontrolled backend usage.

- Token authentication
- Command allowlists
- Additive blocklists
- Hard-denied commands
- Request size limits
- Argument limits
- Redis timeouts
- Metrics token
- Non-root container

## Quick start

Env mode configures one Redis target from environment variables and is suitable for local testing, Docker deployments, and simple single-target bridge deployments.

```bash
docker run --rm -p 7777:8080 \
  -e RRB_MODE=env \
  -e RRB_TOKEN='replace-with-strong-http-token' \
  -e RRB_CONNECTION_STRING='redis://default:replace-with-redis-password@redis:6379' \
  -e RRB_MAX_CONNECTIONS='100' \
  -e RRB_REQUEST_TIMEOUT_MS='5000' \
  -e RRB_ALLOWED_COMMANDS='PING,GET,SET,DEL,EXISTS,EXPIRE,TTL' \
  rubixvi/rubix-redis-bridge:latest
```

Test the bridge:

```bash
curl -sS http://127.0.0.1:7777/ \
  -H "Authorization: Bearer $RRB_TOKEN" \
  -H "Content-Type: application/json" \
  -d '["PING"]'
```

Expected response:

```json
{"result":"PONG"}
```

## API

Rubix Redis Bridge implements the Redis HTTP command flow used by `@upstash/redis` for supported commands, while enforcing bridge-level authentication, command policy, and runtime limits.

Available commands are controlled by `RRB_ALLOWED_COMMANDS`. Unsupported or blocked commands are rejected before Redis execution.

Endpoints:

```txt
GET  /
POST /
POST /pipeline
POST /multi-exec
GET  /healthz
GET  /readyz
GET  /metrics
```

Authentication:

```txt
Authorization: Bearer <token>
```

The bearer scheme is case-insensitive.

Single command:

```json
["SET", "hello", "world"]
```

Pipeline:

```json
[
  ["SET", "hello", "world"],
  ["GET", "hello"]
]
```

Managed transaction:

```json
[
  ["SET", "counter", "1"],
  ["INCR", "counter"]
]
```

Use `POST /pipeline` for non-atomic batches.

Use `POST /multi-exec` for managed transactions. Raw `MULTI`, `EXEC`, `WATCH`, `UNWATCH`, and `DISCARD` are blocked because they alter connection state on shared Redis connections.

## SDK compatibility

Rubix Redis Bridge has been tested with `@upstash/redis` across the supported command surface, including single commands, pipelines, managed transactions, and pipeline error handling.

Confirmed paths:

```txt
redis.set()
redis.get()
redis.ping()
redis.pipeline().exec()
redis.multi().exec()
pipeline.exec({ keepErrors: true })
```

Compatibility depends on the configured allowlist. If the SDK calls a command that is not allowed, the bridge rejects it.

Restricted allowlist example:

```bash
RRB_ALLOWED_COMMANDS=PING,GET,GETDEL,MGET,SET,SETEX,DEL,EXISTS,EXPIRE,TTL,INCR,DECR,HGET,HSET,HDEL,HMGET,HGETALL,ZINCRBY
```

## Upstash Ratelimit

`@upstash/ratelimit` uses Redis Lua scripting for atomic rate-limit operations, which requires a narrower and more controlled command policy than standard Redis commands.

The package typically attempts `EVALSHA` first, then falls back to `EVAL` when Redis returns `NOSCRIPT`.

Enable Upstash Ratelimit:

```bash
RRB_UPSTASH_RATELIMIT=true
RRB_ALLOWED_COMMANDS=PING,GET,GETDEL,MGET,SET,SETEX,DEL,EXISTS,EXPIRE,TTL,INCR,DECR,HGET,HSET,HDEL,HMGET,HGETALL,ZINCRBY,EVALSHA,EVAL,SCRIPT
```

When enabled, `EVAL`, `EVALSHA`, and restricted `SCRIPT` calls can pass policy when also present in `RRB_ALLOWED_COMMANDS`.

`SCRIPT` remains restricted to supported script cache commands. Dangerous subcommands remain blocked.

Only enable this for trusted applications and private deployments.

## Command policy

Command policy defines which Redis commands can be executed through the bridge and ensures unsafe or explicitly blocked commands are rejected before Redis execution.

`RRB_ALLOWED_COMMANDS` must resolve to a non-empty allowlist.

If `RRB_ALLOWED_COMMANDS` is not provided, the bridge uses a conservative default allowlist. If `RRB_ALLOWED_COMMANDS` is explicitly empty, startup fails. This prevents accidental "allow everything" behaviour.

Command names are normalized by default:

```bash
RRB_ALLOWED_COMMANDS=get,set,del
```

becomes:

```txt
GET,SET,DEL
```

`RRB_BLOCKED_COMMANDS` is additive. The bridge applies default blocks first, then adds custom blocked commands.

Custom config cannot remove default blocks or re-enable hard-denied commands.

## Hard-denied commands

Hard-denied commands are blocked by bridge policy regardless of allowlist settings, providing a fixed safety boundary for high-risk Redis operations.

Default-denied scripting and function commands:

```txt
EVAL, EVAL_RO, EVALSHA, EVALSHA_RO, FCALL, FCALL_RO, FUNCTION, SCRIPT
```

When `RRB_UPSTASH_RATELIMIT=true`, only `EVAL`, `EVALSHA`, and restricted `SCRIPT` usage can be enabled through `RRB_ALLOWED_COMMANDS`.

Other denied command groups include administrative, connection-state, transaction-state, destructive, replication, persistence, blocking, pub/sub, expensive, and observability commands.

Examples:

```txt
ACL, AUTH, CLIENT, CLUSTER, COMMAND, CONFIG, DBSIZE, DEBUG, DISCARD,
EXEC, FLUSHALL, FLUSHDB, HELLO, INFO, KEYS, MEMORY, MODULE, MONITOR,
MULTI, PUBLISH, PUBSUB, QUIT, RESET, SAVE, SELECT, SHUTDOWN, SLOWLOG,
SUBSCRIBE, SYNC, WATCH, XGROUP, XREAD, XREADGROUP
```

Use `POST /multi-exec` instead of raw Redis transaction commands.

## Configuration

Configuration controls how the bridge binds, authenticates requests, connects to Redis, limits runtime behaviour, and loads single-target or multi-target settings.

| Variable | Default | Purpose |
| --- | --- | --- |
| `RRB_HOST` | `0.0.0.0` | Bind host |
| `RRB_PORT` | `8080` | Bind port |
| `RRB_MODE` | `file` | `env` or `file` |
| `RRB_TOKEN` | none | HTTP bearer token in `env` mode |
| `RRB_METRICS_TOKEN` | none | Bearer token required to access `/metrics` |
| `RRB_UPSTASH_RATELIMIT` | `false` | Enables ratelimit compatibility |
| `RRB_CONNECTION_STRING` | none | Redis URL in `env` mode |
| `RRB_MAX_CONNECTIONS` | `3` | Concurrent Redis operation cap per target |
| `RRB_ALLOWED_COMMANDS` | conservative default | Allowed commands |
| `RRB_BLOCKED_COMMANDS` | secure default | Additional blocked commands |
| `RRB_MAX_BODY_BYTES` | `1048576` | Request body limit in bytes |
| `RRB_MAX_CONCURRENCY` | `1024` | HTTP API concurrency limit |
| `RRB_MAX_PIPELINE_COMMANDS` | `1000` | Pipeline command count limit |
| `RRB_MAX_COMMAND_ARGS` | `256` | Per-command argument count limit |
| `RRB_MAX_ARG_BYTES` | `262144` | Per-argument byte limit |
| `RRB_REQUEST_TIMEOUT_MS` | `5000` | Timeout for Redis connection acquisition and execution |
| `RRB_CONFIG_FILE` | `/app/rrb-config/tokens.json` | File config path |
| `TOKEN_RESOLUTION_FILE_PATH` | `/app/rrb-config/tokens.json` | Alternate config path |

`RRB_MAX_CONNECTIONS` is an in-flight Redis operation cap. It does not create dedicated Redis TCP connections.

`RRB_REQUEST_TIMEOUT_MS` covers Redis connection acquisition and command execution. A stuck backend returns `504`.

## File mode

File mode supports multiple bearer tokens and Redis targets, allowing one bridge deployment to route authenticated requests to separate Redis backends.

Mount the token config file at:

```txt
/app/rrb-config/tokens.json
```

Example:

```json
{
  "token-one": {
    "rrb_id": "primary_redis",
    "connection_string": "redis://default:password@redis:6379",
    "max_connections": 100
  },
  "token-two": {
    "rrb_id": "secondary_redis",
    "connection_string": "redis://default:password@redis-two:6379",
    "max_connections": 50
  }
}
```

The JSON object key is the bearer token. `max_connections` sets the target operation cap.

Keep the file private and mount it read-only:

```bash
chmod 600 tokens.json
```

The bridge logs a warning when the token file is publicly accessible.

If `rrb_id` is omitted, the bridge derives a redacted target id for logs and metrics.

## Health and metrics

Health and metrics endpoints support operational monitoring without placing Redis command execution and bridge status checks behind the same application request path.

`GET /healthz` reports process health.

`GET /readyz` reports startup readiness. It confirms that at least one Redis target loaded. It does not ping every Redis backend.

Backend failures are returned per command as `503`.

`GET /metrics` exposes Prometheus metrics and requires `RRB_METRICS_TOKEN`.

```bash
curl -sS http://127.0.0.1:7777/metrics \
  -H "Authorization: Bearer $RRB_METRICS_TOKEN"
```

Metrics:

| Metric | Purpose |
| --- | --- |
| `rrb_auth_failed_total` | Auth failures |
| `rrb_command_denied_total` | Denied commands |
| `rrb_redis_operations_total` | Redis operations |
| `rrb_redis_operation_duration_seconds` | Redis latency |
| `rrb_inflight_redis_operations` | In-flight operations |
| `rrb_configured_targets` | Loaded targets |

Prometheus:

```yaml
scrape_configs:
  - job_name: redis-bridge
    metrics_path: /metrics
    static_configs:
      - targets:
          - redis-bridge:8080
    authorization:
      type: Bearer
      credentials: your-metrics-token
```

## Docker Compose

Docker Compose can run the bridge as a local, private, or network-restricted service depending on how the published port is bound.

Build and start:

```bash
docker compose up -d --build
```

Local-only binding:

```yaml
ports:
  - "127.0.0.1:7777:8080"
```

Tailscale-only binding:

```yaml
ports:
  - "<tailscale-ip>:7777:8080"
```

Do not publish the bridge directly to the public internet without network restrictions, rate limits, a reverse proxy, strict command access, and monitoring.

## Testing

The project includes Rust tests and SDK compatibility tests to validate bridge behaviour, command handling, and supported client flows.

Run Rust tests:

```bash
cargo test
```

Run SDK tests:

```bash
pnpm test:sdk
```

Run all tests:

```bash
pnpm test:all
```

PowerShell:

```powershell
$env:RRB_TEST_URL = "http://127.0.0.1:7777"
$env:RRB_TOKEN = "replace-with-bridge-token"
pnpm test:sdk
```

## Deployment

Rubix Redis Bridge should be deployed as a private infrastructure service with tight network access, restricted command policy, explicit authentication, and operational monitoring.

Recommended:

- Require a long random RRB_TOKEN
- Enable require Redis auth
- Narrow command access
- Lower body limits
- Add reverse proxy
- Add rate limits
- Add IP restrictions
- Monitor failures
- Avoid direct Redis

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Support

For support or inquiries:

- LinkedIn: [rubixvi](https://www.linkedin.com/in/rubixvi/)
- Website: [Rubix Studios](https://rubixstudios.com.au)

## Author

Rubix Studios  
[https://rubixstudios.com.au](https://rubixstudios.com.au)
