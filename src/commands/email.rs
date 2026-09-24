//! Handler for `notify email`.

use std::fs;
use std::io::{IsTerminal, Read};
use std::path::Path;

use crate::account;
use crate::cli::EmailArgs;
use crate::client::HttpClient;
use crate::config;
use crate::error::{Error, Result};
use crate::system::base64_url_encode;
use crate::{obj, output};

pub fn run(args: EmailArgs) -> Result<()> {
    let profile_name = args.profile.or(args.account);

    let resolved_profile = if let Some(ref name) = profile_name {
        Some(account::resolve_email(name)?)
    } else {
        let cfg = config::load()?;
        if cfg.email.len() == 1 {
            let (name, _) = cfg.email.iter().next().unwrap();
            Some(account::resolve_email(name)?)
        } else {
            None
        }
    };

    let provider = args
        .provider
        .or_else(|| resolved_profile.as_ref().map(|p| p.provider.clone()))
        .unwrap_or_else(|| "gmail".to_string());

    let from = resolved_profile.as_ref().map(|p| p.from.clone()).unwrap_or_default();

    // Body content: priority stdin > --body > --file
    let body_content = if !std::io::stdin().is_terminal() {
        let mut stdin_content = String::new();
        std::io::stdin()
            .read_to_string(&mut stdin_content)
            .map_err(|e| Error::invalid(format!("Could not read from stdin: {e}")))?;
        stdin_content
    } else if let Some(b) = args.body {
        b
    } else if let Some(ref file_path) = args.file {
        fs::read_to_string(file_path)
            .map_err(|e| Error::invalid(format!("Could not read body file '{}': {e}", file_path)))?
    } else {
        return Err(Error::invalid("No email body provided.")
            .fix("Provide email content via stdin, --body \"...\", or --file <path>."));
    };

    let subject = match args.subject.filter(|s| !s.trim().is_empty()) {
        Some(s) => s,
        None => return Err(Error::invalid("Email subject is required (--subject).")),
    };

    // Recipients
    let mut to_list = expand_recipients(&args.to);
    if to_list.is_empty()
        && let Some(ref def_to) = resolved_profile.as_ref().and_then(|p| p.default_to.clone())
    {
        to_list.push(def_to.clone());
    }
    if to_list.is_empty() {
        return Err(Error::invalid("At least one recipient (--to) is required."));
    }

    let mut cc_list = expand_recipients(&args.cc);
    if let Some(ref p) = resolved_profile {
        for cc in &p.default_cc {
            if !cc_list.contains(cc) {
                cc_list.push(cc.clone());
            }
        }
    }

    let mut bcc_list = expand_recipients(&args.bcc);
    if let Some(ref p) = resolved_profile {
        for bcc in &p.default_bcc {
            if !bcc_list.contains(bcc) {
                bcc_list.push(bcc.clone());
            }
        }
    }

    // Attachments
    let mut attachments = Vec::new();
    for attach_path in &args.attach {
        let path = Path::new(attach_path);
        if !path.is_file() {
            return Err(Error::invalid(format!("Attachment file not found: {attach_path}")));
        }
        let metadata = fs::metadata(path).map_err(|e| Error::invalid(e.to_string()))?;
        if metadata.len() > 25 * 1024 * 1024 {
            return Err(Error::invalid(format!(
                "Attachment '{}' is too large ({} bytes). Maximum allowed is 25MB.",
                attach_path,
                metadata.len()
            )));
        }
        let bytes = fs::read(path).map_err(|e| Error::invalid(e.to_string()))?;
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("attachment").to_string();
        let content_type = mime_type_for(attach_path);
        attachments.push(Attachment { name: file_name, content_type, bytes });
    }

    let is_html = is_html_content(&body_content);
    let plain_body = if is_html { convert_html_to_plain(&body_content) } else { body_content.clone() };
    let html_body = if is_html { Some(body_content) } else { None };

    let mime_message = build_mime_message(MimeMessage {
        from: &from,
        to: &to_list,
        cc: &cc_list,
        bcc: &bcc_list,
        subject: &subject,
        plain_body: &plain_body,
        html_body: html_body.as_deref(),
        attachments: &attachments,
    });

    let raw_base64url = base64_url_encode(mime_message.as_bytes());

    // Token resolution
    let token = if let Some(t) = args.token.filter(|t| !t.trim().is_empty()) {
        t
    } else if let Ok(t) = std::env::var("GMAIL_TOKEN").or_else(|_| std::env::var("NOTIFY_EMAIL_TOKEN")) {
        t
    } else if let Some(t) = resolved_profile.as_ref().and_then(|p| p.secret.clone()) {
        t
    } else {
        read_credential_token(profile_name.as_deref().unwrap_or("default"))?
    };

    let endpoint_override = args
        .endpoint
        .or_else(|| resolved_profile.as_ref().and_then(|p| p.endpoint.clone()))
        .or_else(|| std::env::var("NOTIFY_EMAIL_API_URL").ok());

    let client = HttpClient::new();
    let action_str = if args.draft { "draft_created" } else { "sent" };

    client.send_gmail_api(&token, endpoint_override.as_deref(), &raw_base64url, args.draft)?;

    output::write(&obj! {
        "status" => "ok",
        "provider" => provider,
        "profile" => profile_name.unwrap_or_else(|| "direct".to_string()),
        "action" => action_str,
        "from" => from,
        "to" => to_list,
        "cc" => cc_list,
        "bcc" => bcc_list,
        "subject" => subject,
        "attachments" => attachments.len(),
    });

    Ok(())
}

struct Attachment {
    name: String,
    content_type: &'static str,
    bytes: Vec<u8>,
}

fn expand_recipients(input: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for entry in input {
        for part in entry.split(',') {
            let trimmed = part.trim();
            if !trimmed.is_empty() && !out.contains(&trimmed.to_string()) {
                out.push(trimmed.to_string());
            }
        }
    }
    out
}

fn is_html_content(content: &str) -> bool {
    let lower = content.to_lowercase();
    let tags = [
        "<html", "<body", "<div", "<p>", "<p ", "<span", "<h1", "<h2", "<h3", "<table", "<ul", "<ol", "<li", "<br>",
        "<br/>", "<br />", "<img", "<a href",
    ];
    tags.iter().any(|t| lower.contains(t))
}

fn convert_html_to_plain(html: &str) -> String {
    let mut s = html.to_string();

    // Remove scripts and styles
    while let Some(start) = s.to_lowercase().find("<script") {
        if let Some(end) = s.to_lowercase()[start..].find("</script>") {
            s.replace_range(start..start + end + 9, "");
        } else {
            break;
        }
    }
    while let Some(start) = s.to_lowercase().find("<style") {
        if let Some(end) = s.to_lowercase()[start..].find("</style>") {
            s.replace_range(start..start + end + 8, "");
        } else {
            break;
        }
    }

    s = s
        .replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("<br />", "\n")
        .replace("</p>", "\n\n")
        .replace("</div>", "\n")
        .replace("</li>", "\n")
        .replace("<li>", "• ");

    // Remove remaining HTML tags
    let mut clean = String::with_capacity(s.len());
    let mut in_tag = false;
    for ch in s.chars() {
        if ch == '<' {
            in_tag = true;
        } else if ch == '>' {
            in_tag = false;
        } else if !in_tag {
            clean.push(ch);
        }
    }

    // Entity replacements
    clean
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .trim()
        .to_string()
}

fn mime_type_for(path: &str) -> &'static str {
    let ext = Path::new(path).extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()).unwrap_or_default();

    match ext.as_str() {
        "pdf" => "application/pdf",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xls" => "application/vnd.ms-excel",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "ppt" => "application/vnd.ms-powerpoint",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "txt" => "text/plain",
        "csv" => "text/csv",
        "html" | "htm" => "text/html",
        "json" => "application/json",
        "xml" => "application/xml",
        "zip" => "application/zip",
        "tar" => "application/x-tar",
        "gz" => "application/gzip",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "svg" => "image/svg+xml",
        "mp3" => "audio/mpeg",
        "mp4" => "video/mp4",
        _ => "application/octet-stream",
    }
}

struct MimeMessage<'a> {
    from: &'a str,
    to: &'a [String],
    cc: &'a [String],
    bcc: &'a [String],
    subject: &'a str,
    plain_body: &'a str,
    html_body: Option<&'a str>,
    attachments: &'a [Attachment],
}

fn build_mime_message(msg_data: MimeMessage) -> String {
    let mut msg = String::new();

    if !msg_data.from.is_empty() {
        msg.push_str(&format!("From: {}\r\n", msg_data.from));
    }
    msg.push_str(&format!("To: {}\r\n", msg_data.to.join(", ")));
    if !msg_data.cc.is_empty() {
        msg.push_str(&format!("Cc: {}\r\n", msg_data.cc.join(", ")));
    }
    if !msg_data.bcc.is_empty() {
        msg.push_str(&format!("Bcc: {}\r\n", msg_data.bcc.join(", ")));
    }
    msg.push_str(&format!("Subject: {}\r\n", msg_data.subject));
    msg.push_str(&format!("Date: {}\r\n", config::now_utc()));
    msg.push_str("MIME-Version: 1.0\r\n");

    let boundary_mixed = format!("----=_Part_Mixed_{}", std::process::id());
    let boundary_alt = format!("----=_Part_Alt_{}", std::process::id());

    let has_attachments = !msg_data.attachments.is_empty();
    let has_html = msg_data.html_body.is_some();

    if has_attachments {
        msg.push_str(&format!("Content-Type: multipart/mixed; boundary=\"{boundary_mixed}\"\r\n\r\n"));
        msg.push_str(&format!("--{boundary_mixed}\r\n"));

        if has_html {
            msg.push_str(&format!("Content-Type: multipart/alternative; boundary=\"{boundary_alt}\"\r\n\r\n"));
            append_text_part(&mut msg, msg_data.plain_body, Some(&boundary_alt));
            append_html_part(&mut msg, msg_data.html_body.unwrap(), Some(&boundary_alt));
            msg.push_str(&format!("--{boundary_alt}--\r\n"));
        } else {
            append_text_part(&mut msg, msg_data.plain_body, None);
        }

        for att in msg_data.attachments {
            msg.push_str(&format!("--{boundary_mixed}\r\n"));
            msg.push_str(&format!("Content-Type: {}; name=\"{}\"\r\n", att.content_type, att.name));
            msg.push_str(&format!("Content-Disposition: attachment; filename=\"{}\"\r\n", att.name));
            msg.push_str("Content-Transfer-Encoding: base64\r\n\r\n");

            let b64 = crate::system::base64_encode(&att.bytes);
            for chunk in b64.as_bytes().chunks(76) {
                msg.push_str(std::str::from_utf8(chunk).unwrap_or(""));
                msg.push_str("\r\n");
            }
        }

        msg.push_str(&format!("--{boundary_mixed}--\r\n"));
    } else if has_html {
        msg.push_str(&format!("Content-Type: multipart/alternative; boundary=\"{boundary_alt}\"\r\n\r\n"));
        append_text_part(&mut msg, msg_data.plain_body, Some(&boundary_alt));
        append_html_part(&mut msg, msg_data.html_body.unwrap(), Some(&boundary_alt));
        msg.push_str(&format!("--{boundary_alt}--\r\n"));
    } else {
        msg.push_str("Content-Type: text/plain; charset=UTF-8\r\n\r\n");
        msg.push_str(msg_data.plain_body);
        msg.push_str("\r\n");
    }

    msg
}

fn append_text_part(msg: &mut String, plain_text: &str, boundary: Option<&str>) {
    if let Some(b) = boundary {
        msg.push_str(&format!("--{b}\r\n"));
    }
    msg.push_str("Content-Type: text/plain; charset=UTF-8\r\n\r\n");
    msg.push_str(plain_text);
    msg.push_str("\r\n");
}

fn append_html_part(msg: &mut String, html: &str, boundary: Option<&str>) {
    if let Some(b) = boundary {
        msg.push_str(&format!("--{b}\r\n"));
    }
    msg.push_str("Content-Type: text/html; charset=UTF-8\r\n\r\n");
    msg.push_str(html);
    msg.push_str("\r\n");
}

fn read_credential_token(profile: &str) -> Result<String> {
    // Check credentials file at config_dir/gmail-credentials/{profile}/user_credential.json
    let cred_dir = config::config_dir().join("gmail-credentials").join(profile);
    let user_cred_path = cred_dir.join("user_credential.json");
    if user_cred_path.is_file()
        && let Ok(content) = fs::read_to_string(&user_cred_path)
        && let Ok(val) = serde_json::from_str::<serde_json::Value>(&content)
        && let Some(tok) = val.get("access_token").and_then(|t| t.as_str())
    {
        return Ok(tok.to_string());
    }

    // Check if token was stored under account:{profile} in keystore
    let store = crate::secrets::store()?;
    if let Some(tok) = store.get(&crate::secrets::account_key(profile))? {
        return Ok(tok);
    }

    // If running in tests or mock endpoint, allow empty or dummy token
    if std::env::var("NOTIFY_EMAIL_API_URL").is_ok() {
        return Ok("mock_test_token".to_string());
    }

    Err(Error::auth(format!("No Gmail access token or credentials found for profile '{profile}'."))
        .detail(format!("Expected user credential at '{}' or secret in keystore.", user_cred_path.display()))
        .fix("Configure Gmail OAuth credentials or pass --token <token>."))
}
