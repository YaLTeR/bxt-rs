//! The TAS editor.

use std::fs::{self, File};
use std::path::PathBuf;
use std::rc::Rc;

use bxt_strafe::{Parameters, Player, State};
use glam::Vec3;
use hltas::HLTAS;
use mlua::Lua;

use self::editor::{Constraint, ConstraintType, Direction, Frame, OptimizationGoal, Variable};
use super::cvars::CVar;
use super::triangle_drawing::{self, TriangleApi};
use super::Module;
use crate::ffi::edict;
use crate::handler;
use crate::hooks::bxt;
use crate::hooks::engine::{self, con_print};
use crate::modules::commands::{self, Command};
use crate::utils::*;

mod editor;
use editor::Editor;

mod tracer;
use tracer::Tracer;

mod remote;
pub use remote::{
    is_connected_to_server, maybe_start_client_connection_thread,
    update_client_connection_condition,
};

pub struct TasEditor;
impl Module for TasEditor {
    fn name(&self) -> &'static str {
        "TAS editor"
    }

    fn commands(&self) -> &'static [&'static Command] {
        static COMMANDS: &[&Command] = &[
            &BXT_TAS_OPTIM_INIT,
            &BXT_TAS_OPTIM_RUN,
            &BXT_TAS_OPTIM_STOP,
            &BXT_TAS_OPTIM_SAVE,
            &BXT_TAS_OPTIM_MINIMIZE,
            &BXT_TAS_OPTIM_SIMULATION_START_RECORDING_FRAMES,
            &BXT_TAS_OPTIM_SIMULATION_DONE,
        ];
        COMMANDS
    }

    fn cvars(&self) -> &'static [&'static CVar] {
        static CVARS: &[&CVar] = &[
            &BXT_TAS_OPTIM_RANDOM_FRAMES_TO_CHANGE,
            &BXT_TAS_OPTIM_CHANGE_SINGLE_FRAMES,
            &BXT_TAS_OPTIM_FRAMES,
            &BXT_TAS_OPTIM_SIMULATION_ACCURACY,
            &BXT_TAS_OPTIM_CONSTRAINT_VALUE,
            &BXT_TAS_OPTIM_CONSTRAINT_TYPE,
            &BXT_TAS_OPTIM_CONSTRAINT_VARIABLE,
            &BXT_TAS_OPTIM_DIRECTION,
            &BXT_TAS_OPTIM_VARIABLE,
            &BXT_TAS_OPTIM_LUA_FILE,
        ];
        CVARS
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        commands::Commands.is_enabled(marker)
            && triangle_drawing::TriangleDrawing.is_enabled(marker)
            && engine::svs.is_set(marker)
            && engine::host_frametime.is_set(marker)
    }
}

static EDITOR: MainThreadRefCell<Option<Editor>> = MainThreadRefCell::new(None);
static OPTIMIZE: MainThreadCell<bool> = MainThreadCell::new(false);
static GOAL: MainThreadRefCell<OptimizationGoal> =
    MainThreadRefCell::new(OptimizationGoal::Console {
        variable: Variable::PosX,
        direction: Direction::Maximize,
    });
static CONSTRAINT: MainThreadRefCell<Option<Constraint>> = MainThreadRefCell::new(None);

static BXT_TAS_OPTIM_FRAMES: CVar = CVar::new(b"bxt_tas_optim_frames\0", b"0\0");
static BXT_TAS_OPTIM_RANDOM_FRAMES_TO_CHANGE: CVar =
    CVar::new(b"bxt_tas_optim_random_frames_to_change\0", b"6\0");
static BXT_TAS_OPTIM_CHANGE_SINGLE_FRAMES: CVar =
    CVar::new(b"bxt_tas_optim_change_single_frames\0", b"0\0");

static BXT_TAS_OPTIM_SIMULATION_ACCURACY: CVar =
    CVar::new(b"bxt_tas_optim_simulation_accuracy\0", b"0\0");

static BXT_TAS_OPTIM_VARIABLE: CVar = CVar::new(b"bxt_tas_optim_variable\0", b"pos.x\0");
static BXT_TAS_OPTIM_DIRECTION: CVar = CVar::new(b"bxt_tas_optim_direction\0", b"maximize\0");
static BXT_TAS_OPTIM_CONSTRAINT_VARIABLE: CVar =
    CVar::new(b"bxt_tas_optim_constraint_variable\0", b"\0");
static BXT_TAS_OPTIM_CONSTRAINT_TYPE: CVar = CVar::new(b"bxt_tas_optim_constraint_type\0", b">\0");
static BXT_TAS_OPTIM_CONSTRAINT_VALUE: CVar =
    CVar::new(b"bxt_tas_optim_constraint_value\0", b"0\0");
static BXT_TAS_OPTIM_LUA_FILE: CVar = CVar::new(b"bxt_tas_optim_lua_file\0", b"\0");

static BXT_TAS_OPTIM_INIT: Command = Command::new(
    b"_bxt_tas_optim_init\0",
    handler!(
        "Usage: _bxt_tas_optim_init <script.hltas> <frame number>\n \
          Initializes the optimization with the given script, starting from the given frame.\n",
        optim_init as fn(_, _, _)
    ),
);

unsafe fn get_cvar_f32(marker: MainThreadMarker, name: &str) -> Option<f32> {
    let mut ptr = *engine::cvar_vars.get(marker);
    while !ptr.is_null() {
        match std::ffi::CStr::from_ptr((*ptr).name).to_str() {
            Ok(x) if x == name => {
                return Some((*ptr).value);
            }
            _ => (),
        }

        ptr = (*ptr).next;
    }

    warn!("couldn't find cvar {}", name);
    None
}

fn optim_init(marker: MainThreadMarker, path: PathBuf, first_frame: usize) {
    if !TasEditor.is_enabled(marker) {
        return;
    }

    if path.as_os_str().is_empty() {
        *EDITOR.borrow_mut(marker) = None;
        OPTIMIZE.set(marker, false);
        return;
    }

    let script = match fs::read_to_string(path) {
        Ok(x) => x,
        Err(err) => {
            con_print(marker, &format!("Error reading the script: {}\n", err));
            return;
        }
    };

    let hltas = match HLTAS::from_str(&script) {
        Ok(x) => x,
        Err(err) => {
            con_print(marker, &format!("Error parsing the script: {}\n", err));
            return;
        }
    };

    // TODO: this function must be marked as unsafe. Getting the player data should be safe in a
    // console command callback, however by being safe, this function can be called from anywhere
    // else in the code.
    let player = match unsafe { player_data(marker) } {
        Some(x) => x,
        None => {
            con_print(
                marker,
                "Cannot enable the TAS editor outside of gameplay.\n",
            );
            return;
        }
    };

    // TODO: get current parameters.
    let parameters = unsafe {
        Parameters {
            frame_time: *engine::host_frametime.get(marker) as f32,
            max_velocity: get_cvar_f32(marker, "sv_maxvelocity").unwrap_or(2000.),
            max_speed: get_cvar_f32(marker, "sv_maxspeed").unwrap_or(320.),
            stop_speed: get_cvar_f32(marker, "sv_stopspeed").unwrap_or(100.),
            friction: get_cvar_f32(marker, "sv_friction").unwrap_or(4.),
            edge_friction: get_cvar_f32(marker, "edgefriction").unwrap_or(2.),
            ent_friction: engine::player_edict(marker)
                .map(|x| x.as_ref().v.friction)
                .unwrap_or(1.),
            accelerate: get_cvar_f32(marker, "sv_accelerate").unwrap_or(10.),
            air_accelerate: get_cvar_f32(marker, "sv_airaccelerate").unwrap_or(10.),
            gravity: get_cvar_f32(marker, "sv_gravity").unwrap_or(800.),
            ent_gravity: engine::player_edict(marker)
                .map(|x| x.as_ref().v.gravity)
                .unwrap_or(1.),
            step_size: get_cvar_f32(marker, "sv_stepsize").unwrap_or(18.),
            bounce: get_cvar_f32(marker, "sv_bounce").unwrap_or(1.),
            bhop_cap: get_cvar_f32(marker, "bxt_bhopcap").unwrap_or(0.) != 0.,
        }
    };

    // TODO: this is unsafe outside of gameplay.
    let tracer = unsafe { Tracer::new(marker, false) }.unwrap();

    let initial_frame = Frame {
        state: State::new(&tracer, parameters, player),
        parameters,
    };

    static GENERATION: MainThreadCell<u16> = MainThreadCell::new(0);
    let generation = GENERATION.get(marker);
    GENERATION.set(marker, generation.wrapping_add(1));

    *EDITOR.borrow_mut(marker) = Some(Editor::new(hltas, first_frame, initial_frame, generation));

    if let Err(err) = remote::start_server() {
        con_print(
            marker,
            &format!("Could not start a server for multi-game optimization: {err:?}"),
        );
    }
}

static BXT_TAS_OPTIM_RUN: Command = Command::new(
    b"bxt_tas_optim_run\0",
    handler!(
        "Usage: bxt_tas_optim_run\n \
          Starts the optimization.\n",
        optim_run as fn(_)
    ),
);

fn optim_run(marker: MainThreadMarker) {
    if EDITOR.borrow(marker).is_none() {
        con_print(
            marker,
            "There's nothing to optimize. Call _bxt_tas_optim_init first!\n",
        );
        return;
    }

    let mut set_with_lua = false;
    let lua_path = BXT_TAS_OPTIM_LUA_FILE.to_os_string(marker);
    if !lua_path.is_empty() {
        match fs::read_to_string(BXT_TAS_OPTIM_LUA_FILE.to_os_string(marker)) {
            Ok(code) => {
                let lua = Lua::new();
                match lua.load(&code).exec() {
                    Ok(()) => {
                        if lua.globals().get::<_, mlua::Function>("is_better").is_ok() {
                            if lua.globals().get::<_, mlua::Function>("to_string").is_ok() {
                                if lua.globals().get::<_, mlua::Function>("is_valid").is_ok() {
                                    let lua = Rc::new(lua);
                                    *GOAL.borrow_mut(marker) = OptimizationGoal::Lua(lua.clone());
                                    *CONSTRAINT.borrow_mut(marker) = Some(Constraint::Lua(lua));
                                    set_with_lua = true;
                                } else {
                                    con_print(marker, "Lua code missing is_valid () function.\n");
                                    return;
                                }
                            } else {
                                con_print(marker, "Lua code missing to_string () function.\n");
                                return;
                            }
                        } else {
                            con_print(marker, "Lua code missing is_better () function.\n");
                            return;
                        }
                    }
                    Err(err) => {
                        con_print(marker, &format!("Error evaluating Lua code: {err}\n"));
                        return;
                    }
                }
            }
            Err(err) => {
                con_print(
                    marker,
                    &format!(
                        "Could not read Lua file `{}`: {err}\n",
                        lua_path.to_string_lossy()
                    ),
                );
                return;
            }
        }
    }

    if !set_with_lua {
        let variable = match BXT_TAS_OPTIM_VARIABLE.to_string(marker).parse::<Variable>() {
            Ok(x) => x,
            Err(_) => {
                con_print(
                    marker,
                    "Could not parse bxt_tas_optim_variable. \
                    Valid values are pos.x, pos.y, pos.z, vel.x, vel.y, vel.z and speed.\n",
                );
                return;
            }
        };

        let direction = match BXT_TAS_OPTIM_DIRECTION
            .to_string(marker)
            .parse::<Direction>()
        {
            Ok(x) => x,
            Err(_) => {
                con_print(
                    marker,
                    "Could not parse bxt_tas_optim_direction. \
                    Valid values are maximize and minimize.\n",
                );
                return;
            }
        };

        *GOAL.borrow_mut(marker) = OptimizationGoal::Console {
            variable,
            direction,
        };

        let constraint_variable = BXT_TAS_OPTIM_CONSTRAINT_VARIABLE.to_string(marker);
        if !constraint_variable.is_empty() {
            let variable = if let Ok(x) = BXT_TAS_OPTIM_CONSTRAINT_VARIABLE
                .to_string(marker)
                .parse::<Variable>()
            {
                x
            } else {
                con_print(
                    marker,
                    "Could not parse bxt_tas_optim_constraint_variable. \
                    Valid values are \"\" (to disable), pos.x, pos.y, pos.z, vel.x, vel.y, vel.z \
                    and speed.\n",
                );
                return;
            };

            let type_ = if let Ok(x) = BXT_TAS_OPTIM_CONSTRAINT_TYPE
                .to_string(marker)
                .parse::<ConstraintType>()
            {
                x
            } else {
                con_print(
                    marker,
                    "Could not parse bxt_tas_optim_constraint_type. \
                    Valid values are > and <.\n",
                );
                return;
            };

            let constraint = if let Ok(x) = BXT_TAS_OPTIM_CONSTRAINT_VALUE
                .to_string(marker)
                .parse::<f32>()
            {
                x
            } else {
                con_print(
                    marker,
                    "Could not parse bxt_tas_optim_constraint_value as a number.\n",
                );
                return;
            };

            *CONSTRAINT.borrow_mut(marker) = Some(Constraint::Console {
                variable,
                type_,
                constraint,
            });
        } else {
            *CONSTRAINT.borrow_mut(marker) = None;
        }
    }

    OPTIMIZE.set(marker, true);
}

static BXT_TAS_OPTIM_STOP: Command = Command::new(
    b"bxt_tas_optim_stop\0",
    handler!(
        "Usage: bxt_tas_optim_stop\n \
          Starts the optimization.\n",
        optim_stop as fn(_)
    ),
);

fn optim_stop(marker: MainThreadMarker) {
    OPTIMIZE.set(marker, false);
}

static BXT_TAS_OPTIM_SAVE: Command = Command::new(
    b"bxt_tas_optim_save\0",
    handler!(
        "Usage: bxt_tas_optim_save\n \
          Saves the optimized script.\n",
        optim_save as fn(_)
    ),
);

fn optim_save(marker: MainThreadMarker) {
    if let Some(editor) = &mut *EDITOR.borrow_mut(marker) {
        editor
            .save(File::create("bxt-rs-optimization-best.hltas").unwrap())
            .unwrap();
    } else {
        con_print(
            marker,
            "There's nothing to save. Call _bxt_tas_optim_init first!\n",
        );
    }
}

static BXT_TAS_OPTIM_MINIMIZE: Command = Command::new(
    b"bxt_tas_optim_minimize\0",
    handler!(
        "Usage: bxt_tas_optim_minimize\n \
          Minimizes the optimized script.\n",
        optim_minimize as fn(_)
    ),
);

fn optim_minimize(marker: MainThreadMarker) {
    if let Some(editor) = &mut *EDITOR.borrow_mut(marker) {
        // TODO: this is unsafe outside of gameplay.
        let tracer = unsafe { Tracer::new(marker, false) }.unwrap();
        editor.minimize(&tracer);
    } else {
        con_print(
            marker,
            "There's nothing to minimize. Call _bxt_tas_optim_init first!\n",
        );
    }
}

static BXT_TAS_OPTIM_SIMULATION_START_RECORDING_FRAMES: Command = Command::new(
    b"_bxt_tas_optim_simulation_start_recording_frames\0",
    handler!(
        "Usage: _bxt_tas_optim_simulation_start_recording_frames\n \
          Starts recording frames to send to the remote server.\n",
        optim_simulation_start_recording_frames as fn(_)
    ),
);

fn optim_simulation_start_recording_frames(_marker: MainThreadMarker) {
    remote::start_recording_frames();
}

static BXT_TAS_OPTIM_SIMULATION_DONE: Command = Command::new(
    b"_bxt_tas_optim_simulation_done\0",
    handler!(
        "Usage: _bxt_tas_optim_simulation_done\n \
          Sends simulated frames to the remote server.\n",
        optim_simulation_done as fn(_)
    ),
);

fn optim_simulation_done(_marker: MainThreadMarker) {
    remote::send_simulation_result_to_server();
}

pub unsafe fn maybe_receive_messages_from_remote_server(marker: MainThreadMarker) {
    let cls = match engine::cls.get_opt(marker) {
        Some(x) => x,
        None => return,
    };

    let client_state = (*cls).state;
    if client_state != 1 && client_state != 5 {
        return;
    }

    if let Some(hltas) = remote::receive_new_hltas_to_simulate() {
        engine::prepend_command(
            marker,
            "sensitivity 0;volume 0;MP3Volume 0;bxt_tas_write_log 0;bxt_tas_norefresh_until_last_frames 1\n",
        );

        bxt::tas_load_script(marker, &hltas);
    }
}

pub unsafe fn on_cmd_start(marker: MainThreadMarker) {
    remote::on_frame_simulated(|| {
        let player = player_data(marker).unwrap();

        let parameters = Parameters {
            frame_time: *engine::host_frametime.get(marker) as f32,
            max_velocity: get_cvar_f32(marker, "sv_maxvelocity").unwrap_or(2000.),
            max_speed: get_cvar_f32(marker, "sv_maxspeed").unwrap_or(320.),
            stop_speed: get_cvar_f32(marker, "sv_stopspeed").unwrap_or(100.),
            friction: get_cvar_f32(marker, "sv_friction").unwrap_or(4.),
            edge_friction: get_cvar_f32(marker, "edgefriction").unwrap_or(2.),
            ent_friction: 1.,
            accelerate: get_cvar_f32(marker, "sv_accelerate").unwrap_or(10.),
            air_accelerate: get_cvar_f32(marker, "sv_airaccelerate").unwrap_or(10.),
            gravity: get_cvar_f32(marker, "sv_gravity").unwrap_or(800.),
            ent_gravity: 1.,
            step_size: get_cvar_f32(marker, "sv_stepsize").unwrap_or(18.),
            bounce: get_cvar_f32(marker, "sv_bounce").unwrap_or(1.),
            bhop_cap: get_cvar_f32(marker, "bxt_bhopcap").unwrap_or(0.) != 0.,
        };

        let tracer = Tracer::new(marker, false).unwrap();

        Frame {
            state: State::new(&tracer, parameters, player),
            parameters,
        }
    });
}

unsafe fn player_data(marker: MainThreadMarker) -> Option<Player> {
    // SAFETY: we're not calling any engine functions while the reference is alive.
    let edict = engine::player_edict(marker)?.as_ref();

    Some(Player {
        pos: Vec3::from(edict.v.origin),
        vel: Vec3::from(edict.v.velocity),
        base_vel: Vec3::from(edict.v.basevelocity),
        ducking: edict.v.flags.contains(edict::Flags::FL_DUCKING),
        in_duck_animation: edict.v.bInDuck != 0,
        duck_time: edict.v.flDuckTime,
    })
}

pub fn draw(marker: MainThreadMarker, tri: &TriangleApi) {
    if let Some(editor) = &mut *EDITOR.borrow_mut(marker) {
        if BXT_TAS_OPTIM_SIMULATION_ACCURACY.as_u64(marker) == 2 {
            if OPTIMIZE.get(marker) {
                editor.optimize_with_remote_clients(
                    BXT_TAS_OPTIM_FRAMES.as_u64(marker) as usize,
                    BXT_TAS_OPTIM_RANDOM_FRAMES_TO_CHANGE.as_u64(marker) as usize,
                    BXT_TAS_OPTIM_CHANGE_SINGLE_FRAMES.as_bool(marker),
                    &*GOAL.borrow(marker),
                    CONSTRAINT.borrow(marker).as_ref(),
                    |value| {
                        con_print(marker, &format!("Found new best value: {value}\n"));
                    },
                );
            } else {
                editor.maybe_simulate_all_in_remote_client();
                editor.poll_remote_clients_when_not_optimizing();
            }
        } else {
            // SAFETY: if we have access to TriangleApi, it's safe to do player tracing too.
            let tracer =
                unsafe { Tracer::new(marker, BXT_TAS_OPTIM_SIMULATION_ACCURACY.as_bool(marker)) }
                    .unwrap();

            if OPTIMIZE.get(marker) {
                editor.optimize(
                    &tracer,
                    BXT_TAS_OPTIM_FRAMES.as_u64(marker) as usize,
                    BXT_TAS_OPTIM_RANDOM_FRAMES_TO_CHANGE.as_u64(marker) as usize,
                    BXT_TAS_OPTIM_CHANGE_SINGLE_FRAMES.as_bool(marker),
                    &*GOAL.borrow(marker),
                    CONSTRAINT.borrow(marker).as_ref(),
                    |value| {
                        con_print(marker, &format!("Found new best value: {value}\n"));
                    },
                );
            }

            // Make sure the state is ready for drawing.
            editor.simulate_all(&tracer);
        }

        editor.draw(tri);
    }
}
