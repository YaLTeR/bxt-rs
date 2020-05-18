//! A main thread function pointer.

use std::{cell::Cell, ffi::c_void, ptr::NonNull};

use crate::utils::MainThreadMarker;

/// Main thread function pointer.
pub struct Function<F> {
    ptr: Cell<Option<F>>,
}

// Safety: all methods are guarded with MainThreadMarker.
unsafe impl<F> Sync for Function<F> {}

impl<F> Function<F> {
    /// Creates an empty `Function`.
    pub const fn empty() -> Self {
        Self {
            ptr: Cell::new(None),
        }
    }

    /// Resets the `Function` to the empty state.
    pub fn reset(&self, _marker: MainThreadMarker) {
        self.ptr.set(None);
    }
}

impl<F: Copy> Function<F> {
    /// Retrieves the stored pointer.
    ///
    /// # Panics
    ///
    /// Panics if the `Function` is empty.
    pub fn get(&self, _marker: MainThreadMarker) -> F {
        self.ptr.get().unwrap()
    }

    /// Sets the pointer.
    ///
    /// # Safety
    ///
    /// `ptr` must be a valid pointer of type `F` at least until the `Function` is reset.
    pub unsafe fn set(&self, _marker: MainThreadMarker, ptr: Option<NonNull<c_void>>) {
        self.ptr
            .set(ptr.map(|x| *(&x.as_ptr() as *const *mut c_void as *const F)));
    }

    /// Returns `true` if the `Function` has a pointer stored.
    pub fn is_set(&self, _marker: MainThreadMarker) -> bool {
        self.ptr.get().is_some()
    }
}
