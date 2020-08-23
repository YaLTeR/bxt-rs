//! `hl`, `opfor`, `bshift`.

#![allow(non_snake_case)]

use std::{os::raw::*, ptr::NonNull};

use crate::{
    ffi::{playermove::playermove_s, usercmd::usercmd_s},
    hooks::engine,
    modules::tas_logging,
    utils::*,
};

pub static CMDSTART: Pointer<unsafe extern "C" fn(*mut c_void, *mut usercmd_s, c_uint)> =
    Pointer::empty(b"CmdStart\0");
pub static PM_MOVE: Pointer<unsafe extern "C" fn(*mut playermove_s, c_int)> =
    Pointer::empty(b"PM_Move\0");

/// # Safety
///
/// This function must only be called right after `LoadEntityDLLs()` is called.
pub unsafe fn hook_entity_interface(marker: MainThreadMarker) {
    let functions = engine::GENTITYINTERFACE.get_opt(marker);
    if functions.is_none() {
        return;
    }
    let functions = functions.unwrap().as_mut().unwrap();

    if let Some(pm_move) = &mut functions.pm_move {
        PM_MOVE.set(marker, Some(NonNull::new_unchecked(*pm_move as _)));
        *pm_move = my_PM_Move;
    }

    if let Some(cmd_start) = &mut functions.cmd_start {
        CMDSTART.set(marker, Some(NonNull::new_unchecked(*cmd_start as _)));
        *cmd_start = my_CmdStart;
    }
}

/// # Safety
///
/// This function must only be called right before `ReleaseEntityDlls()` is called.
pub unsafe fn reset_entity_interface(marker: MainThreadMarker) {
    let functions = engine::GENTITYINTERFACE.get_opt(marker);
    if functions.is_none() {
        return;
    }
    let functions = functions.unwrap().as_mut().unwrap();

    if let Some(pm_move) = &mut functions.pm_move {
        *pm_move = PM_MOVE.get(marker);
        PM_MOVE.reset(marker);
    }

    if let Some(cmd_start) = &mut functions.cmd_start {
        *cmd_start = CMDSTART.get(marker);
        CMDSTART.reset(marker);
    }
}

#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn my_CmdStart(
    player: *mut c_void,
    cmd: *mut usercmd_s,
    random_seed: c_uint,
) {
    abort_on_panic(move || {
        let marker = MainThreadMarker::new();

        tas_logging::begin_cmd_frame(marker, *cmd, random_seed);

        CMDSTART.get(marker)(player, cmd, random_seed);
    })
}

#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn my_PM_Move(ppmove: *mut playermove_s, server: c_int) {
    abort_on_panic(move || {
        let marker = MainThreadMarker::new();

        tas_logging::write_pre_pm_state(marker, ppmove);

        PM_MOVE.get(marker)(ppmove, server);

        tas_logging::write_post_pm_state(marker, ppmove);
        tas_logging::end_cmd_frame(marker);
    })
}
