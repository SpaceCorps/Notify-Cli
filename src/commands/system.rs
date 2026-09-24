//! Handler for `notify system`.

use crate::cli::SystemArgs;
use crate::error::Result;
use crate::system::send_notification;
use crate::{obj, output};

pub fn run(args: SystemArgs) -> Result<()> {
    send_notification(&args.title, &args.description)?;

    output::write(&obj! {
        "status" => "ok",
        "type" => "system_notification",
        "title" => args.title,
        "description" => args.description,
        "delivered" => true,
    });

    Ok(())
}
