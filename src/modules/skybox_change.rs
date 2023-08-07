//! bxt_skyname

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
        "Skybox name."
    }

    fn description(&self) -> &'static str {
        "Changing skybox."
    }

    fn cvars(&self) -> &'static [&'static CVar] {
        static CVARS: &[&CVar] = &[&BXT_SKYNAME];
        CVARS
    }

    fn commands(&self) -> &'static [&'static Command] {
        static COMMANDS: &[&Command] = &[&BXT_SKYNAME_FORCE];
        COMMANDS
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        engine::movevars.is_set(marker)
            && engine::R_LoadSkys.is_set(marker)
            && engine::gLoadSky.is_set(marker)
    }
}

static BXT_SKYNAME: CVar = CVar::new(
    b"bxt_skyname\0",
    b"\0",
    "\
Changes skybox name. This does not take effect instantaneously unless force command is also invoked.
However, if specified before map load, it will take effect.
Eg: `bxt_skyname city`",
);

static BXT_SKYNAME_FORCE: Command = Command::new(
    b"bxt_skyname_force\0",
    handler!(
        "bxt_skyname_force

Forces skybox name change.",
        force as fn(_)
    ),
);

static ORIGINAL_SKYNAME: MainThreadRefCell<[i8; 32]> = MainThreadRefCell::new([0; 32]);

pub fn change_name(marker: MainThreadMarker) {
    if !SkyboxChange.is_enabled(marker) {
        return;
    }

    let mv = unsafe { &mut *engine::movevars.get(marker) };
    let bytes = BXT_SKYNAME.to_string(marker).into_bytes();
    let original_skyname = *ORIGINAL_SKYNAME.borrow_mut(marker);

    if original_skyname[0] == 0 {
        // ORIGINAL_SKYNAME is not yet set.
        *ORIGINAL_SKYNAME.borrow_mut(marker) = mv.skyName;
    }

    // Capped size at 32.
    if bytes.len() > 0 && bytes.len() < 32 {
        mv.skyName.fill(0);
        mv.skyName[..bytes.len()].copy_from_slice(bytes.as_slice_of().unwrap());
    }
}

pub fn restore(marker: MainThreadMarker) {
    if !SkyboxChange.is_enabled(marker) {
        return;
    }

    // Demo playback needs this. Though very unnecessary, it is nice to have.
    let mv = unsafe { &mut *engine::movevars.get(marker) };

    mv.skyName = *ORIGINAL_SKYNAME.borrow(marker);
}

fn force(marker: MainThreadMarker) {
    change_name(marker);

    unsafe {
        // One single boolean check.
        *engine::gLoadSky.get(marker) = 1;
        engine::R_LoadSkys.get(marker)();
    }

    restore(marker);
}

pub fn reset_skyname(marker: MainThreadMarker) {
    *ORIGINAL_SKYNAME.borrow_mut(marker) = [0; 32];
}
