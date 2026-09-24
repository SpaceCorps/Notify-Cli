//! Integration tests against an in-process mock HTTP server.
//!
//! Tests verify Slack webhooks, Email Gmail REST API endpoints, System notifications,
//! GUI message boxes, accounts management, and agent self-documentation.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

#[derive(Clone, Debug)]
#[allow(dead_code)]
struct Recorded {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: Option<Value>,
    raw_body: Vec<u8>,
}

type Route = (&'static str, &'static str, u16, Value);

struct Mock {
    url: String,
    log: Arc<Mutex<Vec<Recorded>>>,
}

impl Mock {
    fn start(routes: Vec<Route>) -> Mock {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let url = format!("http://127.0.0.1:{port}/");
        let log = Arc::new(Mutex::new(Vec::new()));
        let log2 = log.clone();

        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let routes = routes.clone();
                let log = log2.clone();
                std::thread::spawn(move || {
                    let mut reader = BufReader::new(stream.try_clone().unwrap());
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 {
                        return;
                    }
                    let mut parts = line.split_whitespace();
                    let method = parts.next().unwrap_or("").to_string();
                    let raw_path = parts.next().unwrap_or("").to_string();
                    let path = raw_path.trim_start_matches('/').to_string();

                    let mut headers = Vec::new();
                    let mut len = 0usize;
                    loop {
                        let mut h = String::new();
                        if reader.read_line(&mut h).unwrap_or(0) == 0 {
                            break;
                        }
                        let h = h.trim_end();
                        if h.is_empty() {
                            break;
                        }
                        if let Some((k, v)) = h.split_once(':') {
                            let (k, v) = (k.trim().to_lowercase(), v.trim().to_string());
                            if k == "content-length" {
                                len = v.parse().unwrap_or(0);
                            }
                            headers.push((k, v));
                        }
                    }

                    let mut buf = vec![0; len];
                    reader.read_exact(&mut buf).unwrap();
                    let body = serde_json::from_slice(&buf).ok();
                    log.lock().unwrap().push(Recorded {
                        method: method.clone(),
                        path: path.clone(),
                        headers,
                        body,
                        raw_body: buf,
                    });

                    let (status, resp) = routes
                        .iter()
                        .find(|(m, p, _, _)| *m == method && *p == path)
                        .map(|(_, _, s, b)| (*s, b.clone()))
                        .unwrap_or((404, json!({"error": "no route"})));

                    let text = if resp.is_string() { resp.as_str().unwrap().to_string() } else { resp.to_string() };

                    let _ = write!(
                        stream,
                        "HTTP/1.1 {status} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",
                        text.len()
                    );
                });
            }
        });

        Mock { url, log }
    }

    fn requests(&self) -> Vec<Recorded> {
        self.log.lock().unwrap().clone()
    }

    fn last(&self, method: &str) -> Recorded {
        self.requests().into_iter().rev().find(|r| r.method == method).expect("no such request")
    }
}

struct Env {
    dir: PathBuf,
}

impl Env {
    fn new() -> Env {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "notify-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        Env { dir }
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_notify"))
            .args(args)
            .env("NOTIFY_CONFIG_DIR", &self.dir)
            .env("NOTIFY_SECRET_STORE", "plaintext")
            .env("NOTIFY_ALLOW_PLAINTEXT_STORE", "1")
            .env("NOTIFY_MOCK_SYSTEM", "1")
            .output()
            .unwrap()
    }

    fn json(&self, args: &[&str]) -> (i32, Value, Value) {
        let mut all = args.to_vec();
        all.push("--json");
        let out = self.run(&all);
        let parse = |b: &[u8]| {
            let s = String::from_utf8_lossy(b);
            let s = s.lines().filter(|l| !l.starts_with("warning:")).collect::<Vec<_>>().join("\n");
            serde_json::from_str(&s).unwrap_or(Value::Null)
        };
        (out.status.code().unwrap_or(1), parse(&out.stdout), parse(&out.stderr))
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn test_agent_readme() {
    let env = Env::new();

    // Plain text markdown output
    let out = env.run(&["agent-readme"]);
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("# notify - agent operating manual"));

    // Structured JSON output
    let (code, stdout_json, _) = env.json(&["agent-readme"]);
    assert_eq!(code, 0);
    assert_eq!(stdout_json["tool"], "notify");
    assert_eq!(stdout_json["version"], "1.0.0");
    assert!(stdout_json["rules"].is_array());
    assert_eq!(stdout_json["exitCodes"]["0"], "ok");
    assert_eq!(stdout_json["exitCodes"]["3"], "auth_required - stop, surface the remediation to a human");
}

#[test]
fn test_system_notification() {
    let env = Env::new();

    let (code, stdout, _) = env.json(&["system", "--title", "Build Passed", "--description", "All tests green"]);
    assert_eq!(code, 0);
    assert_eq!(stdout["status"], "ok");
    assert_eq!(stdout["type"], "system_notification");
    assert_eq!(stdout["title"], "Build Passed");
    assert_eq!(stdout["description"], "All tests green");
    assert_eq!(stdout["delivered"], true);

    // Missing description should fail with invalid_input (code 6)
    let (code, _, stderr) = env.json(&["system", "--title", "Missing"]);
    assert_eq!(code, 6);
    assert_eq!(stderr["code"], "invalid_input");
}

#[test]
fn test_message_box() {
    let env = Env::new();

    // Command: message-box
    let (code, stdout, _) = env.json(&["message-box", "--title", "Confirm Deploy", "--message", "Proceed?"]);
    assert_eq!(code, 0);
    assert_eq!(stdout["status"], "ok");
    assert_eq!(stdout["type"], "message_box");
    assert_eq!(stdout["title"], "Confirm Deploy");
    assert_eq!(stdout["message"], "Proceed?");
    assert_eq!(stdout["dismissed"], true);

    // Alias: messagebox
    let (code, stdout, _) = env.json(&["messagebox", "--title", "Alert", "--message", "Done"]);
    assert_eq!(code, 0);
    assert_eq!(stdout["status"], "ok");
}

#[test]
fn test_accounts_lifecycle() {
    let env = Env::new();

    // Initially empty
    let (code, stdout, _) = env.json(&["accounts", "list"]);
    assert_eq!(code, 0);
    assert_eq!(stdout, json!([]));

    // Add Slack account
    let (code, stdout, _) = env.json(&[
        "accounts",
        "add",
        "alerts",
        "--type",
        "slack",
        "--webhook-url",
        "https://hooks.slack.com/services/T00/B00/X00",
        "--channel",
        "#dev-alerts",
    ]);
    assert_eq!(code, 0);
    assert_eq!(stdout["status"], "created");
    assert_eq!(stdout["name"], "alerts");
    assert_eq!(stdout["type"], "slack");

    // Add duplicate without --force fails with code 6
    let (code, _, stderr) = env.json(&[
        "accounts",
        "add",
        "alerts",
        "--type",
        "slack",
        "--webhook-url",
        "https://hooks.slack.com/services/other",
    ]);
    assert_eq!(code, 6);
    assert_eq!(stderr["code"], "invalid_input");
    assert!(stderr["remediation"].as_str().unwrap().contains("--force"));

    // Add duplicate with --force succeeds
    let (code, stdout, _) = env.json(&[
        "accounts",
        "add",
        "alerts",
        "--type",
        "slack",
        "--webhook-url",
        "https://hooks.slack.com/services/updated",
        "--force",
    ]);
    assert_eq!(code, 0);
    assert_eq!(stdout["status"], "updated");

    // Add Email account
    let (code, stdout, _) = env.json(&[
        "accounts",
        "add",
        "work-mail",
        "--type",
        "email",
        "--from",
        "dev@company.com",
        "--default-to",
        "team@company.com",
    ]);
    assert_eq!(code, 0);
    assert_eq!(stdout["status"], "created");

    // List accounts
    let (code, stdout, _) = env.json(&["accounts", "list"]);
    assert_eq!(code, 0);
    assert_eq!(stdout.as_array().unwrap().len(), 2);

    // Test accounts
    let (code, stdout, _) = env.json(&["accounts", "test", "alerts"]);
    assert_eq!(code, 0);
    assert_eq!(stdout["status"], "ok");
    assert_eq!(stdout["valid"], true);

    let (code, stdout, _) = env.json(&["accounts", "test", "work-mail"]);
    assert_eq!(code, 0);
    assert_eq!(stdout["status"], "ok");

    // Remove account
    let (code, stdout, _) = env.json(&["accounts", "remove", "alerts", "--yes"]);
    assert_eq!(code, 0);
    assert_eq!(stdout["status"], "removed");

    let (code, stdout, _) = env.json(&["accounts", "list"]);
    assert_eq!(code, 0);
    assert_eq!(stdout.as_array().unwrap().len(), 1);
}

#[test]
fn test_slack_webhook() {
    let mock = Mock::start(vec![
        ("POST", "webhook", 200, json!("ok")),
        ("POST", "webhook-err", 403, json!({"error": "invalid_token"})),
    ]);
    let env = Env::new();

    let webhook_url = format!("{}webhook", mock.url);

    // 1. Direct webhook URL with message
    let (code, stdout, _) = env.json(&[
        "slack",
        "--webhook-url",
        &webhook_url,
        "--message",
        "Hello from test",
        "--channel",
        "#test-channel",
    ]);
    assert_eq!(code, 0);
    assert_eq!(stdout["status"], "ok");
    assert_eq!(stdout["provider"], "slack");
    assert_eq!(stdout["delivered"], true);

    let req = mock.last("POST");
    assert_eq!(req.path, "webhook");
    assert_eq!(req.body.as_ref().unwrap()["text"], "Hello from test");
    assert_eq!(req.body.as_ref().unwrap()["channel"], "#test-channel");

    // 2. Via saved profile
    env.json(&[
        "accounts",
        "add",
        "test-profile",
        "--type",
        "slack",
        "--webhook-url",
        &webhook_url,
        "--username",
        "Botty",
    ]);

    let (code, stdout, _) = env.json(&["slack", "test-profile", "--message", "Profile message"]);
    assert_eq!(code, 0);
    assert_eq!(stdout["profile"], "test-profile");

    let req = mock.last("POST");
    assert_eq!(req.body.as_ref().unwrap()["text"], "Profile message");
    assert_eq!(req.body.as_ref().unwrap()["username"], "Botty");

    // 3. Error case - 403 maps to auth_required (code 3)
    let err_url = format!("{}webhook-err", mock.url);
    let (code, _, stderr) = env.json(&["slack", "--webhook-url", &err_url, "--message", "Test Fail"]);
    assert_eq!(code, 3);
    assert_eq!(stderr["code"], "auth_required");
}

#[test]
fn test_email_send_and_draft() {
    let mock = Mock::start(vec![
        ("POST", "messages/send", 200, json!({"id": "msg_999"})),
        ("POST", "drafts", 200, json!({"id": "draft_888"})),
        ("POST", "messages/fail", 401, json!({"error": "invalid_grant"})),
    ]);
    let env = Env::new();

    let send_endpoint = format!("{}messages/send", mock.url);
    let draft_endpoint = format!("{}drafts", mock.url);
    let fail_endpoint = format!("{}messages/fail", mock.url);

    // 1. Send normal email
    let (code, stdout, _) = env.json(&[
        "email",
        "--to",
        "user1@example.com,user2@example.com",
        "--subject",
        "Weekly Status",
        "--body",
        "Everything is running smoothly.",
        "--token",
        "ya29.test_token",
        "--endpoint",
        &send_endpoint,
    ]);
    assert_eq!(code, 0);
    assert_eq!(stdout["status"], "ok");
    assert_eq!(stdout["provider"], "gmail");
    assert_eq!(stdout["action"], "sent");
    assert_eq!(stdout["subject"], "Weekly Status");

    let req = mock.last("POST");
    assert_eq!(req.path, "messages/send");
    assert!(req.body.as_ref().unwrap()["raw"].is_string());

    // 2. Create draft
    let (code, stdout, _) = env.json(&[
        "email",
        "--to",
        "user@example.com",
        "--subject",
        "Draft Report",
        "--body",
        "<p>Hello <b>World</b></p>",
        "--draft",
        "--token",
        "ya29.test_token",
        "--endpoint",
        &draft_endpoint,
    ]);
    assert_eq!(code, 0);
    assert_eq!(stdout["action"], "draft_created");

    let req = mock.last("POST");
    assert_eq!(req.path, "drafts");
    assert!(req.body.as_ref().unwrap()["message"]["raw"].is_string());

    // 3. Error case - 401 maps to auth_required (code 3)
    let (code, _, stderr) = env.json(&[
        "email",
        "--to",
        "user@example.com",
        "--subject",
        "Fail Test",
        "--body",
        "Hi",
        "--token",
        "invalid_tok",
        "--endpoint",
        &fail_endpoint,
    ]);
    assert_eq!(code, 3);
    assert_eq!(stderr["code"], "auth_required");
}

#[test]
fn test_login_command() {
    let env = Env::new();

    // 1. Verify `notify login --help` succeeds
    let output = env.run(&["login", "--help"]);
    assert_eq!(output.status.code(), Some(0));


    // 2. Add an account via `notify login`
    let (code, stdout, _) = env.json(&[
        "login",
        "ci-alerts",
        "--type",
        "slack",
        "--webhook-url",
        "https://hooks.slack.com/services/LOGIN/TEST/123",
    ]);
    assert_eq!(code, 0);
    assert_eq!(stdout["status"], "created");
    assert_eq!(stdout["name"], "ci-alerts");
}

