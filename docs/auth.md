---
title: "Authentication & Credential Guide"
description: "Authentication methods, credential storage, and error handling for developers and AI agents using Notify CLI."
author: "SpaceCorps"
date: "2026-09-24"
---

# Authentication & Credential Guide for Notify CLI

This document outlines authentication methods, credential storage, and security practices for developers and AI agents using Notify CLI.

## Overview
Notify CLI integrates with multiple notification providers:
1. **Slack**: Authenticated via Incoming Webhook URLs.
2. **Gmail API**: Authenticated via OAuth 2.0 access tokens or Bearer tokens.
3. **Local OS**: Native desktop notifications and GUI dialogs require no network authentication.

## Storing Credentials in Native OS Keystores

Notify CLI integrates directly with your operating system's native secure credential vault:
- **macOS**: Apple Keychain via `/usr/bin/security`
- **Linux**: Secret Service API via `secret-tool`
- **Windows**: Windows Data Protection API (DPAPI) via `windows-sys`

### Adding Slack Credentials

```bash
# Add with explicit webhook URL
notify accounts add alerts --type slack --webhook-url "https://hooks.slack.com/services/T00/B00/X00" --channel "#alerts"

# Or pipe secret securely via stdin (avoids shell history leaks)
printf '%s' "$SLACK_WEBHOOK_URL" | notify accounts add alerts --type slack --api-key-stdin
```

### Adding Email Credentials

```bash
# Add email profile with sender address
notify accounts add work --type email --from "me@company.com" --default-to "team@company.com"

# Or configure access token in keystore
notify accounts add work --type email --api-key "$GMAIL_OAUTH_TOKEN" --from "me@company.com"
```

## Environment Variables

For headless CI/CD runners or containerized environments:
- `NOTIFY_ALLOW_PLAINTEXT_STORE=1`: Explicit opt-in to store credentials in a local `chmod 0600` file when no OS keystore is present.
- `NOTIFY_CONFIG_DIR`: Override directory for configuration and keystore files.
- `GMAIL_TOKEN` / `NOTIFY_EMAIL_TOKEN`: Fallback bearer token for Gmail API requests.

## Verifying Accounts

```bash
# List all configured accounts
notify accounts list

# Verify stored secrets and test connectivity
notify accounts list --check
notify accounts test alerts
```
