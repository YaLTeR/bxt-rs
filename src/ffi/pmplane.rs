#![allow(unused)]

use std::{
    mem::{align_of, size_of},
    os::raw::*,
    ptr::null,
};

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct pmplane_t {
    pub normal: [f32; 3],
    pub dist: f32,
}

#[test]
fn bindgen_test_layout_pmplane_t() {
    assert_eq!(
        size_of::<pmplane_t>(),
        16usize,
        concat!("Size of: ", stringify!(pmplane_t))
    );
    assert_eq!(
        align_of::<pmplane_t>(),
        4usize,
        concat!("Alignment of ", stringify!(pmplane_t))
    );
    assert_eq!(
        unsafe { &(*(null::<pmplane_t>())).normal as *const _ as usize },
        0usize,
        concat!(
            "Offset of field: ",
            stringify!(pmplane_t),
            "::",
            stringify!(normal)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<pmplane_t>())).dist as *const _ as usize },
        12usize,
        concat!(
            "Offset of field: ",
            stringify!(pmplane_t),
            "::",
            stringify!(dist)
        )
    );
}
