//! Speedrun and TAS tool for Half-Life 1 and mods.
#![allow(clippy::float_cmp, clippy::type_complexity)]

#[macro_use]
extern crate log;

mod ffi;
mod gl;
mod hooks;
mod modules;
mod utils;
mod vulkan;

#[cfg(windows)]
mod windows;

// Export all functions we want to hook via LD_PRELOAD on Linux.
#[cfg(unix)]
pub use hooks::engine::exported::*;
