# Threat Model

This document describes the security assumptions, trust boundaries, threat scenarios, and recommended controls for Rubix Redis Bridge.

Rubix Redis Bridge is a Redis-over-HTTP bridge for private infrastructure. It allows selected applications to execute supported Redis commands through an HTTP API while the bridge enforces bearer authentication, command policy, request limits, argument limits, Redis operation limits, and timeouts.

This document is intended for operators, maintainers, security reviewers, and contributors who need to understand the security model before deploying, modifying, or extending the bridge.

## Scope

The threat model covers the bridge process, its HTTP API, Redis command policy, token-based target routing, Redis backend access, metrics endpoint, Docker deployment model, and release artifact trust.

The following components are in scope.

| Component | In scope |
| --- | --- |
| HTTP API | `POST /`, `POST /pipeline`, `POST /multi-exec` |
| Operational endpoints | `GET /healthz`, `GET /readyz`, `GET /metrics` |
| Authentication | Bearer token validation for API and metrics access |
| Command policy | Allowlists, additive blocklists, hard-denied commands, Upstash Ratelimit exception handling |
| Request validation | JSON shape, pipeline size, argument count, argument byte limits, body size limit |
| Redis execution | Connection manager usage, per-target operation limits, Redis command timeout handling |
| Multi-target routing | File-mode token to Redis target mapping |
| Docker runtime | Non-root container execution and private binding expectations |
| Supply chain | Dependency checks, image scanning, SBOM, provenance, and image signing workflow |

The following components are outside the bridge security boundary.

| Component | Out of scope |
| --- | --- |
| Redis server hardening | Redis authentication, persistence, network isolation, ACLs, backup policy, and disk security |
| Reverse proxy security | TLS termination, WAF, IP allowlists, rate limiting, and client certificate policy |
| Application security | Security of applications that hold bridge bearer tokens |
| Host security | Docker host hardening, kernel updates, firewall policy, and container runtime security |
| Secret management platform | Storage, rotation, and audit controls for deployment secrets |
| Public internet access controls | Edge policy, Cloudflare Access, Tailscale ACLs, VPN policy, and external DDoS controls |

## Intended deployment model

Rubix Redis Bridge is intended for controlled private deployments.

Recommended deployment patterns include Docker networks, private service networks, Tailscale networks, internal reverse proxies, private Cloudflare Tunnel routes, and server-side workloads that need Upstash-style Redis HTTP access without exposing Redis over TCP.

The bridge should not be treated as a public unauthenticated Redis API, a general-purpose Redis admin proxy, or a replacement for Redis server hardening.

A secure deployment should meet these baseline conditions.

| Requirement | Expected control |
| --- | --- |
| Transport security | TLS is enforced by the reverse proxy or private overlay network |
| API access | Only trusted server-side applications hold bridge bearer tokens |
| Redis access | Redis is reachable only from the bridge or trusted private network peers |
| Metrics access | `/metrics` uses a separate strong token and is not publicly exposed |
| Network exposure | The bridge is bound to localhost, Docker-internal networking, a VPN address, or a protected reverse proxy |
| Command policy | `RRB_ALLOWED_COMMANDS` is kept narrow for the application use case |
| Runtime limits | Body size, pipeline size, argument limits, concurrency, and timeouts are configured intentionally |
| Secret handling | Tokens and Redis credentials are stored in a secret manager or protected environment variables |
| Logging | Logs are collected without exposing bearer tokens or Redis connection strings |

## Security objectives

The bridge is designed to meet the following security objectives.

| Objective | Description |
| --- | --- |
| Require authentication | Redis API requests must include a valid bearer token before command execution is attempted |
| Route by token | In file mode, each bearer token resolves to a configured Redis target |
| Fail closed on missing targets | The bridge must not start without a configured Redis target |
| Fail closed on empty allowlist | The bridge must not start when the command allowlist resolves to empty |
| Reject unsafe commands before Redis | Hard-denied, blocked, and non-allowed commands must be rejected before reaching Redis |
| Prevent connection-state abuse | Commands such as `SELECT`, `AUTH`, `HELLO`, `MULTI`, `EXEC`, `WATCH`, and `DISCARD` are blocked because they can alter shared connection state |
| Limit request size | Oversized HTTP bodies, pipelines, command argument counts, and argument byte sizes must be rejected |
| Bound backend execution | Redis connection acquisition and execution are bounded by timeout and per-target operation limits |
| Separate metrics access | Prometheus metrics require a separate metrics token rather than a Redis API token |
| Reduce secret exposure | Redis connection strings and tokens must not be emitted through debug output |
| Preserve private deployment assumptions | The bridge should be safe for private infrastructure, not advertised as a public Redis gateway |

## Non-goals

The bridge intentionally does not try to solve every Redis or platform security problem.

The following are non-goals.

| Non-goal | Explanation |
| --- | --- |
| Public anonymous access | Every Redis API request requires bearer authentication |
| Full Redis command compatibility | Administrative, blocking, scripting, pub/sub, transaction-state, and connection-state commands are restricted or denied |
| Redis ACL replacement | Redis should still use authentication and network isolation where practical |
| Public multi-tenant Redis hosting | The bridge is not a managed Redis service or tenant isolation platform |
| Browser client security | Bearer tokens should not be embedded in public frontend code |
| Payload content inspection | The bridge validates command shape and policy, but does not classify or inspect application data stored in Redis |
| Response size governance | Request size is controlled, but Redis response size depends on allowed commands and stored data |
| Edge DDoS protection | HTTP flood protection should be handled by the reverse proxy, firewall, CDN, or network layer |

## Trust boundaries

The deployment has several trust boundaries. Security depends on keeping these boundaries explicit.

```text
External client or application
        |
        | HTTPS or private network
        v
Reverse proxy, VPN, tunnel, or Docker network
        |
        | HTTP with bearer token
        v
Rubix Redis Bridge
        |
        | Redis protocol over private network
        v
Redis backend
```

### Boundary 1. Client to network edge

The client may be a server-side application, internal service, job runner, worker, monitoring system, or controlled automation. The edge may be a reverse proxy, VPN, Cloudflare Tunnel, Tailscale network, Docker network, or localhost binding.

Main risks at this boundary include public exposure, missing TLS, bearer token leakage, replay of stolen tokens, brute-force attempts, and HTTP request floods.

Recommended controls include TLS, IP allowlists, private network binding, VPN access, Cloudflare Access or equivalent, reverse proxy rate limits, and long random bearer tokens.

### Boundary 2. Network edge to bridge

The bridge receives HTTP requests and applies bearer authentication, body limits, concurrency limits, JSON parsing, command policy, and argument validation.

Main risks at this boundary include malformed JSON, oversized requests, invalid command arrays, blocked command attempts, abuse of pipelines, and excessive concurrent work.

Recommended controls include `RRB_MAX_BODY_BYTES`, `RRB_MAX_CONCURRENCY`, `RRB_MAX_PIPELINE_COMMANDS`, `RRB_MAX_COMMAND_ARGS`, `RRB_MAX_ARG_BYTES`, and strict command allowlists.

### Boundary 3. Bridge to Redis

The bridge connects to the configured Redis backend through a Redis connection manager. The Redis backend should be private and should not be directly reachable from untrusted networks.

Main risks at this boundary include backend overload, Redis timeout, large response generation, incorrect Redis credentials, over-broad command access, and accidental cross-target access.

Recommended controls include Redis authentication, private networking, per-target `RRB_MAX_CONCURRENCY`, `RRB_REQUEST_TIMEOUT_MS`, narrow allowlists, Redis resource limits, and operational monitoring.

### Boundary 4. Operator to configuration

Operators configure tokens, Redis connection strings, limits, and command policy. Configuration may be loaded from environment variables or a token configuration file.

Main risks at this boundary include weak tokens, accidental public binding, overly broad allowlists, enabling Upstash Ratelimit mode without understanding Lua implications, leaking Redis credentials in environment inspection, and permissive token file permissions.

Recommended controls include secret management, protected environment variables, owner-only config file permissions, config review, narrow command lists, and separate metrics tokens.

## Data flow

The API request flow is as follows.

1. A client sends an HTTP request to the bridge endpoint with `Authorization: Bearer <token>`.
2. The bridge parses the bearer token and compares it against configured tokens.
3. A matching token resolves to a Redis target.
4. The request body is parsed as JSON.
5. The bridge validates the command shape.
6. The command name is normalized to uppercase.
7. Hard-denied commands are rejected before allowlist evaluation, except for the explicit Upstash Ratelimit compatibility path.
8. The bridge checks the command allowlist and additive blocklist.
9. The bridge validates command argument count and argument byte size.
10. The bridge acquires a per-target operation permit.
11. The bridge acquires or reuses a Redis connection manager connection.
12. The Redis operation runs inside the configured timeout.
13. The Redis response is encoded as JSON and returned to the client.
14. Metrics are updated for operation count, duration, in-flight operations, errors, timeouts, and denied commands.

The metrics request flow is separate.

1. A client sends `GET /metrics` with `Authorization: Bearer <metrics-token>`.
2. The bridge validates the metrics token separately from Redis API tokens.
3. Metrics are rendered in Prometheus text format.
4. No Redis command is executed for metrics rendering.

## Assets

The following assets require protection.

| Asset | Sensitivity | Notes |
| --- | --- | --- |
| `RRB_TOKEN` | High | Grants access to the configured Redis target in env mode |
| File-mode bearer tokens | High | Grant access to their mapped Redis target |
| `RRB_METRICS_TOKEN` | Medium to high | Grants visibility into bridge metrics and target identifiers |
| Redis connection strings | High | Usually include Redis credentials and backend location |
| Redis data | High | Sensitivity depends on application data stored in Redis |
| Token config file | High | Contains bearer tokens and Redis connection strings |
| Docker image | Medium to high | Must be trusted because it runs near Redis and secrets |
| CI/CD credentials | High | Can publish release artifacts if compromised |
| Logs and metrics | Medium | May reveal target IDs, traffic patterns, failures, and operational state |

## Threat actors

The following threat actors are considered.

| Actor | Capability |
| --- | --- |
| Unauthenticated internet client | Can send HTTP requests if the bridge is publicly reachable |
| Authenticated but untrusted client | Has a valid bearer token but should only perform a limited command set |
| Compromised application | Holds a valid token and may issue abusive or unexpected Redis commands |
| Internal network peer | Can reach the bridge because of private network access |
| Malicious operator or misconfigured automation | Can change environment variables, token files, or deployment settings |
| Supply-chain attacker | Attempts to compromise dependencies, CI workflows, container image publishing, or release artifacts |
| Redis-level attacker | Has access to the Redis backend or can influence stored data returned through allowed commands |

## Main threats and mitigations

### 1. Public exposure of the bridge

A bridge exposed directly to the public internet receives unauthenticated scans, brute-force attempts, oversized requests, and exploit probes.

| Risk | Control |
| --- | --- |
| Token brute force | Use long random bearer tokens and edge rate limiting |
| Request floods | Use reverse proxy limits, firewall rules, CDN protection, and `RRB_MAX_CONCURRENCY` |
| Oversized bodies | Configure `RRB_MAX_BODY_BYTES` |
| Unauthorized command execution | Bearer authentication is required before command parsing and Redis execution |
| Discovery by scanners | Bind to `127.0.0.1`, Docker networks, VPN addresses, or protected reverse proxies |

Recommended position: do not expose the bridge directly to the public internet. Place it behind private networking, reverse proxy policy, or identity-aware access controls.

### 2. Bearer token theft

A stolen bearer token gives the holder access to the Redis target mapped to that token. The bridge cannot distinguish a stolen token from a legitimate client.

| Risk | Control |
| --- | --- |
| Token used from attacker infrastructure | Enforce IP allowlists, private networking, or identity-aware proxy controls |
| Token embedded in frontend code | Keep tokens server-side only |
| Token leaked through logs or scripts | Store tokens in secret managers and avoid printing environment values |
| Long-lived leaked token | Rotate tokens and revoke old tokens promptly |
| Shared token across many apps | Use file mode with separate tokens per app or target |

Tokens should be treated as privileged infrastructure credentials. They should not be stored in browser code, mobile apps, public repositories, screenshots, CI logs, or client-visible configuration.

### 3. Over-broad command allowlists

If `RRB_ALLOWED_COMMANDS` includes more commands than the application needs, a compromised client can do more damage with a valid token.

| Risk | Control |
| --- | --- |
| Accidental destructive writes | Keep application-specific allowlists narrow |
| Large key enumeration | Avoid broad use of `SCAN` and collection-wide commands unless needed |
| Excessive response generation | Avoid commands that return large collections for unbounded keys |
| Policy drift | Review allowlist changes during deployment reviews |

Recommended practice is to define the smallest command set required by the application. For example, a cache-only integration may need only `PING`, `GET`, `SET`, `SETEX`, `DEL`, `EXISTS`, `EXPIRE`, and `TTL`.

### 4. Hard-denied command bypass attempts

Some Redis commands are unsafe in a shared HTTP bridge because they alter server state, inspect server internals, block connections, subscribe to streams, change connection state, or execute arbitrary scripts.

The bridge hard-denies high-risk commands before Redis execution. Denied groups include scripting, functions, administrative operations, connection-state commands, transaction-state commands, destructive commands, blocking commands, pub/sub, replication, persistence, observability, and expensive keyspace commands.

Examples include `CONFIG`, `FLUSHALL`, `FLUSHDB`, `MONITOR`, `KEYS`, `MODULE`, `ACL`, `CLIENT`, `SELECT`, `AUTH`, `HELLO`, `MULTI`, `EXEC`, `WATCH`, `DISCARD`, `SUBSCRIBE`, `XREAD`, `EVAL`, `EVALSHA`, and `SCRIPT`. The `ratelimit` token type creates a narrow exception for `EVAL`, `EVALSHA`, and validated `SCRIPT` subcommands only.

| Risk | Control |
| --- | --- |
| Allowlist accidentally includes hard-denied command | Startup validation rejects commands that remain hard-denied even under the `ratelimit` profile |
| Runtime attempt to execute hard-denied command | Policy rejects command before Redis execution |
| Connection-state mutation | Connection-state commands are denied |
| Raw transaction state leakage | Raw transaction commands are denied and `/multi-exec` provides managed transaction behaviour |

### 5. Token type boundaries

Bearer tokens can be scoped by bridge capability using token types. Supported values are `command`, `ratelimit`, and `realtime`.

`command` allows standard Redis commands on the Redis HTTP command routes. `ratelimit` allows only the restricted Upstash rate-limit command profile on those command routes: `EVAL`, `EVALSHA`, and validated `SCRIPT` subcommands. `realtime` is accepted by configuration for the future realtime surface, but it does not enable Pub/Sub through the command routes.

| Risk | Control |
| --- | --- |
| Token receives more capabilities than needed | Use the smallest `RRB_TOKEN_TYPE` value required by the client |
| Realtime-only token attempts command access | Command route authorisation rejects the token before Redis execution |
| Future route expansion accidentally shares command policy | Keep token type checks at the route boundary |
| Ratelimit token becomes a general Lua bypass | Restrict the ratelimit profile to `EVAL`, `EVALSHA`, and validated `SCRIPT` subcommands |
| Operator assumes `realtime` enables Redis Pub/Sub through command routes | Document that token type controls route access only |

Recommended position: use `RRB_TOKEN_TYPE=command` for normal Redis HTTP command clients. Use `RRB_TOKEN_TYPE=command,ratelimit` for clients that also use `@upstash/ratelimit`. Use `RRB_TOKEN_TYPE=command,ratelimit,realtime` only when the same token must support standard commands, the ratelimit SDK, and a future realtime route.

### 6. Pipeline abuse

Pipelines can amplify backend work because one HTTP request can contain many Redis commands.

| Risk | Control |
| --- | --- |
| Large pipeline creates backend load | Configure `RRB_MAX_PIPELINE_COMMANDS` |
| Pipeline contains blocked command | Each command is independently validated before execution |
| Pipeline error hides partial failure | Pipeline responses preserve per-item errors |
| Excessive argument volume | Configure `RRB_MAX_COMMAND_ARGS` and `RRB_MAX_ARG_BYTES` |

Pipeline limits should be set according to application needs. The default limit is useful for compatibility but may be too high for exposed or high-risk deployments.

### 7. Large request or argument payloads

An attacker or defective client can send large bodies or large command arguments to consume memory, CPU, network bandwidth, or Redis resources.

| Risk | Control |
| --- | --- |
| Oversized HTTP body | `RRB_MAX_BODY_BYTES` limits request body size on API routes |
| Excessive command arguments | `RRB_MAX_COMMAND_ARGS` limits per-command argument count |
| Large individual argument | `RRB_MAX_ARG_BYTES` limits encoded argument size |
| Large pipeline | `RRB_MAX_PIPELINE_COMMANDS` limits command count |

Health, readiness, and metrics routes are not part of the Redis command API and should remain lightweight. The Redis command API routes are the routes that require strict request body limits.

### 8. Large Redis responses

Request size controls do not automatically bound response size. An allowed command can still return a large value or large collection if Redis contains large data.

| Risk | Control |
| --- | --- |
| Large value returned by `GET` | Application-level limits on stored value size |
| Large collection returned by `HGETALL`, `SMEMBERS`, `LRANGE`, or sorted-set range commands | Avoid broad collection-returning commands where not needed |
| Memory pressure in bridge process | Use narrow allowlists and Redis data design limits |
| Network pressure | Reverse proxy response limits where available |

Recommended practice is to avoid allowing commands that can return unbounded collections unless the application controls key size and cardinality. Prefer bounded range commands and application-level pagination patterns.

### 9. Redis backend overload

The Redis backend can be overloaded by high concurrency, slow commands, large responses, or too many clients sharing one target.

| Risk | Control |
| --- | --- |
| Too many simultaneous Redis operations | Configure per-target `RRB_MAX_CONCURRENCY` |
| Too many simultaneous HTTP requests | Configure `RRB_MAX_CONCURRENCY` and edge rate limits |
| Slow Redis backend | Configure `RRB_REQUEST_TIMEOUT_MS` |
| Backend unavailable | `/readyz` indicates target configuration readiness, while operation errors return unavailable or timeout responses |

`RRB_OPERATION_LIMIT` acts as an in-flight operation cap per target. It should be sized according to Redis capacity and application latency requirements.

### 10. Multi-target isolation failure

In file mode, the bridge routes requests to Redis targets based on bearer token. Incorrect token management can cause accidental cross-target access.

| Risk | Control |
| --- | --- |
| Reused token across environments | Use distinct tokens for each environment and target |
| Token file contains duplicate tokens after trimming | Startup validation rejects duplicate tokens after trimming |
| Token file has permissive permissions | On Unix, the bridge warns when the token file is readable or writable by group or others |
| Target identifiers leak sensitive names | Use neutral `rrb_id` values because metrics and logs may include target IDs |

File mode should use strong unique tokens and protected file permissions such as `0600` on Unix systems.

### 11. Metrics exposure

Metrics can reveal operation volume, failure rates, target identifiers, and denial patterns. Metrics should not be public.

| Risk | Control |
| --- | --- |
| Public scraping of `/metrics` | Configure `RRB_METRICS_TOKEN` and restrict network access |
| Shared API token used for metrics | Metrics token is separate from Redis API tokens |
| Sensitive target names visible in metrics | Use neutral target IDs |
| Denial metrics reveal probing patterns | Restrict metrics to trusted monitoring systems |

If `RRB_METRICS_TOKEN` is not configured, `/metrics` authentication fails. Operators should explicitly configure the metrics token for Prometheus scraping.

### 12. Information leakage through logs and errors

The bridge should not reveal Redis credentials, bearer tokens, or internal secrets through normal debug output. It may still return policy error messages that identify blocked commands.

| Risk | Control |
| --- | --- |
| Redis connection string appears in debug output | Config and target debug output redacts connection strings |
| Token appears in debug output | Avoid logging authorization headers at proxy and application layers |
| Error messages reveal policy | This is acceptable for authenticated clients but should not be exposed publicly |
| Target IDs reveal internal names | Use neutral `rrb_id` values |

Reverse proxies and observability tools should be configured to avoid logging `Authorization` headers.

### 13. Redis data confidentiality

The bridge protects access to Redis commands. It does not classify or encrypt Redis data at the application layer.

| Risk | Control |
| --- | --- |
| Sensitive data returned to authenticated client | Restrict tokens and command allowlists by application need |
| Cross-application data access | Use separate Redis databases, Redis instances, prefixes, or bridge targets where appropriate |
| Direct Redis access bypasses bridge | Keep Redis private and authenticated |
| Redis persistence exposes data at rest | Configure Redis persistence and host storage security according to data sensitivity |

Applications should avoid storing unnecessary sensitive data in Redis. Where sensitive data is required, use appropriate retention, encryption, and access controls outside the bridge.

### 14. Supply-chain compromise

The bridge runs near Redis credentials and application data. Release artifacts and dependencies must be treated as security-sensitive.

| Risk | Control |
| --- | --- |
| Vulnerable Rust dependency | Run dependency auditing in CI or release validation |
| Malicious container image | Verify image signatures and digests before production deployment |
| Image contains known critical vulnerabilities | Scan images before publication |
| Build provenance is unclear | Use SBOM and provenance-enabled image builds |
| Compromised GitHub Actions workflow | Protect repository access, tags, environments, and publishing credentials |

Recommended release controls include format checks, linting, tests, dependency audits, image scanning, SBOM generation, provenance, and Cosign image signing.

### 15. Misconfiguration

Misconfiguration is one of the most likely deployment risks.

| Misconfiguration | Impact | Recommended control |
| --- | --- | --- |
| Weak `RRB_TOKEN` | Token guessing or reuse risk | Use long random tokens |
| Public `0.0.0.0` binding without edge controls | Public attack surface | Bind privately or protect with reverse proxy policy |
| Broad `RRB_ALLOWED_COMMANDS` | Larger blast radius | Use application-specific allowlists |
| Over-scoped `RRB_TOKEN_TYPE` | Wider route access than required | Use the smallest token capability set needed |
| Missing `RRB_METRICS_TOKEN` | Metrics unavailable | Configure a separate metrics token for monitoring |
| Large pipeline limit | Request amplification | Lower `RRB_MAX_PIPELINE_COMMANDS` for exposed deployments |
| High concurrency | Backend overload | Set `RRB_MAX_CONCURRENCY` and `RRB_OPERATION_LIMIT` according to Redis capacity |
| Long request timeout | Resource retention under failure | Keep `RRB_REQUEST_TIMEOUT_MS` bounded |
| Permissive token file permissions | Local secret exposure | Use owner-only file permissions |

## Abuse cases

The following abuse cases should be considered during reviews and deployments.

### Abuse case 1. Stolen token writes to Redis

An attacker obtains a valid bridge token and uses allowed write commands to alter Redis data.

Expected result: the bridge authenticates the token and allows commands that are in policy. The bridge cannot determine whether the authenticated client is legitimate.

Required controls: prevent token theft, scope tokens by target, keep allowlists narrow, rotate compromised tokens, and use private networking or IP restrictions.

### Abuse case 2. Attacker attempts `FLUSHALL`

An attacker sends `['FLUSHALL']` with a valid token.

Expected result: the command is rejected by bridge policy before reaching Redis because it is hard-denied.

Required controls: keep hard-denied command checks in the request path and maintain tests for destructive command rejection.

### Abuse case 3. Compromised client attempts raw Redis transactions

A client sends `MULTI`, `EXEC`, `WATCH`, or `DISCARD` to manipulate connection state.

Expected result: raw transaction-state commands are rejected. Clients should use `POST /multi-exec` for managed transaction behaviour.

Required controls: continue blocking raw transaction-state commands and preserve atomic pipeline behaviour for `/multi-exec`.

### Abuse case 4. Client sends a huge pipeline

A client sends one HTTP request containing an excessive number of commands.

Expected result: the request is rejected if it exceeds `RRB_MAX_PIPELINE_COMMANDS`.

Required controls: set pipeline limits based on deployment needs and keep edge request rate limits in place.

### Abuse case 5. Realtime-only token accesses command route

A client presents a valid token configured with only the `realtime` token type and sends a request to `POST /`.

Expected result: authentication succeeds, but route authorisation rejects the request with forbidden access before Redis execution.

Required controls: keep token type checks at route boundaries and test command-route rejection for non-command tokens.

### Abuse case 6. Public scraper accesses `/metrics`

An unauthorised client requests `/metrics`.

Expected result: the bridge rejects access unless the request includes the configured metrics bearer token.

Required controls: configure `RRB_METRICS_TOKEN`, restrict network access to `/metrics`, and avoid public metrics exposure.

### Abuse case 7. Redis returns a very large value

A valid client requests a key that contains a very large value.

Expected result: the bridge encodes and returns the Redis response. Current request controls do not cap Redis response size.

Required controls: keep large-value commands limited to trusted applications, set application-level value size constraints, and consider future response-size controls if needed.

## Secure configuration guidance

### Minimal private cache profile

Use this profile for a private application cache that does not require Lua scripting.

```bash
RRB_MODE=env
RRB_PORT=8080

RRB_CONNECTION_STRING=redis://default:<redis-password>@redis:6379

RRB_TOKEN=<long-random-token>
RRB_METRICS_TOKEN=<long-random-metrics-token>

RRB_MAX_CONCURRENCY=128
RRB_OPERATION_LIMIT=20
RRB_CONNECTION_SHARDS=4

RRB_TOKEN_TYPE=command

RRB_ALLOWED_COMMANDS=PING,GET,SET,SETEX,DEL,EXISTS,EXPIRE,TTL

RRB_MAX_BODY_BYTES=262144
RRB_MAX_PIPELINE_COMMANDS=100
RRB_MAX_COMMAND_ARGS=32
RRB_MAX_ARG_BYTES=65536
RRB_MAX_RESPONSE_BYTES=1048576

RRB_ACQUIRE_TIMEOUT_MS=100
RRB_REQUEST_TIMEOUT_MS=3000
```

### Command, ratelimit, and realtime token profile

Use this profile when a trusted private application should be prepared for the existing command route, `@upstash/ratelimit`, and a future realtime route.

```bash
RRB_MODE=env

RRB_CONNECTION_STRING=redis://default:<redis-password>@redis:6379

RRB_TOKEN=<long-random-token>
RRB_TOKEN_TYPE=command,ratelimit,realtime
RRB_METRICS_TOKEN=<long-random-metrics-token>

RRB_MAX_CONCURRENCY=1024
RRB_OPERATION_LIMIT=100

RRB_ALLOWED_COMMANDS=PING,GET,GETDEL,MGET,SET,SETEX,DEL,EXISTS,EXPIRE,TTL,INCR,DECR,HGET,HSET,HDEL,HMGET,HGETALL,ZINCRBY

RRB_MAX_BODY_BYTES=1048576
RRB_MAX_PIPELINE_COMMANDS=1000
RRB_MAX_COMMAND_ARGS=256
RRB_MAX_ARG_BYTES=262144
RRB_MAX_RESPONSE_BYTES=10485760

RRB_ACQUIRE_TIMEOUT_MS=100
RRB_REQUEST_TIMEOUT_MS=5000
```

Do not treat `realtime` as a Redis command-policy bypass. It is a route capability only. Do not treat `ratelimit` as general Lua access. It is limited to the Upstash rate-limit command profile.

### Multi-target file mode profile

Use file mode when separate tokens should route to separate Redis targets.

```json
{
  "app-one-token": {
    "rrb_id": "app_one_cache",
    "connection_string": "redis://default:<password>@redis-one:6379",
    "operation_limit": 20,
    "connection_shards": 4
  },
  "app-two-token": {
    "rrb_id": "app_two_cache",
    "connection_string": "redis://default:<password>@redis-two:6379",
    "operation_limit": 20,
    "connection_shards": 4
  }
}
```

Recommended file permissions on Unix systems.

```bash
chmod 600 /app/rrb-config/tokens.json
```

## Deployment checklist

Use this checklist before deploying the bridge.

| Check | Required |
| --- | --- |
| Redis is not exposed directly to the public internet | Yes |
| Bridge is bound to localhost, Docker network, VPN address, or protected reverse proxy | Yes |
| TLS or private network transport is enforced | Yes |
| API bearer token is long, random, and stored as a secret | Yes |
| Metrics token is separate from API tokens | Yes |
| `Authorization` headers are not logged by proxies | Yes |
| `RRB_ALLOWED_COMMANDS` is limited to application needs | Yes |
| `RRB_TOKEN_TYPE` is limited to required route capabilities | Yes |
| Body, pipeline, argument, concurrency, and timeout limits are reviewed | Yes |
| Redis credentials are not committed to source control | Yes |
| File-mode config uses private file permissions | Yes, when file mode is used |
| Docker image digest or signature is verified for production | Recommended |
| Metrics are scraped only by trusted monitoring systems | Recommended |
| Alerts exist for authentication failures, command denials, backend errors, and timeouts | Recommended |

## Security testing expectations

Security-sensitive changes should include or preserve tests for the following behaviour.

| Area | Expected test coverage |
| --- | --- |
| Authentication | Missing, malformed, and invalid bearer tokens are rejected |
| Metrics authentication | `/metrics` rejects missing or invalid metrics token |
| Allowlist policy | Commands not in `RRB_ALLOWED_COMMANDS` are rejected |
| Hard-denied commands | Dangerous commands are rejected even if configured |
| Token type route checks | Tokens without `command` are rejected from command routes |
| Hard-denied scripting | `EVAL`, `EVALSHA`, and `SCRIPT` are rejected for normal `command` tokens even if configured in the allowlist |
| Pipeline limits | Excessive pipeline command counts are rejected |
| Argument limits | Excessive argument count and byte size are rejected |
| Transaction handling | Raw Redis transaction commands are blocked and `/multi-exec` uses managed atomic execution |
| Docker runtime | Container starts, healthcheck passes, and authenticated Redis command succeeds |
| Dependency and image checks | Dependency audit and image vulnerability scanning run during validation or release |

## Residual risks

The following risks remain even when the bridge is configured correctly.

| Risk | Reason |
| --- | --- |
| Authenticated clients can misuse allowed commands | The bridge enforces command policy, not application intent |
| Stolen tokens are sufficient for access | Bearer tokens are possession-based credentials |
| Large Redis responses can still consume memory or bandwidth | Request limits do not bound backend response size |
| Token capability scope must stay narrow | Extra token types widen route access as new bridge surfaces are added |
| Redis data sensitivity depends on application design | The bridge does not classify or encrypt Redis values |
| Private network exposure still matters | Internal services can be compromised or misconfigured |
| Supply-chain trust depends on release process integrity | Operators must verify artifacts and protect CI/CD permissions |

## Future hardening options

The following improvements may further reduce risk for higher-security deployments.

| Option | Benefit |
| --- | --- |
| Response size limit | Bounds memory and network usage from large Redis values |
| Per-token command policy | Allows different tokens to have different allowlists |
| Per-token rate limits | Reduces blast radius from one compromised client |
| Optional HMAC request signing | Reduces replay and token-only misuse risks |
| mTLS support at reverse proxy layer | Stronger client authentication for internal services |
| Structured audit log mode | Improves investigation of denied commands and failed auth attempts |
| Per-token route-specific policy | Separates command and realtime traffic as the route surface grows |
| Config validation report at startup | Makes effective policy easier to review without exposing secrets |

## Summary

Rubix Redis Bridge provides a narrow, security-focused Redis-over-HTTP layer for private infrastructure. Its main protections are bearer authentication, explicit command policy, hard-denied Redis command groups, request and argument limits, per-target operation limits, Redis timeouts, metrics authentication, non-root Docker execution, and release supply-chain controls.

The bridge should be deployed as a private infrastructure component. Its security depends on strong tokens, private network placement, narrow command allowlists, protected Redis backends, and careful token type scoping.

The most important operational rule is simple. Do not expose the bridge as a general public Redis API. Treat it as a controlled service boundary between trusted applications and private Redis backends.
