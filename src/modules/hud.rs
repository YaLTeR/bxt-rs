//! Custom HUD support.

use std::ffi::{CStr, CString};

use glam::{IVec2, IVec3, IVec4};

use super::{tas_studio, Module};
use crate::hooks::engine::{self, SCREENINFO};
use crate::modules::sprite::DIGIT_SPRITE_SIZE;
use crate::modules::{menu, sprite, timer};
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
            && engine::cl_enginefuncs.is_set(marker)
            && engine::Draw_FillRGBABlend.is_set(marker)
            && engine::Draw_String.is_set(marker)
            && sprite::Sprite.is_enabled(marker)
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

// It is convenient to not managing states
pub struct SpriteDigitLine<'a> {
    _marker: MainThreadMarker,
    draw: &'a Draw,
    pos: IVec2,
    rgb: IVec3,
    digit_width: i32,
    digit_height: i32,
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

    pub fn fill(&self, pos: IVec2, size: IVec2, rgba: IVec4) -> i32 {
        unsafe {
            engine::Draw_FillRGBABlend.get(self.marker)(
                pos.x, pos.y, size.x, size.y, rgba.x, rgba.y, rgba.z, rgba.w,
            );
        }

        size.x
    }

    pub fn fill_no_blend(&self, pos: IVec2, size: IVec2, rgba: IVec4) -> i32 {
        unsafe {
            ((&*engine::cl_enginefuncs.get(self.marker)).pfnFillRGBA)(
                pos.x, pos.y, size.x, size.y, rgba.x, rgba.y, rgba.z, rgba.w,
            );
        }

        size.x
    }

    pub fn bitmap(
        &self,
        pos: impl Into<IVec2>,
        bitmap: &[u8],
        dims: impl Into<IVec2>,
        rgb: impl Into<IVec3>,
    ) -> i32 {
        let pos = pos.into();
        let rgb = rgb.into();
        let dims = dims.into();

        for i in 0..dims.y {
            for j in 0..dims.x {
                self.fill_no_blend(
                    pos + IVec2::new(j, i),
                    (1, 1).into(),
                    IVec4 {
                        x: rgb.x,
                        y: rgb.y,
                        z: rgb.z,
                        w: bitmap[(i * dims.x + j) as usize] as i32,
                    },
                );
            }
        }

        dims.x
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

    pub fn sprite_digit(&self, pos: impl Into<IVec2>, digit: u8, rgb: impl Into<IVec3>) -> i32 {
        if !(0..=9).contains(&digit) {
            return 0;
        }

        let binding = sprite::DIGIT_SPRITES.borrow(self.marker);
        let Some(ref digit_sprites) = *binding else {
            return 0;
        };

        let Some((digit_sprite_width, _)) = DIGIT_SPRITE_SIZE.get(self.marker) else {
            return 0;
        };

        let curr_digit = &digit_sprites[digit as usize];

        if curr_digit.pointer == 0 {
            return 0;
        }

        let rgb = rgb.into();
        let pos = pos.into();

        unsafe {
            ((&*engine::cl_enginefuncs.get(self.marker)).pfnSPR_Set)(
                curr_digit.pointer,
                rgb.x,
                rgb.y,
                rgb.z,
            )
        };
        unsafe {
            ((&*engine::cl_enginefuncs.get(self.marker)).pfnSPR_DrawAdditive)(
                0,
                pos.x,
                pos.y,
                &curr_digit.rect,
            )
        };

        // monospace
        digit_sprite_width
    }

    pub fn sprite_dot(&self, pos: impl Into<IVec2>, rgb: impl Into<IVec3>) -> i32 {
        const DOT_320: &[u8] = &[
            143, 199, 122, // 1
            255, 255, 218, // 2
            120, 169, 95, // 3
        ];

        const DOT_640: &[u8] = &[
            21, 114, 128, 128, 83, 21, // 1
            150, 255, 255, 255, 255, 104, // 2
            239, 255, 255, 255, 255, 192, // 3
            226, 255, 255, 255, 255, 165, // 4
            114, 255, 255, 255, 255, 65, // 5
            29, 43, 89, 89, 29, 29, // 6
        ];

        if SCREEN_INFO.get(self.marker).iWidth < 640 {
            self.bitmap(pos, DOT_320, (3, 3), rgb)
        } else {
            self.bitmap(pos, DOT_640, (6, 6), rgb)
        }
    }

    pub fn sprite_digit_line(
        &'_ self,
        pos: impl Into<IVec2>,
        rgb: impl Into<IVec3>,
    ) -> Option<SpriteDigitLine<'_>> {
        let (width, height) = DIGIT_SPRITE_SIZE.get(self.marker)?;

        SpriteDigitLine {
            _marker: self.marker,
            draw: self,
            pos: pos.into(),
            rgb: rgb.into(),
            digit_width: width,
            digit_height: height,
        }
        .into()
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

impl<'a> SpriteDigitLine<'a> {
    pub fn digit(&mut self, digit: u8) -> &mut Self {
        let rv = self.draw.sprite_digit(self.pos, digit, self.rgb);

        self.pos.x += rv;

        self
    }

    pub fn number_pad_zero(&mut self, number: u32, digit_count: usize) -> &mut Self {
        // can only draw max of 2**32... whatever
        let number_to_digit = |mut number: u32| {
            if number == 0 {
                return vec![0; digit_count];
            }

            let mut digits = Vec::new();

            while number > 0 {
                digits.push((number % 10) as u8);
                number /= 10;
            }

            if digits.len() < digit_count {
                (0..(digit_count - digits.len())).for_each(|_| digits.push(0));
            }

            // drawing left to right...
            digits.reverse();
            digits
        };

        number_to_digit(number).iter().for_each(|&digit| {
            self.digit(digit);
        });

        self
    }

    pub fn decimal_separator(&mut self) -> &mut Self {
        let x_spacing = (self.digit_width - 6) / 2;

        self.pos.x += x_spacing;

        let rv = self.draw.sprite_dot(
            (self.pos.x + 1, self.pos.y + self.digit_height - 5),
            self.rgb,
        );

        self.pos.x += rv + x_spacing;

        self
    }

    pub fn colon(&mut self) -> &mut Self {
        let mut local_pos = self.pos;
        let x_spacing = (self.digit_width - 6) / 2;

        local_pos.x += x_spacing;
        self.draw.sprite_dot(local_pos + IVec2::new(1, 2), self.rgb);

        local_pos.y += self.digit_height - 5;
        let _rv = self
            .draw
            .sprite_dot(local_pos + IVec2::new(1, -2), self.rgb);

        // this code looks wrong but it makes the game look right
        self.pos.x += self.digit_width;

        self
    }
}

pub unsafe fn draw_hud(marker: MainThreadMarker) {
    if !Hud.is_enabled(marker) {
        return;
    }

    let draw = Draw { marker };
    tas_studio::draw_hud(marker, &draw);
    menu::draw_custom_menu(marker, &draw);
    timer::draw_timer(marker, &draw);
}
