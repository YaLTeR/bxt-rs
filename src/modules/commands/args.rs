//! Console command arguments.

use std::ffi::CStr;
use std::marker::PhantomData;

use crate::hooks::engine;
use crate::utils::*;

/// An iterator over arguments of a console command.
pub struct Args<'a> {
    index: usize,
    count: usize,
    marker: MainThreadMarker,
    _variance: PhantomData<&'a ()>,
}

impl<'a> Args<'a> {
    /// Creates a new `Args`.
    ///
    /// # Safety
    ///
    /// This function must only be called from within a console command handler. The returned
    /// lifetime should not exceed the duration of the console command handler. No engine functions
    /// should be called while the lifetime is active. Console command arrays must not be modified
    /// while the lifetime is active.
    pub unsafe fn new(marker: MainThreadMarker) -> Self {
        Self {
            index: 0,
            count: engine::Cmd_Argc.get(marker)() as usize,
            marker,
            _variance: PhantomData,
        }
    }
}

impl<'a> Iterator for Args<'a> {
    type Item = &'a CStr;

    fn next(&mut self) -> Option<Self::Item> {
        if self.len() == 0 {
            return None;
        }

        let arg = unsafe { engine::Cmd_Argv.get(self.marker)(self.index as i32) };
        self.index += 1;

        Some(unsafe { CStr::from_ptr(arg) })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.count - self.index;
        (remaining, Some(remaining))
    }
}

impl<'a> ExactSizeIterator for Args<'a> {}
