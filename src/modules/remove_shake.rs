//! `bxt_remove_shake`

use super::Module;
use crate::hooks::engine;
use crate::modules::cvars::{self, CVar};
use crate::utils::*;

pub struct RemoveShake;
impl Module for RemoveShake {
    fn name(&self) -> &'static str {
        "bxt_remove_shake"
    }

    fn cvars(&self) -> &'static [&'static CVar] {
        static CVARS: &[&CVar] = &[&BXT_REMOVE_SHAKE];
        CVARS
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        engine::V_ApplyShake.is_set(marker) && cvars::CVars.is_enabled(marker)
    }
}

static BXT_REMOVE_SHAKE: CVar = CVar::new(b"bxt_remove_shake\0", b"0\0");

/// Returns `true` if shake should currently be removed.
pub fn is_active(marker: MainThreadMarker) -> bool {
    if !RemoveShake.is_enabled(marker) {
        return false;
    }

    BXT_REMOVE_SHAKE.as_bool(marker)
}
