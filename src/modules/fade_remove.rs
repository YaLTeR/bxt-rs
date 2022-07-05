//! `bxt_fade_remove`

use super::Module;
use crate::hooks::engine;
use crate::modules::cvars::{self, CVar};
use crate::utils::*;

pub struct FadeRemove;
impl Module for FadeRemove {
    fn name(&self) -> &'static str {
        "bxt_fade_remove"
    }

    fn description(&self) -> &'static str {
        "Removing the screen blackout effect."
    }

    fn cvars(&self) -> &'static [&'static CVar] {
        static CVARS: &[&CVar] = &[&BXT_FADE_REMOVE];
        CVARS
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        engine::V_FadeAlpha.is_set(marker) && cvars::CVars.is_enabled(marker)
    }
}

static BXT_FADE_REMOVE: CVar = CVar::new(
    b"bxt_fade_remove\0",
    b"0\0",
    "Set to `1` to disable the screen blackout effect.",
);

/// Returns `true` if fade should currently be removed.
pub fn is_active(marker: MainThreadMarker) -> bool {
    if !FadeRemove.is_enabled(marker) {
        return false;
    }

    BXT_FADE_REMOVE.as_bool(marker)
}
