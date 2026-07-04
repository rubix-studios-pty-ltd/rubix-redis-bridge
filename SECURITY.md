# Security Policy

This document covers vulnerability reporting, supported versions, security scope, and disclosure handling for Rubix Redis Bridge.

Deployment hardening, command allowlist design, reverse proxy configuration, Redis backend hardening, and container runtime guidance should be documented in README or deployment-specific documentation rather than this file.

## Supported Versions

Security support applies to the current `0.3.x` release line.

| Version | Supported |
| --- | --- |
| `0.3.x` | Yes |
| `< 0.3.0` | No |

Use the latest patch release in the supported release line for production deployments.

## Reporting a Vulnerability

We take the security of Rubix Redis Bridge seriously. If you believe you have found a security vulnerability, please report it to us following these steps:

1. **DO NOT** create a public GitHub issue for the vulnerability.
2. Contact us directly at one of the following:
   - X: [@rubixvi](https://x.com/rubixstory)
   - Email: [Contact Form](https://rubixstudios.com.au/contact)
   - Facebook: [rubixvi](https://www.facebook.com/rubixstudios/)

Please include the following details in your report:

- Affected version, Docker tag, or commit SHA
- Deployment mode, such as `env` or `file`
- Relevant configuration values with secrets redacted
- Endpoint, request shape, and response observed
- Redis command or command class involved
- Expected behaviour and actual behaviour
- Potential impact
- Suggested mitigation, if known

Do not include live bearer tokens, Redis passwords, production connection strings, private customer data, or unrelated secrets in the report.

## Security Scope

Rubix Redis Bridge is a server-side Redis-over-HTTP bridge. It authenticates HTTP requests and applies command policy before Redis command execution.

The following issue classes are considered in scope for this security policy.

- Authentication bypass
- Bearer token handling flaws
- Metrics authentication bypass
- Command allowlist or blocklist bypass
- Hard-denied Redis command bypass
- Upstash Ratelimit scripting restriction bypass
- Request, argument, pipeline, or transaction limit bypass
- Trusted proxy or client IP handling flaws that affect lockout or access controls
- Secret disclosure through logs, errors, metrics, debug output, or release artifacts
- Container, release, signing, or supply-chain issues that materially affect project integrity
- Denial-of-service issues caused by missing or ineffective application-level limits

The following areas are generally out of scope unless they expose a separate vulnerability in Rubix Redis Bridge.

- Redis server misconfiguration
- Exposing the bridge publicly without network, proxy, or access controls
- Issues requiring possession of a valid bearer token without a policy bypass
- Brute force attempts that are already rate-limited or locked out as designed
- Network-layer DDoS attacks
- Browser-side misuse where bridge tokens are intentionally embedded in public clients
- Redis, operating system, reverse proxy, or container runtime CVEs outside this project
- Social engineering, phishing, or physical access attacks

## Security Model

Rubix Redis Bridge is intended to run as a private infrastructure component between trusted server-side applications and Redis.

It is not intended to be a public unauthenticated Redis API, a browser-facing Redis client, or a replacement for Redis server hardening.

A secure deployment still requires appropriate controls outside the bridge process, including private networking, TLS termination where applicable, strong bearer tokens, Redis authentication or ACLs, secret management, monitoring, and edge protection where the service is reachable through a proxy or public ingress.

## Response Process

1. We will acknowledge receipt of your vulnerability report within 48 hours.
2. Our security team will investigate and validate the issue.
3. We will keep you informed about the progress of fixing the vulnerability.
4. Once fixed, we will notify you and publish a security advisory if necessary.

## Security Update Policy

- Security patches are given the highest priority
- Updates will be released as soon as possible after a vulnerability is confirmed
- If a critical vulnerability is found, we will release a patch version immediately


## Security Monitoring

We continuously monitor our codebase for security issues through:

- Automated dependency scanning
- Regular code reviews
- Third-party security audits
- Community reports

## Disclosure Policy

- We follow responsible disclosure practices
- Security issues will be announced via our changelog and security advisories
- Credit will be given to security researchers who report valid vulnerabilities

## Contact

For any security-related questions, contact:

Rubix Studios  
Website: [https://rubixstudios.com.au](https://rubixstudios.com.au)
