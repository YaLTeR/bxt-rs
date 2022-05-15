//! `opengl32.dll`

#![allow(non_snake_case, non_upper_case_globals)]

use std::ffi::{CStr, CString};
use std::ptr::{self, NonNull};

use winapi::shared::minwindef::PROC;
use winapi::shared::windef::HGLRC;
use winapi::um::winnt::LPCSTR;

use crate::gl;
use crate::utils::*;

pub static wglGetCurrentContext: Pointer<unsafe extern "system" fn() -> HGLRC> =
    Pointer::empty(b"wglGetCurrentContext\0");
pub static wglGetProcAddress: Pointer<unsafe extern "system" fn(LPCSTR) -> PROC> =
    Pointer::empty(b"wglGetProcAddress\0");

static POINTERS: &[&dyn PointerTrait] = &[&wglGetCurrentContext, &wglGetProcAddress];

fn open_library() -> Option<libloading::Library> {
    libloading::os::windows::Library::open_already_loaded("opengl32.dll")
        .ok()
        .map(libloading::Library::from)
}

/// # Safety
///
/// [`reset_pointers()`] must be called before opengl32.dll is unloaded so the pointers don't go
/// stale.
#[instrument(name = "opengl32::find_pointers", skip_all)]
pub unsafe fn find_pointers(marker: MainThreadMarker) {
    let library = match open_library() {
        Some(library) => library,
        None => {
            warn!("error loading opengl32.dll");
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

    if gl::GL.borrow(marker).is_some() {
        // Already loaded from SDL.
        return;
    }

    if wglGetCurrentContext.get(marker)().is_null() {
        // No OpenGL context.
        debug!("wglGetCurrentContext() returned NULL, not loading OpenGL");
        return;
    }

    let load = |name| {
        let name = CString::new(name).unwrap();

        let mut ptr = wglGetProcAddress.get(marker)(name.as_ptr());
        if ptr.is_null() {
            // wglGetProcAddress() only works for extension functions, otherwise
            // we need to get them from opengl32.dll itself.
            ptr = library
                .get(name.as_bytes_with_nul())
                .map(|sym| *sym)
                .unwrap_or(ptr::null_mut());
        }
        ptr as *const _
    };

    let is_extension_supported = |name| {
        let gl = gl::GL.borrow(marker);
        let gl = if let Some(gl) = gl.as_ref() {
            gl
        } else {
            return false;
        };

        let version = gl.GetString(gl::VERSION);
        assert!(!version.is_null());

        let version =
            String::from_utf8(CStr::from_ptr(version.cast()).to_bytes().to_vec()).unwrap();
        let first_digit = version.split('.').next().unwrap();
        let first_digit: u8 = first_digit.parse().unwrap();

        // https://github.com/glium/glium/blob/e6b29a6bab1f1999c8f46d1643d791f505631fab/src/context/extensions.rs#L220
        if first_digit >= 3 {
            let mut num_extensions = 0;
            gl.GetIntegerv(gl::NUM_EXTENSIONS, &mut num_extensions);

            (0..num_extensions)
                .filter_map(|num| {
                    let ext = gl.GetStringi(gl::EXTENSIONS, num as _);
                    assert!(!ext.is_null());
                    CStr::from_ptr(ext.cast()).to_str().ok()
                })
                .any(|ext| ext == name)
        } else {
            let list = gl.GetString(gl::EXTENSIONS);
            assert!(!list.is_null());
            let list = CStr::from_ptr(list.cast()).to_string_lossy();
            list.split(' ').any(|ext| ext == name)
        }
    };

    gl::load_pointers(marker, load, is_extension_supported);
}

pub fn reset_pointers(marker: MainThreadMarker) {
    gl::reset_pointers(marker);

    for pointer in POINTERS {
        pointer.reset(marker);
    }
}
