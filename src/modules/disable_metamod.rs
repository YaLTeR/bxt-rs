//! `Disabling Metamod`

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::path::PathBuf;

use super::Module;
use crate::hooks::{bxt, engine};
use crate::utils::*;

pub struct DisableMetamod;
impl Module for DisableMetamod {
    fn name(&self) -> &'static str {
        "Disabling Metamod"
    }

    fn description(&self) -> &'static str {
        "Metamod (and hence AmxModX) is disabled when used along with bxt-rs for game compatibility."
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        engine::LoadThisDll.is_set(marker)
        // BunnymodXT already has the featuer
        && !bxt::BXT_TAS_LOAD_SCRIPT_FROM_STRING.is_set(marker)
    }
}

const METAMOD_DLL_FILE_NAME: &str = "metamod";

// have to replace the string in place
// bool szDllFilename [8192];
pub unsafe fn replace_dll_name(marker: MainThreadMarker, orig_dll_name: *mut c_char) {
    if !DisableMetamod.is_enabled(marker) {
        return;
    }

    let dll_name_str = CStr::from_ptr(orig_dll_name);
    let dll_name_str_str = dll_name_str.to_string_lossy();

    if !dll_name_str_str.contains(METAMOD_DLL_FILE_NAME) {
        return;
    }

    info!("Found Metamod.");

    let gamedir = engine::com_gamedir.get(marker);
    let gamedir = CStr::from_ptr(gamedir as *mut i8);
    let gamedir = gamedir.to_string_lossy();

    if gamedir == "cstrike" {
        let lib_dll = if cfg!(windows) {
            "dlls\\mp.dll"
        } else {
            "dlls/cs.so"
        };

        // the dll path is actually the full path
        // orig `"/home/khang/bxt/game_isolated/./cstrike/addons/metamod/dlls/metamod.so"`
        // new `"dlls/cs.so"`
        // instead of canonicalize the path, which does not work all the time depending
        // on current working directory,
        // just replace the string where it is needed
        // rfind just to make sure that we don't have any weirdo doing weirdo stuffs
        let Some(start) = dll_name_str_str.rfind("cstrike") else {
            warn!("Failed to find the gamemod `cstrike` in the dll path.");
            return;
        };

        // verify that the path exists
        let lib_dll_path = PathBuf::from(&dll_name_str_str[..start])
            .join("cstrike")
            .join(lib_dll);

        if !lib_dll_path.exists() {
            warn!(
                "Failed to locate original server library find at `{}`.",
                lib_dll_path.display()
            );
            return;
        }

        let Ok(lib_dll) = CString::new(lib_dll) else {
            return;
        };

        // bool szDllFilename [8192];
        let Some(x) = (orig_dll_name as *mut [u8; 1024]).as_mut() else {
            return;
        };
        let copy_start = start + gamedir.len() + 1; // + 1 for the slash
        x[copy_start..(copy_start + lib_dll.as_bytes_with_nul().len())]
            .copy_from_slice(lib_dll.as_bytes_with_nul());

        info!("Successfully disabled Metamod.");
        return;
    }

    info!("No attempt to disable Metamod was made.");
}
