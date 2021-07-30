//! `bxt_novis`

use super::Module;
use crate::{
    hooks::engine,
    modules::cvars::{self, CVar},
    utils::*,
};

pub struct NoVis;
impl Module for NoVis {
    fn name(&self) -> &'static str {
        "bxt_novis"
    }

    fn cvars(&self) -> &'static [&'static CVar] {
        static CVARS: &[&CVar] = &[&BXT_NOVIS];
        CVARS
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        engine::Mod_LeafPVS.is_set(marker) && cvars::CVars.is_enabled(marker)
    }
}

static BXT_NOVIS: CVar = CVar::new(b"bxt_novis\0", b"0\0");

/// Returns `true` if vis should currently be skipped for entities.
pub fn is_active(marker: MainThreadMarker) -> bool {
    if !NoVis.is_enabled(marker) {
        return false;
    }

    BXT_NOVIS.as_bool(marker)
}
