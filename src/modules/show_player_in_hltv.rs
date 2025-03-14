//! `Show player model in HLTV spectate`

use super::Module;
use crate::hooks::{bxt, engine};
use crate::utils::*;

pub struct ShowPlayerInHltv;
impl Module for ShowPlayerInHltv {
    fn name(&self) -> &'static str {
        "Show player model in HLTV spectate"
    }

    fn description(&self) -> &'static str {
        "\
When people do cam path, they usually want to see the player in the demo. This means the demo will be recorded in third person. \
The only way to enter third person and free camera is to do `dem_forcehltv 1` and then use `viewdemo` command. \
For a brief moment in free spectate in `viewdemo`, the player model will show up. \
However, after cycling through all spectating options once, the player model disappears.

This is the fix for that.

Alternatively, with this fix, it is possible to use `playdemo` with `thirdperson` to have the player model show up to just record. \
Free spectate is not available in `playdemo` nonetheless."
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        // BXT has the same functionality.
        !bxt::BXT_TAS_LOAD_SCRIPT_FROM_STRING.is_set(marker)
            && engine::cls_demos.is_set(marker)
            && engine::ClientDLL_IsThirdPerson.is_set(marker)
            && engine::CL_IsSpectateOnly.is_set(marker)
            && engine::pmove.is_set(marker)
            && unsafe { engine::find_cmd(marker, "dem_forcehltv") }.is_some()
    }
}

pub fn should_force_emit_player_entity(marker: MainThreadMarker) -> bool {
    if !ShowPlayerInHltv.is_enabled(marker) {
        return false;
    }

    let is_playingback = unsafe { &*engine::cls_demos.get(marker) }.demoplayback != 0;
    let is_spectate = unsafe { engine::CL_IsSpectateOnly.get(marker)() } != 0;
    let is_iuser1 = unsafe { &**engine::pmove.get(marker) }.iuser1 != 4;

    is_playingback && is_spectate && is_iuser1
}
