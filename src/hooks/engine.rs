//! `hw`, `sw`, `hl`.

use std::{ffi::CString, os::raw::*};

use crate::{
    ffi::{command::cmd_function_s, cvar::cvar_s, playermove::playermove_s, usercmd::usercmd_s},
    hooks::server,
    modules::{commands, cvars, fade_remove, tas_logging},
    utils::*,
};

pub static BUILD_NUMBER: Pointer<unsafe extern "C" fn() -> c_int> =
    Pointer::empty(b"build_number\0");
pub static CLS: Pointer<*mut c_void> = Pointer::empty(b"cls\0");
pub static CMD_ADDMALLOCCOMMAND: Pointer<
    unsafe extern "C" fn(*const c_char, unsafe extern "C" fn(), c_int),
> = Pointer::empty(b"Cmd_AddMallocCommand\0");
pub static CMD_ARGC: Pointer<unsafe extern "C" fn() -> c_int> = Pointer::empty(b"Cmd_Argc\0");
pub static CMD_ARGV: Pointer<unsafe extern "C" fn(c_int) -> *const c_char> =
    Pointer::empty(b"Cmd_Argv\0");
pub static CMD_FUNCTIONS: Pointer<*mut *mut cmd_function_s> = Pointer::empty(b"cmd_functions\0");
pub static CON_PRINTF: Pointer<unsafe extern "C" fn(*const c_char, ...)> =
    Pointer::empty(b"Con_Printf\0");
pub static COM_GAMEDIR: Pointer<*mut [c_char; 260]> = Pointer::empty(b"com_gamedir\0");
pub static CVAR_REGISTERVARIABLE: Pointer<unsafe extern "C" fn(*mut cvar_s)> =
    Pointer::empty(b"Cvar_RegisterVariable\0");
pub static CVAR_VARS: Pointer<*mut *mut cvar_s> = Pointer::empty(b"cvar_vars\0");
pub static GENTITYINTERFACE: Pointer<*mut DllFunctions> = Pointer::empty(b"gEntityInterface\0");
pub static LOADENTITYDLLS: Pointer<unsafe extern "C" fn(*const c_char)> =
    Pointer::empty(b"LoadEntityDLLs\0");
pub static HOST_FRAMETIME: Pointer<*mut c_double> = Pointer::empty(b"host_frametime\0");
pub static HOST_SHUTDOWN: Pointer<unsafe extern "C" fn()> = Pointer::empty(b"Host_Shutdown\0");
pub static MEMORY_INIT: Pointer<unsafe extern "C" fn(*mut c_void, c_int) -> c_int> =
    Pointer::empty(b"Memory_Init\0");
pub static MEM_FREE: Pointer<unsafe extern "C" fn(*mut c_void)> = Pointer::empty(b"Mem_Free\0");
pub static RELEASEENTITYDLLS: Pointer<unsafe extern "C" fn()> =
    Pointer::empty(b"ReleaseEntityDlls\0");
pub static SV: Pointer<*mut c_void> = Pointer::empty(b"sv\0");
pub static SV_FRAME: Pointer<unsafe extern "C" fn()> = Pointer::empty(b"SV_Frame\0");
pub static V_FADEALPHA: Pointer<unsafe extern "C" fn() -> c_int> = Pointer::empty(b"V_FadeAlpha\0");
pub static Z_FREE: Pointer<unsafe extern "C" fn(*mut c_void)> = Pointer::empty(b"Z_Free\0");

static POINTERS: &[&dyn PointerTrait] = &[
    &BUILD_NUMBER,
    &CLS,
    &CMD_ADDMALLOCCOMMAND,
    &CMD_ARGC,
    &CMD_ARGV,
    &CMD_FUNCTIONS,
    &CON_PRINTF,
    &COM_GAMEDIR,
    &CVAR_REGISTERVARIABLE,
    &CVAR_VARS,
    &GENTITYINTERFACE,
    &LOADENTITYDLLS,
    &HOST_FRAMETIME,
    &HOST_SHUTDOWN,
    &MEMORY_INIT,
    &MEM_FREE,
    &RELEASEENTITYDLLS,
    &SV,
    &SV_FRAME,
    &V_FADEALPHA,
    &Z_FREE,
];

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

#[cfg(unix)]
fn find_pointers(marker: MainThreadMarker) {
    let handle = dl::open("hw.so").unwrap();

    for pointer in POINTERS {
        unsafe {
            pointer.set(marker, handle.sym(pointer.symbol()).ok(), None);
        }

        pointer.log(marker);
    }
}

/// # Safety
///
/// The memory starting at `base` with size `size` must be valid to read and not modified over the
/// duration of this call. If any pointers are found in memory, then the memory must be valid until
/// the pointers are reset (according to the safety section of `PointerTrait::set`).
#[cfg(windows)]
pub unsafe fn find_pointers(_marker: MainThreadMarker, _base: *mut c_void, _size: usize) {}

fn reset_pointers(marker: MainThreadMarker) {
    for pointer in POINTERS {
        pointer.reset(marker);
    }
}

#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "C" fn Memory_Init(buf: *mut c_void, size: c_int) -> c_int {
    abort_on_panic(move || {
        let marker = MainThreadMarker::new();

        #[cfg(unix)]
        let _ = env_logger::try_init();

        #[cfg(unix)]
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
