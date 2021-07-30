use std::thread::{self, JoinHandle};

use color_eyre::eyre::{self, ensure, eyre, Context};
use crossbeam_channel::{bounded, Receiver, Sender};
use rust_hawktracer::*;

use super::muxer::{Muxer, MuxerInitError, PixelFormat};
use super::opengl::{self, OpenGl};
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

    /// Difference, in video frames, between how much time passed in-game and how much video we
    /// output.
    video_remainder: f64,

    /// Difference, in seconds, between how much time passed in-game and how much audio we output.
    sound_remainder: f64,

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

    /// How we're capturing the frames.
    capture_type: CaptureType,

    /// Buffer for capturing with ReadPixels.
    buffer: Option<Box<[u8]>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureType {
    Vulkan,
    ReadPixels,
}

#[derive(Debug)]
enum MainToThread {
    Finish,
    GiveExternalHandles,
    AcquireImage,
    Record { frames: usize },
    Mux { pixels: Box<[u8]>, frames: usize },
    Audio(Vec<u8>),
}

#[derive(Debug)]
enum ThreadToMain {
    Error(eyre::Report),
    ExternalHandles(ExternalHandles),
    AcquiredImage,
    Muxed(Box<[u8]>),
}

impl Recorder {
    #[hawktracer(recorder_init)]
    pub unsafe fn init(
        width: i32,
        height: i32,
        fps: u64,
        mut capture_type: CaptureType,
        filename: &str,
    ) -> eyre::Result<Recorder> {
        ensure!(
            width % 2 == 0 && height % 2 == 0,
            "can't handle odd game resolutions yet: {}×{}",
            width,
            height,
        );

        let vulkan = if capture_type == CaptureType::Vulkan {
            match vulkan::init(width as u32, height as u32).wrap_err("error initalizing Vulkan") {
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

        let time_base = 1. / fps as f64;
        let pixel_format = if vulkan.is_some() {
            PixelFormat::I420
        } else {
            PixelFormat::Rgb24Flipped
        };

        let muxer = match Muxer::new(width as u64, height as u64, fps, pixel_format, filename) {
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

        let (to_thread_sender, from_main_receiver) = bounded(2);
        let (to_main_sender, from_thread_receiver) = bounded(1);
        let thread =
            thread::spawn(move || thread(vulkan, muxer, to_main_sender, from_main_receiver));

        Ok(Recorder {
            width,
            height,
            time_base,
            video_remainder: 0.,
            sound_remainder: 0.,
            opengl: None,
            acquired_image: false,
            thread,
            sender: to_thread_sender,
            receiver: from_thread_receiver,
            thread_error: None,
            capture_type,
            buffer: Some(vec![0u8; width as usize * height as usize * 3].into()),
        })
    }

    fn send_to_thread(&mut self, message: MainToThread) {
        if self.sender.send(message).is_ok() {
            // The happy path.
            return;
        }

        // The channel was closed. Try to get the error.
        while let Ok(message) = self.receiver.try_recv() {
            if let ThreadToMain::Error(err) = message {
                self.thread_error = Some(err);
            }
        }
    }

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

    #[hawktracer(initialize_opengl_capturing)]
    unsafe fn initialize_opengl_capturing(&mut self, marker: MainThreadMarker) -> eyre::Result<()> {
        assert_eq!(self.capture_type, CaptureType::Vulkan);

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

    pub unsafe fn capture_opengl(&mut self, marker: MainThreadMarker) -> eyre::Result<()> {
        match self.capture_type {
            CaptureType::Vulkan => {
                if self.opengl.is_none() {
                    self.initialize_opengl_capturing(marker)?;
                }

                self.opengl.as_ref().unwrap().capture()
            }
            CaptureType::ReadPixels => {
                if self.buffer.is_none() {
                    match self.recv_from_thread()? {
                        ThreadToMain::Muxed(buffer) => {
                            self.buffer = Some(buffer);
                        }
                        _ => unreachable!(),
                    }
                }

                opengl::capture_with_read_pixels(
                    marker,
                    self.width,
                    self.height,
                    self.buffer.as_mut().unwrap(),
                )
                .wrap_err("error capturing with glReadPixels")
            }
        }
    }

    #[hawktracer(acquire_image_if_needed)]
    unsafe fn acquire_image_if_needed(&mut self) {
        assert_eq!(self.capture_type, CaptureType::Vulkan);

        if self.acquired_image {
            return;
        }

        let frames = (self.video_remainder + 0.5) as usize;
        if frames == 0 {
            return;
        }

        self.acquired_image = true;

        self.send_to_thread(MainToThread::AcquireImage);
    }

    #[hawktracer(record)]
    unsafe fn record(&mut self, frames: usize) -> eyre::Result<()> {
        match self.capture_type {
            CaptureType::Vulkan => {
                assert!(self.acquired_image);

                // Must wait for this before OpenGL capture can run.
                assert!(matches!(
                    self.recv_from_thread()?,
                    ThreadToMain::AcquiredImage
                ));

                self.acquired_image = false;

                self.send_to_thread(MainToThread::Record { frames });
            }
            CaptureType::ReadPixels => {
                let pixels = self.buffer.take().unwrap();
                self.send_to_thread(MainToThread::Mux { pixels, frames });
            }
        }

        Ok(())
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

    pub fn time_passed(&mut self, time: f64) {
        self.video_remainder += time / self.time_base;
        self.sound_remainder += time;

        if self.capture_type == CaptureType::Vulkan {
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

    #[hawktracer(write_audio_frame)]
    pub fn write_audio_frame(&mut self, samples: Vec<u8>) {
        self.send_to_thread(MainToThread::Audio(samples));
    }

    #[hawktracer(recorder_finish)]
    pub fn finish(mut self) {
        self.send_to_thread(MainToThread::Finish);

        while let Ok(message) = self.receiver.recv() {
            if let ThreadToMain::Error(err) = message {
                self.thread_error = Some(err);
            }
        }

        self.thread.join().unwrap();

        if let Some(err) = self.thread_error {
            error!("recording thread error: {:?}", err);
        }
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

    pub fn time_base(&self) -> f64 {
        self.time_base
    }

    pub fn capture_type(&self) -> CaptureType {
        self.capture_type
    }
}

fn thread(
    vulkan: Option<Vulkan>,
    mut muxer: Muxer,
    s: Sender<ThreadToMain>,
    r: Receiver<MainToThread>,
) {
    while let Ok(message) = r.recv() {
        match process_message(vulkan.as_ref(), &mut muxer, &s, message) {
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

    muxer.close();
}

fn process_message(
    vulkan: Option<&Vulkan>,
    muxer: &mut Muxer,
    s: &Sender<ThreadToMain>,
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
            scoped_tracepoint!(_acquire);

            unsafe { vulkan.unwrap().acquire_image() }?;

            s.send(ThreadToMain::AcquiredImage).unwrap();
        }
        MainToThread::Record { frames } => {
            scoped_tracepoint!(_record);

            unsafe { vulkan.unwrap().convert_colors_and_mux(muxer, frames) }?;
        }
        MainToThread::Mux { pixels, frames } => {
            scoped_tracepoint!(_mux);

            for _ in 0..frames {
                muxer.write_video_frame(&pixels)?;
            }

            s.send(ThreadToMain::Muxed(pixels)).unwrap();
        }
        MainToThread::Audio(samples) => {
            scoped_tracepoint!(_audio);

            muxer.write_audio_frame(&samples)?;
        }
    }

    Ok(false)
}
