// `pmtrace_s` struct is from generated bindgen in playermove.rs

#![allow(unused, nonstandard_style, deref_nullptr)]

use std::mem::{align_of, offset_of, size_of};
use std::os::raw::*;

use super::pmplane::pmplane_t;

pub const qboolean_false_: qboolean = 0;
pub const qboolean_true_: qboolean = 1;
pub type qboolean = c_uint;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct pmtrace_s {
    pub allsolid: qboolean,
    pub startsolid: qboolean,
    pub inopen: qboolean,
    pub inwater: qboolean,
    pub fraction: f32,
    pub endpos: [f32; 3],
    pub plane: pmplane_t,
    pub ent: c_int,
    pub deltavelocity: [f32; 3],
    pub hitgroup: c_int,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of pmtrace_s"][size_of::<pmtrace_s>() - 68usize];
    ["Alignment of pmtrace_s"][align_of::<pmtrace_s>() - 4usize];
    ["Offset of field: pmtrace_s::allsolid"][offset_of!(pmtrace_s, allsolid) - 0usize];
    ["Offset of field: pmtrace_s::startsolid"][offset_of!(pmtrace_s, startsolid) - 4usize];
    ["Offset of field: pmtrace_s::inopen"][offset_of!(pmtrace_s, inopen) - 8usize];
    ["Offset of field: pmtrace_s::inwater"][offset_of!(pmtrace_s, inwater) - 12usize];
    ["Offset of field: pmtrace_s::fraction"][offset_of!(pmtrace_s, fraction) - 16usize];
    ["Offset of field: pmtrace_s::endpos"][offset_of!(pmtrace_s, endpos) - 20usize];
    ["Offset of field: pmtrace_s::plane"][offset_of!(pmtrace_s, plane) - 32usize];
    ["Offset of field: pmtrace_s::ent"][offset_of!(pmtrace_s, ent) - 48usize];
    ["Offset of field: pmtrace_s::deltavelocity"][offset_of!(pmtrace_s, deltavelocity) - 52usize];
    ["Offset of field: pmtrace_s::hitgroup"][offset_of!(pmtrace_s, hitgroup) - 64usize];
};
