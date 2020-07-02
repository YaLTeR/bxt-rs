use std::{ffi::CStr, str::FromStr};

use super::Args;
use crate::hooks::engine::Engine;

/// Parses a value of type `T` from the argument string.
fn parse_arg<T: FromStr>(arg: &CStr) -> Option<T> {
    arg.to_str().ok().and_then(|s| T::from_str(s).ok())
}

/// Trait defining a console command handler.
pub trait CommandHandler<'a> {
    /// Handles the console command.
    ///
    /// # Safety
    ///
    /// This method must only be called from a console command handler callback.
    unsafe fn handle(self, engine: &'a Engine) -> bool;
}

// Can't implement for Fn traits due to https://github.com/rust-lang/rust/issues/25041
//
// And when implementing for fn pointers we have to cast functions to them (with "as fn(_, _)" for
// example) since they can't convert automatically for some reason.
//
// And if the lifetime is not explicit, then "as fn(_, _)" doesn't work because it wants "for<'r>
// fn(&'r _, _)" or something.

impl<'a> CommandHandler<'a> for fn(&'a Engine) {
    unsafe fn handle(self, engine: &'a Engine) -> bool {
        let args = Args::new(engine.marker()).skip(1);
        if args.len() != 0 {
            return false;
        }

        drop(args);
        self(engine);

        true
    }
}

impl<'a, A1: FromStr> CommandHandler<'a> for fn(&'a Engine, A1) {
    unsafe fn handle(self, engine: &'a Engine) -> bool {
        let mut args = Args::new(engine.marker()).skip(1);
        if args.len() != 1 {
            return false;
        }

        let a1 = if let Some(a1) = args.next().and_then(parse_arg) {
            a1
        } else {
            return false;
        };

        drop(args);
        self(engine, a1);

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
                let engine = $crate::hooks::engine::Engine::new(marker);

                let success = $crate::modules::commands::CommandHandler::handle($fn, &engine);
                if !success {
                    engine.print($usage);
                }
            })
        }

        $crate::modules::commands::HandlerFunction(handler)
    }};
}
