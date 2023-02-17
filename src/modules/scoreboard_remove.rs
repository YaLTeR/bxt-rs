//! `bxt_scoreboard_remove`

use super::Module;
use crate::hooks::engine;
use crate::modules::cvars::CVar;
use crate::utils::*;

pub struct ScoreboardRemove;
impl Module for ScoreboardRemove {
    fn name(&self) -> &'static str {
        "bxt_scoreboard_remove"
    }

    fn description(&self) -> &'static str {
        "Hiding the scoreboard."
    }

    fn cvars(&self) -> &'static [&'static CVar] {
        static CVARS: &[&CVar] = &[&BXT_SCOREBOARD_REMOVE];
        CVARS
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        engine::Cbuf_AddText.is_set(marker)
            || engine::Cbuf_AddFilteredText.is_set(marker)
            || engine::Cbuf_AddTextToBuffer.is_set(marker)
    }
}

static BXT_SCOREBOARD_REMOVE: CVar = CVar::new(
    b"bxt_scoreboard_remove\0",
    b"0\0",
    "Set to `1` to hide the scoreboard.",
);

/// Returns `true` if scoreboard should currently be disabled.
pub fn is_active(marker: MainThreadMarker) -> bool {
    if !ScoreboardRemove.is_enabled(marker) {
        return false;
    }

    BXT_SCOREBOARD_REMOVE.as_bool(marker)
}

pub unsafe fn strip_showscores(marker: MainThreadMarker, text: *const i8) -> *const i8 {
    if !is_active(marker) {
        return text;
    }

    if text.is_null() {
        return text;
    }

    let mut text_mut: *const u8 = text.cast();

    for bytes in b"+showscores" {
        if *text_mut != *bytes {
            // Character doesn't match, return original text.
            return text;
        }

        text_mut = text_mut.add(1);
    }

    text_mut.cast()
}
