// `pmplane_t` struct is from generated bindgen in playermove.rs

#![allow(unused, nonstandard_style, deref_nullptr)]

use std::mem::{align_of, offset_of, size_of};
use std::os::raw::*;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct pmplane_t {
    pub normal: [f32; 3],
    pub dist: f32,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of pmplane_t"][size_of::<pmplane_t>() - 16usize];
    ["Alignment of pmplane_t"][align_of::<pmplane_t>() - 4usize];
    ["Offset of field: pmplane_t::normal"][offset_of!(pmplane_t, normal) - 0usize];
    ["Offset of field: pmplane_t::dist"][offset_of!(pmplane_t, dist) - 12usize];
};
