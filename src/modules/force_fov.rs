//! `bxt_force_fov`

use super::Module;
use crate::hooks::engine;
use crate::modules::cvars::{self, CVar};
use crate::utils::*;

pub struct ForceFov;
impl Module for ForceFov {
    fn name(&self) -> &'static str {
        "bxt_force_fov"
    }

    fn description(&self) -> &'static str {
        "Overriding the field-of-view."
    }

    fn cvars(&self) -> &'static [&'static CVar] {
        static CVARS: &[&CVar] = &[&BXT_FORCE_FOV];
        CVARS
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        engine::R_SetFrustum.is_set(marker)
            && engine::scr_fov_value.is_set(marker)
            && cvars::CVars.is_enabled(marker)
    }
}

static BXT_FORCE_FOV: CVar = CVar::new(b"bxt_force_fov\0", b"0\0");

/// Returns the FOV value to force, if any.
pub fn fov(marker: MainThreadMarker) -> Option<f32> {
    if !ForceFov.is_enabled(marker) {
        return None;
    }

    let fov = BXT_FORCE_FOV.as_f32(marker);
    if fov < 1. {
        return None;
    }

    Some(fov)
}
