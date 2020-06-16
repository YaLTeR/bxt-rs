//! `hw`, `sw`, `hl`.

use std::{ffi::CString, os::raw::*};

use crate::{
    ffi::{command::cmd_function_s, cvar::cvar_s, playermove::playermove_s, usercmd::usercmd_s},
    hooks::server,
    modules::{commands, cvars, fade_remove, tas_logging},
    utils::{abort_on_panic, dl, MainThreadMarker, Pointer},
};

pub static BUILD_NUMBER: Pointer<unsafe extern "C" fn() -> c_int> = Pointer::empty();
pub static CLS: Pointer<*mut c_void> = Pointer::empty();
pub static CMD_ADDMALLOCCOMMAND: Pointer<
    unsafe extern "C" fn(*const c_char, unsafe extern "C" fn(), c_int),
> = Pointer::empty();
pub static CMD_ARGC: Pointer<unsafe extern "C" fn() -> c_int> = Pointer::empty();
pub static CMD_ARGV: Pointer<unsafe extern "C" fn(c_int) -> *const c_char> = Pointer::empty();
pub static CMD_FUNCTIONS: Pointer<*mut *mut cmd_function_s> = Pointer::empty();
pub static CON_PRINTF: Pointer<unsafe extern "C" fn(*const c_char, ...)> = Pointer::empty();
pub static COM_GAMEDIR: Pointer<*mut [c_char; 260]> = Pointer::empty();
pub static CVAR_REGISTERVARIABLE: Pointer<unsafe extern "C" fn(*mut cvar_s)> = Pointer::empty();
pub static CVAR_VARS: Pointer<*mut *mut cvar_s> = Pointer::empty();
pub static GENTITYINTERFACE: Pointer<*mut DllFunctions> = Pointer::empty();
pub static LOADENTITYDLLS: Pointer<unsafe extern "C" fn(*const c_char)> = Pointer::empty();
pub static HOST_FRAMETIME: Pointer<*mut c_double> = Pointer::empty();
pub static HOST_SHUTDOWN: Pointer<unsafe extern "C" fn()> = Pointer::empty();
pub static MEMORY_INIT: Pointer<unsafe extern "C" fn(*mut c_void, c_int) -> c_int> =
    Pointer::empty();
pub static MEM_FREE: Pointer<unsafe extern "C" fn(*mut c_void)> = Pointer::empty();
pub static RELEASEENTITYDLLS: Pointer<unsafe extern "C" fn()> = Pointer::empty();
pub static SV: Pointer<*mut c_void> = Pointer::empty();
pub static SV_FRAME: Pointer<unsafe extern "C" fn()> = Pointer::empty();
pub static V_FADEALPHA: Pointer<unsafe extern "C" fn() -> c_int> = Pointer::empty();
pub static Z_FREE: Pointer<unsafe extern "C" fn(*mut c_void)> = Pointer::empty();

#[repr(C)]
pub struct DllFunctions {
    _padding_1: [u8; 136],
    pub pm_move: Option<unsafe extern "C" fn(*mut playermove_s, c_int)>,
    _padding_2: [u8; 32],
    pub cmd_start: Option<unsafe extern "C" fn(*mut c_void, *mut usercmd_s, c_uint)>,
}

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
        BUILD_NUMBER.set(marker, handle.sym(b"build_number\0").ok());
        CLS.set(marker, handle.sym(b"cls\0").ok());
        CMD_ADDMALLOCCOMMAND.set(marker, handle.sym(b"Cmd_AddMallocCommand\0").ok());
        CMD_ARGC.set(marker, handle.sym(b"Cmd_Argc\0").ok());
        CMD_ARGV.set(marker, handle.sym(b"Cmd_Argv\0").ok());
        CMD_FUNCTIONS.set(marker, handle.sym(b"cmd_functions\0").ok());
        COM_GAMEDIR.set(marker, handle.sym(b"com_gamedir\0").ok());
        CON_PRINTF.set(marker, handle.sym(b"Con_Printf\0").ok());
        CVAR_REGISTERVARIABLE.set(marker, handle.sym(b"Cvar_RegisterVariable\0").ok());
        CVAR_VARS.set(marker, handle.sym(b"cvar_vars\0").ok());
        GENTITYINTERFACE.set(marker, handle.sym(b"gEntityInterface\0").ok());
        LOADENTITYDLLS.set(marker, handle.sym(b"LoadEntityDLLs\0").ok());
        HOST_FRAMETIME.set(marker, handle.sym(b"host_frametime\0").ok());
        HOST_SHUTDOWN.set(marker, handle.sym(b"Host_Shutdown\0").ok());
        MEMORY_INIT.set(marker, handle.sym(b"Memory_Init\0").ok());
        MEM_FREE.set(marker, handle.sym(b"Mem_Free\0").ok());
        RELEASEENTITYDLLS.set(marker, handle.sym(b"ReleaseEntityDlls\0").ok());
        SV.set(marker, handle.sym(b"sv\0").ok());
        SV_FRAME.set(marker, handle.sym(b"SV_Frame\0").ok());
        V_FADEALPHA.set(marker, handle.sym(b"V_FadeAlpha\0").ok());
        Z_FREE.set(marker, handle.sym(b"Z_Free\0").ok());
    }
}

fn reset_pointers(marker: MainThreadMarker) {
    BUILD_NUMBER.reset(marker);
    CLS.reset(marker);
    CMD_ADDMALLOCCOMMAND.reset(marker);
    CMD_ARGC.reset(marker);
    CMD_ARGV.reset(marker);
    CMD_FUNCTIONS.reset(marker);
    COM_GAMEDIR.reset(marker);
    CON_PRINTF.reset(marker);
    CVAR_REGISTERVARIABLE.reset(marker);
    CVAR_VARS.reset(marker);
    GENTITYINTERFACE.reset(marker);
    LOADENTITYDLLS.reset(marker);
    HOST_FRAMETIME.reset(marker);
    HOST_SHUTDOWN.reset(marker);
    MEMORY_INIT.reset(marker);
    MEM_FREE.reset(marker);
    RELEASEENTITYDLLS.reset(marker);
    SV.reset(marker);
    SV_FRAME.reset(marker);
    V_FADEALPHA.reset(marker);
    Z_FREE.reset(marker);
}

#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "C" fn Memory_Init(buf: *mut c_void, size: c_int) -> c_int {
    abort_on_panic(move || {
        let marker = MainThreadMarker::new();

        let _ = env_logger::try_init();

        find_pointers(marker);

        let rv = MEMORY_INIT.get(marker)(buf, size);

        cvars::register_all_cvars(marker);
        commands::register_all_commands(marker);
        cvars::deregister_disabled_module_cvars(marker);
        commands::deregister_disabled_module_commands(marker);

        rv
    })
}

#[allow(clippy::missing_safety_doc)]
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

#[allow(clippy::missing_safety_doc)]
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

#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "C" fn SV_Frame() {
    abort_on_panic(move || {
        let marker = MainThreadMarker::new();
        let engine = Engine::new(marker);

        tas_logging::on_sv_frame_start(&engine);

        SV_FRAME.get(marker)();

        tas_logging::on_sv_frame_end(&engine);
    })
}

#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "C" fn ReleaseEntityDlls() {
    abort_on_panic(move || {
        let marker = MainThreadMarker::new();

        server::reset_entity_interface(marker);

        RELEASEENTITYDLLS.get(marker)();
    })
}

#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "C" fn LoadEntityDLLs(base_dir: *const c_char) {
    abort_on_panic(move || {
        let marker = MainThreadMarker::new();

        LOADENTITYDLLS.get(marker)(base_dir);

        server::hook_entity_interface(marker);
    })
}
