//! Windows-specific initialization.

use std::os::raw::*;

use winapi::{
    shared::minwindef::{DWORD, FALSE, HINSTANCE, LPVOID},
    um::{
        handleapi::CloseHandle,
        synchapi::{OpenEventA, SetEvent},
        winnt::EVENT_MODIFY_STATE,
    },
};

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
    // TODO: logging into stdout/stderr isn't visible, need to log into file and/or into a
    // dedicated window.
    env_logger::init();

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
