//! `bxt_moules_list`

use super::{Module, MODULES};
use crate::{
    handler,
    hooks::engine::{self, con_print},
    modules::commands::{self, Command},
    utils::*,
};

pub struct ModuleList;
impl Module for ModuleList {
    fn name(&self) -> &'static str {
        "bxt_module_list"
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
        "Usage: bxt_module_list\n \
          Shows the list of modules and their status.\n",
        module_list as fn(_)
    ),
);

fn module_list(marker: MainThreadMarker) {
    if !ModuleList.is_enabled(marker) {
        return;
    }

    let mut output = String::new();
    for module in MODULES {
        output.push_str(&format!(
            "- {}{}\n",
            if module.is_enabled(marker) {
                ""
            } else {
                "[DISABLED] "
            },
            module.name()
        ));
    }

    con_print(marker, &output);
}
