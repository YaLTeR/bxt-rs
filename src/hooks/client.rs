//! 'client'.

#![allow(non_snake_case, non_upper_case_globals)]

use std::ptr::NonNull;

use super::engine;
use crate::utils::{abort_on_panic, MainThreadMarker, Pointer, PointerTrait};

pub static Shutdown: Pointer<unsafe extern "C" fn()> = Pointer::empty(b"Shutdown\0");

/// # Safety
///
/// This function must only be called right after `ClientDLL_Init()` is called.
pub unsafe fn hook_client_interface(marker: MainThreadMarker) {
    let functions = engine::cl_funcs.get_opt(marker);
    if functions.is_none() {
        return;
    }
    let functions = functions.unwrap().as_mut().unwrap();

    if let Some(shutdown) = &mut functions.shutdown {
        Shutdown.set(marker, Some(NonNull::new_unchecked(*shutdown as _)));
    }
    functions.shutdown = Some(my_Shutdown);
}

/// # Safety
///
/// This function must only be called from the client dll shutdown hook.
unsafe fn reset_client_interface(marker: MainThreadMarker) {
    let functions = engine::cl_funcs.get_opt(marker);
    if functions.is_none() {
        return;
    }
    let functions = functions.unwrap().as_mut().unwrap();

    // The fact that reset_client_interface() is called from the shutdown hook guarantees that the
    // pointers were hooked by bxt-rs first. Therefore we don't need to worry that we're replacing
    // them with bogus null pointers as in the server dll hooks.
    functions.shutdown = Shutdown.get_opt(marker);
    Shutdown.reset(marker);
}

pub unsafe extern "C" fn my_Shutdown() {
    abort_on_panic(move || {
        let marker = MainThreadMarker::new();

        if let Some(f) = Shutdown.get_opt(marker) {
            f();
        };

        reset_client_interface(marker);
    })
}
