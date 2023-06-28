//! The TAS optimizer.

use std::ffi::CStr;
use std::fs::{self, File};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use bxt_ipc_types::Frame;
use bxt_strafe::{Parameters, Player, State};
use glam::Vec3;
use hltas::HLTAS;

use self::objective::{AttemptResult, Constraint, ConstraintType, Direction, Objective, Variable};
use super::cvars::CVar;
use super::player_movement_tracing::Tracer;
use super::triangle_drawing::{self, TriangleApi};
use super::Module;
use crate::ffi::edict;
use crate::handler;
use crate::hooks::bxt;
use crate::hooks::engine::{self, con_print};
use crate::modules::commands::{self, Command};
use crate::utils::*;

mod optimizer;
use optimizer::Optimizer;

mod hltas_ext;

mod objective;

pub mod simulator;

mod remote;
pub use remote::{
    is_connected_to_server, maybe_start_client_connection_thread,
    update_client_connection_condition,
};

pub struct TasOptimizer;
impl Module for TasOptimizer {
    fn name(&self) -> &'static str {
        "TAS optimizer"
    }

    fn description(&self) -> &'static str {
        "Brute-force optimization for TASes."
    }

    fn commands(&self) -> &'static [&'static Command] {
        static COMMANDS: &[&Command] = &[
            &BXT_TAS_OPTIM_INIT,
            &BXT_TAS_OPTIM_DISABLE,
            &BXT_TAS_OPTIM_RESET,
            &BXT_TAS_OPTIM_START,
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
            &BXT_TAS_OPTIM_MULTIPLE_GAMES,
            &BXT_TAS_OPTIM_CONSTRAINT_VALUE,
            &BXT_TAS_OPTIM_CONSTRAINT_TYPE,
            &BXT_TAS_OPTIM_CONSTRAINT_VARIABLE,
            &BXT_TAS_OPTIM_DIRECTION,
            &BXT_TAS_OPTIM_VARIABLE,
            &BXT_TAS_OPTIM_RHAI_FILE,
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

static OPTIMIZER: MainThreadRefCell<Option<Optimizer>> = MainThreadRefCell::new(None);
static OPTIMIZE: MainThreadCell<bool> = MainThreadCell::new(false);
static OBJECTIVE: MainThreadRefCell<Objective> = MainThreadRefCell::new(Objective::Console {
    variable: Variable::PosX,
    direction: Direction::Maximize,
    constraint: None,
});

static OPTIM_STATS_LAST_PRINTED_AT: MainThreadCell<Option<Instant>> = MainThreadCell::new(None);
static OPTIM_STATS_ITERATIONS: MainThreadCell<usize> = MainThreadCell::new(0);
static OPTIM_STATS_ITERATIONS_INVALID: MainThreadCell<usize> = MainThreadCell::new(0);

static BXT_TAS_OPTIM_FRAMES: CVar = CVar::new(
    b"bxt_tas_optim_frames\0",
    b"0\0",
    "\
How much of the script, in number of frames, can be mutated in the single-frame mode.

Use when you want the tail of the script to remain unchanged, but still included in the \
optimization objective.",
);
static BXT_TAS_OPTIM_RANDOM_FRAMES_TO_CHANGE: CVar = CVar::new(
    b"bxt_tas_optim_random_frames_to_change\0",
    b"6\0",
    "Number of individual frames to mutate on every iteration in single-frame mode.",
);
static BXT_TAS_OPTIM_CHANGE_SINGLE_FRAMES: CVar = CVar::new(
    b"bxt_tas_optim_change_single_frames\0",
    b"0\0",
    "\
Set to `0` to make the optimizer mutate entire frame bulks at once. Set to `1` to make the \
optimizer mutate individual frames.

Generally `0` gives better results and produces a script ready to be copy-pasted. `1` can be \
useful for fine-tuning, e.g. if you're very close but barely not making the jump.",
);

static BXT_TAS_OPTIM_SIMULATION_ACCURACY: CVar = CVar::new(
    b"bxt_tas_optim_simulation_accuracy\0",
    b"0\0",
    "\
Set to `1` to enable whole-map player movement tracing.

This makes the optimization considerably slower, so only use it when `0` makes the optimizer path \
go through entities.",
);
static BXT_TAS_OPTIM_MULTIPLE_GAMES: CVar = CVar::new(
    b"bxt_tas_optim_multiple_games\0",
    b"0\0",
    "\
Set to `1` to use multi-game optimization.

When set to `1`, instead of prediction, the optimizer will use game instances launched in parallel \
to run the script. This results in 100% accurate simulation including entity interaction, but is \
much slower compared even to accurate prediction.

You need to start one or more game instances in addition to the one running the optimizer.",
);

static BXT_TAS_OPTIM_VARIABLE: CVar = CVar::new(
    b"bxt_tas_optim_variable\0",
    b"pos.x\0",
    "Variable to optimize. Can be `pos.x`, `pos.y`, `pos.z`, `vel.x`, `vel.y`, `vel.z` \
    or `speed`, which represents the horizontal speed.",
);
static BXT_TAS_OPTIM_DIRECTION: CVar = CVar::new(
    b"bxt_tas_optim_direction\0",
    b"maximize\0",
    "Direction to optimize `bxt_tas_optim_variable` towards. Can be `minimize` or `maximize`.",
);
static BXT_TAS_OPTIM_CONSTRAINT_VARIABLE: CVar = CVar::new(
    b"bxt_tas_optim_constraint_variable\0",
    b"\0",
    "\
Set to a variable to constrain it. Possible values are the same as `bxt_tas_optim_variable`.

A constraint is set by `bxt_tas_optim_constraint_variable`, `bxt_tas_optim_constraint_type` and \
`bxt_tas_optim_constraint_value`. It forces the variable to be less-than or greater-than the \
value. For example, you can set `pos.x < 300` or `speed > 500`. Then those brute-force attempts \
that do not satisfy this constraint get discarded.",
);
static BXT_TAS_OPTIM_CONSTRAINT_TYPE: CVar = CVar::new(
    b"bxt_tas_optim_constraint_type\0",
    b">\0",
    "Type of the constraint. Can be `<` or `>` for less-than and greater-than constraint, \
    respectively.",
);
static BXT_TAS_OPTIM_CONSTRAINT_VALUE: CVar = CVar::new(
    b"bxt_tas_optim_constraint_value\0",
    b"0\0",
    "Value to constraint against.",
);
static BXT_TAS_OPTIM_RHAI_FILE: CVar = CVar::new(
    b"bxt_tas_optim_rhai_file\0",
    b"\0",
    "\
Set to filename.rhai to use a Rhai script as the optimization objective.

The optimization objective can be set either with console variables (`bxt_tas_optim_variable`, \
`bxt_tas_optim_direction` and constraints), or as a [Rhai] script. The script should define three \
functions:

- `is_valid(curr)` that returns whether the brute-force attempt is valid (analogue of constraint),
- `is_better(curr, best)` that returns whether the brute-force attempt is better than the best one,
- `to_string(curr)` that returns a string representation of the optimization objective.

Here's an example script:

```
fn is_valid(curr) {
    // X pos < -3500
    curr.pos[0] < -3500
}

fn is_better(curr, best) {
    // New Y pos > best Y pos
    curr.pos[1] > best.pos[1]
}

fn to_string(curr) {
    // Need to convert to string manually at the moment
    curr.pos[0].to_string()
}
```

The script can also define a `should_pass_all_frames` variable set to `true` to receive an array \
of all simulated frames of the brute-force attempt, rather than just the last one. Note that this \
makes it considerably slower. Here's an example:

```
// Set this to true to get all frames rather than just one
let should_pass_all_frames = true;

fn is_valid(curr) {
    // You can use -1 to grab the last array element like in Python
    curr[-1].pos[0] < -3500
}

fn is_better(curr, best) {
    // Loop through all frames to find the highest Z we ever reached
    let best_z = best[0].pos[2];
    for player in best {
        if player.pos[2] > best_z {
            best_z = player.pos[2];
        }
    }

    let curr_z = curr[0].pos[2];
    for player in curr {
        if player.pos[2] > curr_z {
            curr_z = player.pos[2];
        }
    }

    curr_z > best_z
}

fn to_string(curr) {
    // You can reference variables that you set in is_better() here
    curr_z.to_string()
}
```

The Rhai language has [an online playground with examples](https://rhai.rs/playground/stable/), a \
VSCode extension and a reference which you can find on its website: https://rhai.rs/.

[Rhai]: https://rhai.rs/",
);

static BXT_TAS_OPTIM_INIT: Command = Command::new(
    b"_bxt_tas_optim_init\0",
    handler!(
        "_bxt_tas_optim_init <script.hltas> <frame number>

Initializes the optimization with the given script, starting from the given frame.

You're not meant to use this command directly. Instead, use `bxt_tas_studio_optim_init` (without \
arguments) if using TAS Studio, or `bxt_tas_optim_init` (without arguments) provided in \
Bunnymod XT, which sets the script name and frame number automatically.",
        optim_init as fn(_, _, _)
    ),
);

// TODO: make good
pub unsafe fn parameters(marker: MainThreadMarker) -> Parameters {
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

    // Safety: the reference does not outlive this block, and com_gamedir can only be modified
    // at engine start and while setting the HD models or the addon folder.
    let game_dir = engine::com_gamedir
        .get_opt(marker)
        .and_then(|dir| CStr::from_ptr(dir.cast()).to_str().ok())
        .unwrap_or("valve");
    let is_paranoia = game_dir == "paranoia";
    let is_cstrike = game_dir == "cstrike";
    let is_czero = game_dir == "czero";

    let max_speed = get_cvar_f32(marker, "sv_maxspeed").unwrap_or(320.);
    let client_max_speed = engine::pmove
        .get_opt(marker)
        .map(|pmove| (**pmove).clientmaxspeed)
        .unwrap_or(if is_paranoia { 100. } else { 0. });

    Parameters {
        frame_time: *engine::host_frametime.get(marker) as f32,
        max_velocity: get_cvar_f32(marker, "sv_maxvelocity").unwrap_or(2000.),
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
        max_speed: {
            if is_paranoia {
                max_speed * client_max_speed / 100.
            } else if client_max_speed != 0. {
                max_speed.min(client_max_speed)
            } else {
                max_speed
            }
        },
        bhop_cap_multiplier: {
            if is_cstrike || is_czero {
                0.8f32
            } else {
                0.65f32
            }
        },
        bhop_cap_max_speed_scale: {
            if is_cstrike || is_czero {
                1.2f32
            } else {
                1.7f32
            }
        },
        use_slow_down: !(is_cstrike || is_czero),
        has_stamina: (is_cstrike || is_czero)
            && get_cvar_f32(marker, "bxt_remove_stamina")
                .map(|x| x != 1.)
                .unwrap_or(true),
        duck_animation_slow_down: is_cstrike || is_czero,
    }
}

fn next_generation(marker: MainThreadMarker) -> u16 {
    static GENERATION: MainThreadCell<u16> = MainThreadCell::new(0);
    let generation = GENERATION.get(marker);
    GENERATION.set(marker, generation.wrapping_add(1));
    generation
}

fn optim_init(marker: MainThreadMarker, path: PathBuf, first_frame: usize) {
    if !TasOptimizer.is_enabled(marker) {
        return;
    }

    let script = match fs::read_to_string(path) {
        Ok(x) => x,
        Err(err) => {
            con_print(marker, &format!("Error reading the script: {err}\n"));
            return;
        }
    };

    let hltas = match HLTAS::from_str(&script) {
        Ok(x) => x,
        Err(err) => {
            con_print(marker, &format!("Error parsing the script: {err}\n"));
            return;
        }
    };

    // TODO: this function must be marked as unsafe. Getting the player data should be safe in a
    // console command callback, however by being safe, this function can be called from anywhere
    // else in the code.
    let Some(player) = (unsafe { player_data(marker) }) else {
        con_print(marker, "Cannot enable the TAS optimizer outside of gameplay.\n");
        return;
    };

    // TODO: get current parameters.
    let params = unsafe { parameters(marker) };

    // TODO: this is unsafe outside of gameplay.
    let tracer = unsafe { Tracer::new(marker, false) }.unwrap();

    let initial_frame = Frame {
        state: State::new(&tracer, params, player),
        parameters: params,
    };

    optim_init_internal(marker, hltas, first_frame, initial_frame);
}

pub fn optim_init_internal(
    marker: MainThreadMarker,
    hltas: HLTAS,
    first_frame: usize,
    initial_frame: Frame,
) {
    if !TasOptimizer.is_enabled(marker) {
        return;
    }

    *OPTIMIZER.borrow_mut(marker) = Some(Optimizer::new(
        hltas,
        first_frame,
        initial_frame,
        next_generation(marker),
    ));

    OPTIMIZE.set(marker, false);

    if let Err(err) = remote::start_server() {
        con_print(
            marker,
            &format!("Could not start a server for multi-game optimization: {err:?}"),
        );
    }
}

static BXT_TAS_OPTIM_DISABLE: Command = Command::new(
    b"bxt_tas_optim_disable\0",
    handler!(
        "bxt_tas_optim_disable

Stops and disables the optimizer.",
        optim_disable as fn(_)
    ),
);

fn optim_disable(marker: MainThreadMarker) {
    *OPTIMIZER.borrow_mut(marker) = None;
    OPTIMIZE.set(marker, false);
}

static BXT_TAS_OPTIM_RESET: Command = Command::new(
    b"bxt_tas_optim_reset\0",
    handler!(
        "bxt_tas_optim_reset

Resets the optimizer path back to the non-optimized starting state.

Use bxt_tas_optim_stop;bxt_tas_optim_reset after changing the optimization goal, or after toggling \
bxt_tas_optim_multiple_games, to start from scratch without having to replay the whole TAS.",
        optim_reset as fn(_)
    ),
);

fn optim_reset(marker: MainThreadMarker) {
    if let Some(optimizer) = &mut *OPTIMIZER.borrow_mut(marker) {
        optimizer.reset(next_generation(marker));
        OPTIMIZE.set(marker, false);
    } else {
        con_print(marker, "The optimizer is not initialized.\n");
    }
}

static BXT_TAS_OPTIM_START: Command = Command::new(
    b"bxt_tas_optim_start\0",
    handler!(
        "bxt_tas_optim_start

Starts the optimization.",
        optim_start as fn(_)
    ),
);

fn optim_start(marker: MainThreadMarker) {
    if OPTIMIZER.borrow(marker).is_none() {
        con_print(
            marker,
            "There's nothing to optimize. Call bxt_tas_optim_init first!\n",
        );
        return;
    }

    let mut set_with_script = false;
    let script_path = BXT_TAS_OPTIM_RHAI_FILE.to_os_string(marker);
    if !script_path.is_empty() {
        match fs::read_to_string(BXT_TAS_OPTIM_RHAI_FILE.to_os_string(marker)) {
            Ok(code) => {
                let engine = rhai::Engine::new();
                match engine.compile(code) {
                    Ok(ast) => {
                        let does_function_exist = |name, args: Vec<rhai::Dynamic>| {
                            let options = rhai::CallFnOptions::new()
                                .eval_ast(false)
                                .rewind_scope(false);
                            let rv = engine.call_fn_with_options::<rhai::Dynamic>(
                                options,
                                &mut rhai::Scope::new(),
                                &ast,
                                name,
                                args,
                            );

                            !matches!(
                                rv.as_ref().map_err(|err| &**err),
                                Err(rhai::EvalAltResult::ErrorFunctionNotFound(_, _))
                            )
                        };

                        if does_function_exist(
                            "is_better",
                            vec![rhai::Dynamic::UNIT, rhai::Dynamic::UNIT],
                        ) {
                            if does_function_exist("is_valid", vec![rhai::Dynamic::UNIT]) {
                                if does_function_exist("to_string", vec![rhai::Dynamic::UNIT]) {
                                    *OBJECTIVE.borrow_mut(marker) = Objective::Rhai { engine, ast };
                                    set_with_script = true;
                                } else {
                                    con_print(
                                        marker,
                                        "Rhai script missing to_string(curr) function.\n",
                                    );
                                    return;
                                }
                            } else {
                                con_print(marker, "Rhai script missing is_valid(curr) function.\n");
                                return;
                            }
                        } else {
                            con_print(
                                marker,
                                "Rhai script missing is_better(curr, best) function.\n",
                            );
                            return;
                        }
                    }
                    Err(err) => {
                        con_print(marker, &format!("Error parsing Rhai code: {err}\n"));
                        return;
                    }
                }
            }
            Err(err) => {
                con_print(
                    marker,
                    &format!(
                        "Could not read Rhai file `{}`: {err}\n",
                        script_path.to_string_lossy()
                    ),
                );
                return;
            }
        }
    }

    if !set_with_script {
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

        let constraint_variable = BXT_TAS_OPTIM_CONSTRAINT_VARIABLE.to_string(marker);
        let constraint = if !constraint_variable.is_empty() {
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

            Some(Constraint {
                variable,
                type_,
                constraint,
            })
        } else {
            None
        };

        *OBJECTIVE.borrow_mut(marker) = Objective::Console {
            variable,
            direction,
            constraint,
        };
    }

    OPTIMIZE.set(marker, true);

    OPTIM_STATS_LAST_PRINTED_AT.set(marker, Some(Instant::now()));
    OPTIM_STATS_ITERATIONS.set(marker, 0);
    OPTIM_STATS_ITERATIONS_INVALID.set(marker, 0);
}

static BXT_TAS_OPTIM_STOP: Command = Command::new(
    b"bxt_tas_optim_stop\0",
    handler!(
        "bxt_tas_optim_stop

Stops the optimization.",
        optim_stop as fn(_)
    ),
);

fn optim_stop(marker: MainThreadMarker) {
    OPTIMIZE.set(marker, false);
}

static BXT_TAS_OPTIM_SAVE: Command = Command::new(
    b"bxt_tas_optim_save\0",
    handler!(
        "bxt_tas_optim_save

Saves the optimized script.",
        optim_save as fn(_)
    ),
);

fn optim_save(marker: MainThreadMarker) {
    if let Some(optimizer) = &mut *OPTIMIZER.borrow_mut(marker) {
        // TODO: this is unsafe outside of gameplay.
        let tracer = unsafe { Tracer::new(marker, false) }.unwrap();
        optimizer.minimize(&tracer);

        optimizer
            .save(File::create("bxt-rs-optimization-best.hltas").unwrap())
            .unwrap();
    } else {
        con_print(
            marker,
            "There's nothing to save. Call bxt_tas_optim_init first!\n",
        );
    }
}

static BXT_TAS_OPTIM_MINIMIZE: Command = Command::new(
    b"bxt_tas_optim_minimize\0",
    handler!(
        "bxt_tas_optim_minimize

Minimizes the optimized script. This removes things like enabled autojump that does nothing \
because during this frame bulk the player never lands on the ground. It also joins together \
equivalent frame bulks.",
        optim_minimize as fn(_)
    ),
);

fn optim_minimize(marker: MainThreadMarker) {
    if let Some(optimizer) = &mut *OPTIMIZER.borrow_mut(marker) {
        // TODO: this is unsafe outside of gameplay.
        let tracer = unsafe { Tracer::new(marker, false) }.unwrap();
        optimizer.minimize(&tracer);
    } else {
        con_print(
            marker,
            "There's nothing to minimize. Call bxt_tas_optim_init first!\n",
        );
    }
}

static BXT_TAS_OPTIM_SIMULATION_START_RECORDING_FRAMES: Command = Command::new(
    b"_bxt_tas_optim_simulation_start_recording_frames\0",
    handler!(
        "_bxt_tas_optim_simulation_start_recording_frames

Starts recording frames to send to the remote server.

You're not meant to use this command. It's run automatically by bxt-rs in simulator clients.",
        optim_simulation_start_recording_frames as fn(_)
    ),
);

fn optim_simulation_start_recording_frames(_marker: MainThreadMarker) {
    remote::start_recording_frames();
}

static BXT_TAS_OPTIM_SIMULATION_DONE: Command = Command::new(
    b"_bxt_tas_optim_simulation_done\0",
    handler!(
        "_bxt_tas_optim_simulation_done

Sends simulated frames to the remote server.

You're not meant to use this command. It's run automatically by bxt-rs in simulator clients.",
        optim_simulation_done as fn(_)
    ),
);

fn optim_simulation_done(_marker: MainThreadMarker) {
    remote::send_simulation_result_to_server();
}

pub unsafe fn current_best(marker: MainThreadMarker) -> Option<HLTAS> {
    let Some(ref mut optimizer) = &mut *OPTIMIZER.borrow_mut(marker) else { return None };

    // TODO: this is unsafe outside of gameplay.
    let tracer = unsafe { Tracer::new(marker, false) }.unwrap();
    optimizer.minimize(&tracer);

    Some(optimizer.current_best())
}

pub unsafe fn maybe_receive_messages_from_remote_server(marker: MainThreadMarker) {
    let Some(cls) = engine::cls.get_opt(marker) else { return };

    let client_state = (*cls).state;
    if client_state != 1 && client_state != 5 {
        return;
    }

    if let Some(hltas) = remote::receive_new_hltas_to_simulate() {
        engine::prepend_command(
            marker,
            "volume 0;MP3Volume 0;bxt_tas_write_log 0;bxt_tas_norefresh_until_last_frames 1\n",
        );

        bxt::tas_load_script(marker, &hltas);
    }
}

pub unsafe fn on_cmd_start(marker: MainThreadMarker) {
    remote::on_frame_simulated(|| {
        let player = player_data(marker).unwrap();

        let params = parameters(marker);

        let tracer = Tracer::new(marker, false).unwrap();

        Frame {
            state: State::new(&tracer, params, player),
            parameters: params,
        }
    });
}

pub unsafe fn player_data(marker: MainThreadMarker) -> Option<Player> {
    // SAFETY: we're not calling any engine functions while the reference is alive.
    let edict = engine::player_edict(marker)?.as_ref();

    Some(Player {
        pos: Vec3::from(edict.v.origin),
        vel: Vec3::from(edict.v.velocity),
        base_vel: Vec3::from(edict.v.basevelocity),
        ducking: edict.v.flags.contains(edict::Flags::FL_DUCKING),
        in_duck_animation: edict.v.bInDuck != 0,
        duck_time: edict.v.flDuckTime,
        stamina_time: edict.v.fuser2,
    })
}

pub fn draw(marker: MainThreadMarker, tri: &TriangleApi) {
    if let Some(optimizer) = &mut *OPTIMIZER.borrow_mut(marker) {
        if BXT_TAS_OPTIM_MULTIPLE_GAMES.as_bool(marker) {
            if OPTIMIZE.get(marker) {
                optimizer.optimize_with_remote_clients(
                    BXT_TAS_OPTIM_FRAMES.as_u64(marker) as usize,
                    BXT_TAS_OPTIM_RANDOM_FRAMES_TO_CHANGE.as_u64(marker) as usize,
                    BXT_TAS_OPTIM_CHANGE_SINGLE_FRAMES.as_bool(marker),
                    &OBJECTIVE.borrow(marker),
                    |value| {
                        con_print(marker, &format!("Found new best value: {value}\n"));
                    },
                );
            } else {
                optimizer.maybe_simulate_all_in_remote_client();
                optimizer.poll_remote_clients_when_not_optimizing();
            }
        } else {
            // SAFETY: if we have access to TriangleApi, it's safe to do player tracing too.
            let tracer =
                unsafe { Tracer::new(marker, BXT_TAS_OPTIM_SIMULATION_ACCURACY.as_bool(marker)) }
                    .unwrap();

            if OPTIMIZE.get(marker) {
                if let Some(optimizer) = optimizer.optimize(
                    &tracer,
                    BXT_TAS_OPTIM_FRAMES.as_u64(marker) as usize,
                    BXT_TAS_OPTIM_RANDOM_FRAMES_TO_CHANGE.as_u64(marker) as usize,
                    BXT_TAS_OPTIM_CHANGE_SINGLE_FRAMES.as_bool(marker),
                    &OBJECTIVE.borrow(marker),
                ) {
                    let start = Instant::now();

                    for result in optimizer {
                        match result {
                            AttemptResult::Better { value } => {
                                con_print(marker, &format!("Found new best value: {value}\n"));
                            }
                            AttemptResult::Invalid => {
                                OPTIM_STATS_ITERATIONS_INVALID
                                    .set(marker, OPTIM_STATS_ITERATIONS_INVALID.get(marker) + 1);
                            }
                            _ => (),
                        }

                        OPTIM_STATS_ITERATIONS.set(marker, OPTIM_STATS_ITERATIONS.get(marker) + 1);

                        if start.elapsed() > Duration::from_millis(40) {
                            break;
                        }
                    }
                }

                let now = Instant::now();
                if now - OPTIM_STATS_LAST_PRINTED_AT.get(marker).unwrap() >= Duration::from_secs(1)
                {
                    let iterations = OPTIM_STATS_ITERATIONS.get(marker);
                    let invalid = OPTIM_STATS_ITERATIONS_INVALID.get(marker);
                    eprintln!(
                        "Optim: {} it/s ({:.1}% invalid)",
                        iterations,
                        if iterations == 0 {
                            0.
                        } else {
                            invalid as f32 * 100. / iterations as f32
                        },
                    );

                    OPTIM_STATS_LAST_PRINTED_AT.set(marker, Some(now));
                    OPTIM_STATS_ITERATIONS.set(marker, 0);
                    OPTIM_STATS_ITERATIONS_INVALID.set(marker, 0);
                }
            }

            // Make sure the state is ready for drawing.
            optimizer.simulate_all(&tracer);
        }

        optimizer.draw(tri);
    }
}
