//! Custom HUD support.

use std::ffi::{CStr, CString};

use glam::{IVec2, IVec4};

use super::{tas_studio, Module};
use crate::hooks::engine::{self, SCREENINFO};
use crate::utils::*;

pub struct Hud;
impl Module for Hud {
    fn name(&self) -> &'static str {
        "Custom HUD"
    }

    fn description(&self) -> &'static str {
        "Drawing custom HUD elements."
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        engine::hudGetScreenInfo.is_set(marker)
            // TODO: Add back when delayed dependencies are implemented.
            // && client::HudRedrawFunc.is_set(marker)
            && engine::Draw_FillRGBABlend.is_set(marker)
            && engine::Draw_String.is_set(marker)
    }
}

static SCREEN_INFO: MainThreadCell<SCREENINFO> = MainThreadCell::new(SCREENINFO::zeroed());

pub fn update_screen_info(marker: MainThreadMarker, info: SCREENINFO) {
    SCREEN_INFO.set(marker, info);
}

pub fn screen_info(marker: MainThreadMarker) -> SCREENINFO {
    SCREEN_INFO.get(marker)
}

pub struct Draw {
    marker: MainThreadMarker,
}

pub struct MultiLine<'a> {
    marker: MainThreadMarker,
    draw: &'a Draw,
    pos: IVec2,
}

impl Draw {
    pub fn string(&self, pos: IVec2, string: &[u8]) -> i32 {
        let string = CStr::from_bytes_with_nul(string).unwrap();
        unsafe { engine::Draw_String.get(self.marker)(pos.x, pos.y, string.as_ptr()) }
    }

    pub fn string_owned(&self, pos: IVec2, string: impl Into<Vec<u8>>) -> i32 {
        let string = CString::new(string).unwrap();
        unsafe { engine::Draw_String.get(self.marker)(pos.x, pos.y, string.as_ptr()) }
    }

    pub fn multi_line(&self, pos: IVec2) -> MultiLine {
        MultiLine {
            marker: self.marker,
            draw: self,
            pos,
        }
    }

    pub fn fill(&self, pos: IVec2, size: IVec2, rgba: IVec4) {
        unsafe {
            engine::Draw_FillRGBABlend.get(self.marker)(
                pos.x, pos.y, size.x, size.y, rgba.x, rgba.y, rgba.z, rgba.w,
            );
        }
    }
}

impl<'a> MultiLine<'a> {
    pub fn line(&mut self, string: &[u8]) -> i32 {
        let rv = self.draw.string(self.pos, string);
        self.pos.y += screen_info(self.marker).iCharHeight;
        rv
    }

    pub fn line_owned(&mut self, string: impl Into<Vec<u8>>) -> i32 {
        let rv = self.draw.string_owned(self.pos, string);
        self.pos.y += screen_info(self.marker).iCharHeight;
        rv
    }
}

pub unsafe fn draw_hud(marker: MainThreadMarker) {
    if !Hud.is_enabled(marker) {
        return;
    }

    let draw = Draw { marker };
    tas_studio::draw_hud(marker, &draw);
}
