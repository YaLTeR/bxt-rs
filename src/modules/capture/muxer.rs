use std::{
    io::{self, Write},
    process::{Child, Command, Stdio},
};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use rust_hawktracer::*;
use thiserror::Error;

pub struct Muxer {
    child: Child,
    video_pts: u64,
    audio_pts: u64,
}

#[derive(Error, Debug)]
pub enum MuxerInitError {
    #[error("could not spawn ffmpeg")]
    FfmpegSpawn(io::Error),
    #[error(transparent)]
    Other(#[from] io::Error),
}

fn v<W: Write>(mut writer: W, mut value: u64) -> Result<(), io::Error> {
    let mut elements = [0; 10];
    let mut i = 10;

    loop {
        i -= 1;

        elements[i] = value as u8 & 127;
        if i < 9 {
            elements[i] |= 128;
        }

        value /= 128;
        if value == 0 {
            break;
        }
    }

    writer.write_all(&elements[i..])?;

    Ok(())
}

fn vb<W: Write>(mut writer: W, data: &[u8]) -> Result<(), io::Error> {
    v(&mut writer, data.len() as u64)?;
    writer.write_all(data)?;

    Ok(())
}

fn crc32(data: &[u8]) -> u32 {
    #[rustfmt::skip]
    const TABLE: [u32; 16] = [
        0x00000000, 0x04C11DB7, 0x09823B6E, 0x0D4326D9,
        0x130476DC, 0x17C56B6B, 0x1A864DB2, 0x1E475005,
        0x2608EDB8, 0x22C9F00F, 0x2F8AD6D6, 0x2B4BCB61,
        0x350C9B64, 0x31CD86D3, 0x3C8EA00A, 0x384FBDBD,
    ];

    let mut crc = 0;
    for &byte in data {
        crc ^= (byte as u32) << 24;
        crc = (crc << 4) ^ TABLE[(crc >> 28) as usize];
        crc = (crc << 4) ^ TABLE[(crc >> 28) as usize];
    }
    crc
}

fn packet<W: Write>(mut writer: W, startcode: u64, data: &[u8]) -> Result<(), io::Error> {
    let packet_size = data.len() as u64 + 4; // 4 for the checksum in packet_footer
    assert!(packet_size < 4096);

    writer.write_all(&startcode.to_be_bytes()[..])?;
    v(&mut writer, packet_size)?; // forward-ptr
    writer.write_all(data)?;
    writer.write_all(&crc32(data).to_be_bytes()[..])?; // checksum

    Ok(())
}

impl Muxer {
    #[hawktracer(muxer_new)]
    pub fn new(width: u64, height: u64, fps: u64) -> Result<Self, MuxerInitError> {
        #[rustfmt::skip]
        let args = [
            "-f", "nut",
            "-i", "pipe:",
            "-c:v", "libx264",
            "-crf", "15",
            "-preset", "ultrafast",
            "-color_primaries", "bt709",
            "-color_trc", "bt709",
            "-colorspace", "bt709",
            "-color_range", "tv",
            "-chroma_sample_location", "center",
            "-movflags", "+faststart",
            "-y",
            "output.mp4",
        ];

        let mut command = Command::new("ffmpeg");
        command
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        #[cfg(windows)]
        command.creation_flags(winapi::um::winbase::CREATE_NO_WINDOW);

        let mut child = command.spawn().map_err(MuxerInitError::FfmpegSpawn)?;
        let mut writer = child.stdin.as_mut().unwrap();

        const MAIN_STARTCODE: u64 = 0x4e4d7a561f5f04ad;
        const STREAM_STARTCODE: u64 = 0x4e5311405bf2f9db;

        writer.write_all(b"nut/multimedia container\0")?;

        // Main header.
        let mut buf = Vec::new();
        v(&mut buf, 3)?; // version
        v(&mut buf, 2)?; // stream_count
        v(&mut buf, 65536)?; // max_distance
        v(&mut buf, 2)?; // time_base_count
        v(&mut buf, 1)?; // time_base_num
        v(&mut buf, fps)?; // time_base_denom
        v(&mut buf, 1)?; // time_base_num
        v(&mut buf, 22050)?; // time_base_denom

        for _ in 0..255 {
            // Not 256 because 'N' is skipped.
            v(&mut buf, 1 << 12)?; // tmp_flag = FLAG_CODED
            v(&mut buf, 0)?; // tmp_fields
        }

        v(&mut buf, 0)?; // header_count_minus1
        v(&mut buf, 0)?; // main_flags

        packet(&mut writer, MAIN_STARTCODE, &buf)?;

        // Stream header (video).
        buf.clear();
        v(&mut buf, 0)?; // stream_id
        v(&mut buf, 0)?; // stream_class = video
        vb(&mut buf, b"I420")?; // fourcc
        v(&mut buf, 0)?; // time_base_id
        v(&mut buf, 0)?; // msb_pts_shift
        v(&mut buf, 1)?; // max_pts_distance
        v(&mut buf, 0)?; // decode_delay
        v(&mut buf, 1)?; // stream_flags = FLAG_FIXED_FPS
        vb(&mut buf, &[])?; // codec_specific_data

        v(&mut buf, width)?; // width
        v(&mut buf, height)?; // height
        v(&mut buf, 1)?; // sample_width
        v(&mut buf, 1)?; // sample_height
        v(&mut buf, 2)?; // colorspace_type = narrow-range 709

        packet(&mut writer, STREAM_STARTCODE, &buf)?;

        // Stream header (audio).
        buf.clear();
        v(&mut buf, 1)?; // stream_id
        v(&mut buf, 1)?; // stream_class = audio
        vb(&mut buf, b"PSD\x10")?; // fourcc = little-endian signed interleaved 16-bit
        v(&mut buf, 1)?; // time_base_id
        v(&mut buf, 0)?; // msb_pts_shift
        v(&mut buf, 1)?; // max_pts_distance
        v(&mut buf, 0)?; // decode_delay
        v(&mut buf, 1)?; // stream_flags = FLAG_FIXED_FPS
        vb(&mut buf, &[])?; // codec_specific_data

        v(&mut buf, 22050)?; // samplerate_num
        v(&mut buf, 1)?; // samplerate_denom
        v(&mut buf, 2)?; // channel_count

        packet(&mut writer, STREAM_STARTCODE, &buf)?;

        Ok(Self {
            child,
            video_pts: 0,
            audio_pts: 0,
        })
    }

    #[hawktracer(write_video_frame)]
    pub fn write_video_frame(&mut self, data: &[u8]) -> Result<(), io::Error> {
        const SYNCPOINT_STARTCODE: u64 = 0x4e4be4adeeca4569;

        let mut writer = self.child.stdin.as_mut().unwrap();

        // Syncpoint.
        let mut buf = Vec::new();
        v(&mut buf, self.video_pts * 2)?; // global_key_pts
        v(&mut buf, 0)?; // back_ptr_div16, ???

        packet(&mut writer, SYNCPOINT_STARTCODE, &buf)?;

        // Video frame.
        buf.clear();
        buf.write_all(&[1])?; // frame_code

        let flags =
              (1 << 0) // FLAG_KEY
            | (1 << 3) // FLAG_CODED_PTS
            | (1 << 4) // FLAG_STREAM_ID
            | (1 << 5) // FLAG_SIZE_MSB
            | (1 << 6) // FLAG_CHECKSUM
            ;
        v(&mut buf, flags)?; // coded_flags
        v(&mut buf, 0)?; // stream_id
        v(&mut buf, self.video_pts + (1 << 0))?; // coded_pts = pts + (1 << msb_pts_shift)
        v(&mut buf, data.len() as u64)?; // data_size_msb

        writer.write_all(&buf)?;
        writer.write_all(&crc32(&buf).to_be_bytes()[..])?; // checksum

        {
            scoped_tracepoint!(_write_video_data);
            writer.write_all(&data)?;
        }

        self.video_pts += 1;

        Ok(())
    }

    #[hawktracer(write_audio_frame)]
    pub fn write_audio_frame(&mut self, data: &[u8]) -> Result<(), io::Error> {
        const SYNCPOINT_STARTCODE: u64 = 0x4e4be4adeeca4569;

        let mut writer = self.child.stdin.as_mut().unwrap();

        // Syncpoint.
        let mut buf = Vec::new();
        v(&mut buf, self.audio_pts * 2 + 1)?; // global_key_pts
        v(&mut buf, 0)?; // back_ptr_div16, ???

        packet(&mut writer, SYNCPOINT_STARTCODE, &buf)?;

        // Audio frame.
        buf.clear();
        buf.write_all(&[1])?; // frame_code

        let flags =
              (1 << 0) // FLAG_KEY
            | (1 << 3) // FLAG_CODED_PTS
            | (1 << 4) // FLAG_STREAM_ID
            | (1 << 5) // FLAG_SIZE_MSB
            | (1 << 6) // FLAG_CHECKSUM
            ;
        v(&mut buf, flags)?; // coded_flags
        v(&mut buf, 1)?; // stream_id
        v(&mut buf, self.audio_pts + (1 << 0))?; // coded_pts = pts + (1 << msb_pts_shift)
        v(&mut buf, data.len() as u64)?; // data_size_msb

        writer.write_all(&buf)?;
        writer.write_all(&crc32(&buf).to_be_bytes()[..])?; // checksum
        writer.write_all(data)?;

        let samples = data.len() as u64 / 4; // 1 sample is 2×i16
        self.audio_pts += samples;

        Ok(())
    }

    #[hawktracer(muxer_close)]
    pub fn close(mut self) {
        let _ = self.child.wait();
        // use std::os::unix::ffi::OsStringExt;
        // let output = self.child.wait_with_output().unwrap();
        // let output = std::ffi::OsString::from_vec(output.stderr)
        //     .into_string()
        //     .unwrap();
        // println!("{}", &output);
    }
}
