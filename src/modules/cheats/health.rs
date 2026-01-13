//! `bxt_set_health`

use crate::handler;
use crate::hooks::engine::{self, is_cheat_enabled, player_edict};
use crate::modules::commands::{Command, Commands};
use crate::modules::cvars::CVars;
use crate::modules::Module;
use crate::utils::*;

pub struct CheatHealth;
impl Module for CheatHealth {
    fn name(&self) -> &'static str {
        "bxt_set_health"
    }

    fn description(&self) -> &'static str {
        "Sets player health"
    }

    fn commands(&self) -> &'static [&'static Command] {
        static COMMANDS: &[&Command] = &[&BXT_SET_HEALTH];
        COMMANDS
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        CVars.is_enabled(marker) && Commands.is_enabled(marker)
        && engine::cvar_vars.is_set(marker) // is_cheats_enabled
        && engine::svs.is_set(marker) // player_edict
    }
}

static BXT_SET_HEALTH: Command = Command::new(
    b"bxt_set_health\0",
    handler!(
        "bxt_set_health

Sets current health.",
        set_health as fn(_, _)
    ),
);

fn set_health(marker: MainThreadMarker, value: f32) {
    if !CheatHealth.is_enabled(marker) || unsafe { !is_cheat_enabled(marker) } {
        return;
    }

    let player = unsafe { player_edict(marker) };
    let Some(player) = player.map(|mut player| unsafe { player.as_mut() }) else {
        return;
    };

    player.v.health = value;
}
