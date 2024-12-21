//! Video capture into separate videos per demo.

use std::ffi::{CStr, CString, OsStr};
use std::path::PathBuf;
use std::time::Duration;
use std::{fs, mem};

use super::commands::{self, Command};
use super::cvars::CVar;
use super::{capture, demo_playback, Module};
use crate::handler;
use crate::hooks::engine::{self, con_print};
use crate::utils::*;

mod remote;

pub use remote::{
    maybe_receive_request_from_remote_server, maybe_start_client_connection_thread,
    update_client_connection_condition,
};
use remote::{maybe_receive_status_and_send_requests, start_server};

pub struct CaptureVideoPerDemo;
impl Module for CaptureVideoPerDemo {
    fn name(&self) -> &'static str {
        "Video capture (one video per demo)"
    }

    fn description(&self) -> &'static str {
        "Recording separate video for each demo being played."
    }

    fn commands(&self) -> &'static [&'static Command] {
        static COMMANDS: &[&Command] = &[&BXT_CAP_SEPARATE_START];
        COMMANDS
    }

    fn cvars(&self) -> &'static [&'static CVar] {
        static CVARS: &[&CVar] = &[
            &BXT_CAP_SEPARATE_MULTIGAME,
            &BXT_CAP_SEPARATE_MULTIGAME_EXEC,
        ];
        CVARS
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        capture::Capture.is_enabled(marker)
            && commands::Commands.is_enabled(marker)
            && engine::CL_PlayDemo_f.is_set(marker)
            && engine::cls_demos.is_set(marker)
            && demo_playback::DemoPlayback.is_enabled(marker)
            && engine::Host_FilterTime.is_set(marker)
            && engine::host_frametime.is_set(marker)
            && engine::com_gamedir.is_set(marker)
    }
}

static BXT_CAP_SEPARATE_START: Command = Command::new(
    b"bxt_cap_separate_start\0",
    handler!(
        "bxt_cap_separate_start [directory]

Starts recording every demo into its own separate video with the same file name. If directory is
provided, stores the videos in that directory, otherwise stores them in the same directory as the
corresponding demo file.

This command should be coupled with `playdemo`, `bxt_play_run` or `bxt_play_folder`. For example:
`bxt_play_folder my_folder_with_demos;bxt_cap_separate_start`.

Use `bxt_cap_stop` to stop the recording.",
        cap_separate_start as fn(_),
        cap_separate_start_with_dir as fn(_, _)
    ),
);

static BXT_CAP_SEPARATE_MULTIGAME: CVar = CVar::new(
    b"bxt_cap_separate_multigame\0",
    b"0\0",
    "\
Enables multi-game recording when used along with bxt_play_folder/run.

Simply starting another instance of the game to capture.",
);

static BXT_CAP_SEPARATE_MULTIGAME_EXEC: CVar = CVar::new(
    b"bxt_cap_separate_multigame_exec\0",
    b"\0",
    "\
Sets the config file name (.cfg) to load before capturing.",
);

/// Name of the demo currently being played back.
static CURRENT_DEMO: MainThreadRefCell<Option<CString>> = MainThreadRefCell::new(None);
/// Name of the demo about to be played back.
///
/// Necessary because CL_PlayDemo_f() calls CL_Disconnect().
static CURRENT_DEMO_PENDING: MainThreadRefCell<Option<CString>> = MainThreadRefCell::new(None);
/// Whether the module is active.
///
/// If `true`, playing a new demo will start a new video recording.
static IS_ACTIVE: MainThreadCell<bool> = MainThreadCell::new(false);
/// Target directory to save recorded videos.
///
/// If `None`, videos are saved in the same folders as the demos.
static TARGET_DIR: MainThreadRefCell<Option<PathBuf>> = MainThreadRefCell::new(None);

// static RECORD_REQUEST: MainThreadRefCell<Option<RecordRequest>> = MainThreadRefCell::new(None);
// static PLAY_REQUEST: Arc<Mutex<Option<RecordRequest>>> = Arc::new(Mutex::new(None));

fn cap_separate_start(marker: MainThreadMarker) {
    if !CaptureVideoPerDemo.is_enabled(marker) {
        return;
    }

    *TARGET_DIR.borrow_mut(marker) = None;
    IS_ACTIVE.set(marker, true);

    engine::con_print(
        marker,
        "Demos will be recorded into videos. Use bxt_cap_stop to stop.\n",
    );

    // If we're already playing a demo, start capturing.
    maybe_start_capture(marker);
}

fn cap_separate_start_with_dir(marker: MainThreadMarker, target_dir: PathBuf) {
    if !CaptureVideoPerDemo.is_enabled(marker) {
        return;
    }

    if let Err(err) = fs::create_dir_all(&target_dir) {
        con_print(marker, &format!("Error creating output directory: {err}\n"));
        return;
    }

    engine::con_print(
        marker,
        &format!(
            "Demos will be recorded into videos in {}. Use bxt_cap_stop to stop.\n",
            target_dir.to_string_lossy()
        ),
    );

    *TARGET_DIR.borrow_mut(marker) = Some(target_dir);
    IS_ACTIVE.set(marker, true);

    // If we're already playing a demo, start capturing.
    maybe_start_capture(marker);
}

fn maybe_start_capture(marker: MainThreadMarker) {
    if let Err(err) = start_server() {
        con_print(
            marker,
            &format!("Could not start a server for multi-game recording: {err:?}"),
        );
    }

    let Some(current_demo) = &*CURRENT_DEMO.borrow(marker) else {
        return;
    };
    let current_demo = c_str_to_os_string(current_demo);
    let current_demo = PathBuf::from(current_demo);

    let mut output_path = PathBuf::new();
    match &*TARGET_DIR.borrow(marker) {
        Some(target_dir) => {
            output_path.push(target_dir);
            output_path.push(current_demo.file_name().unwrap_or(OsStr::new("output")));
        }
        None => {
            if let Some(game_dir) = engine::com_gamedir.get_opt(marker) {
                output_path =
                    PathBuf::from(unsafe { CStr::from_ptr(game_dir.cast()) }.to_str().unwrap());
            }
            output_path.push(current_demo);
        }
    }
    output_path.set_extension("mp4");

    let output_path = output_path.to_string_lossy().to_string();
    engine::con_print(marker, &format!("Recording into {}.\n", &output_path));
    capture::cap_start_with_filename(marker, output_path);
}

pub fn stop(marker: MainThreadMarker) {
    IS_ACTIVE.set(marker, false);
}

pub unsafe fn on_before_cl_playdemo_f(marker: MainThreadMarker) {
    if !CaptureVideoPerDemo.is_enabled(marker) {
        return;
    }

    let Some(demo_name) = commands::Args::new(marker).nth(1) else {
        return;
    };
    *CURRENT_DEMO_PENDING.borrow_mut(marker) = Some(demo_name.to_owned());
}

pub unsafe fn on_after_cl_playdemo_f(marker: MainThreadMarker) {
    if !CaptureVideoPerDemo.is_enabled(marker) {
        return;
    }

    *CURRENT_DEMO.borrow_mut(marker) = mem::take(&mut *CURRENT_DEMO_PENDING.borrow_mut(marker));

    {
        // Safety: no engine functions are called while the reference is active.
        let cls_demos = &*engine::cls_demos.get(marker);

        // Has not started playing a demo.
        if cls_demos.demoplayback == 0 {
            return;
        }
    }

    if IS_ACTIVE.get(marker) {
        // Start recording the next demo.
        maybe_start_capture(marker);
    }
}

/// Returns `true` if capture::on_cl_disconnect() should be prevented.
pub unsafe fn on_cl_disconnect(marker: MainThreadMarker) -> bool {
    if !CaptureVideoPerDemo.is_enabled(marker) {
        return false;
    }

    *CURRENT_DEMO.borrow_mut(marker) = None;

    if !IS_ACTIVE.get(marker) {
        return false;
    }

    {
        // Safety: no engine functions are called while the reference is active.
        let cls_demos = &*engine::cls_demos.get(marker);

        // Wasn't playing back a demo.
        if cls_demos.demoplayback == 0 {
            return true;
        }
    }

    capture::cap_stop(marker);

    // cap_stop will reset IS_ACTIVE, but we need to keep recording if there are more demos in the
    // queue. Therefore check the demo queue and reset IS_ACTIVE back to true if there are more.

    {
        // Safety: no engine functions are called while the reference is active.
        let cls_demos = &*engine::cls_demos.get(marker);

        // Will play another demo right after.
        if cls_demos.demonum != -1 && cls_demos.demos[0][0] != 0 {
            IS_ACTIVE.set(marker, true);
        }
    }

    true
}

static POLLING_TIMER: MainThreadRefCell<Duration> = MainThreadRefCell::new(Duration::from_secs(2));

pub fn capture_video_per_demo_multigame_polling(marker: MainThreadMarker) {
    if !CaptureVideoPerDemo.is_enabled(marker) {
        return;
    }

    let mut timer = POLLING_TIMER.borrow_mut(marker);

    if !timer.is_zero() {
        unsafe {
            *timer =
                timer.saturating_sub(Duration::from_secs_f64(*engine::host_frametime.get(marker)))
        };

        return;
    }

    *timer += Duration::from_secs(2);

    if BXT_CAP_SEPARATE_MULTIGAME.as_bool(marker) {
        maybe_receive_status_and_send_requests(marker);
    }

    update_client_connection_condition(marker);

    unsafe {
        maybe_receive_request_from_remote_server(marker);
    }
}
