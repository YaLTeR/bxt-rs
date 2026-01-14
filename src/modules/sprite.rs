//! Custom Sprite.

use std::ffi::CString;
use std::str::from_utf8;

use super::Module;
use crate::hooks::engine::{self, client_sprite_s, con_print, rect_s, SCREENINFO};
use crate::modules::hud::{self};
use crate::utils::*;

pub struct Sprite;
impl Module for Sprite {
    fn name(&self) -> &'static str {
        "Custom Sprite"
    }

    fn description(&self) -> &'static str {
        "Loading and managing sprites."
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        // client::HudVidInitFunc.is_set(marker) // TODO: delayed dependency eventually
        engine::ClientDLL_Init.is_set(marker)
            && engine::hudGetScreenInfo.is_set(marker)
            && engine::cl_enginefuncs.is_set(marker)
            && engine::Con_Printf.is_set(marker)
    }
}

type HSpriteHL = i32; // pointer

#[derive(Clone, Copy, Default)]
pub struct SpriteInfo {
    pub pointer: HSpriteHL,
    pub rect: rect_s,
}

impl SpriteInfo {
    pub fn get_dimensions(&self) -> (i32, i32) {
        (
            self.rect.right - self.rect.left,
            self.rect.bottom - self.rect.top,
        )
    }
}

// Sprite list can be cached but the others have to be reloaded every changelevel/disconnect
static SPRITE_LIST: MainThreadCell<Option<(i32, *mut client_sprite_s)>> = MainThreadCell::new(None);

static IS_LOADED: MainThreadCell<bool> = MainThreadCell::new(false);
pub static DIGIT_SPRITES: MainThreadRefCell<Option<Vec<SpriteInfo>>> = MainThreadRefCell::new(None);
pub static DIGIT_SPRITE_SIZE: MainThreadCell<Option<(i32, i32)>> = MainThreadCell::new(None);

pub fn load_sprite(marker: MainThreadMarker) {
    if !Sprite.is_enabled(marker) {
        return;
    }

    if IS_LOADED.get(marker) {
        return;
    }

    // weird circular dependency
    // maybe screeninfo can go to a different module
    let SCREENINFO { iWidth, .. } = hud::screen_info(marker);

    // this makes sure we have correct sprite resolution
    let srite_res = if iWidth < 640 { 320 } else { 640 };

    let Ok(sprite_hud_file) = CString::new("sprites/hud.txt") else {
        con_print(marker, "cannot allocate string\0");
        return;
    };

    let (sprite_count, sprite_list) = SPRITE_LIST.get(marker).unwrap_or({
        let mut sprite_count = 0;

        let sprite_list = unsafe {
            ((&*engine::cl_enginefuncs.get(marker)).pfnSPR_GetList)(
                sprite_hud_file.as_ptr(),
                &mut sprite_count,
            )
        };

        (sprite_count, sprite_list)
    });

    // fill with default value so it is easier to add into it
    let mut digit_sprites: Vec<SpriteInfo> = vec![SpriteInfo::default(); 10];

    for i in 0..sprite_count {
        let curr_sprite = unsafe {
            // add() is not offset()
            // add() will offset by the struct size
            sprite_list.add(i as usize).as_ref()
        };
        let Some(curr_sprite) = curr_sprite else {
            con_print(marker, "cannot access sprite");
            continue;
        };

        if curr_sprite.iRes != srite_res {
            continue;
        }

        // name contains trailing null here
        let Ok(curr_sprite_entity_name) = from_utf8(&curr_sprite.sprite_entity_name) else {
            con_print(marker, "cannot parse sprite entity name");
            continue;
        };
        let Ok(curr_sprite_file_name) = from_utf8(&curr_sprite.sprite_file_name) else {
            con_print(marker, "cannot parse sprite file name");
            continue;
        };

        // remove trailing nulls
        let curr_sprite_entity_name = curr_sprite_entity_name.trim_matches(char::from(0));
        let curr_sprite_file_name = curr_sprite_file_name.trim_matches(char::from(0));

        // "number_" prefix
        if let Some(rest) = curr_sprite_entity_name.strip_prefix("number_") {
            let parse_res = rest.parse::<usize>();

            if let Ok(digit) = parse_res {
                if digit > 10 {
                    continue;
                };

                let spr_path = format!("sprites/{}.spr", curr_sprite_file_name);
                let Ok(spr_path) = CString::new(spr_path) else {
                    con_print(marker, "cannot allocate string");
                    continue;
                };

                let loaded_sprite_ptr = unsafe {
                    ((&*engine::cl_enginefuncs.get(marker)).pfnSPR_Load)(spr_path.as_ptr())
                };
                let new_digit_sprite_entry = SpriteInfo {
                    pointer: loaded_sprite_ptr,
                    rect: curr_sprite.rc,
                };

                // use digit 0 as the base size for monospace
                if digit == 0 {
                    DIGIT_SPRITE_SIZE.set(marker, new_digit_sprite_entry.get_dimensions().into());
                }

                digit_sprites[digit as usize] = new_digit_sprite_entry;
            };
        }
    }

    // all sprites must be set to commit
    if digit_sprites.iter().all(|x| x.pointer != 0) {
        *DIGIT_SPRITES.borrow_mut(marker) = Some(digit_sprites);
    }

    IS_LOADED.set(marker, true);
}

pub fn reset_on_new_map(marker: MainThreadMarker) {
    IS_LOADED.set(marker, false);
    *DIGIT_SPRITES.borrow_mut(marker) = None;
    DIGIT_SPRITE_SIZE.set(marker, None);
}
