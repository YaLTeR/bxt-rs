use std::ffi::CStr;
use std::str::FromStr;

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
//
// And if the lifetime is not explicit, then "as fn(_, _)" doesn't work because it wants "for<'r>
// fn(&'r _, _)" or something.

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

impl<A1: FromStr, A2: FromStr> CommandHandler for fn(MainThreadMarker, A1, A2) {
    unsafe fn handle(self, marker: MainThreadMarker) -> bool {
        let mut args = Args::new(marker).skip(1);
        if args.len() != 2 {
            return false;
        }

        let a1 = if let Some(a1) = args.next().and_then(parse_arg) {
            a1
        } else {
            return false;
        };

        let a2 = if let Some(a2) = args.next().and_then(parse_arg) {
            a2
        } else {
            return false;
        };

        drop(args);
        self(marker, a1, a2);

        true
    }
}

impl<A1: FromStr, A2: FromStr, A3: FromStr> CommandHandler for fn(MainThreadMarker, A1, A2, A3) {
    unsafe fn handle(self, marker: MainThreadMarker) -> bool {
        let mut args = Args::new(marker).skip(1);
        if args.len() != 3 {
            return false;
        }

        let a1 = if let Some(a1) = args.next().and_then(parse_arg) {
            a1
        } else {
            return false;
        };

        let a2 = if let Some(a2) = args.next().and_then(parse_arg) {
            a2
        } else {
            return false;
        };

        let a3 = if let Some(a3) = args.next().and_then(parse_arg) {
            a3
        } else {
            return false;
        };

        drop(args);
        self(marker, a1, a2, a3);

        true
    }
}

/// Wraps a function accepting `FromStr` arguments as a console command handler.
///
/// The arguments are safely extracted and parsed into their respective types, and if the parsing
/// fails, the help text is printed.
#[macro_export]
macro_rules! handler {
    ($help:literal, $($fn:expr),+) => {{
        /// Handles the console command.
        ///
        /// # Safety
        ///
        /// This function must only be called as a console command handler callback.
        unsafe extern "C" fn handler() {
            $crate::utils::abort_on_panic(move || {
                let marker = $crate::utils::MainThreadMarker::new();

                // Try calling all command handlers. If the argument count doesn't match they will
                // return false.
                $(
                    if $crate::modules::commands::CommandHandler::handle($fn, marker) {
                        return;
                    }
                )+

                // None of the command handlers worked, print the help text.
                $crate::hooks::engine::con_print(marker, concat!("Usage: ", $help, '\n'));
            })
        }

        ($help, handler)
    }};
}
