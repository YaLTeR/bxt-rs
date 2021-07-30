//! Windows-specific initialization.

use std::os::raw::*;

use minhook_sys::*;
use winapi::shared::minwindef::{DWORD, FALSE, HINSTANCE, LPVOID};
use winapi::um::handleapi::CloseHandle;
use winapi::um::libloaderapi::{GetModuleHandleA, GetProcAddress};
use winapi::um::synchapi::{OpenEventA, SetEvent};
use winapi::um::winnt::EVENT_MODIFY_STATE;

use crate::hooks;
use crate::utils::*;

#[no_mangle]
pub unsafe extern "system" fn DllMain(
    _instance: HINSTANCE,
    reason: DWORD,
    _reserved: LPVOID,
) -> c_int {
    abort_on_panic(move || {
        if reason == 1 {
            // DLL_PROCESS_ATTACH
            std::thread::spawn(move || init());
        }

        1
    })
}

/// # Safety
///
/// This function must only be called once from a thread spawned in `DllMain()`.
unsafe fn init() {
    // TODO: assertion fails will result in the launcher hanging forever.
    ensure_logging_hooks();
    ensure_profiling();

    assert!(MH_Initialize() == MH_OK);

    // Hook LoadLibraryA to be able to run code when the loader attempts to load the engine.
    let kernel = GetModuleHandleA(b"kernel32.dll\0".as_ptr().cast());
    assert!(!kernel.is_null());

    let load_library_a = GetProcAddress(kernel, b"LoadLibraryA\0".as_ptr().cast());
    assert!(!load_library_a.is_null());

    let mut orig = None;
    assert!(
        MH_CreateHook(
            load_library_a.cast(),
            hooks::windows::my_LoadLibraryA as _,
            &mut orig as *mut _ as _
        ) == MH_OK
    );

    hooks::windows::LoadLibraryA
        .set(orig.unwrap())
        .ok()
        .unwrap();

    assert!(MH_EnableHook(load_library_a.cast()) == MH_OK);

    // Signal the injector to resume the process.
    let resume_event = OpenEventA(
        EVENT_MODIFY_STATE,
        FALSE,
        b"BunnymodXT-Injector\0".as_ptr().cast(),
    );
    if !resume_event.is_null() {
        SetEvent(resume_event);
        CloseHandle(resume_event);
    }
}
