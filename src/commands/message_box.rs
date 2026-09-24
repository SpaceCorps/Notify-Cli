//! Handler for `notify message-box` (and alias `messagebox`).

use crate::cli::MessageBoxArgs;
use crate::error::Result;
use crate::system::show_message_box;
use crate::{obj, output};

pub fn run(args: MessageBoxArgs) -> Result<()> {
    show_message_box(&args.title, &args.message)?;

    output::write(&obj! {
        "status" => "ok",
        "type" => "message_box",
        "title" => args.title,
        "message" => args.message,
        "dismissed" => true,
    });

    Ok(())
}
