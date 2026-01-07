use crate::ffi::buttons::Buttons;
use crate::ffi::edict::edict_s;
use crate::hooks::engine::{
    self, find_entity_in_sphere, get_entities_by_classname, get_entity_index, get_table_string,
    player_edict,
};
use crate::modules::timer::{timer_reset, timer_start, timer_stop};
use crate::utils::{MainThreadCell, MainThreadMarker, MainThreadRefCell};

const TIMER_START_TARGET: &[&str] = &[
    "counter_start",
    "clockstartbutton",
    "firsttimerelay",
    "but_start",
    "counter_start_button",
    "multi_start",
    "timer_startbutton",
    "start_timer_emi",
    "gogogo",
    "startcounter",
];

const TIMER_STOP_TARGET: &[&str] = &[
    "counter_off",
    "clockstopbutton",
    "clockstop",
    "but_stop",
    "counter_stop_button",
    "multi_stop",
    "stop_counter",
    "m_counter_end_emi",
    "stopcounter",
];

static PREV_BUTTON: MainThreadCell<Buttons> = MainThreadCell::new(Buttons::empty());
static FUNC_BUTTON_CLASSNAME_TABLE_INDEX: MainThreadCell<Option<u32>> = MainThreadCell::new(None);

pub(super) fn simulate_button_use(marker: MainThreadMarker) {
    let player = unsafe { player_edict(marker) };
    let Some(player) = player else {
        return;
    };
    let player = unsafe { player.as_ref() };
    let Some(player_button) = Buttons::from_bits(player.v.button as u16) else {
        return;
    };

    // update prev button right away because we might exit too soon
    let prev_button = PREV_BUTTON.get(marker);
    PREV_BUTTON.set(marker, player_button);

    // not pressing the button, GTFO
    if !player_button.contains(Buttons::IN_USE) {
        return;
    }

    // if prev button is still in use, it means the button is being held
    // do not trigger then
    if prev_button.contains(Buttons::IN_USE) {
        return;
    }

    const PLAYER_SEARCH_RADIUS: f32 = 64.;

    let entity_in_sphere =
        unsafe { find_entity_in_sphere(marker, player.v.origin, PLAYER_SEARCH_RADIUS) };
    let Some(entity_in_sphere) = entity_in_sphere else {
        return;
    };

    let vec_player_origin = glam::Vec3::from_array(player.v.origin);
    let v_forward = (unsafe { &*engine::gGlobalVariables.get(marker) }).v_forward;
    let vec_player_eyes = vec_player_origin + glam::Vec3::from_array(player.v.view_ofs);

    let possible_button = unsafe {
        entity_in_sphere
            .iter()
            // find only buttons first
            .filter(|&&entity| {
                let entity = &*entity;

                // if result is already cached, don't even do anything further
                if let Some(classname) = FUNC_BUTTON_CLASSNAME_TABLE_INDEX.get(marker) {
                    if classname == entity.v.classname {
                        return true;
                    } else {
                        return false;
                    }
                }

                let Some(entity_classname) = get_table_string(marker, entity.v.classname) else {
                    return false;
                };

                if entity_classname == "func_button" {
                    FUNC_BUTTON_CLASSNAME_TABLE_INDEX.set(marker, Some(entity.v.classname));
                    return true;
                }

                false
            })
            // sort them by view angle distance
            // because that takes precedence over distance
            .map(|&entity| {
                let entity = &*entity;
                // VecBModelOrigin()
                let vec_bmodel_origin = glam::Vec3::from_array(entity.v.absmin)
                    + glam::Vec3::from_array(entity.v.size) * 0.5;
                let vec_line_of_sight = vec_bmodel_origin - (vec_player_eyes);
                let vec_line_of_sight = util_clamp_vector_to_box(
                    vec_line_of_sight,
                    glam::Vec3::from_array(entity.v.size) * 0.5,
                );

                let dot_product = vec_line_of_sight.dot(glam::Vec3::from_array(v_forward));

                (entity, dot_product)
            })
            // find the higest dot product
            // aka the most aligned
            .max_by(|(_, dp1), (_, dp2)| dp1.total_cmp(dp2))
    };

    let Some((button_entity, view_angle)) = possible_button else {
        // this just means there are no entities in the list
        return;
    };

    const VIEW_FIELD_NARROW: f32 = 0.7;

    // the smallest angle button is still out of range
    if view_angle < VIEW_FIELD_NARROW {
        return;
    }

    // now we can finally check the button "target"
    let target = unsafe { get_table_string(marker, button_entity.v.target) };
    let Some(target) = target else { return };

    if TIMER_START_TARGET.contains(&target) {
        timer_reset(marker);
        timer_start(marker);
    } else if TIMER_STOP_TARGET.contains(&target) {
        timer_stop(marker);
    }
}

fn util_clamp_vector_to_box(mut input: glam::Vec3, clamp_size: glam::Vec3) -> glam::Vec3 {
    if input.x > clamp_size.x {
        input.x -= clamp_size.x;
    } else if input.x < -clamp_size.x {
        input.x += clamp_size.x;
    } else {
        input.x = 0.0;
    }

    if input.y > clamp_size.y {
        input.y -= clamp_size.y;
    } else if input.y < -clamp_size.y {
        input.y += clamp_size.y;
    } else {
        input.y = 0.0;
    }

    if input.z > clamp_size.z {
        input.z -= clamp_size.z;
    } else if input.z < -clamp_size.z {
        input.z += clamp_size.z;
    } else {
        input.z = 0.0;
    }

    input.normalize()
}

// now it is for the timer zone
// basically the same monster but different beast

enum FoundEntityState {
    NotSet,
    None,
    // can have multiple starts
    Found(Vec<usize>),
}

static START_ZONE: MainThreadRefCell<FoundEntityState> =
    MainThreadRefCell::new(FoundEntityState::NotSet);
static END_ZONE: MainThreadRefCell<FoundEntityState> =
    MainThreadRefCell::new(FoundEntityState::NotSet);

pub(super) fn find_auto_start_zone(marker: MainThreadMarker) {
    if !matches!(*START_ZONE.borrow(marker), FoundEntityState::NotSet)
        || !matches!(*END_ZONE.borrow(marker), FoundEntityState::NotSet)
    {
        return;
    }

    // only load when we are actually in the map
    if (unsafe { &*engine::cls.get(marker) }).state != 5 {
        return;
    }

    // this call is expensive but we only do it once for every map load so...
    let trigger_multiple_vec = unsafe { get_entities_by_classname(marker, "trigger_multiple") };
    let func_button_vec = unsafe { get_entities_by_classname(marker, "func_button") };

    let Some(trigger_multiple_vec) = trigger_multiple_vec else {
        return;
    };

    let Some(func_button_vec) = func_button_vec else {
        return;
    };

    let mut func_button_start: Vec<&str> = vec![];
    let mut func_button_end: Vec<&str> = vec![];

    // now find buttons with the name that we need
    func_button_vec.into_iter().for_each(|entity| {
        let entity = unsafe { &*entity };

        let target = unsafe { get_table_string(marker, entity.v.target) };
        let Some(target) = target else {
            return;
        };

        let targetname = unsafe { get_table_string(marker, entity.v.targetname) };
        let Some(targetname) = targetname else {
            return;
        };

        if TIMER_START_TARGET.contains(&target) {
            func_button_start.push(targetname);
        }

        if TIMER_STOP_TARGET.contains(&target) {
            func_button_end.push(targetname);
        }
    });

    let mut zone_start: Vec<usize> = vec![];
    let mut zone_end: Vec<usize> = vec![];

    // now go through trigger_multiple and find ones that activating hte ones we look at
    trigger_multiple_vec.into_iter().for_each(|entity| {
        let entity_deref = unsafe { &*entity };

        let target = unsafe { get_table_string(marker, entity_deref.v.target) };
        let Some(target) = target else {
            return;
        };

        if func_button_start.contains(&target) {
            let entity_index = unsafe { get_entity_index(marker, entity) };
            let Some(entity_index) = entity_index else {
                return;
            };

            zone_start.push(entity_index);
        }

        if func_button_end.contains(&target) {
            let entity_index = unsafe { get_entity_index(marker, entity) };
            let Some(entity_index) = entity_index else {
                return;
            };

            zone_end.push(entity_index);
        }
    });

    if !zone_start.is_empty() {
        *START_ZONE.borrow_mut(marker) = FoundEntityState::Found(zone_start);
    } else {
        *START_ZONE.borrow_mut(marker) = FoundEntityState::None;
    }

    if !zone_end.is_empty() {
        *END_ZONE.borrow_mut(marker) = FoundEntityState::Found(zone_end);
    } else {
        *END_ZONE.borrow_mut(marker) = FoundEntityState::None;
    }
}

pub(super) fn simulate_touch_timer_zone(marker: MainThreadMarker, touched_entity: *mut edict_s) {
    let start_zone = START_ZONE.borrow(marker);
    let end_zone = END_ZONE.borrow(marker);

    if matches!(
        *start_zone,
        FoundEntityState::NotSet | FoundEntityState::None
    ) || matches!(*end_zone, FoundEntityState::NotSet | FoundEntityState::None)
    {
        return;
    }

    let touched_entity_index = unsafe { get_entity_index(marker, touched_entity) };
    let Some(touched_entity_index) = touched_entity_index else {
        return;
    };

    if let FoundEntityState::Found(start_zone) = &*start_zone {
        if start_zone.contains(&touched_entity_index) {
            timer_reset(marker);
            timer_start(marker);
        }
    }

    if let FoundEntityState::Found(end_zone) = &*end_zone {
        if end_zone.contains(&touched_entity_index) {
            timer_stop(marker);
        }
    }
}
