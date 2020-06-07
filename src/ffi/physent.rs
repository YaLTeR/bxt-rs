#![allow(unused)]

use std::{
    mem::{align_of, size_of},
    os::raw::*,
    ptr::null,
};

#[repr(C)]
#[derive(Debug)]
pub struct physent_s {
    pub name: [c_char; 32usize],
    pub player: c_int,
    pub origin: [f32; 3],
    pub model: *mut c_void,
    pub studiomodel: *mut c_void,
    pub mins: [f32; 3],
    pub maxs: [f32; 3],
    pub info: c_int,
    pub angles: [f32; 3],
    pub solid: c_int,
    pub skin: c_int,
    pub rendermode: c_int,
    pub frame: f32,
    pub sequence: c_int,
    pub controller: [c_uchar; 4usize],
    pub blending: [c_uchar; 2usize],
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

#[cfg(target_arch = "x86")]
#[test]
fn bindgen_test_layout_physent_s() {
    assert_eq!(
        size_of::<physent_s>(),
        224usize,
        concat!("Size of: ", stringify!(physent_s))
    );
    assert_eq!(
        align_of::<physent_s>(),
        4usize,
        concat!("Alignment of ", stringify!(physent_s))
    );
    assert_eq!(
        unsafe { &(*(null::<physent_s>())).name as *const _ as usize },
        0usize,
        concat!(
            "Offset of field: ",
            stringify!(physent_s),
            "::",
            stringify!(name)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<physent_s>())).player as *const _ as usize },
        32usize,
        concat!(
            "Offset of field: ",
            stringify!(physent_s),
            "::",
            stringify!(player)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<physent_s>())).origin as *const _ as usize },
        36usize,
        concat!(
            "Offset of field: ",
            stringify!(physent_s),
            "::",
            stringify!(origin)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<physent_s>())).model as *const _ as usize },
        48usize,
        concat!(
            "Offset of field: ",
            stringify!(physent_s),
            "::",
            stringify!(model)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<physent_s>())).studiomodel as *const _ as usize },
        52usize,
        concat!(
            "Offset of field: ",
            stringify!(physent_s),
            "::",
            stringify!(studiomodel)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<physent_s>())).mins as *const _ as usize },
        56usize,
        concat!(
            "Offset of field: ",
            stringify!(physent_s),
            "::",
            stringify!(mins)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<physent_s>())).maxs as *const _ as usize },
        68usize,
        concat!(
            "Offset of field: ",
            stringify!(physent_s),
            "::",
            stringify!(maxs)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<physent_s>())).info as *const _ as usize },
        80usize,
        concat!(
            "Offset of field: ",
            stringify!(physent_s),
            "::",
            stringify!(info)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<physent_s>())).angles as *const _ as usize },
        84usize,
        concat!(
            "Offset of field: ",
            stringify!(physent_s),
            "::",
            stringify!(angles)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<physent_s>())).solid as *const _ as usize },
        96usize,
        concat!(
            "Offset of field: ",
            stringify!(physent_s),
            "::",
            stringify!(solid)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<physent_s>())).skin as *const _ as usize },
        100usize,
        concat!(
            "Offset of field: ",
            stringify!(physent_s),
            "::",
            stringify!(skin)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<physent_s>())).rendermode as *const _ as usize },
        104usize,
        concat!(
            "Offset of field: ",
            stringify!(physent_s),
            "::",
            stringify!(rendermode)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<physent_s>())).frame as *const _ as usize },
        108usize,
        concat!(
            "Offset of field: ",
            stringify!(physent_s),
            "::",
            stringify!(frame)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<physent_s>())).sequence as *const _ as usize },
        112usize,
        concat!(
            "Offset of field: ",
            stringify!(physent_s),
            "::",
            stringify!(sequence)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<physent_s>())).controller as *const _ as usize },
        116usize,
        concat!(
            "Offset of field: ",
            stringify!(physent_s),
            "::",
            stringify!(controller)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<physent_s>())).blending as *const _ as usize },
        120usize,
        concat!(
            "Offset of field: ",
            stringify!(physent_s),
            "::",
            stringify!(blending)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<physent_s>())).movetype as *const _ as usize },
        124usize,
        concat!(
            "Offset of field: ",
            stringify!(physent_s),
            "::",
            stringify!(movetype)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<physent_s>())).takedamage as *const _ as usize },
        128usize,
        concat!(
            "Offset of field: ",
            stringify!(physent_s),
            "::",
            stringify!(takedamage)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<physent_s>())).blooddecal as *const _ as usize },
        132usize,
        concat!(
            "Offset of field: ",
            stringify!(physent_s),
            "::",
            stringify!(blooddecal)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<physent_s>())).team as *const _ as usize },
        136usize,
        concat!(
            "Offset of field: ",
            stringify!(physent_s),
            "::",
            stringify!(team)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<physent_s>())).classnumber as *const _ as usize },
        140usize,
        concat!(
            "Offset of field: ",
            stringify!(physent_s),
            "::",
            stringify!(classnumber)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<physent_s>())).iuser1 as *const _ as usize },
        144usize,
        concat!(
            "Offset of field: ",
            stringify!(physent_s),
            "::",
            stringify!(iuser1)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<physent_s>())).iuser2 as *const _ as usize },
        148usize,
        concat!(
            "Offset of field: ",
            stringify!(physent_s),
            "::",
            stringify!(iuser2)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<physent_s>())).iuser3 as *const _ as usize },
        152usize,
        concat!(
            "Offset of field: ",
            stringify!(physent_s),
            "::",
            stringify!(iuser3)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<physent_s>())).iuser4 as *const _ as usize },
        156usize,
        concat!(
            "Offset of field: ",
            stringify!(physent_s),
            "::",
            stringify!(iuser4)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<physent_s>())).fuser1 as *const _ as usize },
        160usize,
        concat!(
            "Offset of field: ",
            stringify!(physent_s),
            "::",
            stringify!(fuser1)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<physent_s>())).fuser2 as *const _ as usize },
        164usize,
        concat!(
            "Offset of field: ",
            stringify!(physent_s),
            "::",
            stringify!(fuser2)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<physent_s>())).fuser3 as *const _ as usize },
        168usize,
        concat!(
            "Offset of field: ",
            stringify!(physent_s),
            "::",
            stringify!(fuser3)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<physent_s>())).fuser4 as *const _ as usize },
        172usize,
        concat!(
            "Offset of field: ",
            stringify!(physent_s),
            "::",
            stringify!(fuser4)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<physent_s>())).vuser1 as *const _ as usize },
        176usize,
        concat!(
            "Offset of field: ",
            stringify!(physent_s),
            "::",
            stringify!(vuser1)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<physent_s>())).vuser2 as *const _ as usize },
        188usize,
        concat!(
            "Offset of field: ",
            stringify!(physent_s),
            "::",
            stringify!(vuser2)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<physent_s>())).vuser3 as *const _ as usize },
        200usize,
        concat!(
            "Offset of field: ",
            stringify!(physent_s),
            "::",
            stringify!(vuser3)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<physent_s>())).vuser4 as *const _ as usize },
        212usize,
        concat!(
            "Offset of field: ",
            stringify!(physent_s),
            "::",
            stringify!(vuser4)
        )
    );
}
