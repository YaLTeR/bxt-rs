//! 'client'.

#![allow(non_snake_case, non_upper_case_globals)]

use std::os::raw::*;
use std::ptr::NonNull;

use super::engine::{self, ref_params_s};
use crate::ffi::playermove::usercmd_s;
use crate::modules::{hud, hud_scale, tas_studio, triangle_drawing, viewmodel_sway};
use crate::utils::{abort_on_panic, MainThreadMarker, Pointer, PointerTrait};

pub static HudInitFunc: Pointer<unsafe extern "C" fn()> = Pointer::empty(b"HudInitFunc\0");
pub static HudVidInitFunc: Pointer<unsafe extern "C" fn()> = Pointer::empty(b"HudVidInitFunc\0");
pub static HudRedrawFunc: Pointer<unsafe extern "C" fn(c_float, c_int)> =
    Pointer::empty(b"HudRedrawFunc\0");
pub static HudUpdateClientDataFunc: Pointer<unsafe extern "C" fn(*mut c_void, c_float) -> c_int> =
    Pointer::empty(b"HudUpdateClientDataFunc\0");
pub static CalcRefdef: Pointer<unsafe extern "C" fn(*mut ref_params_s)> =
    Pointer::empty(b"CalcRefdef\0");
pub static IN_ActivateMouse: Pointer<unsafe extern "C" fn()> =
    Pointer::empty(b"IN_ActivateMouse\0");
pub static IN_DeactivateMouse: Pointer<unsafe extern "C" fn()> =
    Pointer::empty(b"IN_DeactivateMouse\0");
pub static DrawTransparentTriangles: Pointer<unsafe extern "C" fn()> =
    Pointer::empty(b"DrawTransparentTriangles\0");
pub static PostRunCmd: Pointer<
    unsafe extern "C" fn(
        from: *mut c_void,
        to: *mut c_void,
        cmd: *mut usercmd_s,
        runfuncs: c_int,
        time: c_double,
        random_seed: c_uint,
    ),
> = Pointer::empty(b"PostRunCmd\0");
pub static Shutdown: Pointer<unsafe extern "C" fn()> = Pointer::empty(b"Shutdown\0");

/// Calls IN_ActivateMouse() or IN_DeactivateMouse() depending on the parameter.
///
/// If the function is not present or has not been found yet, does nothing.
pub fn activate_mouse(marker: MainThreadMarker, activate: bool) {
    // SAFETY: the engine checks a zero-initialized global variable and whether the function pointer
    // is present before dispatching to it.
    unsafe {
        #[allow(clippy::collapsible_else_if)]
        if activate {
            if let Some(f) = IN_ActivateMouse.get_opt(marker) {
                f();
            }
        } else {
            if let Some(f) = IN_DeactivateMouse.get_opt(marker) {
                f();
            }
        }
    }
}

/// # Safety
///
/// This function must only be called right after `ClientDLL_Init()` is called.
pub unsafe fn hook_client_interface(marker: MainThreadMarker) {
    let functions = engine::cl_funcs.get_opt(marker);
    if functions.is_none() {
        return;
    }
    let functions = functions.unwrap().as_mut().unwrap();

    if let Some(ptr) = &mut functions.HudInitFunc {
        HudInitFunc.set(marker, Some(NonNull::new_unchecked(*ptr as _)));
    }
    functions.HudInitFunc = Some(my_HudInitFunc);
    HudInitFunc.log(marker);

    if let Some(ptr) = &mut functions.HudVidInitFunc {
        HudVidInitFunc.set(marker, Some(NonNull::new_unchecked(*ptr as _)));
    }
    functions.HudVidInitFunc = Some(my_HudVidInitFunc);
    HudVidInitFunc.log(marker);

    if let Some(ptr) = &mut functions.HudRedrawFunc {
        HudRedrawFunc.set(marker, Some(NonNull::new_unchecked(*ptr as _)));
    }
    functions.HudRedrawFunc = Some(my_HudRedrawFunc);
    HudRedrawFunc.log(marker);

    if let Some(ptr) = &mut functions.HudUpdateClientDataFunc {
        HudUpdateClientDataFunc.set(marker, Some(NonNull::new_unchecked(*ptr as _)));
    }
    functions.HudUpdateClientDataFunc = Some(my_HudUpdateClientDataFunc);
    HudUpdateClientDataFunc.log(marker);

    if let Some(ptr) = &mut functions.CalcRefdef {
        CalcRefdef.set(marker, Some(NonNull::new_unchecked(*ptr as _)));
    }
    functions.CalcRefdef = Some(my_CalcRefdef);
    CalcRefdef.log(marker);

    if let Some(ptr) = &mut functions.IN_ActivateMouse {
        IN_ActivateMouse.set(marker, Some(NonNull::new_unchecked(*ptr as _)));
    }
    IN_ActivateMouse.log(marker);

    if let Some(ptr) = &mut functions.IN_DeactivateMouse {
        IN_DeactivateMouse.set(marker, Some(NonNull::new_unchecked(*ptr as _)));
    }
    IN_DeactivateMouse.log(marker);

    if let Some(ptr) = &mut functions.DrawTransparentTriangles {
        DrawTransparentTriangles.set(marker, Some(NonNull::new_unchecked(*ptr as _)));
    }
    functions.DrawTransparentTriangles = Some(my_DrawTransparentTriangles);
    DrawTransparentTriangles.log(marker);

    if let Some(ptr) = &mut functions.PostRunCmd {
        PostRunCmd.set(marker, Some(NonNull::new_unchecked(*ptr as _)));
    }
    functions.PostRunCmd = Some(my_PostRunCmd);
    PostRunCmd.log(marker);

    if let Some(shutdown) = &mut functions.Shutdown {
        Shutdown.set(marker, Some(NonNull::new_unchecked(*shutdown as _)));
    }
    functions.Shutdown = Some(my_Shutdown);
    Shutdown.log(marker);
}

/// # Safety
///
/// This function must only be called from the client dll shutdown hook.
unsafe fn reset_client_interface(marker: MainThreadMarker) {
    let functions = engine::cl_funcs.get_opt(marker);
    if functions.is_none() {
        return;
    }
    let functions = functions.unwrap().as_mut().unwrap();

    // The fact that reset_client_interface() is called from the shutdown hook guarantees that the
    // pointers were hooked by bxt-rs first. Therefore we don't need to worry that we're replacing
    // them with bogus null pointers as in the server dll hooks.
    functions.HudInitFunc = HudInitFunc.get_opt(marker);
    HudInitFunc.reset(marker);
    functions.HudVidInitFunc = HudVidInitFunc.get_opt(marker);
    HudVidInitFunc.reset(marker);
    functions.HudRedrawFunc = HudRedrawFunc.get_opt(marker);
    HudRedrawFunc.reset(marker);
    functions.HudUpdateClientDataFunc = HudUpdateClientDataFunc.get_opt(marker);
    HudUpdateClientDataFunc.reset(marker);
    functions.CalcRefdef = CalcRefdef.get_opt(marker);
    CalcRefdef.reset(marker);
    functions.IN_ActivateMouse = IN_ActivateMouse.get_opt(marker);
    IN_ActivateMouse.reset(marker);
    functions.IN_DeactivateMouse = IN_DeactivateMouse.get_opt(marker);
    IN_DeactivateMouse.reset(marker);
    functions.DrawTransparentTriangles = DrawTransparentTriangles.get_opt(marker);
    DrawTransparentTriangles.reset(marker);
    functions.PostRunCmd = PostRunCmd.get_opt(marker);
    PostRunCmd.reset(marker);
    functions.Shutdown = Shutdown.get_opt(marker);
    Shutdown.reset(marker);
}

pub unsafe extern "C" fn my_Shutdown() {
    abort_on_panic(move || {
        let marker = MainThreadMarker::new();

        if let Some(f) = Shutdown.get_opt(marker) {
            f();
        };

        reset_client_interface(marker);
    })
}

pub unsafe extern "C" fn my_HudInitFunc() {
    abort_on_panic(move || {
        let marker = MainThreadMarker::new();

        // HACK: when this is called, m_rawinput has just been registered, and has a default value
        // of 0 because no cfg files had a chance to load. Therefore the client will think that raw
        // input is disabled. To make matters worse, the next time the client will read the value of
        // the cvar is oen second later according to the server time--which doesn't run when the
        // game is paused, so exactly when loading TAS studio right after loading the game. Since
        // the TAS studio only supports m_rawinput 1, we make the client think that it is 1 here
        // right away, thus working around the problem.
        tas_studio::with_m_rawinput_one(marker, || {
            if let Some(f) = HudInitFunc.get_opt(marker) {
                f();
            }
        })
    })
}

pub unsafe extern "C" fn my_HudVidInitFunc() {
    abort_on_panic(move || {
        let marker = MainThreadMarker::new();

        if let Some(f) = HudVidInitFunc.get_opt(marker) {
            hud_scale::with_scaled_screen_info(marker, move || f());
        }
    })
}

pub unsafe extern "C" fn my_HudUpdateClientDataFunc(cdat: *mut c_void, time: c_float) -> c_int {
    abort_on_panic(move || {
        let marker = MainThreadMarker::new();

        if let Some(f) = HudUpdateClientDataFunc.get_opt(marker) {
            hud_scale::with_scaled_screen_info(marker, move || f(cdat, time))
        } else {
            0
        }
    })
}

pub unsafe extern "C" fn my_PostRunCmd(
    from: *mut c_void,
    to: *mut c_void,
    cmd: *mut usercmd_s,
    runfuncs: c_int,
    time: c_double,
    random_seed: c_uint,
) {
    abort_on_panic(move || {
        let marker = MainThreadMarker::new();

        if let Some(f) = PostRunCmd.get_opt(marker) {
            f(from, to, cmd, runfuncs, time, random_seed)
        };

        tas_studio::on_post_run_cmd(marker, cmd);
    })
}

pub unsafe extern "C" fn my_HudRedrawFunc(time: c_float, intermission: c_int) {
    abort_on_panic(move || {
        let marker = MainThreadMarker::new();

        hud_scale::with_scaled_projection_matrix(marker, move || {
            if let Some(f) = HudRedrawFunc.get_opt(marker) {
                f(time, intermission)
            };

            hud::draw_hud(marker);
        });
    })
}

pub unsafe extern "C" fn my_CalcRefdef(rp: *mut ref_params_s) {
    abort_on_panic(move || {
        let marker = MainThreadMarker::new();

        let paused = (*rp).paused;
        if tas_studio::should_unpause_calcrefdef(marker) {
            (*rp).paused = 0;
        }

        CalcRefdef.get(marker)(rp);

        if tas_studio::should_unpause_calcrefdef(marker) {
            (*rp).paused = paused;
        }

        viewmodel_sway::add_viewmodel_sway(marker, &*rp);

        tas_studio::maybe_enable_freecam(marker);
    })
}

pub unsafe extern "C" fn my_DrawTransparentTriangles() {
    abort_on_panic(move || {
        let marker = MainThreadMarker::new();

        if let Some(f) = DrawTransparentTriangles.get_opt(marker) {
            f()
        };

        triangle_drawing::on_draw_transparent_triangles(marker);
    })
}
