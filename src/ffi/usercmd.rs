#![allow(unused, deref_nullptr)]

use std::mem::{align_of, size_of};
use std::os::raw::*;
use std::ptr::null;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct usercmd_s {
    pub lerp_msec: c_short,
    pub msec: c_uchar,
    pub viewangles: [f32; 3usize],
    pub forwardmove: f32,
    pub sidemove: f32,
    pub upmove: f32,
    pub lightlevel: c_uchar,
    pub buttons: c_ushort,
    pub impulse: c_uchar,
    pub weaponselect: c_uchar,
    pub impact_index: c_int,
    pub impact_position: [f32; 3usize],
}

#[test]
fn bindgen_test_layout_usercmd_s() {
    assert_eq!(
        size_of::<usercmd_s>(),
        52usize,
        concat!("Size of: ", stringify!(usercmd_s))
    );
    assert_eq!(
        align_of::<usercmd_s>(),
        4usize,
        concat!("Alignment of ", stringify!(usercmd_s))
    );
    assert_eq!(
        unsafe { &(*(null::<usercmd_s>())).lerp_msec as *const _ as usize },
        0usize,
        concat!(
            "Offset of field: ",
            stringify!(usercmd_s),
            "::",
            stringify!(lerp_msec)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<usercmd_s>())).msec as *const _ as usize },
        2usize,
        concat!(
            "Offset of field: ",
            stringify!(usercmd_s),
            "::",
            stringify!(msec)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<usercmd_s>())).viewangles as *const _ as usize },
        4usize,
        concat!(
            "Offset of field: ",
            stringify!(usercmd_s),
            "::",
            stringify!(viewangles)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<usercmd_s>())).forwardmove as *const _ as usize },
        16usize,
        concat!(
            "Offset of field: ",
            stringify!(usercmd_s),
            "::",
            stringify!(forwardmove)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<usercmd_s>())).sidemove as *const _ as usize },
        20usize,
        concat!(
            "Offset of field: ",
            stringify!(usercmd_s),
            "::",
            stringify!(sidemove)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<usercmd_s>())).upmove as *const _ as usize },
        24usize,
        concat!(
            "Offset of field: ",
            stringify!(usercmd_s),
            "::",
            stringify!(upmove)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<usercmd_s>())).lightlevel as *const _ as usize },
        28usize,
        concat!(
            "Offset of field: ",
            stringify!(usercmd_s),
            "::",
            stringify!(lightlevel)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<usercmd_s>())).buttons as *const _ as usize },
        30usize,
        concat!(
            "Offset of field: ",
            stringify!(usercmd_s),
            "::",
            stringify!(buttons)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<usercmd_s>())).impulse as *const _ as usize },
        32usize,
        concat!(
            "Offset of field: ",
            stringify!(usercmd_s),
            "::",
            stringify!(impulse)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<usercmd_s>())).weaponselect as *const _ as usize },
        33usize,
        concat!(
            "Offset of field: ",
            stringify!(usercmd_s),
            "::",
            stringify!(weaponselect)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<usercmd_s>())).impact_index as *const _ as usize },
        36usize,
        concat!(
            "Offset of field: ",
            stringify!(usercmd_s),
            "::",
            stringify!(impact_index)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<usercmd_s>())).impact_position as *const _ as usize },
        40usize,
        concat!(
            "Offset of field: ",
            stringify!(usercmd_s),
            "::",
            stringify!(impact_position)
        )
    );
}
