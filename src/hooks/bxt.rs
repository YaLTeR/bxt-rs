//! Bunnymod XT.

use std::os::raw::{c_char, c_int};
use std::ptr::NonNull;

use hltas::HLTAS;

use crate::utils::{MainThreadMarker, Pointer, PointerTrait};

pub static BXT_SIMULATION_IPC_IS_CLIENT_INITIALIZED: Pointer<unsafe extern "C" fn() -> c_int> =
    Pointer::empty(b"bxt_simulation_ipc_is_client_initialized\0");
pub static BXT_TAS_LOAD_SCRIPT_FROM_STRING: Pointer<unsafe extern "C" fn(*const c_char)> =
    Pointer::empty(b"bxt_tas_load_script_from_string\0");

static POINTERS: &[&dyn PointerTrait] = &[
    &BXT_SIMULATION_IPC_IS_CLIENT_INITIALIZED,
    &BXT_TAS_LOAD_SCRIPT_FROM_STRING,
];

#[cfg(unix)]
fn open_library() -> Option<libloading::Library> {
    use libc::{RTLD_NOLOAD, RTLD_NOW};

    let library = unsafe {
        libloading::os::unix::Library::open(Some("libBunnymodXT.so"), RTLD_NOW | RTLD_NOLOAD)
    };
    library.ok().map(libloading::Library::from)
}

#[cfg(windows)]
fn open_library() -> Option<libloading::Library> {
    libloading::os::windows::Library::open_already_loaded("BunnymodXT.dll")
        .ok()
        .map(libloading::Library::from)
}

#[instrument(name = "bxt::find_pointers", skip_all)]
pub unsafe fn find_pointers(marker: MainThreadMarker) {
    let Some(library) = open_library() else {
        debug!("could not find Bunnymod XT");
        return;
    };

    for pointer in POINTERS {
        let ptr = library
            .get(pointer.symbol())
            .ok()
            .and_then(|sym| NonNull::new(*sym));
        pointer.set(marker, ptr);
        pointer.log(marker);
    }
}

pub unsafe fn tas_load_script(marker: MainThreadMarker, script: &HLTAS) {
    let mut buf = Vec::new();
    script.to_writer(&mut buf).unwrap();

    // Write the terminating NULL byte.
    buf.push(0);

    BXT_TAS_LOAD_SCRIPT_FROM_STRING.get(marker)(buf.as_ptr().cast());
}

pub fn is_simulation_ipc_client(marker: MainThreadMarker) -> bool {
    BXT_SIMULATION_IPC_IS_CLIENT_INITIALIZED
        .get_opt(marker)
        .map(|f|
            // SAFETY: the function reads a global variable in BXT which is zero-initialized at
            // start and always valid.
            unsafe { f() } != 0)
        .unwrap_or(false)
}
