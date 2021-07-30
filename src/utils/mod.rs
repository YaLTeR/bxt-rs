//! Utility objects.

use std::env;
use std::ffi::{CStr, CString, OsString};
use std::fs::OpenOptions;
use std::panic::{self, catch_unwind, UnwindSafe};
use std::process::abort;
use std::sync::Once;

use log::logger;
use simplelog::{CombinedLogger, LevelFilter, SharedLogger, TermLogger, WriteLogger};

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
/// automatically. https://github.com/rust-lang/rust/issues/52652
pub fn abort_on_panic<R, F: FnOnce() -> R + UnwindSafe>(f: F) -> R {
    match catch_unwind(f) {
        Ok(rv) => rv,
        Err(_) => {
            logger().flush();

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

    // Set up logging.
    let config = simplelog::ConfigBuilder::new()
        .set_thread_level(LevelFilter::Error)
        .set_target_level(LevelFilter::Error)
        .set_location_level(LevelFilter::Off)
        .set_time_format_str("%F %T%.3f")
        .set_time_to_local(true)
        .build();
    let mut logger: Vec<Box<(dyn SharedLogger + 'static)>> = vec![TermLogger::new(
        LevelFilter::Trace,
        config.clone(),
        simplelog::TerminalMode::Stderr,
    )];
    if let Ok(log_file) = OpenOptions::new()
        .append(true)
        .create(true)
        .open("bxt-rs.log")
    {
        logger.push(WriteLogger::new(LevelFilter::Trace, config, log_file));
    }
    let _ = CombinedLogger::init(logger);

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
}

/// Ensures logging, panic and error hooks are in place.
pub fn ensure_logging_hooks() {
    static ONCE: Once = Once::new();
    ONCE.call_once(setup_logging_hooks);
}

#[cfg(feature = "profiling")]
fn setup_profiling() {
    use rust_hawktracer::*;

    let instance = HawktracerInstance::new();
    let listener = instance.create_listener(HawktracerListenerType::ToFile {
        file_path: "trace.bin".into(),
        buffer_size: 4096,
    });

    Box::leak(Box::new(listener));
    Box::leak(Box::new(instance));
}

#[cfg(not(feature = "profiling"))]
fn setup_profiling() {}

/// Ensures profiling is initialized.
pub fn ensure_profiling() {
    static ONCE: Once = Once::new();
    ONCE.call_once(setup_profiling);
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

    CString::new(s.replace("\x00", "\\x00")).unwrap()
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
