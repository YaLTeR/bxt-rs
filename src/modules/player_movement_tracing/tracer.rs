use bxt_strafe::{Hull, Trace, TraceResult};
use glam::Vec3;

use super::PlayerMovementTracing;
use crate::modules::Module;
use crate::utils::marker::MainThreadMarker;

#[derive(Clone, Copy)]
pub struct Tracer {
    marker: MainThreadMarker,
}

impl Tracer {
    /// Creates a new [`Tracer`].
    ///
    /// # Safety
    ///
    /// Player-movement tracing must be safe to do over the entire lifetime of this struct.
    pub unsafe fn new(marker: MainThreadMarker, remove_distance_limit: bool) -> Option<Self> {
        if !PlayerMovementTracing.is_enabled(marker) {
            return None;
        }

        super::maybe_ensure_server_tracing(marker, remove_distance_limit);

        Some(Self { marker })
    }
}

impl Trace for Tracer {
    fn trace(&self, start: Vec3, end: Vec3, hull: Hull) -> TraceResult {
        unsafe { super::player_trace(self.marker, start, end, hull) }
    }
}
