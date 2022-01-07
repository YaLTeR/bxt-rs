#![allow(unused, nonstandard_style, deref_nullptr, clippy::upper_case_acronyms)]

use std::mem::{align_of, size_of};
use std::os::raw::*;
use std::ptr::null;

use bitflags::bitflags;

bitflags! {
    pub struct Flags: i32 {
        const FL_FLY = 1;
        const FL_SWIM = 1 << 1;
        const FL_CONVEYOR = 1 << 2;
        const FL_CLIENT = 1 << 3;
        const FL_INWATER = 1 << 4;
        const FL_MONSTER = 1 << 5;
        const FL_GODMODE = 1 << 6;
        const FL_NOTARGET = 1 << 7;
        const FL_SKIPLOCALHOST = 1 << 8;
        const FL_ONGROUND = 1 << 9;
        const FL_PARTIALGROUND = 1 << 10;
        const FL_WATERJUMP = 1 << 11;
        const FL_FROZEN = 1 << 12;
        const FL_FAKECLIENT = 1 << 13;
        const FL_DUCKING = 1 << 14;
        const FL_FLOAT = 1 << 15;
        const FL_GRAPHED = 1 << 16;
        const FL_IMMUNE_WATER = 1 << 17;
        const FL_IMMUNE_SLIME = 1 << 18;
        const FL_IMMUNE_LAVA = 1 << 19;
        const FL_PROXY = 1 << 20;
        const FL_ALWAYSTHINK = 1 << 21;
        const FL_BASEVELOCITY = 1 << 22;
        const FL_MONSTERCLIP = 1 << 23;
        const FL_ONTRAIN = 1 << 24;
        const FL_WORLDBRUSH = 1 << 25;
        const FL_SPECTATOR = 1 << 26;
        const FL_CUSTOMENTITY = 1 << 29;
        const FL_KILLME = 1 << 30;
        const FL_DORMANT = 1 << 31;
    }
}

pub type string_t = c_int;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct link_s {
    pub prev: *mut link_s,
    pub next: *mut link_s,
}
#[test]
fn bindgen_test_layout_link_s() {
    assert_eq!(
        size_of::<link_s>(),
        8usize,
        concat!("Size of: ", stringify!(link_s))
    );
    assert_eq!(
        align_of::<link_s>(),
        4usize,
        concat!("Alignment of ", stringify!(link_s))
    );
    assert_eq!(
        unsafe { &(*(null::<link_s>())).prev as *const _ as usize },
        0usize,
        concat!(
            "Offset of field: ",
            stringify!(link_s),
            "::",
            stringify!(prev)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<link_s>())).next as *const _ as usize },
        4usize,
        concat!(
            "Offset of field: ",
            stringify!(link_s),
            "::",
            stringify!(next)
        )
    );
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct entvars_s {
    pub classname: string_t,
    pub globalname: string_t,
    pub origin: [f32; 3],
    pub oldorigin: [f32; 3],
    pub velocity: [f32; 3],
    pub basevelocity: [f32; 3],
    pub clbasevelocity: [f32; 3],
    pub movedir: [f32; 3],
    pub angles: [f32; 3],
    pub avelocity: [f32; 3],
    pub punchangle: [f32; 3],
    pub v_angle: [f32; 3],
    pub endpos: [f32; 3],
    pub startpos: [f32; 3],
    pub impacttime: f32,
    pub starttime: f32,
    pub fixangle: c_int,
    pub idealpitch: f32,
    pub pitch_speed: f32,
    pub ideal_yaw: f32,
    pub yaw_speed: f32,
    pub modelindex: c_int,
    pub model: string_t,
    pub viewmodel: c_int,
    pub weaponmodel: c_int,
    pub absmin: [f32; 3],
    pub absmax: [f32; 3],
    pub mins: [f32; 3],
    pub maxs: [f32; 3],
    pub size: [f32; 3],
    pub ltime: f32,
    pub nextthink: f32,
    pub movetype: c_int,
    pub solid: c_int,
    pub skin: c_int,
    pub body: c_int,
    pub effects: c_int,
    pub gravity: f32,
    pub friction: f32,
    pub light_level: c_int,
    pub sequence: c_int,
    pub gaitsequence: c_int,
    pub frame: f32,
    pub animtime: f32,
    pub framerate: f32,
    pub controller: [c_uchar; 4usize],
    pub blending: [c_uchar; 2usize],
    pub scale: f32,
    pub rendermode: c_int,
    pub renderamt: f32,
    pub rendercolor: [f32; 3],
    pub renderfx: c_int,
    pub health: f32,
    pub frags: f32,
    pub weapons: c_int,
    pub takedamage: f32,
    pub deadflag: c_int,
    pub view_ofs: [f32; 3],
    pub button: c_int,
    pub impulse: c_int,
    pub chain: *mut edict_s,
    pub dmg_inflictor: *mut edict_s,
    pub enemy: *mut edict_s,
    pub aiment: *mut edict_s,
    pub owner: *mut edict_s,
    pub groundentity: *mut edict_s,
    pub spawnflags: c_int,
    pub flags: Flags,
    pub colormap: c_int,
    pub team: c_int,
    pub max_health: f32,
    pub teleport_time: f32,
    pub armortype: f32,
    pub armorvalue: f32,
    pub waterlevel: c_int,
    pub watertype: c_int,
    pub target: string_t,
    pub targetname: string_t,
    pub netname: string_t,
    pub message: string_t,
    pub dmg_take: f32,
    pub dmg_save: f32,
    pub dmg: f32,
    pub dmgtime: f32,
    pub noise: string_t,
    pub noise1: string_t,
    pub noise2: string_t,
    pub noise3: string_t,
    pub speed: f32,
    pub air_finished: f32,
    pub pain_finished: f32,
    pub radsuit_finished: f32,
    pub pContainingEntity: *mut edict_s,
    pub playerclass: c_int,
    pub maxspeed: f32,
    pub fov: f32,
    pub weaponanim: c_int,
    pub pushmsec: c_int,
    pub bInDuck: c_int,
    pub flTimeStepSound: c_int,
    pub flSwimTime: c_int,
    pub flDuckTime: c_int,
    pub iStepLeft: c_int,
    pub flFallVelocity: f32,
    pub gamestate: c_int,
    pub oldbuttons: c_int,
    pub groupinfo: c_int,
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
    pub euser1: *mut edict_s,
    pub euser2: *mut edict_s,
    pub euser3: *mut edict_s,
    pub euser4: *mut edict_s,
}
#[test]
fn bindgen_test_layout_entvars_s() {
    assert_eq!(
        size_of::<entvars_s>(),
        676usize,
        concat!("Size of: ", stringify!(entvars_s))
    );
    assert_eq!(
        align_of::<entvars_s>(),
        4usize,
        concat!("Alignment of ", stringify!(entvars_s))
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).classname as *const _ as usize },
        0usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(classname)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).globalname as *const _ as usize },
        4usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(globalname)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).origin as *const _ as usize },
        8usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(origin)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).oldorigin as *const _ as usize },
        20usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(oldorigin)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).velocity as *const _ as usize },
        32usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(velocity)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).basevelocity as *const _ as usize },
        44usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(basevelocity)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).clbasevelocity as *const _ as usize },
        56usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(clbasevelocity)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).movedir as *const _ as usize },
        68usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(movedir)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).angles as *const _ as usize },
        80usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(angles)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).avelocity as *const _ as usize },
        92usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(avelocity)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).punchangle as *const _ as usize },
        104usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(punchangle)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).v_angle as *const _ as usize },
        116usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(v_angle)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).endpos as *const _ as usize },
        128usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(endpos)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).startpos as *const _ as usize },
        140usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(startpos)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).impacttime as *const _ as usize },
        152usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(impacttime)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).starttime as *const _ as usize },
        156usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(starttime)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).fixangle as *const _ as usize },
        160usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(fixangle)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).idealpitch as *const _ as usize },
        164usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(idealpitch)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).pitch_speed as *const _ as usize },
        168usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(pitch_speed)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).ideal_yaw as *const _ as usize },
        172usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(ideal_yaw)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).yaw_speed as *const _ as usize },
        176usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(yaw_speed)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).modelindex as *const _ as usize },
        180usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(modelindex)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).model as *const _ as usize },
        184usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(model)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).viewmodel as *const _ as usize },
        188usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(viewmodel)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).weaponmodel as *const _ as usize },
        192usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(weaponmodel)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).absmin as *const _ as usize },
        196usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(absmin)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).absmax as *const _ as usize },
        208usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(absmax)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).mins as *const _ as usize },
        220usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(mins)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).maxs as *const _ as usize },
        232usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(maxs)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).size as *const _ as usize },
        244usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(size)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).ltime as *const _ as usize },
        256usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(ltime)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).nextthink as *const _ as usize },
        260usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(nextthink)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).movetype as *const _ as usize },
        264usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(movetype)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).solid as *const _ as usize },
        268usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(solid)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).skin as *const _ as usize },
        272usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(skin)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).body as *const _ as usize },
        276usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(body)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).effects as *const _ as usize },
        280usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(effects)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).gravity as *const _ as usize },
        284usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(gravity)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).friction as *const _ as usize },
        288usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(friction)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).light_level as *const _ as usize },
        292usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(light_level)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).sequence as *const _ as usize },
        296usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(sequence)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).gaitsequence as *const _ as usize },
        300usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(gaitsequence)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).frame as *const _ as usize },
        304usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(frame)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).animtime as *const _ as usize },
        308usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(animtime)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).framerate as *const _ as usize },
        312usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(framerate)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).controller as *const _ as usize },
        316usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(controller)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).blending as *const _ as usize },
        320usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(blending)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).scale as *const _ as usize },
        324usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(scale)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).rendermode as *const _ as usize },
        328usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(rendermode)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).renderamt as *const _ as usize },
        332usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(renderamt)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).rendercolor as *const _ as usize },
        336usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(rendercolor)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).renderfx as *const _ as usize },
        348usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(renderfx)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).health as *const _ as usize },
        352usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(health)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).frags as *const _ as usize },
        356usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(frags)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).weapons as *const _ as usize },
        360usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(weapons)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).takedamage as *const _ as usize },
        364usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(takedamage)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).deadflag as *const _ as usize },
        368usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(deadflag)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).view_ofs as *const _ as usize },
        372usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(view_ofs)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).button as *const _ as usize },
        384usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(button)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).impulse as *const _ as usize },
        388usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(impulse)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).chain as *const _ as usize },
        392usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(chain)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).dmg_inflictor as *const _ as usize },
        396usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(dmg_inflictor)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).enemy as *const _ as usize },
        400usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(enemy)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).aiment as *const _ as usize },
        404usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(aiment)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).owner as *const _ as usize },
        408usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(owner)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).groundentity as *const _ as usize },
        412usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(groundentity)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).spawnflags as *const _ as usize },
        416usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(spawnflags)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).flags as *const _ as usize },
        420usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(flags)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).colormap as *const _ as usize },
        424usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(colormap)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).team as *const _ as usize },
        428usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(team)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).max_health as *const _ as usize },
        432usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(max_health)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).teleport_time as *const _ as usize },
        436usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(teleport_time)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).armortype as *const _ as usize },
        440usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(armortype)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).armorvalue as *const _ as usize },
        444usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(armorvalue)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).waterlevel as *const _ as usize },
        448usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(waterlevel)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).watertype as *const _ as usize },
        452usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(watertype)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).target as *const _ as usize },
        456usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(target)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).targetname as *const _ as usize },
        460usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(targetname)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).netname as *const _ as usize },
        464usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(netname)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).message as *const _ as usize },
        468usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(message)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).dmg_take as *const _ as usize },
        472usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(dmg_take)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).dmg_save as *const _ as usize },
        476usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(dmg_save)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).dmg as *const _ as usize },
        480usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(dmg)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).dmgtime as *const _ as usize },
        484usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(dmgtime)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).noise as *const _ as usize },
        488usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(noise)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).noise1 as *const _ as usize },
        492usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(noise1)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).noise2 as *const _ as usize },
        496usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(noise2)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).noise3 as *const _ as usize },
        500usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(noise3)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).speed as *const _ as usize },
        504usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(speed)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).air_finished as *const _ as usize },
        508usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(air_finished)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).pain_finished as *const _ as usize },
        512usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(pain_finished)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).radsuit_finished as *const _ as usize },
        516usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(radsuit_finished)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).pContainingEntity as *const _ as usize },
        520usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(pContainingEntity)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).playerclass as *const _ as usize },
        524usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(playerclass)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).maxspeed as *const _ as usize },
        528usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(maxspeed)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).fov as *const _ as usize },
        532usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(fov)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).weaponanim as *const _ as usize },
        536usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(weaponanim)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).pushmsec as *const _ as usize },
        540usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(pushmsec)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).bInDuck as *const _ as usize },
        544usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(bInDuck)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).flTimeStepSound as *const _ as usize },
        548usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(flTimeStepSound)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).flSwimTime as *const _ as usize },
        552usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(flSwimTime)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).flDuckTime as *const _ as usize },
        556usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(flDuckTime)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).iStepLeft as *const _ as usize },
        560usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(iStepLeft)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).flFallVelocity as *const _ as usize },
        564usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(flFallVelocity)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).gamestate as *const _ as usize },
        568usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(gamestate)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).oldbuttons as *const _ as usize },
        572usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(oldbuttons)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).groupinfo as *const _ as usize },
        576usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(groupinfo)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).iuser1 as *const _ as usize },
        580usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(iuser1)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).iuser2 as *const _ as usize },
        584usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(iuser2)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).iuser3 as *const _ as usize },
        588usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(iuser3)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).iuser4 as *const _ as usize },
        592usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(iuser4)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).fuser1 as *const _ as usize },
        596usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(fuser1)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).fuser2 as *const _ as usize },
        600usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(fuser2)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).fuser3 as *const _ as usize },
        604usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(fuser3)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).fuser4 as *const _ as usize },
        608usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(fuser4)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).vuser1 as *const _ as usize },
        612usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(vuser1)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).vuser2 as *const _ as usize },
        624usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(vuser2)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).vuser3 as *const _ as usize },
        636usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(vuser3)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).vuser4 as *const _ as usize },
        648usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(vuser4)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).euser1 as *const _ as usize },
        660usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(euser1)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).euser2 as *const _ as usize },
        664usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(euser2)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).euser3 as *const _ as usize },
        668usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(euser3)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<entvars_s>())).euser4 as *const _ as usize },
        672usize,
        concat!(
            "Offset of field: ",
            stringify!(entvars_s),
            "::",
            stringify!(euser4)
        )
    );
}
pub type entvars_t = entvars_s;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct edict_s {
    pub free: c_int,
    pub serialnumber: c_int,
    pub area: link_s,
    pub headnode: c_int,
    pub num_leafs: c_int,
    pub leafnums: [c_short; 48usize],
    pub freetime: f32,
    pub pvPrivateData: *mut c_void,
    pub v: entvars_t,
}
#[test]
fn bindgen_test_layout_edict_s() {
    assert_eq!(
        size_of::<edict_s>(),
        804usize,
        concat!("Size of: ", stringify!(edict_s))
    );
    assert_eq!(
        align_of::<edict_s>(),
        4usize,
        concat!("Alignment of ", stringify!(edict_s))
    );
    assert_eq!(
        unsafe { &(*(null::<edict_s>())).free as *const _ as usize },
        0usize,
        concat!(
            "Offset of field: ",
            stringify!(edict_s),
            "::",
            stringify!(free)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<edict_s>())).serialnumber as *const _ as usize },
        4usize,
        concat!(
            "Offset of field: ",
            stringify!(edict_s),
            "::",
            stringify!(serialnumber)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<edict_s>())).area as *const _ as usize },
        8usize,
        concat!(
            "Offset of field: ",
            stringify!(edict_s),
            "::",
            stringify!(area)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<edict_s>())).headnode as *const _ as usize },
        16usize,
        concat!(
            "Offset of field: ",
            stringify!(edict_s),
            "::",
            stringify!(headnode)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<edict_s>())).num_leafs as *const _ as usize },
        20usize,
        concat!(
            "Offset of field: ",
            stringify!(edict_s),
            "::",
            stringify!(num_leafs)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<edict_s>())).leafnums as *const _ as usize },
        24usize,
        concat!(
            "Offset of field: ",
            stringify!(edict_s),
            "::",
            stringify!(leafnums)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<edict_s>())).freetime as *const _ as usize },
        120usize,
        concat!(
            "Offset of field: ",
            stringify!(edict_s),
            "::",
            stringify!(freetime)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<edict_s>())).pvPrivateData as *const _ as usize },
        124usize,
        concat!(
            "Offset of field: ",
            stringify!(edict_s),
            "::",
            stringify!(pvPrivateData)
        )
    );
    assert_eq!(
        unsafe { &(*(null::<edict_s>())).v as *const _ as usize },
        128usize,
        concat!(
            "Offset of field: ",
            stringify!(edict_s),
            "::",
            stringify!(v)
        )
    );
}
