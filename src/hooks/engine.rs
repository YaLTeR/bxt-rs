//! `hw`, `sw`, `hl`.

use std::{ffi::CString, os::raw::*, ptr::null_mut};

use bxt_patterns::Patterns;

use crate::{
    ffi::{command::cmd_function_s, cvar::cvar_s, playermove::playermove_s, usercmd::usercmd_s},
    hooks::server,
    modules::{commands, cvars, fade_remove, tas_logging},
    utils::*,
};

pub static BUILD_NUMBER: Pointer<unsafe extern "C" fn() -> c_int> = Pointer::empty_patterns(
    b"build_number\0",
    // To find, search for "Half-Life %i/%s (hw build %d)". This function is
    // Draw_ConsoleBackground(), and a call to build_number() is right above the snprintf() using
    // this string.
    Patterns(&[
        // 6153
        pattern!(55 8B EC 83 EC 08 A1 ?? ?? ?? ?? 56 33 F6 85 C0),
    ]),
    null_mut(),
);
pub static CLS: Pointer<*mut c_void> = Pointer::empty(b"cls\0");
pub static CMD_ADDMALLOCCOMMAND: Pointer<
    unsafe extern "C" fn(*const c_char, unsafe extern "C" fn(), c_int),
> = Pointer::empty_patterns(
    b"Cmd_AddMallocCommand\0",
    // To find, search for "Cmd_AddCommand: %s already defined as a var". It will give two results,
    // one of them for Cmd_AddCommandWithFlags, another for Cmd_AddMallocCommand.
    // Cmd_AddMallocCommand is slightly smaller, and the allocation call in the middle that takes
    // 0x10 as a parameter calls malloc internally. This allocation call is Mem_ZeroMalloc.
    Patterns(&[
        // 6153
        pattern!(55 8B EC 56 57 8B 7D ?? 57 E8 ?? ?? ?? ?? 8A 08),
    ]),
    null_mut(),
);
pub static CMD_ARGC: Pointer<unsafe extern "C" fn() -> c_int> = Pointer::empty(b"Cmd_Argc\0");
pub static CMD_ARGV: Pointer<unsafe extern "C" fn(c_int) -> *const c_char> =
    Pointer::empty(b"Cmd_Argv\0");
pub static CMD_FUNCTIONS: Pointer<*mut *mut cmd_function_s> = Pointer::empty(b"cmd_functions\0");
pub static CON_PRINTF: Pointer<unsafe extern "C" fn(*const c_char, ...)> = Pointer::empty_patterns(
    b"Con_Printf\0",
    // To find, search for "qconsole.log". One of the three usages is Con_Printf (the one that
    // isn't just many function calls or OutputDebugStringA).
    Patterns(&[
        // 6153
        pattern!(55 8B EC B8 00 10 00 00 E8 ?? ?? ?? ?? 8B 4D),
    ]),
    null_mut(),
);
pub static COM_GAMEDIR: Pointer<*mut [c_char; 260]> = Pointer::empty(b"com_gamedir\0");
pub static CVAR_REGISTERVARIABLE: Pointer<unsafe extern "C" fn(*mut cvar_s)> =
    Pointer::empty_patterns(
        b"Cvar_RegisterVariable\0",
        // To find, search for "Can't register variable %s, already defined".
        Patterns(&[
            // 6153
            pattern!(55 8B EC 83 EC 14 53 56 8B 75 ?? 57 8B 06),
        ]),
        null_mut(),
    );
pub static CVAR_VARS: Pointer<*mut *mut cvar_s> = Pointer::empty(b"cvar_vars\0");
pub static GENTITYINTERFACE: Pointer<*mut DllFunctions> = Pointer::empty(b"gEntityInterface\0");
pub static LOADENTITYDLLS: Pointer<unsafe extern "C" fn(*const c_char)> = Pointer::empty_patterns(
    b"LoadEntityDLLs\0",
    // To find, search for "GetNewDLLFunctions".
    Patterns(&[
        // 6153
        pattern!(55 8B EC B8 90 23 00 00),
    ]),
    LoadEntityDLLs as _,
);
pub static HOST_FRAMETIME: Pointer<*mut c_double> = Pointer::empty(b"host_frametime\0");
pub static HOST_INITIALIZEGAMEDLL: Pointer<unsafe extern "C" fn()> = Pointer::empty_patterns(
    b"Host_InitializeGameDLL\0",
    // To find, search for "Sys_InitializeGameDLL called twice, skipping second call".
    // Alternatively, find LoadEntityDLLs() and go to the parent function.
    Patterns(&[
        // 6153
        pattern!(E8 ?? ?? ?? ?? 8B 0D ?? ?? ?? ?? 33 C0),
    ]),
    null_mut(),
);
pub static HOST_SHUTDOWN: Pointer<unsafe extern "C" fn()> = Pointer::empty_patterns(
    b"Host_Shutdown\0",
    // To find, search for "recursive shutdown".
    Patterns(&[
        // 6153
        pattern!(A1 ?? ?? ?? ?? 53 33 DB 3B C3 74 ?? 68),
    ]),
    Host_Shutdown as _,
);
pub static HOST_TELL_F: Pointer<unsafe extern "C" fn()> = Pointer::empty_patterns(
    b"Host_Tell_f\0",
    // To find, search for "%s TELL: ".
    Patterns(&[
        // 6153
        pattern!(55 8B EC 83 EC 40 A1 ?? ?? ?? ?? 56),
    ]),
    null_mut(),
);
pub static HOST_VALIDSAVE: Pointer<unsafe extern "C" fn() -> c_int> = Pointer::empty_patterns(
    b"Host_ValidSave\0",
    // To find, search for "Not playing a local game.".
    Patterns(&[
        // 6153
        pattern!(A1 ?? ?? ?? ?? B9 01 00 00 00 3B C1 0F 85),
    ]),
    null_mut(),
);
pub static MEMORY_INIT: Pointer<unsafe extern "C" fn(*mut c_void, c_int) -> c_int> =
    Pointer::empty_patterns(
        b"Memory_Init\0",
        // To find, search for "Memory_Init".
        Patterns(&[
            // 6153
            pattern!(55 8B EC 8B 45 ?? 8B 4D ?? 56 BE 00 00 20 00),
        ]),
        Memory_Init as _,
    );
pub static MEM_FREE: Pointer<unsafe extern "C" fn(*mut c_void)> = Pointer::empty_patterns(
    b"Mem_Free\0",
    // Mem_Free is called once in Host_Shutdown to free a pointer after checking that it's != 0. On
    // Windows, it dispatches directly to an underlying function, and the pattern is for the
    // underlying function.
    Patterns(&[
        // 6153
        pattern!(55 8B EC 6A FF 68 ?? ?? ?? ?? 68 ?? ?? ?? ?? 64 A1 ?? ?? ?? ?? 50 64 89 25 ?? ?? ?? ?? 83 EC 18 53 56 57 8B 75 ?? 85 F6),
    ]),
    null_mut(),
);
pub static RELEASEENTITYDLLS: Pointer<unsafe extern "C" fn()> = Pointer::empty_patterns(
    b"ReleaseEntityDlls\0",
    // Find Host_Shutdown(). It has a Mem_Free() if. The 3-rd function above that if is
    // ReleaseEntityDlls().
    Patterns(&[
        // 6153
        pattern!(A1 ?? ?? ?? ?? 56 57 BE ?? ?? ?? ?? 8D 04),
    ]),
    ReleaseEntityDlls as _,
);
pub static SV: Pointer<*mut c_void> = Pointer::empty(b"sv\0");
pub static SV_FRAME: Pointer<unsafe extern "C" fn()> = Pointer::empty_patterns(
    b"SV_Frame\0",
    // To find, search for "%s timed out". It is used in SV_CheckTimeouts(), which is called by
    // SV_Frame().
    Patterns(&[
        // 6153
        pattern!(A1 ?? ?? ?? ?? 85 C0 74 ?? DD 05 ?? ?? ?? ?? A1),
    ]),
    SV_Frame as _,
);
pub static V_FADEALPHA: Pointer<unsafe extern "C" fn() -> c_int> = Pointer::empty_patterns(
    b"V_FadeAlpha\0",
    // To find, search for "%3ifps %3i ms  %4i wpoly %4i epoly". This will lead to either
    // R_RenderView() or its usually-inlined part, and the string will be used within an if. Right
    // above the if is S_ExtraUpdate(), and right above that (maybe in another if) is
    // R_PolyBlend(). Inside R_PolyBlend(), the first call is V_FadeAlpha().
    Patterns(&[
        // 6153
        pattern!(55 8B EC 83 EC 08 D9 05 ?? ?? ?? ?? DC 1D),
    ]),
    V_FadeAlpha as _,
);
pub static Z_FREE: Pointer<unsafe extern "C" fn(*mut c_void)> = Pointer::empty_patterns(
    b"Z_Free\0",
    // To find, search for "Z_Free: NULL pointer".
    Patterns(&[
        pattern!(55 8B EC 56 8B 75 ?? 85 F6 57 75 ?? 68 ?? ?? ?? ?? E8 ?? ?? ?? ?? 83 C4 04 8B 46),
    ]),
    null_mut(),
);

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
    &HOST_INITIALIZEGAMEDLL,
    &HOST_SHUTDOWN,
    &HOST_TELL_F,
    &HOST_VALIDSAVE,
    &MEMORY_INIT,
    &MEM_FREE,
    &RELEASEENTITYDLLS,
    &SV,
    &SV_FRAME,
    &V_FADEALPHA,
    &Z_FREE,
];

#[cfg(windows)]
static ORIGINAL_FUNCTIONS: MainThreadRefCell<Vec<*mut c_void>> = MainThreadRefCell::new(Vec::new());

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
            pointer.set(marker, handle.sym(pointer.symbol()).ok());
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
pub unsafe fn find_pointers(marker: MainThreadMarker, base: *mut c_void, size: usize) {
    use std::{ptr::NonNull, slice};

    use minhook_sys::*;

    // Find all pattern-based pointers.
    {
        let memory = slice::from_raw_parts(base.cast(), size);
        for pointer in POINTERS {
            if let Some((offset, index)) = pointer.patterns().find(memory) {
                pointer.set_with_index(
                    marker,
                    NonNull::new_unchecked(base.add(offset)),
                    Some(index),
                );
            }
        }
    }

    // Find all offset-based pointers.
    match CMD_ADDMALLOCCOMMAND.pattern_index(marker) {
        // 6153
        Some(0) => CMD_FUNCTIONS.set(marker, CMD_ADDMALLOCCOMMAND.by_offset(marker, 43)),
        _ => (),
    }

    match CVAR_REGISTERVARIABLE.pattern_index(marker) {
        // 6153
        Some(0) => CVAR_VARS.set(marker, CVAR_REGISTERVARIABLE.by_offset(marker, 124)),
        _ => (),
    }

    match HOST_INITIALIZEGAMEDLL.pattern_index(marker) {
        // 6153
        Some(0) => {
            // SVS.set(marker, HOST_INITIALIZEGAMEDLL.by_relative_call(marker, 26));
            if !LOADENTITYDLLS.is_set(marker) {
                LOADENTITYDLLS.set(marker, HOST_INITIALIZEGAMEDLL.by_relative_call(marker, 69));
            }
            GENTITYINTERFACE.set(marker, HOST_INITIALIZEGAMEDLL.by_offset(marker, 75));
        }
        _ => (),
    }

    match HOST_TELL_F.pattern_index(marker) {
        // 6153
        Some(0) => {
            CMD_ARGC.set(marker, HOST_TELL_F.by_relative_call(marker, 28));
            CMD_ARGV.set(marker, HOST_TELL_F.by_relative_call(marker, 145));
        }
        _ => (),
    }

    match HOST_VALIDSAVE.pattern_index(marker) {
        // 6153
        Some(0) => {
            SV.set(marker, HOST_VALIDSAVE.by_offset(marker, 19));
            CLS.set(marker, HOST_VALIDSAVE.by_offset(marker, 69));
            if !CON_PRINTF.is_set(marker) {
                CON_PRINTF.set(marker, HOST_VALIDSAVE.by_relative_call(marker, 33));
            }
        }
        _ => (),
    }

    match LOADENTITYDLLS.pattern_index(marker) {
        // 6153
        Some(0) => {
            COM_GAMEDIR.set(marker, LOADENTITYDLLS.by_offset(marker, 51));
        }
        _ => (),
    }

    match RELEASEENTITYDLLS.pattern_index(marker) {
        // 6153
        Some(0) => {
            // SVS.set(marker, RELEASEENTITYDLLS.by_offset(marker, 23));
        }
        _ => (),
    }

    match SV_FRAME.pattern_index(marker) {
        // 6153
        Some(0) => {
            SV.set(marker, SV_FRAME.by_offset(marker, 1));
            HOST_FRAMETIME.set(marker, SV_FRAME.by_offset(marker, 11));
        }
        _ => (),
    }

    // Hook all found pointers.
    for pointer in POINTERS {
        pointer.log(marker);

        if !pointer.is_set(marker) {
            continue;
        }

        let hook_fn = pointer.hook_fn();
        if hook_fn.is_null() {
            continue;
        }

        let original = pointer.get_raw(marker);
        let mut trampoline = null_mut();
        assert!(MH_CreateHook(original.as_ptr(), hook_fn, &mut trampoline,) == MH_OK);

        // Store the original pointer to be able to remove the hook later.
        ORIGINAL_FUNCTIONS
            .borrow_mut(marker)
            .push(original.as_ptr());

        // Store the trampoline pointer which is used to call the original function.
        pointer.set_with_index(
            marker,
            NonNull::new_unchecked(trampoline),
            pointer.pattern_index(marker),
        );

        assert!(MH_EnableHook(original.as_ptr()) == MH_OK);
    }
}

fn reset_pointers(marker: MainThreadMarker) {
    for pointer in POINTERS {
        pointer.reset(marker);
    }

    // Remove all hooks.
    #[cfg(windows)]
    {
        use minhook_sys::*;

        for function in ORIGINAL_FUNCTIONS.borrow_mut(marker).drain(..) {
            assert!(unsafe { MH_RemoveHook(function) } == MH_OK);
        }
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
