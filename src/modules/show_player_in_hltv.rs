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
When record a demo with camera path, it is desirable for the demo to be in third person. \
To enter third person in a demo, there are two options.

`playdemo` and `thirdperson`. If there is campath loaded, this is the most straightforward way to record. \
The player model will always show up. But just like any `playdemo`, you can only play a demo.

`viewdemo` and `dem_forcehltv 1`. There are more functionalities available with this method such as pausing, fastforwarding, and rewinding. \
However, the player model does not consistently show up.

This fix is for `viewdemo` and `dem_forcehltv 1` route. This fix forces the player model to show up."
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
