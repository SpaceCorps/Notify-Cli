//! The manual an agent reads before its first call. Markdown by default so it can be pasted
//! into a system prompt or a CLAUDE.md; `--json` gives the same rules as data.

use crate::{obj, output};

pub fn print() {
    if output::json() {
        output::write(&obj! {
            "tool" => "notify",
            "version" => "1.0.0",
            "rules" => RULES,
            "exitCodes" => obj! {
                "0" => "ok",
                "1" => "error - unclassified, report and stop",
                "2" => "network - retry once, then stop",
                "3" => "auth_required - stop, surface the remediation to a human",
                "4" => "not_found - do not retry",
                "5" => "rate_limited - back off before retrying",
                "6" => "invalid_input - fix the call",
                "7" => "no_account - run notify accounts list",
            },
        });
        return;
    }
    println!("{README}");
}

const RULES: &[&str] = &[
    "Specify an account/profile or explicit destination flag when sending notifications.",
    "Run 'notify accounts list' first if you do not know which accounts exist.",
    "On code auth_required, stop and surface the remediation string. Do not retry.",
    "Use --json when you are going to parse the output in an automated tool loop.",
    "System notifications (notify system) show native toasts without blocking.",
    "Message boxes (notify message-box) block the calling process until dismissed by the user.",
    "Email messages require at least one recipient (--to) and a --subject.",
    "Secrets are kept securely in native OS keystores (Keychain, DPAPI, secret-tool).",
];

const README: &str = r#"# notify - agent operating manual

A cross-platform CLI tool for sending Slack messages, emails, native desktop notifications,
and GUI message box dialogs. Results are YAML on stdout, errors are YAML on stderr,
and `--json` switches both to JSON.

## Commands

### 1. Slack Messages (`notify slack`)

Send a Slack notification using an incoming webhook URL directly or via a saved account profile:

    notify slack my-profile --message "Deployment succeeded"
    notify slack --webhook-url "https://hooks.slack.com/..." --message "Alert: high CPU"
    notify slack my-profile --json-payload '{"text":"Deploy done","blocks":[{"type":"section","text":{"type":"mrkdwn","text":"*Deploy done*"}}]}'
    notify slack my-profile --json-file payload.json

### 2. Email Notifications (`notify email`)

Send emails or create drafts via Gmail API or configured profiles:

    notify email work --to user@example.com --subject "Build Finished" --body "All checks passed"
    cat report.html | notify email work --to team@example.com --subject "Daily Report"
    notify email work --to user@example.com --subject "Review" --file document.html --attach doc.pdf
    notify email work --to client@example.com --subject "Draft Proposal" --body "Draft..." --draft

### 3. Native Desktop Notifications (`notify system`)

Display a non-blocking toast/banner notification on macOS, Linux, or Windows:

    notify system --title "Build Finished" --description "All test suites passed in 42s"

### 4. Native GUI Message Boxes (`notify message-box`)

Display a modal dialog message box that blocks until dismissed:

    notify message-box --title "Action Required" --message "Please review the pull request."

### 5. Managing Accounts & Profiles (`notify accounts`)

Manage Slack profiles, email configurations, and OS keystore credentials:

    notify accounts list [--check]
    notify accounts add my-slack --type slack --webhook-url "https://hooks.slack.com/..."
    notify accounts add work-email --type email --from "user@company.com" --default-to "team@company.com"
    notify accounts test my-slack
    notify accounts remove my-slack --yes

## Credential Storage

Credentials and webhook URLs are stored in your platform's native secure keystore:
- macOS: Apple Keychain via `/usr/bin/security`
- Linux: Secret Service API via `secret-tool`
- Windows: Data Protection API (DPAPI) via `windows-sys`
- Plaintext opt-in fallback: set `NOTIFY_ALLOW_PLAINTEXT_STORE=1`

## Exit Codes

- `0`: Success
- `1`: Unclassified error (`error`)
- `2`: Network timeout or connection failure (`network`)
- `3`: Authentication or authorization required (`auth_required`)
- `4`: Profile or target not found (`not_found`)
- `5`: Rate limit exceeded (`rate_limited`)
- `6`: Invalid argument or input (`invalid_input`)
- `7`: Account/profile not configured (`no_account`)
"#;
