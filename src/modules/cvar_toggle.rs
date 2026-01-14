use std::collections::HashMap;

use once_cell::sync::Lazy;

use super::Module;
use crate::handler;
use crate::hooks::engine::{self, con_print, find_cvar, prepend_command};
use crate::modules::commands::Command;
use crate::modules::cvars::CVar;
use crate::utils::*;

pub struct CVarToggle;

impl Module for CVarToggle {
    fn name(&self) -> &'static str {
        "CVar Toggle"
    }

    fn description(&self) -> &'static str {
        "Toggling and adjusting cvars and commands"
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        engine::cvar_vars.is_set(marker)
    }

    fn commands(&self) -> &'static [&'static super::commands::Command] {
        static COMMANDS: &[&Command] = &[&BXT_CVAR_ADJUST, &BXT_CVAR_TOGGLE, &BXT_CVAR_CYCLE];
        COMMANDS
    }

    fn cvars(&self) -> &'static [&'static CVar] {
        &[]
    }
}

static CVAR_TOGGLES: Lazy<
    MainThreadRefCell<
        HashMap<
            // cvar name
            String,
            // (current value, list of values)
            (usize, Vec<String>),
        >,
    >,
> = Lazy::new(|| MainThreadRefCell::new(HashMap::new()));

static CVAR_CYCLES: Lazy<
    MainThreadRefCell<
        HashMap<
            // hash of the cycle
            String,
            // (current value, list of values)
            (usize, Vec<String>),
        >,
    >,
> = Lazy::new(|| MainThreadRefCell::new(HashMap::new()));
static BXT_CVAR_TOGGLE: Command = Command::new(
    b"bxt_cvar_toggle\0",
    handler!(
        "bxt_cvar_toggle <cvar> <value 1> <value 2> <value 3> ...

Cycles values given a cvar or command. Supports up to 9 values.",
        cvar_toggle as fn(_, _),
        cvar_toggle3 as fn(_, _, _, _),
        cvar_toggle4 as fn(_, _, _, _, _),
        cvar_toggle5 as fn(_, _, _, _, _, _),
        cvar_toggle6 as fn(_, _, _, _, _, _, _),
        cvar_toggle7 as fn(_, _, _, _, _, _, _, _),
        cvar_toggle8 as fn(_, _, _, _, _, _, _, _, _),
        cvar_toggle9 as fn(_, _, _, _, _, _, _, _, _, _),
        cvar_toggle10 as fn(_, _, _, _, _, _, _, _, _, _, _)
    ),
);

static BXT_CVAR_CYCLE: Command = Command::new(
    b"bxt_cvar_cycle\0",
    handler!(
        "bxt_cvar_cycle <command 1> <command 2> <command 3> ...

Cycles through a list of commands. Supports up to 9 commands.",
        cvar_cycle as fn(_, _),
        cvar_cycle2 as fn(_, _, _),
        cvar_cycle3 as fn(_, _, _, _),
        cvar_cycle4 as fn(_, _, _, _, _),
        cvar_cycle5 as fn(_, _, _, _, _, _),
        cvar_cycle6 as fn(_, _, _, _, _, _, _),
        cvar_cycle7 as fn(_, _, _, _, _, _, _, _),
        cvar_cycle8 as fn(_, _, _, _, _, _, _, _, _),
        cvar_cycle9 as fn(_, _, _, _, _, _, _, _, _, _),
        cvar_cycle10 as fn(_, _, _, _, _, _, _, _, _, _, _)
    ),
);

static BXT_CVAR_ADJUST: Command = Command::new(
    b"bxt_cvar_adjust\0",
    handler!(
        "bxt_cvar_adjust <cvar> <+/- value>

Adjusts a cvar by a given value.",
        cvar_adjust as fn(_, _, _)
    ),
);

fn cvar_toggle3(marker: MainThreadMarker, arg1: String, arg2: String, arg3: String) {
    cvar_toggle(marker, format!("{arg1} {arg2} {arg3}"));
}

fn cvar_toggle4(marker: MainThreadMarker, arg1: String, arg2: String, arg3: String, arg4: String) {
    cvar_toggle(marker, format!("{arg1} {arg2} {arg3} {arg4}"));
}

fn cvar_toggle5(
    marker: MainThreadMarker,
    arg1: String,
    arg2: String,
    arg3: String,
    arg4: String,
    arg5: String,
) {
    cvar_toggle(marker, format!("{arg1} {arg2} {arg3} {arg4} {arg5}"));
}

fn cvar_toggle6(
    marker: MainThreadMarker,
    arg1: String,
    arg2: String,
    arg3: String,
    arg4: String,
    arg5: String,
    arg6: String,
) {
    cvar_toggle(marker, format!("{arg1} {arg2} {arg3} {arg4} {arg5} {arg6}"));
}

fn cvar_toggle7(
    marker: MainThreadMarker,
    arg1: String,
    arg2: String,
    arg3: String,
    arg4: String,
    arg5: String,
    arg6: String,
    arg7: String,
) {
    cvar_toggle(
        marker,
        format!("{arg1} {arg2} {arg3} {arg4} {arg5} {arg6} {arg7}"),
    );
}

fn cvar_toggle8(
    marker: MainThreadMarker,
    arg1: String,
    arg2: String,
    arg3: String,
    arg4: String,
    arg5: String,
    arg6: String,
    arg7: String,
    arg8: String,
) {
    cvar_toggle(
        marker,
        format!("{arg1} {arg2} {arg3} {arg4} {arg5} {arg6} {arg7} {arg8}"),
    );
}

fn cvar_toggle9(
    marker: MainThreadMarker,
    arg1: String,
    arg2: String,
    arg3: String,
    arg4: String,
    arg5: String,
    arg6: String,
    arg7: String,
    arg8: String,
    arg9: String,
) {
    cvar_toggle(
        marker,
        format!("{arg1} {arg2} {arg3} {arg4} {arg5} {arg6} {arg7} {arg8} {arg9}"),
    );
}

fn cvar_toggle10(
    marker: MainThreadMarker,
    arg1: String,
    arg2: String,
    arg3: String,
    arg4: String,
    arg5: String,
    arg6: String,
    arg7: String,
    arg8: String,
    arg9: String,
    arg10: String,
) {
    cvar_toggle(
        marker,
        format!("{arg1} {arg2} {arg3} {arg4} {arg5} {arg6} {arg7} {arg8} {arg9} {arg10}"),
    );
}

fn cvar_toggle(marker: MainThreadMarker, args: String) {
    let split = args
        .split_whitespace()
        .map(|x| x.to_owned())
        .collect::<Vec<String>>();

    // at least 3 arguments, cvar, a, b
    if split.len() < 3 {
        con_print(marker, "Invalid usage.\n");
        con_print(marker, BXT_CVAR_TOGGLE.description());
        return;
    }

    let cvar_name = &split[0];
    let toggles = &split[1..];

    let mut binding = CVAR_TOGGLES.borrow_mut(marker);
    let (cvar_curr_toggle, cvar_toggles) = binding
        .entry(cvar_name.clone())
        .or_insert((0, toggles.iter().map(|x| x.to_string()).collect()));

    // if not matching the length, we add new toggles
    // and then toggle the first value
    if cvar_toggles.len() != toggles.len() {
        *cvar_toggles = toggles.iter().map(|x| x.to_string()).collect();
        *cvar_curr_toggle = 0;
    }

    // the command is executed in the next frame, whatever
    prepend_command(
        marker,
        format!("{} {}\n", cvar_name, cvar_toggles[*cvar_curr_toggle]).as_str(),
    );

    *cvar_curr_toggle = (*cvar_curr_toggle + 1) % cvar_toggles.len();
}

fn cvar_cycle2(marker: MainThreadMarker, arg1: String, arg2: String) {
    cvar_cycle(marker, format!("{arg1} {arg2}"));
}

fn cvar_cycle3(marker: MainThreadMarker, arg1: String, arg2: String, arg3: String) {
    cvar_cycle(marker, format!("{arg1} {arg2} {arg3}"));
}

fn cvar_cycle4(marker: MainThreadMarker, arg1: String, arg2: String, arg3: String, arg4: String) {
    cvar_cycle(marker, format!("{arg1} {arg2} {arg3} {arg4}"));
}

fn cvar_cycle5(
    marker: MainThreadMarker,
    arg1: String,
    arg2: String,
    arg3: String,
    arg4: String,
    arg5: String,
) {
    cvar_cycle(marker, format!("{arg1} {arg2} {arg3} {arg4} {arg5}"));
}

fn cvar_cycle6(
    marker: MainThreadMarker,
    arg1: String,
    arg2: String,
    arg3: String,
    arg4: String,
    arg5: String,
    arg6: String,
) {
    cvar_cycle(marker, format!("{arg1} {arg2} {arg3} {arg4} {arg5} {arg6}"));
}

fn cvar_cycle7(
    marker: MainThreadMarker,
    arg1: String,
    arg2: String,
    arg3: String,
    arg4: String,
    arg5: String,
    arg6: String,
    arg7: String,
) {
    cvar_cycle(
        marker,
        format!("{arg1} {arg2} {arg3} {arg4} {arg5} {arg6} {arg7}"),
    );
}

fn cvar_cycle8(
    marker: MainThreadMarker,
    arg1: String,
    arg2: String,
    arg3: String,
    arg4: String,
    arg5: String,
    arg6: String,
    arg7: String,
    arg8: String,
) {
    cvar_cycle(
        marker,
        format!("{arg1} {arg2} {arg3} {arg4} {arg5} {arg6} {arg7} {arg8}"),
    );
}

fn cvar_cycle9(
    marker: MainThreadMarker,
    arg1: String,
    arg2: String,
    arg3: String,
    arg4: String,
    arg5: String,
    arg6: String,
    arg7: String,
    arg8: String,
    arg9: String,
) {
    cvar_cycle(
        marker,
        format!("{arg1} {arg2} {arg3} {arg4} {arg5} {arg6} {arg7} {arg8} {arg9}"),
    );
}

fn cvar_cycle10(
    marker: MainThreadMarker,
    arg1: String,
    arg2: String,
    arg3: String,
    arg4: String,
    arg5: String,
    arg6: String,
    arg7: String,
    arg8: String,
    arg9: String,
    arg10: String,
) {
    cvar_cycle(
        marker,
        format!("{arg1} {arg2} {arg3} {arg4} {arg5} {arg6} {arg7} {arg8} {arg9} {arg10}"),
    );
}

fn cvar_cycle(marker: MainThreadMarker, args: String) {
    let new_cycle = args
        .split_whitespace()
        .map(|x| x.to_owned())
        .collect::<Vec<String>>();

    let cycle_hashable = args;

    let mut binding = CVAR_CYCLES.borrow_mut(marker);
    let (curr_command, commands) = binding
        .entry(cycle_hashable)
        .or_insert((0, new_cycle.iter().map(|x| x.to_string()).collect()));

    prepend_command(marker, format!("{}\n", commands[*curr_command]).as_str());

    *curr_command = (*curr_command + 1) % commands.len();
}

fn cvar_adjust(marker: MainThreadMarker, cvar_name: String, adjustment: f32) {
    let Some(cvar) = (unsafe { find_cvar(marker, &cvar_name) }) else {
        con_print(marker, "Cannot find cvar\n");
        return;
    };

    // in case the cvar takes in a string, the value is always 0
    // it is up to the user that they know what they are doing
    let value = unsafe { (*cvar).value };
    let new_value = value + adjustment;

    prepend_command(marker, format!("{} {}\n", cvar_name, new_value).as_str());
}
