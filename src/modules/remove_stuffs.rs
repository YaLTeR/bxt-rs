//! `Removing visible stuffs from the game`

use super::Module;
use crate::gl;
use crate::hooks::engine;
use crate::modules::cvars::{self, CVar};
use crate::utils::*;

pub struct RemoveStuffs;

impl Module for RemoveStuffs {
    fn name(&self) -> &'static str {
        "Removing visible stuffs from the game"
    }

    fn description(&self) -> &'static str {
        "This helps with gameplay and video recording.

`BXT_REMOVE_MULTITEXTURE=1` in environment variable to disable multitexture support. This gives FPS boost on many maps. \
The only downside is there will be no detailed texture functionalities (`r_detailtextures 0`)."
    }

    fn cvars(&self) -> &'static [&'static CVar] {
        static CVARS: &[&CVar] = &[
            &BXT_REMOVE_FADE,
            &BXT_REMOVE_SHAKE,
            &BXT_REMOVE_LOADING_TEXT,
            &BXT_REMOVE_SCOREBOARD,
            &BXT_REMOVE_SKYBOX,
            &BXT_REMOVE_VIEWMODEL,
            &BXT_REMOVE_WORLD,
            &BXT_REMOVE_ENTITY,
            &BXT_REMOVE_ENTITY_BRUSH,
            &BXT_REMOVE_ENTITY_MODEL,
            &BXT_REMOVE_ENTITY_SPRITE,
            &BXT_REMOVE_HUD,
            &BXT_REMOVE_CROSSHAIR,
        ];
        CVARS
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        cvars::CVars.is_enabled(marker)
            && engine::V_FadeAlpha.is_set(marker)
            && engine::V_ApplyShake.is_set(marker)
            && engine::SCR_DrawLoading.is_set(marker)
            && (engine::Cbuf_AddText.is_set(marker)
                || engine::Cbuf_AddFilteredText.is_set(marker)
                || engine::Cbuf_AddTextToBuffer.is_set(marker))
            && gl::GL.borrow(marker).is_some()
            && engine::R_DrawSkyBox.is_set(marker)
            && engine::R_DrawViewModel.is_set(marker)
            && engine::R_PreDrawViewModel.is_set(marker)
            && engine::ClientDLL_AddEntity.is_set(marker)
            && engine::ClientDLL_HudRedraw.is_set(marker)
            && engine::CheckMultiTextureExtensions.is_set(marker)
            && engine::DrawCrosshair.is_set(marker)
    }
}

macro_rules! generate_remove_stuff {
    ($static_name:ident, $cvar_name:literal, $description:literal, $function_name:ident) => {
        static $static_name: CVar =
            CVar::new(concat!($cvar_name, "\0").as_bytes(), b"0\0", $description);

        pub fn $function_name(marker: MainThreadMarker) -> bool {
            RemoveStuffs.is_enabled(marker) && $static_name.as_bool(marker)
        }
    };
}

generate_remove_stuff!(
    BXT_REMOVE_FADE,
    "bxt_remove_fade",
    "Set to `1` to disable the screen blackout effect.",
    should_remove_fade
);

generate_remove_stuff!(
    BXT_REMOVE_SHAKE,
    "bxt_remove_shake",
    "Set to `1` to disable the screen shake effect.",
    should_remove_shake
);

generate_remove_stuff!(
    BXT_REMOVE_LOADING_TEXT,
    "bxt_remove_loading_text",
    "Set to `1` to disable the LOADING text.",
    should_remove_loading_text
);

generate_remove_stuff!(
    BXT_REMOVE_SKYBOX,
    "bxt_remove_skybox",
    "Set to `1` to disable the skybox.",
    should_remove_skybox
);

generate_remove_stuff!(
    BXT_REMOVE_VIEWMODEL,
    "bxt_remove_viewmodel",
    "Set to `1` to disable rendering viewmodel.",
    should_remove_viewmodel
);

generate_remove_stuff!(
    BXT_REMOVE_WORLD,
    "bxt_remove_world",
    "Set to `1` to disable rendering world brushes.",
    should_remove_world
);

generate_remove_stuff!(
    BXT_REMOVE_ENTITY,
    "bxt_remove_entity",
    "Set to `1` to disable rendering all entities.",
    should_remove_entity
);

generate_remove_stuff!(
    BXT_REMOVE_ENTITY_BRUSH,
    "bxt_remove_entity_brush",
    "Set to `1` to disable rendering entity brushes.",
    should_remove_entity_brush
);

generate_remove_stuff!(
    BXT_REMOVE_ENTITY_MODEL,
    "bxt_remove_entity_model",
    "Set to `1` to disable rendering entity models.",
    should_remove_entity_model
);

generate_remove_stuff!(
    BXT_REMOVE_ENTITY_SPRITE,
    "bxt_remove_entity_sprite",
    "Set to `1` to disable rendering entity sprites.",
    should_remove_entity_sprite
);

generate_remove_stuff!(
    BXT_REMOVE_HUD,
    "bxt_remove_hud",
    "Set to `1` to disable rendering HUDs.",
    should_remove_hud
);

generate_remove_stuff!(
    BXT_REMOVE_CROSSHAIR,
    "bxt_remove_crosshair",
    "Set to `1` to disable rendering sprite crosshair.",
    should_remove_crosshair
);

static BXT_REMOVE_SCOREBOARD: CVar = CVar::new(
    b"bxt_remove_scoreboard\0",
    b"0\0",
    "Set to `1` to hide the scoreboard.",
);

pub fn should_remove_multitexture_support() -> bool {
    std::env::var_os("BXT_REMOVE_MULTITEXTURE").is_some()
}

pub unsafe fn maybe_strip_showscores(marker: MainThreadMarker, text: *const i8) -> *const i8 {
    if !RemoveStuffs.is_enabled(marker) {
        return text;
    }

    if !BXT_REMOVE_SCOREBOARD.as_bool(marker) {
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
