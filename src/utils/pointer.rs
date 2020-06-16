//! A main thread pointer.

use std::{cell::Cell, ffi::c_void, ptr::NonNull};

use crate::utils::*;

/// Main thread pointer.
pub struct Pointer<P> {
    ptr: Cell<Option<P>>,
    symbol: &'static [u8],
}

/// Non-generic `Pointer` methods.
///
/// This trait is needed to be able to have an array of `Pointer`s.
pub trait PointerTrait: Sync {
    /// Sets the pointer.
    ///
    /// # Safety
    ///
    /// `ptr` must be a valid pointer of type `P` at least until the `Pointer` is reset.
    unsafe fn set(&self, marker: MainThreadMarker, ptr: Option<NonNull<c_void>>);

    /// Returns `true` if the `Pointer` has a pointer stored.
    fn is_set(&self, marker: MainThreadMarker) -> bool;

    /// Resets the `Pointer` to the empty state.
    fn reset(&self, marker: MainThreadMarker);

    /// Returns the pointer's symbol name.
    fn symbol(&self) -> &'static [u8];

    /// Logs pointer name and value.
    fn log(&self, marker: MainThreadMarker);
}

// Safety: all methods are guarded with MainThreadMarker.
unsafe impl<P> Sync for Pointer<P> {}

impl<P> Pointer<P> {
    /// Creates an empty `Pointer` with the given symbol name.
    pub const fn empty(symbol: &'static [u8]) -> Self {
        Self {
            ptr: Cell::new(None),
            symbol,
        }
    }
}

impl<P: Copy> Pointer<P> {
    /// Retrieves the stored pointer.
    ///
    /// # Panics
    ///
    /// Panics if the `Pointer` is empty.
    pub fn get(&self, marker: MainThreadMarker) -> P {
        self.get_opt(marker).unwrap()
    }

    /// Retrieves the stored pointer if it's present.
    pub fn get_opt(&self, _marker: MainThreadMarker) -> Option<P> {
        self.ptr.get()
    }
}

impl<P: Copy> PointerTrait for Pointer<P> {
    unsafe fn set(&self, _marker: MainThreadMarker, ptr: Option<NonNull<c_void>>) {
        self.ptr
            .set(ptr.map(|x| *(&x.as_ptr() as *const *mut c_void as *const P)));
    }

    fn is_set(&self, marker: MainThreadMarker) -> bool {
        self.get_opt(marker).is_some()
    }

    fn reset(&self, marker: MainThreadMarker) {
        unsafe {
            self.set(marker, None);
        }
    }

    fn symbol(&self) -> &'static [u8] {
        self.symbol
    }

    fn log(&self, marker: MainThreadMarker) {
        if !log_enabled!(log::Level::Debug) {
            return;
        }

        let name = CStr::from_bytes_with_nul(self.symbol)
            .unwrap()
            .to_str()
            .unwrap();
        let ptr = self
            .get_opt(marker)
            .map(|ptr| unsafe { *(&ptr as *const P as *const *const c_void) });

        match ptr {
            Some(ptr) => debug!("{:p}: {}", ptr, name),
            None => debug!("MISSING: {}", name),
        }
    }
}
