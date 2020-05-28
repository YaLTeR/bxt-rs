//! `hw`, `sw`, `hl`.

use std::{ffi::CString, os::raw::*};

use crate::{
    ffi,
    modules::{commands, cvars, fade_remove},
    utils::{abort_on_panic, dl, Function, MainThreadMarker, Variable},
};

pub static CMD_ADDMALLOCCOMMAND: Function<
    unsafe extern "C" fn(*const c_char, unsafe extern "C" fn(), c_int),
> = Function::empty();
pub static CMD_ARGC: Function<unsafe extern "C" fn() -> c_int> = Function::empty();
pub static CMD_ARGV: Function<unsafe extern "C" fn(c_int) -> *const c_char> = Function::empty();
pub static CMD_FUNCTIONS: Variable<*mut ffi::command::cmd_function_s> = Variable::empty();
pub static CON_PRINTF: Function<unsafe extern "C" fn(*const c_char, ...)> = Function::empty();
pub static CVAR_REGISTERVARIABLE: Function<unsafe extern "C" fn(*mut ffi::cvar::cvar_s)> =
    Function::empty();
pub static CVAR_VARS: Variable<*mut ffi::cvar::cvar_s> = Variable::empty();
pub static HOST_SHUTDOWN: Function<unsafe extern "C" fn()> = Function::empty();
pub static MEMORY_INIT: Function<unsafe extern "C" fn(*mut c_void, c_int) -> c_int> =
    Function::empty();
pub static MEM_FREE: Function<unsafe extern "C" fn(*mut c_void)> = Function::empty();
pub static V_FADEALPHA: Function<unsafe extern "C" fn() -> c_int> = Function::empty();
pub static Z_FREE: Function<unsafe extern "C" fn(*mut c_void)> = Function::empty();

/// Wrapper providing safe access to some engine functions.
///
/// This can be seen as a slightly stronger variant of `MainThreadMarker`. While `MainThreadMarker`
/// merely guarantees being on the main thread, `Engine` also guarantees the ability to call
/// certain engine functions.
// No Clone or Copy ensures that if an Engine is given by reference to some function, the function
// cannot store the Engine in a global variable and access it later when it's unsafe to do so.
pub struct Engine {
    marker: MainThreadMarker,
}

impl Engine {
    /// Creates a new `Engine`.
    ///
    /// # Safety
    ///
    /// All `Engine` methods must be safe to call over the whole lifespan of the `Engine` returned
    /// by this function.
    pub unsafe fn new(marker: MainThreadMarker) -> Self {
        Self { marker }
    }

    /// Returns a `MainThreadMarker`.
    pub fn marker(&self) -> MainThreadMarker {
        self.marker
    }

    /// Prints the string to the console.
    ///
    /// If `Con_Printf` was not found, does nothing.
    ///
    /// # Panics
    ///
    /// Panics if the string cannot be converted to a `CString`.
    pub fn print(&self, s: &str) {
        if !CON_PRINTF.is_set(self.marker) {
            return;
        }

        let s = CString::new(s).unwrap();
        unsafe {
            CON_PRINTF.get(self.marker)(b"%s\0".as_ptr().cast(), s.as_ptr());
        }
    }
}

fn find_pointers(marker: MainThreadMarker) {
    let handle = dl::open("hw.so").unwrap();

    unsafe {
        CMD_ADDMALLOCCOMMAND.set(marker, handle.sym("Cmd_AddMallocCommand").ok());
        CMD_ARGC.set(marker, handle.sym("Cmd_Argc").ok());
        CMD_ARGV.set(marker, handle.sym("Cmd_Argv").ok());
        CMD_FUNCTIONS.set(marker, handle.sym("cmd_functions").ok());
        CON_PRINTF.set(marker, handle.sym("Con_Printf").ok());
        CVAR_REGISTERVARIABLE.set(marker, handle.sym("Cvar_RegisterVariable").ok());
        CVAR_VARS.set(marker, handle.sym("cvar_vars").ok());
        HOST_SHUTDOWN.set(marker, handle.sym("Host_Shutdown").ok());
        MEMORY_INIT.set(marker, handle.sym("Memory_Init").ok());
        MEM_FREE.set(marker, handle.sym("Mem_Free").ok());
        V_FADEALPHA.set(marker, handle.sym("V_FadeAlpha").ok());
        Z_FREE.set(marker, handle.sym("Z_Free").ok());
    }
}

fn reset_pointers(marker: MainThreadMarker) {
    CMD_ADDMALLOCCOMMAND.reset(marker);
    CMD_ARGC.reset(marker);
    CMD_ARGV.reset(marker);
    CMD_FUNCTIONS.reset(marker);
    CON_PRINTF.reset(marker);
    CVAR_REGISTERVARIABLE.reset(marker);
    CVAR_VARS.reset(marker);
    HOST_SHUTDOWN.reset(marker);
    MEMORY_INIT.reset(marker);
    MEM_FREE.reset(marker);
    V_FADEALPHA.reset(marker);
    Z_FREE.reset(marker);
}

#[no_mangle]
pub unsafe extern "C" fn Memory_Init(buf: *mut c_void, size: c_int) -> c_int {
    abort_on_panic(move || {
        let marker = MainThreadMarker::new();

        let _ = pretty_env_logger::try_init();

        find_pointers(marker);

        let rv = MEMORY_INIT.get(marker)(buf, size);

        cvars::register_all_cvars(marker);
        commands::register_all_commands(marker);
        cvars::deregister_disabled_module_cvars(marker);
        commands::deregister_disabled_module_commands(marker);

        rv
    })
}

#[no_mangle]
pub unsafe extern "C" fn Host_Shutdown() {
    abort_on_panic(move || {
        let marker = MainThreadMarker::new();

        commands::deregister_all_commands(marker);

        HOST_SHUTDOWN.get(marker)();

        cvars::mark_all_cvars_as_not_registered(marker);

        reset_pointers(marker);
    })
}

#[no_mangle]
pub unsafe extern "C" fn V_FadeAlpha() -> c_int {
    abort_on_panic(move || {
        let marker = MainThreadMarker::new();

        if fade_remove::is_active(marker) {
            0
        } else {
            V_FADEALPHA.get(marker)()
        }
    })
}
