//! Windows API.

#![allow(non_snake_case, non_upper_case_globals)]

use std::ffi::CStr;
use std::mem::{size_of_val, zeroed};

use once_cell::sync::OnceCell;
use winapi::shared::minwindef::HMODULE;
use winapi::um::processthreadsapi::GetCurrentProcess;
use winapi::um::psapi::K32GetModuleInformation;
use winapi::um::winnt::LPCSTR;

use crate::hooks::engine;
use crate::utils::{abort_on_panic, MainThreadMarker};

pub static LoadLibraryA: OnceCell<unsafe extern "system" fn(LPCSTR) -> HMODULE> = OnceCell::new();

pub unsafe extern "system" fn my_LoadLibraryA(file_name: LPCSTR) -> HMODULE {
    abort_on_panic(move || {
        let rv = LoadLibraryA.get().unwrap()(file_name);

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
                    engine::find_pointers(
                        marker,
                        info.lpBaseOfDll.cast(),
                        info.SizeOfImage as usize,
                    );
                }
            }
        }

        rv
    })
}
