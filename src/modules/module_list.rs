//! `bxt_modules_list`

use super::{Module, MODULES};
use crate::{
    handler,
    hooks::engine::{self, Engine},
    modules::commands::{self, Command},
    utils::MainThreadMarker,
};

pub struct ModuleList;
impl Module for ModuleList {
    fn name(&self) -> &'static str {
        "bxt_modules_list"
    }

    fn commands(&self) -> &'static [&'static Command] {
        static COMMANDS: &[&Command] = &[&BXT_MODULES_LIST];
        &COMMANDS
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        commands::Commands.is_enabled(marker) && engine::CON_PRINTF.is_set(marker)
    }
}

static BXT_MODULES_LIST: Command = Command {
    name: b"bxt_modules_list\0",
    function: handler!(
        "Usage: bxt_module_list\n \
          Shows the list of modules and their status.\n",
        modules_list as fn(_)
    ),
};

fn modules_list(engine: &Engine) {
    if !ModuleList.is_enabled(engine.marker()) {
        return;
    }

    let mut output = String::new();
    for module in MODULES {
        output.push_str(&format!(
            "- {}{}\n",
            if module.is_enabled(engine.marker()) {
                ""
            } else {
                "[DISABLED] "
            },
            module.name()
        ));
    }

    engine.print(&output);
}
