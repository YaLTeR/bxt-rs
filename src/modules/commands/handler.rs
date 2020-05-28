use std::{ffi::CStr, str::FromStr};

use super::Args;
use crate::utils::MainThreadMarker;

/// Parses a value of type `T` from the argument string.
fn parse_arg<T: FromStr>(arg: &CStr) -> Option<T> {
    arg.to_str().ok().and_then(|s| T::from_str(s).ok())
}

/// Trait defining a console command handler.
pub trait CommandHandler {
    /// Handles the console command.
    ///
    /// # Safety
    ///
    /// This method must only be called from a console command handler callback.
    unsafe fn handle(self, marker: MainThreadMarker) -> bool;
}

// Can't implement for Fn traits due to https://github.com/rust-lang/rust/issues/25041
//
// And when implementing for fn pointers we have to cast functions to them (with "as fn(_, _)" for
// example) since they can't convert automatically for some reason.

impl CommandHandler for fn(MainThreadMarker) {
    unsafe fn handle(self, marker: MainThreadMarker) -> bool {
        let args = Args::new(marker).skip(1);
        if args.len() != 0 {
            return false;
        }

        drop(args);
        self(marker);

        true
    }
}

impl<A1: FromStr> CommandHandler for fn(MainThreadMarker, A1) {
    unsafe fn handle(self, marker: MainThreadMarker) -> bool {
        let mut args = Args::new(marker).skip(1);
        if args.len() != 1 {
            return false;
        }

        let a1 = if let Some(a1) = args.next().and_then(parse_arg) {
            a1
        } else {
            return false;
        };

        drop(args);
        self(marker, a1);

        true
    }
}

/// Wraps a function accepting `FromStr` arguments as a console command handler.
///
/// The arguments are safely extracted and parsed into their respective types, and if the parsing
/// fails, usage is printed.
#[macro_export]
macro_rules! handler {
    ($usage:expr, $fn:expr) => {{
        /// Handles the console command.
        ///
        /// # Safety
        ///
        /// This function must only be called as a console command handler callback.
        unsafe extern "C" fn handler() {
            $crate::utils::abort_on_panic(move || {
                let marker = $crate::utils::MainThreadMarker::new();

                let success = $crate::modules::commands::CommandHandler::handle($fn, marker);

                if !success {
                    // Make sure our usage string is valid.
                    let usage: &[u8] = $usage;
                    assert!(*usage.last().unwrap() == 0);

                    let usage: *const std::os::raw::c_char = usage.as_ptr().cast();
                    $crate::engine::CON_PRINTF.get(marker)(b"%s\n\0".as_ptr().cast(), usage);
                }
            })
        }

        handler
    }};
}
