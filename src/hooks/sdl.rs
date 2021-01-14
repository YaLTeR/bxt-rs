#![allow(non_snake_case, non_upper_case_globals)]

use std::{
    os::raw::{c_char, c_int, c_void},
    ptr::NonNull,
};

use crate::utils::*;

pub static SDL_GL_ExtensionSupported: Pointer<unsafe extern "C" fn(*const c_char) -> c_int> =
    Pointer::empty(b"SDL_GL_ExtensionSupported\0");
pub static SDL_GL_GetProcAddress: Pointer<unsafe extern "C" fn(*const c_char) -> *mut c_void> =
    Pointer::empty(b"SDL_GL_GetProcAddress\0");

static POINTERS: &[&dyn PointerTrait] = &[&SDL_GL_ExtensionSupported, &SDL_GL_GetProcAddress];

#[cfg(unix)]
fn open_library() -> Option<libloading::Library> {
    use libc::{RTLD_NOLOAD, RTLD_NOW};

    let library =
        libloading::os::unix::Library::open(Some("libSDL2-2.0.so.0"), RTLD_NOW | RTLD_NOLOAD);
    library.ok().map(libloading::Library::from)
}

#[cfg(windows)]
fn open_library() -> Option<libloading::Library> {
    libloading::os::windows::Library::open_already_loaded("SDL2.DLL")
        .ok()
        .map(libloading::Library::from)
}

/// # Safety
///
/// [`reset_pointers()`] must be called before SDL is unloaded so the pointers don't go stale.
pub unsafe fn find_pointers(marker: MainThreadMarker) {
    let library = match open_library() {
        Some(library) => library,
        None => {
            warn!("error loading SDL2");
            return;
        }
    };

    for pointer in POINTERS {
        let ptr = library
            .get(pointer.symbol())
            .ok()
            .and_then(|sym| NonNull::new(*sym));
        pointer.set(marker, ptr);
        pointer.log(marker);
    }
}

pub fn reset_pointers(marker: MainThreadMarker) {
    for pointer in POINTERS {
        pointer.reset(marker);
    }
}
