//! Viewmodel sway

use glam::Vec3;

use super::cvars::CVars;
use super::Module;
use crate::hooks::engine::{self, ref_params_s};
use crate::modules::cvars::CVar;
use crate::utils::*;

pub struct ViewmodelSway;
impl Module for ViewmodelSway {
    fn name(&self) -> &'static str {
        "Viewmodel sway"
    }

    fn description(&self) -> &'static str {
        "Adding CS:GO-like weapon sway."
    }

    fn cvars(&self) -> &'static [&'static CVar] {
        static CVARS: &[&CVar] = &[
            &BXT_VIEWMODEL_SWAY,
            &BXT_VIEWMODEL_SWAY_MAX,
            &BXT_VIEWMODEL_SWAY_RATE,
        ];
        CVARS
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        CVars.is_enabled(marker)
            && engine::cl_viewent.is_set(marker)
            && engine::cl_viewent_viewmodel.is_set(marker)
    }
}

static BXT_VIEWMODEL_SWAY: CVar = CVar::new(
    b"bxt_viewmodel_sway\0",
    b"0\0",
    "\
Setting to `1` enables weapon sway.",
);

static BXT_VIEWMODEL_SWAY_MAX: CVar = CVar::new(
    b"bxt_viewmodel_sway_max\0",
    b"100\0",
    "\
Max of sway.",
);

static BXT_VIEWMODEL_SWAY_RATE: CVar = CVar::new(
    b"bxt_viewmodel_sway_rate\0",
    b"0.02\0",
    "\
Rate of sway.",
);

static LAST_VIEWANGLES: MainThreadCell<Vec3> = MainThreadCell::new(Vec3::ZERO);

fn normalize_angle(angle: f32) -> f32 {
    let new_angle = angle % 360.;

    if new_angle > 180. {
        new_angle - 360.
    } else if new_angle < -180. {
        new_angle + 360.
    } else {
        new_angle
    }
}

// credits to https://github.com/edgarbarney/halflife-planckepoch/commit/9a38b22acf97a6d5065b466c060e6bcd10c6db2c
fn smooth_values(start: f32, end: f32, speed: f32) -> f32 {
    let d = end - start;
    let dabs = d.abs();

    if dabs > 0.01 {
        if (end - start) > 0. {
            start + (dabs * speed)
        } else {
            start - (dabs * speed)
        }
    } else {
        end
    }
}

pub fn add_viewmodel_sway(marker: MainThreadMarker, rp: &ref_params_s) {
    if !ViewmodelSway.is_enabled(marker) {
        return;
    }

    if !BXT_VIEWMODEL_SWAY.as_bool(marker) {
        return;
    }

    if rp.paused == 1 {
        return;
    }

    // Safety: no engine functions are called while the reference is active.
    let view = unsafe { &mut *engine::cl_viewent_viewmodel.get(marker) };
    let mut last = LAST_VIEWANGLES.get(marker);

    for i in 0..2 {
        last[i] = normalize_angle(view.angles[i] - last[i]);
    }

    let m_x = last[1] * 0.045;
    let m_y = last[0] * 0.045;
    let frameadj = (1. / rp.frametime) * 0.01;
    let max = BXT_VIEWMODEL_SWAY_MAX.as_f32(marker);
    let vertical_reduction = view.angles[0].to_radians().cos().abs();

    last[0] = smooth_values(last[0], m_y * frameadj, rp.frametime * 4.).clamp(-max, max);
    last[1] = smooth_values(last[1], m_x * frameadj, rp.frametime * 4.).clamp(-max, max);

    view.angles[0] -= last[0];
    view.angles[1] -= last[1];

    for i in 0..3 {
        view.origin[i] += BXT_VIEWMODEL_SWAY_RATE.as_f32(marker)
            * vertical_reduction
            * (-last[1].abs() * rp.forward[i] + last[0] * rp.up[i] + last[1] * rp.right[i]);
    }

    LAST_VIEWANGLES.set(marker, view.angles.into());
}

pub fn on_cl_disconnnect(marker: MainThreadMarker) {
    LAST_VIEWANGLES.set(marker, Vec3::ZERO);
}
