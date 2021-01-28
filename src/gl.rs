use std::ffi::CString;

use rust_hawktracer::*;

use crate::{hooks::sdl, modules::capture, utils::*};

mod generated {
    #![allow(clippy::all)]
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

pub use generated::*;

pub static GL: MainThreadRefCell<Option<Gl>> = MainThreadRefCell::new(None);

/// # Safety
///
/// [`reset_pointers()`] must be called before SDL is unloaded so the pointers don't go stale.
#[hawktracer(gl_load_pointers)]
pub unsafe fn load_pointers(marker: MainThreadMarker) {
    *GL.borrow_mut(marker) = Some(Gl::load_with(|name| {
        let name = CString::new(name).unwrap();
        sdl::SDL_GL_GetProcAddress.get(marker)(name.as_ptr())
    }));

    // SDL docs say that on X11 extension function pointers might be non-NULL even when extensions
    // aren't actually available. So we need to check for extension availability manually.
    //
    // https://wiki.libsdl.org/SDL_GL_GetProcAddress#Remarks
    let is_supported = |name| sdl::SDL_GL_ExtensionSupported.get(marker)(name) != 0;

    capture::check_gl_extensions(marker, is_supported);
}

pub fn reset_pointers(marker: MainThreadMarker) {
    capture::reset_gl_state(marker);

    *GL.borrow_mut(marker) = None;
}
