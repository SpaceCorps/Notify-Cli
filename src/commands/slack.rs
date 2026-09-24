//! Handler for `notify slack`.

use std::fs;
use std::io::{IsTerminal, Read};

use serde_json::Value;

use crate::account;
use crate::cli::SlackArgs;
use crate::client::HttpClient;
use crate::error::{Error, Result};
use crate::{obj, output};

pub fn run(args: SlackArgs) -> Result<()> {
    let profile_name = args.profile.or(args.account);

    let (webhook_url, default_channel, default_username, default_icon_emoji) = if let Some(direct_url) =
        args.webhook_url.filter(|u| !u.trim().is_empty())
    {
        (direct_url, None, None, None)
    } else if let Some(ref name) = profile_name {
        let resolved = account::resolve_slack(name)?;
        (resolved.webhook_url, resolved.channel, resolved.username, resolved.icon_emoji)
    } else {
        // Check if there is only 1 Slack profile in config
        let cfg = crate::config::load()?;
        if cfg.slack.len() == 1 {
            let (name, _) = cfg.slack.iter().next().unwrap();
            let resolved = account::resolve_slack(name)?;
            (resolved.webhook_url, resolved.channel, resolved.username, resolved.icon_emoji)
        } else {
            let available = if cfg.slack.is_empty() {
                "(none configured)".to_string()
            } else {
                cfg.slack.keys().cloned().collect::<Vec<_>>().join(", ")
            };
            return Err(Error::invalid("Please specify a Slack profile or --webhook-url.")
                    .detail(format!("Available profiles: {available}"))
                    .fix("Run: notify slack <profile> --message \"...\" or notify slack --webhook-url <url> --message \"...\""));
        }
    };

    let channel = args.channel.or(default_channel);
    let username = args.username.or(default_username);
    let icon_emoji = args.icon_emoji.or(default_icon_emoji);

    // Read payload
    let mut payload = if let Some(msg) = args.message {
        if args.json_payload.is_some() || args.json_file.is_some() {
            return Err(Error::invalid("Only one of --message, --json-payload, or --json-file can be used."));
        }
        serde_json::json!({ "text": msg })
    } else if let Some(raw_json) = args.json_payload {
        if args.json_file.is_some() {
            return Err(Error::invalid("Only one of --message, --json-payload, or --json-file can be used."));
        }
        serde_json::from_str::<Value>(&raw_json).map_err(|e| Error::invalid(format!("Invalid JSON payload: {e}")))?
    } else if let Some(path) = args.json_file {
        let content = fs::read_to_string(&path)
            .map_err(|e| Error::invalid(format!("Could not read JSON file '{}': {e}", path)))?;
        serde_json::from_str::<Value>(&content)
            .map_err(|e| Error::invalid(format!("Invalid JSON in file '{}': {e}", path)))?
    } else if !std::io::stdin().is_terminal() {
        let mut stdin_content = String::new();
        std::io::stdin()
            .read_to_string(&mut stdin_content)
            .map_err(|e| Error::invalid(format!("Could not read from stdin: {e}")))?;
        let trimmed = stdin_content.trim();
        if trimmed.starts_with('{') && trimmed.ends_with('}') {
            serde_json::from_str::<Value>(trimmed).unwrap_or_else(|_| serde_json::json!({ "text": stdin_content }))
        } else {
            serde_json::json!({ "text": stdin_content })
        }
    } else {
        return Err(Error::invalid("No message content provided.")
            .fix("Specify --message <text>, --json-payload <json>, --json-file <path>, or pipe via stdin."));
    };

    if let Value::Object(ref mut map) = payload {
        if let Some(c) = channel.as_deref() {
            map.entry("channel".to_string()).or_insert_with(|| Value::String(c.to_string()));
        }
        if let Some(u) = username.as_deref() {
            map.entry("username".to_string()).or_insert_with(|| Value::String(u.to_string()));
        }
        if let Some(i) = icon_emoji.as_deref() {
            map.entry("icon_emoji".to_string()).or_insert_with(|| Value::String(i.to_string()));
        }
    } else {
        return Err(Error::invalid("JSON payload must be an object."));
    }

    let client = HttpClient::new();
    client.post_slack_webhook(&webhook_url, &payload)?;

    let masked_url = mask_url(&webhook_url);
    output::write(&obj! {
        "status" => "ok",
        "provider" => "slack",
        "profile" => profile_name.unwrap_or_else(|| "direct".to_string()),
        "webhookUrl" => masked_url,
        "channel" => channel.unwrap_or_default(),
        "delivered" => true,
    });

    Ok(())
}

fn mask_url(url: &str) -> String {
    if let Some((prefix, secret)) = url.rsplit_once('/') {
        if secret.len() > 6 { format!("{prefix}/...{}", &secret[secret.len() - 4..]) } else { format!("{prefix}/***") }
    } else {
        "***".to_string()
    }
}
