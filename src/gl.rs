use std::os::raw::c_void;

use rust_hawktracer::*;

use crate::modules::capture;
use crate::utils::*;

mod generated {
    #![allow(clippy::all)]
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

pub use generated::*;

pub static GL: MainThreadRefCell<Option<Gl>> = MainThreadRefCell::new(None);

/// # Safety
///
/// `load` must return valid pointers to OpenGL functions or null pointers.
///
/// [`reset_pointers()`] must be called before the library providing pointers is unloaded so the
/// pointers don't go stale.
#[hawktracer(gl_load_pointers)]
pub unsafe fn load_pointers(
    marker: MainThreadMarker,
    load: impl Fn(&'static str) -> *const c_void,
    is_extension_supported: impl Fn(&'static str) -> bool,
) {
    *GL.borrow_mut(marker) = Some(Gl::load_with(load));

    capture::check_gl_extensions(marker, is_extension_supported);
}

pub fn reset_pointers(marker: MainThreadMarker) {
    capture::reset_gl_state(marker);

    *GL.borrow_mut(marker) = None;
}
