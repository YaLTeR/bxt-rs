// bindgen halflife/common/triangleapi.h --allowlist-type "triangleapi_s" --
// --target=i686-unknown-linux-gnu -Ihalflife/{public,common,engine} -include mathlib.h -include
// const.h
// Keep everything before the generated part
// The constants are `c_int` for code compatibility. Otherwise, the bindgen will generate `c_uint`
// Add `--allowlist-var 'TRI_.*'` to generate the constants
// Change `triangleapi_s` members who are functions from `Option<T>` to `T`
// Remove the unnecessary qualifiers `::std::os::raw::` and `::std::mem::`
// Remove `model_s` dummy struct and import `model_s` from com_model.rs
// Change all `*mut ...` type arguments inside triangleapi_s struct members to `*const ...`

#![allow(unused, nonstandard_style, deref_nullptr, clippy::upper_case_acronyms)]

use std::mem::{align_of, offset_of, size_of};
use std::os::raw::*;
use std::ptr::null;

use super::com_model::model_s;

pub const TRICULLSTYLE_TRI_FRONT: TRICULLSTYLE = 0;
pub const TRICULLSTYLE_TRI_NONE: TRICULLSTYLE = 1;
pub type TRICULLSTYLE = c_int;

pub const TRI_API_VERSION: c_int = 1;

pub const TRI_TRIANGLES: c_int = 0;
pub const TRI_TRIANGLE_FAN: c_int = 1;
pub const TRI_QUADS: c_int = 2;
pub const TRI_POLYGON: c_int = 3;
pub const TRI_LINES: c_int = 4;
pub const TRI_TRIANGLE_STRIP: c_int = 5;
pub const TRI_QUAD_STRIP: c_int = 6;

pub const kRenderNormal: c_int = 0;
pub const kRenderTransColor: c_int = 1;
pub const kRenderTransTexture: c_int = 2;
pub const kRenderGlow: c_int = 3;
pub const kRenderTransAlpha: c_int = 4;
pub const kRenderTransAdd: c_int = 5;

/* automatically generated by rust-bindgen 0.71.1 */

// intentionally commented out
// pub const TRICULLSTYLE_TRI_FRONT: TRICULLSTYLE = 0;
// pub const TRICULLSTYLE_TRI_NONE: TRICULLSTYLE = 1;
// pub type TRICULLSTYLE = c_uint;

#[repr(C)]
#[derive(Debug)]
pub struct triangleapi_s {
    pub version: c_int,
    pub RenderMode: unsafe extern "C" fn(mode: c_int),
    pub Begin: unsafe extern "C" fn(primitiveCode: c_int),
    pub End: unsafe extern "C" fn(),
    pub Color4f: unsafe extern "C" fn(r: f32, g: f32, b: f32, a: f32),
    pub Color4ub: unsafe extern "C" fn(r: c_uchar, g: c_uchar, b: c_uchar, a: c_uchar),
    pub TexCoord2f: unsafe extern "C" fn(u: f32, v: f32),
    pub Vertex3fv: unsafe extern "C" fn(worldPnt: *const f32),
    pub Vertex3f: unsafe extern "C" fn(x: f32, y: f32, z: f32),
    pub Brightness: unsafe extern "C" fn(brightness: f32),
    pub CullFace: unsafe extern "C" fn(style: TRICULLSTYLE),
    pub SpriteTexture: unsafe extern "C" fn(pSpriteModel: *const model_s, frame: c_int) -> c_int,
    pub WorldToScreen: unsafe extern "C" fn(world: *const f32, screen: *const f32) -> c_int,
    pub Fog: unsafe extern "C" fn(flFogColor: *mut f32, flStart: f32, flEnd: f32, bOn: c_int),
    pub ScreenToWorld: unsafe extern "C" fn(screen: *const f32, world: *const f32),
    pub GetMatrix: unsafe extern "C" fn(pname: c_int, matrix: *const f32),
    pub BoxInPVS: unsafe extern "C" fn(mins: *const f32, maxs: *const f32) -> c_int,
    pub LightAtPoint: unsafe extern "C" fn(pos: *const f32, value: *const f32),
    pub Color4fRendermode: unsafe extern "C" fn(r: f32, g: f32, b: f32, a: f32, rendermode: c_int),
    pub FogParams: unsafe extern "C" fn(flDensity: f32, iFogSkybox: c_int),
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of triangleapi_s"][size_of::<triangleapi_s>() - 80usize];
    ["Alignment of triangleapi_s"][align_of::<triangleapi_s>() - 4usize];
    ["Offset of field: triangleapi_s::version"][offset_of!(triangleapi_s, version) - 0usize];
    ["Offset of field: triangleapi_s::RenderMode"][offset_of!(triangleapi_s, RenderMode) - 4usize];
    ["Offset of field: triangleapi_s::Begin"][offset_of!(triangleapi_s, Begin) - 8usize];
    ["Offset of field: triangleapi_s::End"][offset_of!(triangleapi_s, End) - 12usize];
    ["Offset of field: triangleapi_s::Color4f"][offset_of!(triangleapi_s, Color4f) - 16usize];
    ["Offset of field: triangleapi_s::Color4ub"][offset_of!(triangleapi_s, Color4ub) - 20usize];
    ["Offset of field: triangleapi_s::TexCoord2f"][offset_of!(triangleapi_s, TexCoord2f) - 24usize];
    ["Offset of field: triangleapi_s::Vertex3fv"][offset_of!(triangleapi_s, Vertex3fv) - 28usize];
    ["Offset of field: triangleapi_s::Vertex3f"][offset_of!(triangleapi_s, Vertex3f) - 32usize];
    ["Offset of field: triangleapi_s::Brightness"][offset_of!(triangleapi_s, Brightness) - 36usize];
    ["Offset of field: triangleapi_s::CullFace"][offset_of!(triangleapi_s, CullFace) - 40usize];
    ["Offset of field: triangleapi_s::SpriteTexture"]
        [offset_of!(triangleapi_s, SpriteTexture) - 44usize];
    ["Offset of field: triangleapi_s::WorldToScreen"]
        [offset_of!(triangleapi_s, WorldToScreen) - 48usize];
    ["Offset of field: triangleapi_s::Fog"][offset_of!(triangleapi_s, Fog) - 52usize];
    ["Offset of field: triangleapi_s::ScreenToWorld"]
        [offset_of!(triangleapi_s, ScreenToWorld) - 56usize];
    ["Offset of field: triangleapi_s::GetMatrix"][offset_of!(triangleapi_s, GetMatrix) - 60usize];
    ["Offset of field: triangleapi_s::BoxInPVS"][offset_of!(triangleapi_s, BoxInPVS) - 64usize];
    ["Offset of field: triangleapi_s::LightAtPoint"]
        [offset_of!(triangleapi_s, LightAtPoint) - 68usize];
    ["Offset of field: triangleapi_s::Color4fRendermode"]
        [offset_of!(triangleapi_s, Color4fRendermode) - 72usize];
    ["Offset of field: triangleapi_s::FogParams"][offset_of!(triangleapi_s, FogParams) - 76usize];
};
