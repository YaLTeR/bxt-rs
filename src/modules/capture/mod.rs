//! Video capture.

use std::{mem, os::raw::c_char};

use rust_hawktracer::*;

use super::{cvars::CVar, Module};
use crate::{
    handler,
    hooks::engine::{self, con_print},
    modules::commands::Command,
    utils::*,
};

pub struct Capture;
impl Module for Capture {
    fn name(&self) -> &'static str {
        "Video capture"
    }

    fn commands(&self) -> &'static [&'static Command] {
        static COMMANDS: &[&Command] = &[&BXT_CAP_START, &BXT_CAP_STOP];
        COMMANDS
    }

    fn cvars(&self) -> &'static [&'static CVar] {
        static CVARS: &[&CVar] = &[
            &BXT_CAP_FPS,
            &BXT_CAP_VOLUME,
            &BXT_CAP_SOUND_EXTRA,
            &BXT_CAP_FORCE_FALLBACK,
        ];
        CVARS
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        engine::cls_demos.is_set(marker)
            && engine::Host_FilterTime.is_set(marker)
            && engine::host_frametime.is_set(marker)
            && engine::paintbuffer.is_set(marker)
            && engine::paintedtime.is_set(marker)
            && engine::realtime.is_set(marker)
            && engine::S_PaintChannels.is_set(marker)
            && engine::S_TransferStereo16.is_set(marker)
            && engine::shm.is_set(marker)
            && engine::Sys_VID_FlipScreen.is_set(marker)
            && engine::VideoMode_GetCurrentVideoMode.is_set(marker)
            && engine::VideoMode_IsWindowed.is_set(marker)
            && engine::window_rect.is_set(marker)
    }
}

mod muxer;
mod opengl;
mod recorder;
use recorder::{CaptureType, Recorder};
mod vulkan;

#[cfg(unix)]
pub type ExternalObject = std::os::unix::io::RawFd;
#[cfg(windows)]
pub type ExternalObject = *mut std::os::raw::c_void;

static BXT_CAP_FPS: CVar = CVar::new(b"bxt_cap_fps\0", b"60\0");
static BXT_CAP_SOUND_EXTRA: CVar = CVar::new(b"bxt_cap_sound_extra\0", b"0\0");
static BXT_CAP_VOLUME: CVar = CVar::new(b"bxt_cap_volume\0", b"0.4\0");
static BXT_CAP_FORCE_FALLBACK: CVar = CVar::new(b"_bxt_cap_force_fallback\0", b"0\0");

static HAVE_REQUIRED_GL_EXTENSIONS: MainThreadCell<bool> = MainThreadCell::new(false);

pub fn check_gl_extensions(marker: MainThreadMarker, is_supported: impl Fn(*const c_char) -> bool) {
    let mut have_everything = true;

    if !is_supported(b"GL_EXT_memory_object\0".as_ptr().cast()) {
        warn!("MISSING: GL_EXT_memory_object OpenGL extension");
        have_everything = false;
    }

    #[cfg(unix)]
    if !is_supported(b"GL_EXT_memory_object_fd\0".as_ptr().cast()) {
        warn!("MISSING: GL_EXT_memory_object_fd OpenGL extension");
        have_everything = false;
    }

    #[cfg(windows)]
    if !is_supported(b"GL_EXT_memory_object_win32\0".as_ptr().cast()) {
        warn!("MISSING: GL_EXT_memory_object_win32 OpenGL extension");
        have_everything = false;
    }

    if !is_supported(b"GL_EXT_semaphore\0".as_ptr().cast()) {
        warn!("MISSING: GL_EXT_semaphore OpenGL extension");
        have_everything = false;
    }

    #[cfg(unix)]
    if !is_supported(b"GL_EXT_semaphore_fd\0".as_ptr().cast()) {
        warn!("MISSING: GL_EXT_semaphore_fd OpenGL extension");
        have_everything = false;
    }

    #[cfg(windows)]
    if !is_supported(b"GL_EXT_semaphore_win32\0".as_ptr().cast()) {
        warn!("MISSING: GL_EXT_semaphore_win32 OpenGL extension");
        have_everything = false;
    }

    HAVE_REQUIRED_GL_EXTENSIONS.set(marker, have_everything);
}

pub fn reset_gl_state(marker: MainThreadMarker) {
    HAVE_REQUIRED_GL_EXTENSIONS.set(marker, false);

    if let State::Recording(ref mut recorder) = *STATE.borrow_mut(marker) {
        recorder.reset_opengl();
    }
}

#[derive(Clone, Copy)]
pub enum SoundCaptureMode {
    /// Floor time to sample boundary.
    Normal,

    /// Ceil time to sample boundary and capture this number of seconds of extra sound.
    Remaining { extra: f32 },
}

#[allow(clippy::large_enum_variant)]
enum State {
    Idle,
    Starting(String),
    Recording(Recorder),
}

impl State {
    fn set(&mut self, new: Self) {
        let old_state = mem::replace(self, new);

        if let State::Recording(recorder) = old_state {
            recorder.finish();
        }
    }
}

static STATE: MainThreadRefCell<State> = MainThreadRefCell::new(State::Idle);

static BXT_CAP_START: Command = Command::new(
    b"bxt_cap_start\0",
    handler!(
        "Usage: bxt_cap_start [filename.mp4]\n \
          Starts capturing video. The default filename is \"output.mp4\".\n",
        cap_start as fn(_),
        cap_start_with_filename as fn(_, _)
    ),
);

fn cap_start(marker: MainThreadMarker) {
    cap_start_with_filename(marker, "output.mp4".to_string());
}

fn cap_start_with_filename(marker: MainThreadMarker, filename: String) {
    if !Capture.is_enabled(marker) {
        return;
    }

    if !filename.ends_with(".mp4") {
        con_print(marker, "Error: the filename must end with \".mp4\".\n");
        return;
    }

    let mut state = STATE.borrow_mut(marker);
    if !matches!(*state, State::Idle) {
        // Already capturing.
        return;
    }

    *state = State::Starting(filename);
}

static BXT_CAP_STOP: Command = Command::new(
    b"bxt_cap_stop\0",
    handler!(
        "Usage: bxt_cap_stop\n \
          Stops capturing video.\n",
        cap_stop as fn(_)
    ),
);

fn cap_stop(marker: MainThreadMarker) {
    unsafe {
        let mut state = STATE.borrow_mut(marker);
        if let State::Recording(ref mut recorder) = *state {
            match recorder.record_last_frame() {
                Ok(()) => {
                    drop(state);
                    let extra = BXT_CAP_SOUND_EXTRA.as_f32(marker);
                    capture_sound(marker, SoundCaptureMode::Remaining { extra });
                }
                Err(err) => error!("{:?}", err),
            }
        }
    }

    STATE.borrow_mut(marker).set(State::Idle);
}

pub unsafe fn capture_frame(marker: MainThreadMarker) {
    if !Capture.is_enabled(marker) {
        return;
    }

    let mut state = STATE.borrow_mut(marker);
    if matches!(*state, State::Idle) {
        return;
    }

    scoped_tracepoint!(_capture_frame);

    let (width, height) = engine::get_resolution(marker);

    // Initialize the recording if needed.
    if let State::Starting(ref filename) = *state {
        let fps = BXT_CAP_FPS.as_u64(marker).max(1);

        let capture_type =
            if HAVE_REQUIRED_GL_EXTENSIONS.get(marker) && !BXT_CAP_FORCE_FALLBACK.as_bool(marker) {
                CaptureType::Vulkan
            } else {
                CaptureType::ReadPixels
            };
        match Recorder::init(width, height, fps, capture_type, filename) {
            Ok(recorder) => {
                if recorder.capture_type() == CaptureType::ReadPixels {
                    con_print(marker, "Recording in slower fallback mode.\n");
                }
                *state = State::Recording(recorder)
            }
            Err(err) => {
                error!("error initializing the recorder: {:?}", err);
                con_print(marker, &format!("Error initializing recording: {}.\n", err));
                *state = State::Idle;
                return;
            }
        }
    }

    let recorder = match *state {
        State::Recording(ref mut recorder) => recorder,
        _ => unreachable!(),
    };

    // Now that we have the duration of the last frame, record it.
    if let Err(err) = recorder.record_last_frame() {
        error!("{:?}", err);
        con_print(marker, "Error during recording, stopping.\n");
        drop(state);
        cap_stop(marker);
        return;
    }

    // Check for resolution changes.
    if recorder.width() != width || recorder.height() != height {
        con_print(
            marker,
            &format!(
                "Resolution has changed: {}×{} => {}×{}, stopping recording.\n",
                recorder.width(),
                recorder.height(),
                width,
                height
            ),
        );
        cap_stop(marker);
        return;
    }

    // Capture this frame for recording later.
    if let Err(err) = recorder.capture_opengl(marker) {
        error!("{:?}", err);
        con_print(marker, "Error during recording, stopping.\n");
        drop(state);
        cap_stop(marker);
        return;
    }
}

pub unsafe fn skip_paint_channels(marker: MainThreadMarker) -> bool {
    // During recording we're capturing sound manually and don't want the game to mess with it.
    matches!(*STATE.borrow_mut(marker), State::Recording(_))
}

#[hawktracer(capture_sound)]
pub unsafe fn capture_sound(marker: MainThreadMarker, mode: SoundCaptureMode) {
    let end_time = {
        let mut state = STATE.borrow_mut(marker);
        let recorder = match *state {
            State::Recording(ref mut recorder) => recorder,
            _ => unreachable!(),
        };

        let samples_per_second = (**engine::shm.get(marker)).speed;
        let samples = recorder.samples_to_capture(samples_per_second, mode);

        let painted_time = *engine::paintedtime.get(marker);
        painted_time + samples as i32
    };

    engine::S_PaintChannels.get(marker)(end_time);
}

pub unsafe fn on_s_transfer_stereo_16(marker: MainThreadMarker, end: i32) {
    let mut state = STATE.borrow_mut(marker);
    let recorder = match *state {
        State::Recording(ref mut recorder) => recorder,
        _ => return,
    };

    let painted_time = *engine::paintedtime.get(marker);
    let paint_buffer = &*engine::paintbuffer.get(marker);
    let sample_count = (end - painted_time) as usize * 2;

    let volume = (BXT_CAP_VOLUME.as_f32(marker) * 256.) as i32;

    let mut buf = [0; 1026 * 4];
    for (sample, buf) in paint_buffer
        .iter()
        .take(sample_count)
        .zip(buf.chunks_exact_mut(4))
    {
        // Clamping as done in Snd_WriteLinearBlastStereo16().
        let l16 = ((sample.left * volume) >> 8).min(32767).max(-32768) as i16;
        let r16 = ((sample.right * volume) >> 8).min(32767).max(-32768) as i16;

        buf[0..2].copy_from_slice(&l16.to_le_bytes());
        buf[2..4].copy_from_slice(&r16.to_le_bytes());
    }

    recorder.write_audio_frame((&buf[..sample_count * 4]).into());
}

pub unsafe fn on_host_filter_time(marker: MainThreadMarker) -> bool {
    let mut state = STATE.borrow_mut(marker);
    let recorder = match *state {
        State::Recording(ref mut recorder) => recorder,
        _ => return false,
    };

    if (*engine::cls_demos.get(marker)).demoplayback == 0 {
        return false;
    }

    *engine::host_frametime.get(marker) = recorder.time_base();
    let realtime = engine::realtime.get(marker);
    *realtime += recorder.time_base();

    true
}

pub unsafe fn on_cl_disconnect(marker: MainThreadMarker) {
    {
        // Safety: no engine functions are called while the reference is active.
        let cls_demos = &mut *engine::cls_demos.get(marker);

        // Wasn't playing back a demo.
        if cls_demos.demoplayback == 0 {
            return;
        }

        // Will play another demo right after.
        if cls_demos.demonum != -1 && cls_demos.demos[0][0] != 0 {
            return;
        }
    }

    cap_stop(marker);
}

static INSIDE_KEY_EVENT: MainThreadCell<bool> = MainThreadCell::new(false);

pub fn on_key_event_start(marker: MainThreadMarker) {
    INSIDE_KEY_EVENT.set(marker, true);
}

pub fn on_key_event_end(marker: MainThreadMarker) {
    INSIDE_KEY_EVENT.set(marker, false);
}

pub fn prevent_toggle_console(marker: MainThreadMarker) -> bool {
    INSIDE_KEY_EVENT.get(marker)
}

pub unsafe fn time_passed(marker: MainThreadMarker) {
    let mut state = STATE.borrow_mut(marker);
    let recorder = match *state {
        State::Recording(ref mut recorder) => recorder,
        _ => return,
    };

    // Accumulate time for the last frame.
    let time = *engine::host_frametime.get(marker);
    recorder.time_passed(time);

    // Capture sound ASAP.
    //
    // In normal operation the sound was already mixed for the next _snd_mixahead (0.1) seconds and
    // is already playing. If we delay sound capturing until the next flip screen, we'll get next
    // frame's sounds mixed in on this frame, resulting in the audio being slightly ahead of the
    // video.
    drop(state);
    capture_sound(marker, SoundCaptureMode::Normal);
}
