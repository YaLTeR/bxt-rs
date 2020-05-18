//! Simple `dlopen` and `dlsym` abstraction.

use std::{
    ffi::{c_void, CStr, CString},
    ptr::NonNull,
};

use libc::{dlclose, dlerror, dlopen, dlsym, RTLD_NOLOAD, RTLD_NOW};

/// A container for a `dlopen()` handle.
pub struct Handle {
    /// The handle returned by `dlopen()`.
    ptr: NonNull<c_void>,
}

unsafe impl Sync for Handle {}

impl Drop for Handle {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            dlclose(self.ptr.as_ptr());
        }
    }
}

impl Handle {
    /// Obtains a symbol address using `dlsym()`.
    pub fn sym(&self, symbol: &str) -> Result<NonNull<c_void>, CString> {
        // Clear the previous error.
        unsafe {
            dlerror();
        }

        let symbol = CString::new(symbol).unwrap();
        let ptr = unsafe { dlsym(self.ptr.as_ptr(), symbol.as_ptr()) };

        match NonNull::new(ptr) {
            Some(ptr) => Ok(ptr),
            None => {
                let error = unsafe { dlerror() };
                assert!(!error.is_null());
                Err(dbg!(unsafe { CStr::from_ptr(error).to_owned() }))
            }
        }
    }
}

/// Opens a dynamic library and returns the resulting handle.
pub fn open(filename: &str) -> Result<Handle, CString> {
    let filename = CString::new(filename).unwrap();

    let ptr = unsafe { dlopen(filename.as_ptr(), RTLD_NOW | RTLD_NOLOAD) };

    match NonNull::new(ptr) {
        Some(ptr) => Ok(Handle { ptr }),
        None => {
            let error = unsafe { dlerror() };
            assert!(!error.is_null());
            Err(dbg!(unsafe { CStr::from_ptr(error).to_owned() }))
        }
    }
}
