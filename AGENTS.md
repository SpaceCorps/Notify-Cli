# AGENTS.md

Developer and agent operating architecture notes for `Notify-Cli`.

`notify` is a native Rust CLI for cross-platform notifications (Slack webhooks, Gmail REST API, native desktop toast banners, and modal GUI message boxes), built to be driven by humans and autonomous LLM agents. It replaced the .NET global tool `Notify.Console` by Niels Bosma and retains its command interface while upgrading performance, security, and agent discovery.

For the manual an agent reads at runtime, invoke `notify agent-readme` (or `notify agent-readme --json`). This file is for developers and agents maintaining or extending the Rust codebase.

---

## Build & Test Quality Gates

```bash
cargo build --release              # target/release/notify
cargo test --locked                # unit tests + in-process mock integration tests
cargo clippy --all-targets --locked -- -D warnings
cargo fmt --check
cargo install --path . --locked    # install to PATH
```

### Isolated Testing Environment

Always test with an isolated configuration directory so tests never access real credentials:

```bash
export NOTIFY_CONFIG_DIR=$(mktemp -d) NOTIFY_SECRET_STORE=plaintext NOTIFY_ALLOW_PLAINTEXT_STORE=1 NOTIFY_MOCK_SYSTEM=1
```

| Environment Variable | Effect |
|:---|:---|
| `NOTIFY_CONFIG_DIR` | Overrides the configuration directory for `config.yaml` and keystore files |
| `NOTIFY_SECRET_STORE` | Forces a specific backend: `dpapi`, `keychain`, `libsecret`, `plaintext` |
| `NOTIFY_ALLOW_PLAINTEXT_STORE=1` | Permits fallback to 0600 plaintext file when no OS keystore is available |
| `NOTIFY_MOCK_SYSTEM=1` | Bypasses actual GUI/OS notification calls during automated test suites |
| `NOTIFY_EMAIL_API_URL` | Overrides Gmail API base URL for offline mock testing in `tests/cli.rs` |

---

## Project Layout

```
Cargo.toml           # Rust 2024 edition, dependencies, release profile
Cargo.lock           # Deterministic locked dependencies
rustfmt.toml         # max_width = 120, use_small_heuristics = "Max"
LICENSE              # MIT License
README.md            # Modern badge-driven README
AGENTS.md            # Architecture notes and invariants
src/
  main.rs            # Arg parsing, --json pre-scan, clap error formatting
  cli.rs             # Clap derive command definitions & help strings
  client.rs          # Blocking HTTP client (ureq + rustls), status -> ErrorCode mapping
  error.rs           # ErrorCode enum and structured Error { code, message, detail, remediation }
  output.rs          # YAML default (serde_norway), JSON (--json), write_error, obj! macro
  account.rs         # Multi-account resolution and profile lookup
  config.rs          # Atomic config file writes, file locking (.lock), owner permissions
  secrets.rs         # macOS Keychain, Linux secret-tool, Windows DPAPI, plaintext fallback
  system.rs          # Platform OS notifications (osascript/notify-send/powershell) & dialogs
  readme.rs          # agent-readme command handler and embedded agent rules
  commands/
    mod.rs           # Subcommand router
    slack.rs         # Slack webhook dispatch
    email.rs         # Gmail REST API, MIME assembly, attachment handling, drafts
    system.rs        # Native desktop notification dispatch
    message_box.rs   # Modal GUI message box dialog dispatch
    accounts.rs      # Account management (list, add, test, remove)
tests/
  cli.rs             # In-process TCP mock HTTP server integration test suite
docs/                # GitHub Pages SEO website, agent discovery manifests, trust anchors
```

---

## Core Invariants

1. **Blocking HTTP, No Tokio Runtime**: CLIs perform sequential or scoped parallel requests. Spawning an async runtime adds 20–50 ms startup latency and increases binary size. `ureq 3.4` with `rustls` provides pure blocking I/O with 1–3 ms cold start times.
2. **Secrets Never Touch Plaintext**: Webhook URLs, API tokens, and private secrets are stored in native OS keystores (`macOS Keychain`, `Windows DPAPI`, `Linux secret-tool`). The plaintext file is strictly an explicit opt-in via `NOTIFY_ALLOW_PLAINTEXT_STORE=1`.
3. **Structured Outputs on Stdout and Stderr**: Stdout is always valid YAML or JSON. Deletes and actions that produce no upstream body emit a status object (e.g. `{"status": "ok", ...}`) so stdout is never empty.
4. **Stable Error Envelope**: Non-zero exit codes are strictly mapped to the `ErrorCode` enum:
   - `1`: `error`
   - `2`: `network`
   - `3`: `auth_required`
   - `4`: `not_found`
   - `5`: `rate_limited`
   - `6`: `invalid_input`
   - `7`: `no_account`
5. **Atomic Config Updates**: Modifying accounts holds a cross-process lock (`.lock`) and writes via atomic temporary file renaming with `chmod 0600` on POSIX platforms.
