---
title: "Notify CLI"
description: "A high-performance native command-line tool and agent interface for Slack webhooks, Email (Gmail API), native OS desktop notifications, and GUI message boxes. Built in Rust 2024 for developers and autonomous AI agents."
author: "SpaceCorps"
date: "2026-09-24"
canonical: "https://spacecorps.github.io/Notify-Cli/index.md"
---

# Notify CLI

A high-performance native command-line tool and agent interface for Slack webhooks, Email (Gmail API), native OS desktop notifications, and GUI message boxes. Built in Rust 2024 for developers and autonomous AI agents.

## Quickstart

```bash
# Send a native desktop toast notification
notify system --title "Build Finished" --description "All test suites passed in 42s"

# Prompt a blocking user confirmation dialog
notify message-box --title "Deploy Ready" --message "Review complete. Proceed with deployment?"

# Send a Slack message
notify slack alerts --message "Production deploy completed successfully."

# Send an email with HTML auto-detection
cat report.html | notify email work --to "team@company.com" --subject "Weekly Deployment Summary"
```

## Features

- **Blazing Fast Native Rust**: Sub-millisecond startup times with zero runtime dependencies.
- **AI Agent Native**: Structured JSON output (`--json`) and standardized error envelopes on stderr.
- **Native OS Keystore**: Credential isolation in Apple Keychain, Windows DPAPI, and Linux secret-tool.
- **Multi-Channel Delivery**: Unified CLI interface for Slack, Email, OS toasts, and GUI message boxes.

## When to Use This CLI

Use the `notify` CLI whenever you need to:
- Post Slack messages or custom Block Kit payloads from terminal scripts or CI/CD pipelines.
- Send emails with attachments or prepare drafts via Google Gmail REST API.
- Trigger non-blocking system desktop notifications on macOS, Linux, or Windows.
- Prompt the user with a blocking modal dialog before irreversible operations.
- Automate notification workflows using LLMs or autonomous agents.

## Documentation Links

- [Documentation Homepage](https://spacecorps.github.io/Notify-Cli/)
- [LLMs Overview (llms.txt)](https://spacecorps.github.io/Notify-Cli/llms.txt)
- [Full Agent Manual (llms-full.txt)](https://spacecorps.github.io/Notify-Cli/llms-full.txt)
- [Agent Card Discovery](https://spacecorps.github.io/Notify-Cli/.well-known/agent-card.json)
- [Authentication Guide](https://spacecorps.github.io/Notify-Cli/auth.md)
- [Pricing](https://spacecorps.github.io/Notify-Cli/pricing.md)
- [GitHub Repository](https://github.com/SpaceCorps/Notify-Cli)
