// `physent_s` struct is from generated bindgen in playermove.rs

#![allow(unused, nonstandard_style, deref_nullptr)]

use std::mem::{align_of, offset_of, size_of};
use std::os::raw::*;

use super::com_model::model_s;

pub type string_t = c_uint;
pub type byte = c_uchar;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct physent_s {
    pub name: [c_char; 32usize],
    pub player: c_int,
    pub origin: [f32; 3],
    pub model: *mut model_s,
    pub studiomodel: *mut model_s,
    pub mins: [f32; 3],
    pub maxs: [f32; 3],
    pub info: c_int,
    pub angles: [f32; 3],
    pub solid: c_int,
    pub skin: c_int,
    pub rendermode: c_int,
    pub frame: f32,
    pub sequence: c_int,
    pub controller: [byte; 4usize],
    pub blending: [byte; 2usize],
    pub movetype: c_int,
    pub takedamage: c_int,
    pub blooddecal: c_int,
    pub team: c_int,
    pub classnumber: c_int,
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
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of physent_s"][size_of::<physent_s>() - 224usize];
    ["Alignment of physent_s"][align_of::<physent_s>() - 4usize];
    ["Offset of field: physent_s::name"][offset_of!(physent_s, name) - 0usize];
    ["Offset of field: physent_s::player"][offset_of!(physent_s, player) - 32usize];
    ["Offset of field: physent_s::origin"][offset_of!(physent_s, origin) - 36usize];
    ["Offset of field: physent_s::model"][offset_of!(physent_s, model) - 48usize];
    ["Offset of field: physent_s::studiomodel"][offset_of!(physent_s, studiomodel) - 52usize];
    ["Offset of field: physent_s::mins"][offset_of!(physent_s, mins) - 56usize];
    ["Offset of field: physent_s::maxs"][offset_of!(physent_s, maxs) - 68usize];
    ["Offset of field: physent_s::info"][offset_of!(physent_s, info) - 80usize];
    ["Offset of field: physent_s::angles"][offset_of!(physent_s, angles) - 84usize];
    ["Offset of field: physent_s::solid"][offset_of!(physent_s, solid) - 96usize];
    ["Offset of field: physent_s::skin"][offset_of!(physent_s, skin) - 100usize];
    ["Offset of field: physent_s::rendermode"][offset_of!(physent_s, rendermode) - 104usize];
    ["Offset of field: physent_s::frame"][offset_of!(physent_s, frame) - 108usize];
    ["Offset of field: physent_s::sequence"][offset_of!(physent_s, sequence) - 112usize];
    ["Offset of field: physent_s::controller"][offset_of!(physent_s, controller) - 116usize];
    ["Offset of field: physent_s::blending"][offset_of!(physent_s, blending) - 120usize];
    ["Offset of field: physent_s::movetype"][offset_of!(physent_s, movetype) - 124usize];
    ["Offset of field: physent_s::takedamage"][offset_of!(physent_s, takedamage) - 128usize];
    ["Offset of field: physent_s::blooddecal"][offset_of!(physent_s, blooddecal) - 132usize];
    ["Offset of field: physent_s::team"][offset_of!(physent_s, team) - 136usize];
    ["Offset of field: physent_s::classnumber"][offset_of!(physent_s, classnumber) - 140usize];
    ["Offset of field: physent_s::iuser1"][offset_of!(physent_s, iuser1) - 144usize];
    ["Offset of field: physent_s::iuser2"][offset_of!(physent_s, iuser2) - 148usize];
    ["Offset of field: physent_s::iuser3"][offset_of!(physent_s, iuser3) - 152usize];
    ["Offset of field: physent_s::iuser4"][offset_of!(physent_s, iuser4) - 156usize];
    ["Offset of field: physent_s::fuser1"][offset_of!(physent_s, fuser1) - 160usize];
    ["Offset of field: physent_s::fuser2"][offset_of!(physent_s, fuser2) - 164usize];
    ["Offset of field: physent_s::fuser3"][offset_of!(physent_s, fuser3) - 168usize];
    ["Offset of field: physent_s::fuser4"][offset_of!(physent_s, fuser4) - 172usize];
    ["Offset of field: physent_s::vuser1"][offset_of!(physent_s, vuser1) - 176usize];
    ["Offset of field: physent_s::vuser2"][offset_of!(physent_s, vuser2) - 188usize];
    ["Offset of field: physent_s::vuser3"][offset_of!(physent_s, vuser3) - 200usize];
    ["Offset of field: physent_s::vuser4"][offset_of!(physent_s, vuser4) - 212usize];
};
