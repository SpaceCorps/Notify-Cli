//! Handler for `notify accounts` (list, add, test, remove).

use std::io::{BufRead, IsTerminal, Write};

use serde_json::Value;

use crate::cli::AccountsCommand;
use crate::config::{self, EmailProfile, SlackProfile};
use crate::error::{Error, Result};
use crate::secrets;
use crate::{obj, output};

pub fn run(cmd: AccountsCommand) -> Result<()> {
    match cmd {
        AccountsCommand::List { check } => list_accounts(check),
        AccountsCommand::Add(add_args) => add_account(*add_args),
        AccountsCommand::Test { name } => test_account(&name),
        AccountsCommand::Remove { name, yes } => remove_account(&name, yes),
    }
}

fn list_accounts(check: bool) -> Result<()> {
    let cfg = config::load()?;
    let store = secrets::store().ok();

    let mut accounts = Vec::new();

    for (name, slack) in &cfg.slack {
        let has_secret = slack.webhook_url.as_ref().map(|u| !u.trim().is_empty()).unwrap_or(false)
            || store.and_then(|s| s.get(&secrets::account_key(name)).ok().flatten()).is_some();

        let status = if check {
            if has_secret { "configured" } else { "missing_secret" }
        } else if has_secret {
            "ready"
        } else {
            "unconfigured"
        };

        accounts.push(obj! {
            "name" => name,
            "type" => "slack",
            "channel" => slack.channel.as_deref().unwrap_or(""),
            "status" => status,
            "hasSecret" => has_secret,
        });
    }

    for (name, email) in &cfg.email {
        let has_secret = store.and_then(|s| s.get(&secrets::account_key(name)).ok().flatten()).is_some();

        let status = if check {
            if has_secret || email.provider.as_deref() == Some("gmail") { "configured" } else { "missing_credentials" }
        } else {
            "ready"
        };

        accounts.push(obj! {
            "name" => name,
            "type" => "email",
            "provider" => email.provider.as_deref().unwrap_or("gmail"),
            "from" => email.from.as_deref().unwrap_or(""),
            "status" => status,
            "hasSecret" => has_secret,
        });
    }

    output::write(&Value::Array(accounts));
    Ok(())
}

fn add_account(args: crate::cli::AddAccountArgs) -> Result<()> {
    let name = args.name.trim();
    if name.is_empty() {
        return Err(Error::invalid("Account name cannot be empty."));
    }

    let is_slack = args.account_type.eq_ignore_ascii_case("slack");
    let is_email = args.account_type.eq_ignore_ascii_case("email");

    if !is_slack && !is_email {
        return Err(Error::invalid(format!(
            "Invalid account type '{}'. Must be 'slack' or 'email'.",
            args.account_type
        )));
    }

    let mut cfg = config::load()?;
    let already_exists = if is_slack { cfg.find_slack(name).is_some() } else { cfg.find_email(name).is_some() };

    if already_exists && !args.force {
        return Err(Error::invalid(format!("Account '{name}' already exists."))
            .fix(format!("Use --force to overwrite: notify accounts add {name} --force")));
    }

    let secret = if args.api_key_stdin { Some(read_stdin_secret()?) } else { args.webhook_url.or(args.api_key) };

    let store = secrets::store()?;

    // Save secret to keystore if provided
    if let Some(sec) = secret.as_deref().filter(|s| !s.trim().is_empty()) {
        store.set(&secrets::account_key(name), sec)?;
    }

    {
        let _lock = config::lock()?;
        if is_slack {
            let profile = SlackProfile {
                webhook_url: None, // Secret stored in keystore
                channel: args.channel,
                username: args.username,
                icon_emoji: args.icon_emoji,
            };
            cfg.slack.insert(name.to_string(), profile);
        } else {
            let profile = EmailProfile {
                provider: Some(args.provider.unwrap_or_else(|| "gmail".to_string())),
                from: args.from,
                default_to: args.default_to,
                default_cc: None,
                default_bcc: None,
                endpoint: args.endpoint,
            };
            cfg.email.insert(name.to_string(), profile);
        }
        config::save(&cfg)?;
    }

    output::write(&obj! {
        "status" => if already_exists { "updated" } else { "created" },
        "name" => name,
        "type" => if is_slack { "slack" } else { "email" },
        "secretStore" => store.name(),
    });

    Ok(())
}

fn test_account(name: &str) -> Result<()> {
    let cfg = config::load()?;
    if let Some((slack_name, _)) = cfg.find_slack(name) {
        let resolved = crate::account::resolve_slack(slack_name)?;
        output::write(&obj! {
            "status" => "ok",
            "account" => slack_name,
            "type" => "slack",
            "secretSource" => resolved.source,
            "channel" => resolved.channel.unwrap_or_default(),
            "valid" => true,
        });
        return Ok(());
    }

    if let Some((email_name, _)) = cfg.find_email(name) {
        let resolved = crate::account::resolve_email(email_name)?;
        output::write(&obj! {
            "status" => "ok",
            "account" => email_name,
            "type" => "email",
            "provider" => resolved.provider,
            "from" => resolved.from,
            "secretSource" => resolved.source,
            "valid" => true,
        });
        return Ok(());
    }

    Err(Error::not_found(format!("Account '{name}' not found.")).fix("List accounts with: notify accounts list"))
}

fn remove_account(name: &str, yes: bool) -> Result<()> {
    let mut cfg = config::load()?;
    let is_slack = cfg.find_slack(name).is_some();
    let is_email = cfg.find_email(name).is_some();

    if !is_slack && !is_email {
        return Err(Error::not_found(format!("Account '{name}' not found.")));
    }

    if !yes && std::io::stdout().is_terminal() {
        print!("Are you sure you want to remove account '{name}'? [y/N]: ");
        let _ = std::io::stdout().flush();
        let mut line = String::new();
        std::io::stdin().lock().read_line(&mut line).map_err(|e| Error::other(e.to_string()))?;
        if !line.trim().eq_ignore_ascii_case("y") && !line.trim().eq_ignore_ascii_case("yes") {
            return Err(Error::other("Aborted by user."));
        }
    }

    if let Ok(store) = secrets::store() {
        let _ = store.delete(&secrets::account_key(name));
    }

    {
        let _lock = config::lock()?;
        cfg.slack.shift_remove(name);
        cfg.email.shift_remove(name);
        config::save(&cfg)?;
    }

    output::write(&obj! {
        "status" => "removed",
        "name" => name,
    });

    Ok(())
}

fn read_stdin_secret() -> Result<String> {
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line).map_err(|e| Error::invalid(format!("Failed to read stdin: {e}")))?;
    let secret = line.trim().to_string();
    if secret.is_empty() {
        return Err(Error::invalid("No secret provided on stdin."));
    }
    Ok(secret)
}
