//! `bxt-rs Timer`

use super::Module;
use crate::ffi::edict::edict_s;
use crate::handler;
use crate::hooks::engine::{self};
use crate::modules::checkpoint_menu;
use crate::modules::commands::{Command, Commands};
use crate::modules::cvars::{CVar, CVars};
use crate::modules::hud::{self};
use crate::modules::timer::auto_timer::{
    find_auto_start_zone, simulate_button_use, simulate_touch_timer_zone,
};
use crate::utils::*;

mod auto_timer;
mod timer_hud;

pub use timer_hud::*;

pub struct Timer;
impl Module for Timer {
    fn name(&self) -> &'static str {
        "bxt-rs Timer"
    }

    fn description(&self) -> &'static str {
        "Timer and goodies"
    }

    fn commands(&self) -> &'static [&'static Command] {
        static COMMANDS: &[&Command] = &[&BXT_TIMER_START, &BXT_TIMER_STOP, &BXT_TIMER_RESET];
        COMMANDS
    }

    fn cvars(&self) -> &'static [&'static CVar] {
        static CVARS: &[&CVar] = &[&BXT_HUD_TIMER, &BXT_TIMER_AUTOSTART, &BXT_HUD_TIMER_ANCHOR];
        CVARS
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        Commands.is_enabled(marker)
            && CVars.is_enabled(marker)
            && hud::Hud.is_enabled(marker)
            && engine::host_frametime.is_set(marker)
            && engine::gEntityInterface.is_set(marker)
            && engine::FindEntityInSphere.is_set(marker) // find_entity_in_sphere
            && engine::sv.is_set(marker) // find_entity_in_sphere, get_all_entities
            && engine::gGlobalVariables.is_set(marker) // get_table_string
    }
}

static BXT_TIMER_START: Command = Command::new(
    b"bxt_timer_start\0",
    handler!(
        "bxt_timer_start

Starts the timer.",
        timer_start as fn(_)
    ),
);

static BXT_TIMER_STOP: Command = Command::new(
    b"bxt_timer_stop\0",
    handler!(
        "bxt_timer_stop

Stops the timer.",
        timer_stop as fn(_)
    ),
);

static BXT_TIMER_RESET: Command = Command::new(
    b"bxt_timer_reset\0",
    handler!(
        "bxt_timer_reset

Resets the timer.",
        timer_reset as fn(_)
    ),
);

static BXT_HUD_TIMER: CVar = CVar::new(
    b"bxt_hud_timer\0",
    b"0\0",
    "\
Displays timer.",
);

static BXT_HUD_TIMER_ANCHOR: CVar = CVar::new(
    b"bxt_hud_timer_anchor\0",
    b"0 0.5\0",
    "\
Anchor position of timer.",
);

static BXT_TIMER_AUTOSTART: CVar = CVar::new(
    b"bxt_timer_autostart\0",
    b"1\0",
    "\
Automatically starts and stops KZ timer.",
);

type TimeData = f32;

#[derive(Clone, Copy, Default)]
pub enum State {
    #[default]
    Idle,
    Running(TimeData),
    Stopped(TimeData),
}

impl State {
    pub fn get_time(&self) -> TimeData {
        match self {
            State::Idle => 0.,
            State::Running(x) => *x,
            State::Stopped(x) => *x,
        }
    }

    pub fn set_time(&mut self, time: f32) {
        *self = match self {
            State::Idle => State::Running(time),
            State::Running(_) => State::Running(time),
            State::Stopped(_) => State::Stopped(time),
        };
    }
}

pub static TIME: MainThreadRefCell<State> = MainThreadRefCell::new(State::Idle);

fn timer_start(marker: MainThreadMarker) {
    // it is nice to have
    checkpoint_menu::reset_current_run_checkpoint(marker);

    if !Timer.is_enabled(marker) {
        return;
    }

    let mut state = TIME.borrow_mut(marker);

    *state = match std::mem::take(&mut *state) {
        State::Idle => State::Running(0.),
        State::Running(x) => State::Running(x),
        State::Stopped(x) => State::Running(x),
    };
}

fn timer_stop(marker: MainThreadMarker) {
    if !Timer.is_enabled(marker) {
        return;
    }

    let mut state = TIME.borrow_mut(marker);

    *state = match std::mem::take(&mut *state) {
        State::Idle => State::Idle,
        State::Running(x) => State::Stopped(x),
        State::Stopped(x) => State::Stopped(x),
    };
}

fn timer_reset(marker: MainThreadMarker) {
    if !Timer.is_enabled(marker) {
        return;
    }

    let mut state = TIME.borrow_mut(marker);

    *state = State::Idle;
}

fn update_timer(marker: MainThreadMarker) {
    let state = *TIME.borrow(marker);

    // only update timer if it is running
    let State::Running(time) = state else {
        return;
    };

    *TIME.borrow_mut(marker) =
        State::Running(time + unsafe { *engine::host_frametime.get(marker) } as f32);
}

pub fn on_new_frame(marker: MainThreadMarker) {
    if !Timer.is_enabled(marker) {
        return;
    }

    update_timer(marker);

    if BXT_TIMER_AUTOSTART.as_bool(marker) {
        find_auto_start_zone(marker);
        simulate_button_use(marker);
    }
}

pub fn on_dispatch_touch(marker: MainThreadMarker, touched_entity: *mut edict_s) {
    if !Timer.is_enabled(marker) {
        return;
    }

    if BXT_TIMER_AUTOSTART.as_bool(marker) {
        simulate_touch_timer_zone(marker, touched_entity);
    }
}
