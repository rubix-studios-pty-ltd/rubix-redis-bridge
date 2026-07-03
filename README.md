# Rubix Redis Bridge

Rubix Redis Bridge is a small Rust HTTP bridge for Redis.

It provides a secure, production-ready harden Redis HTTP API with an Upstash-style request and response. It is intended for private infrastructure, internal services, Docker deployments, and controlled application integrations that need Redis over HTTP without exposing Redis directly.

[![Tests](https://github.com/rubix-studios-pty-ltd/rubix-redis-bridge/actions/workflows/test.yml/badge.svg)](https://github.com/rubix-studios-pty-ltd/rubix-redis-bridge/actions/workflows/test.yml) [![Release](https://github.com/rubix-studios-pty-ltd/rubix-redis-bridge/actions/workflows/release.yml/badge.svg)](https://github.com/rubix-studios-pty-ltd/rubix-redis-bridge/actions/workflows/release.yml) [![Dependabot](https://img.shields.io/badge/Dependabot-enabled-025E8C?logo=dependabot)](https://github.com/rubix-studios-pty-ltd/rubix-redis-bridge/network/updates) [![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)

## Compatibility

Rubix Redis Bridge supports the core Upstash-style HTTP command format used by `@upstash/redis`.

Available commands are controlled by `RRB_ALLOWED_COMMANDS`, and dangerous Redis command families are hard-denied by the bridge regardless of allowlist configuration.

Implemented endpoints:

```txt
GET  /
POST /
POST /pipeline
POST /multi-exec
GET  /healthz
GET  /readyz
GET  /metrics
```

`GET /metrics` is protected separately by `RRB_METRICS_TOKEN`.

Authentication uses bearer tokens:

```txt
Authorization: Bearer <token>
```

The bearer scheme is case-insensitive, so `Bearer`, `bearer`, or `BEARER` is acceptable.

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

Transaction:

```json
[
  ["SET", "counter", "1"],
  ["INCR", "counter"]
]
```

Use `POST /pipeline` for non-atomic batched Redis execution.

Use `POST /multi-exec` for bridge-managed Redis transactions.

## Upstash SDK

Rubix Redis Bridge has been tested with `@upstash/redis` for the supported command surface.

Confirmed working paths:

```txt
redis.set()
redis.get()
redis.ping()
redis.pipeline().exec()
redis.multi().exec()
pipeline.exec({ keepErrors: true })
```

Compatibility depends on the configured command allowlist. If a command is not included in `RRB_ALLOWED_COMMANDS`, the bridge rejects it even if the SDK supports it.

Example narrow production allowlist:

```bash
RRB_ALLOWED_COMMANDS=PING,GET,SET,DEL,EXISTS,EXPIRE,TTL,INCR,DECR,HGET,HSET,HDEL,HMGET,HGETALL,ZINCRBY
```

With allowlist, SDK methods using commands outside that set will fail by design.

## Upstash Ratelimit

`@upstash/ratelimit` uses Redis Lua scripting.

The normal flow is:

```txt
EVALSHA
fallback to EVAL when Redis returns NOSCRIPT
```

Rubix Redis Bridge supports this through an explicit compatibility flag:

```bash
RRB_UPSTASH_RATELIMIT=true
```

When this flag is enabled, the bridge adjusts the command policy for the Lua flow required by `@upstash/ratelimit`.

The compatibility mode allows these commands through policy:

```txt
EVAL
EVALSHA
SCRIPT
```

This is required because `@upstash/ratelimit` may call `EVALSHA` first, then fall back to `EVAL` when Redis does not already have the script cached.

Lua scripting gives the caller more power than simple Redis data commands.

Example production allowlist for Upstash Redis and Upstash Ratelimit compatibility:

```bash
RRB_UPSTASH_RATELIMIT=true
RRB_ALLOWED_COMMANDS=PING,GET,SET,DEL,EXISTS,EXPIRE,TTL,INCR,DECR,HGET,HSET,HDEL,HMGET,HGETALL,ZINCRBY,EVAL,EVALSHA,SCRIPT
```

## Security

Rubix Redis Bridge is designed to fail closed.

A request must pass authentication, request validation, command policy validation, and runtime limits before it reaches Redis.

Security controls include:

- Bearer token authentication
- Constant-time token comparison without early return across configured targets
- Redacted Debug implementations for token and Redis credential containers
- Redacted command debug output so Redis values are not dumped accidentally
- Request body size limit
- Maximum concurrent HTTP API request limit
- Maximum pipeline command count
- Maximum argument count per Redis command
- Maximum byte size per individual Redis argument
- Fail-closed command policy
- Conservative default command allowlist
- Non-overridable hard-deny list for dangerous Redis command families
- Default blocklist that is always applied
- Optional additive blocklist entries
- Non-root Docker runtime
- Docker health check
- SIGTERM and SIGINT graceful shutdown
- Health and readiness endpoints outside the API concurrency limiter
- Process-level readiness endpoint
- Per-request Redis operation timeout
- Per-target in-flight Redis operation cap

## Commands

`RRB_ALLOWED_COMMANDS` must resolve to a non-empty allowlist.

If `RRB_ALLOWED_COMMANDS` is not provided, the bridge uses a conservative default allowlist for common data commands.

If `RRB_ALLOWED_COMMANDS` is explicitly empty, startup fails. This prevents accidental "allow everything" behaviour.

Command names are normalized to uppercase at config-load time. Lowercase config such as this is accepted:

```bash
RRB_ALLOWED_COMMANDS=get,set,del
```

It is normalized internally to:

```txt
GET,SET,DEL
```

`RRB_BLOCKED_COMMANDS` is additive. The bridge applies the secure default blocklist first, then unions custom entries into that set.

Custom config cannot remove default blocks and cannot re-enable hard-denied commands.

## Hard-denied commands

The following commands are blocked inside the bridge regardless of allowlist or blocklist configuration.

Scripting and Redis Functions are hard-denied because they can execute nested Redis commands internally and bypass an outer command allowlist:

```txt
EVAL_RO, EVALSHA_RO, FCALL, FCALL_RO, FUNCTION, SCRIPT
```

Multiplexed administrative command families are hard-denied as whole families because safe and dangerous operations share the same top-level command name:

```txt
ACL, CLIENT, CLUSTER, COMMAND, CONFIG, MODULE, XGROUP
```

Connection-state and transaction-state commands are hard-denied because the bridge uses cloned `ConnectionManager` handles that share a multiplexed Redis connection. Allowing these commands can poison a reused connection for concurrent or later requests:

```txt
ASKING, AUTH, HELLO, QUIT, READONLY, READWRITE, RESET, SELECT, DISCARD, EXEC,
MULTI, UNWATCH, WATCH
```

The bridge also hard-denies destructive, replication, persistence, blocking, pub/sub subscription, expensive, and observability commands:

```txt
BGREWRITEAOF, BGSAVE, BLMOVE, BLMPOP, BLPOP, BRPOP, BRPOPLPUSH, BZPOPMAX,
BZPOPMIN, BZMPOP, DBSIZE, DEBUG, FLUSHALL, FLUSHDB, INFO, KEYS, LASTSAVE,
LATENCY, MEMORY, MIGRATE, MONITOR, PSUBSCRIBE, PSYNC, PUNSUBSCRIBE, PUBLISH,
PUBSUB, REPLCONF, REPLICAOF, RESTORE, ROLE, SAVE, SHUTDOWN, SLAVEOF, SLOWLOG,
SORT, SORT_RO, SSUBSCRIBE, SUNSUBSCRIBE, SUBSCRIBE, SWAPDB, SYNC, UNSUBSCRIBE,
WAIT, WAITAOF, XREAD, XREADGROUP
```

Use `POST /multi-exec` for transactions instead of raw `MULTI` and `EXEC`.

## Default allowed commands

The default allowlist includes common key, string, hash, list, set, sorted set, HyperLogLog, stream append/range, and scan commands.

For public or semi-public deployments, set `RRB_ALLOWED_COMMANDS` yourself and only include commands the consuming application actually needs.

Example narrow allowlist:

```bash
RRB_ALLOWED_COMMANDS=PING,GET,SET,DEL,EXISTS,EXPIRE,TTL,INCR,DECR,HGET,HSET,HDEL,HMGET,HGETALL,ZINCRBY,EVALSHA,EVAL,SCRIPT
```

## Health and readiness

`GET /healthz` returns whether the HTTP process is alive.

`GET /readyz` returns whether the bridge has loaded at least one Redis target and is ready to accept requests.

`GET /readyz` does not ping an arbitrary Redis backend. In a multi-target bridge, one Redis backend outage should not mark the entire bridge as unready. Backend failures are returned per command as `503`.

Health endpoints are mounted outside the API `ConcurrencyLimitLayer` and request body limit. This prevents Docker, systemd, Kubernetes, or external probes from queueing behind slow Redis traffic and falsely restarting the bridge under load.

The bridge handles both `SIGTERM` and `SIGINT`. Docker, Compose, systemd, and Kubernetes normally send `SIGTERM` on stop or redeploy, so graceful shutdown drains active requests instead of relying only on terminal `Ctrl+C`.

## Metrics

Prometheus metrics are exposed at:

```txt
GET /metrics
```

The metrics endpoint is protected by a separate bearer token:

```bash
RRB_METRICS_TOKEN=replace-with-strong-metrics-token
```

Metrics requests must include:

```txt
Authorization: Bearer <metrics-token>
```

`RRB_METRICS_TOKEN` is separate from `RRB_TOKEN`.

Use `RRB_TOKEN` for Redis API requests.

Use `RRB_METRICS_TOKEN` for Prometheus or compatible metrics scrapers.

This allows metrics access without exposing the Redis command token.

If `RRB_METRICS_TOKEN` is not configured, `/metrics` returns an authentication error.

Useful metrics include:

- rrb_auth_failed_total
- rrb_command_denied_total
- rrb_redis_operations_total
- rrb_redis_operation_duration_seconds
- rrb_inflight_redis_operations
- rrb_configured_targets

Metric purpose:

| Metric | Purpose |
| --- | --- |
| `rrb_auth_failed_total` | Counts failed bearer-token authentication attempts |
| `rrb_command_denied_total` | Counts commands rejected by bridge policy before Redis execution |
| `rrb_redis_operations_total` | Counts Redis operations by target, operation type, and result |
| `rrb_redis_operation_duration_seconds` | Tracks Redis operation latency |
| `rrb_inflight_redis_operations` | Shows current in-flight Redis operations |
| `rrb_configured_targets` | Shows how many Redis targets were loaded at startup |

Metrics are labelled with bridge target ids and operation types.

Prometheus scrape example:

```yaml
scrape_configs:
  - job_name: rubix-redis-bridge
    metrics_path: /metrics
    static_configs:
      - targets:
          - serverless-redis:8080
    authorization:
      type: Bearer
      credentials: your-metrics-token
```

Manual metrics test:

```bash
curl -sS http://127.0.0.1:7777/metrics \
  -H "Authorization: Bearer $RRB_METRICS_TOKEN"
```

## Runtime limits

`RRB_MAX_CONCURRENCY` limits concurrent HTTP API requests across the bridge.

Health endpoints bypass this limit.

`RRB_MAX_CONNECTIONS` limits concurrent Redis operations for a single target in env mode.

In file mode, each target’s `max_connections` value applies to that target.

`RRB_MAX_CONNECTIONS` is an in-flight Redis operation cap. It does not create that number of dedicated Redis TCP connections.

`RRB_REQUEST_TIMEOUT_MS` wraps Redis connection acquisition and command execution. A stuck or partitioned backend returns `504` instead of holding a concurrency slot indefinitely.

## Docker Compose

Build and start:

```bash
docker compose up -d --build
```

Default compose binding is local-only:

```yaml
ports:
  - "127.0.0.1:7777:8080"
```

For a Tailscale-only pattern, bind to the Tailscale IP:

```yaml
ports:
  - "<tailscale-ip>:7777:8080"
```

Do not publish this bridge directly to the public internet without Cloudflare, Traefik middleware, rate limiting, and strict command restrictions.

## Environment mode

Environment mode configures one Redis target from environment variables.

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

Supported environment variables:

| Variable | Default | Purpose |
| --- | ---: | --- |
| `RRB_HOST` | `0.0.0.0` | Bind host |
| `RRB_PORT` | `8080` | Bind port |
| `RRB_MODE` | `file` | `env` or `file` |
| `RRB_TOKEN` | none | HTTP bearer token in `env` mode |
| `RRB_METRICS_TOKEN` | none | Bearer token required to access `/metrics` |
| `RRB_UPSTASH_RATELIMIT` | `false` | Enables command policy compatibility for the `@upstash/ratelimit` Lua flow |
| `REDIS_URL` | none | Fallback Redis URL in `env` mode when `RRB_CONNECTION_STRING` is not set |
| `RRB_CONNECTION_STRING` | none | Redis URL in `env` mode |
| `RRB_MAX_CONNECTIONS` | `3` | Concurrent Redis operation cap per target |
| `RRB_ALLOWED_COMMANDS` | conservative data-command default | Command allowlist. Empty value fails startup |
| `RRB_BLOCKED_COMMANDS` | secure defaults plus custom entries | Additional blocked commands |
| `RRB_MAX_BODY_BYTES` | `1048576` | Request body limit in bytes |
| `RRB_MAX_CONCURRENCY` | `1024` | HTTP API concurrency limit |
| `RRB_MAX_PIPELINE_COMMANDS` | `1000` | Pipeline and transaction command count limit |
| `RRB_MAX_COMMAND_ARGS` | `256` | Per-command argument count limit |
| `RRB_MAX_ARG_BYTES` | `262144` | Per-argument byte limit |
| `RRB_REQUEST_TIMEOUT_MS` | `5000` | Timeout for Redis connection acquisition and command execution |
| `RRB_CONFIG_FILE` | `/app/rrb-config/tokens.json` | Multi-token file config path |
| `TOKEN_RESOLUTION_FILE_PATH` | `/app/rrb-config/tokens.json` | Alternative file config path |

## File mode

File mode supports multiple bearer tokens and Redis targets.

Mount a token config file at:

```txt
/app/rrb-config/tokens.json
```

Example config:

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

The token string is the JSON object key.

`max_connections` is used as the per-target in-flight Redis operation cap around the shared multiplexed Redis connection.

Keep this file private and mount it read-only.

On Unix hosts, set owner-only permissions such as:

```bash
chmod 600 tokens.json
```

The bridge logs a warning if the file is publicly accessible.

If `rrb_id` is omitted, the bridge derives a redacted hash-based target id so logs and metrics do not collapse to the same `redis_target` label.

## Test requests

Single command:

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

Pipeline:

```bash
curl -sS http://127.0.0.1:7777/pipeline \
  -H "Authorization: Bearer $RRB_TOKEN" \
  -H "Content-Type: application/json" \
  -d '[["SET","hello","world"],["GET","hello"]]'
```

Expected response:

```json
[{"result":"OK"},{"result":"world"}]
```

Transaction:

```bash
curl -sS http://127.0.0.1:7777/multi-exec \
  -H "Authorization: Bearer $RRB_TOKEN" \
  -H "Content-Type: application/json" \
  -d '[["SET","counter","1"],["INCR","counter"]]'
```

Expected response:

```json
[{"result":"OK"},{"result":2}]
```

Hard-deny test:

```bash
curl -sS http://127.0.0.1:7777/ \
  -H "Authorization: Bearer $RRB_TOKEN" \
  -H "Content-Type: application/json" \
  -d '["EVAL", "return redis.call(\"GET\",\"x\")", 0]'
```

Expected response:

```json
{"error":"Redis command is hard-denied by bridge policy: EVAL"}
```

With Upstash base64 response encoding:

```bash
curl -sS http://127.0.0.1:7777/ \
  -H "Authorization: Bearer $RRB_TOKEN" \
  -H "Content-Type: application/json" \
  -H "Upstash-Encoding: base64" \
  -d '["GET", "hello"]'
```

## Testing

Run Rust tests:

```bash
cargo test
```

Run SDK compatibility tests:

```bash
pnpm test:sdk
```

Run all tests:

```bash
pnpm test:all
```

Expected test coverage:

```txt
Rust security and config tests
Upstash SDK single command compatibility
Upstash SDK pipeline compatibility
Upstash SDK multi-exec compatibility
Upstash SDK keepErrors pipeline compatibility
```

On PowerShell, set test environment variables like this:

```powershell
$env:RRB_TEST_URL = "http://127.0.0.1:7777"
$env:RRB_TOKEN = "replace-with-bridge-token"
pnpm test:sdk
```

## Recommended

Use Rubix Redis Bridge as a private infrastructure service first.

Recommended deployment posture:

- Bind to 127.0.0.1, Docker network, or Tailscale IP
- Require a long random RRB_TOKEN
- Require Redis AUTH through the Redis connection string
- Keep hard-denied commands non-overridable
- Keep the default blocklist additive
- Use RRB_ALLOWED_COMMANDS for a narrow app-specific allowlist
- Keep RRB_MAX_BODY_BYTES and RRB_MAX_ARG_BYTES low unless large values are explicitly needed
- Put Cloudflare, Traefik, CrowdSec, and rate limits in front if externally exposed
- Avoid exposing raw Redis directly

For Dokploy or Compose, prefer the bridge port being available only on the Docker network, localhost, or Tailscale.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Support

For support or inquiries:

- LinkedIn: [rubixvi](https://www.linkedin.com/in/rubixvi/)
- Website: [Rubix Studios](https://rubixstudios.com.au)

## Author

Rubix Studios  
[https://rubixstudios.com.au](https://rubixstudios.com.au)
