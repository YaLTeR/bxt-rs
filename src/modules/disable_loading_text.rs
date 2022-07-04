//! `bxt_disable_loading_text`

use super::Module;
use crate::hooks::engine;
use crate::modules::cvars::{self, CVar};
use crate::utils::*;

pub struct DisableLoadingText;
impl Module for DisableLoadingText {
    fn name(&self) -> &'static str {
        "bxt_disable_loading_text"
    }

    fn description(&self) -> &'static str {
        "Disabling the LOADING text."
    }

    fn cvars(&self) -> &'static [&'static CVar] {
        static CVARS: &[&CVar] = &[&BXT_DISABLE_LOADING_TEXT];
        CVARS
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        engine::SCR_DrawLoading.is_set(marker) && cvars::CVars.is_enabled(marker)
    }
}

static BXT_DISABLE_LOADING_TEXT: CVar = CVar::new(b"bxt_disable_loading_text\0", b"0\0");

/// Returns `true` if loading text should currently be disabled.
pub fn is_active(marker: MainThreadMarker) -> bool {
    if !DisableLoadingText.is_enabled(marker) {
        return false;
    }

    BXT_DISABLE_LOADING_TEXT.as_bool(marker)
}
