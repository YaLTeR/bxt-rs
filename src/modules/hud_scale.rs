//! `bxt_hud_scale`

use super::Module;
use crate::gl;
use crate::hooks::engine::{self, SCREENINFO};
use crate::modules::cvars::{self, CVar};
use crate::utils::*;

pub struct HudScale;
impl Module for HudScale {
    fn name(&self) -> &'static str {
        "bxt_hud_scale"
    }

    fn cvars(&self) -> &'static [&'static CVar] {
        static CVARS: &[&CVar] = &[&BXT_HUD_SCALE];
        CVARS
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        gl::GL.borrow(marker).is_some()
            && engine::hudGetScreenInfo.is_set(marker)
            && engine::ClientDLL_HudVidInit.is_set(marker)
            && engine::ClientDLL_UpdateClientData.is_set(marker)
            && engine::ClientDLL_HudRedraw.is_set(marker)
            && engine::VideoMode_GetCurrentVideoMode.is_set(marker)
            && engine::VideoMode_IsWindowed.is_set(marker)
            && engine::window_rect.is_set(marker)
            && cvars::CVars.is_enabled(marker)
    }
}

static SCALE_SCREEN_INFO: MainThreadCell<bool> = MainThreadCell::new(false);

static BXT_HUD_SCALE: CVar = CVar::new(b"bxt_hud_scale\0", b"0\0");

/// Returns the HUD scale, if any.
pub fn scale(marker: MainThreadMarker) -> Option<f32> {
    if !HudScale.is_enabled(marker) {
        return None;
    }

    let scale = BXT_HUD_SCALE.as_f32(marker);
    if scale == 0. {
        return None;
    }

    Some(scale)
}

/// Runs `f` with the GL projection matrix set to a scaled HUD orthographic matrix.
pub unsafe fn with_scaled_projection_matrix<T>(
    marker: MainThreadMarker,
    f: impl FnOnce() -> T,
) -> T {
    let scale = match scale(marker) {
        Some(scale) => scale,
        None => {
            return f();
        }
    };

    {
        let (width, height) = engine::get_resolution(marker);

        let gl = crate::gl::GL.borrow(marker);
        let gl = gl.as_ref().unwrap();

        gl.MatrixMode(crate::gl::PROJECTION);
        gl.PushMatrix(); // Save the existing matrix.
        gl.LoadIdentity();
        gl.Ortho(
            0.,
            (width as f32 / scale).into(),
            (height as f32 / scale).into(),
            0.,
            -99999.,
            99999.,
        );
    }

    let rv = f();

    {
        let gl = crate::gl::GL.borrow(marker);
        let gl = gl.as_ref().unwrap();

        gl.MatrixMode(crate::gl::PROJECTION);
        gl.PopMatrix(); // Load the matrix we saved above.
    }

    rv
}

pub fn with_scaled_screen_info<T>(marker: MainThreadMarker, f: impl FnOnce() -> T) -> T {
    if SCALE_SCREEN_INFO.get(marker) {
        // Recursive invocation; don't want to set to false in the end.
        return f();
    }

    SCALE_SCREEN_INFO.set(marker, true);

    let rv = f();

    SCALE_SCREEN_INFO.set(marker, false);

    rv
}

pub fn maybe_scale_screen_info(marker: MainThreadMarker, screen_info: &mut SCREENINFO) {
    if !SCALE_SCREEN_INFO.get(marker) {
        return;
    }

    let scale = scale(marker).unwrap_or(1.);
    screen_info.iWidth = (screen_info.iWidth as f32 / scale) as i32;
    screen_info.iHeight = (screen_info.iHeight as f32 / scale) as i32;
}
