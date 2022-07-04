//! `bxt_shake_remove`

use super::Module;
use crate::hooks::engine;
use crate::modules::cvars::{self, CVar};
use crate::utils::*;

pub struct ShakeRemove;
impl Module for ShakeRemove {
    fn name(&self) -> &'static str {
        "bxt_shake_remove"
    }

    fn description(&self) -> &'static str {
        "Removing the screen shake effect."
    }

    fn cvars(&self) -> &'static [&'static CVar] {
        static CVARS: &[&CVar] = &[&BXT_SHAKE_REMOVE];
        CVARS
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        engine::V_ApplyShake.is_set(marker) && cvars::CVars.is_enabled(marker)
    }
}

static BXT_SHAKE_REMOVE: CVar = CVar::new(b"bxt_shake_remove\0", b"0\0");

/// Returns `true` if shake should currently be removed.
pub fn is_active(marker: MainThreadMarker) -> bool {
    if !ShakeRemove.is_enabled(marker) {
        return false;
    }

    BXT_SHAKE_REMOVE.as_bool(marker)
}
