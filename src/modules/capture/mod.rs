//! Video capture.

use std::{mem, os::raw::c_char, ptr::null_mut};

use color_eyre::eyre::{self, ensure, Context};
use rust_hawktracer::*;

use self::muxer::MuxerInitError;

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
        &COMMANDS
    }

    fn cvars(&self) -> &'static [&'static CVar] {
        static CVARS: &[&CVar] = &[&BXT_CAP_FPS, &BXT_CAP_VOLUME, &BXT_CAP_PLAYDEMOSTOP];
        &CVARS
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
            && HAVE_REQUIRED_GL_EXTENSIONS.get(marker)
            && crate::vulkan::VULKAN.get().is_some()
    }
}

mod muxer;
use muxer::Muxer;
mod opengl;
use opengl::OpenGL;
mod vulkan;
use vulkan::Vulkan;

#[cfg(unix)]
pub type ExternalObject = std::os::unix::io::RawFd;
#[cfg(windows)]
pub type ExternalObject = *mut std::os::raw::c_void;

static BXT_CAP_FPS: CVar = CVar::new(b"bxt_cap_fps\0", b"60\0");
static BXT_CAP_VOLUME: CVar = CVar::new(b"bxt_cap_volume\0", b"0.4\0");
static BXT_CAP_PLAYDEMOSTOP: CVar = CVar::new(b"bxt_cap_playdemostop\0", b"1\0");

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
        recorder.opengl = None;
    }
}

#[derive(Clone, Copy)]
pub enum SoundCaptureMode {
    /// Floor time to sample boundary.
    Normal,

    /// Ceil time to sample boundary.
    Remaining,
}

struct Recorder {
    /// Video width.
    width: i32,

    /// Video height.
    height: i32,

    /// The target time base.
    time_base: f64,

    /// Difference, in video frames, between how much time passed in-game and how much video we
    /// output.
    video_remainder: f64,

    /// Difference, in seconds, between how much time passed in-game and how much audio we output.
    sound_remainder: f64,

    /// Vulkan state.
    vulkan: Vulkan,

    /// Muxer and ffmpeg process.
    muxer: Muxer,

    /// OpenGL state; might be missing if the capturing just started or just after an engine
    /// restart.
    opengl: Option<OpenGL>,
}

impl Recorder {
    #[hawktracer(recorder_init)]
    unsafe fn init(width: i32, height: i32, fps: u64) -> eyre::Result<Recorder> {
        ensure!(
            width % 2 == 0 && height % 2 == 0,
            "can't handle odd game resolutions yet: {}×{}",
            width,
            height,
        );

        let vulkan =
            vulkan::init(width as u32, height as u32).wrap_err("error initalizing Vulkan")?;

        let time_base = 1. / fps as f64;

        let muxer = match Muxer::new(width as u64, height as u64, fps as u64) {
            Ok(muxer) => muxer,
            Err(err @ MuxerInitError::FfmpegSpawn(_)) => {
                return Err(err).wrap_err(
                    #[cfg(unix)]
                    "could not start ffmpeg. Make sure you have \
                    ffmpeg installed and present in PATH",
                    #[cfg(windows)]
                    "could not start ffmpeg. Make sure you have \
                    ffmpeg.exe in the Half-Life folder",
                );
            }
            Err(err) => {
                return Err(err).wrap_err("error initializing muxing");
            }
        };

        Ok(Recorder {
            width,
            height,
            time_base,
            video_remainder: 0.,
            sound_remainder: 0.,
            vulkan,
            muxer,
            opengl: None,
        })
    }

    #[hawktracer(initialize_opengl_capturing)]
    unsafe fn initialize_opengl_capturing(&mut self, marker: MainThreadMarker) -> eyre::Result<()> {
        let external_image_frame_memory = self.vulkan.external_image_frame_memory()?;
        let external_semaphore = self.vulkan.external_semaphore()?;
        let size = self.vulkan.image_frame_memory_size();

        self.opengl = Some(opengl::init(
            marker,
            self.width,
            self.height,
            size,
            external_image_frame_memory,
            external_semaphore,
        )?);

        Ok(())
    }

    unsafe fn ensure_opengl(&mut self, marker: MainThreadMarker) -> eyre::Result<()> {
        if self.opengl.is_some() {
            return Ok(());
        }

        self.initialize_opengl_capturing(marker)
    }

    unsafe fn capture_opengl(&self) -> eyre::Result<()> {
        self.opengl.as_ref().unwrap().capture()
    }

    #[hawktracer(acquire_and_capture)]
    unsafe fn acquire_and_capture(&mut self, frames: usize) -> eyre::Result<()> {
        self.vulkan.acquire_image()?;
        self.vulkan
            .convert_colors_and_mux(&mut self.muxer, frames)?;
        Ok(())
    }

    #[hawktracer(record_last_frame)]
    unsafe fn record_last_frame(&mut self) -> eyre::Result<()> {
        // Push this frame as long as it takes up the most of the video frame.
        // Remainder is > -0.5 at all times.
        let frames = (self.video_remainder + 0.5) as usize;
        self.video_remainder -= frames as f64;

        if frames > 0 {
            self.acquire_and_capture(frames)?;
        }

        Ok(())
    }

    fn time_passed(&mut self, time: f64) {
        self.video_remainder += time / self.time_base;
        self.sound_remainder += time;
    }

    fn samples_to_capture(&mut self, samples_per_second: i32, mode: SoundCaptureMode) -> i32 {
        let samples = self.sound_remainder * samples_per_second as f64;
        let samples_rounded = match mode {
            SoundCaptureMode::Normal => samples.floor(),
            SoundCaptureMode::Remaining => samples.ceil(),
        };

        self.sound_remainder = (samples - samples_rounded) / samples_per_second as f64;

        samples_rounded as i32
    }

    #[hawktracer(write_audio_frame)]
    fn write_audio_frame(&mut self, samples: &[u8]) -> eyre::Result<()> {
        self.muxer.write_audio_frame(samples)?;
        Ok(())
    }

    #[hawktracer(recorder_finish)]
    fn finish(self) {
        self.muxer.close();
    }
}

#[allow(clippy::large_enum_variant)]
enum State {
    Idle,
    Starting,
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
        "Usage: bxt_cap_start\n \
          Starts capturing video.\n",
        cap_start as fn(_)
    ),
);

fn cap_start(marker: MainThreadMarker) {
    if !Capture.is_enabled(marker) {
        return;
    }

    let mut state = STATE.borrow_mut(marker);
    if !matches!(*state, State::Idle) {
        // Already capturing.
        return;
    }

    *state = State::Starting;
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
            if let Err(err) = recorder.record_last_frame() {
                error!("error in Vulkan capturing: {:?}", err);
                con_print(marker, "Error in Vulkan capturing, stopping recording.\n");
                *state = State::Idle;
                return;
            }

            drop(state);
            capture_sound(marker, SoundCaptureMode::Remaining);
        }
    }

    STATE.borrow_mut(marker).set(State::Idle);
}

unsafe fn get_resolution(marker: MainThreadMarker) -> (i32, i32) {
    if engine::VideoMode_IsWindowed.get(marker)() != 0 {
        let rect = *engine::window_rect.get(marker);
        (rect.right - rect.left, rect.bottom - rect.top)
    } else {
        let mut width = 0;
        let mut height = 0;
        engine::VideoMode_GetCurrentVideoMode.get(marker)(&mut width, &mut height, null_mut());
        (width, height)
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

    scoped_tracepoint!(capture_frame_);

    let (width, height) = get_resolution(marker);

    // Initialize the recording if needed.
    if matches!(*state, State::Starting) {
        let fps = BXT_CAP_FPS.as_u64(marker).max(1);

        match Recorder::init(width, height, fps) {
            Ok(recorder) => *state = State::Recording(recorder),
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
        error!("error in Vulkan capturing: {:?}", err);
        con_print(marker, "Error in Vulkan capturing, stopping recording.\n");
        *state = State::Idle;
        return;
    }

    // Check for resolution changes.
    if recorder.width != width || recorder.height != height {
        con_print(
            marker,
            &format!(
                "Resolution has changed: {}×{} => {}×{}, stopping recording.\n",
                recorder.width, recorder.height, width, height
            ),
        );
        cap_stop(marker);
        return;
    }

    // We'll need OpenGL. Initialize it if it isn't.
    if let Err(err) = recorder.ensure_opengl(marker) {
        error!("error initializing OpenGL capturing: {:?}", err);
        con_print(marker, "Error initializing OpenGL, stopping recording.\n");

        drop(state);
        cap_stop(marker);
        return;
    }

    // Capture this frame for recording later.
    if let Err(err) = recorder.capture_opengl() {
        error!("error capturing frame: {:?}", err);
        con_print(
            marker,
            "Error capturing frame with OpenGL, stopping recording.\n",
        );

        // Make sure we don't call Vulkan as OpenGL could've failed in the middle leaving semaphore
        // in a bad state.
        recorder.video_remainder = 0.;

        drop(state);
        cap_stop(marker);
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

    recorder
        .write_audio_frame(&buf[..sample_count * 4])
        .unwrap();
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

    *engine::host_frametime.get(marker) = recorder.time_base;
    let realtime = engine::realtime.get(marker);
    *realtime += recorder.time_base;

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
        if cls_demos.demonum != -1 {
            return;
        }
    }

    if !BXT_CAP_PLAYDEMOSTOP.as_bool(marker) {
        return;
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
