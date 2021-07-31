//! `bxt_wallhack`

use super::Module;
use crate::gl;
use crate::hooks::engine;
use crate::modules::cvars::{self, CVar};
use crate::utils::*;

pub struct Wallhack;
impl Module for Wallhack {
    fn name(&self) -> &'static str {
        "bxt_wallhack"
    }
    fn cvars(&self) -> &'static [&'static CVar] {
        static CVARS: &[&CVar] = &[&BXT_WALLHACK, &BXT_WALLHACK_ADDITIVE, &BXT_WALLHACK_ALPHA];
        &CVARS
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        engine::R_DrawSequentialPoly.is_set(marker) && cvars::CVars.is_enabled(marker)
    }
}

static BXT_WALLHACK: CVar = CVar::new(b"bxt_wallhack\0", b"0\0");
static BXT_WALLHACK_ADDITIVE: CVar = CVar::new(b"bxt_wallhack_additive\0", b"0\0");
static BXT_WALLHACK_ALPHA: CVar = CVar::new(b"bxt_wallhack_alpha\0", b"0.6\0");

/// Returns `true` if wallhack is enabled
pub fn is_active(marker: MainThreadMarker) -> bool {
    if !Wallhack.is_enabled(marker) {
        return false;
    }

    BXT_WALLHACK.as_bool(marker)
}

pub fn with_wallhack<T>(marker: MainThreadMarker, f: impl FnOnce() -> T) -> T {
    let gl = crate::gl::GL.borrow(marker);
    let gl = gl.as_ref().unwrap();
    unsafe {
        gl.Enable(gl::BLEND);
        gl.DepthMask(gl::FALSE);

        if BXT_WALLHACK_ADDITIVE.as_bool(marker) {
            gl.BlendFunc(gl::SRC_ALPHA, gl::ONE)
        } else {
            gl.BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA)
        }

        gl.Color4f(1.0f32, 1.0f32, 1.0f32, BXT_WALLHACK_ALPHA.as_f32(marker));
    }

    let rv = f();

    unsafe {
        gl.Disable(gl::BLEND);
        gl.DepthMask(gl::TRUE);
    }

    rv
}

pub fn with_after_wallhack<T>(marker: MainThreadMarker, f: impl FnOnce() -> T) -> T {
    let gl = crate::gl::GL.borrow(marker);
    let gl = gl.as_ref().unwrap();
    unsafe {
        gl.ClearColor(0.0f32, 0.0f32, 0.0f32, 1.0f32);
        gl.Clear(gl::COLOR_BUFFER_BIT);
    }

    let rv = f();

    rv
}
