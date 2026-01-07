//! ""Manually"" done.
//!
//! There are no bindings from hlsdk.

#![allow(unused, nonstandard_style, deref_nullptr)]

use std::os::raw::*;

use crate::ffi::com_model::model_s;
use crate::ffi::edict::edict_s;

type CRC32_t = u32;
// TODO make it a rust enum
type server_state_t = i32;
type byte = c_uchar;
pub type qboolean = c_int;

#[repr(C)]
pub struct sizebuf_s {
    pub buffername: *mut c_char,
    pub flags: u16,
    pub data: *mut byte,
    pub maxsize: i32,
    pub cursize: i32,
}

type resourectype_t = i32;

#[repr(C)]
pub struct resource_s {
    pub szFileName: [c_char; 64],
    pub type_: resourectype_t,
    pub nIndex: c_int,
    pub nDownloadSize: c_int,
    pub ucFlags: c_uchar,
    pub rgucMD5_hash: [c_uchar; 16],
    pub playernum: c_uchar,
    pub rguc_reserved: [c_uchar; 32],
    pub pNext: *mut resource_s,
    pub pPrev: *mut resource_s,
}

#[repr(C)]
pub struct consistency_s {
    pub filename: *mut c_char,
    pub issound: c_int,
    pub orig_index: c_int,
    pub value: c_int,
    pub check_type: c_int,
    pub mins: [c_float; 3],
    pub maxs: [c_float; 3],
}

#[repr(C)]
pub struct event_s {
    pub index: c_ushort,
    pub filename: *mut c_char,
    pub filesize: c_int,
    pub pszScript: *mut c_char,
}

#[repr(C)]
pub struct server_t {
    pub active: qboolean,
    pub paused: qboolean,
    pub loadgame: qboolean,
    pub time: c_double,
    pub oldtime: c_double,
    pub lastcheck: c_int,
    pub lastchecktime: c_double,
    pub name: [c_char; 64],
    pub oldname: [c_char; 64],
    pub startspot: [c_char; 64],
    pub modelname: [c_char; 64],
    pub worldmodel: *mut model_s,
    pub worldmapCRC: CRC32_t,
    pub clientdllmd5: [c_uchar; 16],
    pub resourcelist: [resource_s; 1280],
    pub num_resources: c_int,
    pub consistency_list: [consistency_s; 512],
    pub num_consistency: c_int,
    pub model_precache: [*mut c_char; 512],
    pub models: [*mut model_s; 512],
    pub model_precache_flags: [c_uchar; 512],
    pub event_precache: [event_s; 256],
    pub sound_precache: [*mut c_char; 512],
    pub sound_precache_hashedlookup: [c_short; 1023],
    pub sound_precache_hashedlookup_built: qboolean,
    pub generic_precache: [*mut c_char; 512],
    pub generic_precache_names: [[c_char; 64]; 512],
    pub num_generic_names: c_int,
    pub lightstyles: [*mut c_char; 64],
    pub num_edicts: c_int,
    pub max_edicts: c_int,
    pub edicts: *mut edict_s,
    pub baselines: *mut c_void,          // entity_state_s
    pub instance_baselines: *mut c_void, // extra_baselines_s
    pub state: server_state_t,
    pub datagram: sizebuf_s,
    pub datagram_buf: [c_uchar; 4000],
    pub reliable_datagram: sizebuf_s,
    pub reliable_datagram_buf: [c_uchar; 4000],
    pub multicast: sizebuf_s,
    pub multicast_buf: [c_uchar; 1024],
    pub spectator: sizebuf_s,
    pub spectator_buf: [c_uchar; 1024],
    pub signon: sizebuf_s,
    pub signon_data: [c_uchar; 32768],
}
