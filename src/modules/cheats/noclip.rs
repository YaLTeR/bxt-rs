//! `Variable noclip speed`

use crate::ffi::playermove::playermove_s;
use crate::handler;
use crate::hooks::engine::{self, is_cheat_enabled};
use crate::modules::commands::{Command, Commands};
use crate::modules::cvars::{CVar, CVars};
use crate::modules::Module;
use crate::utils::*;

pub struct CheatNoclip;
impl Module for CheatNoclip {
    fn name(&self) -> &'static str {
        "Variable noclip speed"
    }

    fn description(&self) -> &'static str {
        "Variable noclip speed and forced noclip."
    }

    fn commands(&self) -> &'static [&'static Command] {
        static COMMANDS: &[&Command] = &[&BXT_NOCLIP];
        COMMANDS
    }

    fn cvars(&self) -> &'static [&'static CVar] {
        static CVARS: &[&CVar] = &[&BXT_NOCLIP_SPEED];
        CVARS
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        CVars.is_enabled(marker) && Commands.is_enabled(marker) && engine::cvar_vars.is_set(marker) // is_cheats_enabled
    }
}

static BXT_NOCLIP_SPEED: CVar = CVar::new(
    b"bxt_noclip_speed\0",
    b"0\0",
    "\
Speed of during noclip.",
);

static BXT_NOCLIP: Command = Command::new(
    b"bxt_noclip\0",
    handler!(
        "bxt_noclip

Toggles noclip.

This is an alternative to `noclip` command where it is guaranteed to work as long as `sv_cheats` is enabled.",
        toggle_noclip as fn(_)
    ),
);

static IS_NOCLIP_TOGGLED: MainThreadCell<bool> = MainThreadCell::new(false);

fn toggle_noclip(marker: MainThreadMarker) {
    if unsafe { !is_cheat_enabled(marker) } {
        return;
    }

    let curr = IS_NOCLIP_TOGGLED.get(marker);

    IS_NOCLIP_TOGGLED.set(marker, !curr);
}

static RESTORED_MAXSPEED: MainThreadCell<f32> = MainThreadCell::new(0.);
static RESTORED_CLIENT_MAXSPEED: MainThreadCell<f32> = MainThreadCell::new(0.);
static RESTORED_MOVETYPE: MainThreadCell<i32> = MainThreadCell::new(0);

pub fn pre_pm_move(marker: MainThreadMarker, ppmove: *mut playermove_s) {
    if !CheatNoclip.is_enabled(marker) {
        return;
    }

    let noclip_speed = BXT_NOCLIP_SPEED.as_f32(marker);
    let ppmove = unsafe { &mut *ppmove };

    if IS_NOCLIP_TOGGLED.get(marker) {
        RESTORED_MOVETYPE.set(marker, ppmove.movetype);
        ppmove.movetype = 8; // MOVETYPE_NOCLIP
    }

    if ppmove.movetype != 8 // MOVETYPE_NOCLIP
     || noclip_speed == 0.
    {
        return;
    }

    let client_maxspeed = ppmove.clientmaxspeed;

    // store to restore later
    RESTORED_MAXSPEED.set(marker, ppmove.maxspeed);
    RESTORED_CLIENT_MAXSPEED.set(marker, client_maxspeed);

    // we will be using clientmaxspeed as the base speed
    // in some mods, they don't set this value but use sv_maxspeed instead
    // this makes it minimally maxspeed instead
    if client_maxspeed == 0. {
        ppmove.clientmaxspeed = ppmove.maxspeed;
    }

    ppmove.cmd.forwardmove = ppmove.cmd.forwardmove / client_maxspeed * noclip_speed;
    ppmove.cmd.sidemove = ppmove.cmd.sidemove / client_maxspeed * noclip_speed;
    ppmove.cmd.upmove = ppmove.cmd.upmove / client_maxspeed * noclip_speed;

    ppmove.clientmaxspeed = noclip_speed;
    ppmove.maxspeed = noclip_speed;
}

pub fn post_pm_move(marker: MainThreadMarker, player_move: *mut playermove_s) {
    if !CheatNoclip.is_enabled(marker) {
        return;
    }

    let player_move = unsafe { &mut *player_move };
    let noclip_speed = BXT_NOCLIP_SPEED.as_f32(marker);

    if player_move.movetype != 8 || noclip_speed == 0. {
        return;
    }

    player_move.maxspeed = RESTORED_MAXSPEED.get(marker);
    player_move.clientmaxspeed = RESTORED_CLIENT_MAXSPEED.get(marker);

    if IS_NOCLIP_TOGGLED.get(marker) {
        player_move.movetype = RESTORED_MOVETYPE.get(marker);
    }
}
