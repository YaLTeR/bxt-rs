use std::ptr::null_mut;

use self::get_ghost::GhostInfo;
use super::commands::Command;
use super::cvars::CVar;
use super::triangle_drawing::triangle_api::{Primitive, RenderMode};
use super::triangle_drawing::TriangleApi;
use super::Module;
use crate::ffi::edict::{edict_s, entvars_s};
use crate::handler;
use crate::hooks::engine::{self, con_print, create_entity, player_edict};
use crate::hooks::server::{self, CBaseEntity__Create_Linux};
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
        ];
        COMMANDS
    }

    fn cvars(&self) -> &'static [&'static CVar] {
        static CVARS: &[&CVar] = &[
            &BXT_GHOST_FRAMETIME_OVERRIDE,
            &BXT_GHOST_PLAY_ON_CONNECT,
            &BXT_GHOST_SPEED,
            &BXT_GHOST_PATH,
            &BXT_GHOST_PATH_PROGRESSIVE,
        ];
        CVARS
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        engine::host_frametime.is_set(marker)
        // && server::CBaseEntity__Create.is_set(marker)
        && engine::svs.is_set(marker)
        && engine::hudSetViewAngles.is_set(marker)
        && engine::sv_paused.is_set(marker)
        && engine::cls.is_set(marker)
        && engine::CreateNamedEntity.is_set(marker)
        && engine::gEntityInterface.is_set(marker)
        && engine::gGlobalVariables.is_set(marker)
    }
}

#[derive(Debug)]
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
    // Animation info
    curr_anim: Animation,
    last_origin: [f32; 3],
    // To avoid calculation where a ghost is done playing.
    should_stop: bool,
    // RGBA
    path_color: [f32; 4],
}

#[derive(Debug)]
enum Animation {
    /// 4
    OnGround,
    /// 6 or 7
    InAir,
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
    // If not idling then switch to spawning right away.
    // So, if ghost is added after playing, then ghost will show up.
    // Ignore the idling state so the ghosts won't play right after add.
    let mut state = STATE.borrow_mut(marker);

    if !matches!(*state, State::Idle) {
        *state = State::Spawning;
    }

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
                // better to start in air so we can use the z diff
                curr_anim: Animation::InAir,
                last_origin: [0f32; 3],
                should_stop: false,
                path_color: [0., 1., 0., 1.],
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
            "{}: name {} offset {} {} {} {} {:?}\n",
            index,
            ghost.ghost_info.ghost_name,
            ghost.offset,
            if ghost.is_player { "player" } else { "" },
            if ghost.is_transparent {
                "transparent"
            } else {
                ""
            },
            if ghost.is_spectable { "spectable" } else { "" },
            ghost.path_color
        );
    }

    con_print(marker, &s);
}

static BXT_GHOST_OPTION: Command = Command::new(
    b"bxt_ghost_option\0",
    handler!(
        "\
Change ghost settings.

player: Mark the current ghost as main player (first person).
transparent: Make the ghost transparent
path_color <r> <g> <b> <a>: Change the color of the ghost path.
<number>: change the offset of the ghost.

Example: `bxt_ghost_option 0 player`",
        ghost_option as fn(_, _, _)
    ),
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

    let mut options = option.split_whitespace();

    match options.next().unwrap() {
        "player" => ghost.is_player = !ghost.is_player,
        "transparent" => ghost.is_transparent = !ghost.is_transparent,
        "path_color" => {
            let colors = options
                .filter_map(|s| s.parse::<f32>().ok())
                .collect::<Vec<f32>>();
            if colors.len() != 4 {
                return;
            }

            ghost.path_color = [colors[0], colors[1], colors[2], colors[3]];
        }
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
    if !can_play_ghost(marker) {
        return;
    }

    let mut state = STATE.borrow_mut(marker);

    let mut ghosts = GHOSTS.borrow_mut(marker);

    if ghosts.len() == 0 {
        con_print(marker, "There are no ghosts.\n");
        return;
    }

    // reset just in case
    ghosts.iter_mut().for_each(|ghost| {
        ghost.should_stop = false;
    });

    *state = match *state {
        State::Idle => State::Spawning,
        State::Playing => State::Paused,
        State::Stopped => {
            // Reset time if it is stopped. Then set back to playing.
            TIME.set(marker, 0.);
            State::Playing
        }
        _ => State::Playing,
    };

    // Set some variables
    ghosts.iter_mut().for_each(|ghost| {
        // If player then renderamt 0 so the ghost doesn't appear on top of firstperson
        if ghost.is_player {
            if let Some(edict) = &mut ghost.edict {
                edict.v.renderamt = 0.;
                edict.v.rendermode = 2; // texture
            }
        } else if let Some(edict) = &mut ghost.edict {
            edict.v.rendermode = 0; // normal
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
    if !can_play_ghost(marker) {
        return;
    }

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
    // So we have to spawn regardless of what happens.
    // if ghost.is_player { return; }

    // If spawning in the middle of a play, this will do the job.
    let mut state = STATE.borrow_mut(marker);
    *state = State::Spawning;

    // This means we will go back here again in the next tick if this is invoked while playing.
    // So we need to stop ourselves from duplicating spawns
    if ghost.edict.is_some() {
        return;
    }

    let entity_name = if ghost.is_spectable {
        "player"
    } else {
        "info_target"
    };

    let new_edict = create_entity(marker, entity_name, [0., 0., 0.]);

    if let Some(new_edict) = new_edict {
        let player = player_edict(marker).unwrap().as_ref();

        (*new_edict).v.modelindex = player.v.modelindex;
        (*new_edict).v.sequence = 19; // starts with in air
        (*new_edict).v.v_angle = [0., 0., 0.];
        (*new_edict).v.angles = [0., 0., 0.];

        ghost.edict = Some(&mut *new_edict);
    }
}

static BXT_GHOST_PATH: CVar = CVar::new(
    b"bxt_ghost_path\0",
    b"0\0",
    "Visualizes the path of a ghost",
);

static BXT_GHOST_PATH_PROGRESSIVE: CVar = CVar::new(
    b"bxt_ghost_path_progressive\0",
    b"0\0",
    "Progressively draws the path of the ghost as the ghost is playing back.",
);

static TIME: MainThreadCell<f64> = MainThreadCell::new(0.);

pub fn draw_ghost_path(marker: MainThreadMarker, tri: &TriangleApi) {
    if !BXT_GHOST_PATH.as_bool(marker) {
        return;
    }

    let gl = crate::gl::GL.borrow(marker);
    if let Some(gl) = gl.as_ref() {
        unsafe {
            gl.LineWidth(2.);
        }
    }

    tri.render_mode(RenderMode::TransColor);
    tri.begin(Primitive::Lines);

    if !BXT_GHOST_PATH_PROGRESSIVE.as_bool(marker) {
        GHOSTS.borrow_mut(marker).iter().for_each(|ghost| {
            ghost
                .ghost_info
                .frames
                .iter()
                .zip(ghost.ghost_info.frames.iter().skip(1))
                .for_each(|(curr, next)| {
                    tri.color(
                        ghost.path_color[0],
                        ghost.path_color[1],
                        ghost.path_color[2],
                        ghost.path_color[3],
                    );
                    tri.vertex(curr.origin);
                    tri.vertex(next.origin);
                });
        });
    } else {
        let ghost_frametime = BXT_GHOST_FRAMETIME_OVERRIDE.as_f32(marker) as f64;
        let frametime = if ghost_frametime == 0. {
            None
        } else {
            Some(ghost_frametime)
        };

        GHOSTS.borrow_mut(marker).iter().for_each(|ghost| {
            let stop_frame = ghost
                .ghost_info
                .get_frame_index(TIME.get(marker) + ghost.offset, frametime);

            (0..(stop_frame - 1)).for_each(|frame_index| {
                tri.color(
                    ghost.path_color[0],
                    ghost.path_color[1],
                    ghost.path_color[2],
                    ghost.path_color[3],
                );
                tri.vertex(ghost.ghost_info.frames[frame_index].origin);
                tri.vertex(ghost.ghost_info.frames[frame_index + 1].origin);
            });
        });
    }

    tri.end();

    if let Some(gl) = gl.as_ref() {
        unsafe {
            gl.LineWidth(1.);
        }
    }
}

pub fn update_ghosts(marker: MainThreadMarker) {
    if !can_play_ghost(marker) {
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
            // spawn() uses STATE so we need to drop it first here.
            drop(state);

            ghosts.iter_mut().for_each(|ghost| {
                ghost.should_stop = false;
                unsafe { spawn(marker, ghost) }
            });

            // Done with spawning and start playing
            *STATE.borrow_mut(marker) = State::Playing;
        }
        // Will update ghost even when in pause. The only difference is the timer update.
        State::Playing | State::Paused => {
            let passed_time = unsafe { *engine::host_frametime.get(marker) };
            let is_engine_paused = unsafe { *engine::sv_paused.get(marker) };

            let passed_time = if matches!(*state, State::Playing) && is_engine_paused != 1 {
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
                        // (edict.v.angles).copy_from_slice(&frame.viewangles.to_array());
                        // only change yaw
                        edict.v.angles[1] = frame.viewangles[1];

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

                        edict.v.frame = (edict.v.frame + passed_time as f32) % 256.;

                        // This is how spastic the animation is.
                        edict.v.framerate = 1.;

                        // Inferred animation
                        // Z changes and previously on ground
                        if edict.v.origin[2] != ghost.last_origin[2]
                            && matches!(ghost.curr_anim, Animation::OnGround)
                        {
                            edict.v.sequence = 6;
                            edict.v.gaitsequence = 6; // ACT_HOP
                            ghost.curr_anim = Animation::InAir;
                            // edict.v.frame = 0.;
                        } else if edict.v.origin[2] == ghost.last_origin[2]
                            && matches!(ghost.curr_anim, Animation::InAir)
                        {
                            edict.v.sequence = 4;
                            edict.v.gaitsequence = 4; // ACT_RUN
                            ghost.curr_anim = Animation::OnGround;
                            // edict.v.frame = 0.;
                        }

                        // Actual animation from a demo
                        if let Some(anim) = frame.anim {
                            // For some reasons, sequence doesn't work as intended.
                            // if let Some(sequence) = anim.sequence {
                            //     edict.v.sequence = sequence as i32;
                            // }

                            if let Some(frame) = anim.frame {
                                edict.v.frame = frame;
                            }

                            if let Some(animtime) = anim.animtime {
                                edict.v.animtime = animtime;
                            }

                            if let Some(gaitsequence) = anim.gaitsequence {
                                // We don't have a way to attach weapon to a model so this is the
                                // way. walk (3) will have the
                                // player T-pose. So we make them run (4) instead.
                                let gaitsequence = if gaitsequence == 3 { 4 } else { gaitsequence };

                                edict.v.gaitsequence = gaitsequence;
                                edict.v.sequence = gaitsequence;
                            }

                            edict.v.blending = anim.blending;
                        }

                        ghost.last_origin = edict.v.origin;
                    }
                } else {
                    if let Some(edict) = &mut ghost.edict {
                        // stop animation
                        edict.v.framerate = 0.;
                        ghost.should_stop = true;
                    }
                }
            });

            // If playing, will increase the timer.
            // If paused, will not increase the timer.
            // By doing this, we can seek ahead.
            TIME.set(marker, TIME.get(marker) + passed_time);

            // Automatically set state to Stopped in case all ghosts are done.
            // This is to avoid doing bxt_ghost_stop then bxt_ghost_play again.
            let should_stop = ghosts.iter().all(|ghost| ghost.should_stop);
            if should_stop {
                *state = State::Stopped;
            }
        }
        // Stopped, do nothing
        // Player's viewangles and origin are not updated so player can move around.
        // Ideally, it should set TIME to 0. But preferrably the animation should freeze at the end.
        // Frozen animation at the end looks better.
        State::Stopped => (),
    }
}

pub fn on_cl_disconnect(marker: MainThreadMarker) {
    // Disconnecting will make it idling.
    // With this, we can spawn the entities again if we start a new map.
    // Otherwise, starting a new map will have State::Stop instead.
    *STATE.borrow_mut(marker) = State::Idle;
    TIME.set(marker, 0.);
}

fn can_play_ghost(marker: MainThreadMarker) -> bool {
    if !Ghost.is_enabled(marker) {
        return false;
    }

    // Not in a map
    if unsafe { (*engine::cls.get(marker)).state != 5 } {
        return false;
    }

    true
}
