use std::thread::{self, JoinHandle};

use color_eyre::eyre::{self, ensure, eyre, Context};
use crossbeam_channel::{bounded, Receiver, Sender};
use rayon::prelude::*;

use super::muxer::{Muxer, MuxerInitError, PixelFormat};
use super::opengl::{self, OpenGl, Uuids};
use super::vulkan::{self, ExternalHandles, Vulkan};
use super::SoundCaptureMode;
use crate::utils::*;

pub struct Recorder {
    /// Video width.
    width: i32,

    /// Video height.
    height: i32,

    /// The target time base.
    time_base: f64,

    /// The slowdown factor. For example, `2` means two times slower.
    slowdown: f64,

    /// Difference, in video frames, between how much time passed in-game and how much video we
    /// output.
    ///
    /// When sampling it is `>= 0`, otherwise it is `> -0.5`.
    video_remainder: f64,

    /// Difference, in seconds, between how much time passed in-game and how much audio we output.
    sound_remainder: f64,

    /// How much time contributes to each frame's average when sampling. `0` means no sampling.
    sampling_exposure: f64,

    /// Time step to use when recording with sampling.
    sampling_time_step: f64,

    /// Last frame's start time for computing its weight for sampling.
    sampling_last_frame_start: f64,

    /// OpenGL state; might be missing if the capturing just started or just after an engine
    /// restart.
    opengl: Option<OpenGl>,

    /// Whether Vulkan has already acquired the last frame.
    acquired_image: bool,

    /// Vulkan recording and muxing thread.
    thread: JoinHandle<()>,

    /// Sender for messages to the thread.
    sender: Sender<MainToThread>,

    /// Receiver for messages from the thread.
    receiver: Receiver<ThreadToMain>,

    /// Error from the thread if it sent one.
    thread_error: Option<eyre::Report>,

    /// FFmpeg output from the thread if it sent one.
    ffmpeg_output: Option<String>,

    /// How we're capturing the frames.
    capture_type: CaptureType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureType {
    Vulkan(Uuids),
    ReadPixels,
}

#[derive(Debug)]
enum MainToThread {
    Finish,
    GiveExternalHandles,
    AcquireImage,
    Captured { buffer: Box<[u8]> },
    Record { frames: usize },
    Accumulate { weight: f32 },
    Audio(Vec<u8>),
}

#[derive(Debug)]
enum ThreadToMain {
    Error(eyre::Report),
    ExternalHandles(ExternalHandles),
    AcquiredImage,
    PixelBuffer(Box<[u8]>),
    FfmpegOutput(String),
}

impl Recorder {
    #[allow(clippy::too_many_arguments)]
    #[instrument(name = "Recorder::init")]
    pub unsafe fn init(
        width: i32,
        height: i32,
        fps: u64,
        slowdown: f64,
        mut capture_type: CaptureType,
        filename: &str,
        custom_ffmpeg_args: Option<&[&str]>,
        sampling_exposure: f64,
        sampling_min_fps: f64,
    ) -> eyre::Result<Recorder> {
        ensure!(
            width % 2 == 0 && height % 2 == 0,
            "can't handle odd game resolutions yet: {}×{}",
            width,
            height,
        );

        ensure!(
            sampling_exposure >= 0.,
            "sampling exposure must be >= 0, but it is {}",
            sampling_exposure,
        );

        ensure!(
            sampling_exposure <= 1.,
            "sampling exposure must be <= 1, but it is {} (can't handle exposure longer \
            than one frame yet)",
            sampling_exposure,
        );

        let is_sampling = sampling_exposure != 0.;

        let vulkan = if let CaptureType::Vulkan(ref uuids) = capture_type {
            match vulkan::init(width as u32, height as u32, uuids, is_sampling)
                .wrap_err("error initalizing Vulkan")
            {
                Ok(vulkan) => Some(vulkan),
                Err(err) => {
                    warn!("{:?}", err);
                    capture_type = CaptureType::ReadPixels;
                    None
                }
            }
        } else {
            None
        };

        let recording_fps = fps as f64 * slowdown;
        let time_base = 1. / recording_fps;

        let sampling_exposure = sampling_exposure * time_base;

        // Pick a sampling FPS >= the min FPS that divides the recording FPS evenly.
        let sampling_fps = (sampling_min_fps / recording_fps).ceil() * recording_fps;
        let sampling_time_step = 1. / sampling_fps;

        let sampling_buffers = if is_sampling && vulkan.is_none() {
            let count = width as usize * height as usize * 3;
            Some((vec![0u16; count].into(), vec![0u8; count].into()))
        } else {
            None
        };

        let pixel_format = if vulkan.is_some() {
            PixelFormat::I420
        } else {
            PixelFormat::Rgb24Flipped
        };

        let muxer = match Muxer::new(
            width as u64,
            height as u64,
            fps,
            pixel_format,
            filename,
            custom_ffmpeg_args,
        ) {
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

        // When recording with sampling and exposure < 1, muxing the final frame can span many
        // in-game frames that send audio samples, but are ignored for the purposes of video
        // capture. We make the main-to-thread channel size big so that sending those audio samples
        // doesn't block on waiting for the frame to be muxed.
        let (to_thread_sender, from_main_receiver) = bounded(64);
        let (to_main_sender, from_thread_receiver) = bounded(2);

        let pixels = if vulkan.is_none() {
            let buffer: Box<[u8]> = vec![0u8; width as usize * height as usize * 3].into();
            let pixels = buffer.clone();
            to_main_sender
                .send(ThreadToMain::PixelBuffer(buffer))
                .unwrap();
            Some(pixels)
        } else {
            None
        };

        let thread = thread::Builder::new()
            .name("Recording Thread".to_string())
            .spawn(move || {
                thread(
                    vulkan,
                    muxer,
                    pixels,
                    sampling_buffers,
                    to_main_sender,
                    from_main_receiver,
                )
            })
            .unwrap();

        Ok(Recorder {
            width,
            height,
            time_base,
            slowdown,
            video_remainder: 0.,
            sound_remainder: 0.,
            sampling_exposure,
            sampling_time_step,
            sampling_last_frame_start: 0.,
            opengl: None,
            acquired_image: false,
            thread,
            sender: to_thread_sender,
            receiver: from_thread_receiver,
            thread_error: None,
            ffmpeg_output: None,
            capture_type,
        })
    }

    #[instrument(skip_all)]
    fn send_to_thread(&mut self, message: MainToThread) {
        if self.sender.send(message).is_ok() {
            // The happy path.
            return;
        }

        // The channel was closed. Try to get the error.
        while let Ok(message) = self.receiver.try_recv() {
            match message {
                ThreadToMain::Error(err) => {
                    self.thread_error = Some(err);
                }
                ThreadToMain::FfmpegOutput(output) => self.ffmpeg_output = Some(output),
                _ => (),
            }
        }
    }

    #[instrument(skip_all)]
    fn recv_from_thread(&mut self) -> eyre::Result<ThreadToMain> {
        match self.receiver.recv() {
            Err(_) => Err(self
                .thread_error
                .take()
                .unwrap_or_else(|| eyre!("recording thread error"))),
            Ok(ThreadToMain::Error(err)) => Err(err),
            Ok(message) => Ok(message),
        }
    }

    #[instrument(skip_all)]
    unsafe fn initialize_opengl_capturing(&mut self, marker: MainThreadMarker) -> eyre::Result<()> {
        assert!(matches!(self.capture_type, CaptureType::Vulkan(_)));

        self.send_to_thread(MainToThread::GiveExternalHandles);
        let external_handles = match self.recv_from_thread()? {
            ThreadToMain::ExternalHandles(handles) => handles,
            _ => unreachable!(),
        };

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

    #[instrument(skip_all)]
    pub unsafe fn capture_opengl(&mut self, marker: MainThreadMarker) -> eyre::Result<()> {
        match self.capture_type {
            CaptureType::Vulkan(_) => {
                if self.opengl.is_none() {
                    self.initialize_opengl_capturing(marker)?;
                }

                if self.acquired_image {
                    // Must wait for this before OpenGL capture can run.
                    assert!(matches!(
                        self.recv_from_thread()?,
                        ThreadToMain::AcquiredImage
                    ));

                    self.acquired_image = false;
                }

                self.opengl.as_ref().unwrap().capture()
            }
            CaptureType::ReadPixels => {
                let mut buffer = match self.recv_from_thread()? {
                    ThreadToMain::PixelBuffer(buffer) => buffer,
                    _ => unreachable!(),
                };

                opengl::capture_with_read_pixels(marker, self.width, self.height, &mut buffer)
                    .wrap_err("error capturing with glReadPixels")?;

                self.send_to_thread(MainToThread::Captured { buffer });

                Ok(())
            }
        }
    }

    fn is_sampling(&self) -> bool {
        self.sampling_exposure != 0.
    }

    fn current_frame_length(&self) -> usize {
        assert!(!self.is_sampling());

        // Push this frame as long as it takes up the most of the video frame.
        // Remainder is > -0.5 at all times.
        (self.video_remainder + 0.5) as usize
    }

    fn current_sampling_weight(&self) -> f64 {
        assert!(self.is_sampling());

        let sampling_start = self.frame_time() - self.sampling_exposure;
        let frame_end = self.video_remainder * self.frame_time();
        let frame_start = self.sampling_last_frame_start.max(sampling_start);
        (frame_end.min(self.frame_time()) - frame_start) / self.sampling_exposure
    }

    #[instrument(skip_all)]
    unsafe fn acquire_image_if_needed(&mut self) {
        assert!(matches!(self.capture_type, CaptureType::Vulkan(_)));

        if self.acquired_image {
            return;
        }

        if self.is_sampling() {
            let weight = self.current_sampling_weight();
            if weight <= 0. {
                return;
            }
        } else {
            let frames = self.current_frame_length();
            if frames == 0 {
                return;
            }
        }

        self.acquired_image = true;
        self.send_to_thread(MainToThread::AcquireImage);
    }

    #[instrument("Recorder::accumulate", skip(self))]
    fn accumulate(&mut self, weight: f32) {
        assert!(self.is_sampling());

        if matches!(self.capture_type, CaptureType::Vulkan(_)) {
            assert!(self.acquired_image);
        }

        if weight < 255. / (254. * 65535. * 2.) {
            // If the weight is so small that any pixel is guaranteed to round down to zero, skip
            // the whole step. This is needed because after completing a frame we frequently end up
            // with a miniscule amount of weight left due to imprecision.
            return;
        }

        self.send_to_thread(MainToThread::Accumulate { weight });
    }

    #[instrument("Recorder::record", skip(self))]
    unsafe fn record(&mut self, frames: usize) {
        self.send_to_thread(MainToThread::Record { frames });
    }

    #[instrument(skip_all)]
    pub unsafe fn record_last_frame(&mut self) -> eyre::Result<()> {
        if self.is_sampling() {
            loop {
                let weight = self.current_sampling_weight();

                // This update must happen after computing the current weight.
                self.sampling_last_frame_start = self.video_remainder * self.frame_time();

                // Sanity check with some allowance for floating-point imprecision.
                assert!(weight < 1.00001);
                let weight = weight.min(1.);

                if weight <= 0. {
                    break;
                }
                self.accumulate(weight as f32);

                // If we crossed a frame boundary, record the frame.
                if self.video_remainder >= 1. {
                    self.record(1);
                    self.video_remainder -= 1.;
                    self.sampling_last_frame_start = 0.;
                }

                // Optimization for long frames: if we crossed more frame boundaries, record this
                // frame enough times right away.
                let full_frames = self.video_remainder as usize;
                if full_frames > 0 {
                    self.accumulate(1.);
                    self.record(full_frames);
                    self.video_remainder -= full_frames as f64;
                }
            }

            assert!(
                (self.sampling_last_frame_start - self.video_remainder * self.frame_time()).abs()
                    < 0.00001
            );
        } else {
            let frames = self.current_frame_length();
            self.video_remainder -= frames as f64;

            if frames > 0 {
                self.record(frames);
            }
        }

        Ok(())
    }

    pub fn time_passed(&mut self, time: f64) {
        self.video_remainder += time / self.frame_time();
        self.sound_remainder += time * self.slowdown;

        if let CaptureType::Vulkan(_) = self.capture_type {
            unsafe {
                self.acquire_image_if_needed();
            }
        }
    }

    pub fn samples_to_capture(&mut self, samples_per_second: i32, mode: SoundCaptureMode) -> i32 {
        let samples = self.sound_remainder * samples_per_second as f64;
        let samples_rounded = match mode {
            SoundCaptureMode::Normal => samples.floor(),
            SoundCaptureMode::Remaining { extra } => {
                (samples + extra as f64 * samples_per_second as f64).ceil()
            }
        };

        self.sound_remainder = (samples - samples_rounded) / samples_per_second as f64;

        samples_rounded as i32
    }

    #[instrument(name = "Recorder::write_audio_frame", skip_all)]
    pub fn write_audio_frame(&mut self, samples: Vec<u8>) {
        self.send_to_thread(MainToThread::Audio(samples));
    }

    #[instrument(name = "Recorder::finish", skip_all)]
    pub fn finish(mut self) -> Option<String> {
        self.send_to_thread(MainToThread::Finish);

        while let Ok(message) = self.receiver.recv() {
            match message {
                ThreadToMain::Error(err) => {
                    self.thread_error = Some(err);
                }
                ThreadToMain::FfmpegOutput(output) => self.ffmpeg_output = Some(output),
                _ => (),
            }
        }

        self.thread.join().unwrap();

        if let Some(err) = self.thread_error {
            error!("recording thread error: {:?}", err);
        }

        self.ffmpeg_output.take()
    }

    pub fn reset_opengl(&mut self) {
        self.opengl = None;
    }

    pub fn width(&self) -> i32 {
        self.width
    }

    pub fn height(&self) -> i32 {
        self.height
    }

    fn frame_time(&self) -> f64 {
        self.time_base
    }

    pub fn time_for_current_frame(&self) -> f64 {
        if self.is_sampling() {
            self.sampling_time_step
        } else {
            self.frame_time()
        }
    }

    pub fn capture_type(&self) -> &CaptureType {
        &self.capture_type
    }
}

fn thread(
    vulkan: Option<Vulkan>,
    mut muxer: Muxer,
    mut pixels: Option<Box<[u8]>>,
    mut sampling_buffers: Option<(Box<[u16]>, Box<[u8]>)>,
    s: Sender<ThreadToMain>,
    r: Receiver<MainToThread>,
) {
    while let Ok(message) = r.recv() {
        match process_message(
            vulkan.as_ref(),
            &mut muxer,
            &s,
            &mut pixels,
            &mut sampling_buffers,
            message,
        ) {
            Ok(done) => {
                if done {
                    break;
                }
            }
            Err(err) => {
                s.send(ThreadToMain::Error(err)).unwrap();
                break;
            }
        }
    }

    let output = muxer.close();
    s.send(ThreadToMain::FfmpegOutput(output)).unwrap();
}

fn process_message(
    vulkan: Option<&Vulkan>,
    muxer: &mut Muxer,
    s: &Sender<ThreadToMain>,
    pixels: &mut Option<Box<[u8]>>,
    sampling_buffers: &mut Option<(Box<[u16]>, Box<[u8]>)>,
    message: MainToThread,
) -> eyre::Result<bool> {
    match message {
        MainToThread::Finish => {
            return Ok(true);
        }
        MainToThread::GiveExternalHandles => {
            let handles = vulkan.unwrap().external_handles()?;
            s.send(ThreadToMain::ExternalHandles(handles)).unwrap();
        }
        MainToThread::AcquireImage => {
            let _span = info_span!("acquire").entered();

            unsafe { vulkan.unwrap().acquire_image() }?;

            s.send(ThreadToMain::AcquiredImage).unwrap();
        }
        MainToThread::Captured { buffer } => {
            let old_pixels = pixels.replace(buffer);

            // Send the second buffer back to the main thread so it can use it for the next frame.
            s.send(ThreadToMain::PixelBuffer(old_pixels.unwrap()))
                .unwrap();
        }
        MainToThread::Accumulate { weight } => {
            let _span = info_span!("accumulate").entered();

            assert!(pixels.is_some() || vulkan.is_some());

            if let Some(pixels) = pixels.as_ref() {
                let (sampling_buffer, _output_buffer) = sampling_buffers.as_mut().unwrap();

                accumulate(sampling_buffer, pixels, weight);
            } else {
                unsafe { vulkan.unwrap().accumulate(weight) }?;
            }
        }
        MainToThread::Record { frames } => {
            let _span = info_span!("record").entered();

            assert!(sampling_buffers.is_some() || pixels.is_some() || vulkan.is_some());

            if let Some((sampling_buffer, output_buffer)) = sampling_buffers.as_mut() {
                convert_and_zero(output_buffer, sampling_buffer);

                for _ in 0..frames {
                    muxer.write_video_frame(output_buffer)?;
                }
            } else if let Some(pixels) = pixels {
                for _ in 0..frames {
                    muxer.write_video_frame(pixels)?;
                }
            } else {
                unsafe { vulkan.unwrap().convert_colors_and_mux(muxer, frames) }?;
            }
        }
        MainToThread::Audio(samples) => {
            let _span = info_span!("audio").entered();

            muxer.write_audio_frame(&samples)?;
        }
    }

    Ok(false)
}

#[instrument(skip_all)]
fn accumulate(sampling_buffer: &mut [u16], pixels: &[u8], weight: f32) {
    assert!((0. ..=1.).contains(&weight));

    // Expand range to u32 to operate in integers in the inner loop.
    const ONE_IN_U32: u32 = 256 * 256 * 256;
    let weight = (weight * ONE_IN_U32 as f32) as u32;
    assert!(weight <= ONE_IN_U32);

    // HACK: writing out this expression inline for some reason prevents the bounds checks from
    // getting optimized out.
    // https://github.com/rust-lang/rust/issues/103132
    #[inline(always)]
    fn f(x: u8, weight: u32) -> u32 {
        assert!(weight <= ONE_IN_U32);
        (x as u32 * weight + 256 * 256 / 2) / (256 * 256)
    }

    sampling_buffer
        .par_iter_mut()
        .zip(pixels)
        .for_each(|(sample, buf)| {
            *sample = sample.saturating_add(f(*buf, weight) as u16);
        });
}

#[instrument(skip_all)]
fn convert_and_zero(output_buffer: &mut [u8], sampling_buffer: &mut [u16]) {
    for (out, sample) in output_buffer.iter_mut().zip(&*sampling_buffer) {
        // Using saturating_add is 80% faster according to benchmarks, likely because it removes the
        // bounds check, which allows the loop to be vectorized.
        *out = (sample.saturating_add(128) / 256) as u8;
    }

    // Zeroing the buffer separately is 50% faster according to benchmarks.
    sampling_buffer.fill(0);
}
