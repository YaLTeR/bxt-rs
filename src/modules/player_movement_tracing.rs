//! Player-movement tracing.

use bxt_strafe::{Hull, TraceResult};
use glam::Vec3;

use super::Module;
use crate::ffi::playermove::TraceFlags;
use crate::hooks::engine::{self};
use crate::utils::*;

pub struct PlayerMovementTracing;
impl Module for PlayerMovementTracing {
    fn name(&self) -> &'static str {
        "Player-movement tracing"
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        engine::pmove.is_set(marker)
    }
}

static REMOVE_DISTANCE_LIMIT: MainThreadCell<bool> = MainThreadCell::new(false);

pub unsafe fn maybe_ensure_server_tracing(marker: MainThreadMarker, remove_distance_limit: bool) {
    if !PlayerMovementTracing.is_enabled(marker) {
        return;
    }

    if !engine::g_svmove.is_set(marker) {
        return;
    }

    *engine::pmove.get(marker) = engine::g_svmove.get(marker);

    if !remove_distance_limit {
        return;
    }

    if !engine::sv_areanodes.is_set(marker)
        || !engine::SV_AddLinksToPM.is_set(marker)
        || !engine::SV_AddLinksToPM_.is_set(marker)
    {
        return;
    }

    REMOVE_DISTANCE_LIMIT.set(marker, true);
    engine::SV_AddLinksToPM.get(marker)(engine::sv_areanodes.get(marker), &[0., 0., 0.]);
    REMOVE_DISTANCE_LIMIT.set(marker, false);
}

pub unsafe fn maybe_adjust_distance_limit(
    marker: MainThreadMarker,
    mins: *mut [f32; 3],
    maxs: *mut [f32; 3],
) {
    if !REMOVE_DISTANCE_LIMIT.get(marker) {
        return;
    }

    *mins = [f32::NEG_INFINITY; 3];
    *maxs = [f32::INFINITY; 3];
}

pub unsafe fn player_trace(
    marker: MainThreadMarker,
    start: Vec3,
    end: Vec3,
    hull: Hull,
) -> TraceResult {
    if !PlayerMovementTracing.is_enabled(marker) {
        panic!("tracing is not available");
    }

    let pmove = *engine::pmove.get(marker);
    let orig_hull = (*pmove).usehull;

    (*pmove).usehull = match hull {
        Hull::Standing => 0,
        Hull::Ducked => 1,
        Hull::Point => 2,
    };

    let tr = ((*pmove).PM_PlayerTrace)(
        start.as_ref().as_ptr(),
        end.as_ref().as_ptr(),
        TraceFlags::PM_NORMAL,
        -1,
    );

    (*pmove).usehull = orig_hull;

    TraceResult {
        all_solid: tr.allsolid != 0,
        start_solid: tr.startsolid != 0,
        fraction: tr.fraction,
        end_pos: Vec3::from(tr.endpos),
        plane_normal: Vec3::from(tr.plane.normal),
        entity: tr.ent,
    }
}
