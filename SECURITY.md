# Security Policy

## Reporting a vulnerability

Please **do not open a public issue** for security problems. Use GitHub's
[private vulnerability reporting](https://github.com/ludoviciphilip/PrismBI/security/advisories/new)
instead. You'll get an answer within a few days.

## Scope

Especially relevant: the auto-updater (Ed25519-signed `SHA256SUMS.txt`, release URL allowlist),
the webview CSP / Tauri IPC surface, credential storage (Secret Service / Windows Credential
Manager), and parsing of untrusted `.pbip` / `.pbix` / M / DAX input.

## Supported versions

Only the latest release receives fixes.
