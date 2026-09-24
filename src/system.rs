//! Native OS desktop notifications and GUI message boxes.
//!
//! Windows: PowerShell Notification Balloon / MessageBox
//! macOS: osascript notification / dialog
//! Linux: notify-send / zenity

use std::process::Command;

use crate::error::{Error, Result};

/// Sends a native OS desktop notification (toast/banner).
pub fn send_notification(title: &str, description: &str) -> Result<()> {
    if std::env::var("NOTIFY_MOCK_SYSTEM").as_deref() == Ok("1") {
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        show_macos_notification(title, description)
    }

    #[cfg(target_os = "windows")]
    {
        show_windows_notification(title, description)
    }

    #[cfg(target_os = "linux")]
    {
        show_linux_notification(title, description)
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = (title, description);
        Err(Error::other("Desktop notifications are not supported on this platform."))
    }
}

/// Shows a native OS message box dialog (blocks until dismissed).
pub fn show_message_box(title: &str, message: &str) -> Result<()> {
    if std::env::var("NOTIFY_MOCK_SYSTEM").as_deref() == Ok("1") {
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        show_macos_messagebox(title, message)
    }

    #[cfg(target_os = "windows")]
    {
        show_windows_messagebox(title, message)
    }

    #[cfg(target_os = "linux")]
    {
        show_linux_messagebox(title, message)
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = (title, message);
        Err(Error::other("Message boxes are not supported on this platform."))
    }
}

#[cfg(any(target_os = "macos", test))]
fn show_macos_notification(title: &str, description: &str) -> Result<()> {
    let title_esc = escape_applescript(title);
    let desc_esc = escape_applescript(description);
    let script = format!("display notification \"{desc_esc}\" with title \"{title_esc}\"");
    run_cmd("osascript", &["-e", &script], "macOS osascript notification")
}

#[cfg(any(target_os = "macos", test))]
fn show_macos_messagebox(title: &str, message: &str) -> Result<()> {
    let title_esc = escape_applescript(title);
    let msg_esc = escape_applescript(message);
    let script =
        format!("display dialog \"{msg_esc}\" with title \"{title_esc}\" buttons {{\"OK\"}} default button \"OK\"");
    run_cmd("osascript", &["-e", &script], "macOS osascript dialog")
}

#[allow(dead_code)]
#[cfg(any(target_os = "linux", test))]
fn show_linux_notification(title: &str, description: &str) -> Result<()> {
    run_cmd("notify-send", &[title, description], "Linux notify-send")
}

#[allow(dead_code)]
#[cfg(any(target_os = "linux", test))]
fn show_linux_messagebox(title: &str, message: &str) -> Result<()> {
    let title_arg = format!("--title={title}");
    let text_arg = format!("--text={message}");
    run_cmd("zenity", &["--info", &title_arg, &text_arg], "Linux zenity")
}

#[allow(dead_code)]
#[cfg(any(target_os = "windows", test))]
fn show_windows_notification(title: &str, description: &str) -> Result<()> {
    let title_esc = title.replace('\'', "''");
    let desc_esc = description.replace('\'', "''");
    let script = format!(
        r#"
Add-Type -AssemblyName System.Windows.Forms
$n = New-Object System.Windows.Forms.NotifyIcon
$n.Icon = [System.Drawing.SystemIcons]::Information
$n.BalloonTipTitle = '{title_esc}'
$n.BalloonTipText = '{desc_esc}'
$n.Visible = $true
$n.ShowBalloonTip(5000)
Start-Sleep -Milliseconds 100
"#
    );
    let encoded = encode_utf16le_base64(&script);
    run_cmd("powershell", &["-NoProfile", "-EncodedCommand", &encoded], "Windows PowerShell notification")
}

#[allow(dead_code)]
#[cfg(any(target_os = "windows", test))]
fn show_windows_messagebox(title: &str, message: &str) -> Result<()> {
    let title_esc = title.replace('\'', "''");
    let msg_esc = message.replace('\'', "''");
    let script = format!(
        r#"
Add-Type -AssemblyName System.Windows.Forms
[System.Windows.Forms.MessageBox]::Show('{msg_esc}', '{title_esc}', 'OK', 'Information')
"#
    );
    let encoded = encode_utf16le_base64(&script);
    run_cmd("powershell", &["-NoProfile", "-EncodedCommand", &encoded], "Windows PowerShell MessageBox")
}

fn run_cmd(prog: &str, args: &[&str], desc: &str) -> Result<()> {
    let output = Command::new(prog).args(args).output().map_err(|e| {
        Error::other(format!("Failed to execute {desc}: {e}")).fix(format!("Ensure '{prog}' is installed and in PATH."))
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(Error::other(format!("{desc} exited with error.")).detail(if stderr.is_empty() {
            format!("Exit status: {}", output.status)
        } else {
            stderr
        }));
    }

    Ok(())
}

fn escape_applescript(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[allow(dead_code)]
#[cfg(any(target_os = "windows", test))]
fn encode_utf16le_base64(s: &str) -> String {
    let mut utf16_bytes = Vec::with_capacity(s.len() * 2);
    for u in s.encode_utf16() {
        utf16_bytes.push((u & 0xFF) as u8);
        utf16_bytes.push((u >> 8) as u8);
    }
    base64_encode(&utf16_bytes)
}

pub fn base64_encode(bytes: &[u8]) -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = if chunk.len() > 1 { chunk[1] } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] } else { 0 };
        out.push(CHARSET[(b0 >> 2) as usize] as char);
        out.push(CHARSET[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            out.push(CHARSET[(((b1 & 0x0F) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(CHARSET[(b2 & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

pub fn base64_url_encode(bytes: &[u8]) -> String {
    base64_encode(bytes).replace('+', "-").replace('/', "_").trim_end_matches('=').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64_encode() {
        assert_eq!(base64_encode(b"hello world"), "aGVsbG8gd29ybGQ=");
        assert_eq!(base64_url_encode(b"hello world"), "aGVsbG8gd29ybGQ");
    }

    #[test]
    fn test_escape_applescript() {
        assert_eq!(escape_applescript("Hello \"World\" \\ test"), "Hello \\\"World\\\" \\\\ test");
    }
}
