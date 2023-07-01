//! Bunnymod XT.

use std::os::raw::{c_char, c_int};
use std::ptr::NonNull;

use hltas::HLTAS;

use crate::modules::tas_studio;
use crate::utils::{abort_on_panic, MainThreadMarker, Pointer, PointerTrait};

pub static BXT_ON_TAS_PLAYBACK_FRAME: Pointer<
    *mut Option<unsafe extern "C" fn(OnTasPlaybackFrameData) -> c_int>,
> = Pointer::empty(b"bxt_on_tas_playback_frame\0");
pub static BXT_ON_TAS_PLAYBACK_STOPPED: Pointer<*mut Option<unsafe extern "C" fn()>> =
    Pointer::empty(b"bxt_on_tas_playback_stopped\0");
pub static BXT_SIMULATION_IPC_IS_CLIENT_INITIALIZED: Pointer<unsafe extern "C" fn() -> c_int> =
    Pointer::empty(b"bxt_simulation_ipc_is_client_initialized\0");
pub static BXT_TAS_LOAD_SCRIPT_FROM_STRING: Pointer<unsafe extern "C" fn(*const c_char)> =
    Pointer::empty(b"bxt_tas_load_script_from_string\0");
pub static BXT_IS_TAS_EDITOR_ACTIVE: Pointer<unsafe extern "C" fn() -> c_int> =
    Pointer::empty(b"bxt_is_tas_editor_active\0");

static POINTERS: &[&dyn PointerTrait] = &[
    &BXT_ON_TAS_PLAYBACK_FRAME,
    &BXT_ON_TAS_PLAYBACK_STOPPED,
    &BXT_SIMULATION_IPC_IS_CLIENT_INITIALIZED,
    &BXT_TAS_LOAD_SCRIPT_FROM_STRING,
    &BXT_IS_TAS_EDITOR_ACTIVE,
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

    set_callbacks(marker);
}

fn set_callbacks(marker: MainThreadMarker) {
    if let Some(bxt_on_tas_playback_frame) = BXT_ON_TAS_PLAYBACK_FRAME.get_opt(marker) {
        // SAFETY: this is a global variable in BXT which is accessed only from the main game thread
        // (which is the current thread as we have a marker).
        unsafe {
            let current_ptr = *bxt_on_tas_playback_frame;
            assert!(current_ptr.is_none() || current_ptr == Some(on_tas_playback_frame));
            *bxt_on_tas_playback_frame = Some(on_tas_playback_frame);
        }
    }

    if let Some(bxt_on_tas_playback_stopped) = BXT_ON_TAS_PLAYBACK_STOPPED.get_opt(marker) {
        // SAFETY: this is a global variable in BXT which is accessed only from the main game thread
        // (which is the current thread as we have a marker).
        unsafe {
            let current_ptr = *bxt_on_tas_playback_stopped;
            assert!(current_ptr.is_none() || current_ptr == Some(on_tas_playback_stopped));
            *bxt_on_tas_playback_stopped = Some(on_tas_playback_stopped);
        }
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

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct OnTasPlaybackFrameData {
    pub strafe_cycle_frame_count: u32,
    pub prev_predicted_trace_fractions: [f32; 4],
    pub prev_predicted_trace_normal_zs: [f32; 4],
}

unsafe extern "C" fn on_tas_playback_frame(data: OnTasPlaybackFrameData) -> c_int {
    abort_on_panic(move || {
        let marker = MainThreadMarker::new();

        let stop = tas_studio::on_tas_playback_frame(marker, data);
        stop.into()
    })
}

unsafe extern "C" fn on_tas_playback_stopped() {
    abort_on_panic(move || {
        let marker = MainThreadMarker::new();

        tas_studio::on_tas_playback_stopped(marker);
    })
}
