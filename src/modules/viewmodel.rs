//! `bxt_viewmodel_remove`

use super::Module;
use crate::hooks::engine;
use crate::modules::cvars::CVar;
use crate::utils::*;

pub struct Viewmodel;
impl Module for Viewmodel {
    fn name(&self) -> &'static str {
        "bxt_viewmodel_remove"
    }

    fn description(&self) -> &'static str {
        "Removing the viewmodel."
    }

    fn cvars(&self) -> &'static [&'static CVar] {
        static CVARS: &[&CVar] = &[&BXT_VIEWMODEL_REMOVE];
        CVARS
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        engine::R_DrawViewModel.is_set(marker) && engine::R_PreDrawViewModel.is_set(marker)
    }
}

static BXT_VIEWMODEL_REMOVE: CVar = CVar::new(
    b"bxt_viewmodel_remove\0",
    b"0\0",
    "Set to `1` to disable rendering viewmodel.",
);

pub fn is_removed(marker: MainThreadMarker) -> bool {
    if !Viewmodel.is_enabled(marker) {
        return false;
    }

    BXT_VIEWMODEL_REMOVE.as_bool(marker)
}
