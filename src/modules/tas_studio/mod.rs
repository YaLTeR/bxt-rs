//! Interactive editor for TASes.

use std::ffi::CStr;
use std::fs::{self, read_to_string};
use std::io::Write;
use std::iter::zip;
use std::mem;
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use bxt_ipc_types::Frame;
use bxt_strafe::{Parameters, TraceResult};
use color_eyre::eyre::{self, eyre, Context};
use glam::{IVec2, IVec4, Vec2, Vec3};
use hltas::types::{
    AutoMovement, DuckBeforeCollision, FrameBulk, LeaveGroundAction, LeaveGroundActionSpeed,
    LeaveGroundActionType, StrafeDir, StrafeSettings, StrafeType,
};
use hltas::HLTAS;

use self::editor::operation::Key;
use self::editor::toggle_auto_action::ToggleAutoActionTarget;
use self::editor::utils::{bulk_and_first_frame_idx, FrameBulkExt};
use self::editor::KeyboardState;
use self::remote::{AccurateFrame, PlayRequest};
use super::commands::{Command, Commands};
use super::cvars::CVar;
use super::hud::Hud;
use super::player_movement_tracing::{PlayerMovementTracing, Tracer};
use super::tas_optimizer::{self, optim_init_internal, parameters, player_data};
use super::triangle_drawing::{TriangleApi, TriangleDrawing};
use super::{hud, Module};
use crate::ffi::buttons::Buttons;
use crate::ffi::cvar::cvar_s;
use crate::ffi::usercmd::usercmd_s;
use crate::handler;
use crate::hooks::bxt::{OnTasPlaybackFrameData, BXT_IS_TAS_EDITOR_ACTIVE};
use crate::hooks::engine::con_print;
use crate::hooks::{bxt, client, engine, sdl};
use crate::modules::tas_studio::editor::MaxAccelYawOffsetMode;
use crate::utils::*;

pub struct TasStudio;
impl Module for TasStudio {
    fn name(&self) -> &'static str {
        "TAS studio"
    }

    fn description(&self) -> &'static str {
        "Interactive editor for TASes."
    }

    fn cvars(&self) -> &'static [&'static CVar] {
        static CVARS: &[&CVar] = &[
            &BXT_HUD_TAS_STUDIO,
            &BXT_TAS_STUDIO_CAMERA_EDITOR,
            &BXT_TAS_STUDIO_AUTO_SMOOTHING,
            &BXT_TAS_STUDIO_SHOW_PLAYER_BBOX,
            &BXT_TAS_STUDIO_SMOOTH_WINDOW_S,
            &BXT_TAS_STUDIO_SMOOTH_SMALL_WINDOW_S,
            &BXT_TAS_STUDIO_SMOOTH_SMALL_WINDOW_MULTIPLIER,
            &BXT_TAS_STUDIO_LINE_WIDTH,
        ];
        CVARS
    }

    fn commands(&self) -> &'static [&'static Command] {
        static COMMANDS: &[&Command] = &[
            &BXT_TAS_STUDIO_CONVERT_HLTAS_FROM_BXT_TAS_NEW,
            &BXT_TAS_STUDIO_SMOOTH_GLOBALLY,
            &BXT_TAS_STUDIO_NEW,
            &BXT_TAS_STUDIO_LOAD,
            &BXT_TAS_STUDIO_CONVERT_HLTAS,
            &BXT_TAS_STUDIO_REPLAY,
            &BXT_TAS_STUDIO_SET_STOP_FRAME,
            &BXT_TAS_STUDIO_SET_YAWSPEED,
            &BXT_TAS_STUDIO_SET_PITCH,
            &BXT_TAS_STUDIO_SET_YAW,
            &BXT_TAS_STUDIO_SET_FRAME_TIME,
            &BXT_TAS_STUDIO_SET_COMMANDS,
            &BXT_TAS_STUDIO_UNSET_PITCH,
            &BXT_TAS_STUDIO_UNSET_YAW,
            &BXT_TAS_STUDIO_SELECT_NEXT,
            &BXT_TAS_STUDIO_SELECT_PREV,
            &BXT_TAS_STUDIO_SPLIT,
            &BXT_TAS_STUDIO_DELETE,
            &BXT_TAS_STUDIO_DELETE_LAST,
            &BXT_TAS_STUDIO_TOGGLE,
            &BXT_TAS_STUDIO_HIDE,
            &BXT_TAS_STUDIO_SMOOTH,
            &BXT_TAS_STUDIO_BRANCH_CLONE,
            &BXT_TAS_STUDIO_BRANCH_FOCUS_ID,
            &BXT_TAS_STUDIO_BRANCH_FOCUS_NEXT,
            &BXT_TAS_STUDIO_BRANCH_HIDE_ID,
            &BXT_TAS_STUDIO_BRANCH_HIDE_AND_FOCUS_NEXT,
            &BXT_TAS_STUDIO_BRANCH_SHOW_ID,
            &BXT_TAS_STUDIO_UNDO,
            &BXT_TAS_STUDIO_REDO,
            &BXT_TAS_STUDIO_CLOSE,
            &BXT_TAS_STUDIO_OPTIM_INIT,
            &BXT_TAS_STUDIO_OPTIM_APPLY,
            &PLUS_BXT_TAS_STUDIO_INSERT_CAMERA_LINE,
            &MINUS_BXT_TAS_STUDIO_INSERT_CAMERA_LINE,
            &PLUS_BXT_TAS_STUDIO_LOOK_AROUND,
            &MINUS_BXT_TAS_STUDIO_LOOK_AROUND,
        ];
        COMMANDS
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        engine::Host_FilterTime.is_set(marker)
            && engine::Cbuf_InsertText.is_set(marker)
            && engine::hudGetViewAngles.is_set(marker)
            && engine::window_rect.is_set(marker)
            && engine::cls.is_set(marker)
            && sdl::SDL_SetRelativeMouseMode.is_set(marker)
            && sdl::SDL_GetMouseState.is_set(marker)
            && bxt::BXT_TAS_LOAD_SCRIPT_FROM_STRING.is_set(marker)
            && bxt::BXT_IS_TAS_EDITOR_ACTIVE.is_set(marker)
            && bxt::BXT_ON_TAS_PLAYBACK_FRAME.is_set(marker)
            && bxt::BXT_ON_TAS_PLAYBACK_STOPPED.is_set(marker)
            && bxt::BXT_TAS_NEW.is_set(marker)
            && bxt::BXT_TAS_NOREFRESH_UNTIL_LAST_FRAMES.is_set(marker)
            && bxt::BXT_TAS_STUDIO_NOREFRESH_OVERRIDE.is_set(marker)
            && TriangleDrawing.is_enabled(marker)
            && Commands.is_enabled(marker)
            && PlayerMovementTracing.is_enabled(marker)
            && Hud.is_enabled(marker)
    }
}

mod editor;
use editor::Editor;

mod remote;
pub use remote::{
    is_connected_to_server, maybe_start_client_connection_thread,
    update_client_connection_condition,
};

mod hltas_bridge;
mod watcher;
use hltas_bridge::Bridge;

static ENABLE_FREECAM_ON_CALCREFDEF: MainThreadCell<bool> = MainThreadCell::new(false);
static LAST_BUTTONS: MainThreadCell<Buttons> = MainThreadCell::new(Buttons::empty());
static INSERT_CAMERA_LINE_DOWN: MainThreadCell<bool> = MainThreadCell::new(false);

static BXT_HUD_TAS_STUDIO: CVar = CVar::new(
    b"bxt_hud_tas_studio\0",
    b"1\0",
    "\
Whether to show the TAS studio HUD when in the TAS editor.",
);

static BXT_TAS_STUDIO_CAMERA_EDITOR: CVar = CVar::new(
    b"bxt_tas_studio_camera_editor\0",
    b"0\0",
    "\
Switches the TAS editor to the camera editor mode.",
);

static BXT_TAS_STUDIO_AUTO_SMOOTHING: CVar = CVar::new(
    b"_bxt_tas_studio_auto_smoothing\0",
    b"0\0",
    "\
Enables automatic global smoothing when working on the TAS. Requires a two-game setup.

As you edit the TAS in the editor, it will be simulated in the second game as usual, then it will \
be simulated again with global smoothing applied to the entire script. The global-smoothed path \
will be displayed in orange alongside the original path.

This is useful when working with global smoothing, as it will ever so slightly change the inputs, \
which can easily snowball into desyncs. Seeing the smoothed path will let you adjust the TAS to \
avoid big desyncs.",
);

static BXT_TAS_STUDIO_SHOW_PLAYER_BBOX: CVar = CVar::new(
    b"bxt_tas_studio_show_player_bbox\0",
    b"0\0",
    "\
Whether to show the player bbox for the frame under the cursor.",
);

static BXT_TAS_STUDIO_SMOOTH_WINDOW_S: CVar = CVar::new(
    b"_bxt_tas_studio_smooth_window_s\0",
    b"0.15\0",
    "\
Smoothing window size in seconds.

Smoothing averages camera angles in a window this big centered around every frame.",
);

static BXT_TAS_STUDIO_SMOOTH_SMALL_WINDOW_S: CVar = CVar::new(
    b"_bxt_tas_studio_smooth_small_window_s\0",
    b"0.03\0",
    "\
Smoothing small window size in seconds.

Smoothing averages camera angles in a window centered around every frame. Within this window,\
there's a smaller window which has a higher contribution to the final smoothed camera angle. This\
console variable defines the size of the small window.",
);

static BXT_TAS_STUDIO_SMOOTH_SMALL_WINDOW_MULTIPLIER: CVar = CVar::new(
    b"_bxt_tas_studio_smooth_small_window_multiplier\0",
    b"3\0",
    "\
Smoothing small window impact multiplier.

Smoothing averages camera angles in a window centered around every frame. Within this window,\
there's a smaller window which has a higher contribution to the final smoothed camera angle. This\
console variable defines how much stronger the influence of the camera angles in the smaller window\
is compared to the big window.",
);

static BXT_TAS_STUDIO_LINE_WIDTH: CVar = CVar::new(
    b"bxt_tas_studio_line_width\0",
    b"2\0",
    "\
The line width used for TAS editor drawing, in pixels.",
);

static BXT_TAS_STUDIO_NEW: Command = Command::new(
    b"bxt_tas_studio_new\0",
    handler!(
        "bxt_tas_studio_new <filename> <starting command> <FPS>

Creates a new TAS project ready to use with the TAS studio.
        
- filename is the filename of the project that will be created. The .hltasproj extension will be \
appended automatically.
- starting command is the command to launch the map or load the save which the TAS will start \
from, for example \"map c1a0\" or \"load tas-start\".
- FPS is the initial FPS for the TAS, for example 100 or 250 or 1000.

The TAS will use your current movement settings such as bxt_bhopcap and sv_maxspeed.

Example: bxt_tas_studio_new full_game \"map c1a0\" 100",
        new as fn(_, _, _, _)
    ),
);

fn new(marker: MainThreadMarker, filename: String, command: String, fps: i32) {
    // TODO: This can be implemented in a higher effort basis without requiring the intermediate
    // .hltas file.
    let hltas_filename = format!("{filename}.hltas");
    if Path::new(&hltas_filename).exists() {
        con_print(
            marker,
            &format!(
                "Error creating the new TAS: the script named \
                 {hltas_filename} already exists. Please rename or remove it first."
            ),
        );
        return;
    }

    let hltasproj_filename = format!("{filename}.hltasproj");
    if Path::new(&hltasproj_filename).exists() {
        con_print(
            marker,
            &format!(
                "Error creating the new TAS: the target .hltasproj script \
                 ({hltasproj_filename}) already exists. Please rename or remove it first."
            ),
        );
        return;
    }

    let frame_time = match fps {
        1000 => "0.001",
        500 => "0.002",
        250 => "0.004",
        100 => "0.010000001",
        _ => {
            con_print(
                marker,
                "You specified FPS = %d, however only FPS = 1000, 500, 250 or 100 are \
                 currently supported. If you need another FPS value, use one of the supported \
                 FPS values, and then change the frametime manually in the script",
            );

            if fps > 0 {
                con_print(
                    marker,
                    &format!(" (you will want something around {})", 1. / fps as f32),
                );
            }

            con_print(marker, ".\n");
            return;
        }
    };

    // TODO: new() should be marked as unsafe because this is not always safe.
    unsafe { bxt::tas_new(marker, filename, command, frame_time.to_owned()) };
}

static BXT_TAS_STUDIO_LOAD: Command = Command::new(
    b"bxt_tas_studio_load\0",
    handler!(
        "bxt_tas_studio_load <tas.hltasproj>

Loads the TAS project, plays it back and opens the TAS editor.",
        load as fn(_, _)
    ),
);

fn load(marker: MainThreadMarker, path: PathBuf) {
    let editor = match Editor::open(&path) {
        Ok(editor) => editor,
        Err(err) => {
            con_print(marker, &format!("Error loading the TAS project: {err}\n"));
            return;
        }
    };

    let bridge = Bridge::with_project_path(&path, editor.script());
    *STATE.borrow_mut(marker) = State::PreparingToPlayToEditor(editor, bridge, false);
}

static BXT_TAS_STUDIO_CONVERT_HLTAS: Command = Command::new(
    b"bxt_tas_studio_convert_hltas\0",
    handler!(
        "bxt_tas_studio_convert_hltas <tas.hltas>

Converts the HLTAS into a TAS project with the same name and .hltasproj extension, plays it back \
and opens the TAS editor.",
        convert_hltas as fn(_, _)
    ),
);

fn convert_hltas(marker: MainThreadMarker, path: PathBuf) {
    if let Err(err) = convert(marker, path) {
        con_print(marker, &format!("Error converting the HLTAS: {err}\n"));
    }
}

fn convert(marker: MainThreadMarker, path: PathBuf) -> eyre::Result<()> {
    let script = read_to_string(&path).context("error reading the HLTAS to string")?;
    let script = HLTAS::from_str(&script)
        .map_err(|err| eyre!(err.to_string()))
        .context("error parsing the HLTAS")?;
    let project_path = path.with_extension("hltasproj");
    let editor =
        Editor::create(&project_path, &script).context("error creating the TAS project")?;

    let bridge = Bridge::with_project_path(&project_path, editor.script());
    *STATE.borrow_mut(marker) = State::PreparingToPlayToEditor(editor, bridge, false);
    Ok(())
}

static BXT_TAS_STUDIO_CONVERT_HLTAS_FROM_BXT_TAS_NEW: Command = Command::new(
    b"_bxt_tas_studio_convert_hltas_from_bxt_tas_new\0",
    handler!(
        "_bxt_tas_studio_convert_hltas_from_bxt_tas_new <tas.hltas>

This is a command used internally by Bunnymod XT. You should use `bxt_tas_studio_convert_hltas` \
instead.",
        convert_hltas_from_bxt_tas_new as fn(_, _)
    ),
);

fn convert_hltas_from_bxt_tas_new(marker: MainThreadMarker, mut path: String) {
    if let Err(err) = convert(marker, PathBuf::from(&path)) {
        con_print(marker, &format!("Error converting the HLTAS: {err}\n"));
        return;
    }

    if let Err(err) = fs::remove_file(&path) {
        con_print(marker, &format!("Error removing .hltas: {err}\n"));
    }

    path.push_str("proj");
    if path.contains(' ') {
        path.insert(0, '"');
        path.push('"');
    }

    con_print(
        marker,
        &format!(
            "New TAS has been created successfully. Use this command for launching it:\n \
             bxt_tas_studio_load {path}\n",
        ),
    )
}

fn norefresh_until_stop_frame_frame_idx(marker: MainThreadMarker, editor: &Editor) -> usize {
    let norefresh_last_frames =
        unsafe { bxt::BXT_TAS_NOREFRESH_UNTIL_LAST_FRAMES.get(marker)() } as usize;

    if editor.stop_frame() == 0 {
        (editor.branch().frames.len() - 1).saturating_sub(norefresh_last_frames)
    } else {
        (editor.stop_frame() as usize).saturating_sub(norefresh_last_frames)
    }
}

fn set_effective_norefresh_until_stop_frame(marker: MainThreadMarker, editor: &Editor) {
    let stop_frame = norefresh_until_stop_frame_frame_idx(marker, editor);
    let value = (editor.branch().frames.len() - 1).saturating_sub(stop_frame);

    unsafe { bxt::BXT_TAS_STUDIO_NOREFRESH_OVERRIDE.get(marker)(value as i32) };
}

static BXT_TAS_STUDIO_REPLAY: Command = Command::new(
    b"bxt_tas_studio_replay\0",
    handler!(
        "bxt_tas_studio_replay

Replays the currently loaded TAS up to the stop frame.",
        replay as fn(_)
    ),
);

fn replay(marker: MainThreadMarker) {
    let mut state = STATE.borrow_mut(marker);
    *state = match mem::take(&mut *state) {
        State::Editing {
            mut editor, bridge, ..
        } => {
            editor.cancel_ongoing_adjustments();
            set_effective_norefresh_until_stop_frame(marker, &editor);
            State::PreparingToPlayToEditor(editor, bridge, true)
        }
        State::PlayingToEditor { editor, bridge, .. } => {
            set_effective_norefresh_until_stop_frame(marker, &editor);
            State::PreparingToPlayToEditor(editor, bridge, true)
        }
        other => other,
    };
}

static BXT_TAS_STUDIO_SET_STOP_FRAME: Command = Command::new(
    b"bxt_tas_studio_set_stop_frame\0",
    handler!(
        "bxt_tas_studio_set_stop_frame [frame]

Sets the stop frame to the frame under the cursor, or to the given frame number, if provided. The
stop frame is a frame where the TAS playback stops and shows the TAS editor UI.",
        set_stop_frame_to_hovered as fn(_),
        set_stop_frame as fn(_, _)
    ),
);

fn set_stop_frame_to_hovered(marker: MainThreadMarker) {
    let mut state = STATE.borrow_mut(marker);
    let State::Editing { editor, .. } = &mut *state else {
        return;
    };

    if let Err(err) = editor.set_stop_frame_to_hovered() {
        con_print(marker, &format!("Error setting stop frame: {err}\n"));
        if err.is_internal() {
            error!("error setting stop frame: {err:?}");
            *state = State::Idle;
        }
    }
}

fn set_stop_frame(marker: MainThreadMarker, stop_frame: u32) {
    let mut state = STATE.borrow_mut(marker);
    let State::Editing { editor, .. } = &mut *state else {
        return;
    };

    if let Err(err) = editor.set_stop_frame(stop_frame) {
        con_print(marker, &format!("Error setting stop frame: {err}\n"));
        if err.is_internal() {
            error!("error setting stop frame: {err:?}\n");
            *state = State::Idle;
        }
    }
}

static BXT_TAS_STUDIO_SET_PITCH: Command = Command::new(
    b"bxt_tas_studio_set_pitch\0",
    handler!(
        "bxt_tas_studio_set_pitch <pitch>

Sets the pitch of the selected frame bulk.",
        set_pitch as fn(_, _)
    ),
);

fn set_pitch(marker: MainThreadMarker, pitch: f32) {
    let mut state = STATE.borrow_mut(marker);
    let State::Editing { editor, .. } = &mut *state else {
        return;
    };

    if let Err(err) = editor.set_pitch(Some(pitch)) {
        con_print(marker, &format!("Error setting pitch: {err}\n"));
        if err.is_internal() {
            error!("error setting pitch: {err:?}\n");
            *state = State::Idle;
        }
    }
}

static BXT_TAS_STUDIO_UNSET_PITCH: Command = Command::new(
    b"bxt_tas_studio_unset_pitch\0",
    handler!(
        "bxt_tas_studio_unset_pitch

Unsets the pitch of the selected frame bulk.",
        unset_pitch as fn(_)
    ),
);

fn unset_pitch(marker: MainThreadMarker) {
    let mut state = STATE.borrow_mut(marker);
    let State::Editing { editor, .. } = &mut *state else {
        return;
    };

    if let Err(err) = editor.set_pitch(None) {
        con_print(marker, &format!("Error unsetting pitch: {err}\n"));
        if err.is_internal() {
            error!("error unsetting pitch: {err:?}\n");
            *state = State::Idle;
        }
    }
}

static BXT_TAS_STUDIO_SET_YAW: Command = Command::new(
    b"bxt_tas_studio_set_yaw\0",
    handler!(
        "bxt_tas_studio_set_yaw <yaw>

Sets the yaw of the selected frame bulk.",
        set_yaw as fn(_, _)
    ),
);

fn set_yaw(marker: MainThreadMarker, yaw: f32) {
    let mut state = STATE.borrow_mut(marker);
    let State::Editing { editor, .. } = &mut *state else {
        return;
    };

    if let Err(err) = editor.set_yaw(Some(yaw)) {
        con_print(marker, &format!("Error setting yaw: {err}\n"));
        if err.is_internal() {
            error!("error setting yaw: {err:?}\n");
            *state = State::Idle;
        }
    }
}

static BXT_TAS_STUDIO_UNSET_YAW: Command = Command::new(
    b"bxt_tas_studio_unset_yaw\0",
    handler!(
        "bxt_tas_studio_unset_yaw

Unsets the yaw of the selected frame bulk.",
        unset_yaw as fn(_)
    ),
);

fn unset_yaw(marker: MainThreadMarker) {
    let mut state = STATE.borrow_mut(marker);
    let State::Editing { editor, .. } = &mut *state else {
        return;
    };

    if let Err(err) = editor.set_yaw(None) {
        con_print(marker, &format!("Error unsetting yaw: {err}\n"));
        if err.is_internal() {
            error!("error unsetting yaw: {err:?}\n");
            *state = State::Idle;
        }
    }
}

static BXT_TAS_STUDIO_SET_YAWSPEED: Command = Command::new(
    b"bxt_tas_studio_set_yawspeed\0",
    handler!(
        "bxt_tas_studio_set_yawspeed
        
Sets the yawspeed of the selected frame bulk for constant turn rate strafing.",
        set_yawspeed as fn(_, _)
    ),
);

fn set_yawspeed(marker: MainThreadMarker, yawspeed: f32) {
    let mut state = STATE.borrow_mut(marker);
    let State::Editing { editor, .. } = &mut *state else {
        return;
    };

    if let Err(err) = editor.set_yawspeed(Some(yawspeed)) {
        con_print(marker, &format!("Error setting yawspeed: {err}\n"));
        if err.is_internal() {
            error!("error setting yawspeed: {err:?}\n");
            *state = State::Idle;
        }
    }
}

static BXT_TAS_STUDIO_SET_FRAME_TIME: Command = Command::new(
    b"bxt_tas_studio_set_frame_time\0",
    handler!(
        "bxt_tas_studio_set_frame_time <frame time>
        
Sets the frame time of the selected frame bulk.",
        set_frame_time as fn(_, _)
    ),
);

fn set_frame_time(marker: MainThreadMarker, frame_time: String) {
    let mut state = STATE.borrow_mut(marker);
    let State::Editing { editor, .. } = &mut *state else {
        return;
    };

    if let Err(err) = editor.set_frame_time(frame_time) {
        con_print(marker, &format!("Error setting frame time: {err}\n"));
        if err.is_internal() {
            error!("error setting frame time: {err:?}\n");
            *state = State::Idle;
        }
    }
}

static BXT_TAS_STUDIO_SET_COMMANDS: Command = Command::new(
    b"bxt_tas_studio_set_commands\0",
    handler!(
        "bxt_tas_studio_set_commands <console commands>
        
Sets the console commands of the selected frame bulk.",
        set_commands as fn(_, _)
    ),
);

fn set_commands(marker: MainThreadMarker, commands: String) {
    let mut state = STATE.borrow_mut(marker);
    let State::Editing { editor, .. } = &mut *state else {
        return;
    };

    let commands = if commands.is_empty() {
        None
    } else {
        Some(commands)
    };

    if let Err(err) = editor.set_commands(commands) {
        con_print(marker, &format!("Error setting commands: {err}\n"));
        if err.is_internal() {
            error!("error setting commands: {err:?}\n");
            *state = State::Idle;
        }
    }
}

static BXT_TAS_STUDIO_SELECT_NEXT: Command = Command::new(
    b"bxt_tas_studio_select_next\0",
    handler!(
        "bxt_tas_studio_select_next

Selects the next frame bulk.",
        select_next as fn(_)
    ),
);

fn select_next(marker: MainThreadMarker) {
    let mut state = STATE.borrow_mut(marker);
    let State::Editing { editor, .. } = &mut *state else {
        return;
    };

    if let Err(err) = editor.select_next() {
        con_print(marker, &format!("Error selecting frame bulk: {err}\n"));
        if err.is_internal() {
            error!("error selecting frame bulk: {err:?}\n");
            *state = State::Idle;
        }
    }
}

static BXT_TAS_STUDIO_SELECT_PREV: Command = Command::new(
    b"bxt_tas_studio_select_prev\0",
    handler!(
        "bxt_tas_studio_select_prev

Selects the previous frame bulk.",
        select_prev as fn(_)
    ),
);

fn select_prev(marker: MainThreadMarker) {
    let mut state = STATE.borrow_mut(marker);
    let State::Editing { editor, .. } = &mut *state else {
        return;
    };

    if let Err(err) = editor.select_prev() {
        con_print(marker, &format!("Error selecting frame bulk: {err}\n"));
        if err.is_internal() {
            error!("error selecting frame bulk: {err:?}\n");
            *state = State::Idle;
        }
    }
}

static PLUS_BXT_TAS_STUDIO_INSERT_CAMERA_LINE: Command = Command::new(
    b"+bxt_tas_studio_insert_camera_line\0",
    handler!(
        "+bxt_tas_studio_insert_camera_line [key]

Hold to insert camera lines in the camera editor mode.",
        plus_insert_camera_line as fn(_),
        plus_insert_camera_line_key as fn(_, _)
    ),
);

fn plus_insert_camera_line(marker: MainThreadMarker) {
    if !matches!(*STATE.borrow(marker), State::Editing { .. }) {
        return;
    }

    INSERT_CAMERA_LINE_DOWN.set(marker, true);
}

fn plus_insert_camera_line_key(marker: MainThreadMarker, _key: i32) {
    plus_insert_camera_line(marker);
}

static MINUS_BXT_TAS_STUDIO_INSERT_CAMERA_LINE: Command = Command::new(
    b"-bxt_tas_studio_insert_camera_line\0",
    handler!(
        "-bxt_tas_studio_insert_camera_line [key]

Hold to insert camera lines in the camera editor mode.",
        minus_insert_camera_line as fn(_),
        minus_insert_camera_line_key as fn(_, _)
    ),
);

fn minus_insert_camera_line(marker: MainThreadMarker) {
    if !matches!(*STATE.borrow(marker), State::Editing { .. }) {
        return;
    }

    INSERT_CAMERA_LINE_DOWN.set(marker, false);
}

fn minus_insert_camera_line_key(marker: MainThreadMarker, _key: i32) {
    minus_insert_camera_line(marker);
}

static PLUS_BXT_TAS_STUDIO_LOOK_AROUND: Command = Command::new(
    b"+bxt_tas_studio_look_around\0",
    handler!(
        "+bxt_tas_studio_look_around [key]

Hold to look around in the TAS editor.",
        plus_look_around as fn(_),
        plus_look_around_key as fn(_, _)
    ),
);

fn plus_look_around(marker: MainThreadMarker) {
    if !matches!(*STATE.borrow(marker), State::Editing { .. }) {
        return;
    }

    sdl::set_relative_mouse_mode(marker, true);
    client::activate_mouse(marker, true);
}

fn plus_look_around_key(marker: MainThreadMarker, _key: i32) {
    plus_look_around(marker);
}

static MINUS_BXT_TAS_STUDIO_LOOK_AROUND: Command = Command::new(
    b"-bxt_tas_studio_look_around\0",
    handler!(
        "-bxt_tas_studio_look_around [key]

Hold to look around in the TAS editor.",
        minus_look_around as fn(_),
        minus_look_around_key as fn(_, _)
    ),
);

fn minus_look_around(marker: MainThreadMarker) {
    if !matches!(*STATE.borrow(marker), State::Editing { .. }) {
        return;
    }

    sdl::set_relative_mouse_mode(marker, false);
    client::activate_mouse(marker, false);
}

fn minus_look_around_key(marker: MainThreadMarker, _key: i32) {
    minus_look_around(marker);
}

static BXT_TAS_STUDIO_UNDO: Command = Command::new(
    b"bxt_tas_studio_undo\0",
    handler!(
        "bxt_tas_studio_undo

Undoes the last change to the script.",
        undo as fn(_)
    ),
);

fn undo(marker: MainThreadMarker) {
    let mut state = STATE.borrow_mut(marker);
    let State::Editing {
        editor,
        last_generation,
        last_branch_idx,
        simulate_at,
        ..
    } = &mut *state
    else {
        return;
    };

    if let Err(err) = editor.undo() {
        con_print(marker, &format!("Error undoing: {err}\n"));
        if err.is_internal() {
            error!("error undoing: {err:?}\n");
            *state = State::Idle;
        }
        return;
    }

    // Force a bridged file update.
    *last_generation = editor.generation();
    *last_branch_idx = editor.branch_idx();
    *simulate_at = Some(Instant::now() + Duration::from_millis(100));
}

static BXT_TAS_STUDIO_REDO: Command = Command::new(
    b"bxt_tas_studio_redo\0",
    handler!(
        "bxt_tas_studio_redo

Redoes the last change to the script.",
        redo as fn(_)
    ),
);

fn redo(marker: MainThreadMarker) {
    let mut state = STATE.borrow_mut(marker);
    let State::Editing {
        editor,
        last_generation,
        last_branch_idx,
        simulate_at,
        ..
    } = &mut *state
    else {
        return;
    };

    if let Err(err) = editor.redo() {
        con_print(marker, &format!("Error redoing: {err}\n"));
        if err.is_internal() {
            error!("error redoing: {err:?}\n");
            *state = State::Idle;
        }
        return;
    }

    // Force a bridged file update.
    *last_generation = editor.generation();
    *last_branch_idx = editor.branch_idx();
    *simulate_at = Some(Instant::now() + Duration::from_millis(100));
}

static BXT_TAS_STUDIO_BRANCH_CLONE: Command = Command::new(
    b"bxt_tas_studio_branch_clone\0",
    handler!(
        "bxt_tas_studio_branch_clone

Clones the current branch.",
        branch_clone as fn(_)
    ),
);

fn branch_clone(marker: MainThreadMarker) {
    let mut state = STATE.borrow_mut(marker);
    let State::Editing { editor, .. } = &mut *state else {
        return;
    };

    if let Err(err) = editor.branch_clone() {
        con_print(marker, &format!("Error cloning branch: {err}\n"));
        if err.is_internal() {
            error!("error cloning branch: {err:?}\n");
            *state = State::Idle;
        }
    }
}

static BXT_TAS_STUDIO_BRANCH_FOCUS_ID: Command = Command::new(
    b"bxt_tas_studio_branch_focus_id\0",
    handler!(
        "bxt_tas_studio_branch_focus_id <index>

Focuses branch with the given index.",
        branch_focus_id as fn(_, _)
    ),
);

fn branch_focus_id(marker: MainThreadMarker, branch_idx: usize) {
    let mut state = STATE.borrow_mut(marker);
    let State::Editing { editor, .. } = &mut *state else {
        return;
    };

    if let Err(err) = editor.branch_focus(branch_idx) {
        con_print(marker, &format!("Error focusing branch: {err}\n"));
        if err.is_internal() {
            error!("error focusing branch: {err:?}\n");
            *state = State::Idle;
        }
    }
}

static BXT_TAS_STUDIO_BRANCH_FOCUS_NEXT: Command = Command::new(
    b"bxt_tas_studio_branch_focus_next\0",
    handler!(
        "bxt_tas_studio_branch_focus_next

Focuses the next visible branch.",
        branch_focus_next as fn(_)
    ),
);

fn branch_focus_next(marker: MainThreadMarker) {
    let mut state = STATE.borrow_mut(marker);
    let State::Editing { editor, .. } = &mut *state else {
        return;
    };

    if let Err(err) = editor.branch_focus_next() {
        con_print(marker, &format!("Error focusing branch: {err}\n"));
        if err.is_internal() {
            error!("error focusing branch: {err:?}\n");
            *state = State::Idle;
        }
    }
}

static BXT_TAS_STUDIO_BRANCH_HIDE_ID: Command = Command::new(
    b"bxt_tas_studio_branch_hide_id\0",
    handler!(
        "bxt_tas_studio_branch_hide_id <index>

Hides the branch with the given index.",
        branch_hide_id as fn(_, _)
    ),
);

fn branch_hide_id(marker: MainThreadMarker, branch_idx: usize) {
    let mut state = STATE.borrow_mut(marker);
    let State::Editing { editor, .. } = &mut *state else {
        return;
    };

    if let Err(err) = editor.branch_hide(branch_idx) {
        con_print(marker, &format!("Error hiding branch: {err}\n"));
        if err.is_internal() {
            error!("error hiding branch: {err:?}\n");
            *state = State::Idle;
        }
    }
}

static BXT_TAS_STUDIO_BRANCH_HIDE_AND_FOCUS_NEXT: Command = Command::new(
    b"bxt_tas_studio_branch_hide_and_focus_next\0",
    handler!(
        "bxt_tas_studio_branch_hide_and_focus_next <index>

Hides the currently focused branch and focuses the next visible branch.",
        branch_hide_and_focus_next as fn(_)
    ),
);

fn branch_hide_and_focus_next(marker: MainThreadMarker) {
    let mut state = STATE.borrow_mut(marker);
    let State::Editing { editor, .. } = &mut *state else {
        return;
    };

    if let Err(err) = editor.branch_hide_and_focus_next() {
        con_print(
            marker,
            &format!("Error hiding branch and focusing next: {err}\n"),
        );
        if err.is_internal() {
            error!("error hiding branch and focusing next: {err:?}\n");
            *state = State::Idle;
        }
    }
}

static BXT_TAS_STUDIO_BRANCH_SHOW_ID: Command = Command::new(
    b"bxt_tas_studio_branch_show_id\0",
    handler!(
        "bxt_tas_studio_branch_show_id <index>

Shows the branch with the given index.",
        branch_show_id as fn(_, _)
    ),
);

fn branch_show_id(marker: MainThreadMarker, branch_idx: usize) {
    let mut state = STATE.borrow_mut(marker);
    let State::Editing { editor, .. } = &mut *state else {
        return;
    };

    if let Err(err) = editor.branch_show(branch_idx) {
        con_print(marker, &format!("Error showing branch: {err}\n"));
        if err.is_internal() {
            error!("error showing branch: {err:?}\n");
            *state = State::Idle;
        }
    }
}

static BXT_TAS_STUDIO_TOGGLE: Command = Command::new(
    b"bxt_tas_studio_toggle\0",
    handler!(
        "bxt_tas_studio_toggle <what>

Toggles a value on the selected frame bulk.

Values that you can toggle:

- s03: speed increasing strafing
- s13: quick turn strafing
- s22: slow down strafing
- s00: speed increasing strafing to the left
- s01: speed increasing strafing to the right
- s10: quick turn strafing to the left
- s11: quick turn strafing to the right
- s06: left-right strafing
- s07: right-left strafing
- s40: constant turn rate to the left
- s41: constant turn rate to the right
- s50: accelerated turn to the left
- s51: accelerated turn to the right
- lgagst: makes autojump and ducktap trigger at optimal speed
- autojump
- ducktap
- jumpbug
- dbc: duck before collision
- dbcceilings: duck before collision, including ceilings
- dbg: duck before ground
- dwj: duck when jump (useful for the long-jump module)
- forward: +forward
- left: +moveleft
- right: +moveright
- back: +back
- up: +moveup
- down: +movedown
- jump: +jump
- duck: +duck
- use: +use
- attack1: +attack1
- attack2: +attack2
- reload: +reload",
        toggle as fn(_, _)
    ),
);

fn toggle(marker: MainThreadMarker, what: String) {
    let mut state = STATE.borrow_mut(marker);
    let State::Editing { editor, .. } = &mut *state else {
        return;
    };

    let what = what.trim().to_ascii_lowercase();

    let target = match &*what {
        "s03" => ToggleAutoActionTarget::Strafe {
            dir: StrafeDir::Yaw(0.),
            type_: StrafeType::MaxAccel,
        },
        "s13" => ToggleAutoActionTarget::Strafe {
            dir: StrafeDir::Yaw(0.),
            type_: StrafeType::MaxAngle,
        },
        "s22" => ToggleAutoActionTarget::Strafe {
            dir: StrafeDir::Best,
            type_: StrafeType::MaxDeccel,
        },
        "s00" => ToggleAutoActionTarget::Strafe {
            dir: StrafeDir::Left,
            type_: StrafeType::MaxAccel,
        },
        "s01" => ToggleAutoActionTarget::Strafe {
            dir: StrafeDir::Right,
            type_: StrafeType::MaxAccel,
        },
        "s10" => ToggleAutoActionTarget::Strafe {
            dir: StrafeDir::Left,
            type_: StrafeType::MaxAngle,
        },
        "s11" => ToggleAutoActionTarget::Strafe {
            dir: StrafeDir::Right,
            type_: StrafeType::MaxAngle,
        },
        "s06" => ToggleAutoActionTarget::Strafe {
            dir: StrafeDir::LeftRight(NonZeroU32::new(1).unwrap()),
            type_: StrafeType::MaxAccel,
        },
        "s07" => ToggleAutoActionTarget::Strafe {
            dir: StrafeDir::RightLeft(NonZeroU32::new(1).unwrap()),
            type_: StrafeType::MaxAccel,
        },
        "s40" => ToggleAutoActionTarget::Strafe {
            dir: StrafeDir::Left,
            type_: StrafeType::ConstYawspeed(210.),
        },
        "s41" => ToggleAutoActionTarget::Strafe {
            dir: StrafeDir::Right,
            type_: StrafeType::ConstYawspeed(210.),
        },
        "s50" => ToggleAutoActionTarget::Strafe {
            dir: StrafeDir::Left,
            type_: StrafeType::MaxAccelYawOffset {
                start: 0.,
                target: 0.,
                accel: 0.,
            },
        },
        "s51" => ToggleAutoActionTarget::Strafe {
            dir: StrafeDir::Right,
            type_: StrafeType::MaxAccelYawOffset {
                start: 0.,
                target: 0.,
                accel: 0.,
            },
        },
        "lgagst" => ToggleAutoActionTarget::LeaveGroundAtOptimalSpeed,
        "autojump" => ToggleAutoActionTarget::AutoJump,
        "ducktap" => ToggleAutoActionTarget::DuckTap,
        "jumpbug" => ToggleAutoActionTarget::JumpBug,
        "dbc" => ToggleAutoActionTarget::DuckBeforeCollision,
        "dbcceilings" => ToggleAutoActionTarget::DuckBeforeCollisionIncludingCeilings,
        "dbg" => ToggleAutoActionTarget::DuckBeforeGround,
        "dwj" => ToggleAutoActionTarget::DuckWhenJump,

        _ => {
            let key = match &*what {
                "forward" => Key::Forward,
                "left" => Key::Left,
                "right" => Key::Right,
                "back" => Key::Back,
                "up" => Key::Up,
                "down" => Key::Down,
                "jump" => Key::Jump,
                "duck" => Key::Duck,
                "use" => Key::Use,
                "attack1" => Key::Attack1,
                "attack2" => Key::Attack2,
                "reload" => Key::Reload,

                _ => {
                    con_print(
                        marker,
                        &format!(
                            "Error: unknown value.\n\nUsage: {}\n",
                            BXT_TAS_STUDIO_TOGGLE.description()
                        ),
                    );
                    return;
                }
            };

            if let Err(err) = editor.toggle_key(key) {
                con_print(marker, &format!("Error toggling value: {err}\n"));
                if err.is_internal() {
                    error!("error toggling value: {err:?}\n");
                    *state = State::Idle;
                }
            }

            return;
        }
    };

    if let Err(err) = editor.toggle_auto_action(target) {
        con_print(marker, &format!("Error toggling value: {err}\n"));
        if err.is_internal() {
            error!("error toggling value: {err:?}\n");
            *state = State::Idle;
        }
    }
}

static BXT_TAS_STUDIO_HIDE: Command = Command::new(
    b"bxt_tas_studio_hide\0",
    handler!(
        "bxt_tas_studio_hide

Hides the frames before the one under cursor to avoid clutter. If there's no visible frame under \
the cursor, makes all frames visible.",
        hide as fn(_)
    ),
);

fn hide(marker: MainThreadMarker) {
    let mut state = STATE.borrow_mut(marker);
    let State::Editing { editor, .. } = &mut *state else {
        return;
    };

    if let Err(err) = editor.hide_frames_up_to_hovered() {
        con_print(marker, &format!("Error hiding: {err}\n"));
        if err.is_internal() {
            error!("error hiding: {err:?}\n");
            *state = State::Idle;
        }
    }
}

static BXT_TAS_STUDIO_SMOOTH: Command = Command::new(
    b"bxt_tas_studio_smooth\0",
    handler!(
        "bxt_tas_studio_smooth

Applies smoothing to the hovered segment.",
        smooth as fn(_)
    ),
);

fn smooth(marker: MainThreadMarker) {
    let mut state = STATE.borrow_mut(marker);
    let State::Editing { editor, .. } = &mut *state else {
        return;
    };

    if let Err(err) = editor.apply_smoothing_to_hovered_segment() {
        con_print(marker, &format!("Error applying smoothing: {err}\n"));
        if err.is_internal() {
            error!("error applying smoothing: {err:?}\n");
            *state = State::Idle;
        }
    }
}

static BXT_TAS_STUDIO_SMOOTH_GLOBALLY: Command = Command::new(
    b"_bxt_tas_studio_smooth_globally\0",
    handler!(
        "_bxt_tas_studio_smooth_globally

Applies smoothing to the entire script.",
        smooth_globally as fn(_)
    ),
);

fn smooth_globally(marker: MainThreadMarker) {
    let mut state = STATE.borrow_mut(marker);
    let State::Editing { editor, .. } = &mut *state else {
        return;
    };

    if let Err(err) = editor.apply_global_smoothing() {
        con_print(marker, &format!("Error applying smoothing: {err}\n"));
        if err.is_internal() {
            error!("error applying smoothing: {err:?}\n");
            *state = State::Idle;
        }
    }
}

static BXT_TAS_STUDIO_DELETE: Command = Command::new(
    b"bxt_tas_studio_delete\0",
    handler!(
        "bxt_tas_studio_delete

Deletes the selected frame bulk or the line under cursor in the camera editor.",
        delete as fn(_)
    ),
);

fn delete(marker: MainThreadMarker) {
    let mut state = STATE.borrow_mut(marker);
    let State::Editing { editor, .. } = &mut *state else {
        return;
    };

    if let Err(err) = editor.delete_selected() {
        con_print(marker, &format!("Error deleting selected: {err}\n"));
        if err.is_internal() {
            error!("error deleting selected: {err:?}\n");
            *state = State::Idle;
        }
    }
}

static BXT_TAS_STUDIO_DELETE_LAST: Command = Command::new(
    b"bxt_tas_studio_delete_last\0",
    handler!(
        "bxt_tas_studio_delete_last

Deletes the last frame bulk of the current branch.",
        delete_last as fn(_)
    ),
);

fn delete_last(marker: MainThreadMarker) {
    let mut state = STATE.borrow_mut(marker);
    let State::Editing { editor, .. } = &mut *state else {
        return;
    };

    if let Err(err) = editor.delete_last() {
        con_print(marker, &format!("Error deleting last: {err}\n"));
        if err.is_internal() {
            error!("error deleting last: {err:?}\n");
            *state = State::Idle;
        }
    }
}

static BXT_TAS_STUDIO_SPLIT: Command = Command::new(
    b"bxt_tas_studio_split\0",
    handler!(
        "bxt_tas_studio_split

Splits the frame bulk at frame under cursor.",
        split as fn(_)
    ),
);

fn split(marker: MainThreadMarker) {
    let mut state = STATE.borrow_mut(marker);
    let State::Editing {
        editor,
        last_generation,
        last_branch_idx,
        simulate_at,
        ..
    } = &mut *state
    else {
        return;
    };

    if let Err(err) = editor.split() {
        con_print(marker, &format!("Error splitting: {err}\n"));
        if err.is_internal() {
            error!("error splitting: {err:?}");
            *state = State::Idle;
        }
        return;
    }

    // Force a bridged file update.
    *last_generation = editor.generation();
    *last_branch_idx = editor.branch_idx();
    *simulate_at = Some(Instant::now() + Duration::from_millis(100));
}

static BXT_TAS_STUDIO_CLOSE: Command = Command::new(
    b"bxt_tas_studio_close\0",
    handler!(
        "bxt_tas_studio_close

Closes the TAS studio.",
        close as fn(_)
    ),
);

fn close(marker: MainThreadMarker) {
    *STATE.borrow_mut(marker) = State::Idle;

    sdl::set_relative_mouse_mode(marker, true);
    client::activate_mouse(marker, true);
}

static BXT_TAS_STUDIO_OPTIM_INIT: Command = Command::new(
    b"bxt_tas_studio_optim_init\0",
    handler!(
        "bxt_tas_studio_optim_init

Initializes the optimization starting from the selected frame bulk.",
        optim_init as fn(_)
    ),
);

fn optim_init(marker: MainThreadMarker) {
    let mut state = STATE.borrow_mut(marker);
    let State::Editing { editor, .. } = &mut *state else {
        return;
    };

    let hltas = editor.script().clone();
    let Some(bulk_idx) = editor.selected_bulk_idx() else {
        return;
    };
    let first_frame = bulk_and_first_frame_idx(&hltas).nth(bulk_idx).unwrap().1 - 1;
    let Some(initial_frame) = editor.branch().frames.get(first_frame).cloned() else {
        return;
    };

    optim_init_internal(marker, hltas, first_frame, initial_frame);
}

static BXT_TAS_STUDIO_OPTIM_APPLY: Command = Command::new(
    b"bxt_tas_studio_optim_apply\0",
    handler!(
        "bxt_tas_studio_optim_apply

Applies the current best optimization result to the current branch.",
        optim_apply as fn(_)
    ),
);

fn optim_apply(marker: MainThreadMarker) {
    let mut state = STATE.borrow_mut(marker);
    let State::Editing { editor, .. } = &mut *state else {
        return;
    };

    let new_script = match unsafe { tas_optimizer::current_best(marker) } {
        Some(x) => x,
        None => return,
    };

    if let Err(err) = editor.rewrite(new_script) {
        con_print(marker, &format!("Error rewriting the script: {err}\n"));
        if err.is_internal() {
            error!("error rewriting the script: {err:?}\n");
            *state = State::Idle;
        }
    }
}

enum State {
    /// Doing nothing special.
    Idle,
    /// Playing a HLTAS.
    Playing {
        generation: u16,
        branch_idx: usize,
        is_smoothed: bool,
        frames_played: usize,
        next_frame_params: Option<Parameters>,
    },
    /// Preparing to play a HLTAS, will open the editor afterwards.
    PreparingToPlayToEditor(Editor, Bridge, bool),
    /// Playing a HLTAS, will open the editor afterwards.
    PlayingToEditor {
        editor: Editor,
        frames_played: usize,
        bridge: Bridge,
        next_frame_params: Option<Parameters>,
        is_replay: bool,
    },
    /// Editing a HLTAS.
    Editing {
        editor: Editor,
        last_generation: u16,
        last_branch_idx: usize,
        simulate_at: Option<Instant>,
        bridge: Bridge,
    },
}

impl Default for State {
    fn default() -> Self {
        Self::Idle
    }
}

static STATE: MainThreadRefCell<State> = MainThreadRefCell::new(State::Idle);

pub unsafe fn maybe_receive_messages_from_remote_server(marker: MainThreadMarker) {
    if !TasStudio.is_enabled(marker) {
        return;
    }

    let client_state = (*engine::cls.get(marker)).state;
    if client_state != 1 && client_state != 5 {
        return;
    }

    // No long borrows of STATE below because bxt::tas_load_script calls
    // bxt_on_tas_playback_stopped, which also borrows STATE.
    let prev_state = mem::take(&mut *STATE.borrow_mut(marker));
    match prev_state {
        State::PreparingToPlayToEditor(editor, bridge, is_replay) => {
            engine::prepend_command(marker, "bxt_tas_write_log 1\n");

            // This might be too late for the client to process (especially with stop_frame 0), but
            // at least it'll fix the camera next time the user unpauses...
            engine::prepend_command(marker, "m_rawinput 1\n");

            // This allows focus on game I think.
            // In BunnymodXT, HwDLL::SetTASEditorMode is called.
            // In bxt-rs, the function is not called. So here we only call the important part.
            // If game is not focused, is possible for pitch to get set to 0 when on ground.
            client::activate_mouse(marker, true);
            sdl::set_relative_mouse_mode(marker, true);

            let script = if BXT_TAS_STUDIO_AUTO_SMOOTHING.as_bool(marker) {
                editor.smoothed_script()
            } else {
                None
            };
            let script = script.unwrap_or_else(|| editor.script());
            bxt::tas_load_script(marker, script);

            *STATE.borrow_mut(marker) = State::PlayingToEditor {
                editor,
                frames_played: 0,
                bridge,
                next_frame_params: None,
                is_replay,
            };

            // TODO: stop server on close?
            if let Err(err) = remote::start_server() {
                con_print(
                    marker,
                    &format!("Could not start a server for multi-game TAS simulation: {err:?}"),
                );
            }
        }
        other => *STATE.borrow_mut(marker) = other,
    }

    let mut state = STATE.borrow_mut(marker);
    match &mut *state {
        State::Idle | State::Playing { .. } => {
            let mut play_request = None;
            while let Ok(Some(request)) = remote::receive_request_from_server() {
                play_request = Some(request);
            }
            if let Some(PlayRequest {
                script,
                generation,
                branch_idx,
                is_smoothed,
            }) = play_request
            {
                engine::prepend_command(
                    marker,
                    "volume 0;MP3Volume 0;\
                     bxt_tas_write_log 0;bxt_tas_norefresh_until_last_frames 1;_bxt_norefresh 1\n",
                );

                drop(state);
                bxt::tas_load_script(marker, &script);

                *STATE.borrow_mut(marker) = State::Playing {
                    generation,
                    branch_idx,
                    is_smoothed,
                    frames_played: 0,
                    next_frame_params: None,
                };
            }
        }
        State::PlayingToEditor { editor, .. } | State::Editing { editor, .. } => {
            while let Ok(Some(frame)) = remote::receive_frame_from_client() {
                // Don't truncate the frames here as it makes it more annoying to work on TASes with
                // loading desync or other inconsistencies.
                if let Some(play_request) = editor.apply_accurate_frame(frame, false) {
                    info!("sending second play request");
                    remote::maybe_send_request_to_client(play_request);
                }
            }
            editor.recompute_extra_camera_frame_data_if_needed();
        }
        State::PreparingToPlayToEditor(_, _, _) => unreachable!(),
    }
}

pub unsafe fn on_tas_playback_frame(
    marker: MainThreadMarker,
    data: OnTasPlaybackFrameData,
) -> bool {
    let mut stop = false;
    let mut state = STATE.borrow_mut(marker);

    if let State::Playing {
        frames_played,
        next_frame_params,
        ..
    }
    | State::PlayingToEditor {
        frames_played,
        next_frame_params,
        ..
    } = &mut *state
    {
        let player = player_data(marker).unwrap();
        let new_next_frame_params = parameters(marker);

        // For the initial frame we don't have params, so just use the current ones.
        let params = next_frame_params.take().unwrap_or(new_next_frame_params);
        *next_frame_params = Some(new_next_frame_params);

        let tracer = Tracer::new(marker, false).unwrap();

        // TODO: prev_frame_input, which is not set here, is important.
        let mut strafe_state = bxt_strafe::State::new(&tracer, params, player);
        strafe_state.strafe_cycle_frame_count = data.strafe_cycle_frame_count;
        strafe_state.max_accel_yaw_offset_value = data.max_accel_yaw_offset.value;
        strafe_state.prev_max_accel_yaw_offset_start = data.max_accel_yaw_offset.start;
        strafe_state.prev_max_accel_yaw_offset_target = data.max_accel_yaw_offset.target;
        strafe_state.prev_max_accel_yaw_offset_accel = data.max_accel_yaw_offset.accel;
        // LEFT = 0, RIGHT = 1. Very nice.
        strafe_state.prev_max_accel_yaw_offset_right = data.max_accel_yaw_offset.dir == 1;

        // Get view angles for this frame.
        unsafe {
            let mut view_angles = [0.; 3];
            engine::hudGetViewAngles.get(marker)(&mut view_angles);
            strafe_state.prev_frame_input.pitch = view_angles[0].to_radians();
            strafe_state.prev_frame_input.yaw = view_angles[1].to_radians();
        }

        // We don't have a good way to extract real trace results from the movement code, so let's
        // make up trace results based on previous frame's predicted fractions and normal Zs from
        // BXT.
        for (fraction, z) in zip(
            data.prev_predicted_trace_fractions,
            data.prev_predicted_trace_normal_zs,
        ) {
            strafe_state.move_traces.push(TraceResult {
                all_solid: false,
                start_solid: false,
                fraction,
                end_pos: Vec3::ZERO,
                plane_normal: Vec3::new(0., 0., z),
                entity: -1,
            });
        }
        let frame = Frame {
            state: strafe_state,
            parameters: params,
        };

        let frame_idx = *frames_played;
        *frames_played += 1;

        let (generation, branch_idx) = match &*state {
            State::Playing {
                generation,
                branch_idx,
                ..
            } => (*generation, *branch_idx),
            State::PlayingToEditor { editor, .. } => (editor.generation(), editor.branch_idx()),
            _ => unreachable!(),
        };

        let is_smoothed = if let State::Playing { is_smoothed, .. } = *state {
            is_smoothed
        } else {
            false
        };

        let accurate_frame = AccurateFrame {
            frame_idx,
            frame,
            generation,
            branch_idx,
            is_smoothed,
        };

        match &mut *state {
            State::Playing { .. } => {
                if let Err(()) = remote::send_frame_to_server(accurate_frame) {
                    *state = State::default();
                    stop = true;
                }
            }
            State::PlayingToEditor {
                editor,
                frames_played,
                is_replay,
                ..
            } => {
                let _ = editor.apply_accurate_frame(accurate_frame, true);
                editor.recompute_extra_camera_frame_data_if_needed();

                // If we've just loaded the TAS (i.e. it's not a replay), then stop right away.
                if !*is_replay {
                    stop = true;
                }

                // If stop_frame is 0, play the whole TAS, as that results in more intuitive
                // behavior.
                if editor.stop_frame() != 0 && *frames_played == editor.stop_frame() as usize + 1 {
                    stop = true;
                }
            }
            _ => unreachable!(),
        };
    }

    if stop {
        debug!("stopping TAS playback");
    }

    stop
}

pub unsafe fn on_tas_playback_stopped(marker: MainThreadMarker) {
    if !TasStudio.is_enabled(marker) {
        // This can be called by BXT during unhooking if the player manually closes or restarts the
        // game during a TAS playback. When that happens, our pointers are already reset, so running
        // the code below panics. However, in this case the logical thing to do is to reset the
        // state to Idle anyway, so let's do it.
        *STATE.borrow_mut(marker) = State::Idle;
        return;
    }

    let mut state = STATE.borrow_mut(marker);

    *state = match mem::take(&mut *state) {
        State::Playing { .. } => {
            engine::prepend_command(marker, "setpause\n");

            State::Idle
        }
        State::PlayingToEditor { editor, bridge, .. } => {
            let generation = editor.generation();
            let branch_idx = editor.branch_idx();
            remote::maybe_send_request_to_client(PlayRequest {
                script: editor.script().clone(),
                generation,
                branch_idx,
                is_smoothed: false,
            });

            sdl::set_relative_mouse_mode(marker, false);
            client::activate_mouse(marker, false);

            // When we show_ui we stop, and when we stop we don't insert any commands, so we can
            // use wait.
            engine::prepend_command(
                marker,
                "_bxt_norefresh 0;setpause;stop;bxt_timer_stop;bxt_cap_stop\n",
            );

            ENABLE_FREECAM_ON_CALCREFDEF.set(marker, true);

            if BXT_IS_TAS_EDITOR_ACTIVE.get(marker)() != 0 {
                // If the TAS editor got enabled, print a warning message and disable it, but keep
                // the TAS studio running. This is because otherwise there's no easy way for the
                // user to actually remove the bxt_tas_editor 1 command (since the script is in the
                // .hltasproj).
                con_print(
                    marker,
                    "The Bunnymod XT TAS editor was enabled while playing back the script in the \
                     bxt-rs TAS studio! This is not supported. Please remove any bxt_tas_editor 1 \
                     commands from the script!\n",
                );

                engine::prepend_command(marker, "bxt_tas_editor 0\n");
            }

            State::Editing {
                editor,
                last_generation: generation,
                last_branch_idx: branch_idx,
                simulate_at: None,
                bridge,
            }
        }
        other => other,
    };
}

pub fn draw(marker: MainThreadMarker, tri: &TriangleApi) {
    let mut state = STATE.borrow_mut(marker);
    let State::Editing {
        editor,
        last_generation,
        last_branch_idx,
        simulate_at,
        bridge,
    } = &mut *state
    else {
        return;
    };

    let _span = info_span!("tas_studio::draw").entered();

    if let Some(script) = bridge.new_script() {
        if let Err(err) = editor.rewrite(script) {
            con_print(marker, &format!("Error rewriting the script: {err}\n"));
            if err.is_internal() {
                error!("error rewriting the script: {err:?}\n");
                *state = State::Idle;
            }
            return;
        }
    }

    editor.set_in_camera_editor(BXT_TAS_STUDIO_CAMERA_EDITOR.as_bool(marker));
    editor.set_auto_smoothing(BXT_TAS_STUDIO_AUTO_SMOOTHING.as_bool(marker));
    editor.set_show_player_bbox(BXT_TAS_STUDIO_SHOW_PLAYER_BBOX.as_bool(marker));
    editor.set_smooth_window_s(BXT_TAS_STUDIO_SMOOTH_WINDOW_S.as_f32(marker));
    editor.set_smooth_small_window_s(BXT_TAS_STUDIO_SMOOTH_SMALL_WINDOW_S.as_f32(marker));
    editor.set_smooth_small_window_multiplier(
        BXT_TAS_STUDIO_SMOOTH_SMALL_WINDOW_MULTIPLIER.as_f32(marker),
    );
    editor.set_norefresh_until_stop_frame(norefresh_until_stop_frame_frame_idx(marker, editor));

    // SAFETY: if we have access to TriangleApi, it's safe to do player tracing too.
    let tracer = unsafe { Tracer::new(marker, true) }.unwrap();

    let (width, height) = unsafe { engine::get_resolution(marker) };
    let world_to_screen = |world| {
        let screen_normalized = tri.world_to_screen(world)?;
        let screen = (screen_normalized * Vec2::new(1., -1.) + 1.) / 2.
            * Vec2::new(width as f32, height as f32);
        Some(screen)
    };
    let mouse = sdl::mouse_state(marker);

    let last_buttons = LAST_BUTTONS.get(marker);
    let keyboard = KeyboardState {
        adjust_faster: last_buttons.contains(Buttons::IN_ALT1),
        adjust_slower: last_buttons.contains(Buttons::IN_DUCK),
        insert_camera_line: INSERT_CAMERA_LINE_DOWN.get(marker),
    };

    let deadline = Instant::now() + Duration::from_millis(20);
    if let Err(err) = editor.tick(&tracer, world_to_screen, mouse, keyboard, deadline) {
        con_print(marker, &format!("Error ticking the TAS editor: {err}\n"));
        *state = State::Idle;
        return;
    }

    if *last_generation != editor.generation() || *last_branch_idx != editor.branch_idx() {
        *last_generation = editor.generation();
        *last_branch_idx = editor.branch_idx();
        *simulate_at = Some(Instant::now() + Duration::from_millis(100));
    }
    if let Some(at) = *simulate_at {
        if Instant::now() > at {
            *simulate_at = None;
            remote::maybe_send_request_to_client(PlayRequest {
                script: editor.script().clone(),
                generation: *last_generation,
                branch_idx: *last_branch_idx,
                is_smoothed: false,
            });

            bridge.update_on_disk(editor.script().clone());
        }
    }

    let gl = crate::gl::GL.borrow(marker);
    if let Some(gl) = gl.as_ref() {
        let width = BXT_TAS_STUDIO_LINE_WIDTH.as_f32(marker).max(0.);
        unsafe {
            gl.LineWidth(width);
        }
    }

    editor.draw(tri);

    if let Some(gl) = gl.as_ref() {
        unsafe {
            gl.LineWidth(1.);
        }
    }
}

fn add_frame_bulk_hud_lines(text: &mut Vec<u8>, bulk: &FrameBulk) {
    // Add strafing info.
    text.extend(b"Strafing:\0");
    match bulk.auto_actions.movement {
        Some(AutoMovement::Strafe(settings)) => {
            text.extend(b"  ");
            hltas::write::gen_strafe(&mut *text, settings).unwrap();

            text.extend(b" (");
            match settings.type_ {
                StrafeType::MaxAccel => text.extend(b"speed increasing"),
                StrafeType::MaxAngle => text.extend(b"quick turn"),
                StrafeType::MaxDeccel => text.extend(b"slow down"),
                StrafeType::ConstSpeed => text.extend(b"constant speed"),
                StrafeType::ConstYawspeed(yawspeed) => {
                    write!(text, "turn rate: {yawspeed:.0}").unwrap();
                }
                StrafeType::MaxAccelYawOffset {
                    start,
                    target,
                    accel,
                } => {
                    // The values are so tiny that only this would make it sensible.
                    let start = start * 100.;
                    let target = target * 100.;
                    let accel = accel * 100.;
                    write!(text, "{start:.0} {target:.0} {accel:.2}").unwrap();
                }
            }
            text.extend(b")\0");
        }
        _ => text.extend(b"  disabled\0"),
    };

    // Add auto actions.
    text.extend(b"Enabled Actions:\0");

    if let Some(LeaveGroundAction { speed, type_, .. }) = bulk.auto_actions.leave_ground_action {
        if speed != LeaveGroundActionSpeed::Any {
            text.extend(b"  lgagst\0");
        }

        match type_ {
            LeaveGroundActionType::Jump => text.extend(b"  auto jump\0"),
            LeaveGroundActionType::DuckTap { zero_ms: false } => text.extend(b"  duck tap\0"),
            LeaveGroundActionType::DuckTap { zero_ms: true } => text.extend(b"  duck tap (0 ms)\0"),
        };
    }

    if bulk.auto_actions.jump_bug.is_some() {
        text.extend(b"  jump bug\0");
    }
    if let Some(DuckBeforeCollision {
        including_ceilings, ..
    }) = bulk.auto_actions.duck_before_collision
    {
        text.extend(b"  duck before collision\0");
        if including_ceilings {
            text.extend(b"    (incl. ceilings)\0");
        }
    }
    if bulk.auto_actions.duck_before_ground.is_some() {
        text.extend(b"  duck before ground\0");
    }
    if bulk.auto_actions.duck_when_jump.is_some() {
        text.extend(b"  duck when jump\0");
    }

    // Add movement keys.
    if bulk.movement_keys.forward {
        text.extend(b"  forward\0");
    }
    if bulk.movement_keys.left {
        text.extend(b"  left\0");
    }
    if bulk.movement_keys.right {
        text.extend(b"  right\0");
    }
    if bulk.movement_keys.back {
        text.extend(b"  back\0");
    }
    if bulk.movement_keys.up {
        text.extend(b"  up\0");
    }
    if bulk.movement_keys.down {
        text.extend(b"  down\0");
    }

    // Add action keys.
    if bulk.action_keys.jump {
        text.extend(b"  jump\0");
    }
    if bulk.action_keys.duck {
        text.extend(b"  duck\0");
    }
    if bulk.action_keys.use_ {
        text.extend(b"  use\0");
    }
    if bulk.action_keys.attack_1 {
        text.extend(b"  attack1\0");
    }
    if bulk.action_keys.attack_2 {
        text.extend(b"  attack2\0");
    }
    if bulk.action_keys.reload {
        text.extend(b"  reload\0");
    }

    // Add other parameters.
    write!(text, "Frame Count: {}\0", bulk.frame_count).unwrap();
    write!(text, "Frame Time: {}\0", &bulk.frame_time).unwrap();
    if let Some(pitch) = bulk.pitch {
        write!(text, "Pitch: {pitch:.3}\0").unwrap();
    }
    if let Some(yaw) = bulk.yaw() {
        write!(text, "Yaw: {yaw:.3}\0").unwrap();
    }
    if let Some(AutoMovement::Strafe(StrafeSettings {
        dir: StrafeDir::LeftRight(count) | StrafeDir::RightLeft(count),
        ..
    })) = bulk.auto_actions.movement
    {
        write!(text, "Left-Right Frame Count: {count}\0").unwrap();
    }
    if let Some(command) = &bulk.console_command {
        text.extend(b"Commands:\0");
        write!(text, "  {command}\0").unwrap();
    }
}

pub fn draw_hud(marker: MainThreadMarker, draw: &hud::Draw) {
    if !TasStudio.is_enabled(marker) {
        return;
    }

    if !BXT_HUD_TAS_STUDIO.as_bool(marker) {
        return;
    }

    let state = STATE.borrow(marker);
    let State::Editing { editor, .. } = &*state else {
        return;
    };

    let _span = info_span!("tas_studio::draw_hud").entered();

    let info = hud::screen_info(marker);

    let mut text = Vec::with_capacity(1024);
    text.extend(b"TAS Studio Status\0");

    write!(&mut text, "Re-records: {}\0", editor.undo_log_len()).unwrap();

    write!(&mut text, "Branch #{}\0", editor.branch_idx()).unwrap();

    match editor.selected_bulk_idx() {
        None => text.extend(b"  no frame bulk selected\0"),
        Some(selected_bulk_idx) => {
            let script = editor.script();

            let (bulk, _) = bulk_and_first_frame_idx(script)
                .nth(selected_bulk_idx)
                .unwrap();

            add_frame_bulk_hud_lines(&mut text, bulk);
        }
    };

    if let Some(hovered_frame) = editor.hovered_frame() {
        let hovered_frame_idx = editor.hovered_frame_idx().unwrap();
        add_hovered_frame_hud_lines(&mut text, hovered_frame_idx, hovered_frame);
    }

    // Measure using our longest string and draw background.
    let height = text.split_inclusive(|c| *c == b'\0').count() as i32 * info.iCharHeight;

    const PADDING: i32 = 8;
    let width = draw.string(
        IVec2::new(0, -info.iCharHeight * 2),
        b"  Duration: 0.001 s (1000 FPS)\0",
    );
    draw.fill(
        IVec2::new(0, 2 * info.iCharHeight),
        IVec2::new(width + 2 * PADDING, height + 2 * PADDING),
        IVec4::new(0, 0, 0, 150),
    );

    let mut ml = draw.multi_line(IVec2::new(PADDING, 2 * info.iCharHeight + PADDING));
    for line in text.split_inclusive(|c| *c == b'\0') {
        ml.line(line);
    }

    if let Some(side_strafe_accelerated_yawspeed_adjustment) =
        editor.side_strafe_accelerated_yawspeed_adjustment()
    {
        draw.string(
            IVec2::new(
                info.iWidth / 2 - info.iCharHeight * 3,
                info.iHeight / 2 + info.iCharHeight * 2,
            ),
            match side_strafe_accelerated_yawspeed_adjustment.mode {
                MaxAccelYawOffsetMode::StartAndTarget => b"Start and Target\0",
                MaxAccelYawOffsetMode::Target => b"Target\0",
                MaxAccelYawOffsetMode::Acceleration => b"Acceleration\0",
                MaxAccelYawOffsetMode::Start => b"Start\0",
                MaxAccelYawOffsetMode::Alt => b"Alt\0",
            },
        );
    }
}

fn add_hovered_frame_hud_lines(text: &mut Vec<u8>, frame_idx: usize, frame: &Frame) {
    text.extend(b"\0Frame Under Cursor:\0");

    write!(text, "  Frame #{}\0", frame_idx).unwrap();

    let frame_time = frame.parameters.frame_time;
    let fps = (1. / frame_time).round();
    write!(text, "  Duration: {frame_time:.3} s ({fps} FPS)\0").unwrap();

    let yaw = frame.state.prev_frame_input.yaw.to_degrees();
    write!(text, "  Yaw: {:.3}\0", yaw).unwrap();
    let pitch = frame.state.prev_frame_input.pitch.to_degrees();
    write!(text, "  Pitch: {:.3}\0", pitch).unwrap();

    write!(
        text,
        "  {:.0} HP {:.1} AP\0",
        frame.state.player.health, frame.state.player.armor,
    )
    .unwrap();

    let vel = frame.state.player.vel;
    write!(text, "  X Speed: {:.1}\0", vel.x).unwrap();
    write!(text, "  Y Speed: {:.1}\0", vel.y).unwrap();
    write!(text, "  Z Speed: {:.1}\0", vel.z).unwrap();
    let xy_speed = vel.truncate().length();
    write!(text, "  XY Speed: {:.1}\0", xy_speed).unwrap();

    write!(text, "  X Pos: {:.1}\0", frame.state.player.pos.x).unwrap();
    write!(text, "  Y Pos: {:.1}\0", frame.state.player.pos.y).unwrap();
    write!(text, "  Z Pos: {:.1}\0", frame.state.player.pos.z).unwrap();

    write!(text, "  Stamina: {:.1}\0", frame.state.player.stamina_time).unwrap();
}

static PREVENT_UNPAUSE: MainThreadCell<bool> = MainThreadCell::new(false);

pub fn maybe_prevent_unpause(marker: MainThreadMarker, closure: impl FnOnce()) {
    let state = STATE.borrow(marker);
    if matches!(*state, State::Editing { .. }) {
        PREVENT_UNPAUSE.set(marker, true);
    }

    closure();

    PREVENT_UNPAUSE.set(marker, false);
}

pub unsafe fn should_skip_command(marker: MainThreadMarker, text: *const i8) -> bool {
    if !PREVENT_UNPAUSE.get(marker) {
        return false;
    }

    if text.is_null() {
        return false;
    }

    let text = CStr::from_ptr(text).to_bytes();
    text == b"unpause"
}

pub fn should_clear(marker: MainThreadMarker) -> bool {
    let state = STATE.borrow(marker);
    matches!(*state, State::Editing { .. })
}

pub unsafe fn on_post_run_cmd(marker: MainThreadMarker, cmd: *mut usercmd_s) {
    LAST_BUTTONS.set(marker, Buttons::from_bits_truncate((*cmd).buttons));
}

pub fn is_main_instance(marker: MainThreadMarker) -> bool {
    let state = STATE.borrow(marker);
    !matches!(*state, State::Idle | State::Playing { .. })
}

pub unsafe fn with_m_rawinput_one<T>(marker: MainThreadMarker, f: impl FnOnce() -> T) -> T {
    // TODO: make good.
    unsafe fn find_cvar(marker: MainThreadMarker, name: &str) -> Option<*mut cvar_s> {
        let mut ptr = *engine::cvar_vars.get_opt(marker)?;
        while !ptr.is_null() {
            match std::ffi::CStr::from_ptr((*ptr).name).to_str() {
                Ok(x) if x == name => {
                    return Some(ptr);
                }
                _ => (),
            }

            ptr = (*ptr).next;
        }

        None
    }

    let m_rawinput = find_cvar(marker, "m_rawinput");
    let mut prev = None;

    if let Some(m_rawinput) = m_rawinput {
        prev = Some((*m_rawinput).value);
        // HACK: this is technically not enough (we're not replacing the string value), but at least
        // the vanilla client only checks the value.
        (*m_rawinput).value = 1.;
    }

    let rv = f();

    if let Some(m_rawinput) = m_rawinput {
        (*m_rawinput).value = prev.unwrap();
    }

    rv
}

pub unsafe fn should_unpause_calcrefdef(marker: MainThreadMarker) -> bool {
    let state = STATE.borrow(marker);
    matches!(*state, State::Editing { .. })
}

pub unsafe fn maybe_enable_freecam(marker: MainThreadMarker) {
    if ENABLE_FREECAM_ON_CALCREFDEF.get(marker) {
        ENABLE_FREECAM_ON_CALCREFDEF.set(marker, false);
        engine::prepend_command(marker, "bxt_freecam 1\n");
    }
}
