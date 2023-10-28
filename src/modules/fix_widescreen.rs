//! `bxt_fix_widescreen_fov`

use super::Module;
use crate::hooks::engine;
use crate::modules::cvars::CVar;
use crate::utils::*;

pub struct FixWidescreen;
impl Module for FixWidescreen {
    fn name(&self) -> &'static str {
        "bxt_fix_widescreen_fov"
    }

    fn description(&self) -> &'static str {
        "Correcting widescreen vertical field-of-view."
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        engine::R_SetFrustum.is_set(marker)
            && engine::scr_fov_value.is_set(marker)
            && engine::VideoMode_IsWindowed.is_set(marker)
            && engine::VideoMode_GetCurrentVideoMode.is_set(marker)
            && engine::window_rect.is_set(marker)
    }

    fn cvars(&self) -> &'static [&'static CVar] {
        static CVARS: &[&CVar] = &[&BXT_FIX_WIDESCREEN_FOV];
        CVARS
    }
}

static BXT_FIX_WIDESCREEN_FOV: CVar = CVar::new(
    b"bxt_fix_widescreen_fov\0",
    b"0\0",
    "\
Fixes reduction in vertical field-of-view of widescreen. Compatible with `bxt_force_fov.`",
);

// My guess is that every render cycle, `scr_fov_value` is reset to some values.
// So even if we override it, at some points, it is back to something else entirely different.
// But that is not the case for loading. If we don't do this, we will doubly process the value.
//
// YaLTeR: It's set in the client DLL in HUD_something which runs every frame but not during loads.
static PREV_FOV: MainThreadRefCell<f32> = MainThreadRefCell::new(0.);

pub fn fix_widescreen_fov(marker: MainThreadMarker) {
    if !FixWidescreen.is_enabled(marker) {
        return;
    }

    if !BXT_FIX_WIDESCREEN_FOV.as_bool(marker) {
        return;
    }

    let fov = unsafe { *engine::scr_fov_value.get(marker) };
    let prev_fov = *PREV_FOV.borrow(marker);

    if fov != prev_fov {
        let (width, height) = unsafe { engine::get_resolution(marker) };
        let current_aspect_ratio = width as f32 / height as f32;
        let default_aspect_ratio = 3f32 / 4f32;

        let new_fov =
            (((fov.to_radians() / 2.).tan() * default_aspect_ratio * current_aspect_ratio)
                .atan()
                .to_degrees()
                * 2.)
                .clamp(10f32, 150f32);

        unsafe { *engine::scr_fov_value.get(marker) = new_fov };

        *PREV_FOV.borrow_mut(marker) = new_fov;
    }
}
