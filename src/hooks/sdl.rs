#![allow(non_snake_case, non_upper_case_globals)]

use std::ffi::CString;
use std::os::raw::{c_char, c_int, c_void};
use std::ptr::NonNull;

use crate::gl;
use crate::utils::*;

pub static SDL_GL_ExtensionSupported: Pointer<unsafe extern "C" fn(*const c_char) -> c_int> =
    Pointer::empty(b"SDL_GL_ExtensionSupported\0");
pub static SDL_GL_GetProcAddress: Pointer<unsafe extern "C" fn(*const c_char) -> *const c_void> =
    Pointer::empty(b"SDL_GL_GetProcAddress\0");

static POINTERS: &[&dyn PointerTrait] = &[&SDL_GL_ExtensionSupported, &SDL_GL_GetProcAddress];

#[cfg(unix)]
fn open_library() -> Option<libloading::Library> {
    use libc::{RTLD_NOLOAD, RTLD_NOW};

    let library = unsafe {
        libloading::os::unix::Library::open(Some("libSDL2-2.0.so.0"), RTLD_NOW | RTLD_NOLOAD)
    };
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
#[instrument(name = "sdl::find_pointers", skip_all)]
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

    let load = |name| {
        let name = CString::new(name).unwrap();
        SDL_GL_GetProcAddress.get(marker)(name.as_ptr())
    };

    // SDL docs say that on X11 extension function pointers might be non-NULL even when extensions
    // aren't actually available. So we need to check for extension availability manually.
    //
    // https://wiki.libsdl.org/SDL_GL_GetProcAddress#Remarks
    let is_extension_supported = |name| {
        let name = CString::new(name).unwrap();
        SDL_GL_ExtensionSupported.get(marker)(name.as_ptr()) != 0
    };

    gl::load_pointers(marker, load, is_extension_supported);
}

pub fn reset_pointers(marker: MainThreadMarker) {
    gl::reset_pointers(marker);

    for pointer in POINTERS {
        pointer.reset(marker);
    }
}
