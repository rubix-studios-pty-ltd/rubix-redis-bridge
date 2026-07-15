# Rubix Redis Bridge

Rubix Redis Bridge is a secure self-hosted HTTP gateway for Redis-compatible backends, designed for teams that use Upstash Redis SDKs but need private infrastructure, KV backend variant support, stricter command policy, and operational control.

It provides controlled Redis-over-HTTP access for private infrastructure, internal services, Docker networks, Tailscale networks, serverless workloads, and application integrations that cannot connect to Redis over TCP.

Applications can use the supported `@upstash/redis`, `@upstash/ratelimit`, and `@upstash/realtime` flows while the bridge enforces authentication, policy, limits, and per-target controls.

[![CI](https://github.com/rubix-studios-pty-ltd/rubix-redis-bridge/actions/workflows/ci.yml/badge.svg)](https://github.com/rubix-studios-pty-ltd/rubix-redis-bridge/actions/workflows/ci.yml) [![Release](https://github.com/rubix-studios-pty-ltd/rubix-redis-bridge/actions/workflows/release.yml/badge.svg)](https://github.com/rubix-studios-pty-ltd/rubix-redis-bridge/actions/workflows/release.yml) [![Dependabot](https://img.shields.io/badge/Dependabot-enabled-025E8C?logo=dependabot)](https://github.com/rubix-studios-pty-ltd/rubix-redis-bridge/network/updates) [![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)

## Features

Rubix Redis Bridge provides the core components required to expose Redis safely over HTTP for platforms, workloads, and applications that require API and SDK-compatible Redis access.

- Redis over HTTP
- `@upstash/redis` SDK support
- `@upstash/realtime` SDK support
- Single commands
- Pipeline requests
- Managed transactions
- Realtime SSE subscriptions
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
- Realtime limits
- Metrics token
- Non-root container

## Quick start

Env mode configures one Redis target from environment variables and is suitable for local testing, Docker deployments, and simple single-target bridge deployments.

```bash
docker run --rm -p 8080:8080 \
  -e RRB_MODE=env \
  -e RRB_TOKEN='replace-with-strong-http-token' \
  -e RRB_TOKEN_TYPE='command' \
  -e RRB_CONNECTION_STRING='redis://default:replace-with-redis-password@redis:6379' \
  -e RRB_OPERATION_LIMIT='100' \
  -e RRB_CONNECTION_SHARDS='4' \
  -e RRB_REQUEST_TIMEOUT_MS='5000' \
  -e RRB_ALLOWED_COMMANDS='PING,GET,SET,DEL,EXISTS,EXPIRE,TTL' \
  rubixvi/rubix-redis-bridge:latest
```

Test the bridge:

```bash
curl -sS http://127.0.0.1:8080/ \
  -H "Authorization: Bearer $RRB_TOKEN" \
  -H "Content-Type: application/json" \
  -d '["PING"]'
```

Expected response:

```json
{"result":"PONG"}
```

## API

Rubix Redis Bridge implements the Redis HTTP command flow used by `@upstash/redis` for supported commands, while enforcing bridge-level authentication, policy, and limits.

Available commands are controlled by `RRB_ALLOWED_COMMANDS`. Unsupported or blocked commands are rejected before Redis execution.

Endpoints:

```txt
GET  /
POST /
POST /pipeline
POST /multi-exec
POST /subscribe/<channel>
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

`POST /subscribe/<channel>` implements the SSE subscription flow used by `@upstash/redis`. It requires a token with the `realtime` capability. Each subscription receives a dedicated Redis Pub/Sub connection and does not consume ordinary command concurrency or a per-target operation permit.

## Upstash SDK

Rubix Redis Bridge has been tested with `@upstash/redis` across the supported command surface, including single commands, pipelines, managed transactions, and pipeline error handling.

Compatibility depends on the configured allowlist. If the SDK calls a command that is not allowed, the bridge rejects it.

Restricted allowlist example:

```bash
RRB_ALLOWED_COMMANDS=PING,GET,GETDEL,MGET,SET,SETEX,DEL,EXISTS,EXPIRE,TTL,INCR,DECR,HGET,HSET,HDEL,HMGET,HGETALL,ZINCRBY
```

### Realtime

`@upstash/realtime` uses `XADD`, `XRANGE`, `EXPIRE`, `PUBLISH`, and the SDK subscription endpoint. The `realtime` token capability grants only this command profile and `POST /subscribe/<channel>`. These commands do not need to be added to `RRB_ALLOWED_COMMANDS`.

```ts
import { Realtime } from '@upstash/realtime'
import { Redis } from '@upstash/redis'

const redis = new Redis({
  url: 'https://redis-bridge.example.com',
  token: process.env.RRB_TOKEN,
})

const realtime = new Realtime({
  redis,
  history: {
    expireAfterSecs: 3600,
    maxLength: 1000,
  },
})
```

Realtime channels may contain path separators such as `/`. Commas, line breaks, and null characters are rejected because they conflict with the Upstash SSE message framing.

## Backend

Rubix Redis Bridge connects to the backend through the Redis protocol. Compatibility depends on the backend implementation, the configured command allowlist, and whether Lua/script commands are required.

| Backend | Status | Notes |
| --- | --- | --- |
| Redis | Supported | Primary backend |
| Valkey | Supported | Compatible backend |
| Dragonfly | Supported | Compatible backend |
| Kvrocks | Supported | Compatible backend |
| Garnet | Partial | Lua/script tests failed |

## Token types

`RRB_TOKEN_TYPE` defines which bridge capabilities a bearer token may access. Supported values are `command`, `ratelimit`, and `realtime`. Use a comma-separated list when a token needs more than one capability.

```bash
RRB_TOKEN_TYPE=command,ratelimit,realtime
```

`command` allows standard Redis commands on the HTTP command routes: `POST /`, `POST /pipeline`, and `POST /multi-exec`.

`ratelimit` allows the restricted Upstash rate-limit command profile on the same command routes. This currently permits `EVAL`, `EVALSHA`, and safe `SCRIPT` subcommands required by `@upstash/ratelimit`, while keeping `EVAL_RO`, `EVALSHA_RO`, `FCALL`, `FUNCTION`, and other high-risk commands denied. A token with only `ratelimit` can access command routes, but only for the ratelimit profile.

`realtime` allows the exact command profile required by `@upstash/realtime`: `EXPIRE`, `PUBLISH`, `XADD`, and `XRANGE`. It also allows `POST /subscribe/<channel>`. A token with only `realtime` cannot execute general Redis commands.

Use `command,realtime` when one Redis client also needs general command access. Add `ratelimit` only when the same token uses `@upstash/ratelimit`.

In file mode, set `token_type` per token. If omitted, the token defaults to `command`.

## Command policy

Command policy defines which Redis commands can be executed through the bridge and ensures unsafe or explicitly blocked commands are rejected before Redis execution.

`RRB_ALLOWED_COMMANDS` must resolve to a non-empty allowlist. If `RRB_ALLOWED_COMMANDS` is not provided, the bridge uses a conservative default allowlist. If `RRB_ALLOWED_COMMANDS` is explicitly empty, startup fails. This prevents accidental "allow everything" behaviour.

Command names are normalized by default:

```bash
RRB_ALLOWED_COMMANDS=get,set,del
```

becomes:

```txt
GET,SET,DEL
```

`RRB_BLOCKED_COMMANDS` is additive. The bridge applies default blocks first, then adds custom blocked commands. Custom config cannot remove default blocks or re-enable hard-denied commands. Scoped exceptions exist for the `ratelimit` command profile and `PUBLISH` with a `realtime` token. Any scoped command can still be disabled by adding it to `RRB_BLOCKED_COMMANDS`.

## Hard-denied commands

Hard-denied commands are blocked by bridge policy regardless of allowlist settings, providing a fixed safety boundary for high-risk Redis operations.

Default-denied scripting and function commands for normal `command` tokens:

```txt
EVAL, EVAL_RO, EVALSHA, EVALSHA_RO, FCALL, FCALL_RO, FUNCTION, SCRIPT
```

For tokens with `ratelimit`, only `EVAL`, `EVALSHA`, and validated `SCRIPT` subcommands are removed from the hard-deny path. For tokens with `realtime`, only `PUBLISH` receives a hard-deny exception. `SUBSCRIBE` and `UNSUBSCRIBE` remain blocked as raw commands and are managed through the dedicated SSE route and connection lifecycle.

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
| `RRB_MODE` | `file` | Configuration mode. Use `env` for a single Redis target or `file` for token-based multi-target routing |
| `RRB_CONFIG_FILE` | `/app/rrb-config/tokens.json` | File-mode configuration path |
| `TOKEN_RESOLUTION_FILE_PATH` | `/app/rrb-config/tokens.json` | Alternate file-mode configuration path |
| `RRB_CONNECTION_STRING` | none | Redis connection URL used in `env` mode |
| `RRB_TOKEN` | none | HTTP bearer token in `env` mode |
| `RRB_TOKEN_TYPE` | `command` | Comma-separated token capabilities. Supported values: `command`, `ratelimit`, `realtime` |
| `RRB_HASH_TOKEN` | none | HMAC-SHA256 key for file-mode token hashes |
| `RRB_METRICS_TOKEN` | none | Bearer token required to access `/metrics` |
| `RRB_TRUST_PROXY_HEADERS` | `false` | Enables client IP resolution from trusted proxy headers |
| `RRB_TRUSTED_PROXIES` | none | Trusted proxy IP addresses or CIDR ranges |
| `RRB_MAX_CONCURRENCY` | `1024` | Global HTTP API concurrency limit |
| `RRB_REALTIME_MAX_CONCURRENCY` | `1024` | Maximum active realtime SSE subscriptions |
| `RRB_OPERATION_LIMIT` | `100` | Maximum in-flight Redis operations per target |
| `RRB_CONNECTION_SHARDS` | `4` | Redis `ConnectionManager` shards per target |
| `RRB_ALLOWED_COMMANDS` | conservative default | Allowed Redis commands |
| `RRB_BLOCKED_COMMANDS` | secure default | Additional Redis commands to block |
| `RRB_AUTH_LOCKOUT_FAILURES` | `10` | Failed authentication attempts allowed before lockout |
| `RRB_AUTH_LOCKOUT_WINDOW_SECONDS` | `300` | Time window for counting authentication failures |
| `RRB_AUTH_LOCKOUT_SECONDS` | `300` | Lockout duration after too many authentication failures |
| `RRB_AUTH_LOCKOUT_MAX_ENTRIES` | `65536` | Maximum number of client IPs tracked by the lockout cache |
| `RRB_MAX_BODY_BYTES` | `1048576` | Maximum HTTP request body size in bytes |
| `RRB_MAX_PIPELINE_COMMANDS` | `1000` | Maximum Redis commands allowed in one pipeline request |
| `RRB_MAX_COMMAND_ARGS` | `256` | Maximum argument count per Redis command |
| `RRB_MAX_ARG_BYTES` | `262144` | Maximum byte size for a single Redis command argument |
| `RRB_MAX_RESPONSE_BYTES` | `10485760` | Maximum encoded JSON response size returned |
| `RRB_REQUEST_TIMEOUT_MS` | `5000` | Timeout for Redis connection acquisition and command execution after an operation permit is acquired |
| `RRB_ACQUIRE_TIMEOUT_MS` | `100` | Timeout while waiting for per-target Redis operation capacity |

`RRB_MAX_CONCURRENCY` limits the number of concurrent HTTP requests handled by the bridge. It is a global API-level limit.

`RRB_REALTIME_MAX_CONCURRENCY` independently limits active SSE subscriptions. A permit is retained until the client disconnects or the Redis Pub/Sub connection closes. Realtime connections do not use `RRB_MAX_CONCURRENCY`, `RRB_OPERATION_LIMIT`, or the shared command connection shards after setup.

`RRB_OPERATION_LIMIT` limits the number of Redis operations that may be in flight for a single Redis target. This is the per-target admission-control and backpressure limit.

`RRB_CONNECTION_SHARDS` controls how many Redis ConnectionManager instances are created lazily per target. Admitted Redis operations are distributed across these shards using round-robin selection. Typical values are 4 to 8. Keep this lower than `RRB_OPERATION_LIMIT`.

`RRB_ACQUIRE_TIMEOUT_MS` applies backpressure before Redis execution. When the target operation gate is saturated, the bridge rejects quickly with `429` instead of retaining many request bodies while waiting for Redis capacity.

`RRB_REQUEST_TIMEOUT_MS` covers Redis connection acquisition and command execution after an operation permit is available. A stuck backend returns `504`.

`RRB_MAX_RESPONSE_BYTES` limits the encoded JSON response returned by the bridge. This prevents unbounded HTTP responses from Redis values such as large lists, large hashes, or large binary strings.

## Trusted Proxy

`RRB_AUTH_LOCKOUT_FAILURES` counts failed authentication attempts per client IP within `RRB_AUTH_LOCKOUT_WINDOW_SECONDS`. When the threshold is reached, invalid or missing credentials from that IP are blocked for `RRB_AUTH_LOCKOUT_SECONDS`. A valid bearer token can still authenticate from the same IP.

`RRB_AUTH_LOCKOUT_MAX_ENTRIES` caps the number of tracked client IPs. Stale entries are cleaned before new entries are rejected.

`RRB_TRUST_PROXY_HEADERS` enables forwarded client IP resolution behind a trusted reverse proxy. It is disabled by default.

`RRB_TRUSTED_PROXIES` defines which proxy IPs or CIDR ranges are allowed to provide client IP headers.

```bash
RRB_TRUST_PROXY_HEADERS=true
RRB_TRUSTED_PROXIES=127.0.0.1/32,172.20.0.0/16
```

Only include proxies that overwrite or sanitize forwarded headers. Requests from untrusted peers cannot control the client IP used for authentication lockout.

Supported client IP headers are checked in this order:

```txt
CF-Connecting-IP
True-Client-IP
X-Forwarded-For
X-Real-IP
Forwarded
```

## File mode

File mode supports multiple bearer tokens and Redis targets, allowing one bridge deployment to route authenticated requests to separate Redis backends.

Mount the token config file at:

```txt
/app/rrb-config/tokens.json
```

Example:

```json
{
  "version": 1,
  "targets": [
    {
      "rrb_id": "primary_redis",
      "connection_string": "redis://default:password@redis:6379",
      "operation_limit": 100,
      "connection_shards": 4,
      "tokens": [
        {
          "id": "primary_app",
          "name": "Production app token",
          "hash": "0000000000000000000000000000000000000000000000000000000000000000",
          "token_type": "command,ratelimit,realtime",
          "enabled": true
        }
      ]
    },
    {
      "rrb_id": "secondary_redis",
      "connection_string": "redis://default:password@redis-two:6379",
      "operation_limit": 50,
      "connection_shards": 4,
      "tokens": [
        {
          "id": "secondary_app",
          "hash": "0000000000000000000000000000000000000000000000000000000000000000",
          "token_type": "command",
          "enabled": true
        }
      ]
    }
  ]
}
```

The client receives only the opaque bearer token. `rrb_id` and token `id` remain internal configuration values. They are not returned in Redis responses.

File mode always uses HMAC-SHA256. Set `RRB_HASH_TOKEN` and generate the stored hash from the full opaque token:

```bash
printf '%s' "$RRB_TOKEN" | openssl dgst -sha256 -hmac "$RRB_HASH_TOKEN" -binary | xxd -p -c 256
```

Store only the 64-character hex digest.

Keep the file private and mount it read-only:

```bash
chmod 600 tokens.json
```

The bridge logs a warning when the token file is publicly accessible.

## Health and metrics

Health and metrics endpoints support operational monitoring without placing Redis command execution and bridge status checks behind the same application request path.

`GET /healthz` reports process health.

`GET /readyz` reports startup readiness. It confirms that at least one Redis target loaded.

Backend failures are returned per command as `503`.

`GET /metrics` exposes Prometheus metrics and requires `RRB_METRICS_TOKEN`. Do not expose this endpoint to external networks. Bind it privately, scrape it over an internal network, or protect it behind trusted infrastructure access controls.

```bash
curl -sS http://127.0.0.1:8080/metrics \
  -H "Authorization: Bearer $RRB_METRICS_TOKEN"
```

Metrics:

| Metric | Purpose |
| --- | --- |
| `rrb_auth_failed_total` | Failed authentication attempts. |
| `rrb_auth_lockouts_total` | Client IP lockouts created. |
| `rrb_auth_locked_requests_total` | Requests rejected while locked out. |
| `rrb_auth_lockout_entry_limit_total` | Lockout table capacity rejections. |
| `rrb_auth_lockout_tracked_ips` | Currently tracked lockout entries. |
| `rrb_auth_lockout_locked_ips` | Currently locked client IPs. |
| `rrb_request_denied_total` | Pre-Redis denied requests by route and reason. |
| `rrb_command_denied_total` | Commands denied by bridge policy. |
| `rrb_redis_operations_total` | Redis operations executed. |
| `rrb_redis_operation_duration_seconds` | Redis operation latency. |
| `rrb_inflight_redis_operations` | In-flight Redis operations. |
| `rrb_realtime_total` | Accepted realtime connections. |
| `rrb_realtime_inflight` | Active realtime connections. |
| `rrb_configured_targets` | Loaded Redis targets. |

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
  - "127.0.0.1:8080:8080"
```

Tailscale-only binding:

```yaml
ports:
  - "<tailscale-ip>:8080:8080"
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
$env:RRB_TEST_URL = "http://127.0.0.1:8080"
$env:RRB_TOKEN = "replace-with-bridge-token"
pnpm test:sdk
```

## Deployment

Rubix Redis Bridge should be deployed as a private infrastructure service with tight network access, restricted command policy, explicit authentication, and operational monitoring.

Recommended:

- Random hex values for RRB_TOKEN
- Random hex values for RRB_METRICS_TOKEN
- Enable Redis authentication
- Narrow command access to the minimum required command set
- Lower request body limits where possible
- Place the bridge behind a reverse proxy
- Apply reverse proxy rate limits
- Restrict access by trusted IP ranges or private networking
- Monitor authentication failures, denied commands, lockouts, and latency
- Avoid exposing the Redis server directly to public networks

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Support

For support or inquiries:

- LinkedIn: [rubixvi](https://www.linkedin.com/in/rubixvi/)
- Website: [Rubix Studios](https://rubixstudios.com.au)

## Author

Rubix Studios  
[https://rubixstudios.com.au](https://rubixstudios.com.au)
