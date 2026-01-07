//! `bxt_esp`

use std::ffi::CStr;

use once_cell::sync::Lazy;

use super::Module;
use crate::ffi::r_efx::cl_entity_s;
use crate::gl;
use crate::hooks::engine;
use crate::modules::cvars::{self, CVar};
use crate::utils::*;

pub struct WallhackEsp;
impl Module for WallhackEsp {
    fn name(&self) -> &'static str {
        "bxt_esp"
    }

    fn description(&self) -> &'static str {
        "Seeing players through walls."
    }

    fn cvars(&self) -> &'static [&'static CVar] {
        static CVARS: &[&CVar] = &[
            &BXT_ESP_PLAYER,
            &BXT_ESP_PLAYER_OUTLINE,
            &BXT_ESP_PLAYER_OUTLINE_COLORMODE,
        ];
        CVARS
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        gl::GL.borrow(marker).is_some()
            && cvars::CVars.is_enabled(marker)
            && engine::R_StudioDrawPoints.is_set(marker)
            && engine::currententity.is_set(marker)
    }
}

static BXT_ESP_PLAYER: CVar = CVar::new(
    b"bxt_esp_player\0",
    b"0\0",
    "Whether to see player through wall.",
);

static BXT_ESP_PLAYER_OUTLINE: CVar = CVar::new(
    b"bxt_esp_player_outline\0",
    b"1\0",
    "Whether to see player outline. Needs `bxt_esp_player` enabled.",
);

static BXT_ESP_PLAYER_OUTLINE_COLORMODE: CVar = CVar::new(
    b"bxt_esp_player_outline_colormode\0",
    b"0\0",
    "\
0: Set by team. Blue = CT. Red = T.
1: White outline.
Other values: Random color where input is the seed.
",
);

type Color = [f32; 3];
const fn dim_light(color: &Color) -> Color {
    [color[0] * 0.50, color[1] * 0.50, color[2] * 0.50]
}

// figma colors
const ROYAL_BLUE: Color = [0.188, 0.361, 0.871];
const STRAWBERRY: Color = [0.98, 0.314, 0.325];
const SNOW: Color = [1., 0.98, 0.98];
const LIME_GREEN: Color = [0.537, 0.953, 0.212];
const AQUA: Color = [0., 1., 0.941];
const NEON_ORANGE: Color = [1., 0.361, 0.];
const LEMON: Color = [1., 0.969, 0.];
const ROSE_GOLD: Color = [0.871, 0.631, 0.576];
const NEON_BLUE: Color = [0.137, 0.137, 1.];
const JAY_GREEN: Color = [0., 0.733, 0.467];
const ORCHID: Color = [0.929, 0.502, 0.914];
const CREAM: Color = [0.992, 0.984, 0.831];
const SIENNA: Color = [0.533, 0.176, 0.09];
const LIGHT_PINK: Color = [1., 0.71, 0.753];
const PLATINUM: Color = [0.851, 0.851, 0.851];
const HOT_MAGENTA: Color = [1., 0.114, 0.808];

// append color and do not change the first 3 colors
const COLORS: Lazy<Vec<Color>> = Lazy::new(|| {
    vec![
        ROYAL_BLUE,
        STRAWBERRY,
        SNOW,
        LIME_GREEN,
        AQUA,
        NEON_ORANGE,
        LEMON,
        ROSE_GOLD,
        NEON_BLUE,
        JAY_GREEN,
        ORCHID,
        CREAM,
        SIENNA,
        LIGHT_PINK,
        PLATINUM,
        HOT_MAGENTA,
    ]
});
const COLORS_DIMMED: Lazy<Vec<Color>> = Lazy::new(|| COLORS.iter().map(dim_light).collect());

fn is_ct(current_entity: &cl_entity_s) -> bool {
    let s = unsafe { CStr::from_ptr(current_entity.model as *mut i8) };
    let ct = &["gign", "gsg9", "sas", "urban", "vip"];
    let model_name_str = s.to_string_lossy();

    ct.iter().any(|x| model_name_str.contains(x))
}

fn random(nth: usize, len: usize, seed: usize) -> usize {
    // length should be even so we have a more interesting rng
    let multiplier = seed.wrapping_mul(2).wrapping_add(1);

    multiplier.wrapping_mul(nth).wrapping_add(seed) % len
}

fn select_color(marker: MainThreadMarker, current_entity: &cl_entity_s) -> [Color; 2] {
    let option = BXT_ESP_PLAYER_OUTLINE_COLORMODE.as_f32(marker) as usize;

    match option {
        0 => {
            if is_ct(current_entity) {
                [COLORS[0], COLORS_DIMMED[0]]
            } else {
                [COLORS[1], COLORS_DIMMED[1]]
            }
        }
        1 => [COLORS[2], COLORS_DIMMED[2]],
        x => {
            // sub 1 because entity 0 is worldspawn and player starts at 1
            let index = random(current_entity.index as usize - 1, COLORS.len(), x);
            [COLORS[index], COLORS_DIMMED[index]]
        }
    }
}

// Note to self: hooking R_LightLambert and R_StudioDrawPlayer doesn't work.
// On Linux, there are two functions with the same signature for some reasons.
// The hooked function is never called.
pub fn with_wallhack_esp_player<T>(marker: MainThreadMarker, f: impl Fn() -> T) -> T {
    if !WallhackEsp.is_enabled(marker) {
        return f();
    }

    if !(BXT_ESP_PLAYER.as_bool(marker)) {
        return f();
    }

    // player = 0 means not a player meaning this is a normal model
    let current_entity = unsafe { &mut **engine::currententity.get(marker) };
    let player_value = current_entity.player;
    let is_player = player_value != 0;

    let gl = crate::gl::GL.borrow(marker);
    let gl = gl.as_ref().unwrap();

    if !is_player {
        // player model will be drawn over viewmodel
        // viewmodel is drawn after player model
        // so here, we make sure that viewmodel is drawn over player

        // nice hack, model name [u8; 64] is the first memeber of the struct
        // so the model struct can be interpreted as a c string
        let s = unsafe { CStr::from_ptr(current_entity.model as *mut i8) };

        if !s.to_string_lossy().contains("v_") {
            return f();
        }

        unsafe {
            gl.DepthRange(0., 0.005);
            let rv = f();
            gl.DepthRange(0., 1.);

            return rv;
        }
    }

    let [color_bright, color_dimmed] = select_color(marker, current_entity);

    // outline drawing
    if BXT_ESP_PLAYER_OUTLINE.as_bool(marker) {
        // color mix is the actual color for outline texture
        let r_colormix = unsafe { &mut *engine::r_colormix.get(marker) };
        // setting ambientlight to 255 so the lines will be fullbright
        // again, the reason is that we are using the model drawing function to draw outline
        // this makes the drawing a lot easier but we are dependant on the code inside the drawing
        // function.
        // ambient light does affect 1. outline color 2. outline intensity
        // colormix fixes color
        // ambient fixes intensity
        let r_ambientlight = unsafe { &mut *engine::r_ambientlight.get(marker) };

        let old_r_colormix = *r_colormix;
        let old_r_ambientlight = *r_ambientlight;

        unsafe {
            // disables all textures and use our own color
            gl.Disable(gl::TEXTURE_2D);

            // draw as line, cool trick
            gl.PolygonMode(gl::FRONT_AND_BACK, gl::LINE);

            gl.LineWidth(2.);
        }

        // make intensity 255
        *r_ambientlight = 255;

        // pass 1: player outline when player is invisible
        // outline is bright
        *r_colormix = color_bright;

        unsafe {
            gl.Enable(gl::DEPTH_TEST);
            // compare within that depth range
            // need this comparison so that dimmed outline doesnt draw over another player model
            gl.DepthFunc(gl::LESS);
            // don't write to mask so that the second pass has correct info
            gl.DepthMask(gl::FALSE);

            // this draws through wall so the outline should always be visible
            gl.DepthRange(0., 0.02);
        }

        f();

        // pass 2: player outline when player is visible
        // the outline is dimmed
        *r_colormix = color_dimmed;

        unsafe {
            // norm
            gl.DepthFunc(gl::LESS);
            gl.DepthMask(gl::TRUE);

            // uses normal depth range
            gl.DepthRange(0., 1.);
            f();
        }

        // restore settings
        *r_ambientlight = old_r_ambientlight;
        *r_colormix = old_r_colormix;
    }

    // player drawing
    // drawing player fragments
    unsafe {
        // enable texture again to draw correctly
        gl.Enable(gl::TEXTURE_2D);
        gl.DepthFunc(gl::LESS);
        gl.DepthMask(gl::TRUE);

        gl.DepthRange(0., 0.01);
        gl.PolygonMode(gl::FRONT_AND_BACK, gl::FILL);
    }
    let rv = f();

    // clean up
    unsafe {
        gl.DepthRange(0., 1.);
        gl.LineWidth(1.);
    }

    rv
}
