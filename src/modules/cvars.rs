//! Console variables.

use std::{
    cell::UnsafeCell,
    ffi::{c_void, CStr, OsString},
    ptr::null_mut,
};

use super::{Module, MODULES};
use crate::{ffi::cvar as ffi, hooks::engine, utils::*};

/// Console variable.
#[derive(Debug)]
pub struct CVar {
    /// The variable itself, linked into the engine cvar list.
    ///
    /// Invariant: `name` and `string` pointers are valid.
    /// Invariant: when `string` is pointing to `name.as_ptr()`, the cvar isn't registered.
    /// Invariant: this is not moved out of while the variable is registered.
    ///
    /// Do not call any engine functions while a reference into the registered `ffi::cvar_s` is
    /// active. Assume any engine function can end up modifying its contents.
    raw: UnsafeCell<ffi::cvar_s>,
    /// Storage for the name.
    name: &'static [u8],
    /// Storage for the default value.
    default_value: &'static [u8],
}

// Safety: all methods accessing `cvar` require a `MainThreadMarker`.
unsafe impl Sync for CVar {}

impl CVar {
    /// Creates a new variable.
    pub const fn new(name: &'static [u8], default_value: &'static [u8]) -> Self {
        Self {
            raw: UnsafeCell::new(ffi::cvar_s {
                name: name.as_ptr().cast(),
                string: default_value.as_ptr().cast(),
                flags: 0,
                value: 0.,
                next: null_mut(),
            }),
            name,
            default_value,
        }
    }

    /// Returns `true` if the variable is currently registered in the engine.
    fn is_registered(&self, _marker: MainThreadMarker) -> bool {
        // Safety: we're not calling any engine methods while the reference is active.
        let raw = unsafe { &*self.raw.get() };

        raw.string != self.default_value.as_ptr().cast()
    }

    /// Returns the `bool` value of the variable.
    ///
    /// # Panics
    ///
    /// Panics if the variable is not registered.
    pub fn as_bool(&self, marker: MainThreadMarker) -> bool {
        assert!(self.is_registered(marker));

        // Safety: we're not calling any engine methods while the reference is active.
        let raw = unsafe { &*self.raw.get() };

        raw.value != 0.
    }

    /// Returns the value of the variable as an `OsString`.
    ///
    /// Use this for variables representing filenames and paths.
    ///
    /// # Panics
    ///
    /// Panics if the variable is not registered.
    pub fn to_os_string(&self, marker: MainThreadMarker) -> OsString {
        assert!(self.is_registered(marker));

        // Safety: we're not calling any engine methods while the reference is active.
        let raw = unsafe { &*self.raw.get() };

        let c_str = unsafe { CStr::from_ptr(raw.string) };
        c_str_to_os_string(c_str)
    }
}

/// Registers the variable in the engine.
///
/// As part of the registration the engine will store a pointer to the `raw` field of `cvar`, hence
/// `cvar` must not move after the registration, which is enforced by the 'static lifetime and not
/// having any interior mutability in the public interface.
///
/// # Safety
///
/// This function must only be called when it's safe to register console variables.
///
/// # Panics
///
/// Panics if the variable is already registered.
unsafe fn register(marker: MainThreadMarker, cvar: &'static CVar) {
    assert!(!cvar.is_registered(marker));

    // Make sure the provided name and value are valid C strings.
    assert!(CStr::from_bytes_with_nul(cvar.name).is_ok());
    assert!(CStr::from_bytes_with_nul(cvar.default_value).is_ok());

    engine::Cvar_RegisterVariable.get(marker)(cvar.raw.get());
}

/// Marks this variable as not registered.
///
/// # Safety
///
/// This function must only be called when the engine does not contain any references to the
/// variable.
unsafe fn mark_as_not_registered(_marker: MainThreadMarker, cvar: &CVar) {
    // Safety: we're not calling any engine methods while the reference is active.
    let raw = &mut *cvar.raw.get();

    raw.string = cvar.default_value.as_ptr().cast();
}

/// De-registers the variable.
///
/// # Safety
///
/// This function must only be called when it's safe to de-register console variables.
///
/// # Panics
///
/// Panics if the variable is not registered.
unsafe fn deregister(marker: MainThreadMarker, cvar: &CVar) {
    assert!(cvar.is_registered(marker));

    // Find a pointer to `cvar`. Start from `cvar_vars` (which points to the first registered
    // variable). On each iteration, check if the pointer points to `cvar`, and if not, follow it.
    // `cvar_vars` can't be null because there's at least one registered variable (the one we're
    // de-registering).
    let mut prev_ptr = engine::cvar_vars.get(marker);

    while *prev_ptr != cvar.raw.get() {
        // The next pointer can't be null because we still haven't found our (registered) variable.
        assert!(!(**prev_ptr).next.is_null());

        prev_ptr = &mut (**prev_ptr).next;
    }

    // Make it point to the variable after `cvar`. If there are no variables after `cvar`, it will
    // be set to null as it should be.
    *prev_ptr = (**prev_ptr).next;

    // Free the engine-allocated string and mark the variable as not registered.
    engine::Z_Free.get(marker)((*cvar.raw.get()).string as *mut c_void);
    mark_as_not_registered(marker, cvar);
}

/// # Safety
///
/// This function must only be called right after `Memory_Init()` completes.
pub unsafe fn register_all_cvars(marker: MainThreadMarker) {
    if !CVars.is_enabled(marker) {
        return;
    }

    for module in MODULES {
        for cvar in module.cvars() {
            register(marker, cvar);
        }
    }
}

/// # Safety
///
/// This function must only be called right after `Host_Shutdown()` is called.
pub unsafe fn mark_all_cvars_as_not_registered(marker: MainThreadMarker) {
    if !CVars.is_enabled(marker) {
        return;
    }

    for module in MODULES {
        for cvar in module.cvars() {
            // Safety: at this point the engine has no references into the variables and the memory
            // for the variable values is about to be freed.
            mark_as_not_registered(marker, cvar);
        }
    }
}

/// # Safety
///
/// This function must only be called when it's safe to de-register console variables.
pub unsafe fn deregister_disabled_module_cvars(marker: MainThreadMarker) {
    if !CVars.is_enabled(marker) {
        return;
    }

    for module in MODULES {
        if module.is_enabled(marker) {
            continue;
        }

        for cvar in module.cvars() {
            if !cvar.is_registered(marker) {
                continue;
            }

            deregister(marker, cvar);
        }
    }
}

pub struct CVars;
impl Module for CVars {
    fn name(&self) -> &'static str {
        "Console variables"
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        engine::Memory_Init.is_set(marker)
            && engine::Host_Shutdown.is_set(marker)
            && engine::Cvar_RegisterVariable.is_set(marker)
            && engine::Z_Free.is_set(marker)
            && engine::cvar_vars.is_set(marker)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cvar_names_and_values() {
        for module in MODULES {
            for cvar in module.cvars() {
                assert!(CStr::from_bytes_with_nul(cvar.name).is_ok());
                assert!(CStr::from_bytes_with_nul(cvar.default_value).is_ok());
            }
        }
    }
}
