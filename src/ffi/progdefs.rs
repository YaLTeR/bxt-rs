//! Written manually and it is available in hlsdk

#![allow(unused, nonstandard_style, deref_nullptr)]

use std::os::raw::*;

use crate::ffi::edict::edict_s;

pub type string_t = u32;
pub type vec3_t = [f32; 3];

#[repr(C)]
pub struct globalvars_t {
    pub time: c_float,
    pub frametime: c_float,
    pub force_retouch: c_float,
    pub mapname: string_t,
    pub startspot: string_t,
    pub deathmatch: c_float,
    pub coop: c_float,
    pub teamplay: c_float,
    pub serverflags: c_float,
    pub found_secrets: c_float,
    pub v_forward: vec3_t,
    pub v_up: vec3_t,
    pub v_right: vec3_t,
    pub trace_allsolid: c_float,
    pub trace_startsolid: c_float,
    pub trace_fraction: c_float,
    pub trace_endpos: vec3_t,
    pub trace_plane_normal: vec3_t,
    pub trace_plane_dist: c_float,
    pub trace_ent: *mut edict_s,
    pub trace_inopen: c_float,
    pub trace_inwater: c_float,
    pub trace_hitgroup: c_int,
    pub trace_flags: c_int,
    pub msg_entity: c_int,
    pub cdAudioTrack: c_int,
    pub maxClients: c_int,
    pub maxEntities: c_int,
    pub pStringBase: *const c_char,

    pub pSaveData: *mut c_void,
    pub vecLandmarkOffset: vec3_t,
}
