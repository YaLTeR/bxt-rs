//! The main thread marker.

use std::marker::PhantomData;

/// This marker serves as a static guarantee of being on the main game thread. Functions that
/// should only be called from the main game thread should accept an argument of this type.
#[derive(Clone, Copy)]
pub struct MainThreadMarker {
    // Mark as !Send and !Sync.
    _marker: PhantomData<*const ()>,
}

impl MainThreadMarker {
    /// Creates a new `MainThreadMarker`.
    ///
    /// # Safety
    /// This should only be called from the main game thread.
    #[inline]
    pub unsafe fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}
