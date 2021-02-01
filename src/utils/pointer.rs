//! A main thread pointer.

use std::{
    cell::Cell,
    ffi::c_void,
    ptr::{null_mut, NonNull},
};

use bxt_patterns::Patterns;

use crate::utils::*;

/// Main thread pointer.
pub struct Pointer<P> {
    ptr: Cell<Inner<P>>,
    symbol: &'static [u8],
    patterns: Patterns,
    hook_fn: *mut c_void,
}

/// Enum representing a found or not found pointer.
#[derive(Clone, Copy)]
enum Inner<P> {
    NotFound,
    Found {
        ptr: P,
        pattern_index: Option<usize>,
    },
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

    /// Sets the pointer with a pattern index.
    ///
    /// # Safety
    ///
    /// `ptr` must be a valid pointer of type `P` at least until the `Pointer` is reset.
    unsafe fn set_with_index(
        &self,
        marker: MainThreadMarker,
        ptr: NonNull<c_void>,
        pattern_index: Option<usize>,
    );

    /// Sets the pointer if it is currently empty.
    ///
    /// # Safety
    ///
    /// `ptr` must be a valid pointer of type `P` at least until the `Pointer` is reset.
    unsafe fn set_if_empty(&self, marker: MainThreadMarker, ptr: Option<NonNull<c_void>>);

    /// Returns `true` if the `Pointer` has a pointer stored.
    fn is_set(&self, marker: MainThreadMarker) -> bool;

    /// Resets the `Pointer` to the empty state.
    fn reset(&self, marker: MainThreadMarker);

    /// Gets the stored pointer in raw form (casted to `*mut c_void`)..
    ///
    /// # Panics
    ///
    /// Panics if the `Pointer` is empty.
    fn get_raw(&self, marker: MainThreadMarker) -> NonNull<c_void>;

    /// Returns the index of the pattern which matched this pointer, if any.
    fn pattern_index(&self, marker: MainThreadMarker) -> Option<usize>;

    /// Returns the pointer's symbol name.
    fn symbol(&self) -> &'static [u8];

    /// Returns the pointer's patterns.
    fn patterns(&self) -> Patterns;

    /// Returns the pointer's hook function.
    fn hook_fn(&self) -> *mut c_void;

    /// Logs pointer name and value.
    fn log(&self, marker: MainThreadMarker);

    /// Returns a pointer offset from this pointer.
    ///
    /// # Safety
    ///
    /// See pointer [`offset()`](https://doc.rust-lang.org/std/primitive.pointer.html#method.offset)
    /// Safety section.
    unsafe fn offset(&self, marker: MainThreadMarker, offset: isize) -> Option<NonNull<c_void>> {
        if !self.is_set(marker) {
            return None;
        }

        let ptr = self.get_raw(marker).as_ptr();
        let ptr = ptr.offset(offset);
        NonNull::new(ptr)
    }

    /// Returns a pointer stored at an offset from this pointer.
    ///
    /// # Safety
    ///
    /// The memory stored at an offset from this pointer must be valid.
    ///
    /// # Panics
    ///
    /// Panics if the `Pointer` is empty.
    unsafe fn by_offset(&self, marker: MainThreadMarker, offset: isize) -> Option<NonNull<c_void>> {
        let ptr = self.get_raw(marker).as_ptr();
        let ptr = *ptr.offset(offset).cast();
        NonNull::new(ptr)
    }

    /// Returns a pointer stored at an offset from this pointer plus offset plus 4.
    ///
    /// This is used for getting the address from E8 ?? ?? ?? ?? relative call instructions.
    ///
    /// # Safety
    ///
    /// The memory stored at an offset from this pointer must be valid.
    ///
    /// # Panics
    ///
    /// Panics if the `Pointer` is empty.
    unsafe fn by_relative_call(
        &self,
        marker: MainThreadMarker,
        offset: isize,
    ) -> Option<NonNull<c_void>> {
        let ptr = self.get_raw(marker).as_ptr();
        let ptr = ptr.offset(*ptr.offset(offset).cast::<isize>() + offset + 4);
        NonNull::new(ptr)
    }
}

// Safety: all methods are guarded with MainThreadMarker.
unsafe impl<P> Sync for Pointer<P> {}

impl<P> Pointer<P> {
    /// Creates an empty `Pointer` with the given symbol name.
    pub const fn empty(symbol: &'static [u8]) -> Self {
        // https://github.com/rust-lang/rust/issues/64992
        const EMPTY_SLICE: &[&[Option<u8>]] = &[];
        Self::empty_patterns(symbol, Patterns(EMPTY_SLICE), null_mut())
    }

    /// Creates an empty `Pointer` with the given symbol name, patterns and hook function.
    pub const fn empty_patterns(
        symbol: &'static [u8],
        patterns: Patterns,
        hook_fn: *mut c_void,
    ) -> Self {
        Self {
            ptr: Cell::new(Inner::NotFound),
            symbol,
            patterns,
            hook_fn,
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
        match self.ptr.get() {
            Inner::NotFound => None,
            Inner::Found { ptr, .. } => Some(ptr),
        }
    }
}

impl<P: Copy> PointerTrait for Pointer<P> {
    unsafe fn set(&self, _marker: MainThreadMarker, ptr: Option<NonNull<c_void>>) {
        let new_ptr = match ptr {
            Some(ptr) => Inner::Found {
                ptr: *(&ptr.as_ptr() as *const *mut c_void as *const P),
                pattern_index: None,
            },
            None => Inner::NotFound,
        };

        self.ptr.set(new_ptr);
    }

    unsafe fn set_with_index(
        &self,
        _marker: MainThreadMarker,
        ptr: NonNull<c_void>,
        pattern_index: Option<usize>,
    ) {
        let new_ptr = Inner::Found {
            ptr: *(&ptr.as_ptr() as *const *mut c_void as *const P),
            pattern_index,
        };

        self.ptr.set(new_ptr);
    }

    unsafe fn set_if_empty(&self, marker: MainThreadMarker, ptr: Option<NonNull<c_void>>) {
        if !self.is_set(marker) {
            self.set(marker, ptr);
        }
    }

    fn is_set(&self, marker: MainThreadMarker) -> bool {
        self.get_opt(marker).is_some()
    }

    fn reset(&self, marker: MainThreadMarker) {
        unsafe {
            self.set(marker, None);
        }
    }

    fn get_raw(&self, marker: MainThreadMarker) -> NonNull<c_void> {
        unsafe { NonNull::new_unchecked(*(&self.get(marker) as *const P as *const *mut c_void)) }
    }

    fn pattern_index(&self, _marker: MainThreadMarker) -> Option<usize> {
        if let Inner::Found { pattern_index, .. } = self.ptr.get() {
            pattern_index
        } else {
            None
        }
    }

    fn symbol(&self) -> &'static [u8] {
        self.symbol
    }

    fn patterns(&self) -> Patterns {
        self.patterns
    }

    fn hook_fn(&self) -> *mut c_void {
        self.hook_fn
    }

    fn log(&self, marker: MainThreadMarker) {
        let ptr = self
            .get_opt(marker)
            .map(|ptr| unsafe { *(&ptr as *const P as *const *const c_void) });

        log_pointer(self.symbol, ptr);
    }
}

// Extracted out of monomorphized function according to cargo llvm-lines.
fn log_pointer(name: &'static [u8], ptr: Option<*const c_void>) {
    if !log_enabled!(log::Level::Debug) {
        return;
    }

    let name = CStr::from_bytes_with_nul(name).unwrap().to_str().unwrap();

    match ptr {
        Some(ptr) => debug!("{:p}: {}", ptr, name),
        None => debug!("MISSING: {}", name),
    }
}
