//! `Custom Menu`

use std::sync::Arc;

use super::Module;
use crate::hooks::engine::{self, prepend_command, SCREENINFO};
use crate::modules::cvars::{CVar, CVars};
use crate::modules::hud::{self, MultiHUDLine};
use crate::utils::*;

pub struct Menu;
impl Module for Menu {
    fn name(&self) -> &'static str {
        "Custom Menu"
    }

    fn description(&self) -> &'static str {
        "Drawing custom HUD menu."
    }

    fn cvars(&self) -> &'static [&'static CVar] {
        static CVARS: &[&CVar] = &[&BXT_MENU_ANCHOR];
        CVARS
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        engine::VGUI2_DrawStringClient.is_set(marker)
            && CVars.is_enabled(marker)
            && hud::Hud.is_enabled(marker)
            && engine::Cbuf_InsertText.is_set(marker) // prepend_command
    }
}

static BXT_MENU_ANCHOR: CVar = CVar::new(
    b"_bxt_menu_anchor\0",
    b"20 x\0",
    "\
Location of custom menu in (x, y).

If the second value (y) is a literal \"x\", it will automatically be decided by bxt-rs.
",
);

fn get_anchor_location(marker: MainThreadMarker) -> (i32, i32) {
    let value = BXT_MENU_ANCHOR.to_string(marker);

    let values = value.split_ascii_whitespace().collect::<Vec<&str>>();

    const DEFAULT_X: i32 = 20;
    let get_default_y = || calculate_menu_y_pos(marker, LINE_COUNT);

    if values.len() != 2 {
        return (DEFAULT_X, get_default_y());
    } else {
        let x = values[0].parse::<i32>().unwrap_or(DEFAULT_X);

        let y = if values[1] == "x" {
            get_default_y()
        } else {
            values[1].parse::<i32>().unwrap_or(get_default_y())
        };

        return (x, y);
    }
}

type CustomMenuLabel = String;
type CustomMenuCallback = Arc<dyn Fn(MainThreadMarker) + 'static + Send + Sync>;
type CustomMenuToggleValue = Arc<dyn Fn(MainThreadMarker) -> bool + 'static + Send + Sync>;
type CustomMenuCycleValue = Arc<dyn Fn(MainThreadMarker) -> usize + 'static + Send + Sync>;
type CustomMenuItemExtraText = Arc<dyn Fn(MainThreadMarker) -> String + 'static + Send + Sync>;

// TODO submenu should point to a reference instead of owning the submenu?
// if the menu stucture is big, the cloning is wasteful and super nested.
// but it is fine for now.
#[derive(Clone)]
pub enum CustomMenuItem {
    SubMenu(CustomMenu),
    Action {
        label: CustomMenuLabel,
        callback: CustomMenuCallback,
        extra_text: Option<CustomMenuItemExtraText>,
    },
    /// [`CustomMenuItem::Toggle`] is created programmatically.
    ///
    /// Toggles ON and OFF for selected element.
    Toggle {
        label: CustomMenuLabel,
        value: CustomMenuToggleValue,
        /// Callback implementation must change the value
        callback: CustomMenuCallback,
    },
    /// [`CustomMenuItem::Cycle`] is created programmatically.
    ///
    /// Cycles between defined options.
    Cycle {
        label: CustomMenuLabel,
        options: Vec<String>,
        value: CustomMenuCycleValue,
        callback: CustomMenuCallback,
    },
    /// [`CustomMenuItem::CycleCommand`] is user-defined and created dynamically.
    ///
    /// The command manages its own state and does not change the program state.
    CycleCommand {
        label: CustomMenuLabel,
        options: Vec<String>,
        value: usize,
        command: String,
        /// Whether to exclude the selected option argument from the commmand.
        ///
        /// Seems pretty useless but maybe people will find a use for it.
        exclude_option: bool,
    },
    /// Takes up space and does nothing.
    Empty,
}

#[derive(Clone)]
pub struct CustomMenu {
    pub label: CustomMenuLabel,
    pub items: Vec<CustomMenuItem>,
}

#[derive(Clone)]
struct CustomMenuDisplay {
    custom_menu: CustomMenu,
    page: usize,
}

// Stack stucture of current menus.
static CUSTOM_MENU_DISPLAY_STATE: MainThreadRefCell<Vec<CustomMenuDisplay>> =
    MainThreadRefCell::new(Vec::new());

// 7 item per menu page
// "slot8" = previous page
// "slot9" = next page
// "slot10" = back to previous menu or close menu
const PAGE_SIZE: usize = 7;
const LINE_COUNT: usize = PAGE_SIZE
+ 3 // slot8 slot9 slot10
+ 2 // title and title padding
;

pub fn draw_custom_menu(marker: MainThreadMarker, draw: &hud::Draw) {
    if !Menu.is_enabled(marker) {
        return;
    }

    let binding = CUSTOM_MENU_DISPLAY_STATE.borrow_mut(marker);
    let Some(curr_menu_display) = binding.last() else {
        return;
    };

    let (x_pos, y_pos) = get_anchor_location(marker);

    let pos = (x_pos, y_pos);

    let mut multi_line = draw.multi_hud_line(pos.into());

    let draw_red = |s: &mut MultiHUDLine, x: &str| s.same_line(x.as_bytes(), 210, 24, 0);
    let draw_white = |s: &mut MultiHUDLine, x: &str| s.same_line(x.as_bytes(), 255, 255, 255);
    let draw_white_ln = |s: &mut MultiHUDLine, x: &str| s.new_line(x.as_bytes(), 255, 255, 255);
    let draw_yellow_ln = |s: &mut MultiHUDLine, x: &str| s.new_line(x.as_bytes(), 255, 210, 64);
    let draw_red_ln = |s: &mut MultiHUDLine, x: &str| s.new_line(x.as_bytes(), 210, 24, 0);

    let padding = |s: &mut MultiHUDLine| draw_white_ln(s, "\0");

    // draw title and page info
    draw_white_ln(
        &mut multi_line,
        format!(
            "{} ({}/{})\0",
            curr_menu_display.custom_menu.label,
            curr_menu_display.page + 1,
            (curr_menu_display.custom_menu.items.len() + PAGE_SIZE - 1) / PAGE_SIZE
        )
        .as_str(),
    );
    padding(&mut multi_line);

    // draw item menu
    curr_menu_display
        .custom_menu
        .items
        .iter()
        .skip(curr_menu_display.page * PAGE_SIZE)
        .take(PAGE_SIZE)
        .enumerate()
        .for_each(|(idx, item)| {
            match item {
                CustomMenuItem::SubMenu(CustomMenu { label, .. }) => {
                    // write the option number label, it should be red
                    draw_red(&mut multi_line, format!("{}. \0", idx + 1).as_str());

                    // then write the actual text
                    draw_white_ln(&mut multi_line, format!("{}\0", label).as_str());
                }
                CustomMenuItem::Action {
                    label, extra_text, ..
                } => {
                    draw_red(&mut multi_line, format!("{}. \0", idx + 1).as_str());

                    if let Some(extra_text) = extra_text {
                        let extra_text = extra_text(marker);

                        draw_white(&mut multi_line, format!("{} \0", label).as_str());
                        draw_yellow_ln(&mut multi_line, format!("{}\0", extra_text).as_str());
                    } else {
                        draw_white_ln(&mut multi_line, format!("{}\0", label).as_str());
                    }
                }
                CustomMenuItem::Toggle { label, value, .. } => {
                    // write option number like always
                    draw_red(&mut multi_line, format!("{}. \0", idx + 1).as_str());

                    // write label, dont draw new line
                    draw_white(&mut multi_line, format!("{}: \0", label).as_str());

                    // write value in yellow and draw new line
                    let value = value(marker);

                    if value {
                        draw_yellow_ln(&mut multi_line, format!("ON\0").as_str());
                    } else {
                        draw_red_ln(&mut multi_line, format!("OFF\0").as_str());
                    }
                }
                CustomMenuItem::Cycle {
                    label,
                    options,
                    value,
                    ..
                } => {
                    draw_red(&mut multi_line, format!("{}. \0", idx + 1).as_str());
                    draw_white(&mut multi_line, format!("{}: \0", label).as_str());

                    let selected_index = value(marker);

                    assert!(
                        selected_index < options.len(),
                        "menu select item outside of range"
                    );

                    draw_yellow_ln(
                        &mut multi_line,
                        format!("{}\0", options[selected_index]).as_str(),
                    );
                }
                CustomMenuItem::CycleCommand {
                    label,
                    options,
                    value,
                    ..
                } => {
                    // the same as [`CustomMenuItem::CycleCommand`]
                    draw_red(&mut multi_line, format!("{}. \0", idx + 1).as_str());
                    draw_white(&mut multi_line, format!("{}\0", label).as_str());
                    draw_yellow_ln(&mut multi_line, format!(" {}\0", options[*value]).as_str());
                }
                CustomMenuItem::Empty => {
                    padding(&mut multi_line);
                }
            };
        });

    // draw empty lines if any
    let current_page_item_count = curr_menu_display
        .custom_menu
        .items
        .len()
        .saturating_sub(curr_menu_display.page * PAGE_SIZE)
        .min(PAGE_SIZE);

    let pad_count = PAGE_SIZE - current_page_item_count;

    for _ in 0..pad_count {
        padding(&mut multi_line);
    }

    padding(&mut multi_line);

    // draw navigation options
    let is_on_page_1 = curr_menu_display.page == 0;
    let can_go_next_page = curr_menu_display.page
    // have to sat_sub so that 7 items won't spawn new menu
        < (curr_menu_display.custom_menu.items.len().saturating_sub(1) / PAGE_SIZE);
    let is_submenu = binding.len() > 1; // the name is not accurate cuz can stack multiple main menu

    // previous
    if !is_on_page_1 {
        draw_red(&mut multi_line, "8. \0");
        draw_white_ln(&mut multi_line, "Previous\0");
    } else {
        padding(&mut multi_line);
    }

    // next
    if can_go_next_page {
        draw_red(&mut multi_line, "9. \0");
        draw_white_ln(&mut multi_line, "Next\0");
    } else {
        padding(&mut multi_line);
    }

    // close/black
    // there is always option 0
    draw_red(&mut multi_line, "0. \0");
    if is_submenu {
        draw_white_ln(&mut multi_line, "Back\0");
    } else {
        draw_white_ln(&mut multi_line, "Close\0");
    }
}

pub fn handle_interact_custom_menu(marker: MainThreadMarker, text: *const i8) -> *const i8 {
    if !Menu.is_enabled(marker) {
        return text;
    }

    if text.is_null() {
        return text;
    }

    let mut binding = CUSTOM_MENU_DISPLAY_STATE.borrow_mut(marker);

    let Some(curr_menu_display) = binding.last_mut() else {
        return text;
    };

    let mut text_mut: *const u8 = text.cast();

    let matching_progressive_f = |needle: &[u8], mut haystack: *const u8| {
        for bytes in needle {
            if unsafe { *haystack } != *bytes {
                return None;
            }

            haystack = unsafe { haystack.add(1) };
        }

        Some(haystack)
    };

    // TODO, this is bad, will match any command starting with `slotN`
    // but it is good enough for now
    // prefix "slot"
    if let Some(new_text_mut) = matching_progressive_f(b"slot", text_mut) {
        text_mut = new_text_mut;
    } else {
        return text;
    };

    // then parse numbers
    let options = [
        "10", // parse 10 first becuase it has prefix "1"
        "1", "2", "3", "4", "5", "6", "7", "8", "9",
    ];

    let is_on_page_1 = curr_menu_display.page == 0;
    let can_go_next_page = curr_menu_display.page
        < (curr_menu_display.custom_menu.items.len().saturating_sub(1) / PAGE_SIZE);

    for (idx, option) in options.iter().enumerate() {
        if let Some(new_text_mut) = matching_progressive_f(option.as_bytes(), text_mut) {
            // matched option idx
            match idx {
                1..=7 => {
                    // 1-7 = menu items
                    let item_idx = curr_menu_display.page * PAGE_SIZE + (idx - 1);

                    if let Some(item) = curr_menu_display.custom_menu.items.get_mut(item_idx) {
                        match item {
                            CustomMenuItem::SubMenu(submenu) => {
                                let submenu_cloned = submenu.clone();

                                // need to drop to change the borrow
                                // otherwise epic panic happens
                                drop(binding);

                                let mut binding = CUSTOM_MENU_DISPLAY_STATE.borrow_mut(marker);

                                binding.push(CustomMenuDisplay {
                                    custom_menu: submenu_cloned,
                                    page: 0,
                                });
                            }
                            CustomMenuItem::Action { callback, .. }
                            | CustomMenuItem::Toggle { callback, .. }
                            | CustomMenuItem::Cycle { callback, .. } => {
                                // WTF??? Windows???
                                // Windows will crash if I don't do this.
                                let cloned_cb = callback.clone();

                                drop(binding);

                                cloned_cb(marker);
                            }
                            CustomMenuItem::CycleCommand {
                                options,
                                command,
                                value: selected_index,
                                exclude_option,
                                ..
                            } => {
                                *selected_index = (*selected_index + 1) % options.len();

                                let formatted_command = format!(
                                    "{} {}\0",
                                    command,
                                    if *exclude_option {
                                        ""
                                    } else {
                                        options[*selected_index].as_str()
                                    }
                                );

                                // Windows will crash if I don't do this
                                drop(binding);

                                prepend_command(marker, formatted_command.as_str());
                            }
                            CustomMenuItem::Empty => (),
                        }
                    }
                }
                8 => {
                    // 8 = previous page
                    if !is_on_page_1 {
                        curr_menu_display.page -= 1;
                    }
                }
                9 => {
                    // 9 = next page
                    if can_go_next_page {
                        curr_menu_display.page += 1;
                    }
                }
                0 => {
                    // 10 = back/close
                    binding.pop();
                }
                _ => unreachable!("more than 10 options matched"),
            }

            text_mut = new_text_mut;

            return text_mut.cast();
        }
    }

    // no matched index, meaning it is `slotBOGUS`
    // return original text
    text
}

/// Returns boolean indicating whether the menu is on or off
pub fn toggle_menu_display(marker: MainThreadMarker, custom_menu: CustomMenu) -> bool {
    let mut binding = CUSTOM_MENU_DISPLAY_STATE.borrow_mut(marker);

    // if menu is on top then close it
    if binding
        .last()
        .is_some_and(|x| x.custom_menu.label == custom_menu.label)
    {
        binding.clear();
        return false;
    };

    // clear menu cuz stacking main menu seems bad
    binding.clear();

    binding.push(CustomMenuDisplay {
        custom_menu,
        page: 0,
    });

    return true;
}

fn calculate_menu_y_pos(marker: MainThreadMarker, entry_count: usize) -> i32 {
    let SCREENINFO {
        iSize: _,
        iWidth: _,
        iHeight,
        iFlags: _,
        iCharHeight,
        charWidths: _,
    } = hud::screen_info(marker);

    // must be consistent with the font height used in MultiHUDLine
    let font_height = 12.max(iCharHeight);

    // pulled from CHudMenu::Draw in cl_dll/menu.cpp
    (iHeight / 2) - ((entry_count as i32 / 2) * font_height) - (3 * font_height + font_height / 3)
}
