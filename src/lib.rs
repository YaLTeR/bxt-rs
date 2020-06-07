//! Speedrun and TAS tool for Half-Life 1 and mods.
#![allow(clippy::float_cmp, clippy::type_complexity)]

#[macro_use]
extern crate log;

pub mod hooks;
pub use hooks::*;

pub mod ffi;
pub mod modules;
pub mod utils;
