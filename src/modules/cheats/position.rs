//! `bxt_set_pos`

use crate::handler;
use crate::hooks::engine::{self, con_print, is_cheat_enabled, player_edict};
use crate::modules::commands::{Command, Commands};
use crate::modules::cvars::CVars;
use crate::modules::Module;
use crate::utils::*;

pub struct CheatPos;
impl Module for CheatPos {
    fn name(&self) -> &'static str {
        "bxt_set_pos"
    }

    fn description(&self) -> &'static str {
        "Sets player position"
    }

    fn commands(&self) -> &'static [&'static Command] {
        static COMMANDS: &[&Command] = &[&BXT_SET_POS];
        COMMANDS
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        CVars.is_enabled(marker) && Commands.is_enabled(marker)
        && engine::cvar_vars.is_set(marker) // is_cheats_enabled
        && engine::svs.is_set(marker) // player_edict
    }
}

static BXT_SET_POS: Command = Command::new(
    b"bxt_set_pos\0",
    handler!(
        "bxt_set_pos

Sets current position.",
        set_pos_str as fn(_, _),
        set_pos_xyz as fn(_, _, _, _)
    ),
);

fn set_pos_str(marker: MainThreadMarker, pos: String) {
    let pos = pos
        .split_ascii_whitespace()
        .filter_map(|x| x.parse::<f32>().ok())
        .collect::<Vec<f32>>();

    if pos.len() != 3 {
        con_print(marker, "Needs 3 numbers\n");
        return;
    }

    set_pos_xyz(marker, pos[0], pos[1], pos[2]);
}

fn set_pos_xyz(marker: MainThreadMarker, x: f32, y: f32, z: f32) {
    if !CheatPos.is_enabled(marker) || unsafe { !is_cheat_enabled(marker) } {
        return;
    }

    let player = unsafe { player_edict(marker) };
    let Some(player) = player.map(|mut player| unsafe { player.as_mut() }) else {
        return;
    };

    player.v.origin = [x, y, z];
}
