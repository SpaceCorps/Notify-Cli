//! Command-line argument definitions and clap hierarchy.

use clap::{Args, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "notify",
    version,
    about = "Cross-platform notification CLI - Slack, Email, System desktop notifications, and GUI message boxes",
    long_about = None
)]
pub struct Cli {
    #[arg(long, global = true, help = "Output raw JSON instead of YAML")]
    pub json: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    #[command(about = "Send a Slack message using a webhook URL or config profile")]
    Slack(SlackArgs),

    #[command(about = "Send an email using an email provider (Gmail API) or config profile")]
    Email(EmailArgs),

    #[command(about = "Show a native OS desktop notification (toast/banner)")]
    System(SystemArgs),

    #[command(name = "message-box", alias = "messagebox", about = "Show a native OS message box dialog")]
    MessageBox(MessageBoxArgs),

    #[command(about = "Manage notification accounts and keystore credentials")]
    Accounts {
        #[command(subcommand)]
        command: AccountsCommand,
    },

    #[command(name = "agent-readme", about = "Print the agent operating manual in markdown or JSON")]
    AgentReadme,
}

#[derive(Args, Debug)]
pub struct SlackArgs {
    #[arg(value_name = "PROFILE", help = "Configured Slack profile name or account")]
    pub profile: Option<String>,

    #[arg(short = 'a', long = "account", help = "Account/profile name (alias for PROFILE)")]
    pub account: Option<String>,

    #[arg(long = "webhook-url", help = "Direct Slack incoming webhook URL")]
    pub webhook_url: Option<String>,

    #[arg(short = 'm', long = "message", help = "Plain text message to send")]
    pub message: Option<String>,

    #[arg(
        long = "json-payload",
        alias = "payload",
        alias = "data",
        help = "Raw JSON payload to send (Slack block kit / custom payload)"
    )]
    pub json_payload: Option<String>,

    #[arg(long = "json-file", help = "Path to file containing JSON payload")]
    pub json_file: Option<String>,

    #[arg(long = "channel", help = "Override destination channel (e.g. #alerts)")]
    pub channel: Option<String>,

    #[arg(long = "username", help = "Override sender bot username")]
    pub username: Option<String>,

    #[arg(long = "icon-emoji", help = "Override sender bot emoji (e.g. :bell:)")]
    pub icon_emoji: Option<String>,
}

#[derive(Args, Debug)]
pub struct EmailArgs {
    #[arg(value_name = "PROFILE", help = "Configured Email profile name or account")]
    pub profile: Option<String>,

    #[arg(short = 'a', long = "account", help = "Account/profile name (alias for PROFILE)")]
    pub account: Option<String>,

    #[arg(long = "to", help = "Recipient email address(es) (repeatable or comma-separated)")]
    pub to: Vec<String>,

    #[arg(long = "cc", help = "CC recipient email address(es) (repeatable or comma-separated)")]
    pub cc: Vec<String>,

    #[arg(long = "bcc", help = "BCC recipient email address(es) (repeatable or comma-separated)")]
    pub bcc: Vec<String>,

    #[arg(short = 's', long = "subject", help = "Email subject line")]
    pub subject: Option<String>,

    #[arg(short = 'b', long = "body", help = "Email body text or HTML")]
    pub body: Option<String>,

    #[arg(long = "file", help = "Path to file containing email body content")]
    pub file: Option<String>,

    #[arg(long = "attach", help = "File path(s) to attach (repeatable)")]
    pub attach: Vec<String>,

    #[arg(long = "draft", help = "Create a draft instead of sending")]
    pub draft: bool,

    #[arg(long = "provider", help = "Email provider (default: gmail)")]
    pub provider: Option<String>,

    #[arg(long = "endpoint", help = "Custom email API endpoint URL")]
    pub endpoint: Option<String>,

    #[arg(long = "token", help = "Bearer authentication token for API")]
    pub token: Option<String>,
}

#[derive(Args, Debug)]
pub struct SystemArgs {
    #[arg(short = 't', long = "title", help = "Notification title")]
    pub title: String,

    #[arg(
        short = 'd',
        long = "description",
        alias = "desc",
        alias = "message",
        help = "Notification description text"
    )]
    pub description: String,
}

#[derive(Args, Debug)]
pub struct MessageBoxArgs {
    #[arg(short = 't', long = "title", help = "Dialog title")]
    pub title: String,

    #[arg(short = 'm', long = "message", help = "Dialog message text")]
    pub message: String,
}

#[derive(Subcommand, Debug)]
pub enum AccountsCommand {
    #[command(about = "List all configured accounts and credentials")]
    List {
        #[arg(long, help = "Verify account credentials against the remote service")]
        check: bool,
    },

    #[command(about = "Add or update a notification account/profile")]
    Add(Box<AddAccountArgs>),

    #[command(about = "Test an account connection and credentials")]
    Test {
        #[arg(value_name = "NAME", help = "Account name to test")]
        name: String,
    },

    #[command(about = "Remove an account and delete its credentials from the keystore")]
    Remove {
        #[arg(value_name = "NAME", help = "Account name to remove")]
        name: String,

        #[arg(short = 'y', long = "yes", help = "Skip interactive confirmation")]
        yes: bool,
    },
}

#[derive(Args, Debug)]
pub struct AddAccountArgs {
    #[arg(value_name = "NAME", help = "Account / profile name")]
    pub name: String,

    #[arg(long = "type", default_value = "slack", help = "Account type (slack or email)")]
    pub account_type: String,

    #[arg(long = "webhook-url", help = "Slack incoming webhook URL")]
    pub webhook_url: Option<String>,

    #[arg(long = "api-key", help = "API key or secret token")]
    pub api_key: Option<String>,

    #[arg(long = "api-key-stdin", help = "Read API key or webhook URL from stdin")]
    pub api_key_stdin: bool,

    #[arg(long = "channel", help = "Default Slack channel")]
    pub channel: Option<String>,

    #[arg(long = "username", help = "Default Slack username")]
    pub username: Option<String>,

    #[arg(long = "icon-emoji", help = "Default Slack icon emoji")]
    pub icon_emoji: Option<String>,

    #[arg(long = "from", help = "From email address")]
    pub from: Option<String>,

    #[arg(long = "default-to", help = "Default recipient email address")]
    pub default_to: Option<String>,

    #[arg(long = "provider", help = "Email provider (default: gmail)")]
    pub provider: Option<String>,

    #[arg(long = "endpoint", help = "Custom endpoint URL")]
    pub endpoint: Option<String>,

    #[arg(long = "force", help = "Overwrite existing account without error")]
    pub force: bool,
}
