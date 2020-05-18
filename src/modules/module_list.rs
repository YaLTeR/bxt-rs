//! `bxt_modules_list`

use super::{Module, MODULES};
use crate::{
    engine,
    modules::commands::{self, Command},
    utils::{abort_on_panic, MainThreadMarker},
};
use std::ffi::CString;

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
    function: modules_list,
};

unsafe extern "C" fn modules_list() {
    abort_on_panic(move || {
        let marker = MainThreadMarker::new();

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

        let c_string = CString::new(output).unwrap();
        engine::CON_PRINTF.get(marker)(b"%s\0".as_ptr().cast(), c_string.as_ptr());
    })
}
