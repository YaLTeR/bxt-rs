// `usercmd_s` struct is from generated bindgen in playermove.rs

#![allow(unused, nonstandard_style, deref_nullptr)]

use std::mem::{align_of, offset_of, size_of};
use std::os::raw::*;

pub type byte = c_uchar;

// this type is expected to have Clone and Copy because of
// `my_CmdStart` in server.rs
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct usercmd_s {
    pub lerp_msec: c_short,
    pub msec: byte,
    pub viewangles: [f32; 3],
    pub forwardmove: f32,
    pub sidemove: f32,
    pub upmove: f32,
    pub lightlevel: byte,
    pub buttons: c_ushort,
    pub impulse: byte,
    pub weaponselect: byte,
    pub impact_index: c_int,
    pub impact_position: [f32; 3],
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of usercmd_s"][size_of::<usercmd_s>() - 52usize];
    ["Alignment of usercmd_s"][align_of::<usercmd_s>() - 4usize];
    ["Offset of field: usercmd_s::lerp_msec"][offset_of!(usercmd_s, lerp_msec) - 0usize];
    ["Offset of field: usercmd_s::msec"][offset_of!(usercmd_s, msec) - 2usize];
    ["Offset of field: usercmd_s::viewangles"][offset_of!(usercmd_s, viewangles) - 4usize];
    ["Offset of field: usercmd_s::forwardmove"][offset_of!(usercmd_s, forwardmove) - 16usize];
    ["Offset of field: usercmd_s::sidemove"][offset_of!(usercmd_s, sidemove) - 20usize];
    ["Offset of field: usercmd_s::upmove"][offset_of!(usercmd_s, upmove) - 24usize];
    ["Offset of field: usercmd_s::lightlevel"][offset_of!(usercmd_s, lightlevel) - 28usize];
    ["Offset of field: usercmd_s::buttons"][offset_of!(usercmd_s, buttons) - 30usize];
    ["Offset of field: usercmd_s::impulse"][offset_of!(usercmd_s, impulse) - 32usize];
    ["Offset of field: usercmd_s::weaponselect"][offset_of!(usercmd_s, weaponselect) - 33usize];
    ["Offset of field: usercmd_s::impact_index"][offset_of!(usercmd_s, impact_index) - 36usize];
    ["Offset of field: usercmd_s::impact_position"]
        [offset_of!(usercmd_s, impact_position) - 40usize];
};
