//! Utility objects.

use std::env;
use std::ffi::{CStr, CString, OsString};
use std::fs::OpenOptions;
use std::panic::{self, catch_unwind, UnwindSafe};
use std::process::abort;
use std::sync::Once;

use git_version::git_version;
use tracing_subscriber::prelude::*;

pub mod marker;
pub use marker::*;

pub mod pointer;
pub use pointer::*;

pub mod main_thread_cell;
pub use main_thread_cell::*;

pub mod main_thread_ref_cell;
pub use main_thread_ref_cell::*;

/// Runs the given function and aborts the process if it panics.
///
/// It's necessary to wrap the code of each hook in this function until Rust finally does this
/// automatically. <https://github.com/rust-lang/rust/issues/52652>
pub fn abort_on_panic<R, F: FnOnce() -> R + UnwindSafe>(f: F) -> R {
    match catch_unwind(f) {
        Ok(rv) => rv,
        Err(_) => {
            #[cfg(windows)]
            {
                unsafe {
                    winapi::um::winuser::MessageBoxA(
                        std::ptr::null_mut(),
                        b"An internal error has occurred in bxt-rs. The game will close. \
                          Check bxt-rs.log for diagnostic information.\0"
                            .as_ptr()
                            .cast(),
                        b"bxt-rs\0".as_ptr().cast(),
                        winapi::um::winuser::MB_ICONERROR,
                    );
                }
            }

            abort()
        }
    }
}

fn setup_logging_hooks() {
    env::set_var("RUST_BACKTRACE", "full");

    // Only write the message to the terminal (skipping span arguments) so it's less spammy.
    let term_layer = tracing_subscriber::fmt::layer().fmt_fields(
        tracing_subscriber::fmt::format::debug_fn(|writer, field, value| {
            if field.name() == "message" {
                write!(writer, "{:?}", value)
            } else {
                Ok(())
            }
        }),
    );

    // Disable ANSI colors on Windows as they don't work properly in the legacy console window.
    // https://github.com/tokio-rs/tracing/issues/445
    #[cfg(windows)]
    let term_layer = term_layer.with_ansi(false);

    let file_layer = OpenOptions::new()
        .append(true)
        .create(true)
        .open("bxt-rs.log")
        .ok()
        .map(|file| {
            tracing_subscriber::fmt::layer()
                .with_writer(file)
                .with_ansi(false)
        });

    let profiling_layer = if env::var_os("BXT_RS_PROFILE").is_some() {
        let (chrome_layer, guard) = tracing_chrome::ChromeLayerBuilder::new()
            .file("trace.json")
            .include_args(true)
            .include_locations(false)
            .build();

        Box::leak(Box::new(guard));

        Some(chrome_layer)
    } else {
        None
    };

    tracing_subscriber::registry()
        .with(file_layer)
        .with(profiling_layer)
        // Term layer must be last, otherwise the log file will have some ANSI codes:
        // https://github.com/tokio-rs/tracing/issues/1817
        .with(term_layer)
        .init();

    // Set up panic and error hooks.
    let builder = color_eyre::config::HookBuilder::new()
        .capture_span_trace_by_default(false)
        .theme(color_eyre::config::Theme::new()); // Log files don't do ANSI.

    let (panic_hook, eyre_hook) = builder.into_hooks();

    // Install the panic hook manually since we want to output to error!() rather than stderr.
    panic::set_hook(Box::new(move |panic_info| {
        error!("{}", panic_hook.panic_report(panic_info));
    }));

    eyre_hook.install().unwrap();

    info!(
        "{} version {}",
        env!("CARGO_PKG_NAME"),
        git_version!(cargo_prefix = "cargo:", fallback = "unknown")
    );
}

/// Ensures logging, panic and error hooks are in place.
pub fn ensure_logging_hooks() {
    static ONCE: Once = Once::new();
    ONCE.call_once(setup_logging_hooks);
}

/// Converts a `CStr` into an `OsString`.
#[cfg(unix)]
pub fn c_str_to_os_string(c_str: &CStr) -> OsString {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    OsStr::from_bytes(c_str.to_bytes()).to_os_string()
}

/// Converts a `CStr` into an `OsString`.
#[cfg(windows)]
pub fn c_str_to_os_string(c_str: &CStr) -> OsString {
    // TODO: this will fail for invalid UTF-8. Can cvars contain invalid UTF-8? What to do in this
    // case?
    c_str.to_str().unwrap().into()
}

/// Converts a `&str` to a `CString`, changing null-bytes into `"\x00"`.
pub fn to_cstring_lossy(s: &str) -> CString {
    if let Ok(s) = CString::new(s) {
        return s;
    }

    CString::new(s.replace('\x00', "\\x00")).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_cstring_lossy_normal() {
        let c_string = to_cstring_lossy("hello");
        assert_eq!(c_string.to_str(), Ok("hello"));
    }

    #[test]
    fn to_cstring_lossy_null_byte() {
        let c_string = to_cstring_lossy("hel\x00lo");
        assert_eq!(c_string.to_str(), Ok("hel\\x00lo"));
    }

    #[test]
    fn to_cstring_lossy_null_byte_end() {
        let c_string = to_cstring_lossy("hello\x00");
        assert_eq!(c_string.to_str(), Ok("hello\\x00"));
    }
}
