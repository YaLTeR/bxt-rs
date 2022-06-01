#![allow(non_snake_case, non_upper_case_globals)]

use std::ffi::CString;
use std::os::raw::{c_char, c_int, c_void};
use std::ptr::NonNull;

use bxt_patterns::Patterns;

use crate::gl;
use crate::modules::tas_editor;
use crate::utils::*;

pub static SDL_GL_ExtensionSupported: Pointer<unsafe extern "C" fn(*const c_char) -> c_int> =
    Pointer::empty(b"SDL_GL_ExtensionSupported\0");
pub static SDL_GL_GetProcAddress: Pointer<unsafe extern "C" fn(*const c_char) -> *const c_void> =
    Pointer::empty(b"SDL_GL_GetProcAddress\0");
pub static SDL_WarpMouseInWindow: Pointer<unsafe extern "C" fn(*mut c_void, c_int, c_int)> =
    Pointer::empty_patterns(
        b"SDL_WarpMouseInWindow\0",
        Patterns(&[]),
        my_SDL_WarpMouseInWindow as _,
    );

static POINTERS: &[&dyn PointerTrait] = &[
    &SDL_GL_ExtensionSupported,
    &SDL_GL_GetProcAddress,
    &SDL_WarpMouseInWindow,
];

#[cfg(windows)]
static ORIGINAL_FUNCTIONS: MainThreadRefCell<Vec<*mut c_void>> = MainThreadRefCell::new(Vec::new());

#[cfg(windows)]
unsafe fn maybe_hook(marker: MainThreadMarker, pointer: &dyn PointerTrait) {
    use minhook_sys::*;

    if !pointer.is_set(marker) {
        return;
    }

    let hook_fn = pointer.hook_fn();
    if hook_fn.is_null() {
        return;
    }

    let original = pointer.get_raw(marker);
    let mut trampoline = std::ptr::null_mut();
    assert_eq!(
        MH_CreateHook(original.as_ptr(), hook_fn, &mut trampoline),
        MH_OK
    );

    ORIGINAL_FUNCTIONS
        .borrow_mut(marker)
        .push(original.as_ptr());

    pointer.set_with_index(
        marker,
        NonNull::new_unchecked(trampoline),
        pointer.pattern_index(marker),
    );

    assert_eq!(MH_EnableHook(original.as_ptr()), MH_OK);
}

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

    #[cfg(windows)]
    {
        // Hook all found pointers on Windows.
        for &pointer in POINTERS {
            maybe_hook(marker, pointer);
        }
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

    // Remove all hooks.
    #[cfg(windows)]
    {
        use minhook_sys::*;

        for function in ORIGINAL_FUNCTIONS.borrow_mut(marker).drain(..) {
            assert_eq!(unsafe { MH_RemoveHook(function) }, MH_OK);
        }
    }
}

use exported::*;

/// Functions exported for `LD_PRELOAD` hooking.
pub mod exported {
    #![allow(clippy::missing_safety_doc)]

    use super::*;

    #[export_name = "SDL_WarpMouseInWindow"]
    pub unsafe extern "C" fn my_SDL_WarpMouseInWindow(window: *mut c_void, x: c_int, y: c_int) {
        abort_on_panic(move || {
            let marker = MainThreadMarker::new();

            if tas_editor::is_connected_to_server() {
                // Don't warp the mouse from simulator clients.
                return;
            }

            SDL_WarpMouseInWindow.get(marker)(window, x, y);
        })
    }
}
