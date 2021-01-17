//! `Cell` accessible only from the main thread.

#![allow(unused)]

use std::cell::Cell;

use crate::utils::*;

/// `Cell` accessible only from the main thread.
pub struct MainThreadCell<T>(Cell<T>);

// Safety: all methods are guarded with MainThreadMarker.
unsafe impl<T> Send for MainThreadCell<T> {}
unsafe impl<T> Sync for MainThreadCell<T> {}

impl<T> MainThreadCell<T> {
    /// Creates a new `MainThreadCell` containing the given value.
    pub const fn new(value: T) -> Self {
        Self(Cell::new(value))
    }

    /// Sets the contained value.
    pub fn set(&self, _marker: MainThreadMarker, val: T) {
        self.0.set(val);
    }
}

impl<T: Copy> MainThreadCell<T> {
    /// Returns a copy of the contained value.
    pub fn get(&self, _marker: MainThreadMarker) -> T {
        self.0.get()
    }
}
