//! `bxt_help`

use std::fmt::Write;

use super::{Module, MODULES};
use crate::handler;
use crate::hooks::engine::{self, con_print};
use crate::modules::commands::{self, Command};
use crate::utils::*;

pub struct Help;
impl Module for Help {
    fn name(&self) -> &'static str {
        "Help"
    }

    fn description(&self) -> &'static str {
        "Printing the list of modules."
    }

    fn commands(&self) -> &'static [&'static Command] {
        static COMMANDS: &[&Command] = &[&BXT_HELP];
        COMMANDS
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        commands::Commands.is_enabled(marker) && engine::Con_Printf.is_set(marker)
    }
}

static BXT_HELP: Command = Command::new(
    b"bxt_help\0",
    handler!(
        "bxt_help

Shows the list of modules and their status.",
        help as fn(_)
    ),
);

fn help(marker: MainThreadMarker) {
    if !Help.is_enabled(marker) {
        return;
    }

    let mut output = "bxt-rs modules:\n".to_string();
    for module in MODULES {
        writeln!(
            output,
            "- {}{}",
            if module.is_enabled(marker) {
                ""
            } else {
                "[DISABLED] "
            },
            module.name()
        )
        .expect("writing to `String` should never error");
    }

    con_print(marker, &output);
}
