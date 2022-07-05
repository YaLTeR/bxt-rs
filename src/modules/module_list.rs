//! `bxt_module_list`

use std::fmt::Write;

use super::{Module, MODULES};
use crate::handler;
use crate::hooks::engine::{self, con_print};
use crate::modules::commands::{self, Command};
use crate::utils::*;

pub struct ModuleList;
impl Module for ModuleList {
    fn name(&self) -> &'static str {
        "bxt_module_list"
    }

    fn description(&self) -> &'static str {
        "Printing the list of modules."
    }

    fn commands(&self) -> &'static [&'static Command] {
        static COMMANDS: &[&Command] = &[&BXT_MODULE_LIST];
        COMMANDS
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        commands::Commands.is_enabled(marker) && engine::Con_Printf.is_set(marker)
    }
}

static BXT_MODULE_LIST: Command = Command::new(
    b"bxt_module_list\0",
    handler!(
        "bxt_module_list

Shows the list of modules and their status.",
        module_list as fn(_)
    ),
);

fn module_list(marker: MainThreadMarker) {
    if !ModuleList.is_enabled(marker) {
        return;
    }

    let mut output = String::new();
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
