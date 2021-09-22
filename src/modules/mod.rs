//! Modules providing the actual functionality.
//!
//! The idea is that each module has more or less self-contained functionality and function pointer
//! requirements. This way if some function pointer isn't found, it disables only those modules
//! which require it for functioning, while everything else stays working.
//!
//! Every module is represented by a unit struct implementing the [`Module`] trait. All modules live
//! in the global [`MODULES`] array where they all can be operated on at once as trait objects.

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
pub mod rng_set;
pub mod shake_remove;
pub mod skybox_remove;
pub mod tas_logging;
pub mod tas_recording;
pub mod tas_rng_fix;
pub mod wallhack;

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
    ///
    /// If you return `false`, the module's console variables and commands will be de-registered. So
    /// return `false` only if some essential function or piece of functionality is unavailable. If
    /// the module can still work (perhaps in a limited fashion) with certain functions missing,
    /// don't include them in this check, instead check them individually before using.
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
    &rng_set::RngSet,
    &shake_remove::ShakeRemove,
    &skybox_remove::SkyboxRemove,
    &tas_logging::TasLogging,
    &tas_recording::TasRecording,
    &tas_rng_fix::TasRngFix,
    &wallhack::Wallhack,
];
