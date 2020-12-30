//! `hw`, `sw`, `hl`.

#![allow(non_snake_case, non_upper_case_globals)]

use std::{
    os::raw::*,
    ptr::{null_mut, NonNull},
};

use bxt_macros::pattern;
use bxt_patterns::Patterns;

use crate::{
    ffi::{command::cmd_function_s, cvar::cvar_s, playermove::playermove_s, usercmd::usercmd_s},
    hooks::{sdl, server},
    modules::{commands, cvars, fade_remove, tas_logging},
    utils::*,
};

pub static build_number: Pointer<unsafe extern "C" fn() -> c_int> = Pointer::empty_patterns(
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
pub static cls: Pointer<*mut client_static_s> = Pointer::empty(b"cls\0");
pub static Cmd_AddMallocCommand: Pointer<
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
pub static Cmd_Argc: Pointer<unsafe extern "C" fn() -> c_int> = Pointer::empty(b"Cmd_Argc\0");
pub static Cmd_Argv: Pointer<unsafe extern "C" fn(c_int) -> *const c_char> =
    Pointer::empty(b"Cmd_Argv\0");
pub static cmd_functions: Pointer<*mut *mut cmd_function_s> = Pointer::empty(b"cmd_functions\0");
pub static Con_Printf: Pointer<unsafe extern "C" fn(*const c_char, ...)> = Pointer::empty_patterns(
    b"Con_Printf\0",
    // To find, search for "qconsole.log". One of the three usages is Con_Printf (the one that
    // isn't just many function calls or OutputDebugStringA).
    Patterns(&[
        // 6153
        pattern!(55 8B EC B8 00 10 00 00 E8 ?? ?? ?? ?? 8B 4D),
    ]),
    null_mut(),
);
pub static com_gamedir: Pointer<*mut [c_char; 260]> = Pointer::empty(b"com_gamedir\0");
pub static Cvar_RegisterVariable: Pointer<unsafe extern "C" fn(*mut cvar_s)> =
    Pointer::empty_patterns(
        b"Cvar_RegisterVariable\0",
        // To find, search for "Can't register variable %s, already defined".
        Patterns(&[
            // 6153
            pattern!(55 8B EC 83 EC 14 53 56 8B 75 ?? 57 8B 06),
        ]),
        null_mut(),
    );
pub static cvar_vars: Pointer<*mut *mut cvar_s> = Pointer::empty(b"cvar_vars\0");
pub static gEntityInterface: Pointer<*mut DllFunctions> = Pointer::empty(b"gEntityInterface\0");
pub static LoadEntityDLLs: Pointer<unsafe extern "C" fn(*const c_char)> = Pointer::empty_patterns(
    b"LoadEntityDLLs\0",
    // To find, search for "GetNewDLLFunctions".
    Patterns(&[
        // 6153
        pattern!(55 8B EC B8 90 23 00 00),
    ]),
    my_LoadEntityDLLs as _,
);
pub static host_frametime: Pointer<*mut c_double> = Pointer::empty(b"host_frametime\0");
pub static Host_InitializeGameDLL: Pointer<unsafe extern "C" fn()> = Pointer::empty_patterns(
    b"Host_InitializeGameDLL\0",
    // To find, search for "Sys_InitializeGameDLL called twice, skipping second call".
    // Alternatively, find LoadEntityDLLs() and go to the parent function.
    Patterns(&[
        // 6153
        pattern!(E8 ?? ?? ?? ?? 8B 0D ?? ?? ?? ?? 33 C0),
    ]),
    null_mut(),
);
pub static Host_Shutdown: Pointer<unsafe extern "C" fn()> = Pointer::empty_patterns(
    b"Host_Shutdown\0",
    // To find, search for "recursive shutdown".
    Patterns(&[
        // 6153
        pattern!(A1 ?? ?? ?? ?? 53 33 DB 3B C3 74 ?? 68),
    ]),
    my_Host_Shutdown as _,
);
pub static Host_Tell_f: Pointer<unsafe extern "C" fn()> = Pointer::empty_patterns(
    b"Host_Tell_f\0",
    // To find, search for "%s TELL: ".
    Patterns(&[
        // 6153
        pattern!(55 8B EC 83 EC 40 A1 ?? ?? ?? ?? 56),
    ]),
    null_mut(),
);
pub static Host_ValidSave: Pointer<unsafe extern "C" fn() -> c_int> = Pointer::empty_patterns(
    b"Host_ValidSave\0",
    // To find, search for "Not playing a local game.".
    Patterns(&[
        // 6153
        pattern!(A1 ?? ?? ?? ?? B9 01 00 00 00 3B C1 0F 85),
    ]),
    null_mut(),
);
pub static Memory_Init: Pointer<unsafe extern "C" fn(*mut c_void, c_int) -> c_int> =
    Pointer::empty_patterns(
        b"Memory_Init\0",
        // To find, search for "Memory_Init".
        Patterns(&[
            // 6153
            pattern!(55 8B EC 8B 45 ?? 8B 4D ?? 56 BE 00 00 20 00),
        ]),
        my_Memory_Init as _,
    );
pub static Mem_Free: Pointer<unsafe extern "C" fn(*mut c_void)> = Pointer::empty_patterns(
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
pub static ReleaseEntityDlls: Pointer<unsafe extern "C" fn()> = Pointer::empty_patterns(
    b"ReleaseEntityDlls\0",
    // Find Host_Shutdown(). It has a Mem_Free() if. The 3-rd function above that if is
    // ReleaseEntityDlls().
    Patterns(&[
        // 6153
        pattern!(A1 ?? ?? ?? ?? 56 57 BE ?? ?? ?? ?? 8D 04),
    ]),
    my_ReleaseEntityDlls as _,
);
pub static sv: Pointer<*mut c_void> = Pointer::empty(b"sv\0");
pub static SV_Frame: Pointer<unsafe extern "C" fn()> = Pointer::empty_patterns(
    b"SV_Frame\0",
    // To find, search for "%s timed out". It is used in SV_CheckTimeouts(), which is called by
    // SV_Frame().
    Patterns(&[
        // 6153
        pattern!(A1 ?? ?? ?? ?? 85 C0 74 ?? DD 05 ?? ?? ?? ?? A1),
    ]),
    my_SV_Frame as _,
);
pub static V_FadeAlpha: Pointer<unsafe extern "C" fn() -> c_int> = Pointer::empty_patterns(
    b"V_FadeAlpha\0",
    // To find, search for "%3ifps %3i ms  %4i wpoly %4i epoly". This will lead to either
    // R_RenderView() or its usually-inlined part, and the string will be used within an if. Right
    // above the if is S_ExtraUpdate(), and right above that (maybe in another if) is
    // R_PolyBlend(). Inside R_PolyBlend(), the first call is V_FadeAlpha().
    Patterns(&[
        // 6153
        pattern!(55 8B EC 83 EC 08 D9 05 ?? ?? ?? ?? DC 1D),
    ]),
    my_V_FadeAlpha as _,
);
pub static Z_Free: Pointer<unsafe extern "C" fn(*mut c_void)> = Pointer::empty_patterns(
    b"Z_Free\0",
    // To find, search for "Z_Free: NULL pointer".
    Patterns(&[
        pattern!(55 8B EC 56 8B 75 ?? 85 F6 57 75 ?? 68 ?? ?? ?? ?? E8 ?? ?? ?? ?? 83 C4 04 8B 46),
    ]),
    null_mut(),
);

static POINTERS: &[&dyn PointerTrait] = &[
    &build_number,
    &cls,
    &Cmd_AddMallocCommand,
    &Cmd_Argc,
    &Cmd_Argv,
    &cmd_functions,
    &Con_Printf,
    &com_gamedir,
    &Cvar_RegisterVariable,
    &cvar_vars,
    &gEntityInterface,
    &LoadEntityDLLs,
    &host_frametime,
    &Host_InitializeGameDLL,
    &Host_Shutdown,
    &Host_Tell_f,
    &Host_ValidSave,
    &Memory_Init,
    &Mem_Free,
    &ReleaseEntityDlls,
    &sv,
    &SV_Frame,
    &V_FadeAlpha,
    &Z_Free,
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

#[repr(C)]
pub struct client_static_s {
    pub state: c_int,
}

/// Prints the string to the console.
///
/// If `Con_Printf` was not found, does nothing.
///
/// Any null-bytes are replaced with a literal `"\x00"`.
pub fn con_print(marker: MainThreadMarker, s: &str) {
    if !Con_Printf.is_set(marker) {
        return;
    }

    let s = to_cstring_lossy(s);

    // Safety: Con_Printf() uses global buffers which are always valid, and external calls are
    // guarded with other global variables being non-zero, so they cannot be incorrectly called
    // either.
    unsafe {
        Con_Printf.get(marker)(b"%s\0".as_ptr().cast(), s.as_ptr());
    }
}

/// # Safety
///
/// [`reset_pointers()`] must be called before hw is unloaded so the pointers don't go stale.
#[cfg(unix)]
unsafe fn find_pointers(marker: MainThreadMarker) {
    use libc::{RTLD_NOLOAD, RTLD_NOW};
    use libloading::os::unix::Library;

    let library = Library::open(Some("hw.so"), RTLD_NOW | RTLD_NOLOAD).unwrap();

    for pointer in POINTERS {
        let ptr = library
            .get(pointer.symbol())
            .ok()
            .and_then(|sym| NonNull::new(*sym));
        pointer.set(marker, ptr);
        pointer.log(marker);
    }
}

/// # Safety
///
/// The memory starting at `base` with size `size` must be valid to read and not modified over the
/// duration of this call. If any pointers are found in memory, then the memory must be valid until
/// the pointers are reset (according to the safety section of `PointerTrait::set`).
#[allow(clippy::single_match)]
#[cfg(windows)]
pub unsafe fn find_pointers(marker: MainThreadMarker, base: *mut c_void, size: usize) {
    use std::slice;

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
    let ptr = &Cmd_AddMallocCommand;
    match ptr.pattern_index(marker) {
        // 6153
        Some(0) => cmd_functions.set(marker, ptr.by_offset(marker, 43)),
        _ => (),
    }

    let ptr = &Cvar_RegisterVariable;
    match ptr.pattern_index(marker) {
        // 6153
        Some(0) => cvar_vars.set(marker, ptr.by_offset(marker, 124)),
        _ => (),
    }

    let ptr = &Host_InitializeGameDLL;
    match ptr.pattern_index(marker) {
        // 6153
        Some(0) => {
            // svs.set(marker, ptr.by_offset(marker, 26));
            LoadEntityDLLs.set_if_empty(marker, ptr.by_relative_call(marker, 69));
            gEntityInterface.set(marker, ptr.by_offset(marker, 75));
        }
        _ => (),
    }

    let ptr = &Host_Tell_f;
    match ptr.pattern_index(marker) {
        // 6153
        Some(0) => {
            Cmd_Argc.set(marker, ptr.by_relative_call(marker, 28));
            Cmd_Argv.set(marker, ptr.by_relative_call(marker, 145));
        }
        _ => (),
    }

    let ptr = &Host_ValidSave;
    match ptr.pattern_index(marker) {
        // 6153
        Some(0) => {
            sv.set(marker, ptr.by_offset(marker, 19));
            cls.set(marker, ptr.by_offset(marker, 69));
            Con_Printf.set_if_empty(marker, ptr.by_relative_call(marker, 33));
        }
        _ => (),
    }

    let ptr = &LoadEntityDLLs;
    match ptr.pattern_index(marker) {
        // 6153
        Some(0) => {
            com_gamedir.set(marker, ptr.by_offset(marker, 51));
        }
        _ => (),
    }

    let ptr = &ReleaseEntityDlls;
    match ptr.pattern_index(marker) {
        // 6153
        Some(0) => {
            // svs.set(marker, ptr.by_offset(marker, 23));
        }
        _ => (),
    }

    let ptr = &SV_Frame;
    match ptr.pattern_index(marker) {
        // 6153
        Some(0) => {
            sv.set(marker, ptr.by_offset(marker, 1));
            host_frametime.set(marker, ptr.by_offset(marker, 11));
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

use exported::*;

/// Functions exported for `LD_PRELOAD` hooking.
pub mod exported {
    #![allow(clippy::missing_safety_doc)]

    use super::*;

    #[export_name = "Memory_Init"]
    pub unsafe extern "C" fn my_Memory_Init(buf: *mut c_void, size: c_int) -> c_int {
        abort_on_panic(move || {
            let marker = MainThreadMarker::new();

            #[cfg(unix)]
            let _ = env_logger::try_init();

            #[cfg(unix)]
            find_pointers(marker);

            // hw depends on SDL so it must be loaded by now.
            sdl::find_pointers(marker);

            let rv = Memory_Init.get(marker)(buf, size);

            cvars::register_all_cvars(marker);
            commands::register_all_commands(marker);
            cvars::deregister_disabled_module_cvars(marker);
            commands::deregister_disabled_module_commands(marker);

            rv
        })
    }

    #[export_name = "Host_Shutdown"]
    pub unsafe extern "C" fn my_Host_Shutdown() {
        abort_on_panic(move || {
            let marker = MainThreadMarker::new();

            commands::deregister_all_commands(marker);

            Host_Shutdown.get(marker)();

            cvars::mark_all_cvars_as_not_registered(marker);

            sdl::reset_pointers(marker);
            reset_pointers(marker);
        })
    }

    #[export_name = "V_FadeAlpha"]
    pub unsafe extern "C" fn my_V_FadeAlpha() -> c_int {
        abort_on_panic(move || {
            let marker = MainThreadMarker::new();

            if fade_remove::is_active(marker) {
                0
            } else {
                V_FadeAlpha.get(marker)()
            }
        })
    }

    #[export_name = "SV_Frame"]
    pub unsafe extern "C" fn my_SV_Frame() {
        abort_on_panic(move || {
            let marker = MainThreadMarker::new();

            tas_logging::begin_physics_frame(marker);

            SV_Frame.get(marker)();

            tas_logging::end_physics_frame(marker);
        })
    }

    #[export_name = "ReleaseEntityDlls"]
    pub unsafe extern "C" fn my_ReleaseEntityDlls() {
        abort_on_panic(move || {
            let marker = MainThreadMarker::new();

            server::reset_entity_interface(marker);

            // After updating pointers some modules might have got disabled.
            cvars::deregister_disabled_module_cvars(marker);
            commands::deregister_disabled_module_commands(marker);

            ReleaseEntityDlls.get(marker)();
        })
    }

    #[export_name = "LoadEntityDLLs"]
    pub unsafe extern "C" fn my_LoadEntityDLLs(base_dir: *const c_char) {
        abort_on_panic(move || {
            let marker = MainThreadMarker::new();

            LoadEntityDLLs.get(marker)(base_dir);

            server::hook_entity_interface(marker);

            // After updating pointers some modules might have got disabled.
            cvars::deregister_disabled_module_cvars(marker);
            commands::deregister_disabled_module_commands(marker);
        })
    }
}
