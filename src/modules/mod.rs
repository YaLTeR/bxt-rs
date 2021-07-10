//! Modules providing the actual functionality.

use crate::utils::*;

pub mod commands;
use commands::Command;

pub mod cvars;
use cvars::CVar;

pub mod capture;
pub mod demo_playback;
pub mod fade_remove;
pub mod force_fov;
pub mod hud_scale;
pub mod module_list;
pub mod novis;
pub mod shake_remove;
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
    &capture::Capture,
    &commands::Commands,
    &cvars::CVars,
    &demo_playback::DemoPlayback,
    &fade_remove::FadeRemove,
    &force_fov::ForceFov,
    &hud_scale::HudScale,
    &module_list::ModuleList,
    &novis::NoVis,
    &shake_remove::ShakeRemove,
    &tas_logging::TasLogging,
];
