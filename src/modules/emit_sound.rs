//! `bxt_emit_sound`

use std::ffi::CString;
use std::num::ParseFloatError;
use std::ptr::NonNull;
use std::str::FromStr;

use super::Module;
use crate::handler;
use crate::hooks::engine;
use crate::modules::commands::Command;
use crate::utils::*;

pub struct EmitSound;
impl Module for EmitSound {
    fn name(&self) -> &'static str {
        "bxt_emit_sound"
    }

    fn description(&self) -> &'static str {
        "Exposing some sound functions."
    }

    fn commands(&self) -> &'static [&'static Command] {
        static COMMANDS: &[&Command] = &[
            &BXT_EMIT_SOUND_STOP_SOUND_EXCEPT,
            &BXT_EMIT_SOUND_DYNAMIC,
            &BXT_EMIT_SOUND,
        ];
        COMMANDS
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        engine::SV_StartSound.is_set(marker)
            && engine::S_StartDynamicSound.is_set(marker)
            && engine::S_StopSound.is_set(marker)
            && engine::S_PrecacheSound.is_set(marker)
    }
}

#[derive(Default)]
struct SoundInfo {
    sound: String,
    channel: i32,
    /// Volume is [0..1]
    volume: f32,
    from: i32,
    to: i32,
    /// This is the "radius" of the sound. Close to 1, you hear sound quieter when moving farther
    /// from entity. "NORM" for sound-emitting entity is 0.8.
    /// 0 is not quite to make it "global". Negative value will do.
    attenuation: f32,
    flag: i32,
    /// 100 is no shift.
    pitch: i32,
}

impl SoundInfo {
    fn new(sound: String, channel: i32) -> Self {
        SoundInfo {
            sound,
            channel,
            volume: 1.,
            from: 0,
            to: 0,
            attenuation: 0.,
            flag: 0,
            pitch: 100,
        }
    }
}

// Eh.
// Parse float then round then convert to integer to avoid a more convoluted solution.
impl FromStr for SoundInfo {
    type Err = ParseFloatError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut rv = SoundInfo::default();

        let mut iter = s.split_ascii_whitespace();
        rv.sound = iter.next().unwrap_or_default().to_string();
        rv.channel = (f32::from_str(iter.next().unwrap_or_default())?).round() as i32;
        rv.volume = f32::from_str(iter.next().unwrap_or_default())?;
        rv.from = (f32::from_str(iter.next().unwrap_or_default())?).round() as i32;
        rv.to = (f32::from_str(iter.next().unwrap_or_default())?).round() as i32;
        rv.attenuation = f32::from_str(iter.next().unwrap_or_default())?;
        rv.flag = (f32::from_str(iter.next().unwrap_or_default())?).round() as i32;
        rv.pitch = (f32::from_str(iter.next().unwrap_or_default())?).round() as i32;

        Ok(rv)
    }
}

static BXT_EMIT_SOUND: Command = Command::new(
    b"bxt_emit_sound\0",
    handler!(
        "bxt_emit_sound <sound> <channel> [volume] [from] [to] [attenuation] [flag] [pitch]
Plays sound file directly from SV_StartSound.",
        emit_sound as fn(_, _, _),
        emit_sound_full as fn(_, _)
    ),
);

fn emit_sound(marker: MainThreadMarker, sound: String, channel: i32) {
    emit_sound_full(marker, SoundInfo::new(sound, channel));
}

fn emit_sound_full(marker: MainThreadMarker, info: SoundInfo) {
    if let Some(player) = unsafe { engine::player_edict(marker) } {
        let mut entity = unsafe { NonNull::new(player.as_ptr().add(info.from as usize)).unwrap() };

        let entity = unsafe { entity.as_mut() };
        let binding = CString::new(info.sound).unwrap();
        let sound = binding.as_ptr();

        unsafe {
            // TODO: precache the sound so it can play any sound like dynamic option.
            engine::S_PrecacheSound.get(marker)(sound);

            engine::SV_StartSound.get(marker)(
                info.to,
                entity,
                info.channel,
                sound,
                // [0,255]
                (info.volume * 255.).round() as i32,
                info.attenuation,
                info.flag,
                info.pitch,
            );
        };
    }
}

static BXT_EMIT_SOUND_DYNAMIC: Command = Command::new(
    b"bxt_emit_sound_dynamic\0",
    handler!(
        "bxt_emit_sound_dynamic <sound> <channel> [volume] [from] [to] [attenuation] [flag] [pitch]

Plays sound file directly from S_StartDynamicSound.",
        emit_sound_dynamic as fn(_, _, _),
        emit_sound_dynamic_full as fn(_, _)
    ),
);

fn emit_sound_dynamic(marker: MainThreadMarker, sound: String, channel: i32) {
    emit_sound_dynamic_full(marker, SoundInfo::new(sound, channel))
}

fn emit_sound_dynamic_full(marker: MainThreadMarker, info: SoundInfo) {
    if let Some(player) = unsafe { engine::player_edict(marker) } {
        let origin = unsafe {
            if info.to == 0 {
                // It does this to have the sound follow player's vieworg rather than origin.
                *engine::listener_origin.get(marker)
            } else {
                let to = NonNull::new(player.as_ptr().add(info.to as usize)).unwrap();
                to.as_ref().v.origin
            }
        };

        let binding = CString::new(info.sound).unwrap();
        let sound = binding.as_ptr();
        let precache_result = unsafe { engine::S_PrecacheSound.get(marker)(sound) };

        unsafe {
            engine::S_StartDynamicSound.get(marker)(
                info.from,
                info.channel,
                precache_result,
                origin.as_ptr(),
                info.volume,
                info.attenuation,
                info.flag,
                info.pitch,
            )
        };
    }
}

static BXT_EMIT_SOUND_STOP_SOUND_EXCEPT: Command = Command::new(
    b"bxt_emit_sound_stop_sound_except\0",
    handler!(
        "bxt_emit_sound_stop_sound_except <list of channels seperated by space>.
        
Stops emitting sounds from every channel except given list of channels.",
        stop_sound as fn(_, _)
    ),
);

// It goes up to 14-15 something. I've seen up to 8 used so far.
static MAX_CHANNELS: i32 = 10;

fn stop_sound(marker: MainThreadMarker, channels: String) {
    let mut list = vec![];
    // There should be only 8 usable channels from SV_StartSounds.
    // But SND_PickDynamicChannel says something very weird otherwise.
    let mut list0 = Vec::from_iter(0..MAX_CHANNELS + 1);
    let iter = channels.split_ascii_whitespace();

    for channel in iter {
        if let Ok(allow) = channel.parse::<i32>() {
            list.push(allow);
        }
    }

    // Eh.
    list.sort();

    for channel in list.iter().rev() {
        let channel = (*channel).clamp(0i32, MAX_CHANNELS);
        list0.remove(channel as usize);
    }

    for channel in list0 {
        unsafe { engine::S_StopSound.get(marker)(0, channel) };
    }
}
