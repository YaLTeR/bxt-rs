#![allow(unused)]

use std::{
    mem::{align_of, size_of},
    os::raw::*,
    ptr::null,
};

use crate::ffi::pmplane::pmplane_t;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct pmtrace_s {
    pub allsolid: u32,
    pub startsolid: u32,
    pub inopen: u32,
    pub inwater: u32,
    pub fraction: f32,
    pub endpos: [f32; 3],
    pub plane: pmplane_t,
    pub ent: c_int,
    pub deltavelocity: [f32; 3],
    pub hitgroup: c_int,
}

#[test]
fn bindgen_test_layout_pmtrace_s() {
    assert_eq!(
        size_of::<pmtrace_s>(),
        68usize,
        concat!("Size of: ", stringify!(pmtrace_s))
    );
    assert_eq!(
        align_of::<pmtrace_s>(),
        4usize,
        concat!("Alignment of ", stringify!(pmtrace_s))
    );
    assert_eq!(
        unsafe { &(*(null::<pmtrace_s>())).allsolid as *const _ as usize },
        0usize,
        concat!(
            "Offset of field: ",
            stringify!(pmtrace_s),
            "::",
            stringify!(allsolid)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<pmtrace_s>())).startsolid as *const _ as usize },
        4usize,
        concat!(
            "Offset of field: ",
            stringify!(pmtrace_s),
            "::",
            stringify!(startsolid)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<pmtrace_s>())).inopen as *const _ as usize },
        8usize,
        concat!(
            "Offset of field: ",
            stringify!(pmtrace_s),
            "::",
            stringify!(inopen)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<pmtrace_s>())).inwater as *const _ as usize },
        12usize,
        concat!(
            "Offset of field: ",
            stringify!(pmtrace_s),
            "::",
            stringify!(inwater)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<pmtrace_s>())).fraction as *const _ as usize },
        16usize,
        concat!(
            "Offset of field: ",
            stringify!(pmtrace_s),
            "::",
            stringify!(fraction)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<pmtrace_s>())).endpos as *const _ as usize },
        20usize,
        concat!(
            "Offset of field: ",
            stringify!(pmtrace_s),
            "::",
            stringify!(endpos)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<pmtrace_s>())).plane as *const _ as usize },
        32usize,
        concat!(
            "Offset of field: ",
            stringify!(pmtrace_s),
            "::",
            stringify!(plane)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<pmtrace_s>())).ent as *const _ as usize },
        48usize,
        concat!(
            "Offset of field: ",
            stringify!(pmtrace_s),
            "::",
            stringify!(ent)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<pmtrace_s>())).deltavelocity as *const _ as usize },
        52usize,
        concat!(
            "Offset of field: ",
            stringify!(pmtrace_s),
            "::",
            stringify!(deltavelocity)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<pmtrace_s>())).hitgroup as *const _ as usize },
        64usize,
        concat!(
            "Offset of field: ",
            stringify!(pmtrace_s),
            "::",
            stringify!(hitgroup)
        )
    );
}
