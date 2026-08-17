# Security policy

## Scope

This repository contains local Rust source, public configuration examples and generated local
diagnostics only. It requires no API keys, cloud credentials, accounts, telemetry or network
access at runtime.

Do not commit credentials, private recordings, machine-specific logs, `.env` files or private
calibration data. The `.gitignore` excludes common local artefacts, but contributors must inspect
staged changes before each commit.

## Reporting a vulnerability

Please use GitHub's private security-advisory reporting flow for this repository. Do not post a
security issue with reproduction details publicly before a maintainer has assessed it.

Potential issues include unsafe dependency use, path traversal in profile loading, unintended
network access, disclosure through logs/crash reports, or real-time callback behaviour that causes
denial of service. The crate forbids unsafe Rust and profiles are parsed as data only.

## Local diagnostics

Daily logs and panic reports are written outside the repository to the operating system's local
application-data directory, with a temporary-directory fallback. Review them before attaching them
to an issue because crash reports include operating-system and backtrace details.
