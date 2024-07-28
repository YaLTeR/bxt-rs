//! Modules providing the actual functionality.
//!
//! The idea is that each module has more or less self-contained functionality and function pointer
//! requirements. This way if some function pointer isn't found, it disables only those modules
//! which require it for functioning, while everything else stays working.
//!
//! Every module is represented by a unit struct implementing the [`Module`] trait. All modules live
//! in the global [`MODULES`] array where they all can be operated on at once as trait objects.

// Modules have a lot of unsafe functions that are only intended to be called in one particular spot
// of the game code, and nowhere else. They are frequently called accordingly, too, e.g.
// `on_pm_move_start()`. Documenting this for every such function doesn't seem to add any meaningful
// clarity while being tedious. Therefore, allow missing safety doc for modules.
#![allow(clippy::missing_safety_doc)]

use crate::utils::*;

pub mod commands;
use commands::Command;

pub mod cvars;
use cvars::CVar;

pub mod campath;
pub mod capture;
pub mod capture_skip_non_gameplay;
pub mod capture_video_per_demo;
pub mod comment_overflow_fix;
pub mod demo_playback;
pub mod disable_loading_text;
pub mod emit_sound;
pub mod fade_remove;
pub mod fix_widescreen;
pub mod force_fov;
pub mod help;
pub mod hud;
pub mod hud_scale;
pub mod lightstyle;
pub mod novis;
pub mod player_movement_tracing;
pub mod remote_forbid;
pub mod rng_set;
pub mod scoreboard_remove;
pub mod shake_remove;
pub mod skybox_change;
pub mod skybox_remove;
pub mod tas_logging;
pub mod tas_optimizer;
pub mod tas_recording;
pub mod tas_server_time_fix;
pub mod tas_studio;
pub mod triangle_drawing;
pub mod viewmodel_remove;
pub mod viewmodel_sway;
pub mod wallhack;

/// Trait for getting module information.
pub trait Module: Sync {
    /// Returns the name of the module.
    fn name(&self) -> &'static str;

    /// Returns the description of the module.
    ///
    /// For short descriptions, try to return a string that would fit this phrase: "This module
    /// provides support for <description>". For example, `Playing multiple demos at once.` -- this
    /// fits the phrase: "This module provides support for playing multiple demos at once."
    fn description(&self) -> &'static str;

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
    &campath::Campath,
    &capture::Capture,
    &capture_skip_non_gameplay::CaptureSkipNonGameplay,
    &capture_video_per_demo::CaptureVideoPerDemo,
    &commands::Commands,
    &comment_overflow_fix::CommentOverflowFix,
    &cvars::CVars,
    &demo_playback::DemoPlayback,
    &disable_loading_text::DisableLoadingText,
    &emit_sound::EmitSound,
    &fade_remove::FadeRemove,
    &fix_widescreen::FixWidescreen,
    &force_fov::ForceFov,
    &help::Help,
    &hud::Hud,
    &hud_scale::HudScale,
    &lightstyle::LightStyle,
    &novis::NoVis,
    &player_movement_tracing::PlayerMovementTracing,
    &remote_forbid::RemoteForbid,
    &rng_set::RngSet,
    &scoreboard_remove::ScoreboardRemove,
    &shake_remove::ShakeRemove,
    &skybox_change::SkyboxChange,
    &skybox_remove::SkyboxRemove,
    &tas_logging::TasLogging,
    &tas_optimizer::TasOptimizer,
    &tas_recording::TasRecording,
    &tas_server_time_fix::TasServerTimeFix,
    &tas_studio::TasStudio,
    &triangle_drawing::TriangleDrawing,
    &viewmodel_remove::ViewmodelRemove,
    &viewmodel_sway::ViewmodelSway,
    &wallhack::Wallhack,
];
