use color_eyre::eyre::{self, ensure, Context};
use rust_hawktracer::*;

use super::{
    muxer::{Muxer, MuxerInitError},
    opengl::{self, OpenGL},
    vulkan::{self, Vulkan},
    SoundCaptureMode,
};
use crate::utils::*;

pub struct Recorder {
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

    /// Whether Vulkan has already acquired the last frame.
    acquired_image: bool,
}

impl Recorder {
    #[hawktracer(recorder_init)]
    pub unsafe fn init(width: i32, height: i32, fps: u64) -> eyre::Result<Recorder> {
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
            acquired_image: false,
        })
    }

    #[hawktracer(initialize_opengl_capturing)]
    unsafe fn initialize_opengl_capturing(&mut self, marker: MainThreadMarker) -> eyre::Result<()> {
        let external_handles = self.vulkan.external_handles()?;

        self.opengl = Some(opengl::init(
            marker,
            self.width,
            self.height,
            external_handles.size,
            external_handles.external_image_frame_memory,
            external_handles.external_semaphore,
        )?);

        Ok(())
    }

    pub unsafe fn ensure_opengl(&mut self, marker: MainThreadMarker) -> eyre::Result<()> {
        if self.opengl.is_some() {
            return Ok(());
        }

        self.initialize_opengl_capturing(marker)
    }

    pub unsafe fn capture_opengl(&self) -> eyre::Result<()> {
        self.opengl.as_ref().unwrap().capture()
    }

    #[hawktracer(acquire_image_if_needed)]
    pub unsafe fn acquire_image_if_needed(&mut self) -> eyre::Result<()> {
        if self.acquired_image {
            return Ok(());
        }

        let frames = (self.video_remainder + 0.5) as usize;
        if frames == 0 {
            return Ok(());
        }

        self.acquired_image = true;
        self.vulkan.acquire_image()
    }

    #[hawktracer(record)]
    pub unsafe fn record(&mut self, frames: usize) -> eyre::Result<()> {
        assert!(self.acquired_image);
        self.acquired_image = false;

        self.vulkan.convert_colors_and_mux(&mut self.muxer, frames)
    }

    #[hawktracer(record_last_frame)]
    pub unsafe fn record_last_frame(&mut self) -> eyre::Result<()> {
        // Push this frame as long as it takes up the most of the video frame.
        // Remainder is > -0.5 at all times.
        let frames = (self.video_remainder + 0.5) as usize;
        self.video_remainder -= frames as f64;

        if frames > 0 {
            self.record(frames)?;
        }

        Ok(())
    }

    pub fn time_passed(&mut self, time: f64) -> eyre::Result<()> {
        self.video_remainder += time / self.time_base;
        self.sound_remainder += time;
        unsafe { self.acquire_image_if_needed() }
    }

    pub fn samples_to_capture(&mut self, samples_per_second: i32, mode: SoundCaptureMode) -> i32 {
        let samples = self.sound_remainder * samples_per_second as f64;
        let samples_rounded = match mode {
            SoundCaptureMode::Normal => samples.floor(),
            SoundCaptureMode::Remaining => samples.ceil(),
        };

        self.sound_remainder = (samples - samples_rounded) / samples_per_second as f64;

        samples_rounded as i32
    }

    #[hawktracer(write_audio_frame)]
    pub fn write_audio_frame(&mut self, samples: &[u8]) -> eyre::Result<()> {
        self.muxer.write_audio_frame(samples)?;
        Ok(())
    }

    #[hawktracer(recorder_finish)]
    pub fn finish(self) {
        self.muxer.close();
    }

    pub fn reset_opengl(&mut self) {
        self.opengl = None;
    }

    pub fn reset_video_remainder(&mut self) {
        self.video_remainder = 0.;
    }

    pub fn width(&self) -> i32 {
        self.width
    }

    pub fn height(&self) -> i32 {
        self.height
    }

    pub fn time_base(&self) -> f64 {
        self.time_base
    }
}
