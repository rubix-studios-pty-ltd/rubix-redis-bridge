## [1.1.1] - 2026-07-21

### Bug Fixes

- Update test configuration for RRB_TEST_URL and RRB_TOKEN
- Update docker-compose ports for local and Tailscale configurations
- Update port configuration in README and CI workflow to use 8080

### Miscellaneous Tasks

- Update dependencies to latest versions and bump schema version in biome.json
- Update packageManager version to pnpm@11.14.0
- Update redis, tokio, and tokio-macros dependencies to latest versions
- Update dependencies in Cargo.lock, package.json, and pnpm-lock.yaml to latest versions

## [1.1.0] - 2026-07-15

### Features

- Enhance token management and command authorization
- Realtime compatability with upstash realtime sdk
- Integrate Redis client into RedisTarget for improved connection management

### Bug Fixes

- Ambiguous response size metrics
- Bug ratelimit commands bypass fail close

### Refactor

- Rename AuthFailureResult to AuthFailure and update related logic
- Reorganize authentication module and remove unused tests
- Update module paths for headers, parse, and trusted components
- Enhance lockout functionality with new methods and improved assertions
- Reorganize security module and remove obsolete tests
- Remove obsolete hash and targets tests
- Remove obsolete test files for headers, parse, and trusted modules
- Remove obsolete authentication and lockout test files
- Reorganize Redis module and remove obsolete components
- Update module imports in auth test structure
- Update module exports for testing in various components
- Move is_locked_at method for better test organization
- Remove obsolete database test script
- Streamline module exports for testing across multiple components
- Remove obsolete authentication and security test files
- Update test messages for clarity and consistency
- Rename token_type module to token and update exports
- Clean up whitespace in Upstash SDK tests
- Rename TokenTypes to TokenCaps for improved clarity

### Documentation

- Update README to clarify Rubix Redis Bridge functionality
- Update README to reflect changes in token types and command authorization

### Miscellaneous Tasks

- Update npm version command to ignore scripts during release preparation
- Update dependencies and configuration settings
- Update dependencies in Cargo.lock
- Update GitHub Actions workflows to use latest action versions
- Update dependencies in Cargo.lock and package.json
- Update Node.js action version in build workflow to v7.0.0

## [1.0.1] - 2026-07-06

### Bug Fixes

- Connection shards
- Update environment variable references in README and code

### Refactor

- Simplify operation_limit assignment in load_env_target function

### Documentation

- Update README to reflect environment variable name change
- Update README table formatting for backend compatibility section

### Miscellaneous Tasks

- Update crossbeam-utils to version 0.8.22 and format code for clarity
- Update .dockerignore, .gitignore, and environment configurations

## [1.0.0] - 2026-07-06

### Bug Fixes

- Dry run test release script

### Refactor

- Enhance release preparation script with dry run support and file restoration
- Rename RedisTarget to Redis and update related configurations

### Documentation

- Update backend compatibility details in README
- Correct wording in README for clarity on security recommendations
- Improve clarity and detail in README content
- Refine README for improved clarity and conciseness
- Clarify deployment guidelines in THREAT_MODEL.md

### Miscellaneous Tasks

- Update changelog formatting and ensure proper line breaks
- Update changelog formatting and commit parser groups for consistency
- Add newline to changelog body for improved formatting
- Update package name and configuration in package.json
- Update configuration and enhance token management

## [0.3.3] - 2026-07-05

### Refactor

- Restructure Redis response handling and serialization
- Unify Redis value handling across modules

### Miscellaneous Tasks

- Update package manager and reorder devDependencies
- Update dependencies and improve Redis response handling

## [0.3.2] - 2026-07-05

### Features

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

### Bug Fixes

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

### Other

- Reorganise config.rs to modular managable files
- Organise env checks

### Refactor

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

### Documentation

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

### Styling

- Format imports in lockout tests for consistency

### Testing

- Enhance Upstash Redis tests with additional command coverage and error handling

### Miscellaneous Tasks

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
