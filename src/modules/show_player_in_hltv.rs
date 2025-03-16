//! Show player model in HLTV spectate

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
When recording a demo with a custom camera path, you frequently want to see the player in third person. \
There are two ways to do this.

- `playdemo` and `thirdperson`. If a campath is loaded, this is the most straightforward way to record. \
The player model will always show up.

- `viewdemo` and `dem_forcehltv 1`. This gives you more functionality such as pausing, fast-forwarding, and rewinding. \
However, the player model is not consistently visible.

This module makes the player model consistently visible in `viewdemo` with `dem_forcehltv 1`."
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        // BXT has the same functionality.
        !bxt::BXT_TAS_LOAD_SCRIPT_FROM_STRING.is_set(marker)
            && engine::cls_demos.is_set(marker)
            && engine::ClientDLL_IsThirdPerson.is_set(marker)
            && engine::CL_IsSpectateOnly.is_set(marker)
            && engine::pmove.is_set(marker)
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
