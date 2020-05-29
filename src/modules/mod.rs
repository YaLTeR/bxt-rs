//! Modules providing the actual functionality.

use crate::utils::MainThreadMarker;

pub mod commands;
use commands::Command;

pub mod cvars;
use cvars::CVar;

pub mod fade_remove;
pub mod module_list;
pub mod tas_logging;

/// Trait for getting module information.
pub trait Module: Sync {
    /// Returns the name of the module.
    fn name(&self) -> &'static str;

    /// Returns the console commands defined by the module.
    fn commands(&self) -> &'static [&'static Command] {
        &[]
    }

    /// Returns the console variables defined by the module.
    fn cvars(&self) -> &'static [&'static CVar] {
        &[]
    }

    /// Returns `true` if the module is enabled.
    fn is_enabled(&self, marker: MainThreadMarker) -> bool;
}

/// All modules.
pub static MODULES: &[&dyn Module] = &[
    &commands::Commands,
    &cvars::CVars,
    &fade_remove::FadeRemove,
    &module_list::ModuleList,
    &tas_logging::TASLogging,
];
