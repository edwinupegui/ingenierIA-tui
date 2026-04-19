# Security Policy

## Reporting a Vulnerability

If you discover a security vulnerability, please **do not** open a public GitHub issue.

Instead, report it by opening a [GitHub Security Advisory](https://github.com/your-org/ingenieria-tui/security/advisories/new) (private disclosure).

Include:
- Description of the vulnerability
- Steps to reproduce
- Potential impact
- Suggested fix (if any)

You will receive a response within 72 hours. If confirmed, a patch will be released as soon as possible.

## Scope

- Source code in this repository
- Released binaries (`ingenieria` CLI)
- npm packages (`@your-org/ingenieria-*`)

## Out of Scope

- The backend server (`ingenieria-mcp`) — report those separately
- Issues in third-party dependencies (report upstream)

## OAuth App Note

The `COPILOT_CLIENT_ID` in `src/services/copilot.rs` is a placeholder. You must register your own [GitHub OAuth App](https://github.com/settings/developers) and replace it before building.
