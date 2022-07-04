//! `_bxt_rng_set`.

use super::Module;
use crate::handler;
use crate::hooks::engine::{self, RngState};
use crate::modules::commands::{self, Command};
use crate::utils::*;

pub struct RngSet;
impl Module for RngSet {
    fn name(&self) -> &'static str {
        "_bxt_rng_set"
    }

    fn description(&self) -> &'static str {
        "Setting the engine RNG state."
    }

    fn commands(&self) -> &'static [&'static Command] {
        static COMMANDS: &[&Command] = &[&BXT_RNG_SET];
        COMMANDS
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        commands::Commands.is_enabled(marker)
            && engine::idum.is_set(marker)
            && engine::ran1_iy.is_set(marker)
            && engine::ran1_iv.is_set(marker)
    }
}

static BXT_RNG_SET: Command = Command::new(
    b"_bxt_rng_set\0",
    handler!(
        "Usage: _bxt_rng_set \"<idum> <iy> <iv[0]> <iv[1]> ... <iv[31]>\"\n \
          Sets the non-shared RNG state.\n",
        rng_set as fn(_, _)
    ),
);

fn rng_set(marker: MainThreadMarker, rng_state: RngState) {
    if !RngSet.is_enabled(marker) {
        return;
    }

    engine::set_rng_state(marker, rng_state);
}
