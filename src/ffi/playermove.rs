// bindgen halflife/pm_shared/pm_defs.h --allowlist-type "playermove_s" --
// --target=i686-unknown-linux-gnu -Ihalflife/{public,common,engine} -include mathlib.h -include
// const.h
// Keep everything before the generated part
// Then change playermove_s.flags member to use [`edict::Flags`] type
// Then change playermove_s.PlayerTrace argument `traceFlags` to use [`TraceFlags`] type
// Then change playermove_s.PlayerTrace argument `start` and `end` to `*const ...` from `*mut ...`
// Change `playermove_s` members who are functions from `Option<T>` to `T`.
// Remove the unnecessary qualifiers `::std::os::raw::` and `::std::mem::`
// Remove `vec_t` and `[f32; 3]` and replace `[f32; 3]` with `[f32; 3]`
// Remove `physent_t` alias and replace `physent_t` with `physent_s`
// Remove `pmtrace_t` alias and replace `pmtrace_t` with `pmtrace_s`
// Remove `usercmd_t` alias and replace `usercmd_t` with `usercmd_s`
// Remove `edict_t` alias and import `edict_s` from edict.rs
// Remove `model_s` dummy struct and import `model_s` from com_model.rs

// For some reasons, this `playermove_s` also generates physent, pmplane, pmtrace, and usercmd.
// So put those structs along with tests in their own respective files.

#![allow(unused, nonstandard_style, deref_nullptr)]

use std::mem::{align_of, offset_of, size_of};
use std::os::raw::*;
use std::ptr::null;

use bitflags::bitflags;

use super::com_model::model_s;
use super::edict::edict_s;
use super::physent::physent_s;
use super::pmplane::pmplane_t;
use super::pmtrace::pmtrace_s;
use super::usercmd::usercmd_s;
use crate::ffi::edict;

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[repr(C)]
    pub struct TraceFlags: c_int {
        const PM_NORMAL = 0;
        const PM_STUDIO_IGNORE = 1;
        const PM_STUDIO_BOX = 1 << 1;
        const PM_GLASS_IGNORE = 1 << 2;
        const PM_WORLD_ONLY = 1 << 3;
    }
}

pub const PM_TRACELINE_PHYSENTONLY: TraceLineFlag = 0;
pub const PM_TRACELINE_ANYVISIBLE: TraceLineFlag = 1;
pub type TraceLineFlag = c_int;

/* automatically generated by rust-bindgen 0.71.1 */

#[doc = "\tCopyright (c) 1996-2002, Valve LLC. All rights reserved.\n\n\tThis product contains software technology licensed from Id\n\tSoftware, Inc. (\"Id Technology\").  Id Technology (c) 1996 Id Software, Inc.\n\tAll Rights Reserved.\n\n   Use, distribution, and modification of this source code and/or resulting\n   object code is restricted to non-commercial enhancements to products from\n   Valve LLC.  All other use, distribution, or modification is prohibited\n   without written permission from Valve LLC."]
pub type string_t = c_uint;
pub type byte = c_uchar;
pub const qboolean_false_: qboolean = 0;
pub const qboolean_true_: qboolean = 1;
pub type qboolean = c_uint;
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct plane_t {
    pub normal: [f32; 3],
    pub dist: f32,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of plane_t"][size_of::<plane_t>() - 16usize];
    ["Alignment of plane_t"][align_of::<plane_t>() - 4usize];
    ["Offset of field: plane_t::normal"][offset_of!(plane_t, normal) - 0usize];
    ["Offset of field: plane_t::dist"][offset_of!(plane_t, dist) - 12usize];
};
#[repr(C)]
#[derive(Debug)]
pub struct trace_t {
    pub allsolid: qboolean,
    pub startsolid: qboolean,
    pub inopen: qboolean,
    pub inwater: qboolean,
    pub fraction: f32,
    pub endpos: [f32; 3],
    pub plane: plane_t,
    pub ent: *mut edict_s,
    pub hitgroup: c_int,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of trace_t"][size_of::<trace_t>() - 56usize];
    ["Alignment of trace_t"][align_of::<trace_t>() - 4usize];
    ["Offset of field: trace_t::allsolid"][offset_of!(trace_t, allsolid) - 0usize];
    ["Offset of field: trace_t::startsolid"][offset_of!(trace_t, startsolid) - 4usize];
    ["Offset of field: trace_t::inopen"][offset_of!(trace_t, inopen) - 8usize];
    ["Offset of field: trace_t::inwater"][offset_of!(trace_t, inwater) - 12usize];
    ["Offset of field: trace_t::fraction"][offset_of!(trace_t, fraction) - 16usize];
    ["Offset of field: trace_t::endpos"][offset_of!(trace_t, endpos) - 20usize];
    ["Offset of field: trace_t::plane"][offset_of!(trace_t, plane) - 32usize];
    ["Offset of field: trace_t::ent"][offset_of!(trace_t, ent) - 48usize];
    ["Offset of field: trace_t::hitgroup"][offset_of!(trace_t, hitgroup) - 52usize];
};
#[repr(C)]
#[derive(Debug)]
pub struct playermove_s {
    pub player_index: c_int,
    pub server: qboolean,
    pub multiplayer: qboolean,
    pub time: f32,
    pub frametime: f32,
    pub forward: [f32; 3],
    pub right: [f32; 3],
    pub up: [f32; 3],
    pub origin: [f32; 3],
    pub angles: [f32; 3],
    pub oldangles: [f32; 3],
    pub velocity: [f32; 3],
    pub movedir: [f32; 3],
    pub basevelocity: [f32; 3],
    pub view_ofs: [f32; 3],
    pub flDuckTime: f32,
    pub bInDuck: qboolean,
    pub flTimeStepSound: c_int,
    pub iStepLeft: c_int,
    pub flFallVelocity: f32,
    pub punchangle: [f32; 3],
    pub flSwimTime: f32,
    pub flNextPrimaryAttack: f32,
    pub effects: c_int,
    pub flags: edict::Flags,
    pub usehull: c_int,
    pub gravity: f32,
    pub friction: f32,
    pub oldbuttons: c_int,
    pub waterjumptime: f32,
    pub dead: qboolean,
    pub deadflag: c_int,
    pub spectator: c_int,
    pub movetype: c_int,
    pub onground: c_int,
    pub waterlevel: c_int,
    pub watertype: c_int,
    pub oldwaterlevel: c_int,
    pub sztexturename: [c_char; 256usize],
    pub chtexturetype: c_char,
    pub maxspeed: f32,
    pub clientmaxspeed: f32,
    pub iuser1: c_int,
    pub iuser2: c_int,
    pub iuser3: c_int,
    pub iuser4: c_int,
    pub fuser1: f32,
    pub fuser2: f32,
    pub fuser3: f32,
    pub fuser4: f32,
    pub vuser1: [f32; 3],
    pub vuser2: [f32; 3],
    pub vuser3: [f32; 3],
    pub vuser4: [f32; 3],
    pub numphysent: c_int,
    pub physents: [physent_s; 600usize],
    pub nummoveent: c_int,
    pub moveents: [physent_s; 64usize],
    pub numvisent: c_int,
    pub visents: [physent_s; 600usize],
    pub cmd: usercmd_s,
    pub numtouch: c_int,
    pub touchindex: [pmtrace_s; 600usize],
    pub physinfo: [c_char; 256usize],
    pub movevars: *mut movevars_s,
    pub player_mins: [[f32; 3]; 4usize],
    pub player_maxs: [[f32; 3]; 4usize],
    pub PM_Info_ValueForKey:
        unsafe extern "C" fn(s: *const c_char, key: *const c_char) -> *const c_char,
    pub PM_Particle:
        unsafe extern "C" fn(origin: *mut f32, color: c_int, life: f32, zpos: c_int, zvel: c_int),
    pub PM_TestPlayerPosition: unsafe extern "C" fn(pos: *mut f32, ptrace: *mut pmtrace_s) -> c_int,
    pub Con_NPrintf: unsafe extern "C" fn(idx: c_int, fmt: *mut c_char, ...),
    pub Con_DPrintf: unsafe extern "C" fn(fmt: *mut c_char, ...),
    pub Con_Printf: unsafe extern "C" fn(fmt: *mut c_char, ...),
    pub Sys_FloatTime: unsafe extern "C" fn() -> f64,
    pub PM_StuckTouch: unsafe extern "C" fn(hitent: c_int, ptraceresult: *mut pmtrace_s),
    pub PM_PointContents: unsafe extern "C" fn(p: *mut f32, truecontents: *mut c_int) -> c_int,
    pub PM_TruePointContents: unsafe extern "C" fn(p: *mut f32) -> c_int,
    pub PM_HullPointContents:
        unsafe extern "C" fn(hull: *mut hull_s, num: c_int, p: *mut f32) -> c_int,
    pub PM_PlayerTrace: unsafe extern "C" fn(
        start: *const f32,
        end: *const f32,
        traceFlags: TraceFlags,
        ignore_pe: c_int,
    ) -> pmtrace_s,
    pub PM_TraceLine: unsafe extern "C" fn(
        start: *const f32,
        end: *const f32,
        flags: TraceLineFlag,
        usehulll: c_int,
        ignore_pe: c_int,
    ) -> *mut pmtrace_s,
    pub RandomLong: unsafe extern "C" fn(lLow: c_int, lHigh: c_int) -> c_int,
    pub RandomFloat: unsafe extern "C" fn(flLow: f32, flHigh: f32) -> f32,
    pub PM_GetModelType: unsafe extern "C" fn(mod_: *mut model_s) -> c_int,
    pub PM_GetModelBounds: unsafe extern "C" fn(mod_: *mut model_s, mins: *mut f32, maxs: *mut f32),
    pub PM_HullForBsp: unsafe extern "C" fn(pe: *mut physent_s, offset: *mut f32) -> *mut c_void,
    pub PM_TraceModel: unsafe extern "C" fn(
        pEnt: *mut physent_s,
        start: *mut f32,
        end: *mut f32,
        trace: *mut trace_t,
    ) -> f32,
    pub COM_FileSize: unsafe extern "C" fn(filename: *mut c_char) -> c_int,
    pub COM_LoadFile:
        unsafe extern "C" fn(path: *mut c_char, usehunk: c_int, pLength: *mut c_int) -> *mut byte,
    pub COM_FreeFile: unsafe extern "C" fn(buffer: *mut c_void),
    pub memfgets: unsafe extern "C" fn(
        pMemFile: *mut byte,
        fileSize: c_int,
        pFilePos: *mut c_int,
        pBuffer: *mut c_char,
        bufferSize: c_int,
    ) -> *mut c_char,
    pub runfuncs: qboolean,
    pub PM_PlaySound: unsafe extern "C" fn(
        channel: c_int,
        sample: *const c_char,
        volume: f32,
        attenuation: f32,
        fFlags: c_int,
        pitch: c_int,
    ),
    pub PM_TraceTexture:
        unsafe extern "C" fn(ground: c_int, vstart: *mut f32, vend: *mut f32) -> *const c_char,
    pub PM_PlaybackEventFull: unsafe extern "C" fn(
        flags: c_int,
        clientindex: c_int,
        eventindex: c_ushort,
        delay: f32,
        origin: *mut f32,
        angles: *mut f32,
        fparam1: f32,
        fparam2: f32,
        iparam1: c_int,
        iparam2: c_int,
        bparam1: c_int,
        bparam2: c_int,
    ),
    pub PM_PlayerTraceEx: unsafe extern "C" fn(
        start: *const f32,
        end: *const f32,
        traceFlags: TraceFlags,
        pfnIgnore: unsafe extern "C" fn(pe: *mut physent_s) -> c_int,
    ) -> pmtrace_s,
    pub PM_TestPlayerPositionEx: unsafe extern "C" fn(
        pos: *mut f32,
        ptrace: *mut pmtrace_s,
        pfnIgnore: unsafe extern "C" fn(pe: *mut physent_s) -> c_int,
    ) -> c_int,
    pub PM_TraceLineEx: unsafe extern "C" fn(
        start: *const f32,
        end: *const f32,
        flags: TraceLineFlag,
        usehulll: c_int,
        pfnIgnore: unsafe extern "C" fn(pe: *mut physent_s) -> c_int,
    ) -> *mut pmtrace_s,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of playermove_s"][size_of::<playermove_s>() - 325068usize];
    ["Alignment of playermove_s"][align_of::<playermove_s>() - 4usize];
    ["Offset of field: playermove_s::player_index"]
        [offset_of!(playermove_s, player_index) - 0usize];
    ["Offset of field: playermove_s::server"][offset_of!(playermove_s, server) - 4usize];
    ["Offset of field: playermove_s::multiplayer"][offset_of!(playermove_s, multiplayer) - 8usize];
    ["Offset of field: playermove_s::time"][offset_of!(playermove_s, time) - 12usize];
    ["Offset of field: playermove_s::frametime"][offset_of!(playermove_s, frametime) - 16usize];
    ["Offset of field: playermove_s::forward"][offset_of!(playermove_s, forward) - 20usize];
    ["Offset of field: playermove_s::right"][offset_of!(playermove_s, right) - 32usize];
    ["Offset of field: playermove_s::up"][offset_of!(playermove_s, up) - 44usize];
    ["Offset of field: playermove_s::origin"][offset_of!(playermove_s, origin) - 56usize];
    ["Offset of field: playermove_s::angles"][offset_of!(playermove_s, angles) - 68usize];
    ["Offset of field: playermove_s::oldangles"][offset_of!(playermove_s, oldangles) - 80usize];
    ["Offset of field: playermove_s::velocity"][offset_of!(playermove_s, velocity) - 92usize];
    ["Offset of field: playermove_s::movedir"][offset_of!(playermove_s, movedir) - 104usize];
    ["Offset of field: playermove_s::basevelocity"]
        [offset_of!(playermove_s, basevelocity) - 116usize];
    ["Offset of field: playermove_s::view_ofs"][offset_of!(playermove_s, view_ofs) - 128usize];
    ["Offset of field: playermove_s::flDuckTime"][offset_of!(playermove_s, flDuckTime) - 140usize];
    ["Offset of field: playermove_s::bInDuck"][offset_of!(playermove_s, bInDuck) - 144usize];
    ["Offset of field: playermove_s::flTimeStepSound"]
        [offset_of!(playermove_s, flTimeStepSound) - 148usize];
    ["Offset of field: playermove_s::iStepLeft"][offset_of!(playermove_s, iStepLeft) - 152usize];
    ["Offset of field: playermove_s::flFallVelocity"]
        [offset_of!(playermove_s, flFallVelocity) - 156usize];
    ["Offset of field: playermove_s::punchangle"][offset_of!(playermove_s, punchangle) - 160usize];
    ["Offset of field: playermove_s::flSwimTime"][offset_of!(playermove_s, flSwimTime) - 172usize];
    ["Offset of field: playermove_s::flNextPrimaryAttack"]
        [offset_of!(playermove_s, flNextPrimaryAttack) - 176usize];
    ["Offset of field: playermove_s::effects"][offset_of!(playermove_s, effects) - 180usize];
    ["Offset of field: playermove_s::flags"][offset_of!(playermove_s, flags) - 184usize];
    ["Offset of field: playermove_s::usehull"][offset_of!(playermove_s, usehull) - 188usize];
    ["Offset of field: playermove_s::gravity"][offset_of!(playermove_s, gravity) - 192usize];
    ["Offset of field: playermove_s::friction"][offset_of!(playermove_s, friction) - 196usize];
    ["Offset of field: playermove_s::oldbuttons"][offset_of!(playermove_s, oldbuttons) - 200usize];
    ["Offset of field: playermove_s::waterjumptime"]
        [offset_of!(playermove_s, waterjumptime) - 204usize];
    ["Offset of field: playermove_s::dead"][offset_of!(playermove_s, dead) - 208usize];
    ["Offset of field: playermove_s::deadflag"][offset_of!(playermove_s, deadflag) - 212usize];
    ["Offset of field: playermove_s::spectator"][offset_of!(playermove_s, spectator) - 216usize];
    ["Offset of field: playermove_s::movetype"][offset_of!(playermove_s, movetype) - 220usize];
    ["Offset of field: playermove_s::onground"][offset_of!(playermove_s, onground) - 224usize];
    ["Offset of field: playermove_s::waterlevel"][offset_of!(playermove_s, waterlevel) - 228usize];
    ["Offset of field: playermove_s::watertype"][offset_of!(playermove_s, watertype) - 232usize];
    ["Offset of field: playermove_s::oldwaterlevel"]
        [offset_of!(playermove_s, oldwaterlevel) - 236usize];
    ["Offset of field: playermove_s::sztexturename"]
        [offset_of!(playermove_s, sztexturename) - 240usize];
    ["Offset of field: playermove_s::chtexturetype"]
        [offset_of!(playermove_s, chtexturetype) - 496usize];
    ["Offset of field: playermove_s::maxspeed"][offset_of!(playermove_s, maxspeed) - 500usize];
    ["Offset of field: playermove_s::clientmaxspeed"]
        [offset_of!(playermove_s, clientmaxspeed) - 504usize];
    ["Offset of field: playermove_s::iuser1"][offset_of!(playermove_s, iuser1) - 508usize];
    ["Offset of field: playermove_s::iuser2"][offset_of!(playermove_s, iuser2) - 512usize];
    ["Offset of field: playermove_s::iuser3"][offset_of!(playermove_s, iuser3) - 516usize];
    ["Offset of field: playermove_s::iuser4"][offset_of!(playermove_s, iuser4) - 520usize];
    ["Offset of field: playermove_s::fuser1"][offset_of!(playermove_s, fuser1) - 524usize];
    ["Offset of field: playermove_s::fuser2"][offset_of!(playermove_s, fuser2) - 528usize];
    ["Offset of field: playermove_s::fuser3"][offset_of!(playermove_s, fuser3) - 532usize];
    ["Offset of field: playermove_s::fuser4"][offset_of!(playermove_s, fuser4) - 536usize];
    ["Offset of field: playermove_s::vuser1"][offset_of!(playermove_s, vuser1) - 540usize];
    ["Offset of field: playermove_s::vuser2"][offset_of!(playermove_s, vuser2) - 552usize];
    ["Offset of field: playermove_s::vuser3"][offset_of!(playermove_s, vuser3) - 564usize];
    ["Offset of field: playermove_s::vuser4"][offset_of!(playermove_s, vuser4) - 576usize];
    ["Offset of field: playermove_s::numphysent"][offset_of!(playermove_s, numphysent) - 588usize];
    ["Offset of field: playermove_s::physents"][offset_of!(playermove_s, physents) - 592usize];
    ["Offset of field: playermove_s::nummoveent"]
        [offset_of!(playermove_s, nummoveent) - 134992usize];
    ["Offset of field: playermove_s::moveents"][offset_of!(playermove_s, moveents) - 134996usize];
    ["Offset of field: playermove_s::numvisent"][offset_of!(playermove_s, numvisent) - 149332usize];
    ["Offset of field: playermove_s::visents"][offset_of!(playermove_s, visents) - 149336usize];
    ["Offset of field: playermove_s::cmd"][offset_of!(playermove_s, cmd) - 283736usize];
    ["Offset of field: playermove_s::numtouch"][offset_of!(playermove_s, numtouch) - 283788usize];
    ["Offset of field: playermove_s::touchindex"]
        [offset_of!(playermove_s, touchindex) - 283792usize];
    ["Offset of field: playermove_s::physinfo"][offset_of!(playermove_s, physinfo) - 324592usize];
    ["Offset of field: playermove_s::movevars"][offset_of!(playermove_s, movevars) - 324848usize];
    ["Offset of field: playermove_s::player_mins"]
        [offset_of!(playermove_s, player_mins) - 324852usize];
    ["Offset of field: playermove_s::player_maxs"]
        [offset_of!(playermove_s, player_maxs) - 324900usize];
    ["Offset of field: playermove_s::PM_Info_ValueForKey"]
        [offset_of!(playermove_s, PM_Info_ValueForKey) - 324948usize];
    ["Offset of field: playermove_s::PM_Particle"]
        [offset_of!(playermove_s, PM_Particle) - 324952usize];
    ["Offset of field: playermove_s::PM_TestPlayerPosition"]
        [offset_of!(playermove_s, PM_TestPlayerPosition) - 324956usize];
    ["Offset of field: playermove_s::Con_NPrintf"]
        [offset_of!(playermove_s, Con_NPrintf) - 324960usize];
    ["Offset of field: playermove_s::Con_DPrintf"]
        [offset_of!(playermove_s, Con_DPrintf) - 324964usize];
    ["Offset of field: playermove_s::Con_Printf"]
        [offset_of!(playermove_s, Con_Printf) - 324968usize];
    ["Offset of field: playermove_s::Sys_FloatTime"]
        [offset_of!(playermove_s, Sys_FloatTime) - 324972usize];
    ["Offset of field: playermove_s::PM_StuckTouch"]
        [offset_of!(playermove_s, PM_StuckTouch) - 324976usize];
    ["Offset of field: playermove_s::PM_PointContents"]
        [offset_of!(playermove_s, PM_PointContents) - 324980usize];
    ["Offset of field: playermove_s::PM_TruePointContents"]
        [offset_of!(playermove_s, PM_TruePointContents) - 324984usize];
    ["Offset of field: playermove_s::PM_HullPointContents"]
        [offset_of!(playermove_s, PM_HullPointContents) - 324988usize];
    ["Offset of field: playermove_s::PM_PlayerTrace"]
        [offset_of!(playermove_s, PM_PlayerTrace) - 324992usize];
    ["Offset of field: playermove_s::PM_TraceLine"]
        [offset_of!(playermove_s, PM_TraceLine) - 324996usize];
    ["Offset of field: playermove_s::RandomLong"]
        [offset_of!(playermove_s, RandomLong) - 325000usize];
    ["Offset of field: playermove_s::RandomFloat"]
        [offset_of!(playermove_s, RandomFloat) - 325004usize];
    ["Offset of field: playermove_s::PM_GetModelType"]
        [offset_of!(playermove_s, PM_GetModelType) - 325008usize];
    ["Offset of field: playermove_s::PM_GetModelBounds"]
        [offset_of!(playermove_s, PM_GetModelBounds) - 325012usize];
    ["Offset of field: playermove_s::PM_HullForBsp"]
        [offset_of!(playermove_s, PM_HullForBsp) - 325016usize];
    ["Offset of field: playermove_s::PM_TraceModel"]
        [offset_of!(playermove_s, PM_TraceModel) - 325020usize];
    ["Offset of field: playermove_s::COM_FileSize"]
        [offset_of!(playermove_s, COM_FileSize) - 325024usize];
    ["Offset of field: playermove_s::COM_LoadFile"]
        [offset_of!(playermove_s, COM_LoadFile) - 325028usize];
    ["Offset of field: playermove_s::COM_FreeFile"]
        [offset_of!(playermove_s, COM_FreeFile) - 325032usize];
    ["Offset of field: playermove_s::memfgets"][offset_of!(playermove_s, memfgets) - 325036usize];
    ["Offset of field: playermove_s::runfuncs"][offset_of!(playermove_s, runfuncs) - 325040usize];
    ["Offset of field: playermove_s::PM_PlaySound"]
        [offset_of!(playermove_s, PM_PlaySound) - 325044usize];
    ["Offset of field: playermove_s::PM_TraceTexture"]
        [offset_of!(playermove_s, PM_TraceTexture) - 325048usize];
    ["Offset of field: playermove_s::PM_PlaybackEventFull"]
        [offset_of!(playermove_s, PM_PlaybackEventFull) - 325052usize];
    ["Offset of field: playermove_s::PM_PlayerTraceEx"]
        [offset_of!(playermove_s, PM_PlayerTraceEx) - 325056usize];
    ["Offset of field: playermove_s::PM_TestPlayerPositionEx"]
        [offset_of!(playermove_s, PM_TestPlayerPositionEx) - 325060usize];
    ["Offset of field: playermove_s::PM_TraceLineEx"]
        [offset_of!(playermove_s, PM_TraceLineEx) - 325064usize];
};
#[repr(C)]
#[derive(Debug)]
pub struct movevars_s {
    pub _address: u8,
}
#[repr(C)]
#[derive(Debug)]
pub struct hull_s {
    _unused: [u8; 0],
}
