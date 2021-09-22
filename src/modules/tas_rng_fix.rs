//! TAS RNG fix.

use super::Module;
use crate::hooks::engine;
use crate::utils::*;

pub struct TasRngFix;
impl Module for TasRngFix {
    fn name(&self) -> &'static str {
        "TAS RNG fix"
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        engine::RandomLong.is_set(marker) && engine::S_StartDynamicSound.is_set(marker)
    }
}

static INSIDE_S_START_DYNAMIC_SOUND: MainThreadCell<bool> = MainThreadCell::new(false);

pub fn on_s_start_dynamic_sound_start(marker: MainThreadMarker) {
    INSIDE_S_START_DYNAMIC_SOUND.set(marker, true);
}

pub fn on_s_start_dynamic_sound_end(marker: MainThreadMarker) {
    INSIDE_S_START_DYNAMIC_SOUND.set(marker, false);
}

pub fn should_skip_random_long(marker: MainThreadMarker) -> bool {
    // S_StartDynamicSound() tries to find a free sound channel. If it finds one, it
    // plays the sound and calls RandomLong() once. If it doesn't find a free channel,
    // it exits early. Thus, the availability of a free sound channel, which may vary
    // depending on the TAS playback speed, influences the non-shared RNG state. To fix
    // this, avoid the RandomLong() call inside S_StartDynamicSound() altogether.

    INSIDE_S_START_DYNAMIC_SOUND.get(marker)
}
