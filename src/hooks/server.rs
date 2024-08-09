//! `hl`, `opfor`, `bshift`.

#![allow(non_snake_case, non_upper_case_globals)]

use std::os::raw::*;
use std::ptr::{null_mut, NonNull};

use bxt_macros::pattern;
use bxt_patterns::Patterns;

use crate::ffi::edict::{edict_s, entvars_s};
use crate::ffi::playermove::playermove_s;
use crate::ffi::usercmd::usercmd_s;
use crate::hooks::engine;
use crate::modules::{tas_logging, tas_optimizer, tas_recording, tas_server_time_fix};
use crate::utils::*;

pub static CmdStart: Pointer<unsafe extern "C" fn(*mut c_void, *mut usercmd_s, c_uint)> =
    Pointer::empty(b"CmdStart\0");
pub static PM_Move: Pointer<unsafe extern "C" fn(*mut playermove_s, c_int)> =
    Pointer::empty(b"PM_Move\0");

pub static CBaseEntity__Create: Pointer<
    unsafe extern "C" fn(*mut c_char, *mut c_float, *mut c_float, *mut c_void) -> *mut c_void,
> = Pointer::empty_patterns(
    b"CBaseEntity::Create\0",
    Patterns(&[
        // cstrike 8684
        pattern!(A1 ?? ?? ?? ?? 56 8B 74 24 ?? 57 2B B0 ?? ?? ?? ?? 56 FF 15),
    ]),
    null_mut(),
);
pub static CBaseEntity__Create_Linux: Pointer<
    unsafe extern "C" fn(*mut c_char, *mut c_float, *mut c_float, *mut c_void) -> *mut c_void,
> = Pointer::empty(b"_ZN11CBaseEntity6CreateEPcRK6VectorS3_P7edict_s\0");
pub static UTIL_Remove: Pointer<unsafe extern "C" fn(*mut c_void)> =
    Pointer::empty(b"_Z11UTIL_RemoveP11CBaseEntity\0");
// pub static CBasePlayer__TakeDamage: Pointer<unsafe extern "C" fn(*mut c_void, *mut entvars_s,
// *mut entvars_s, c_float, c_int)> =
// Pointer::empty_patterns(b"_ZN11CBasePlayer10TakeDamageEP9entvars_sS1_fi\0", Patterns(&[]),
// my_CBasePlayer__TakeDamage as _);

static POINTERS: &[&dyn PointerTrait] = &[
    &CBaseEntity__Create,
    &CBaseEntity__Create_Linux,
    &UTIL_Remove,
    // &CBasePlayer__TakeDamage,
];

#[cfg(unix)]
fn open_library(library_path: &str) -> Option<libloading::Library> {
    use libc::{RTLD_NOLOAD, RTLD_NOW};

    let library =
        unsafe { libloading::os::unix::Library::open(Some(library_path), RTLD_NOW | RTLD_NOLOAD) };
    library.ok().map(libloading::Library::from)
}

#[cfg(windows)]
fn open_library(library_path: &str) -> Option<libloading::Library> {
    libloading::os::windows::Library::open_already_loaded(library_path)
        .ok()
        .map(libloading::Library::from)
}

#[instrument(name = "server::find_pointers", skip_all)]
pub unsafe fn find_pointers(marker: MainThreadMarker, library_path: &str) {
    let Some(library) = open_library(library_path) else {
        debug!("could not find server library");
        return;
    };

    for pointer in POINTERS {
        let ptr = library
            .get(pointer.symbol())
            .ok()
            .and_then(|sym| NonNull::new(*sym));
        pointer.set(marker, ptr);
        pointer.log(marker);
    }

    // set_callbacks(marker);
}

/// # Safety
///
/// This function must only be called right after `LoadEntityDLLs()` is called.
pub unsafe fn hook_entity_interface(marker: MainThreadMarker) {
    let functions = engine::gEntityInterface.get_opt(marker);
    if functions.is_none() {
        return;
    }
    let functions = functions.unwrap().as_mut().unwrap();

    if let Some(pm_move) = &mut functions.pm_move {
        PM_Move.set(marker, Some(NonNull::new_unchecked(*pm_move as _)));
        *pm_move = my_PM_Move;
    }

    if let Some(cmd_start) = &mut functions.cmd_start {
        CmdStart.set(marker, Some(NonNull::new_unchecked(*cmd_start as _)));
        *cmd_start = my_CmdStart;
    }
}

/// # Safety
///
/// This function must only be called right before `ReleaseEntityDlls()` is called.
pub unsafe fn reset_entity_interface(marker: MainThreadMarker) {
    let functions = engine::gEntityInterface.get_opt(marker);
    if functions.is_none() {
        return;
    }
    let functions = functions.unwrap().as_mut().unwrap();

    if let Some(pm_move) = &mut functions.pm_move {
        *pm_move = PM_Move.get(marker);
        PM_Move.reset(marker);
    }

    if let Some(cmd_start) = &mut functions.cmd_start {
        *cmd_start = CmdStart.get(marker);
        CmdStart.reset(marker);
    }
}

pub unsafe extern "C" fn my_CmdStart(
    player: *mut c_void,
    cmd: *mut usercmd_s,
    random_seed: c_uint,
) {
    abort_on_panic(move || {
        let marker = MainThreadMarker::new();

        tas_logging::begin_cmd_frame(marker, *cmd, random_seed);
        tas_recording::on_cmd_start(marker, *cmd, random_seed);
        tas_optimizer::on_cmd_start(marker);

        CmdStart.get(marker)(player, cmd, random_seed);
    })
}

pub unsafe extern "C" fn my_PM_Move(ppmove: *mut playermove_s, server: c_int) {
    abort_on_panic(move || {
        let marker = MainThreadMarker::new();

        tas_logging::write_pre_pm_state(marker, ppmove);
        tas_server_time_fix::on_pm_move_start(marker, ppmove);

        PM_Move.get(marker)(ppmove, server);

        tas_server_time_fix::on_pm_move_end(marker, ppmove);
        tas_logging::write_post_pm_state(marker, ppmove);
        tas_logging::end_cmd_frame(marker);
    })
}

use exported::*;

mod exported {
    use super::*;

    // #[export_name = "_ZN11CBasePlayer10TakeDamageEP9entvars_sS1_fi"]
    // pub unsafe fn my_CBasePlayer__TakeDamage(thisptr: *mut c_void, pevInflictor: *mut entvars_s,
    // pevAttacker: *mut entvars_s, flDamage: c_float, bitsDamageType: c_int) {
    //     abort_on_panic(move || {
    //         let marker = MainThreadMarker::new();

    //         println!("this runs");

    //         CBasePlayer__TakeDamage.get(marker)(thisptr, pevInflictor, pevAttacker, flDamage,
    // bitsDamageType);     });
    // }
}
