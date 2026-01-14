//! Custom HUD support.

use std::ffi::{CStr, CString};

use glam::{IVec2, IVec4};

use super::{tas_studio, Module};
use crate::hooks::engine::{self, SCREENINFO};
use crate::modules::menu;
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

pub struct MultiHUDLine<'a> {
    marker: MainThreadMarker,
    draw: &'a Draw,
    pos: IVec2,
    default_pos: IVec2,
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

    pub fn multi_line(&'_ self, pos: IVec2) -> MultiLine<'_> {
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

    // String that looks similar to classic HUD menu.
    pub fn hud_string(&'_ self, pos: IVec2, string: &[u8], r: i32, g: i32, b: i32) -> i32 {
        let string = CStr::from_bytes_with_nul(string).unwrap();
        unsafe {
            engine::VGUI2_DrawStringClient.get(self.marker)(pos.x, pos.y, string.as_ptr(), r, g, b)
        }
    }

    pub fn multi_hud_line(&'_ self, pos: IVec2) -> MultiHUDLine<'_> {
        MultiHUDLine {
            marker: self.marker,
            draw: self,
            pos,
            default_pos: pos,
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

impl<'a> MultiHUDLine<'a> {
    pub fn same_line(&mut self, string: &[u8], r: i32, g: i32, b: i32) -> i32 {
        let rv = self.draw.hud_string(self.pos, string, r, g, b);

        self.pos.x += rv;

        rv
    }

    pub fn new_line(&mut self, string: &[u8], r: i32, g: i32, b: i32) -> i32 {
        let rv = self.draw.hud_string(self.pos, string, r, g, b);
        let font_height = 12.max(screen_info(self.marker).iCharHeight);

        self.pos.y += font_height;

        // reset x position because it is good
        self.pos.x = self.default_pos.x;

        rv
    }
}

pub unsafe fn draw_hud(marker: MainThreadMarker) {
    if !Hud.is_enabled(marker) {
        return;
    }

    let draw = Draw { marker };
    tas_studio::draw_hud(marker, &draw);
    menu::draw_custom_menu(marker, &draw);
}
