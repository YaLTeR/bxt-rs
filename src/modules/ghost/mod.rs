use std::array::from_fn;
use std::ptr::null_mut;

use self::get_ghost::GhostInfo;
use super::commands::Command;
use super::cvars::CVar;
use super::Module;
use crate::ffi::edict::{edict_s, entvars_s};
use crate::handler;
use crate::hooks::engine::{self, con_print, player_edict, sv_player};
use crate::hooks::server::{self, CBaseEntity__Create};
use crate::hooks::utils::get_entvars;
use crate::utils::*;

mod get_ghost;
mod misc;
use alloc::ffi::CString;

use get_ghost::get_ghost;
use libc::c_void;

extern crate alloc;

pub struct Ghost;
impl Module for Ghost {
    fn name(&self) -> &'static str {
        "Ghost playback"
    }

    fn description(&self) -> &'static str {
        "Playing back ghost from demo and various WRBot formats."
    }

    fn commands(&self) -> &'static [&'static Command] {
        static COMMANDS: &[&Command] = &[
            &BXT_GHOST_ADD,
            &BXT_GHOST_SHOW,
            &BXT_GHOST_OPTION,
            &BXT_GHOST_PLAY,
            &BXT_GHOST_STOP,
            &BXT_GHOST_RESET,
            &BXT_TEST_WHAT,
        ];
        COMMANDS
    }

    fn cvars(&self) -> &'static [&'static CVar] {
        static CVARS: &[&CVar] = &[
            &BXT_GHOST_FRAMETIME_OVERRIDE,
            &BXT_GHOST_PLAY_ON_CONNECT,
            &BXT_GHOST_SPEED,
        ];
        CVARS
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        engine::host_frametime.is_set(marker)
        // && server::CBaseEntity__Create.is_set(marker)
        && engine::svs.is_set(marker)
        && engine::hudSetViewAngles.is_set(marker)
    }
}

struct BxtGhostInfo<'a> {
    ghost_info: GhostInfo,
    offset: f64,
    // Will be the controlling player
    is_player: bool,
    is_transparent: bool,
    // In multiplayer mode, will be other "player" entities.
    is_spectable: bool,
    // Edict will be spawned for every ghost regardless of conditions.
    // The rest lies on implementation.
    // Option type is for when add the ghost for the first time.
    edict: Option<&'a mut edict_s>,
    // Eh, good enough to free the CBaseEntity
    _cbase_entity: Option<*mut c_void>,
}

static GHOSTS: MainThreadRefCell<Vec<BxtGhostInfo>> = MainThreadRefCell::new(vec![]);

static BXT_GHOST_ADD: Command = Command::new(
    b"bxt_ghost_add\0",
    handler!(
        "xxx <name>

xxx.",
        ghost_add as fn(_, _, _)
    ),
);

fn ghost_add(marker: MainThreadMarker, file_name: String, offset: f64) {
    match get_ghost(&file_name) {
        Ok(ghost_info) => {
            GHOSTS.borrow_mut(marker).push(BxtGhostInfo {
                ghost_info,
                offset,
                is_player: false,
                is_transparent: false,
                is_spectable: false,
                edict: None,
                _cbase_entity: None,
            });
        }
        Err(err) => con_print(marker, &format!("Cannot read file.\n{}\n", err)),
    }
}

static BXT_GHOST_SHOW: Command = Command::new(
    b"bxt_ghost_show\0",
    handler!("Show current ghosts.", ghost_show as fn(_)),
);

fn ghost_show(marker: MainThreadMarker) {
    let mut s = String::new();
    let ghosts = &*GHOSTS.borrow(marker);

    for (index, ghost) in ghosts.iter().enumerate() {
        s += &format!(
            "{}: name {} offset {} {} {} {}\n",
            index,
            ghost.ghost_info.ghost_name,
            ghost.offset,
            if ghost.is_player { "player" } else { "" },
            if ghost.is_transparent {
                "transparent"
            } else {
                ""
            },
            if ghost.is_spectable { "spectable" } else { "" }
        );
    }

    con_print(marker, &s);
}

static BXT_GHOST_OPTION: Command = Command::new(
    b"bxt_ghost_option\0",
    handler!("Change ghost settings.", ghost_option as fn(_, _, _)),
);

fn ghost_option(marker: MainThreadMarker, index: usize, option: String) {
    let mut ghosts = GHOSTS.borrow_mut(marker);
    let ghost = ghosts.get_mut(index);
    let is_spawned = matches!(*STATE.borrow_mut(marker), State::Paused | State::Playing);
    let cannot_change = || {
        con_print(
            marker,
            "Entity is already spawned. Consider removing this ghost and try again.\n",
        )
    };

    if ghost.is_none() {
        return;
    }

    let ghost = ghost.unwrap();

    match option.as_str() {
        "player" => ghost.is_player = !ghost.is_player,
        "transparent" => ghost.is_transparent = !ghost.is_transparent,
        maybe_float => {
            let maybe_float = maybe_float.parse::<f64>();
            if let Ok(float) = maybe_float {
                ghost.offset = float;
            } else {
                con_print(marker, "Unable to add such option.");
            }
        }
    }
}

static BXT_GHOST_FRAMETIME_OVERRIDE: CVar = CVar::new(
    b"bxt_ghost_frametime_override\0",
    b"0\0",
    "Use default frame time for all ghosts",
);

static BXT_GHOST_PLAY_ON_CONNECT: CVar = CVar::new(
    b"bxt_ghost_play_on_connect\0",
    b"0\0",
    "Load ghosts as soon as new server is started.",
);

static BXT_GHOST_SPEED: CVar = CVar::new(b"bxt_ghost_speed\0", b"1\0", "Playback speed.");
// #[derive(Clone)]
enum State {
    Idle,
    Spawning,
    // Playing and Pausing are the same.
    // Pausing will not increase timer but still update timer.
    Playing,
    Paused,
    // Stopped means no timer update.
    // It is helpful in a way that the player can move around
    // in case ghost is player.
    Stopped,
}

static STATE: MainThreadRefCell<State> = MainThreadRefCell::new(State::Idle);

static BXT_GHOST_PLAY: Command =
    Command::new(b"bxt_ghost_play\0", handler!("Play ghosts.", play as fn(_)));

pub fn play(marker: MainThreadMarker) {
    let mut state = STATE.borrow_mut(marker);

    *state = match *state {
        State::Idle => State::Spawning,
        State::Playing => State::Paused,
        _ => State::Playing,
    };

    let mut ghosts = GHOSTS.borrow_mut(marker);

    // Set some variables
    ghosts.iter_mut().for_each(|ghost| {
        // If player then renderamt 0 so the ghost doesn't appear on top of firstperson
        if ghost.is_player {
            if let Some(edict) = &mut ghost.edict {
                edict.v.renderamt = 0.;
                edict.v.rendermode = 2; // texture
            }
        } else {
            if let Some(edict) = &mut ghost.edict {
                edict.v.rendermode = 0; // normal
            }
        }
    });

    // Check if it is ok to play ghosts
    let mut spectable_count = 1; // the host is a player
    let mut player_count = 0; // player as in the host

    for ghost in &*ghosts {
        if ghost.is_player {
            player_count += 1;
        } else if ghost.is_spectable {
            spectable_count += 1;
        }
    }

    if spectable_count > 32 {
        con_print(
            marker,
            &format!(
                "Cannot play ghosts. Too many spectable ({} > 32).",
                spectable_count
            ),
        );
        *state = State::Idle;
    }

    if player_count > 1 {
        con_print(
            marker,
            &format!(
                "Cannot play ghosts. Too many player ({} > 1).",
                player_count
            ),
        );
        *state = State::Idle;
    }
}

static BXT_GHOST_STOP: Command = Command::new(
    b"bxt_ghost_stop\0",
    handler!("Stop playing ghosts.", ghost_stop as fn(_)),
);

fn ghost_stop(marker: MainThreadMarker) {
    *STATE.borrow_mut(marker) = State::Stopped;

    // Reset timer?
    TIME.set(marker, 0.);
}

static BXT_GHOST_RESET: Command = Command::new(
    b"bxt_ghost_reset\0",
    handler!("Reset all data related to ghost.", ghost_reset as fn(_)),
);

fn ghost_reset(marker: MainThreadMarker) {
    *STATE.borrow_mut(marker) = State::Idle;

    // Reset timer?
    TIME.set(marker, 0.);

    // Since we are clearing everything, must free CBaseEntity
    free_ghost_cbase(marker);
    GHOSTS.borrow_mut(marker).clear();
}

// TODO: really remove the entity because this doesn't work good enough
pub fn free_ghost_cbase(marker: MainThreadMarker) {
    GHOSTS.borrow_mut(marker).iter_mut().for_each(|ghost| {
        if let Some(edict) = &mut ghost.edict {
            edict.free = 1;
        }
        if let Some(cbase) = ghost._cbase_entity {
            unsafe { server::UTIL_Remove.get(marker)(cbase) };
        }
    });
}

unsafe fn spawn(marker: MainThreadMarker, ghost: &mut BxtGhostInfo) {
    // Spawns ghost even if it is player.
    // Use renderamt to make the ghost disappear.
    // if ghost.is_player {
    //     return;
    // }

    let entity_name = if ghost.is_spectable {
        "player"
    } else {
        "info_target"
    };

    let new_entity =
        &mut *create_entity(marker, entity_name.to_owned(), [0., 0., 0.], [0., 0., 0.]);

    // Just for convenience and obviousness
    let player = player_edict(marker).unwrap().as_ref();
    new_entity.modelindex = player.v.modelindex;

    ghost.edict = Some(&mut *new_entity.pContainingEntity);
}

static TIME: MainThreadCell<f64> = MainThreadCell::new(0.);

pub fn update_ghosts(marker: MainThreadMarker) {
    if !Ghost.is_enabled(marker) {
        return;
    }

    // Will do something as long as it is not idling.
    if matches!(*STATE.borrow_mut(marker), State::Idle) {
        return;
    }

    let mut state = STATE.borrow_mut(marker);
    let mut ghosts = GHOSTS.borrow_mut(marker);
    let time = TIME.get(marker);

    match *state {
        // Already returned from previous condition
        State::Idle => unreachable!(),
        State::Spawning => {
            ghosts
                .iter_mut()
                .for_each(|ghost| unsafe { spawn(marker, ghost) });

            // Done with spawning and start playing
            *state = State::Playing;
        }
        // Will update ghost even when in pause. The only difference is the timer update.
        State::Playing | State::Paused => {
            let passed_time = unsafe { *engine::host_frametime.get(marker) };
            let passed_time = if matches!(*state, State::Playing) {
                passed_time * BXT_GHOST_SPEED.as_f32(marker) as f64
            } else {
                0.
            };

            ghosts.iter_mut().for_each(|ghost| {
                let ghost_frametime = BXT_GHOST_FRAMETIME_OVERRIDE.as_f32(marker) as f64;
                let frametime = if ghost_frametime == 0. {
                    None
                } else {
                    Some(ghost_frametime)
                };

                // Add offset here
                let frame = ghost.ghost_info.get_frame(time + ghost.offset, frametime);

                if let Some(frame) = frame {
                    if ghost.is_player {
                        let player = unsafe { player_edict(marker).unwrap().as_mut() };

                        player.v.origin.copy_from_slice(&frame.origin.to_array());
                        unsafe {
                            engine::hudSetViewAngles.get(marker)(&mut frame.viewangles.to_array())
                        };

                        // The player is free falling all the time. This will fix it.
                        player.v.velocity[2] = 0.;
                    }

                    if let Some(edict) = &mut ghost.edict {
                        (edict.v.origin).copy_from_slice(&frame.origin.to_array());
                        (edict.v.v_angle).copy_from_slice(&frame.viewangles.to_array());

                        if ghost.is_transparent {
                            edict.v.rendermode = 5;
                            edict.v.renderamt = 192.;
                        }

                        // If player then don't show the overlapping ghost
                        // Do this in the update() instead of spawn().
                        // Even the player will have a ghost by design. It just doesn't show up.
                        if ghost.is_player {
                            edict.v.renderamt = 0.;
                            edict.v.rendermode = 2; // texture
                        }

                        edict.v.sequence = 4;
                        edict.v.framerate = 1.;
                        edict.v.frame = (edict.v.frame + passed_time as f32 / 100.) % 256.;
                        println!("frame is {}", edict.v.frame);
                    }
                }
            });

            // If playing, will increase the timer.
            // If paused, will not increase the timer.
            // By doing this, we can seek ahead.

            TIME.set(marker, TIME.get(marker) + passed_time);
        }
        // Stopped, do nothing
        // Player's viewangles and origin are not updated so player can move around.
        State::Stopped => (),
    }
}

static BXT_TEST_WHAT: Command = Command::new(
    b"bxt_test_what\0",
    handler!(
        "xxx <name>

xxx.",
        test_what as fn(_, _)
    ),
);

fn test_what(marker: MainThreadMarker, what: String) {
    println!("received input {}", what);

    unsafe {
        let huh = &*engine::sv_player.get(marker);
        println!("{:?}", huh.v);
        // // let mut origin = (*engine::sv_player.get(marker)).v.origin;
        // // origin[2] += 100.;

        // let origin = [427.317352, -1069.668945, -1659.968750 + 100.];
        // let viewangles = [0., 0., 0.];

        // // let entity = CBaseEntity__Create.get(marker)(a, origin.as_mut_ptr(),
        // // viewangles.as_mut_ptr(), null_mut()); let entity = get_entvars(entity);
        // let entity = &mut *create_entity(marker, what, origin, viewangles);
        // // println!("{:?}", entity as en);
        // // println!("{:?}", entity);

        // let player = player_edict(marker).unwrap().as_ref();

        // entity.modelindex = player.v.modelindex;
    }
}

unsafe fn create_entity(
    marker: MainThreadMarker,
    entity: String,
    origin: [f32; 3],
    viewangles: [f32; 3],
) -> *mut entvars_s {
    let s = CString::new(entity).unwrap();
    let s = s.into_raw();
    let entity = CBaseEntity__Create.get(marker)(
        s,
        origin.to_owned().as_mut_ptr(),
        viewangles.to_owned().as_mut_ptr(),
        null_mut(),
    );
    get_entvars(entity)
}
