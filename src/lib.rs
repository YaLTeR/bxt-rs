//! Speedrun and TAS tool for Half-Life 1 and mods.

#[macro_use]
extern crate log;

pub mod hooks;
pub use hooks::*;

pub mod ffi;
pub mod modules;
pub mod utils;
