//! Outputs the Markdown source for the bxt-rs modules wiki page.

use std::fmt::Write;

extern crate bxt_rs;
use bxt_rs::modules::MODULES;

fn main() {
    println!(
        "\
# bxt-rs Modules

Each module represents a feature or a set of features of bxt-rs. Console commands and variables \
starting with an underscore (`_`) are not meant for normal use.

This wiki page is generated automatically with `src/bin/gen-wiki.rs`. Do not edit it by hand."
    );

    let mut sorted_modules = MODULES.to_vec();
    sorted_modules.sort_unstable_by_key(|m| m.name());

    for module in sorted_modules {
        println!("\n## {}", module.name());

        println!("\n{}", module.description());

        // Print console commands.
        let mut sorted_commands = module.commands().to_vec();
        sorted_commands.sort_unstable_by_key(|c| c.name());
        sorted_commands.sort_by_key(|c| c.name()[0] == b'_');

        if !sorted_commands.is_empty() {
            println!("\n### Console Commands");
        }

        for command in sorted_commands {
            let mut lines = command.description().lines();
            let first_line = lines.next().unwrap();
            let rest = lines.fold(String::new(), |mut s, l| {
                writeln!(s, "  {l}").unwrap();
                s
            });

            println!("\n- `{first_line}`\n{rest}");
        }

        // Print console variables.
        let mut sorted_cvars = module.cvars().to_vec();
        sorted_cvars.sort_unstable_by_key(|c| c.name());
        sorted_cvars.sort_by_key(|c| c.name()[0] == b'_');

        if !sorted_cvars.is_empty() {
            println!("\n### Console Variables");
        }

        for cvar in sorted_cvars {
            let description = cvar.description().lines().fold(String::new(), |mut s, l| {
                writeln!(s, "  {l}").unwrap();
                s
            });

            println!(
                "\n- `{}` (default: `\"{}\"`)\n\n{}",
                cvar.name_str(),
                cvar.default_value_str(),
                description
            );
        }
    }
}
