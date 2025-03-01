//! Raw bindings to C structs and functions.
//!
//! These are generated with a command similar to:
//!
//! ```sh
//! bindgen /path/to/halflife/pm_shared/pm_defs.h --whitelist-type playermove_s -- --target=i686-unknown-linux-gnu -I/path/to/halflife/{public,common} -include mathlib.h -include const.h > src/ffi/playermove.rs
//! ```
//!
//! and then manually cleaned up a bit.

pub mod buttons;
pub mod com_model;
pub mod command;
pub mod cvar;
pub mod edict;
pub mod playermove;
pub mod triangleapi;
