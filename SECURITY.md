# Security policy

## Scope

This repository contains local Rust source and public engine-profile examples. It requires no API
keys, cloud credentials, accounts, telemetry or network access at runtime, and does not store
runtime data on disk.

Do not commit credentials, private recordings, machine-specific logs, `.env` files or private
calibration data. The `.gitignore` excludes common local artefacts, but contributors must inspect
staged changes before each commit.

## Reporting a vulnerability

Please use GitHub's private security-advisory reporting flow for this repository. Do not post a
security issue with reproduction details publicly before a maintainer has assessed it.

Potential issues include unsafe dependency use, unintended network access, or real-time callback
behaviour that causes denial of service. The crate forbids unsafe Rust and profiles are parsed as
data only.
