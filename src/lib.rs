//! Speedrun and TAS tool for Half-Life 1 and mods.
#![allow(clippy::float_cmp, clippy::type_complexity)]

#[macro_use]
extern crate log;

mod ffi;
mod hooks;
mod modules;
mod utils;

pub use hooks::engine::{
    Host_Shutdown, LoadEntityDLLs, Memory_Init, ReleaseEntityDlls, SV_Frame, V_FadeAlpha,
};
