//! Configuration management for Notify-Cli.
//!
//! Stores Slack profiles, Email profiles, and metadata in `config.yaml`.
//! Sensitive secrets (like Slack webhook URLs and OAuth/API tokens) are stored
//! securely in the native OS keystore via [`crate::secrets`], with fallback to
//! legacy plaintext `config.yaml` values if already configured.

use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case", default)]
pub struct SlackProfile {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webhook_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_emoji: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case", default)]
pub struct EmailProfile {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_cc: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_bcc: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", default)]
pub struct Config {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<u32>,
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub slack: IndexMap<String, SlackProfile>,
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub email: IndexMap<String, EmailProfile>,
}

impl Default for Config {
    fn default() -> Self {
        Config { version: Some(1), slack: IndexMap::new(), email: IndexMap::new() }
    }
}

impl Config {
    /// Case-insensitive search for Slack profile
    pub fn find_slack(&self, name: &str) -> Option<(&String, &SlackProfile)> {
        self.slack.iter().find(|(k, _)| k.eq_ignore_ascii_case(name))
    }

    /// Case-insensitive search for Email profile
    pub fn find_email(&self, name: &str) -> Option<(&String, &EmailProfile)> {
        self.email.iter().find(|(k, _)| k.eq_ignore_ascii_case(name))
    }
}

pub fn config_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("NOTIFY_CONFIG_DIR").filter(|v| !v.is_empty()) {
        return PathBuf::from(dir);
    }

    // Check local directory if config.yaml exists
    if Path::new("config.yaml").is_file() {
        return PathBuf::from(".");
    }

    #[cfg(windows)]
    let dir = {
        let appdata = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_default();
        let legacy = appdata.join("Notify.Console");
        if legacy.join("config.yaml").is_file() { legacy } else { appdata.join("notify-cli") }
    };

    #[cfg(not(windows))]
    let dir = {
        let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
        let legacy = home.join(".config").join("notify-console");
        if legacy.join("config.yaml").is_file() {
            legacy
        } else if cfg!(target_os = "macos") {
            home.join("Library").join("Application Support").join("notify-cli")
        } else {
            std::env::var_os("XDG_CONFIG_HOME")
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".config"))
                .join("notify-cli")
        }
    };

    dir
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.yaml")
}

pub fn ensure_dir() -> Result<()> {
    let dir = config_dir();
    if dir == Path::new(".") || dir.is_dir() {
        return Ok(());
    }
    fs::create_dir_all(&dir)?;
    restrict_to_owner(&dir);
    Ok(())
}

pub fn load() -> Result<Config> {
    let path = config_path();
    let text = match fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Config::default()),
        Err(e) => return Err(e.into()),
    };
    if text.trim().is_empty() {
        return Ok(Config::default());
    }
    serde_norway::from_str::<Option<Config>>(&text).map(Option::unwrap_or_default).map_err(|e| {
        Error::other(format!("{} is not valid YAML.", path.display()))
            .detail(e.to_string())
            .fix(format!("Check syntax or reset: {}", path.display()))
    })
}

pub fn save(config: &Config) -> Result<()> {
    ensure_dir()?;
    let yaml = serde_norway::to_string(config).map_err(|e| Error::other(e.to_string()))?;
    atomic_write(&config_path(), yaml.as_bytes())
}

/// Write to a temp file in the same directory, then rename over - never a partial file.
pub fn atomic_write(path: &Path, contents: &[u8]) -> Result<()> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    fs::write(&tmp, contents)?;
    restrict_to_owner(&tmp);
    fs::rename(&tmp, path)?;
    Ok(())
}

/// 0600 (0700 for directories) on Unix. On Windows the DPAPI blob is already user-scoped.
pub fn restrict_to_owner(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = if path.is_dir() { 0o700 } else { 0o600 };
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(mode));
    }
    #[cfg(not(unix))]
    let _ = path;
}

pub struct Lock(#[allow(dead_code)] File);

pub fn lock() -> Result<Lock> {
    ensure_dir()?;
    let path = config_dir().join(".lock");
    let file = OpenOptions::new().create(true).truncate(false).read(true).write(true).open(&path)?;

    let deadline = Instant::now() + Duration::from_secs(15);
    let mut delay = Duration::from_millis(25);
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(Lock(file)),
            Err(fs::TryLockError::WouldBlock) if Instant::now() < deadline => {
                std::thread::sleep(delay);
                delay = (delay * 2).min(Duration::from_millis(400));
            }
            Err(fs::TryLockError::WouldBlock) => {
                return Err(Error::other(format!(
                    "Timed out waiting for the lock at {}. Another process may be stuck.",
                    path.display()
                )));
            }
            Err(fs::TryLockError::Error(e)) => return Err(e.into()),
        }
    }
}

pub fn now_utc() -> String {
    let secs =
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    format_utc(secs)
}

fn format_utc(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_utc() {
        assert_eq!(format_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(format_utc(1_790_000_000), "2026-09-21T14:13:20Z");
    }

    #[test]
    fn reads_the_dotnet_layout() {
        let yaml = r##"
slack:
  my-profile:
    webhook_url: https://hooks.slack.com/services/T00/B00/X00
    channel: "#general"
email:
  work:
    provider: gmail
    from: me@company.com
    default_to: team@company.com
"##;
        let c: Config = serde_norway::from_str(yaml).unwrap();
        let (name, s) = c.find_slack("my-profile").unwrap();
        assert_eq!(name, "my-profile");
        assert_eq!(s.webhook_url.as_deref(), Some("https://hooks.slack.com/services/T00/B00/X00"));
        assert_eq!(s.channel.as_deref(), Some("#general"));

        let (ename, e) = c.find_email("work").unwrap();
        assert_eq!(ename, "work");
        assert_eq!(e.provider.as_deref(), Some("gmail"));
        assert_eq!(e.from.as_deref(), Some("me@company.com"));
        assert_eq!(e.default_to.as_deref(), Some("team@company.com"));
    }
}
