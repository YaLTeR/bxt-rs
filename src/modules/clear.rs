//! `bxt_clear`

use super::commands::Command;
use super::Module;
use crate::handler;
use crate::hooks::engine;
use crate::modules::cvars::{self, CVar};
use crate::utils::*;

pub struct Clear;

impl Module for Clear {
    fn name(&self) -> &'static str {
        "bxt_clear"
    }

    fn description(&self) -> &'static str {
        "Clearing screen with selected color."
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        cvars::CVars.is_enabled(marker) && engine::R_Clear.is_set(marker)
    }

    fn cvars(&self) -> &'static [&'static CVar] {
        static CVARS: &[&CVar] = &[&BXT_CLEAR];
        CVARS
    }

    fn commands(&self) -> &'static [&'static Command] {
        static COMMANDS: &[&Command] = &[&BXT_CLEAR_COLOR];
        COMMANDS
    }
}

static CLEAR_COLOR: MainThreadRefCell<[f32; 3]> = MainThreadRefCell::new([0f32; 3]);

static BXT_CLEAR: CVar = CVar::new(
    b"bxt_clear\0",
    b"\0",
    "\
Forces clearing screen to avoid hall of mirror effect.

Can be used with `bxt_clear_color` to replace cleared pixel with selected color. This is useful for green screen effect.",
);
static BXT_CLEAR_COLOR: Command = Command::new(
    b"bxt_clear_color\0",
    handler!(
        "\
bxt_clear_color \"RED GREEN BLUE\"

This command sets the color to replace cleared pixel.

For example: `bxt_clear_color \"255 0 255\"` for magenta clear color.",
        set_clear_color as fn(_, _)
    ),
);

pub fn should_clear_frame(marker: MainThreadMarker) -> bool {
    Clear.is_enabled(marker) && BXT_CLEAR.as_bool(marker)
}

fn set_clear_color(marker: MainThreadMarker, color_text: String) {
    let color: Vec<_> = color_text
        .split_whitespace()
        .filter_map(|s| s.parse::<f32>().ok())
        .collect();

    let color = if color.len() < 3 {
        [0., 0., 0.]
    } else {
        [color[0] / 255.0, color[1] / 255.0, color[2] / 255.0]
    };

    *CLEAR_COLOR.borrow_mut(marker) = color;
}

pub fn get_clear_color(marker: MainThreadMarker) -> [f32; 3] {
    *CLEAR_COLOR.borrow(marker)
}
