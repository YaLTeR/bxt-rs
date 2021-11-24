//! `bxt_remove_fade`

use super::Module;
use crate::hooks::engine;
use crate::modules::cvars::{self, CVar};
use crate::utils::*;

pub struct RemoveFade;
impl Module for RemoveFade {
    fn name(&self) -> &'static str {
        "bxt_remove_fade"
    }

    fn cvars(&self) -> &'static [&'static CVar] {
        static CVARS: &[&CVar] = &[&BXT_REMOVE_FADE];
        CVARS
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        engine::V_FadeAlpha.is_set(marker) && cvars::CVars.is_enabled(marker)
    }
}

static BXT_REMOVE_FADE: CVar = CVar::new(b"bxt_remove_fade\0", b"0\0");

/// Returns `true` if fade should currently be removed.
pub fn is_active(marker: MainThreadMarker) -> bool {
    if !RemoveFade.is_enabled(marker) {
        return false;
    }

    BXT_REMOVE_FADE.as_bool(marker)
}
