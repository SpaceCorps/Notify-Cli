//! Blocking HTTP client for Slack webhooks, Email REST APIs, and connectivity testing.

use std::time::Duration;

use serde_json::Value;
use ureq::Agent;
use ureq::http::Response;

use crate::error::{Error, ErrorCode, Result};

const MAX_BODY: u64 = 64 * 1024 * 1024;

#[derive(Clone)]
pub struct HttpClient {
    agent: Agent,
}

impl Default for HttpClient {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpClient {
    pub fn new() -> HttpClient {
        let agent: Agent = Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(60)))
            .timeout_connect(Some(Duration::from_secs(15)))
            .http_status_as_error(false)
            .user_agent(concat!("notify-cli/", env!("CARGO_PKG_VERSION")))
            .build()
            .into();

        HttpClient { agent }
    }

    /// Post payload to Slack Webhook URL
    pub fn post_slack_webhook(&self, url: &str, payload: &Value) -> Result<()> {
        let json_bytes = serde_json::to_vec(payload).map_err(|e| Error::invalid(e.to_string()))?;

        let res = self
            .agent
            .post(url)
            .header("Content-Type", "application/json")
            .send(&json_bytes[..])
            .map_err(transport_error)?;

        let status = res.status().as_u16();
        let body = read_body(res)?;

        if status == 200 {
            // Slack returns "ok" text or json {"ok": true}
            return Ok(());
        }

        Err(status_error(status, &body, "Slack Webhook API"))
    }

    /// Send email via Gmail REST API
    pub fn send_gmail_api(
        &self,
        token: &str,
        endpoint_override: Option<&str>,
        raw_base64url: &str,
        is_draft: bool,
    ) -> Result<Value> {
        let default_endpoint = if is_draft {
            "https://gmail.googleapis.com/gmail/v1/users/me/drafts"
        } else {
            "https://gmail.googleapis.com/gmail/v1/users/me/messages/send"
        };
        let url = endpoint_override.unwrap_or(default_endpoint);

        let payload = if is_draft {
            serde_json::json!({
                "message": {
                    "raw": raw_base64url
                }
            })
        } else {
            serde_json::json!({
                "raw": raw_base64url
            })
        };

        let json_bytes = serde_json::to_vec(&payload).map_err(|e| Error::invalid(e.to_string()))?;

        let mut req = self.agent.post(url).header("Content-Type", "application/json");
        if !token.is_empty() {
            req = req.header("Authorization", &format!("Bearer {token}"));
        }

        let res = req.send(&json_bytes[..]).map_err(transport_error)?;

        let status = res.status().as_u16();
        let body = read_body(res)?;

        if (200..300).contains(&status) {
            if body.trim().is_empty() {
                return Ok(Value::Object(Default::default()));
            }
            return serde_json::from_str(&body).or_else(|_| Ok(Value::String(body)));
        }

        Err(status_error(status, &body, "Gmail API"))
    }

    /// Generic POST JSON to endpoint
    #[allow(dead_code)]
    pub fn post_json(&self, url: &str, auth_bearer: Option<&str>, body: &Value) -> Result<Value> {
        let json_bytes = serde_json::to_vec(body).map_err(|e| Error::invalid(e.to_string()))?;

        let mut req = self.agent.post(url).header("Content-Type", "application/json");
        if let Some(token) = auth_bearer {
            req = req.header("Authorization", &format!("Bearer {token}"));
        }

        let res = req.send(&json_bytes[..]).map_err(transport_error)?;
        let status = res.status().as_u16();
        let body_str = read_body(res)?;

        if (200..300).contains(&status) {
            if body_str.trim().is_empty() {
                return Ok(Value::Object(Default::default()));
            }
            return serde_json::from_str(&body_str).or_else(|_| Ok(Value::String(body_str)));
        }

        Err(status_error(status, &body_str, "HTTP API"))
    }
}

fn read_body(mut response: Response<ureq::Body>) -> Result<String> {
    let bytes = response.body_mut().with_config().limit(MAX_BODY).read_to_vec().map_err(transport_error)?;
    Ok(String::from_utf8_lossy(&bytes).trim().to_string())
}

fn transport_error(e: ureq::Error) -> Error {
    match e {
        ureq::Error::Timeout(_) => {
            Error::new(ErrorCode::Network, "The HTTP request timed out.").fix("Retry once, then stop.")
        }
        other => Error::new(ErrorCode::Network, "Could not reach the remote API.")
            .detail(other.to_string())
            .fix("Check internet connection and endpoint URL, then retry."),
    }
}

fn status_error(status: u16, body: &str, service: &str) -> Error {
    let mut detail = format!("HTTP {status}");
    if !body.is_empty() {
        detail.push_str(": ");
        detail.push_str(body);
    }

    match status {
        400 => Error::new(ErrorCode::InvalidInput, format!("{service} rejected request as invalid."))
            .detail(detail)
            .fix("Verify payload structure and parameter values."),
        401 | 403 => Error::new(ErrorCode::AuthRequired, format!("{service} authentication failed."))
            .detail(detail)
            .fix("Check credentials, webhook URL, or OAuth token permissions."),
        404 => Error::new(ErrorCode::NotFound, format!("{service} endpoint not found."))
            .detail(detail)
            .fix("Verify the URL or profile configuration."),
        429 => Error::new(ErrorCode::RateLimited, format!("{service} rate limit exceeded."))
            .detail(detail)
            .fix("Wait and back off before retrying."),
        500..=599 => Error::new(ErrorCode::Network, format!("{service} returned a server error."))
            .detail(detail)
            .fix("Upstream server is experiencing issues. Retry once, then stop."),
        _ => Error::new(ErrorCode::Error, format!("{service} request failed."))
            .detail(detail)
            .fix("Inspect the error response details above."),
    }
}
