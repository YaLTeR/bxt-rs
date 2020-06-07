#![allow(unused, nonstandard_style)]

use std::{
    mem::{align_of, size_of},
    os::raw::*,
    ptr::null,
};

use crate::ffi::{
    edict, physent::physent_s, pmplane::pmplane_t, pmtrace::pmtrace_s, usercmd::usercmd_s,
};

#[repr(C)]
pub struct playermove_s {
    pub player_index: c_int,
    pub server: u32,
    pub multiplayer: u32,
    pub time: f32,
    pub frametime: f32,
    pub forward: [f32; 3],
    pub right: [f32; 3],
    pub up: [f32; 3],
    pub origin: [f32; 3],
    pub angles: [f32; 3],
    pub oldangles: [f32; 3],
    pub velocity: [f32; 3],
    pub movedir: [f32; 3],
    pub basevelocity: [f32; 3],
    pub view_ofs: [f32; 3],
    pub flDuckTime: f32,
    pub bInDuck: u32,
    pub flTimeStepSound: c_int,
    pub iStepLeft: c_int,
    pub flFallVelocity: f32,
    pub punchangle: [f32; 3],
    pub flSwimTime: f32,
    pub flNextPrimaryAttack: f32,
    pub effects: c_int,
    pub flags: edict::Flags,
    pub usehull: c_int,
    pub gravity: f32,
    pub friction: f32,
    pub oldbuttons: c_int,
    pub waterjumptime: f32,
    pub dead: u32,
    pub deadflag: c_int,
    pub spectator: c_int,
    pub movetype: c_int,
    pub onground: c_int,
    pub waterlevel: c_int,
    pub watertype: c_int,
    pub oldwaterlevel: c_int,
    pub sztexturename: [c_char; 256usize],
    pub chtexturetype: c_char,
    pub maxspeed: f32,
    pub clientmaxspeed: f32,
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
    pub numphysent: c_int,
    pub physents: [physent_s; 600usize],
    pub nummoveent: c_int,
    pub moveents: [physent_s; 64usize],
    pub numvisent: c_int,
    pub visents: [physent_s; 600usize],
    pub cmd: usercmd_s,
    pub numtouch: c_int,
    pub touchindex: [pmtrace_s; 600usize],
    pub physinfo: [c_char; 256usize],
    pub movevars: *mut c_void,
    pub player_mins: [[f32; 3]; 4usize],
    pub player_maxs: [[f32; 3]; 4usize],
    pub PM_Info_ValueForKey:
        Option<unsafe extern "C" fn(s: *const c_char, key: *const c_char) -> *const c_char>,
    pub PM_Particle: Option<
        unsafe extern "C" fn(origin: *mut f32, color: c_int, life: f32, zpos: c_int, zvel: c_int),
    >,
    pub PM_TestPlayerPosition:
        Option<unsafe extern "C" fn(pos: *mut f32, ptrace: *mut pmtrace_s) -> c_int>,
    pub Con_NPrintf: Option<unsafe extern "C" fn(idx: c_int, fmt: *mut c_char, ...)>,
    pub Con_DPrintf: Option<unsafe extern "C" fn(fmt: *mut c_char, ...)>,
    pub Con_Printf: Option<unsafe extern "C" fn(fmt: *mut c_char, ...)>,
    pub Sys_FloatTime: Option<unsafe extern "C" fn() -> f64>,
    pub PM_StuckTouch: Option<unsafe extern "C" fn(hitent: c_int, ptraceresult: *mut pmtrace_s)>,
    pub PM_PointContents:
        Option<unsafe extern "C" fn(p: *mut f32, truecontents: *mut c_int) -> c_int>,
    pub PM_TruePointContents: Option<unsafe extern "C" fn(p: *mut f32) -> c_int>,
    pub PM_HullPointContents:
        Option<unsafe extern "C" fn(hull: *mut c_void, num: c_int, p: *mut f32) -> c_int>,
    pub PM_PlayerTrace: Option<
        unsafe extern "C" fn(
            start: *mut f32,
            end: *mut f32,
            traceFlags: c_int,
            ignore_pe: c_int,
        ) -> pmtrace_s,
    >,
    pub PM_TraceLine: Option<
        unsafe extern "C" fn(
            start: *mut f32,
            end: *mut f32,
            flags: c_int,
            usehulll: c_int,
            ignore_pe: c_int,
        ) -> *mut pmtrace_s,
    >,
    pub RandomLong: Option<unsafe extern "C" fn(lLow: c_int, lHigh: c_int) -> c_int>,
    pub RandomFloat: Option<unsafe extern "C" fn(flLow: f32, flHigh: f32) -> f32>,
    pub PM_GetModelType: Option<unsafe extern "C" fn(mod_: *mut c_void) -> c_int>,
    pub PM_GetModelBounds:
        Option<unsafe extern "C" fn(mod_: *mut c_void, mins: *mut f32, maxs: *mut f32)>,
    pub PM_HullForBsp:
        Option<unsafe extern "C" fn(pe: *mut physent_s, offset: *mut f32) -> *mut c_void>,
    pub PM_TraceModel: Option<
        unsafe extern "C" fn(
            pEnt: *mut physent_s,
            start: *mut f32,
            end: *mut f32,
            trace: *mut c_void,
        ) -> f32,
    >,
    pub COM_FileSize: Option<unsafe extern "C" fn(filename: *mut c_char) -> c_int>,
    pub COM_LoadFile: Option<
        unsafe extern "C" fn(
            path: *mut c_char,
            usehunk: c_int,
            pLength: *mut c_int,
        ) -> *mut c_uchar,
    >,
    pub COM_FreeFile: Option<unsafe extern "C" fn(buffer: *mut c_void)>,
    pub memfgets: Option<
        unsafe extern "C" fn(
            pMemFile: *mut c_uchar,
            fileSize: c_int,
            pFilePos: *mut c_int,
            pBuffer: *mut c_char,
            bufferSize: c_int,
        ) -> *mut c_char,
    >,
    pub runfuncs: u32,
    pub PM_PlaySound: Option<
        unsafe extern "C" fn(
            channel: c_int,
            sample: *const c_char,
            volume: f32,
            attenuation: f32,
            fFlags: c_int,
            pitch: c_int,
        ),
    >,
    pub PM_TraceTexture: Option<
        unsafe extern "C" fn(ground: c_int, vstart: *mut f32, vend: *mut f32) -> *const c_char,
    >,
    pub PM_PlaybackEventFull: Option<
        unsafe extern "C" fn(
            flags: c_int,
            clientindex: c_int,
            eventindex: c_ushort,
            delay: f32,
            origin: *mut f32,
            angles: *mut f32,
            fparam1: f32,
            fparam2: f32,
            iparam1: c_int,
            iparam2: c_int,
            bparam1: c_int,
            bparam2: c_int,
        ),
    >,
    pub PM_PlayerTraceEx: Option<
        unsafe extern "C" fn(
            start: *mut f32,
            end: *mut f32,
            traceFlags: c_int,
            pfnIgnore: Option<unsafe extern "C" fn(pe: *mut physent_s) -> c_int>,
        ) -> pmtrace_s,
    >,
    pub PM_TestPlayerPositionEx: Option<
        unsafe extern "C" fn(
            pos: *mut f32,
            ptrace: *mut pmtrace_s,
            pfnIgnore: Option<unsafe extern "C" fn(pe: *mut physent_s) -> c_int>,
        ) -> c_int,
    >,
    pub PM_TraceLineEx: Option<
        unsafe extern "C" fn(
            start: *mut f32,
            end: *mut f32,
            flags: c_int,
            usehulll: c_int,
            pfnIgnore: Option<unsafe extern "C" fn(pe: *mut physent_s) -> c_int>,
        ) -> *mut pmtrace_s,
    >,
}

#[cfg(target_arch = "x86")]
#[test]
fn bindgen_test_layout_playermove_s() {
    assert_eq!(
        size_of::<playermove_s>(),
        325068usize,
        concat!("Size of: ", stringify!(playermove_s))
    );
    assert_eq!(
        align_of::<playermove_s>(),
        4usize,
        concat!("Alignment of ", stringify!(playermove_s))
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).player_index as *const _ as usize },
        0usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(player_index)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).server as *const _ as usize },
        4usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(server)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).multiplayer as *const _ as usize },
        8usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(multiplayer)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).time as *const _ as usize },
        12usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(time)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).frametime as *const _ as usize },
        16usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(frametime)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).forward as *const _ as usize },
        20usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(forward)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).right as *const _ as usize },
        32usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(right)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).up as *const _ as usize },
        44usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(up)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).origin as *const _ as usize },
        56usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(origin)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).angles as *const _ as usize },
        68usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(angles)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).oldangles as *const _ as usize },
        80usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(oldangles)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).velocity as *const _ as usize },
        92usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(velocity)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).movedir as *const _ as usize },
        104usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(movedir)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).basevelocity as *const _ as usize },
        116usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(basevelocity)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).view_ofs as *const _ as usize },
        128usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(view_ofs)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).flDuckTime as *const _ as usize },
        140usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(flDuckTime)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).bInDuck as *const _ as usize },
        144usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(bInDuck)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).flTimeStepSound as *const _ as usize },
        148usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(flTimeStepSound)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).iStepLeft as *const _ as usize },
        152usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(iStepLeft)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).flFallVelocity as *const _ as usize },
        156usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(flFallVelocity)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).punchangle as *const _ as usize },
        160usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(punchangle)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).flSwimTime as *const _ as usize },
        172usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(flSwimTime)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).flNextPrimaryAttack as *const _ as usize },
        176usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(flNextPrimaryAttack)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).effects as *const _ as usize },
        180usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(effects)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).flags as *const _ as usize },
        184usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(flags)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).usehull as *const _ as usize },
        188usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(usehull)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).gravity as *const _ as usize },
        192usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(gravity)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).friction as *const _ as usize },
        196usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(friction)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).oldbuttons as *const _ as usize },
        200usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(oldbuttons)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).waterjumptime as *const _ as usize },
        204usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(waterjumptime)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).dead as *const _ as usize },
        208usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(dead)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).deadflag as *const _ as usize },
        212usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(deadflag)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).spectator as *const _ as usize },
        216usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(spectator)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).movetype as *const _ as usize },
        220usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(movetype)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).onground as *const _ as usize },
        224usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(onground)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).waterlevel as *const _ as usize },
        228usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(waterlevel)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).watertype as *const _ as usize },
        232usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(watertype)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).oldwaterlevel as *const _ as usize },
        236usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(oldwaterlevel)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).sztexturename as *const _ as usize },
        240usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(sztexturename)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).chtexturetype as *const _ as usize },
        496usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(chtexturetype)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).maxspeed as *const _ as usize },
        500usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(maxspeed)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).clientmaxspeed as *const _ as usize },
        504usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(clientmaxspeed)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).iuser1 as *const _ as usize },
        508usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(iuser1)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).iuser2 as *const _ as usize },
        512usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(iuser2)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).iuser3 as *const _ as usize },
        516usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(iuser3)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).iuser4 as *const _ as usize },
        520usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(iuser4)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).fuser1 as *const _ as usize },
        524usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(fuser1)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).fuser2 as *const _ as usize },
        528usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(fuser2)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).fuser3 as *const _ as usize },
        532usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(fuser3)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).fuser4 as *const _ as usize },
        536usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(fuser4)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).vuser1 as *const _ as usize },
        540usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(vuser1)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).vuser2 as *const _ as usize },
        552usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(vuser2)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).vuser3 as *const _ as usize },
        564usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(vuser3)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).vuser4 as *const _ as usize },
        576usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(vuser4)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).numphysent as *const _ as usize },
        588usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(numphysent)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).physents as *const _ as usize },
        592usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(physents)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).nummoveent as *const _ as usize },
        134992usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(nummoveent)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).moveents as *const _ as usize },
        134996usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(moveents)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).numvisent as *const _ as usize },
        149332usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(numvisent)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).visents as *const _ as usize },
        149336usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(visents)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).cmd as *const _ as usize },
        283736usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(cmd)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).numtouch as *const _ as usize },
        283788usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(numtouch)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).touchindex as *const _ as usize },
        283792usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(touchindex)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).physinfo as *const _ as usize },
        324592usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(physinfo)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).movevars as *const _ as usize },
        324848usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(movevars)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).player_mins as *const _ as usize },
        324852usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(player_mins)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).player_maxs as *const _ as usize },
        324900usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(player_maxs)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).PM_Info_ValueForKey as *const _ as usize },
        324948usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(PM_Info_ValueForKey)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).PM_Particle as *const _ as usize },
        324952usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(PM_Particle)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).PM_TestPlayerPosition as *const _ as usize },
        324956usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(PM_TestPlayerPosition)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).Con_NPrintf as *const _ as usize },
        324960usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(Con_NPrintf)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).Con_DPrintf as *const _ as usize },
        324964usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(Con_DPrintf)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).Con_Printf as *const _ as usize },
        324968usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(Con_Printf)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).Sys_FloatTime as *const _ as usize },
        324972usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(Sys_FloatTime)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).PM_StuckTouch as *const _ as usize },
        324976usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(PM_StuckTouch)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).PM_PointContents as *const _ as usize },
        324980usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(PM_PointContents)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).PM_TruePointContents as *const _ as usize },
        324984usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(PM_TruePointContents)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).PM_HullPointContents as *const _ as usize },
        324988usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(PM_HullPointContents)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).PM_PlayerTrace as *const _ as usize },
        324992usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(PM_PlayerTrace)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).PM_TraceLine as *const _ as usize },
        324996usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(PM_TraceLine)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).RandomLong as *const _ as usize },
        325000usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(RandomLong)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).RandomFloat as *const _ as usize },
        325004usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(RandomFloat)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).PM_GetModelType as *const _ as usize },
        325008usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(PM_GetModelType)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).PM_GetModelBounds as *const _ as usize },
        325012usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(PM_GetModelBounds)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).PM_HullForBsp as *const _ as usize },
        325016usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(PM_HullForBsp)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).PM_TraceModel as *const _ as usize },
        325020usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(PM_TraceModel)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).COM_FileSize as *const _ as usize },
        325024usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(COM_FileSize)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).COM_LoadFile as *const _ as usize },
        325028usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(COM_LoadFile)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).COM_FreeFile as *const _ as usize },
        325032usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(COM_FreeFile)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).memfgets as *const _ as usize },
        325036usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(memfgets)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).runfuncs as *const _ as usize },
        325040usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(runfuncs)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).PM_PlaySound as *const _ as usize },
        325044usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(PM_PlaySound)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).PM_TraceTexture as *const _ as usize },
        325048usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(PM_TraceTexture)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).PM_PlaybackEventFull as *const _ as usize },
        325052usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(PM_PlaybackEventFull)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).PM_PlayerTraceEx as *const _ as usize },
        325056usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(PM_PlayerTraceEx)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).PM_TestPlayerPositionEx as *const _ as usize },
        325060usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(PM_TestPlayerPositionEx)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<playermove_s>())).PM_TraceLineEx as *const _ as usize },
        325064usize,
        concat!(
            "Offset of field: ",
            stringify!(playermove_s),
            "::",
            stringify!(PM_TraceLineEx)
        )
    );
}
