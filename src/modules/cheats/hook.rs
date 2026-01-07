//! `Hook Movement`

use std::ptr::null_mut;

use crate::handler;
use crate::hooks::engine::{self, is_cheat_enabled, player_edict};
use crate::modules::commands::{Command, Commands};
use crate::modules::cvars::{CVar, CVars};
use crate::modules::player_movement_tracing::player_trace;
use crate::modules::Module;
use crate::utils::*;

pub struct CheatHook;
impl Module for CheatHook {
    fn name(&self) -> &'static str {
        "Hook Movement"
    }

    fn description(&self) -> &'static str {
        "Moving without travelling."
    }

    fn commands(&self) -> &'static [&'static Command] {
        static COMMANDS: &[&Command] = &[&PLUS_BXT_HOOK, &MINUS_BXT_HOOK];
        COMMANDS
    }

    fn cvars(&self) -> &'static [&'static CVar] {
        static CVARS: &[&CVar] = &[&BXT_HOOK_SPEED];
        CVARS
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        Commands.is_enabled(marker)
            && CVars.is_enabled(marker)
            && engine::r_refdef_vieworg.is_set(marker)
            && engine::r_refdef_viewangles.is_set(marker)
            && engine::cl_enginefuncs.is_set(marker)
            && engine::svs.is_set(marker) // player_edict
    }
}

static BXT_HOOK_SPEED: CVar = CVar::new(
    b"bxt_hook_speed\0",
    b"869\0",
    "\
Speed of during hook.",
);

static PLUS_BXT_HOOK: Command = Command::new(
    b"+bxt_hook\0",
    handler!(
        "+bxt_hook

Hooks to a point.",
        enable_hook as fn(_),
        enable_hook_key as fn(_, _)
    ),
);

static MINUS_BXT_HOOK: Command = Command::new(
    b"-bxt_hook\0",
    handler!(
        "-bxt_hook

Hooks to a point.",
        disable_hook as fn(_),
        disable_hook_key as fn(_, _)
    ),
);

// Should be reset on change level but that will be way too much for this
static HOOK_ENABLED: MainThreadCell<bool> = MainThreadCell::new(false);
static HOOK_POINT: MainThreadCell<Option<glam::Vec3>> = MainThreadCell::new(None);

fn enable_hook_key(marker: MainThreadMarker, _key: i32) {
    enable_hook(marker);
}

fn enable_hook(marker: MainThreadMarker) {
    HOOK_ENABLED.set(marker, true);

    // hook point is only calculated once because this function runs once
    // due to how the engine handles +/- toggle

    let vieworg = glam::Vec3::from_array(unsafe { *engine::r_refdef_vieworg.get(marker) });
    let viewangles = engine::r_refdef_viewangles.get(marker);
    let mut forward = [0f32; 3];

    // v_forward from globalvars and pm_move don't work
    unsafe {
        ((&*engine::cl_enginefuncs.get(marker)).pfnAngleVectors)(
            viewangles,
            forward.as_mut_ptr() as *mut [f32; 3],
            null_mut(),
            null_mut(),
        )
    };

    let end = vieworg + glam::Vec3::from_array(forward).normalize() * 8192.;

    // use trace so that the hook will "attach" on a wall instead of going through it
    let trace_rensult = unsafe { player_trace(marker, vieworg, end, bxt_strafe::Hull::Point) };

    HOOK_POINT.set(marker, trace_rensult.end_pos.into());
}

fn disable_hook_key(marker: MainThreadMarker, _key: i32) {
    disable_hook(marker);
}

fn disable_hook(marker: MainThreadMarker) {
    HOOK_ENABLED.set(marker, false);
}

pub fn hook_player(marker: MainThreadMarker) {
    if !CheatHook.is_enabled(marker)
        || !HOOK_ENABLED.get(marker)
        || HOOK_POINT.get(marker).is_none()
    {
        return;
    }

    // this call is expensive so it will be last
    if unsafe { !is_cheat_enabled(marker) } {
        return;
    }

    let find_model_index_f =
        (unsafe { &*(&*engine::cl_enginefuncs.get(marker)).pEventAPI }).EV_FindModelIndex;
    let Some(find_model_index_f) = find_model_index_f else {
        return;
    };

    // finally i get to use this feature
    let beam = unsafe { find_model_index_f(c"sprites/smoke.spr".as_ptr()) };

    // draw beam
    let beam_point_f = (unsafe { &*(&*engine::cl_enginefuncs.get(marker)).pEfxAPI }).R_BeamPoints;
    let Some(beam_point_f) = beam_point_f else {
        return;
    };

    let player = unsafe { player_edict(marker) };
    let Some(player) = player.and_then(|mut player| unsafe { player.as_mut().into() }) else {
        return;
    };

    unsafe {
        beam_point_f(
            player.v.origin.as_ptr(),                                // start
            HOOK_POINT.get(marker).unwrap().to_array().as_ptr(),     // end
            beam,                                                    // model
            (*engine::gGlobalVariables.get(marker)).frametime * 1.5, // life
            0.5,                                                     // width
            0.,                                                      // amp
            64.,                                                     // brightness
            0.,                                                      // speed
            0,                                                       // start frame
            0.,                                                      // frame rate
            255.,                                                    // r
            128.,                                                    // g
            0.,                                                      // b
        )
    };

    // actually moving the player
    let target_velocity =
        (HOOK_POINT.get(marker).unwrap() - glam::Vec3::from_array(player.v.origin)).normalize()
            * BXT_HOOK_SPEED.as_f32(marker);
    player.v.velocity = target_velocity.into();
}
