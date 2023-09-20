//! bxt_cap_skip_non_gameplay_frames.

use super::cvars::CVar;
use super::{capture, Module};
use crate::hooks::engine;
use crate::utils::*;

pub struct CaptureSkipNonGameplay;
impl Module for CaptureSkipNonGameplay {
    fn name(&self) -> &'static str {
        "Video capture (skips non-gameplay frames)"
    }

    fn description(&self) -> &'static str {
        "Skipping loading frames or non-functional frames."
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        capture::Capture.is_enabled(marker)
            && engine::cls.is_set(marker)
            && engine::cl_stats.is_set(marker)
            && engine::cls_demoframecount.is_set(marker)
            && engine::cls_demos.is_set(marker)
    }

    fn cvars(&self) -> &'static [&'static super::cvars::CVar] {
        static CVARS: &[&CVar] = &[&BXT_CAP_SKIP_NON_GAMEPLAY_FRAMES];
        CVARS
    }
}

static BXT_CAP_SKIP_NON_GAMEPLAY_FRAMES: CVar = CVar::new(
    b"bxt_cap_skip_non_gameplay_frames\0",
    b"0\0",
    "\
Skipping recording non-gameplay frames such as main menu, loading screen, or demo load.
Set to `0` to disable. Set to `1` to enable. Default is `2`. \
Any values higher than `1` will be the extra 'gameplay' frames being skipped. \
For example, `2` means ''1'' extra gameplay frame skipped during capture.",
);

static FRAMES_SKIPPED: MainThreadCell<u32> = MainThreadCell::new(0);

pub unsafe fn should_skip_non_gameplay_frames(marker: MainThreadMarker) -> bool {
    if !CaptureSkipNonGameplay.is_enabled(marker) {
        return false;
    }

    let skip_frames = BXT_CAP_SKIP_NON_GAMEPLAY_FRAMES.as_u64(marker);

    if skip_frames == 0 {
        return false;
    }

    if (&*engine::cls.get(marker)).state != 5 {
        // If state is not 5, skip frame.
        // State 4 is still loading.
        return true;
    }

    // demoplayback is updated to 1 after state 4 is done.
    // The current implementation will skip all the frames until some viewmodel values are set.
    if (&*engine::cls_demos.get(marker)).demoplayback == 1 {
        // For some reasons, the "true" first frame will be catched in this condition
        // despite having no viewmodel shown in demo.
        // So it does technically capture all non-loading frames.
        if (*engine::cl_stats.get(marker))[2] == 0 {
            // Fallback when there is no viewmodel assigned ever. Happens when no weapons are picked
            // up. Frame 7 is guaranteed to be the "start" frame most of the time.
            if *engine::cls_demoframecount.get(marker) < 7 {
                return true;
            }
        }

        // Alternative use of the cvar to skip multiple starting frames.
        if FRAMES_SKIPPED.get(marker) + 1 < skip_frames as u32 {
            FRAMES_SKIPPED.set(marker, FRAMES_SKIPPED.get(marker) + 1);
            return true;
        }
    }

    false
}

pub fn on_cl_disconnect(marker: MainThreadMarker) {
    FRAMES_SKIPPED.set(marker, 0);
}
