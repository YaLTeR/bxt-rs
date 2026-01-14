//! `Manual Autofuncs`

use std::num::NonZeroU32;

use super::Module;
use crate::handler;
use crate::hooks::engine::{self, prepend_command};
use crate::modules::commands::{self, Command};
use crate::modules::player_movement_tracing::{self};
use crate::modules::tas_optimizer;
use crate::utils::*;

pub struct ManualAutofuncs;
impl Module for ManualAutofuncs {
    fn name(&self) -> &'static str {
        "Manual Autofuncs"
    }

    fn description(&self) -> &'static str {
        "Autojump, ducktap, and jumpbug binds."
    }

    fn commands(&self) -> &'static [&'static Command] {
        static COMMANDS: &[&Command] = &[
            &PLUS_BXT_AUTOJUMP,
            &MINUS_BXT_AUTOJUMP,
            &PLUS_BXT_DUCKTAP,
            &MINUS_BXT_DUCKTAP,
            &PLUS_BXT_JUMPBUG,
            &MINUS_BXT_JUMPBUG,
        ];
        COMMANDS
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        commands::Commands.is_enabled(marker)
            && player_movement_tracing::PlayerMovementTracing.is_enabled(marker)
    }
}

// curse rust macro doesn't allow prefix....
// might have to use paste if this is too annoying
static PLUS_BXT_AUTOJUMP: Command = Command::new(
    b"+bxt_autojump\0",
    handler!(
        "+bxt_autojump

Holds key to autojump",
        enable_autojump as fn(_),
        enable_autojump_key as fn(_, _)
    ),
);

static MINUS_BXT_AUTOJUMP: Command = Command::new(
    b"-bxt_autojump\0",
    handler!(
        "-bxt_autojump

Holds key to autojump",
        disable_autojump as fn(_),
        disable_autojump_key as fn(_, _)
    ),
);

static IS_AUTOJUMP_ENABLED: MainThreadCell<bool> = MainThreadCell::new(false);

fn enable_autojump_key(marker: MainThreadMarker, _key: i32) {
    enable_autojump(marker);
}

fn enable_autojump(marker: MainThreadMarker) {
    IS_AUTOJUMP_ENABLED.set(marker, true);
}

fn disable_autojump_key(marker: MainThreadMarker, _key: i32) {
    disable_autojump(marker);
}

fn disable_autojump(marker: MainThreadMarker) {
    IS_AUTOJUMP_ENABLED.set(marker, false);
}

// ---------------

static PLUS_BXT_DUCKTAP: Command = Command::new(
    b"+bxt_ducktap\0",
    handler!(
        "+bxt_ducktap

Holds key to ducktap",
        enable_ducktap as fn(_),
        enable_ducktap_key as fn(_, _)
    ),
);

static MINUS_BXT_DUCKTAP: Command = Command::new(
    b"-bxt_ducktap\0",
    handler!(
        "-bxt_ducktap

Holds key to ducktap",
        disable_ducktap as fn(_),
        disable_ducktap_key as fn(_, _)
    ),
);

static IS_DUCKTAP_ENABLED: MainThreadCell<bool> = MainThreadCell::new(false);

fn enable_ducktap_key(marker: MainThreadMarker, _key: i32) {
    enable_ducktap(marker);
}

fn enable_ducktap(marker: MainThreadMarker) {
    IS_DUCKTAP_ENABLED.set(marker, true);
}

fn disable_ducktap_key(marker: MainThreadMarker, _key: i32) {
    disable_ducktap(marker);
}

fn disable_ducktap(marker: MainThreadMarker) {
    IS_DUCKTAP_ENABLED.set(marker, false);
}

// ---------------

static PLUS_BXT_JUMPBUG: Command = Command::new(
    b"+bxt_jumpbug\0",
    handler!(
        "+bxt_jumpbug

Holds key to jumpbug",
        enable_jumpbug as fn(_),
        enable_jumpbug_key as fn(_, _)
    ),
);

static MINUS_BXT_JUMPBUG: Command = Command::new(
    b"-bxt_jumpbug\0",
    handler!(
        "-bxt_jumpbug

Holds key to jumpbug",
        disable_jumpbug as fn(_),
        disable_jumpbug_key as fn(_, _)
    ),
);

static IS_JUMPBUG_ENABLED: MainThreadCell<bool> = MainThreadCell::new(false);

fn enable_jumpbug_key(marker: MainThreadMarker, _key: i32) {
    enable_jumpbug(marker);
}

fn enable_jumpbug(marker: MainThreadMarker) {
    IS_JUMPBUG_ENABLED.set(marker, true);
}

fn disable_jumpbug_key(marker: MainThreadMarker, _key: i32) {
    disable_jumpbug(marker);
}

fn disable_jumpbug(marker: MainThreadMarker) {
    IS_JUMPBUG_ENABLED.set(marker, false);
}

// --- now i can do my stuffs

static IN_JUMP: MainThreadCell<bool> = MainThreadCell::new(false);
static IN_DUCK: MainThreadCell<bool> = MainThreadCell::new(false);

macro_rules! press_key {
    ($set:ident, $state:ident, $command:literal, $marker:ident) => {{
        if ($set && !$state.get($marker)) {
            prepend_command($marker, concat!("+", $command, "\n"));
            $state.set($marker, true);
        } else if (!$set && $state.get($marker)) {
            prepend_command($marker, concat!("-", $command, "\n"));
            $state.set($marker, false);
        }
    }};
}

static PARAMETER: MainThreadRefCell<bxt_strafe::Parameters> =
    MainThreadRefCell::new(bxt_strafe::Parameters {
        frame_time: 0.010000001, // only this matters
        max_velocity: 2000.,
        max_speed: 320.,
        stop_speed: 100.,
        friction: 4.,
        edge_friction: 2.,
        ent_friction: 1.,
        accelerate: 10.,
        air_accelerate: 10.,
        gravity: 800.,
        ent_gravity: 1.,
        step_size: 18.,
        bounce: 1.,
        bhop_cap: false,
        bhop_cap_multiplier: 0.65,
        bhop_cap_max_speed_scale: 1.7,
        use_slow_down: true,
        has_stamina: false,
        duck_animation_slow_down: false,
    });

pub fn process_manual_autofuncs(marker: MainThreadMarker) {
    if !ManualAutofuncs.is_enabled(marker) {
        return;
    }

    let ducktap = IS_DUCKTAP_ENABLED.get(marker);
    let autojump = IS_AUTOJUMP_ENABLED.get(marker);
    let jumpbug = IS_JUMPBUG_ENABLED.get(marker);

    if !(autojump || ducktap || jumpbug) {
        // always reset the keys
        if IN_JUMP.get(marker) {
            IN_JUMP.set(marker, false);
        }

        if IN_DUCK.get(marker) {
            IN_DUCK.set(marker, false);
        }

        return;
    }

    // caching data is cringe
    let world_tracer = unsafe { player_movement_tracing::Tracer::new(marker, false).unwrap() };

    // this data is cached though
    // I tried doing it like tas_optimizer but it the framerate went from 650fps down to 600
    // Maybe tas_optimizer can optimize there
    let parameters = &mut PARAMETER.borrow_mut(marker);

    // this doesn't need caching because each frame will update most of the data anyway
    let player_data = unsafe { tas_optimizer::player_data(marker) };
    let Some(player_data) = player_data else {
        return;
    };

    let host_frametime = unsafe { *engine::host_frametime.get(marker) };
    let host_frametime = ((host_frametime * 1000.).floor() * 0.001) as f32;

    parameters.frame_time = host_frametime;

    let state = bxt_strafe::State::new(&world_tracer, **parameters, player_data);

    // mimicking BXT
    // not sure why but it works and i won't question it
    let mut jump = false;
    let mut duck = false;

    if ducktap && matches!(state.place, bxt_strafe::Place::Ground) {
        // dont duck while already ducking
        // which is enough for ducktap
        if !IN_DUCK.get(marker) && !state.player.in_duck_animation {
            duck = true;
        }
    } else if autojump && !matches!(state.place, bxt_strafe::Place::Air) {
        jump = true;
    } else if jumpbug && matches!(state.place, bxt_strafe::Place::Air) {
        // simulate a frame
        let jumpbug_bulk = hltas::types::FrameBulk {
            auto_actions: hltas::types::AutoActions {
                jump_bug: Some(hltas::types::JumpBug {
                    times: hltas::types::Times::UnlimitedWithinFrameBulk,
                }),
                ..Default::default()
            },
            movement_keys: Default::default(),
            action_keys: Default::default(),
            frame_time: host_frametime.to_string(),
            pitch: None,
            // simulate 1 frame
            frame_count: NonZeroU32::new(1).unwrap(),
            console_command: None,
        };
        let (_, input) = state.simulate(&world_tracer, **parameters, &jumpbug_bulk);

        duck = input.duck;
        jump = input.jump;
    };

    if jumpbug {
        press_key!(duck, IN_DUCK, "duck", marker);
        press_key!(jump, IN_JUMP, "jump", marker);
    } else if ducktap {
        press_key!(duck, IN_DUCK, "duck", marker);
    } else if autojump {
        press_key!(jump, IN_JUMP, "jump", marker);
    }
}
