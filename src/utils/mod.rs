//! Utility objects.

use std::{
    ffi::{CStr, OsStr, OsString},
    panic::{catch_unwind, UnwindSafe},
    process::abort,
};

pub mod marker;
pub use marker::MainThreadMarker;

pub mod function;
pub use function::Function;

pub mod variable;
pub use variable::Variable;

pub mod main_thread_ref_cell;
pub use main_thread_ref_cell::MainThreadRefCell;

pub mod dl;

/// Runs the given function and aborts the process if it panics.
///
/// It's necessary to wrap the code of each hook in this function until Rust finally does this
/// automatically. https://github.com/rust-lang/rust/issues/52652
pub fn abort_on_panic<R, F: FnOnce() -> R + UnwindSafe>(f: F) -> R {
    match catch_unwind(f) {
        Ok(rv) => rv,
        Err(_) => abort(),
    }
}

/// Converts a `CStr` into an `OsString`.
#[cfg(unix)]
pub fn c_str_to_os_string(c_str: &CStr) -> OsString {
    use std::os::unix::ffi::OsStrExt;
    OsStr::from_bytes(c_str.to_bytes()).to_os_string()
}
