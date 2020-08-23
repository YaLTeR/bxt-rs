//! Console commands.

use std::{cell::Cell, ffi::CStr};

use super::{Module, MODULES};
use crate::{hooks::engine, utils::*};

mod args;
pub use args::Args;

mod handler;
pub use handler::CommandHandler;

/// Pointer to a command handler function.
// Required until https://github.com/rust-lang/rust/issues/63997 is stabilized.
#[repr(transparent)]
pub struct HandlerFunction(pub unsafe extern "C" fn());

/// Console command.
pub struct Command {
    /// Name of the command.
    name: &'static [u8],

    /// Handler function.
    function: HandlerFunction,

    /// Whether the command is currently registered in the engine.
    is_registered: Cell<bool>,
}

// Safety: all methods accessing `command` require a `MainThreadMarker`.
unsafe impl Sync for Command {}

impl Command {
    /// Creates a new command.
    pub const fn new(name: &'static [u8], function: HandlerFunction) -> Self {
        Self {
            name,
            function,
            is_registered: Cell::new(false),
        }
    }

    /// Returns whether the command is currently registered in the engine.
    pub fn is_registered(&self, _marker: MainThreadMarker) -> bool {
        self.is_registered.get()
    }
}

/// Registers the command.
///
/// # Safety
///
/// This function must only be called when it's safe to register console commands.
unsafe fn register(marker: MainThreadMarker, command: &Command) {
    assert!(!command.is_registered(marker));

    // Make sure the provided name is a valid C string.
    assert!(CStr::from_bytes_with_nul(command.name).is_ok());

    engine::Cmd_AddMallocCommand.get(marker)(command.name.as_ptr().cast(), command.function.0, 0);

    command.is_registered.set(true);
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
    assert!(command.is_registered(marker));

    let name = CStr::from_bytes_with_nul(command.name).unwrap();

    // Find a pointer to the command. Start from `cmd_functions` (which points to the first
    // registered command). On each iteration, check if the pointer points to a command with the
    // name we're searching for, and if not, follow it. `cmd_functions` can't be null because
    // there's at least one registered command (the one we're de-registering).
    let mut prev_ptr = engine::cmd_functions.get(marker);
    assert!(!prev_ptr.is_null());

    while CStr::from_ptr((**prev_ptr).name) != name {
        // The next pointer can't be null because we still haven't found our (registered) command.
        assert!(!(**prev_ptr).next.is_null());

        prev_ptr = &mut (**prev_ptr).next;
    }

    let command_ptr = *prev_ptr;

    // Make it point to the next command. If there are no commands left, it will be set to null as
    // it should be.
    *prev_ptr = (**prev_ptr).next;

    // Free the engine-allocated command.
    engine::Mem_Free.get(marker)(command_ptr.cast());

    command.is_registered.set(false);
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
            if !command.is_registered(marker) {
                continue;
            }

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
            if !command.is_registered(marker) {
                continue;
            }

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
        engine::Memory_Init.is_set(marker)
            && engine::Host_Shutdown.is_set(marker)
            && engine::Cmd_AddMallocCommand.is_set(marker)
            && engine::Mem_Free.is_set(marker)
            && engine::cmd_functions.is_set(marker)
            && engine::Cmd_Argc.is_set(marker)
            && engine::Cmd_Argv.is_set(marker)
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
