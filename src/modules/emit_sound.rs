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
        static COMMANDS: &[&Command] = &[&BXT_EMIT_SOUND_STOP_SOUND_EXCEPT, &BXT_EMIT_SOUND];
        COMMANDS
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        engine::S_StartDynamicSound.is_set(marker)
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
    /// "NORM" is 0.8
    attenuation: f32,
    flag: i32,
    /// 100 is no shift.
    pitch: i32,
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

Plays sound file directly from S_StartDynamicSound.",
        emit_sound as fn(_, _, _),
        emit_sound_full as fn(_, _)
    ),
);

fn emit_sound(marker: MainThreadMarker, sound: String, channel: i32) {
    emit_sound_full(
        marker,
        SoundInfo {
            sound,
            channel,
            volume: 1.,
            from: 0,
            to: 0,
            attenuation: 0.8,
            flag: 0,
            pitch: 100,
        },
    )
}

fn emit_sound_full(marker: MainThreadMarker, info: SoundInfo) {
    if let Some(player) = unsafe { engine::player_edict(marker) } {
        // TODO: mimic S_StartDynamicSound fully so sounds played from player will follow.
        let to = unsafe { NonNull::new(player.as_ptr().add(info.to as usize)).unwrap() };
        let origin = unsafe { to.as_ref() }.v.origin;

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
        
Stops emitting sounds from every channel except for given list of channels.",
        stop_sound as fn(_, _)
    ),
);

fn stop_sound(marker: MainThreadMarker, channels: String) {
    let mut list: Vec<i32> = Vec::new();
    // There should be only 8 usable channels from SV_StartSounds.
    // But SND_PickDynamicChannel says something very weird otherwise.
    let mut list0: Vec<i32> = vec![0, 1, 2, 3, 4, 5, 6, 7];
    let iter = channels.split_ascii_whitespace();

    for channel in iter {
        if let Ok(allow) = channel.parse::<i32>() {
            list.push(allow);
        }
    }

    // Eh.
    list.sort();

    for channel in list.iter().rev() {
        list0.remove(*channel as usize);
    }

    for channel in list0 {
        unsafe { engine::S_StopSound.get(marker)(0, channel) };
    }
}
