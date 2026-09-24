# Notify CLI

[![Release](https://img.shields.io/github/v/release/SpaceCorps/Notify-Cli?color=blue&label=version)](https://github.com/SpaceCorps/Notify-Cli/releases/latest)
[![CI](https://github.com/SpaceCorps/Notify-Cli/actions/workflows/ci.yml/badge.svg)](https://github.com/SpaceCorps/Notify-Cli/actions/workflows/ci.yml)
[![Docs](https://img.shields.io/badge/docs-online-success)](https://spacecorps.github.io/Notify-Cli/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

A blazing fast, native command-line tool and agent interface for sending notifications across Slack, Email (Gmail REST API), native OS desktop notifications, and modal message boxes. Built in Rust 2024 for developers and autonomous AI workflows.

Originally created as `Notify.Console` by [Niels Bosma](https://github.com/nielsbosma/Notify.Console). Re-architected in Rust under [SpaceCorps](https://github.com/SpaceCorps).

---

## Highlights

- ⚡ **Sub-3ms Startup**: Compiled as a native static binary with zero runtime dependencies. Cold starts in 1–3 ms.
- 💬 **Slack Webhooks**: Post text messages, custom JSON, or Slack Block Kit components directly to incoming webhooks with channel and username overrides.
- ✉️ **Gmail REST API**: Send plain text or rich HTML emails with auto-fallback, multiple recipients (To, CC, BCC), attachments (up to 25MB), or draft creation (`--draft`).
- 🖥️ **Native Desktop Alerts**: Non-blocking desktop toast notifications on macOS (`osascript`), Linux (`notify-send`), and Windows (PowerShell balloon tips).
- 🛑 **Modal Message Boxes**: Blocking GUI dialogs on macOS (`display dialog`), Linux (`zenity`), and Windows (`MessageBox`) for critical user confirmation.
- 🔐 **OS Keystore Integration**: Secrets never touch plaintext files without explicit opt-in. Stored securely in macOS Keychain, Windows DPAPI, or Linux Secret Service (`secret-tool`).
- 🤖 **AI Agent Native**: Emits YAML by default for terminal readability, structured JSON via `--json`, standardized stderr error envelopes with remediation guidance, and embedded `notify agent-readme`.

---

## Installation

### Using Cargo

```bash
cargo install --git https://github.com/SpaceCorps/Notify-Cli --locked
```

### Pre-built Standalone Binaries

Download standalone binary archives directly from the [GitHub Releases](https://github.com/SpaceCorps/Notify-Cli/releases/latest) page:

| Platform | Architecture | Binary Package |
|:---|:---|:---|
| **macOS** | Apple Silicon (`aarch64`) | [`notify-v1.0.0-aarch64-apple-darwin.tar.gz`](https://github.com/SpaceCorps/Notify-Cli/releases/download/v1.0.0/notify-v1.0.0-aarch64-apple-darwin.tar.gz) |
| **macOS** | Intel (`x86_64`) | [`notify-v1.0.0-x86_64-apple-darwin.tar.gz`](https://github.com/SpaceCorps/Notify-Cli/releases/download/v1.0.0/notify-v1.0.0-x86_64-apple-darwin.tar.gz) |
| **Linux** | x86_64 (musl static) | [`notify-v1.0.0-x86_64-unknown-linux-musl.tar.gz`](https://github.com/SpaceCorps/Notify-Cli/releases/download/v1.0.0/notify-v1.0.0-x86_64-unknown-linux-musl.tar.gz) |
| **Windows**| x64 (MSVC) | [`notify-v1.0.0-x86_64-pc-windows-msvc.zip`](https://github.com/SpaceCorps/Notify-Cli/releases/download/v1.0.0/notify-v1.0.0-x86_64-pc-windows-msvc.zip) |

---

## Quickstart

### 1. System Notifications (Non-Blocking)

```bash
notify system --title "Build Finished" --description "All tests passed in 3.4s"
```

| Platform | Underlying Engine |
|:---|:---|
| **macOS** | `osascript` / `display notification` |
| **Linux** | `notify-send` |
| **Windows**| PowerShell `System.Windows.Forms.NotifyIcon` |

### 2. Modal Message Boxes (Blocking)

Blocks the calling process until the user clicks OK or dismisses the dialog:

```bash
notify message-box --title "Deploy Ready" --message "Review complete. Proceed with deployment?"
```

| Platform | Underlying Engine |
|:---|:---|
| **macOS** | `osascript` / `display dialog` |
| **Linux** | `zenity --info` |
| **Windows**| PowerShell `[System.Windows.Forms.MessageBox]::Show` |

### 3. Slack Webhooks

```bash
# Direct webhook URL
notify slack --webhook-url "https://hooks.slack.com/services/..." --message "Production deploy completed"

# Using a saved profile from OS keystore
notify slack alerts --message "High CPU alert on worker node 3"

# Custom Block Kit or JSON payload
notify slack alerts --json-payload '{"text":"Deploy done","blocks":[{"type":"section","text":{"type":"mrkdwn","text":"*Deploy done*"}}]}'

# Piping from stdin
cat alert.json | notify slack alerts
```

### 4. Email (Gmail REST API)

```bash
# Basic email
notify email work --to "team@company.com" --subject "Status" --body "All checks green"

# Multiple recipients & CC/BCC
notify email work \
  --to "dev1@company.com,dev2@company.com" \
  --cc "lead@company.com" \
  --subject "Release Notes" \
  --body "Version 1.0.0 is live"

# HTML email via stdin (automatic HTML detection & plain-text fallback)
cat report.html | notify email work --to "team@company.com" --subject "Weekly Report"

# Email with file attachments
notify email work \
  --to "client@company.com" \
  --subject "Invoices" \
  --body "Attached files for review" \
  --attach invoice.pdf \
  --attach summary.xlsx

# Create draft in Gmail without sending
notify email work --to "client@company.com" --subject "Draft Proposal" --body "Draft..." --draft
```

### 5. Managing Accounts & Profiles

```bash
# List all accounts
notify accounts list --check

# Store Slack incoming webhook URL in native OS keystore
notify accounts add alerts --type slack --webhook-url "https://hooks.slack.com/services/..." --channel "#alerts"

# Store Email configuration
notify accounts add work --type email --from "me@company.com" --default-to "team@company.com"

# Test account credentials
notify accounts test alerts

# Remove account and purge credentials from keystore
notify accounts remove alerts --yes
```

---

## Agentic Discovery & Automation

Notify CLI is designed for first-class autonomous AI agent integration:

- **Self-Documentation**: Run `notify agent-readme` (or `notify agent-readme --json`) for embedded instructions, operational invariants, and error mappings without network roundtrips.
- **Machine-Readable Outputs**: Pass `--json` to receive structured JSON on stdout and stderr.
- **Standardized Error Envelopes**: Exit codes map to structured categories (`auth_required`, `not_found`, `rate_limited`, `invalid_input`, `no_account`).

```json
{
  "error": "Slack profile 'missing' not found.",
  "code": "no_account",
  "detail": "Available profiles: alerts, team",
  "remediation": "Add this profile: notify accounts add missing --type slack --webhook-url <url>"
}
```

---

## Documentation & Links

- **Documentation Site**: [https://spacecorps.github.io/Notify-Cli/](https://spacecorps.github.io/Notify-Cli/)
- **Agent Overview (`llms.txt`)**: [https://spacecorps.github.io/Notify-Cli/llms.txt](https://spacecorps.github.io/Notify-Cli/llms.txt)
- **Exhaustive Agent Manual (`llms-full.txt`)**: [https://spacecorps.github.io/Notify-Cli/llms-full.txt](https://spacecorps.github.io/Notify-Cli/llms-full.txt)
- **Authentication Guide**: [auth.md](https://spacecorps.github.io/Notify-Cli/auth.md)
- **Pricing & Licensing**: [pricing.md](https://spacecorps.github.io/Notify-Cli/pricing.md)
- **About & Lineage**: [about.html](https://spacecorps.github.io/Notify-Cli/about.html)

---

## License

Notify CLI is open-source software licensed under the [MIT License](LICENSE).
Originally created by Niels Bosma as `Notify.Console`.
