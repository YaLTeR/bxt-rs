//! `bxt_skybox_remove`

use super::Module;
use crate::gl;
use crate::hooks::engine;
use crate::modules::cvars::{self, CVar};
use crate::utils::*;

pub struct SkyboxRemove;
impl Module for SkyboxRemove {
    fn name(&self) -> &'static str {
        "bxt_skybox_remove"
    }

    fn description(&self) -> &'static str {
        "Disabling the skybox drawing."
    }

    fn cvars(&self) -> &'static [&'static CVar] {
        static CVARS: &[&CVar] = &[&BXT_SKYBOX_REMOVE];
        CVARS
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        gl::GL.borrow(marker).is_some()
            && engine::R_DrawSkyBox.is_set(marker)
            && cvars::CVars.is_enabled(marker)
    }
}

static BXT_SKYBOX_REMOVE: CVar = CVar::new(b"bxt_skybox_remove\0", b"0\0");

/// Returns `true` if skybox should currently be disabled.
pub fn is_active(marker: MainThreadMarker) -> bool {
    if !SkyboxRemove.is_enabled(marker) {
        return false;
    }

    BXT_SKYBOX_REMOVE.as_bool(marker)
}

pub fn on_r_clear(marker: MainThreadMarker) {
    if !is_active(marker) {
        return;
    }

    let gl = crate::gl::GL.borrow(marker);
    let gl = gl.as_ref().unwrap();

    unsafe {
        gl.ClearColor(0., 0., 0., 1.);
        gl.Clear(gl::COLOR_BUFFER_BIT);
    }
}
