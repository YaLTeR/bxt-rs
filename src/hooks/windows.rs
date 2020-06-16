//! Windows API.

use std::{
    ffi::CStr,
    mem::{size_of_val, zeroed},
};

use once_cell::sync::OnceCell;
use winapi::{
    shared::minwindef::HMODULE,
    um::{processthreadsapi::GetCurrentProcess, psapi::K32GetModuleInformation, winnt::LPCSTR},
};

use crate::{
    hooks::engine,
    utils::{abort_on_panic, MainThreadMarker},
};

pub static LOADLIBRARYA: OnceCell<unsafe extern "system" fn(LPCSTR) -> HMODULE> = OnceCell::new();

#[allow(non_snake_case)]
pub unsafe extern "system" fn LoadLibraryA(file_name: LPCSTR) -> HMODULE {
    abort_on_panic(move || {
        let rv = LOADLIBRARYA.get().unwrap()(file_name);

        if file_name.is_null() || rv.is_null() {
            return rv;
        }

        if let Ok(file_name) = CStr::from_ptr(file_name).to_str() {
            if file_name == "hw.dll" || file_name == "sw.dll" {
                // The loader is loading the engine. This is the main thread.
                let process = GetCurrentProcess();
                let mut info = zeroed();
                if K32GetModuleInformation(process, rv, &mut info, size_of_val(&info) as _) != 0 {
                    let marker = MainThreadMarker::new();
                    engine::find_pointers(marker, info.lpBaseOfDll, info.SizeOfImage as usize);
                }
            }
        }

        rv
    })
}
