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
        "Erasing `+showscores` in demo message"
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
    "Set to `1` to hide scoreboard in demo.",
);

/// Returns `true` if scoreboard should currently be disabled.
pub fn is_active(marker: MainThreadMarker) -> bool {
    if !ScoreboardRemove.is_enabled(marker) {
        return false;
    }

    BXT_SCOREBOARD_REMOVE.as_bool(marker)
}

pub unsafe fn on_cbuf_stuffs(marker: MainThreadMarker, text: *const i8) -> *const i8 {
    if !is_active(marker) {
        return text;
    }

    if text.is_null() {
        return text;
    }

    let mut text_mut: *const u8 = text.cast();

    // Simple comparisons before committing
    if *text_mut == b'+' && *(text_mut.offset(1)) == b's' && *(text_mut.offset(2)) == b'h' {
        let a = "owscores";
        text_mut = text_mut.offset(3);

        for byte in a.bytes() {
            if byte == *text_mut {
                text_mut = text_mut.offset(1);
            } else {
                return text.cast();
            }
        }
    }

    text_mut.cast()
}
