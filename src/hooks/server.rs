//! `hl`, `opfor`, `bshift`.

#![allow(non_snake_case, non_upper_case_globals)]

use std::os::raw::*;
use std::ptr::NonNull;

use crate::ffi::edict::edict_s;
use crate::ffi::playermove::playermove_s;
use crate::ffi::usercmd::usercmd_s;
use crate::hooks::engine;
use crate::modules::{
    cheats, tas_logging, tas_optimizer, tas_recording, tas_server_time_fix, timer,
};
use crate::utils::*;

pub static CmdStart: Pointer<unsafe extern "C" fn(*const edict_s, *const usercmd_s, c_uint)> =
    Pointer::empty(b"CmdStart\0");
pub static PM_Move: Pointer<unsafe extern "C" fn(*mut playermove_s, c_int)> =
    Pointer::empty(b"PM_Move\0");
pub static DispatchTouch: Pointer<unsafe extern "C" fn(*mut edict_s, *mut edict_s)> =
    Pointer::empty(b"DispatchTouch\0");

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

    if let Some(pfnTouch) = &mut functions.pfnTouch {
        DispatchTouch.set(marker, Some(NonNull::new_unchecked(*pfnTouch as _)));
        *pfnTouch = my_DispatchTouch;
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

    if let Some(pfnTouch) = &mut functions.pfnTouch {
        *pfnTouch = DispatchTouch.get(marker);
        DispatchTouch.reset(marker);
    }
}

pub unsafe extern "C" fn my_CmdStart(
    player: *const edict_s,
    cmd: *const usercmd_s,
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
        cheats::noclip::pre_pm_move(marker, ppmove);

        PM_Move.get(marker)(ppmove, server);

        tas_server_time_fix::on_pm_move_end(marker, ppmove);
        tas_logging::write_post_pm_state(marker, ppmove);
        tas_logging::end_cmd_frame(marker);
        cheats::noclip::post_pm_move(marker, ppmove);
    })
}

pub unsafe extern "C" fn my_DispatchTouch(used: *mut edict_s, other: *mut edict_s) {
    abort_on_panic(move || {
        let marker = MainThreadMarker::new();

        timer::on_dispatch_touch(marker, used);

        DispatchTouch.get(marker)(used, other);
    })
}
