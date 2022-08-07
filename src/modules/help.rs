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
        "Printing the list of modules, their console commands and variables and documentation."
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
        "bxt_help [module|command|variable]

Without arguments, shows the list of modules and their status.

With an argument, shows help for that module, command or variable.",
        help as fn(_),
        help_about as fn(_, _)
    ),
);

fn help(marker: MainThreadMarker) {
    if !Help.is_enabled(marker) {
        return;
    }

    let mut output = "bxt-rs modules:\n".to_string();

    let mut sorted_modules = MODULES.to_vec();
    sorted_modules.sort_unstable_by_key(|m| m.name().to_ascii_lowercase());
    sorted_modules.sort_by_key(|m| m.name().starts_with('_'));
    sorted_modules.sort_by_key(|m| !m.is_enabled(marker));

    for module in sorted_modules {
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

    writeln!(
        output,
        "\nUse `bxt_help name` to view help for a module, a console command or a console variable."
    )
    .expect("writing to `String` should never error");

    con_print(marker, &output);
}

fn help_about(marker: MainThreadMarker, what: String) {
    if !Help.is_enabled(marker) {
        return;
    }

    // First check console commands and variables for an exact match.
    for module in MODULES {
        for command in module.commands() {
            if command.name_str().eq_ignore_ascii_case(&what) {
                let mut output = format!("Console command in module \"{}\".\n\n", module.name());
                output += command.description();
                output += "\n";

                con_print(marker, &output);
                return;
            }
        }

        for cvar in module.cvars() {
            if cvar.name_str().eq_ignore_ascii_case(&what) {
                let mut output = format!("Console variable in module \"{}\".\n\n", module.name());

                writeln!(
                    output,
                    "Default value: {} \"{}\"\n",
                    cvar.name_str(),
                    cvar.default_value_str()
                )
                .expect("writing to `String` should never error");

                output += cvar.description();
                output += "\n";

                con_print(marker, &output);
                return;
            }
        }
    }

    // Then check module names for an exact match.
    for module in MODULES {
        if module.name().eq_ignore_ascii_case(&what) {
            let mut output = format!("Module \"{}\".", module.name());

            if !module.is_enabled(marker) {
                output += " [DISABLED]";
            }

            writeln!(output, "\n\n{}", module.description())
                .expect("writing to `String` should never error");

            if !module.commands().is_empty() {
                let mut sorted_commands = module.commands().to_vec();
                sorted_commands.sort_unstable_by_key(|c| c.name());
                sorted_commands.sort_by_key(|c| c.name()[0] == b'_');

                output += "\nConsole commands:\n";
                for command in sorted_commands {
                    writeln!(output, "- {}", command.name_str())
                        .expect("writing to `String` should never error");
                }
            }

            if !module.cvars().is_empty() {
                let mut sorted_cvars = module.cvars().to_vec();
                sorted_cvars.sort_unstable_by_key(|c| c.name());
                sorted_cvars.sort_by_key(|c| c.name()[0] == b'_');

                output += "\nConsole variables:\n";
                for cvar in sorted_cvars {
                    writeln!(output, "- {}", cvar.name_str())
                        .expect("writing to `String` should never error");
                }
            }

            con_print(marker, &output);
            return;
        }
    }

    con_print(
        marker,
        &format!(
            "Could not find anything matching `{}`.
        
Use `bxt_help name` to view help for a module, a console command or a console variable.\n",
            what
        ),
    );
}
