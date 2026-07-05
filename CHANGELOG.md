## [0.3.3] - 2026-07-05

### 🚜 Refactor

- Restructure Redis response handling and serialization
- Unify Redis value handling across modules

### ⚙️ Miscellaneous Tasks

- Update package manager and reorder devDependencies
- Update dependencies and improve Redis response handling
## [0.3.2] - 2026-07-05

### 🚀 Features

- Prometheus /metrics
- Add metrics token support and enhance metrics authentication
- Startup log information
- Define allowed, denied, and connection commands for command handling
- Implement authentication lockout mechanism
- Enhance authentication lockout and proxy handling
- Enhanced lockout and proxy forward trust
- Additonal lockout metrics and rejection for prometheus
- Add new authentication lockout metrics to collectors
- Enhance Redis command execution with response size limits and acquire timeout
- Implement response size limits and error handling for Redis operations
- Enhance Redis command execution with response size limits

### 🐛 Bug Fixes

- Cosign image digest for docker
- Security tests
- Clean up whitespace in security tests
- Compatability with upstash rate limiter
- Lint and format
- Update allowed commands for Upstash compatibility and refactor related configurations
- Add missing newline at end of upstash test file
- Rust complaint for production unused constants.
- Type and import array
- Regression on blocked commands initialization
- Trusted proxy false should not error when trusted proxies are set.
- Function name for set lockout state
- Correct social media handles in SECURITY.md for accuracy

### 💼 Other

- Reorganise config.rs to modular managable files
- Organise env checks

### 🚜 Refactor

- Update job names in workflows for clarity
- Update default allowed and blocked commands in configuration
- Rename functions and test cases for clarity in redis_value and security modules
- Remove AppState and RedisTarget implementations; rename encoding functions in metrics and redis_value modules
- Rename token file permission function and update test names for clarity; remove unused security module
- Remove commented TODO for command constant relocation
- Resolve cargo clippy complaint 'unnecessary use of copied'
- Rename encoding functions for clarity and improve handling of unsupported RedisValue variants
- Remove redis_value module and update imports accordingly
- Reorganize security module and update command imports
- Move denied_commands function to a new location for improved clarity and organization
- Migrate TrustedProxies to client module
- Simplify authentication failure handling
- Enhance lockout test assertions for clarity
- Update test names for upstash ratelimit consistency

### 📚 Documentation

- Add RRB_METRICS_TOKEN to README for metrics authentication
- ZINCRBY is required by upstash redis.
- Update README.md with additional badges for CI, CodeQL, Dependabot, Socket, and License
- Update CI badge link in README.md
- Remove Socket badge from README.md
- Add additional badges for Tests and Release in README.md
- Consolidate badge display in README.md for improved readability
- Remove CodeQL workflow file and update badge display in README.md
- Updated docker image url
- Improve documentations

### 🎨 Styling

- Format imports in lockout tests for consistency

### 🧪 Testing

- Enhance Upstash Redis tests with additional command coverage and error handling

### ⚙️ Miscellaneous Tasks

- Release pipeline hardening
- Workflow premissions
- Workflow permissions
- Remove dependabot configuration file
- Update CodeQL action versions in workflow configuration
- Upgrade actions/checkout and Docker action versions in workflow files
- Upgrade pnpm and Node.js action versions in workflow files
- Update .dockerignore to include additional files and directories for better build context management
- Update num-bigint to version 0.4.7 and remove metrics module
- Bump version of rubix-redis-bridge to 0.2.2
- Bump version of rubix-redis-bridge to 0.2.3
- Update README and remove test workflow
- Add cargo deny check to release workflow and update cosign installer version
- Update .dockerignore to include .docs and reorder target directory for improved build context
- Bump version of rubix-redis-bridge to 0.3.0
- Clean up .dockerignore by removing test entry
- Update .dockerignore to include 'docs' and add SECURITY.md
- Update deny.toml to include BSL-1.0 license
- Remove unnecessary blank line in SECURITY.md for improved readability
- Release and changelog pipelines
- Remove incorrect commit
