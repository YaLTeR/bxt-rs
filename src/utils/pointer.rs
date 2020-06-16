//! A main thread pointer.

use std::{cell::Cell, ffi::c_void, ptr::NonNull};

use crate::utils::MainThreadMarker;

/// Main thread pointer.
pub struct Pointer<P> {
    ptr: Cell<Option<P>>,
}

// Safety: all methods are guarded with MainThreadMarker.
unsafe impl<P> Sync for Pointer<P> {}

impl<P> Pointer<P> {
    /// Creates an empty `Pointer`.
    pub const fn empty() -> Self {
        Self {
            ptr: Cell::new(None),
        }
    }

    /// Resets the `Pointer` to the empty state.
    pub fn reset(&self, _marker: MainThreadMarker) {
        self.ptr.set(None);
    }
}

impl<P: Copy> Pointer<P> {
    /// Retrieves the stored pointer.
    ///
    /// # Panics
    ///
    /// Panics if the `Pointer` is empty.
    pub fn get(&self, _marker: MainThreadMarker) -> P {
        self.ptr.get().unwrap()
    }

    /// Retrieves the stored pointer if it's present.
    pub fn get_opt(&self, _marker: MainThreadMarker) -> Option<P> {
        self.ptr.get()
    }

    /// Sets the pointer.
    ///
    /// # Safety
    ///
    /// `ptr` must be a valid pointer of type `P` at least until the `Pointer` is reset.
    pub unsafe fn set(&self, _marker: MainThreadMarker, ptr: Option<NonNull<c_void>>) {
        self.ptr
            .set(ptr.map(|x| *(&x.as_ptr() as *const *mut c_void as *const P)));
    }

    /// Returns `true` if the `Pointer` has a pointer stored.
    pub fn is_set(&self, _marker: MainThreadMarker) -> bool {
        self.ptr.get().is_some()
    }
}
