use std::cell::Cell;

use bxt_strafe::{Hull, Trace, TraceResult};
use glam::Vec3;

use super::PlayerMovementTracing;
use crate::modules::Module;
use crate::utils::marker::MainThreadMarker;

pub struct Tracer {
    marker: MainThreadMarker,
    remove_distance_limit: bool,
    ensured: Cell<bool>,
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

        Some(Self {
            marker,
            remove_distance_limit,
            ensured: Cell::new(false),
        })
    }
}

impl Trace for Tracer {
    fn trace(&self, start: Vec3, end: Vec3, hull: Hull) -> TraceResult {
        if !self.ensured.replace(true) {
            // Ensure server tracing lazily to support drawing the TAS editor outside gameplay to
            // some extent (and also to avoid unnecessary calls).
            unsafe { super::maybe_ensure_server_tracing(self.marker, self.remove_distance_limit) };
        }

        unsafe { super::player_trace(self.marker, start, end, hull) }
    }
}
