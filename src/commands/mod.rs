//! Subcommand dispatch.

pub mod accounts;
pub mod email;
pub mod message_box;
pub mod slack;
pub mod system;

use crate::cli::Command;
use crate::error::Result;

pub fn run(cmd: Command) -> Result<()> {
    match cmd {
        Command::Slack(args) => slack::run(args),
        Command::Email(args) => email::run(args),
        Command::System(args) => system::run(args),
        Command::MessageBox(args) => message_box::run(args),
        Command::Login(args) => accounts::run(crate::cli::AccountsCommand::Add(args)),
        Command::Accounts { command } => accounts::run(command),

        Command::AgentReadme => {
            crate::readme::print();
            Ok(())
        }
    }
}
