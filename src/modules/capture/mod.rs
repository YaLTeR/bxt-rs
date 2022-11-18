//! Video capture.

use std::mem;

use color_eyre::eyre::Context;

use super::cvars::CVar;
use super::Module;
use crate::hooks::engine::{self, con_print};
use crate::modules::commands::Command;
use crate::utils::*;
use crate::{gl, handler};

pub struct Capture;
impl Module for Capture {
    fn name(&self) -> &'static str {
        "Video capture"
    }

    fn description(&self) -> &'static str {
        "Recording videos from demos and TAS playback."
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
            &BXT_CAP_SLOWDOWN,
            &BXT_CAP_SAMPLING_EXPOSURE,
            &BXT_CAP_FORCE_FALLBACK,
            &BXT_CAP_OVERRIDE_FFMPEG_ARGS,
            &BXT_CAP_SAMPLING_MIN_FPS,
        ];
        CVARS
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        gl::GL.borrow(marker).is_some()
            && engine::cls_demos.is_set(marker)
            && engine::Host_FilterTime.is_set(marker)
            && engine::host_frametime.is_set(marker)
            && engine::paintbuffer.is_set(marker)
            && engine::paintedtime.is_set(marker)
            && engine::realtime.is_set(marker)
            && engine::S_PaintChannels.is_set(marker)
            && engine::S_TransferStereo16.is_set(marker)
            && engine::shm.is_set(marker)
            && (engine::Sys_VID_FlipScreen.is_set(marker)
                || engine::Sys_VID_FlipScreen_old.is_set(marker))
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

static BXT_CAP_FPS: CVar = CVar::new(
    b"bxt_cap_fps\0",
    b"60\0",
    "Frames-per-second of the recorded video.",
);
static BXT_CAP_SOUND_EXTRA: CVar = CVar::new(
    b"bxt_cap_sound_extra\0",
    b"0\0",
    "How many extra seconds of audio to mix and capture past the end of the recording.",
);
static BXT_CAP_VOLUME: CVar = CVar::new(
    b"bxt_cap_volume\0",
    b"0.4\0",
    "\
Volume of the recording.

This is the same as the `volume` console variable, but for the recorded video. The `volume` \
variable itself does not affect the recorded video.",
);
static BXT_CAP_SLOWDOWN: CVar = CVar::new(
    b"bxt_cap_slowdown\0",
    b"1\0",
    "\
Slowdown factor for the recording.

For example, `2` means that the video will be two times slower than the realtime playback. \
Especially useful for TASes.",
);
static BXT_CAP_SAMPLING_MIN_FPS: CVar = CVar::new(
    b"_bxt_cap_sampling_min_fps\0",
    b"7200\0",
    "Minimum recording FPS for frames that make up a sampled frame.",
);
static BXT_CAP_SAMPLING_EXPOSURE: CVar = CVar::new(
    b"bxt_cap_sampling_exposure\0",
    b"0\0",
    "\
How much of the sampled frame contributes to it. E.g. `1` means that the whole frame duration \
is averaged, `0.5` means that half of the frame is averaged, `0.25` means that a quarter of the \
frame is averaged, and so on. The averaging always happens towards the end of the frame: that is, \
an exposure of `0.5` means that every frame is an average of the second half of that frame's \
duration.

`0` disables sampling.",
);
static BXT_CAP_FORCE_FALLBACK: CVar = CVar::new(
    b"_bxt_cap_force_fallback\0",
    b"0\0",
    "Set to `1` to force the use of simple OpenGL capturing instead of the fast \
    GPU-accelerated Vulkan capturing. Try this if you get artifacts on the recorded video.",
);
static BXT_CAP_OVERRIDE_FFMPEG_ARGS: CVar = CVar::new(
    b"_bxt_cap_override_ffmpeg_args\0",
    b"\0",
    "\
Extra arguments to pass to FFmpeg.

When using this variable, you might want to also add some of the following arguments, that bxt-rs \
adds automatically when this variable is unset: `-c:v libx264 -crf 15 -preset ultrafast \
-color_primaries bt709 -color_trc bt709 -colorspace bt709 -color_range tv \
-chroma_sample_location center`.",
);

static HAVE_REQUIRED_GL_EXTENSIONS: MainThreadCell<bool> = MainThreadCell::new(false);

pub fn check_gl_extensions(marker: MainThreadMarker, is_supported: impl Fn(&'static str) -> bool) {
    let mut have_everything = true;

    if !is_supported("GL_EXT_memory_object") {
        warn!("MISSING: GL_EXT_memory_object OpenGL extension");
        have_everything = false;
    }

    #[cfg(unix)]
    if !is_supported("GL_EXT_memory_object_fd") {
        warn!("MISSING: GL_EXT_memory_object_fd OpenGL extension");
        have_everything = false;
    }

    #[cfg(windows)]
    if !is_supported("GL_EXT_memory_object_win32") {
        warn!("MISSING: GL_EXT_memory_object_win32 OpenGL extension");
        have_everything = false;
    }

    if !is_supported("GL_EXT_semaphore") {
        warn!("MISSING: GL_EXT_semaphore OpenGL extension");
        have_everything = false;
    }

    #[cfg(unix)]
    if !is_supported("GL_EXT_semaphore_fd") {
        warn!("MISSING: GL_EXT_semaphore_fd OpenGL extension");
        have_everything = false;
    }

    #[cfg(windows)]
    if !is_supported("GL_EXT_semaphore_win32") {
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

#[derive(Debug, Clone, Copy)]
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

static STATE: MainThreadRefCell<State> = MainThreadRefCell::new(State::Idle);

static BXT_CAP_START: Command = Command::new(
    b"bxt_cap_start\0",
    handler!(
        "bxt_cap_start [filename.mp4]

Starts capturing video. The default filename is `output.mp4`.

If the filename ends with `.wav`, captures only the sound.",
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

    if !filename.ends_with(".mp4") && !filename.ends_with(".wav") {
        con_print(
            marker,
            "Error: the filename must end with \".mp4\" or \".wav\".\n",
        );
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
        "bxt_cap_stop

Stops capturing video.",
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

    // Couldn't replace above because capture_sound() needs the state in place.
    let old_state = mem::replace(&mut *STATE.borrow_mut(marker), State::Idle);
    let stopped = !matches!(old_state, State::Idle);
    if let State::Recording(recorder) = old_state {
        if let Some(ffmpeg_output) = recorder.finish() {
            let output = ffmpeg_output.trim();
            if !output.is_empty() {
                warn!("FFmpeg output:\n{}", output);
                con_print(marker, &format!("FFmpeg output:\n{}\n", output));
            }
        }
    }

    if stopped {
        con_print(marker, "Recording stopped.\n");
    }
}

pub unsafe fn capture_frame(marker: MainThreadMarker) {
    if !Capture.is_enabled(marker) {
        return;
    }

    let mut state = STATE.borrow_mut(marker);
    if matches!(*state, State::Idle) {
        return;
    }

    let _span = info_span!("capture_frame").entered();

    let (width, height) = engine::get_resolution(marker);

    // Initialize the recording if needed.
    if let State::Starting(ref filename) = *state {
        let fps = BXT_CAP_FPS.as_u64(marker).max(1);
        let slowdown = BXT_CAP_SLOWDOWN.as_f32(marker).max(0.1) as f64;

        let capture_type = if HAVE_REQUIRED_GL_EXTENSIONS.get(marker)
            && !BXT_CAP_FORCE_FALLBACK.as_bool(marker)
            // Check Vulkan last.
            //
            // On some Windows AMD GPU setups initializing Vulkan and then doing anything with
            // OpenGL causes a crash in the driver (atioglxx.dll). To remedy this, we initialize
            // Vulkan lazily. This way if you don't use video recording, or if you have
            // _bxt_cap_force_fallback 1, Vulkan is never initialized, so you can avoid the crash.
            && crate::vulkan::VULKAN.is_some()
        {
            match opengl::get_uuids(marker).wrap_err("error getting OpenGL UUIDs") {
                Ok(uuids) => CaptureType::Vulkan(uuids),
                Err(err) => {
                    warn!("{:?}", err);
                    CaptureType::ReadPixels
                }
            }
        } else {
            CaptureType::ReadPixels
        };

        let custom_ffmpeg_args = BXT_CAP_OVERRIDE_FFMPEG_ARGS.to_string(marker);
        let custom_ffmpeg_args: Option<Vec<&str>> = {
            let args = custom_ffmpeg_args.trim();
            if args.is_empty() {
                None
            } else {
                Some(args.split_ascii_whitespace().collect())
            }
        };
        let custom_ffmpeg_args = custom_ffmpeg_args.as_deref();

        let sampling_exposure = BXT_CAP_SAMPLING_EXPOSURE.as_f32(marker).into();
        let sampling_min_fps = BXT_CAP_SAMPLING_MIN_FPS
            .as_f32(marker)
            .max(fps as f32)
            .into();

        match Recorder::init(
            width,
            height,
            fps,
            slowdown,
            capture_type,
            filename,
            custom_ffmpeg_args,
            sampling_exposure,
            sampling_min_fps,
        ) {
            Ok(recorder) => {
                if matches!(recorder.capture_type(), CaptureType::ReadPixels) {
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
    }
}

pub unsafe fn skip_paint_channels(marker: MainThreadMarker) -> bool {
    // During recording we're capturing sound manually and don't want the game to mess with it.
    matches!(*STATE.borrow_mut(marker), State::Recording(_))
}

#[instrument(skip(marker))]
pub unsafe fn capture_sound(marker: MainThreadMarker, mode: SoundCaptureMode) {
    let end_time = {
        let mut state = STATE.borrow_mut(marker);
        let recorder = match *state {
            State::Recording(ref mut recorder) => recorder,
            _ => unreachable!(),
        };

        if (*engine::shm.get(marker)).is_null() {
            // If we're running with -nosound, write blank samples.
            let samples = recorder.samples_to_capture(22050, mode);
            recorder.write_audio_frame(vec![0; samples as usize * 4]);
            return;
        }

        let samples_per_second = (**engine::shm.get(marker)).speed;
        let samples = recorder.samples_to_capture(samples_per_second, mode);

        let painted_time = *engine::paintedtime.get(marker);
        painted_time + samples
    };

    engine::S_PaintChannels.get(marker)(end_time);
}

pub unsafe fn on_s_transfer_stereo_16(marker: MainThreadMarker, end: i32) {
    let mut state = STATE.borrow_mut(marker);
    let State::Recording(ref mut recorder) = *state else { return };

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
        let l16 = ((sample.left.wrapping_mul(volume)) >> 8).clamp(-32768, 32767) as i16;
        let r16 = ((sample.right.wrapping_mul(volume)) >> 8).clamp(-32768, 32767) as i16;

        buf[0..2].copy_from_slice(&l16.to_le_bytes());
        buf[2..4].copy_from_slice(&r16.to_le_bytes());
    }

    recorder.write_audio_frame((&buf[..sample_count * 4]).into());
}

pub unsafe fn on_host_filter_time(marker: MainThreadMarker) -> bool {
    let mut state = STATE.borrow_mut(marker);
    let State::Recording(ref mut recorder) = *state else { return false };

    if (*engine::cls_demos.get(marker)).demoplayback == 0 {
        return false;
    }

    let time = recorder.time_for_current_frame();
    *engine::host_frametime.get(marker) = time;
    *engine::realtime.get(marker) += time;

    true
}

pub unsafe fn on_cl_disconnect(marker: MainThreadMarker) {
    {
        // Safety: no engine functions are called while the reference is active.
        let cls_demos = &*engine::cls_demos.get(marker);

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
    let State::Recording(ref mut recorder) = *state else { return };

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
