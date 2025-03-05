//! Player-movement tracing.

use bxt_strafe::{Hull, TraceResult};
use glam::Vec3;

use super::Module;
use crate::ffi::playermove::TraceFlags;
use crate::hooks::engine;
use crate::utils::*;

pub mod tracer;
pub use tracer::Tracer;

pub struct PlayerMovementTracing;
impl Module for PlayerMovementTracing {
    fn name(&self) -> &'static str {
        "Player-movement tracing"
    }

    fn description(&self) -> &'static str {
        "Makes bxt-rs able to use the game's player movement collision detection."
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        engine::pmove.is_set(marker)
            && engine::g_svmove.is_set(marker)
            && engine::sv_areanodes.is_set(marker)
            && engine::SV_AddLinksToPM.is_set(marker)
        // SV_AddLinksToPM() with an underscore is required to adjust the distance limit, but not
        // required to initialize the tracing.
    }
}

static REMOVE_DISTANCE_LIMIT: MainThreadCell<bool> = MainThreadCell::new(false);

pub unsafe fn maybe_ensure_server_tracing(marker: MainThreadMarker, remove_distance_limit: bool) {
    if !PlayerMovementTracing.is_enabled(marker) {
        return;
    }

    *engine::pmove.get(marker) = engine::g_svmove.get(marker);
    let origin = &(**engine::pmove.get(marker)).origin;

    REMOVE_DISTANCE_LIMIT.set(marker, remove_distance_limit);
    engine::SV_AddLinksToPM.get(marker)(engine::sv_areanodes.get(marker), origin);
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
