//! `Checkpoint Menu`

use std::f32;
use std::ffi::CStr;
use std::sync::Arc;

use super::Module;
use crate::ffi::buttons::Buttons;
use crate::ffi::edict::{self, edict_s};
use crate::handler;
use crate::hooks::engine::{self, con_print, find_cvar, player_edict, prepend_command};
use crate::modules::commands::{Command, Commands};
use crate::modules::cvars::{CVar, CVars};
use crate::modules::menu::{self, CustomMenu, CustomMenuItem};
use crate::modules::player_movement_tracing::{self, player_trace};
use crate::modules::timer;
use crate::utils::*;

pub struct CheckpointMenu;
impl Module for CheckpointMenu {
    fn name(&self) -> &'static str {
        "Checkpoint Menu"
    }

    fn description(&self) -> &'static str {
        "Checkpoint system with HUD menu."
    }

    fn commands(&self) -> &'static [&'static Command] {
        static COMMANDS: &[&Command] = &[
            &BXT_CHECKPOINT_MENU,
            &BXT_CHECKPOINT_CREATE,
            &BXT_CHECKPOINT_GOTO,
            &BXT_CHECKPOINT_GOTO_START,
            &BXT_CHECKPOINT_GOTO_LAST,
            &BXT_CHECKPOINT_SET_START,
        ];
        COMMANDS
    }

    fn cvars(&self) -> &'static [&'static CVar] {
        static CVARS: &[&CVar] = &[
            &BXT_CHECKPOINT_WITH_VEL,
            &BXT_CHECKPOINT_CONDITION,
            &BXT_CHECKPOINT_RESET_ON_DISCONNECT,
            &BXT_CHECKPOINT_RESTORE_TIME,
        ];
        CVARS
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        Commands.is_enabled(marker) && CVars.is_enabled(marker)
        && menu::Menu.is_enabled(marker) && engine::svs.is_set(marker) // player_edict
        && player_movement_tracing::PlayerMovementTracing.is_enabled(marker)
        && engine::hudSetViewAngles.is_set(marker)
    }
}

static BXT_CHECKPOINT_MENU: Command = Command::new(
    b"bxt_checkpoint_menu\0",
    handler!(
        "bxt_checkpoint_menu

Toggles checkpoint menu.
Needs `sv_cheats` enabled.",
        toggle_menu as fn(_)
    ),
);

static BXT_CHECKPOINT_CREATE: Command = Command::new(
    b"bxt_checkpoint_create\0",
    handler!(
        "bxt_checkpoint_create

Creates a checkpoint.",
        create_checkpoint as fn(_)
    ),
);

static BXT_CHECKPOINT_GOTO: Command = Command::new(
    b"bxt_checkpoint_goto\0",
    handler!(
        "bxt_checkpoint_goto

Gotos current checkpoint.",
        go_checkpoint as fn(_)
    ),
);

static BXT_CHECKPOINT_GOTO_LAST: Command = Command::new(
    b"bxt_checkpoint_goto_last\0",
    handler!(
        "bxt_checkpoint_goto_last

Gotos last checkpoint and removes current checkpoint.",
        go_last_checkpoint as fn(_)
    ),
);

static BXT_CHECKPOINT_GOTO_START: Command = Command::new(
    b"bxt_checkpoint_goto_start\0",
    handler!(
        "bxt_checkpoint_goto_start

Gotos start checkpoint and resets the run.",
        go_start as fn(_)
    ),
);

static BXT_CHECKPOINT_SET_START: Command = Command::new(
    b"bxt_checkpoint_set_start\0",
    handler!(
        "bxt_checkpoint_set_start

Sets start checkpoint.",
        set_start as fn(_)
    ),
);

static BXT_CHECKPOINT_WITH_VEL: CVar = CVar::new(
    b"bxt_checkpoint_with_vel\0",
    b"1\0",
    "\
Whether go checkpoint restores velocity.",
);

static BXT_CHECKPOINT_CONDITION: CVar = CVar::new(
    b"bxt_checkpoint_condition\0",
    b"3\0",
    "\
Condition to register a checkpoint.

0: Ground only
1: Ground + Ladder
2: Ground + Ladder + Slide
3: Any condition",
);

static BXT_CHECKPOINT_RESET_ON_DISCONNECT: CVar = CVar::new(
    b"bxt_checkpoint_reset_on_disconnect\0",
    b"1\0",
    "\
Whether checkpoints and related data reset upon disconnect.",
);

static BXT_CHECKPOINT_RESTORE_TIME: CVar = CVar::new(
    b"bxt_checkpoint_restore_time\0",
    b"1\0",
    "\
Whether checkpoints restore timer data at checkpoint.",
);

fn is_enabled(marker: MainThreadMarker) -> bool {
    if !CheckpointMenu.is_enabled(marker) {
        return false;
    }

    let sv_cheats = unsafe { find_cvar(marker, "sv_cheats") };
    let Some(sv_cheats) = sv_cheats else {
        return false;
    };
    let sv_cheats_value = (unsafe { *sv_cheats }).value;

    // sv_cheats is not enabled
    if sv_cheats_value == 0. {
        con_print(marker, "sv_cheats is not enabled\n");
        return false;
    }

    true
}

fn toggle_menu(marker: MainThreadMarker) {
    if !is_enabled(marker) {
        return;
    }

    let custom_menu = get_checkpoint_menu();

    menu::toggle_menu_display(marker, custom_menu);
}

type Vec3 = [f32; 3];

#[derive(Clone, Copy)]
struct CheckPointEntry {
    origin: Vec3,
    viewangles: Vec3,
    velocity: Vec3,
    is_duck: bool,
    gravity: f32,
    time: f32,
}

static CHECKPOINT_DATA: MainThreadRefCell<Vec<CheckPointEntry>> = MainThreadRefCell::new(vec![]);
static START_POINT: MainThreadCell<Option<CheckPointEntry>> = MainThreadCell::new(None);
static GO_CHECKPOINT_COUNT: MainThreadCell<usize> = MainThreadCell::new(0);

fn create_checkpoint(marker: MainThreadMarker) {
    if !is_enabled(marker) {
        return;
    }

    let Some(new_entry) = get_player_entry(marker) else {
        return;
    };

    // SAFETY: should pass because get_player_entry() uses get_player_entity() internally
    let player = get_player_entity(marker).unwrap();

    let condition = BXT_CHECKPOINT_CONDITION.as_u64(marker);

    let is_on_ground = player.v.flags.contains(edict::Flags::FL_ONGROUND);
    let is_ladder = player.v.movetype == 5; // MOVETYPE_FLY
    let is_slide = {
        let mut origin_z_offset = player.v.origin;
        origin_z_offset[2] -= 2.;

        let trace_result = unsafe {
            player_trace(
                marker,
                player.v.origin.into(),
                origin_z_offset.into(),
                if new_entry.is_duck {
                    bxt_strafe::Hull::Ducked
                } else {
                    bxt_strafe::Hull::Standing
                },
            )
        };

        // player can still walk on this face. Only value immediately above it is slide-able
        const SLANTED_FACE: f32 = f32::consts::FRAC_1_SQRT_2;

        trace_result.plane_normal.z < SLANTED_FACE && trace_result.plane_normal.z > 0.
    };

    if (condition == 0 && is_on_ground)
        || (condition == 1 && (is_ladder || is_on_ground))
        || (condition == 2 && (is_slide || is_ladder || is_on_ground))
        || condition == 3
    {
        (*CHECKPOINT_DATA.borrow_mut(marker)).push(new_entry);
    }
}

fn go_checkpoint(marker: MainThreadMarker) {
    if !is_enabled(marker) {
        return;
    }

    let player = unsafe { player_edict(marker) };
    let Some(mut player) = player else { return };
    let player = unsafe { player.as_mut() };

    let binding = CHECKPOINT_DATA.borrow_mut(marker);
    let Some(CheckPointEntry {
        origin,
        viewangles,
        velocity,
        is_duck,
        gravity,
        time,
    }) = binding.last()
    else {
        return;
    };

    player.v.origin = *origin;

    // can't just set viewangles directly
    // player.v.v_angle = *viewangles;
    unsafe { engine::hudSetViewAngles.get(marker)(viewangles) };

    // not moving after go check
    // if not set to 0, player velocity will accumulate if BXT_CHECKPOINT_WITH_VEL = 0
    player.v.velocity = [0f32; 3];

    if BXT_CHECKPOINT_WITH_VEL.as_bool(marker) {
        player.v.velocity = *velocity;
    }

    // whatever, maybe somebody in the future will turn that button into bitflag soon
    if *is_duck {
        player.v.flags.set(edict::Flags::FL_DUCKING, *is_duck);
        player.v.button |= Buttons::IN_DUCK.bits() as i32;
    }

    player.v.gravity = *gravity;

    // cs1.6 stamina reset
    if is_game_dir(marker, "cstrike") {
        player.v.fuser2 = 0.;
    }

    // annoying punchangle
    player.v.punchangle = [0f32; 3];

    // increment go checkpoint count
    let go_check_count = GO_CHECKPOINT_COUNT.get(marker);
    GO_CHECKPOINT_COUNT.set(marker, go_check_count + 1);

    // checkpoint timer
    if BXT_CHECKPOINT_RESTORE_TIME.as_bool(marker) {
        timer::TIME.borrow_mut(marker).set_time(*time);
    }
}

fn go_last_checkpoint(marker: MainThreadMarker) {
    if !is_enabled(marker) {
        return;
    }

    (*CHECKPOINT_DATA.borrow_mut(marker)).pop();

    go_checkpoint(marker);
}

fn go_start(marker: MainThreadMarker) {
    if !is_enabled(marker) {
        return;
    }

    // this is convoluted, isn't it
    let Some(start_point) = START_POINT.get(marker) else {
        return;
    };

    // only do things if there is start
    (*CHECKPOINT_DATA.borrow_mut(marker)).clear();

    (*CHECKPOINT_DATA.borrow_mut(marker)).push(start_point);
    go_checkpoint(marker);
    (*CHECKPOINT_DATA.borrow_mut(marker)).clear();

    // also reset timer and start timer again
    prepend_command(marker, "bxt_timer_reset; bxt_timer_start\n");

    // reset go check count when go to start
    GO_CHECKPOINT_COUNT.set(marker, 0);
}

fn set_start(marker: MainThreadMarker) {
    START_POINT.set(marker, get_player_entry(marker));
}

macro_rules! toggle_item {
    ($label:literal, $cvar:expr) => {{
        CustomMenuItem::Toggle {
            label: $label.into(),
            value: Arc::new(move |marker| $cvar.as_bool(marker)),
            callback: Arc::new(move |marker| {
                let value = !$cvar.as_bool(marker);

                prepend_command(
                    marker,
                    format!("{} {}\n", $cvar.name_str(), if value { "1" } else { "0" }).as_str(),
                );
            }),
        }
    }};
}

fn get_checkpoint_menu() -> menu::CustomMenu {
    CustomMenu {
        label: "Checkpoint Menu".to_string(),
        items: vec![
            // 1
            CustomMenuItem::Action {
                label: "CheckPoint".into(),
                callback: Arc::new(create_checkpoint),
                extra_text: Some(Arc::new(move |marker| {
                    format!("#{}", CHECKPOINT_DATA.borrow(marker).len())
                })),
            },
            // 2
            CustomMenuItem::Action {
                label: "GoCheck".into(),
                callback: Arc::new(go_checkpoint),
                extra_text: Some(Arc::new(move |marker| {
                    format!("#{}", GO_CHECKPOINT_COUNT.get(marker))
                })),
            },
            // 3
            CustomMenuItem::Empty,
            // 4
            CustomMenuItem::Action {
                label: "Start".into(),
                callback: Arc::new(go_start),
                extra_text: None,
            },
            // 5
            CustomMenuItem::Empty,
            // 6
            CustomMenuItem::Action {
                label: "Last Checkpoint".into(),
                callback: Arc::new(go_last_checkpoint),
                extra_text: None,
            },
            // 7
            CustomMenuItem::Action {
                label: "Set Start".into(),
                callback: Arc::new(set_start),
                extra_text: Some(Arc::new(move |marker| {
                    if START_POINT.get(marker).is_some() {
                        "ON".into()
                    } else {
                        "OFF".into()
                    }
                })),
            },
            // Page 2
            // 1
            toggle_item!("Restore Velocity", BXT_CHECKPOINT_WITH_VEL),
            // 2
            {
                let options = vec![
                    "Ground".into(),
                    "Ladder".into(),
                    "Slide".into(),
                    "Any".into(),
                ];
                let option_len = options.len() as u64;

                CustomMenuItem::Cycle {
                    label: "CP Condition".into(),
                    options,
                    value: Arc::new(move |marker| BXT_CHECKPOINT_CONDITION.as_u64(marker) as usize),
                    callback: Arc::new(move |marker| {
                        let value = (BXT_CHECKPOINT_CONDITION.as_u64(marker) + 1) % option_len;

                        prepend_command(
                            marker,
                            format!("{} {}\n", BXT_CHECKPOINT_CONDITION.name_str(), value).as_str(),
                        );
                    }),
                }
            },
            // 3
            toggle_item!("Reset on Disconnect", BXT_CHECKPOINT_RESET_ON_DISCONNECT),
            // 4
            toggle_item!("Restore Time", BXT_CHECKPOINT_RESTORE_TIME),
        ],
    }
}

fn get_player_entity<'a>(marker: MainThreadMarker) -> Option<&'a edict_s> {
    let player = unsafe { player_edict(marker) }?;
    let player = unsafe { player.as_ref() };

    Some(player)
}

fn get_player_entry(marker: MainThreadMarker) -> Option<CheckPointEntry> {
    let player = get_player_entity(marker)?;

    let is_duck = (player.v.button & Buttons::IN_DUCK.bits() as i32) != 0
        || (player.v.flags.contains(edict::Flags::FL_DUCKING));

    let time = timer::TIME.borrow(marker).get_time();

    let new_entry = CheckPointEntry {
        origin: player.v.origin,
        viewangles: player.v.v_angle,
        velocity: player.v.velocity,
        is_duck,
        gravity: player.v.gravity,
        time,
    };

    Some(new_entry)
}

static GAME_DIR: MainThreadRefCell<Option<String>> = MainThreadRefCell::new(None);

fn is_game_dir(marker: MainThreadMarker, what: &str) -> bool {
    if let Some(game_dir) = GAME_DIR.borrow(marker).as_ref() {
        return game_dir == what;
    }

    // need to set gamedir first
    let game_dir = engine::com_gamedir
        .get_opt(marker)
        .and_then(|dir| unsafe { CStr::from_ptr(dir.cast()).to_str().ok() })
        .unwrap_or("valve");

    let rv = game_dir == what;

    *GAME_DIR.borrow_mut(marker) = Some(game_dir.to_string());

    rv
}

pub fn reset_current_run_checkpoint(marker: MainThreadMarker) {
    CHECKPOINT_DATA.borrow_mut(marker).clear();
    GO_CHECKPOINT_COUNT.set(marker, 0);
}

fn reset_checkpoints(marker: MainThreadMarker) {
    START_POINT.set(marker, None);
    reset_current_run_checkpoint(marker);
}

pub fn reset_on_new_map(marker: MainThreadMarker) {
    // resets on new map... unless 😳
    if !BXT_CHECKPOINT_RESET_ON_DISCONNECT.as_bool(marker) {
        return;
    }

    reset_checkpoints(marker);
}
