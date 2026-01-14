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

macro_rules! impl_command_handler {
    ($($arg:ident),*) => {
        impl<$($arg: FromStr),*> CommandHandler for fn(MainThreadMarker, $($arg),*) {
            unsafe fn handle(self, marker: MainThreadMarker) -> bool {
                let mut args = Args::new(marker).skip(1);
                let expected_len = 0 $(+ { let _ = stringify!($arg); 1 })*;

                if args.len() != expected_len {
                    return false;
                }

                $(
                    let $arg = match args.next().and_then(|s| parse_arg::<$arg>(s)) {
                        Some(val) => val,
                        None => return false,
                    };
                )*

                drop(args);
                self(marker, $($arg),*);

                true
            }
        }
    };
}

impl_command_handler!();
impl_command_handler!(A1);
impl_command_handler!(A1, A2);
impl_command_handler!(A1, A2, A3);
impl_command_handler!(A1, A2, A3, A4);
impl_command_handler!(A1, A2, A3, A4, A5);
impl_command_handler!(A1, A2, A3, A4, A5, A6);
impl_command_handler!(A1, A2, A3, A4, A5, A6, A7);
impl_command_handler!(A1, A2, A3, A4, A5, A6, A7, A8);
impl_command_handler!(A1, A2, A3, A4, A5, A6, A7, A8, A9);
impl_command_handler!(A1, A2, A3, A4, A5, A6, A7, A8, A9, A10);

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
