use std::ffi::CString;

use crate::{hooks::sdl, utils::*};

mod generated {
    #![allow(clippy::all)]
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

pub use generated::*;

pub static GL: MainThreadRefCell<Option<Gl>> = MainThreadRefCell::new(None);

/// # Safety
///
/// [`reset_pointers()`] must be called before SDL is unloaded so the pointers don't go stale.
pub unsafe fn load_pointers(marker: MainThreadMarker) {
    *GL.borrow_mut(marker) = Some(Gl::load_with(|name| {
        let name = CString::new(name).unwrap();
        sdl::SDL_GL_GetProcAddress.get(marker)(name.as_ptr())
    }));
}

pub fn reset_pointers(marker: MainThreadMarker) {
    *GL.borrow_mut(marker) = None;
}
