//! Multi-account and profile resolution.

use crate::config;
use crate::error::{Error, Result};
use crate::secrets;

#[derive(Clone, Debug)]
pub struct ResolvedSlackProfile {
    #[allow(dead_code)]
    pub name: String,
    pub webhook_url: String,
    pub channel: Option<String>,
    pub username: Option<String>,
    pub icon_emoji: Option<String>,
    pub source: &'static str,
}

#[derive(Clone, Debug)]
pub struct ResolvedEmailProfile {
    #[allow(dead_code)]
    pub name: String,
    pub provider: String,
    pub from: String,
    pub default_to: Option<String>,
    pub default_cc: Vec<String>,
    pub default_bcc: Vec<String>,
    pub endpoint: Option<String>,
    pub secret: Option<String>,
    pub source: &'static str,
}

pub fn resolve_slack(name: &str) -> Result<ResolvedSlackProfile> {
    let cfg = config::load()?;
    let (found_name, profile) = match cfg.find_slack(name) {
        Some((k, v)) => (k.clone(), v.clone()),
        None => {
            let available = if cfg.slack.is_empty() {
                "(none configured)".to_string()
            } else {
                cfg.slack.keys().cloned().collect::<Vec<_>>().join(", ")
            };
            return Err(Error::no_account(format!("Slack profile '{name}' not found."))
                .detail(format!("Available profiles: {available}"))
                .fix(format!("Add this profile: notify accounts add {name} --type slack --webhook-url <url>")));
        }
    };

    let mut source = "config";
    let webhook_url = if let Some(url) = profile.webhook_url.filter(|u| !u.trim().is_empty()) {
        url
    } else {
        let store = secrets::store()?;
        let secret = store
            .get(&secrets::account_key(&found_name))?
            .or_else(|| store.get(&format!("account:slack:{}", found_name.to_lowercase())).ok().flatten());
        match secret {
            Some(s) => {
                source = store.name();
                s
            }
            None => {
                return Err(Error::auth(format!(
                    "No webhook URL configured or stored for Slack profile '{found_name}'."
                ))
                .fix(format!(
                    "Store the webhook URL: notify accounts add {found_name} --type slack --webhook-url <url>"
                )));
            }
        }
    };

    Ok(ResolvedSlackProfile {
        name: found_name,
        webhook_url,
        channel: profile.channel,
        username: profile.username,
        icon_emoji: profile.icon_emoji,
        source,
    })
}

pub fn resolve_email(name: &str) -> Result<ResolvedEmailProfile> {
    let cfg = config::load()?;
    let (found_name, profile) = match cfg.find_email(name) {
        Some((k, v)) => (k.clone(), v.clone()),
        None => {
            let available = if cfg.email.is_empty() {
                "(none configured)".to_string()
            } else {
                cfg.email.keys().cloned().collect::<Vec<_>>().join(", ")
            };
            return Err(Error::no_account(format!("Email profile '{name}' not found."))
                .detail(format!("Available profiles: {available}"))
                .fix(format!("Add this profile: notify accounts add {name} --type email --from <email>")));
        }
    };

    let store = secrets::store().ok();
    let secret = if let Some(s) = store {
        s.get(&secrets::account_key(&found_name))
            .ok()
            .flatten()
            .or_else(|| s.get(&format!("account:email:{}", found_name.to_lowercase())).ok().flatten())
    } else {
        None
    };

    let from = profile.from.unwrap_or_default();
    let provider = profile.provider.unwrap_or_else(|| "gmail".to_string());

    let has_secret = secret.is_some();
    Ok(ResolvedEmailProfile {
        name: found_name,
        provider,
        from,
        default_to: profile.default_to,
        default_cc: profile.default_cc.unwrap_or_default(),
        default_bcc: profile.default_bcc.unwrap_or_default(),
        endpoint: profile.endpoint,
        secret,
        source: if has_secret { "keystore" } else { "config" },
    })
}
