//! bxt_skybox_name

use byte_slice_cast::AsSliceOf;

use super::commands::Command;
use super::Module;
use crate::handler;
use crate::hooks::engine;
use crate::modules::cvars::CVar;
use crate::utils::*;

pub struct SkyboxChange;
impl Module for SkyboxChange {
    fn name(&self) -> &'static str {
        "Skybox name"
    }

    fn description(&self) -> &'static str {
        "Changing skybox."
    }

    fn cvars(&self) -> &'static [&'static CVar] {
        static CVARS: &[&CVar] = &[&BXT_SKYBOX_NAME];
        CVARS
    }

    fn commands(&self) -> &'static [&'static Command] {
        static COMMANDS: &[&Command] = &[&BXT_SKYBOX_RELOAD];
        COMMANDS
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        engine::movevars.is_set(marker)
            && engine::R_LoadSkys.is_set(marker)
            && engine::gLoadSky.is_set(marker)
    }
}

static BXT_SKYBOX_NAME: CVar = CVar::new(
    b"bxt_skybox_name\0",
    b"\0",
    "\
Sets skybox name.

This does not take effect instantaneously unless reload command is also invoked.

Example: `bxt_skybox_name city`",
);

static BXT_SKYBOX_RELOAD: Command = Command::new(
    b"bxt_skybox_reload\0",
    handler!(
        "bxt_skybox_reload

Forces skybox name change.",
        reload as fn(_)
    ),
);

static ORIGINAL_SKYNAME: MainThreadRefCell<[i8; 32]> = MainThreadRefCell::new([0; 32]);

fn change_name(marker: MainThreadMarker) {
    let mv = unsafe { &mut *engine::movevars.get(marker) };
    let bytes = BXT_SKYBOX_NAME.to_string(marker).into_bytes();
    let original_skyname = *ORIGINAL_SKYNAME.borrow_mut(marker);

    // Make sure we don't have any side effects.
    assert_eq!(original_skyname[0], 0);

    *ORIGINAL_SKYNAME.borrow_mut(marker) = mv.skyName;

    // Capped size at 32.
    if !bytes.is_empty() && bytes.len() < 32 {
        mv.skyName.fill(0);
        mv.skyName[..bytes.len()].copy_from_slice(bytes.as_slice_of().unwrap());
    }
}

pub fn with_changed_name<T>(marker: MainThreadMarker, f: impl FnOnce() -> T) -> T {
    if !SkyboxChange.is_enabled(marker) {
        return f();
    }

    change_name(marker);

    let rv = f();

    restore(marker);

    rv
}

fn restore(marker: MainThreadMarker) {
    // Demo playback needs this. Though very unnecessary, it is nice to have.
    let mv = unsafe { &mut *engine::movevars.get(marker) };

    mv.skyName = *ORIGINAL_SKYNAME.borrow(marker);

    // Reset.
    *ORIGINAL_SKYNAME.borrow_mut(marker) = [0; 32];
}

fn reload(marker: MainThreadMarker) {
    with_changed_name(marker, move || {
        unsafe {
            // One single boolean check.
            *engine::gLoadSky.get(marker) = 1;
            engine::R_LoadSkys.get(marker)();
        }
    })
}
