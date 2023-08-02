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
        "Exposing SV_StartSound to allow usage of other sound channels."
    }

    fn commands(&self) -> &'static [&'static Command] {
        static COMMANDS: &[&Command] = &[&BXT_EMIT_SOUND];
        COMMANDS
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        engine::SV_StartSound.is_set(marker) && engine::S_PrecacheSound.is_set(marker)
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
struct SoundInfo {
    sound: String,
    channel: i32,
    volume: i32,
    entity_index: u32,
    recipent: i32,
    attenuation: f32,
    flag: i32,
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
        rv.volume = (f32::from_str(iter.next().unwrap_or_default())?).round() as i32;
        rv.entity_index = (f32::from_str(iter.next().unwrap_or_default())?).round() as u32;
        rv.recipent = (f32::from_str(iter.next().unwrap_or_default())?).round() as i32;
        rv.attenuation = f32::from_str(iter.next().unwrap_or_default())?;
        rv.flag = (f32::from_str(iter.next().unwrap_or_default())?).round() as i32;
        rv.pitch = (f32::from_str(iter.next().unwrap_or_default())?).round() as i32;

        Ok(rv)
    }
}

static BXT_EMIT_SOUND: Command = Command::new(
    b"bxt_emit_sound\0",
    handler!(
        "bxt_emit_sound <sound> <channel> [volume] [entity index] [recipent] [attenuation] [flag] [pitch]
Plays sound file directly from SV_StartSound along with custom arguments.",
        play_sound as fn(_, _, _),
        play_sound_with_volume as fn(_, _, _, _),
        play_sound_full as fn(_, _)
    ),
);

fn play_sound(marker: MainThreadMarker, sound: String, channel: i32) {
    play_sound_with_volume(marker, sound, channel, 255);
}

fn play_sound_with_volume(marker: MainThreadMarker, sound: String, channel: i32, volume: i32) {
    play_sound_full(
        marker,
        SoundInfo {
            sound,
            channel,
            volume,
            entity_index: 0,
            recipent: 0,
            attenuation: 0.8,
            flag: 0,
            pitch: 100,
        },
    );
}

fn play_sound_full(marker: MainThreadMarker, info: SoundInfo) {
    if let Some(player) = unsafe { engine::player_edict(marker) } {
        // Player is usually index 0, from there just goes up.
        // bxt_emit_sound "common/bodysplat.wav 0 255 0 0 0.8 0 100"
        let mut entity =
            unsafe { NonNull::new(player.as_ptr().add(info.entity_index as usize)).unwrap() };

        let entity = unsafe { entity.as_mut() };
        let binding = CString::new(info.sound).unwrap();
        let sound = binding.as_ptr();

        unsafe {
            // Need to precache so it can play.
            engine::S_PrecacheSound.get(marker)(sound);

            engine::SV_StartSound.get(marker)(
                info.recipent,
                entity,
                info.channel,
                sound,
                info.volume,
                info.attenuation,
                info.flag,
                info.pitch,
            );
        };
    }
}
