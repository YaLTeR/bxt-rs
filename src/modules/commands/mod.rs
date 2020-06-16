//! Console commands.

use std::ffi::CStr;

use super::{Module, MODULES};
use crate::{hooks::engine, utils::MainThreadMarker};

mod args;
pub use args::Args;

mod handler;
pub use handler::CommandHandler;

/// Console command.
#[derive(Debug)]
pub struct Command {
    /// Name of the command.
    pub name: &'static [u8],

    /// Handler function.
    pub function: unsafe extern "C" fn(),
}

/// Registers the command.
///
/// # Safety
///
/// This function must only be called when it's safe to register console commands.
unsafe fn register(marker: MainThreadMarker, command: &Command) {
    // Make sure the provided name is a valid C string.
    assert!(CStr::from_bytes_with_nul(command.name).is_ok());

    engine::CMD_ADDMALLOCCOMMAND.get(marker)(command.name.as_ptr().cast(), command.function, 0);
}

/// De-registers the command.
///
/// # Safety
///
/// This function must only be called when it's safe to de-register console commands. The command
/// must have been registered with `register()`.
///
/// # Panics
///
/// Panics if the command is not registered.
unsafe fn deregister(marker: MainThreadMarker, command: &Command) {
    let name = CStr::from_bytes_with_nul(command.name).unwrap();

    // Find a pointer to the command. Start from `cmd_functions` (which points to the first
    // registered command). On each iteration, check if the pointer points to a command with the
    // name we're searching for, and if not, follow it. `cmd_functions` can't be null because
    // there's at least one registered command (the one we're de-registering).
    let mut prev_ptr = engine::CMD_FUNCTIONS.get(marker);
    assert!(!prev_ptr.is_null());

    while CStr::from_ptr((**prev_ptr).name) != name {
        // The next pointer can't be null because we still haven't found our (registered) command.
        assert!(!(**prev_ptr).next.is_null());

        prev_ptr = &mut (**prev_ptr).next;
    }

    let command = *prev_ptr;

    // Make it point to the next command. If there are no commands left, it will be set to null as
    // it should be.
    *prev_ptr = (**prev_ptr).next;

    // Free the engine-allocated command.
    engine::MEM_FREE.get(marker)(command.cast());
}

/// # Safety
///
/// This function must only be called right after `Memory_Init()` completes.
pub unsafe fn register_all_commands(marker: MainThreadMarker) {
    if !Commands.is_enabled(marker) {
        return;
    }

    for module in MODULES {
        for command in module.commands() {
            trace!(
                "registering {}",
                CStr::from_bytes_with_nul(command.name)
                    .unwrap()
                    .to_string_lossy()
            );

            register(marker, command);
        }
    }
}

/// # Safety
///
/// This function must only be called right before `Host_Shutdown()` is called.
pub unsafe fn deregister_all_commands(marker: MainThreadMarker) {
    if !Commands.is_enabled(marker) {
        return;
    }

    for module in MODULES {
        // Disabled modules already had their commands de-registered.
        if !module.is_enabled(marker) {
            continue;
        }

        for command in module.commands() {
            trace!(
                "de-registering {}",
                CStr::from_bytes_with_nul(command.name)
                    .unwrap()
                    .to_string_lossy()
            );

            deregister(marker, command);
        }
    }
}

/// # Safety
///
/// This function must only be called when it's safe to de-register console commands.
pub unsafe fn deregister_disabled_module_commands(marker: MainThreadMarker) {
    if !Commands.is_enabled(marker) {
        return;
    }

    for module in MODULES {
        if module.is_enabled(marker) {
            continue;
        }

        for command in module.commands() {
            trace!(
                "de-registering {}",
                CStr::from_bytes_with_nul(command.name)
                    .unwrap()
                    .to_string_lossy()
            );

            deregister(marker, command);
        }
    }
}

pub struct Commands;
impl Module for Commands {
    fn name(&self) -> &'static str {
        "Console commands"
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        engine::MEMORY_INIT.is_set(marker)
            && engine::HOST_SHUTDOWN.is_set(marker)
            && engine::CMD_ADDMALLOCCOMMAND.is_set(marker)
            && engine::MEM_FREE.is_set(marker)
            && engine::CMD_FUNCTIONS.is_set(marker)
            && engine::CMD_ARGC.is_set(marker)
            && engine::CMD_ARGV.is_set(marker)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_names() {
        for module in MODULES {
            for command in module.commands() {
                assert!(CStr::from_bytes_with_nul(command.name).is_ok());
            }
        }
    }
}
