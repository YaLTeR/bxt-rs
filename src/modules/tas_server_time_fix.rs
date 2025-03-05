//! TAS server time fix.

use super::Module;
use crate::ffi::playermove::playermove_s;
use crate::hooks::server;
use crate::utils::*;

// TODO: the client needs a similar fix.
pub struct TasServerTimeFix;
impl Module for TasServerTimeFix {
    fn name(&self) -> &'static str {
        "TAS server time fix"
    }

    fn description(&self) -> &'static str {
        "Fixes server-side movement non-determinism when the player is stuck."
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        server::PM_Move.is_set(marker)
    }
}

static ORIGINAL_DATA: MainThreadCell<Option<(*mut playermove_s, unsafe extern "C" fn() -> f64)>> =
    MainThreadCell::new(None);

#[allow(non_snake_case)]
unsafe extern "C" fn my_Sys_FloatTime() -> f64 {
    abort_on_panic(|| {
        // Safety: assuming the game doesn't call Sys_FloatTime() from another thread.
        let marker = MainThreadMarker::new();

        (*ORIGINAL_DATA.get(marker).unwrap().0).time as f64 / 1000.
    })
}

pub unsafe fn on_pm_move_start(marker: MainThreadMarker, ppmove: *mut playermove_s) {
    if ORIGINAL_DATA.get(marker).is_some() {
        return;
    }

    ORIGINAL_DATA.set(marker, Some((ppmove, (*ppmove).Sys_FloatTime)));
    (*ppmove).Sys_FloatTime = my_Sys_FloatTime;
}

pub unsafe fn on_pm_move_end(marker: MainThreadMarker, ppmove: *mut playermove_s) {
    if let Some((ppmove_, sys_floattime)) = ORIGINAL_DATA.get(marker) {
        // Sanity checks.
        #[allow(clippy::fn_address_comparisons)]
        if ppmove == ppmove_
            && std::ptr::fn_addr_eq(
                (*ppmove).Sys_FloatTime,
                my_Sys_FloatTime as unsafe extern "C" fn() -> f64,
            )
        {
            (*ppmove).Sys_FloatTime = sys_floattime;
            ORIGINAL_DATA.set(marker, None);
        }
    }
}
