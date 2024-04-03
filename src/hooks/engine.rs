//! `hw`, `sw`, `hl`.

#![allow(non_snake_case, non_upper_case_globals)]

use std::ffi::CString;
use std::fmt;
use std::num::ParseIntError;
use std::os::raw::*;
use std::ptr::{null_mut, NonNull};
use std::str::FromStr;

use bxt_macros::pattern;
use bxt_patterns::Patterns;

use crate::ffi::com_model::{mleaf_s, model_s};
use crate::ffi::command::cmd_function_s;
use crate::ffi::cvar::cvar_s;
use crate::ffi::edict::edict_s;
use crate::ffi::playermove::playermove_s;
use crate::ffi::triangleapi::triangleapi_s;
use crate::ffi::usercmd::usercmd_s;
#[cfg(windows)]
use crate::hooks::opengl32;
use crate::hooks::{bxt, sdl, server};
use crate::modules::*;
use crate::utils::*;

pub static build_number: Pointer<unsafe extern "C" fn() -> c_int> = Pointer::empty_patterns(
    b"build_number\0",
    // To find, search for "Half-Life %i/%s (hw build %d)". This function is
    // Draw_ConsoleBackground(), and a call to build_number() is right above the snprintf() using
    // this string.
    Patterns(&[
        // 6153
        pattern!(55 8B EC 83 EC 08 A1 ?? ?? ?? ?? 56 33 F6 85 C0),
        // 4554
        pattern!(A1 ?? ?? ?? ?? 83 EC 08 57 33 FF 85 C0),
        // 3248
        pattern!(A1 ?? ?? ?? ?? 83 EC 08 56),
        // CoF-5936
        pattern!(55 8B EC 83 EC 10 C7 45 ?? 00 00 00 00 C7 45 ?? 00 00 00 00 C7 45 ?? 00 00 00 00 83 3D ?? ?? ?? ?? 00),
    ]),
    null_mut(),
);
pub static CBaseUI__HideGameUI: Pointer<unsafe extern "fastcall" fn(*mut c_void)> =
    // 8th pointer in CBaseUI vtable.
    // To find, search for "chromehtml.dll". You are in CBaseUI::Initialize and that will be
    // the second pointer of CBaseUI vtable.
    Pointer::empty_patterns(
            b"_ZN7CBaseUI10HideGameUIEv\0",
            Patterns(&[
                // 8684
                pattern!(56 8B F1 8B 0D ?? ?? ?? ?? 8B 01 FF 50 ?? 8B 0D ?? ?? ?? ?? 8B 11 FF 52 ?? FF 15),
            ]),
            my_CBaseUI__HideGameUI as _,
        );
pub static Cbuf_AddFilteredText: Pointer<unsafe extern "C" fn(*const c_char)> =
    Pointer::empty_patterns(
        b"Cbuf_AddFilteredText\0",
        Patterns(&[]),
        my_Cbuf_AddFilteredText as _,
    );
pub static Cbuf_AddText: Pointer<unsafe extern "C" fn(*const c_char)> =
    Pointer::empty_patterns(b"Cbuf_AddText\0", Patterns(&[]), my_Cbuf_AddText as _);
pub static Cbuf_AddTextToBuffer: Pointer<unsafe extern "C" fn(*const c_char, *mut c_void)> =
    Pointer::empty_patterns(
        b"Cbuf_AddTextToBuffer\0",
        // To find, search for "Cbuf_AddTextToBuffer: overflow".
        Patterns(&[
            // 8684
            pattern!(55 8B EC 56 57 8B 7D ?? 57 E8 ?? ?? ?? ?? 8B 75),
        ]),
        my_Cbuf_AddTextToBuffer as _,
    );
pub static Cbuf_InsertText: Pointer<unsafe extern "C" fn(*const c_char)> =
    Pointer::empty(b"Cbuf_InsertText\0");
pub static CL_Disconnect: Pointer<unsafe extern "C" fn()> = Pointer::empty_patterns(
    b"CL_Disconnect\0",
    // To find, search for "ExitGame".
    Patterns(&[
        // 6153
        pattern!(55 8B EC 83 EC 14 53 56 33 DB),
        // 4554
        pattern!(83 EC 14 C7 05 ?? ?? ?? ?? F0 69 F8 C0),
        // CoF-5936
        pattern!(55 8B EC 83 EC 18 56 57 C7 05 ?? ?? ?? ?? 00 00 00 00),
    ]),
    my_CL_Disconnect as _,
);
pub static cl_funcs: Pointer<*mut ClientDllFunctions> = Pointer::empty(b"cl_funcs\0");
pub static CL_GameDir_f: Pointer<unsafe extern "C" fn()> = Pointer::empty_patterns(
    b"CL_GameDir_f\0",
    // To find, search for "gamedir is ".
    Patterns(&[
        // 6153
        pattern!(E8 ?? ?? ?? ?? 83 F8 02 74 ?? 68 ?? ?? ?? ?? 68 ?? ?? ?? ?? E8 ?? ?? ?? ?? 83 C4 08 C3),
        // CoF-5936
        pattern!(55 8B EC E8 ?? ?? ?? ?? 83 F8 02 74 ?? 68 ?? ?? ?? ?? 68 ?? ?? ?? ?? E8 ?? ?? ?? ?? 83 C4 08 EB),
    ]),
    null_mut(),
);
pub static cl_lightstyle: Pointer<*mut [lightstyle_t; 64]> = Pointer::empty(b"cl_lightstyle\0");
pub static CL_Move: Pointer<unsafe extern "C" fn()> = Pointer::empty_patterns(
    b"CL_Move\0",
    // To find, search for "Client Move".
    Patterns(&[
        // 6153
        pattern!(55 8B EC 81 EC 78 08 00 00),
    ]),
    my_CL_Move as _,
);
pub static CL_Parse_LightStyle: Pointer<unsafe extern "C" fn()> = Pointer::empty_patterns(
    b"CL_Parse_LightStyle\0",
    // To find, search for "svc_lightstyle > MAX_LIGHTSTYLES"
    Patterns(&[
        // 8684
        pattern!(56 57 E8 ?? ?? ?? ?? 8B ?? 83 ?? ?? ?? ?? 68),
    ]),
    my_CL_Parse_LightStyle as _,
);
pub static CL_PlayDemo_f: Pointer<unsafe extern "C" fn()> = Pointer::empty_patterns(
    b"CL_PlayDemo_f\0",
    // To find, search for "playdemo <demoname> <replayspeed>: plays a demo".
    Patterns(&[
        // 8684
        pattern!(55 8B EC 81 EC 00 01 00 00 A1 ?? ?? ?? ?? 53),
        // 4554
        pattern!(A1 ?? ?? ?? ?? 81 EC 00 01 00 00 83 F8 01),
        // CoF-5936
        pattern!(55 8B EC 81 EC 00 01 00 00 56 57 83 3D ?? ?? ?? ?? 01),
    ]),
    my_CL_PlayDemo_f as _,
);
pub static CL_ViewDemo_f: Pointer<unsafe extern "C" fn()> = Pointer::empty_patterns(
    b"CL_ViewDemo_f\0",
    // To find, search for "viewdemo not available".
    Patterns(&[
        // 8684
        pattern!(55 8B EC 81 EC 04 01 00 00 A1 ?? ?? ?? ?? 56 BE 01 00 00 00 57 3B C6 0F 85),
    ]),
    my_CL_ViewDemo_f as _,
);
pub static ClientDLL_Init: Pointer<unsafe extern "C" fn()> = Pointer::empty_patterns(
    b"ClientDLL_Init\0",
    // To find, search for "cl_dlls\\client.dll" (with a backslash).
    Patterns(&[
        // 6153
        pattern!(55 8B EC 81 EC 00 02 00 00 68),
    ]),
    my_ClientDLL_Init as _,
);
pub static ClientDLL_DrawTransparentTriangles: Pointer<unsafe extern "C" fn()> =
    Pointer::empty_patterns(
        b"ClientDLL_DrawTransparentTriangles\0",
        // To find, search for "HUD_DrawTransparentTriangles". This sets the
        // HUD_DrawTransparentTriangles pointer in cl_funcs; the larger function calling
        // the pointer is ClientDLL_DrawTransparentTriangles().
        Patterns(&[
            // 8684
            pattern!(A1 ?? ?? ?? ?? 85 C0 74 ?? FF D0 6A 00 FF 15 ?? ?? ?? ?? 59 C3 90 90 90 90 90 90 90 90 90 90 90 A1 ?? ?? ?? ?? 85 C0 74 ?? FF E0),
            // CoF-5936
            pattern!(55 8B EC 83 3D ?? ?? ?? ?? 00 74 ?? FF 15 ?? ?? ?? ?? 6A 00 FF 15 ?? ?? ?? ?? 83 C4 04 5D C3 55 8B EC 83 3D ?? ?? ?? ?? 00 74 06 FF 15 ?? ?? ?? ?? 5D),
        ]),
        null_mut(),
    );
pub static cl: Pointer<*mut client_state_t> = Pointer::empty(b"cl\0");
pub static cl_stats: Pointer<*mut [i32; 32]> = Pointer::empty(
    // Not a real symbol name.
    b"cl_stats\0",
);
pub static cl_viewent: Pointer<*mut cl_entity_s> = Pointer::empty(
    // Not a real symbol name.
    b"cl_viewent\0",
);
pub static cl_viewent_viewmodel: Pointer<*mut cl_entity_s_viewmodel> = Pointer::empty(
    // Not a real symbol name.
    b"cl_viewent_viewmodel\0",
);
pub static cls: Pointer<*mut client_static_s> = Pointer::empty(b"cls\0");
pub static cls_demoframecount: Pointer<*mut c_int> = Pointer::empty(
    // Not a real symbol name.
    b"cls_demoframecount\0",
);
pub static cls_demos: Pointer<*mut client_static_s_demos> = Pointer::empty(
    // Not a real symbol name.
    b"cls_demos\0",
);
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
        // 4554
        pattern!(56 57 8B 7C 24 ?? 57 E8 ?? ?? ?? ?? 8A 08 83 C4 04 84 C9 74 ?? 57 68 ?? ?? ?? ?? E8 ?? ?? ?? ?? 83 C4 08 5F ?? C3),
        // CoF-5936
        pattern!(55 8B EC 51 8B 45 ?? 50 E8 ?? ?? ?? ?? 83 C4 04 0F BE 08 85 C9 74 16 8B 55 08 52 68 ?? ?? ?? ?? E8 ?? ?? ?? ?? 83 C4 08),
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
        // 4554
        pattern!(B8 00 10 00 00 E8 ?? ?? ?? ?? 8B 8C 24),
        // CoF-5936
        pattern!(55 8B EC B8 04 10 00 00 E8 ?? ?? ?? ?? 56),
    ]),
    null_mut(),
);
pub static Con_ToggleConsole_f: Pointer<unsafe extern "C" fn()> = Pointer::empty_patterns(
    b"Con_ToggleConsole_f\0",
    // To find, search for "toggleconsole". Look for console command registration, the callback
    // will be Con_ToggleConsole_f().
    Patterns(&[
        // 6153
        pattern!(E8 ?? ?? ?? ?? 85 C0 74 ?? E9 ?? ?? ?? ?? E9 ?? ?? ?? ?? 90 90 90 90 90 90 90 90 90 90 90 90 90),
        // 1600
        pattern!(A1 ?? ?? ?? ?? B9 01 00 00 00 3B C1 75 ?? A1),
        // CoF-5936
        pattern!(55 8B EC E8 ?? ?? ?? ?? 85 C0 74 ?? E8 ?? ?? ?? ?? EB),
    ]),
    my_Con_ToggleConsole_f as _,
);
pub static com_gamedir: Pointer<*mut [c_char; 260]> = Pointer::empty(b"com_gamedir\0");
pub static Cvar_RegisterVariable: Pointer<unsafe extern "C" fn(*mut cvar_s)> =
    Pointer::empty_patterns(
        b"Cvar_RegisterVariable\0",
        // To find, search for "Can't register variable %s, already defined".
        Patterns(&[
            // 6153
            pattern!(55 8B EC 83 EC 14 53 56 8B 75 ?? 57 8B 06),
            // 4554
            pattern!(83 EC 14 53 56 8B 74 24),
            // CoF-5936
            pattern!(55 8B EC 83 EC 24 8B 45 ?? 8B 08 51 E8 ?? ?? ?? ?? 83 C4 04 85 C0 74 18 8B 55 08 8B 02 50 68 ?? ?? ?? ?? E8 ?? ?? ?? ?? 83 C4 08),
        ]),
        null_mut(),
    );
pub static cvar_vars: Pointer<*mut *mut cvar_s> = Pointer::empty(b"cvar_vars\0");
pub static Draw_FillRGBABlend: Pointer<
    unsafe extern "C" fn(c_int, c_int, c_int, c_int, c_int, c_int, c_int, c_int),
> = Pointer::empty_patterns(
    b"Draw_FillRGBABlend\0",
    // 130th pointer in cl_enginefuncs.
    Patterns(&[
        // 8684
        pattern!(55 8B EC 83 EC 08 8D 45 ?? 8D 4D ?? 50 8D 55 ?? 51 8D 45 ?? 52 8D 4D ?? 50 8D 55 ?? 51 8D 45 ?? 52 8D 4D ?? 50 51 FF 15 ?? ?? ?? ?? 83 C4 20 68 E1 0D 00 00 FF 15 ?? ?? ?? ?? 68 E2 0B 00 00 FF 15 ?? ?? ?? ?? 68 00 00 04 46 68 00 22 00 00 68 00 23 00 00 FF 15 ?? ?? ?? ?? 68 03 03 00 00),
    ]),
    null_mut(),
);
pub static Draw_String: Pointer<unsafe extern "C" fn(c_int, c_int, *const c_char) -> c_int> =
    Pointer::empty_patterns(
        b"Draw_String\0",
        // To find, search for "Downloading %s". You are in SCR_DrawDownloadText().
        // Draw_String() will be the last call in the conditional block, below two other calls
        // including Draw_SetTextColor() taking in three identical float arguments.
        Patterns(&[
            // 8684
            pattern!(55 8B EC 56 57 E8 ?? ?? ?? ?? 8B 4D),
        ]),
        null_mut(),
    );
pub static DrawCrosshair: Pointer<unsafe extern "C" fn(c_int, c_int)> = Pointer::empty_patterns(
    b"DrawCrosshair\0",
    // To find, search for "Client.dll SPR_DrawHoles error:  invalid frame". This is
    // SPR_DrawHoles(), it's used in two places: a data table (cl_enginefuncs) and DrawCrosshair().
    Patterns(&[
        // 6153
        pattern!(55 8B EC A1 ?? ?? ?? ?? 85 C0 74 ?? 8B 0D ?? ?? ?? ?? 8B 15),
        // 4554
        pattern!(A1 ?? ?? ?? ?? 85 C0 74 5C 8B 0D ?? ?? ?? ?? 8B 15 ?? ?? ?? ?? 51 8B 0D),
        // CoF-5936
        pattern!(55 8B EC 83 3D ?? ?? ?? ?? 00 74 ?? A1 ?? ?? ?? ?? 50 8B 0D ?? ?? ?? ?? 51),
    ]),
    my_DrawCrosshair as _,
);
pub static frametime_remainder: Pointer<*mut f64> = Pointer::empty(
    // Not a real symbol name.
    b"frametime_remainder\0",
);
pub static GL_BeginRendering: Pointer<
    unsafe extern "C" fn(*mut c_int, *mut c_int, *mut c_int, *mut c_int),
> = Pointer::empty_patterns(
    b"GL_BeginRendering\0",
    // To find, search for "R_BeginFrame". The function using this string is
    // GLimp_LogNewFrame() and the function calling that is GL_BeginRendering().
    Patterns(&[
        // 6153
        pattern!(55 8B EC 8B 45 ?? 8B 4D ?? 56 57),
        // 4554
        pattern!(8B 44 24 ?? 8B 4C 24 ?? 8B 54 24 ?? C7 00 00 00 00 00),
        // CoF-5936
        pattern!(55 8B EC 8B 45 ?? C7 00 00 00 00 00 8B 4D),
    ]),
    null_mut(),
);
pub static gEntityInterface: Pointer<*mut DllFunctions> = Pointer::empty(b"gEntityInterface\0");
pub static gLoadSky: Pointer<*mut c_int> = Pointer::empty(b"gLoadSky\0");
pub static g_svmove: Pointer<*mut playermove_s> = Pointer::empty(b"g_svmove\0");
pub static Key_Event: Pointer<unsafe extern "C" fn(c_int, c_int)> = Pointer::empty_patterns(
    b"Key_Event\0",
    // To find, search for "ctrl-alt-del pressed".
    Patterns(&[
        // 6153
        pattern!(55 8B EC 81 EC 00 04 00 00 8B 45 ?? 56 3D 00 01 00 00),
        // 4554
        pattern!(81 EC 00 04 00 00 8D 84 24 ?? ?? ?? ?? 8D 8C 24),
        // 1600
        pattern!(81 EC 00 04 00 00 56 8B B4 24 ?? ?? ?? ?? 57 8B BC 24),
        // CoF-5936
        pattern!(55 8B EC 81 EC 08 04 00 00 8D 45 0C 50 8D 4D 08 51 FF 15 ?? ?? ?? ?? 83 C4 08),
    ]),
    my_Key_Event as _,
);
pub static LoadEntityDLLs: Pointer<unsafe extern "C" fn(*const c_char)> = Pointer::empty_patterns(
    b"LoadEntityDLLs\0",
    // To find, search for "GetNewDLLFunctions".
    Patterns(&[
        // 6153
        // Don't use this for com_gamedir as the pattern matches versions with different offsets.
        pattern!(55 8B EC B8 90 23 00 00),
        // 4554
        pattern!(81 EC 94 04 00 00 53 56 E8),
        // 1600
        pattern!(81 EC AC 05 00 00 E8),
        // CoF-5936
        pattern!(55 8B EC 81 EC BC 04 00 00 E8),
    ]),
    my_LoadEntityDLLs as _,
);
pub static Mod_LeafPVS: Pointer<unsafe extern "C" fn(*mut mleaf_s, *mut model_s) -> *mut c_void> =
    Pointer::empty_patterns(
        b"Mod_LeafPVS\0",
        // To find, search for "Spawned a NULL entity!", the referencing function is
        // CreateNamedEntity. Find cross references, go to the global data, that data is
        // g_engfuncsExportedToDlls Go up 5 entries and you'll find PVSFindEntities, inside
        // this function first function call is Mod_PointInLeaf and the 2nd one is
        // Mod_LeafPVS.
        Patterns(&[
            // 6153
            pattern!(55 8B EC 8B 55 ?? 8B 45 ?? 8B 8A),
            // 4554
            pattern!(8B 54 24 ?? 8B 44 24 ?? 8B 8A),
            // CoF-5936
            pattern!(55 8B EC 51 8B 45 ?? 8B 4D ?? 3B 88 ?? ?? ?? ?? 75 07 B8 ?? ?? ?? ?? EB ?? 83 3D ?? ?? ?? ?? 00),
        ]),
        my_Mod_LeafPVS as _,
    );
pub static Host_FilterTime: Pointer<unsafe extern "C" fn(c_float) -> c_int> =
    Pointer::empty_patterns(
        b"Host_FilterTime\0",
        // To find, search for "-sys_ticrate". The parent will be _Host_Frame().
        Patterns(&[
            // 6153
            pattern!(55 8B EC 83 EC 08 D9 05 ?? ?? ?? ?? D8 1D ?? ?? ?? ?? DF E0 25),
            // 4554
            pattern!(55 8B EC 83 E4 F8 83 EC 08 D9 05 ?? ?? ?? ?? D8 1D ?? ?? ?? ?? DF E0 F6 C4 41),
            // 3248
            pattern!(55 8B EC 83 E4 F8 83 EC 08 D9 05 ?? ?? ?? ?? D8 1D ?? ?? ?? ?? DF E0 25 00 41 00 00),
            // 1712
            pattern!(55 8B EC 83 E4 F8 83 EC 08 D9 45 08 DC 05 ?? ?? ?? ?? A1 ?? ?? ?? ?? 85 C0 DD 1D ?? ?? ?? ?? 0F 85 E1 00 00 00),
            // CoF-5936
            pattern!(55 8B EC 83 EC 14 D9 05 ?? ?? ?? ?? D8 1D ?? ?? ?? ?? DF E0 F6 C4 41),
        ]),
        my_Host_FilterTime as _,
    );
pub static host_frametime: Pointer<*mut c_double> = Pointer::empty(b"host_frametime\0");
pub static Host_InitializeGameDLL: Pointer<unsafe extern "C" fn()> = Pointer::empty_patterns(
    b"Host_InitializeGameDLL\0",
    // To find, search for "Sys_InitializeGameDLL called twice, skipping second call".
    // Alternatively, find LoadEntityDLLs() and go to the parent function.
    Patterns(&[
        // 6153
        pattern!(E8 ?? ?? ?? ?? 8B 0D ?? ?? ?? ?? 33 C0 83 F9 01),
        // 1600
        pattern!(E8 ?? ?? ?? ?? A1 ?? ?? ?? ?? 85 C0 74 ?? 68 ?? ?? ?? ?? E8 ?? ?? ?? ?? 83 C4 04 C3),
        // CoF-5936
        pattern!(55 8B EC 83 EC 0C C6 45 ?? 2D),
    ]),
    null_mut(),
);
pub static Host_NextDemo: Pointer<unsafe extern "C" fn()> = Pointer::empty_patterns(
    b"Host_NextDemo\0",
    // To find, search for "No demos listed with startdemos".
    Patterns(&[
        // 6153
        pattern!(55 8B EC 81 EC 00 04 00 00 83 3D ?? ?? ?? ?? FF 0F 84),
        // 4554
        pattern!(A1 ?? ?? ?? ?? 81 EC 00 04 00 00 83 F8 FF 0F 84 87 00 00 00),
        // 1712
        pattern!(A1 ?? ?? ?? ?? 81 EC 00 04 00 00 83 F8 FF 0F 84 82 00 00 00),
        // CoF-5936
        pattern!(55 8B EC 81 EC 00 04 00 00 83 3D ?? ?? ?? ?? FF 75 05),
    ]),
    my_Host_NextDemo as _,
);
pub static Host_Shutdown: Pointer<unsafe extern "C" fn()> = Pointer::empty_patterns(
    b"Host_Shutdown\0",
    // To find, search for "recursive shutdown".
    Patterns(&[
        // 6153
        pattern!(A1 ?? ?? ?? ?? 53 33 DB 3B C3 74 ?? 68),
        // 3248
        pattern!(53 33 DB 53 68 ?? ?? ?? ?? FF 15),
        // 1600
        pattern!(A1 ?? ?? ?? ?? 85 C0 74 ?? 68 ?? ?? ?? ?? E8 ?? ?? ?? ?? 83 C4 04 C3 A1 ?? ?? ?? ?? C7 05 ?? ?? ?? ?? 01 00 00 00 85 C0),
        // CoF-5936
        pattern!(55 8B EC 83 EC 08 83 3D ?? ?? ?? ?? 00 74 ?? 68),
    ]),
    my_Host_Shutdown as _,
);
pub static Host_Tell_f: Pointer<unsafe extern "C" fn()> = Pointer::empty_patterns(
    b"Host_Tell_f\0",
    // To find, search for "%s TELL: ".
    Patterns(&[
        // 6153
        pattern!(55 8B EC 83 EC 40 A1 ?? ?? ?? ?? 56),
        // 4554
        pattern!(A1 ?? ?? ?? ?? 83 EC 40 83 F8 01 56 75 0A E8 ?? ?? ?? ?? ?? 83 C4 40 C3 E8 ?? ?? ?? ?? 83 F8 03 0F 8C 7A 01 00 00),
        // 3248
        pattern!(A1 ?? ?? ?? ?? 83 EC 40 83 F8 01 56 75 09),
        // 1712
        pattern!(A1 ?? ?? ?? ?? 83 EC 40 83 F8 01 56 75 0A E8 ?? ?? ?? ?? ?? 83 C4 40 C3 E8 ?? ?? ?? ?? 83 F8 03 0F 8C 82 01 00 00),
        // CoF-5936
        pattern!(55 8B EC 83 EC 54 83 3D ?? ?? ?? ?? 01 75 0A E8 ?? ?? ?? ?? E9 ?? ?? ?? ?? E8 ?? ?? ?? ?? 83 F8 03 7D 05),
    ]),
    null_mut(),
);
pub static Host_ValidSave: Pointer<unsafe extern "C" fn() -> c_int> = Pointer::empty_patterns(
    b"Host_ValidSave\0",
    // To find, search for "Not playing a local game.".
    Patterns(&[
        // 6153
        pattern!(A1 ?? ?? ?? ?? B9 01 00 00 00 3B C1 0F 85),
        // CoF-5936
        pattern!(55 8B EC 83 3D ?? ?? ?? ?? 01 74 ?? 33 C0),
    ]),
    null_mut(),
);
pub static hudGetScreenInfo: Pointer<unsafe extern "C" fn(*mut SCREENINFO) -> c_int> =
    Pointer::empty_patterns(
        b"hudGetScreenInfo\0",
        // 13th pointer in cl_enginefuncs.
        Patterns(&[
            // 6153
            pattern!(55 8B EC 8D 45 ?? 50 FF 15 ?? ?? ?? ?? 8B 45 ?? 83 C4 04 85 C0 75 ?? 5D C3 81 38 14 02 00 00),
            // 4554
            pattern!(8D 44 24 ?? 50 FF 15 ?? ?? ?? ?? 8B 44 24 ?? 83 C4 04 85 C0 75 ?? C3 81 38 14 02 00 00),
            // 1600
            pattern!(56 8B 74 24 ?? 85 F6 75 ?? 33 C0 ?? C3 81 ?? 14 02 00 00),
            // CoF-5936
            pattern!(55 8B EC 8D 45 ?? 50 FF 15 ?? ?? ?? ?? 83 C4 04 83 7D ?? 00 75 ?? 33 C0 EB ?? 8B 4D ?? 81 39 14 02 00 00),
        ]),
        my_hudGetScreenInfo as _,
    );
pub static hudGetViewAngles: Pointer<unsafe extern "C" fn(*mut [c_float; 3])> =
    Pointer::empty_patterns(
        b"hudGetViewAngles\0",
        // 35th pointer in cl_enginefuncs.
        //
        // Be careful! The very next function is hudSetViewAngles() which looks VERY similar, yet
        // does the exact opposite thing!
        Patterns(&[
            // 8684
            pattern!(55 8B EC 8D 45 ?? 50 FF 15 ?? ?? ?? ?? 8B 55),
        ]),
        null_mut(),
    );
pub static idum: Pointer<*mut c_int> = Pointer::empty(
    // Not a real symbol name.
    b"idum\0",
);
pub static Memory_Init: Pointer<unsafe extern "C" fn(*mut c_void, c_int) -> c_int> =
    Pointer::empty_patterns(
        b"Memory_Init\0",
        // To find, search for "Memory_Init".
        Patterns(&[
            // 6153
            pattern!(55 8B EC 8B 45 ?? 8B 4D ?? 56 BE 00 00 20 00),
            // 4554
            pattern!(8B 44 24 ?? 8B 4C 24 ?? 56 BE 00 00 20 00),
            // 1600
            pattern!(8B 44 24 ?? 8B 4C 24 ?? 56 BE 00 00 02 00),
            // CoF-5936
            pattern!(55 8B EC 83 EC 08 C7 45 ?? 00 00 20 00),
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
        // 4554
        pattern!(56 8B 74 24 ?? 85 F6 74 ?? 6A 09),
    ]),
    null_mut(),
);
pub static movevars: Pointer<*mut movevars_s> = Pointer::empty(b"movevars\0");
pub static paintbuffer: Pointer<*mut [portable_samplepair_t; 1026]> =
    Pointer::empty(b"paintbuffer\0");
pub static paintedtime: Pointer<*mut c_int> = Pointer::empty(b"paintedtime\0");
pub static pmove: Pointer<*mut *mut playermove_s> = Pointer::empty(b"pmove\0");
pub static ran1: Pointer<unsafe extern "C" fn() -> c_int> = Pointer::empty_patterns(
    b"ran1\0",
    // Find RandomLong(). The function it calls in the loop is ran1().
    Patterns(&[
        // 6153
        pattern!(8B 0D ?? ?? ?? ?? 56 85 C9 ?? ?? 8B 35),
        // CoF-5936
        pattern!(55 8B EC 83 EC 08 83 3D ?? ?? ?? ?? 00 ?? ?? 83 3D ?? ?? ?? ?? 00 0F 85),
    ]),
    null_mut(),
);
pub static ran1_iy: Pointer<*mut c_int> = Pointer::empty(
    // Not a real symbol name.
    b"ran1::iy\0",
);
pub static ran1_iv: Pointer<*mut [c_int; 32]> = Pointer::empty(
    // Not a real symbol name.
    b"ran1::iv\0",
);
pub static realtime: Pointer<*mut f64> = Pointer::empty(b"realtime\0");
pub static r_refdef: Pointer<*mut c_void> = Pointer::empty(b"r_refdef\0");
pub static r_refdef_vieworg: Pointer<*mut [c_float; 3]> = Pointer::empty(
    // Not a real symbol name.
    b"r_refdef_vieworg\0",
);
pub static r_refdef_viewangles: Pointer<*mut [c_float; 3]> = Pointer::empty(
    // Not a real symbol name.
    b"r_refdef_viewangles\0",
);
pub static R_DrawSequentialPoly: Pointer<
    unsafe extern "C" fn(*mut c_void, *mut c_int) -> *mut c_void,
> = Pointer::empty_patterns(
    b"R_DrawSequentialPoly\0",
    // To find, search for "Too many decal surfaces!\n". This string will be used once in
    // R_RenderBrushPoly and twice in R_DrawSequentialPoly.
    Patterns(&[
        // 6153
        pattern!(55 8B EC 51 A1 ?? ?? ?? ?? 53 56 57 83 B8 ?? ?? ?? ?? 01),
        // 4554
        pattern!(A1 ?? ?? ?? ?? 53 55 56 8B 88),
        // 1600
        pattern!(A1 ?? ?? ?? ?? 53 55 BD 01 00 00 00 8B 88 F8 02 00 00 56 3B CD 57 75 62 E8 ?? ?? ?? ?? 68 03 03 00 00 68 02 03 00 00),
        // CoF-5936
        pattern!(55 8B EC 83 EC 1C A1 ?? ?? ?? ?? 83 B8 ?? ?? ?? ?? 01),
    ]),
    my_R_DrawSequentialPoly as _,
);
pub static R_Clear: Pointer<unsafe extern "C" fn() -> *mut c_void> = Pointer::empty_patterns(
    b"R_Clear\0",
    // To find, search for "R_RenderView". This is R_RenderView, the call before two if
    // (global == 0) {} conditions is R_Clear.
    Patterns(&[
        // 6153
        pattern!(8B 15 ?? ?? ?? ?? 33 C0 83 FA 01),
        // 3248
        pattern!(D9 05 ?? ?? ?? ?? DC 1D ?? ?? ?? ?? DF E0 F6 C4 ?? ?? ?? D9 05 ?? ?? ?? ?? D8 1D ?? ?? ?? ?? DF E0 F6 C4 ?? ?? ?? 68 ?? ?? ?? ?? EB),
        // CoF-5936
        pattern!(55 8B EC 33 C0 83 3D ?? ?? ?? ?? 01 0F 9F C0 50 E8 ?? ?? ?? ?? 83 C4 04 D9 05 ?? ?? ?? ?? DC 1D),
    ]),
    my_R_Clear as _,
);
pub static R_DrawSkyBox: Pointer<unsafe extern "C" fn()> = Pointer::empty_patterns(
    b"R_DrawSkyBox\0",
    // To find, search for "ClipSkyPolygon: MAX_CLIP_VERTS" string.
    // This is ClipSkyPolygon. On Windows, right below that function is R_DrawSkyChain.
    // Last call in R_DrawSkyChain is R_DrawSkyBox. Alternatively, search for the byte
    // sequence "42 B0 47 34 C3 BE D2 BF", the referencing function is R_DrawSkyBox.
    Patterns(&[
        // 6153
        pattern!(55 8B EC 83 EC 1C A1 ?? ?? ?? ?? 53 56),
        // 4554
        pattern!(83 EC 1C A1 ?? ?? ?? ?? 53 55),
        // 1712
        pattern!(83 EC 0C 53 55 56 57 E8 ?? ?? ?? ?? 33 FF),
        // CoF-5936
        pattern!(55 8B EC 83 EC 24 C7 45 ?? 00 00 00 00 C7 45 ?? 00 00 80 3F),
    ]),
    my_R_DrawSkyBox as _,
);
pub static R_DrawViewModel: Pointer<unsafe extern "C" fn()> = Pointer::empty_patterns(
    // To find, search for "R_RenderView". This is R_RenderView.
    // There will be an assignment of `mirror = false` and a function call follows.
    // The next line should be branching of `r_refdef.onlyClientDraws == false`, which will repeat
    // again in R_RenderView(). R_DrawViewModel is called in the block where branch appears the
    // second time. In that branch block, it contains two functions called. The first one is
    // R_DrawViewModel().
    b"R_DrawViewModel\0",
    Patterns(&[
        // 8684
        pattern!(55 8B EC 83 EC 50 D9 05 ?? ?? ?? ?? D8 1D ?? ?? ?? ?? 56 57 33 FF C7 45),
        // 4554
        pattern!(83 EC ?? D9 05 ?? ?? ?? ?? D8 1D ?? ?? ?? ?? 56 57 33 FF C7 44),
    ]),
    my_R_DrawViewModel as _,
);
pub static R_LoadSkys: Pointer<unsafe extern "C" fn()> = Pointer::empty_patterns(
    b"R_LoadSkys\0",
    // To find, search for "done\n".
    Patterns(&[
        // 8684
        pattern!(55 8B EC 83 EC 6C A1 ?? ?? ?? ?? 56),
    ]),
    my_R_LoadSkys as _,
);
pub static R_PreDrawViewModel: Pointer<unsafe extern "C" fn()> = Pointer::empty_patterns(
    // To find, search for "R_RenderView". This is R_RenderView.
    // There will be an assignment of `mirror = false` and a function call follows.
    // The next line should be branching of `r_refdef.onlyClientDraws == false`, which will repeat
    // again in R_RenderView(). In that branching block, there is one function called, that is
    // R_PreDrawViewModel().
    b"R_PreDrawViewModel\0",
    Patterns(&[
        // 8684
        pattern!(D9 05 ?? ?? ?? ?? D8 1D ?? ?? ?? ?? 56 C7 05),
    ]),
    my_R_PreDrawViewModel as _,
);
pub static R_RenderView: Pointer<unsafe extern "C" fn()> = Pointer::empty_patterns(
    b"R_RenderView\0",
    // To find, search for "R_RenderView: NULL worldmodel".
    Patterns(&[
        // 8684
        pattern!(55 8B EC 83 EC 14 D9 05 ?? ?? ?? ?? D8 1D ?? ?? ?? ?? DF E0 F6 C4 44 0F 8A ?? ?? ?? ?? A1 ?? ?? ?? ?? 85 C0 74),
    ]),
    my_R_RenderView as _,
);
pub static R_SetFrustum: Pointer<unsafe extern "C" fn()> = Pointer::empty_patterns(
    b"R_SetFrustum\0",
    // To find, search for "R_RenderView". This is R_RenderView(). The call between two if (global
    // == 0) {} conditions is R_RenderScene(). Open R_RenderScene(). The second call after the
    // first check is R_SetFrustum().
    Patterns(&[
        // 6153
        pattern!(55 8B EC 83 EC 08 DB 05),
        // 4554
        pattern!(83 EC 08 DB 05 ?? ?? ?? ?? A1 ?? ?? ?? ?? 56 89 44 24 04),
        // CoF-5936
        pattern!(55 8B EC 83 EC 0C A1 ?? ?? ?? ?? 89 45 ?? DB 05),
    ]),
    my_R_SetFrustum as _,
);
pub static ReleaseEntityDlls: Pointer<unsafe extern "C" fn()> = Pointer::empty_patterns(
    b"ReleaseEntityDlls\0",
    // Find Host_Shutdown(). It has a Mem_Free() if. The 3-rd function above that if is
    // ReleaseEntityDlls().
    Patterns(&[
        // 6153
        pattern!(A1 ?? ?? ?? ?? 56 57 BE ?? ?? ?? ?? 8D 04),
        // CoF-5936
        pattern!(55 8B EC 83 EC 08 C7 45 ?? ?? ?? ?? ?? A1 ?? ?? ?? ?? 6B C0 0C),
    ]),
    my_ReleaseEntityDlls as _,
);
pub static S_PaintChannels: Pointer<unsafe extern "C" fn(c_int)> = Pointer::empty_patterns(
    b"S_PaintChannels\0",
    // To find, search for "Start profiling 10,000 calls to DSP". This is S_Say(). A call below
    // which has an argument of something + 0x4e2000 is S_PaintChannels().
    Patterns(&[
        // 6153
        pattern!(55 8B EC A1 ?? ?? ?? ?? 53 8B 5D ?? 3B C3 0F 8D),
        // 4554
        pattern!(A1 ?? ?? ?? ?? 55 8B 6C 24),
        // CoF-5936
        pattern!(55 8B EC 83 EC 14 A1 ?? ?? ?? ?? 3B 45),
    ]),
    my_S_PaintChannels as _,
);
pub static S_TransferStereo16: Pointer<unsafe extern "C" fn(c_int)> = Pointer::empty_patterns(
    b"S_TransferStereo16\0",
    // To find, find S_PaintChannels(), go into the last call before the while () condition in the
    // end and this will be the function that that one falls through into. Alternatively, search
    // for "S_TransferStereo16".
    Patterns(&[
        // 6153
        pattern!(55 8B EC 83 EC 0C D9 05 ?? ?? ?? ?? D8 0D),
        // 4554
        pattern!(D9 05 ?? ?? ?? ?? D8 0D ?? ?? ?? ?? 83 EC 0C 53 56 57 E8 ?? ?? ?? ?? 8B 4C 24 1C A3 ?? ?? ?? ?? A1 ?? ?? ?? ?? C7 05 ?? ?? ?? ?? ?? ?? ?? ?? 8D 3C 09 8D 34 00 A1 ?? ?? ?? ?? 85 C0 74 55 E8),
        // 3248
        pattern!(D9 05 ?? ?? ?? ?? D8 0D ?? ?? ?? ?? 83 EC 0C 53 56 57 E8 ?? ?? ?? ?? 8B 4C 24 1C A3 ?? ?? ?? ?? A1 ?? ?? ?? ?? C7 05 ?? ?? ?? ?? ?? ?? ?? ?? 8D 3C 09 8D 34 00 A1 ?? ?? ?? ?? 85 C0 74 54 E8),
        // CoF-5936
        pattern!(55 8B EC 83 EC 24 D9 05 ?? ?? ?? ?? D8 0D ?? ?? ?? ?? E8),
    ]),
    my_S_TransferStereo16 as _,
);
pub static SCR_DrawLoading: Pointer<unsafe extern "C" fn()> = Pointer::empty_patterns(
    b"SCR_DrawLoading\0",
    // To find, search for "cz_worldmap" string in Steampipe DLL.
    // This is SCR_DrawPause. Right below that function is SCR_DrawLoading.
    // This pattern also works for the pre-Steampipe builds.

    // Another way to find this would be as follows:
    // To find, search for "transition" string, there is two functions with that string:
    // SCR_BeginLoadingPlaque and SCR_EndLoadingPlaque. Go to SCR_EndLoadingPlaque, it can be
    // recognized by having much less code than in SCR_BeginLoadingPlaque. Find second variable
    // inside of that function, it would be scr_drawloading boolean. Now to references of
    // variable and find other function with shortest code in it, that would be SCR_DrawLoading
    // function.
    Patterns(&[
        // 6153
        pattern!(A1 ?? ?? ?? ?? 85 C0 74 05 E9 ?? ?? FF FF C3 90),
        // CoF-5936
        pattern!(55 8B EC 83 3D ?? ?? ?? ?? 00 75 ?? EB ?? E8 ?? ?? ?? ?? 5D C3 55 8B EC E8),
    ]),
    my_SCR_DrawLoading as _,
);
pub static SCR_DrawPause: Pointer<unsafe extern "C" fn()> = Pointer::empty_patterns(
    b"SCR_DrawPause\0",
    // To find, search for "cz_worldmap". You are in SCR_DrawPause().
    Patterns(&[
        pattern!(D9 05 ?? ?? ?? ?? D8 1D ?? ?? ?? ?? DF E0 F6 C4 44 7B ?? A1 ?? ?? ?? ?? 85 C0 74 ?? E8 ?? ?? ?? ?? 85),
    ]),
    my_SCR_DrawPause as _,
);
pub static scr_fov_value: Pointer<*mut c_float> = Pointer::empty(b"scr_fov_value\0");
pub static shm: Pointer<*mut *mut dma_t> = Pointer::empty(b"shm\0");
pub static sv: Pointer<*mut c_void> = Pointer::empty(b"sv\0");
pub static svs: Pointer<*mut server_static_s> = Pointer::empty(b"svs\0");
pub static sv_areanodes: Pointer<*mut c_void> = Pointer::empty(b"sv_areanodes\0");
pub static SV_AddLinksToPM: Pointer<unsafe extern "C" fn(*mut c_void, *const [f32; 3])> =
    Pointer::empty(b"SV_AddLinksToPM\0");
pub static SV_AddLinksToPM_: Pointer<
    unsafe extern "C" fn(*mut c_void, *mut [f32; 3], *mut [f32; 3]),
> = Pointer::empty_patterns(
    b"SV_AddLinksToPM_\0",
    // To find, search for "SV_AddLinksToPM:  pmove->nummoveent >= MAX_MOVEENTS\n"
    Patterns(&[
        // 8684
        pattern!(55 8B EC 83 EC 14 8B 4D ?? 53 8B 5D),
        // 4554
        pattern!(83 EC 10 53 55 56 57 8B 5C 24),
        // 3248
        pattern!(83 EC 10 53 8B 5C 24 ?? 55 56 57),
        // CoF-5936
        pattern!(55 8B EC 83 EC 24 56 57 8B 45 ?? 8B 48),
    ]),
    my_SV_AddLinksToPM_ as _,
);
pub static SV_ExecuteClientMessage: Pointer<unsafe extern "C" fn(*mut c_void)> =
    Pointer::empty_patterns(
        b"SV_ExecuteClientMessage\0",
        // To find, search for "SV_ReadClientMessage: badread".
        Patterns(&[
            // 8684
            pattern!(55 8B EC 8B 0D ?? ?? ?? ?? 56 8B 75 ?? C7 05 ?? ?? ?? ?? 00 00 00 00),
            // CoF-5936
            pattern!(55 8B EC 83 EC 0C C7 05 ?? ?? ?? ?? 00 00 00 00 8B 45 08),
        ]),
        null_mut(),
    );
pub static SV_Frame: Pointer<unsafe extern "C" fn()> = Pointer::empty_patterns(
    b"SV_Frame\0",
    // To find, search for "%s timed out". It is used in SV_CheckTimeouts(), which is called by
    // SV_Frame().
    Patterns(&[
        // 6153
        pattern!(A1 ?? ?? ?? ?? 85 C0 74 ?? DD 05 ?? ?? ?? ?? A1),
        // CoF-5936
        pattern!(55 8B EC 83 3D ?? ?? ?? ?? 00 75 ?? EB ?? DD 05 ?? ?? ?? ?? D9 1D ?? ?? ?? ?? A1 ?? ?? ?? ?? A3 ?? ?? ?? ?? 8B 0D),
    ]),
    my_SV_Frame as _,
);
pub static SV_RunCmd: Pointer<unsafe extern "C" fn(*mut usercmd_s, c_int)> =
    Pointer::empty_patterns(
        b"SV_RunCmd\0",
        // To find, find SV_AddLinksToPM_(), go to the referenced caller function,
        // this is SV_AddLinksToPM(), go to the referenced function once again,
        // this is SV_RunCmd().
        Patterns(&[
            // 8684
            pattern!(55 8B EC 81 EC ?? ?? ?? ?? 56 57 8B 75 08 B9 0D 00 00 00 8D 7D 84 F3 A5 A1 ?? ?? ?? ?? DD 80 ?? ?? ?? ?? DC 1D ?? ?? ?? ?? DF E0 25 00 41 00 00),
            // 4554
            pattern!(55 8B EC 81 EC ?? ?? ?? ?? 56 57 8B 75 08 B9 0D 00 00 00 8D 7D 84 F3 A5 A1 ?? ?? ?? ?? DD 80 ?? ?? ?? ?? DC 1D ?? ?? ?? ?? DF E0 F6 C4 41),
            // CoF-5936
            pattern!(55 8B EC 81 EC ?? ?? ?? ?? 56 57 8B 75 08 B9 ?? 00 00 00 8D 7D 80 F3 A5 A1 ?? ?? ?? ?? DD 80 ?? ?? ?? ?? DC 1D ?? ?? ?? ?? DF E0 F6 C4 41),
        ]),
        null_mut(),
    );
pub static Sys_VID_FlipScreen: Pointer<unsafe extern "C" fn()> = Pointer::empty_patterns(
    b"_Z18Sys_VID_FlipScreenv\0",
    // To find, search for "Sys_InitLauncherInterface()". Go into function right after the one that
    // accepts this string as an argument. The last function pointer assigned is
    // Sys_VID_FlipScreen(). It checks one pointer for NULL then calls SDL_GL_SwapWindow().
    Patterns(&[
        // 6153
        pattern!(A1 ?? ?? ?? ?? 85 C0 74 ?? 8B 00),
        // 4554
        pattern!(A1 ?? ?? ?? ?? 50 FF 15 ?? ?? ?? ?? C3),
        // CoF-5936
        pattern!(55 8B EC A1 ?? ?? ?? ?? 50 FF 15 ?? ?? ?? ?? 5D),
    ]),
    my_Sys_VID_FlipScreen as _,
);
pub static Sys_VID_FlipScreen_old: Pointer<unsafe extern "system" fn(*mut c_void)> =
    Pointer::empty_patterns(
        // Not a real symbol name.
        b"_Z18Sys_VID_FlipScreenv_old\0",
        // To find, search for "wglSwapBuffers". This pointer is assigned to a global, which is
        // called in a single function, this is Sys_VID_FlipScreen().
        Patterns(&[
            // 1712
            pattern!(8B 44 24 ?? 50 FF 15 ?? ?? ?? ?? C2 04 00),
        ]),
        my_Sys_VID_FlipScreen_old as _,
    );
pub static tri: Pointer<*const triangleapi_s> = Pointer::empty(b"tri\0");
pub static V_ApplyShake: Pointer<unsafe extern "C" fn(*mut [f32; 3], *mut [f32; 3], c_float)> =
    Pointer::empty_patterns(
        b"V_ApplyShake\0",
        // To find, search for "ScreenShake". This is ClientDLL_Init(), near the bottom there are
        // two similar function calls, one is using our string as the 1st param and another
        // function as the 2nd param, open that function in the 2nd param. This is
        // V_ScreenShake(), right above it is V_ApplyShake().
        Patterns(&[
            // 6153
            pattern!(55 8B EC 8D 45 ?? 8D 4D ?? 50 8D 55 ?? 51 52 FF 15 ?? ?? ?? ?? 8B 45 ?? 83 C4 0C),
            // 4554
            pattern!(8D 44 24 ?? 8D 4C 24 ?? 50 8D 54 24 ?? 51 52 FF 15 ?? ?? ?? ?? 8B 44 24 ?? 83 C4 0C),
            // 1712
            pattern!(8B 44 24 ?? 85 C0 74 ?? 8B 4C 24 ?? 50),
            // CoF-5936
            pattern!(55 8B EC 8D 45 ?? 50 8D 4D ?? 51 8D 55 ?? 52 FF 15 ?? ?? ?? ?? 83 C4 0C 83 7D ?? 00),
        ]),
        my_V_ApplyShake as _,
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
        // 4554
        pattern!(D9 05 ?? ?? ?? ?? DC 1D ?? ?? ?? ?? 8A 0D ?? ?? ?? ?? 83 EC 08),
        // CoF-5936
        pattern!(55 8B EC 83 EC 0C C7 45 ?? 00 00 00 00 D9 05 ?? ?? ?? ?? DC 1D ?? ?? ?? ?? DF E0),
    ]),
    my_V_FadeAlpha as _,
);
pub static V_RenderView: Pointer<unsafe extern "C" fn()> = Pointer::empty_patterns(
    b"V_RenderView\0",
    // To find, search for "R_RenderView: NULL worldmodel". This is an output error for
    // R_RenderView(). Then, find a function calling R_RenderView() exactly 3 times. That
    // function will be V_RenderView().
    Patterns(&[
        // 8684
        pattern!(55 8B EC 81 EC F4 00 00 00 A1 ?? ?? ?? ?? 56 57),
    ]),
    my_V_RenderView as _,
);
pub static VideoMode_IsWindowed: Pointer<unsafe extern "C" fn() -> c_int> = Pointer::empty_patterns(
    b"VideoMode_IsWindowed\0",
    // To find, first find GL_BeginRendering(). The first check is for the
    // return value of VideoMode_IsWindowed().
    Patterns(&[
        // 6153
        pattern!(8B 0D ?? ?? ?? ?? 85 C9 74 ?? 8B 01 FF 50 ?? 84 C0),
        // 3248
        pattern!(8B 0D ?? ?? ?? ?? 8B 01 FF 50 ?? 25 FF 00 00 00),
        // CoF-5936
        pattern!(55 8B EC 51 83 3D ?? ?? ?? ?? 00 74 ?? A1 ?? ?? ?? ?? 8B 10),
    ]),
    null_mut(),
);
pub static VideoMode_GetCurrentVideoMode: Pointer<
    unsafe extern "C" fn(*mut c_int, *mut c_int, *mut c_int),
> = Pointer::empty_patterns(
    b"VideoMode_GetCurrentVideoMode\0",
    // To find, first find GL_BeginRendering(). The first if calls
    // VideoMode_GetCurrentVideoMode().
    Patterns(&[
        // 6153
        pattern!(55 8B EC 8B 0D ?? ?? ?? ?? 8B 01 FF 50 ?? 85 C0),
        // 4554
        pattern!(8B 0D ?? ?? ?? ?? 8B 01 FF 50 ?? 85 C0 74 ?? 8B 4C 24),
        // CoF-5936
        pattern!(55 8B EC 51 A1 ?? ?? ?? ?? 8B 10 8B 0D ?? ?? ?? ?? FF 52),
    ]),
    null_mut(),
);
pub static window_rect: Pointer<*mut Rect> = Pointer::empty(b"window_rect\0");
pub static Z_Free: Pointer<unsafe extern "C" fn(*mut c_void)> = Pointer::empty_patterns(
    b"Z_Free\0",
    // To find, search for "Z_Free: NULL pointer".
    Patterns(&[
        // 6153
        pattern!(55 8B EC 56 8B 75 ?? 85 F6 57 75 ?? 68 ?? ?? ?? ?? E8 ?? ?? ?? ?? 83 C4 04 8B 46),
        // 4554
        pattern!(56 8B 74 24 ?? 85 F6 57 75 ?? 68 ?? ?? ?? ?? E8 ?? ?? ?? ?? 83 C4 04 8B 46),
        // CoF-5936
        pattern!(55 8B EC 83 EC 08 83 7D ?? 00 75 ?? 68 ?? ?? ?? ?? E8 ?? ?? ?? ?? 83 C4 04 8B 45),
    ]),
    null_mut(),
);

pub static client_s_edict_offset: MainThreadCell<Option<usize>> = MainThreadCell::new(None);

static POINTERS: &[&dyn PointerTrait] = &[
    &build_number,
    &CBaseUI__HideGameUI,
    &Cbuf_AddFilteredText,
    &Cbuf_AddText,
    &Cbuf_AddTextToBuffer,
    &Cbuf_InsertText,
    &CL_Disconnect,
    &cl_funcs,
    &CL_GameDir_f,
    &cl_lightstyle,
    &CL_Move,
    &CL_Parse_LightStyle,
    &CL_PlayDemo_f,
    &CL_ViewDemo_f,
    &ClientDLL_Init,
    &ClientDLL_DrawTransparentTriangles,
    &cl,
    &cl_stats,
    &cl_viewent,
    &cl_viewent_viewmodel,
    &cls,
    &cls_demoframecount,
    &cls_demos,
    &Cmd_AddMallocCommand,
    &Cmd_Argc,
    &Cmd_Argv,
    &cmd_functions,
    &Con_Printf,
    &Con_ToggleConsole_f,
    &com_gamedir,
    &Cvar_RegisterVariable,
    &cvar_vars,
    &DrawCrosshair,
    &Draw_FillRGBABlend,
    &Draw_String,
    &frametime_remainder,
    &GL_BeginRendering,
    &gEntityInterface,
    &gLoadSky,
    &g_svmove,
    &Key_Event,
    &LoadEntityDLLs,
    &Mod_LeafPVS,
    &Host_FilterTime,
    &host_frametime,
    &Host_InitializeGameDLL,
    &Host_NextDemo,
    &Host_Shutdown,
    &Host_Tell_f,
    &Host_ValidSave,
    &hudGetScreenInfo,
    &hudGetViewAngles,
    &idum,
    &movevars,
    &Memory_Init,
    &Mem_Free,
    &paintbuffer,
    &paintedtime,
    &pmove,
    &ran1,
    &ran1_iy,
    &ran1_iv,
    &realtime,
    &r_refdef,
    &r_refdef_vieworg,
    &r_refdef_viewangles,
    &R_RenderView,
    &R_SetFrustum,
    &ReleaseEntityDlls,
    &R_Clear,
    &R_DrawSequentialPoly,
    &R_DrawSkyBox,
    &R_DrawViewModel,
    &R_LoadSkys,
    &R_PreDrawViewModel,
    &S_PaintChannels,
    &S_TransferStereo16,
    &SCR_DrawLoading,
    &SCR_DrawPause,
    &scr_fov_value,
    &shm,
    &sv,
    &svs,
    &sv_areanodes,
    &SV_AddLinksToPM,
    &SV_AddLinksToPM_,
    &SV_ExecuteClientMessage,
    &SV_Frame,
    &SV_RunCmd,
    &Sys_VID_FlipScreen,
    &Sys_VID_FlipScreen_old,
    &tri,
    &V_ApplyShake,
    &V_FadeAlpha,
    &V_RenderView,
    &VideoMode_IsWindowed,
    &VideoMode_GetCurrentVideoMode,
    &window_rect,
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
pub struct ClientDllFunctions {
    pub InitFunc: Option<NonNull<c_void>>,
    pub HudInitFunc: Option<unsafe extern "C" fn()>,
    pub HudVidInitFunc: Option<unsafe extern "C" fn()>,
    pub HudRedrawFunc: Option<unsafe extern "C" fn(c_float, c_int)>,
    pub HudUpdateClientDataFunc: Option<unsafe extern "C" fn(*mut c_void, c_float) -> c_int>,
    pub HudResetFunc: Option<NonNull<c_void>>,
    pub ClientMove: Option<NonNull<c_void>>,
    pub ClientMoveInit: Option<NonNull<c_void>>,
    pub ClientTextureType: Option<NonNull<c_void>>,
    pub IN_ActivateMouse: Option<unsafe extern "C" fn()>,
    pub IN_DeactivateMouse: Option<unsafe extern "C" fn()>,
    pub IN_MouseEvent: Option<NonNull<c_void>>,
    pub IN_ClearStates: Option<NonNull<c_void>>,
    pub IN_Accumulate: Option<NonNull<c_void>>,
    pub CL_CreateMove: Option<NonNull<c_void>>,
    pub CL_IsThirdPerson: Option<NonNull<c_void>>,
    pub CL_GetCameraOffsets: Option<NonNull<c_void>>,
    pub FindKey: Option<NonNull<c_void>>,
    pub CamThink: Option<NonNull<c_void>>,
    pub CalcRefdef: Option<unsafe extern "C" fn(*mut ref_params_s)>,
    pub AddEntity: Option<NonNull<c_void>>,
    pub CreateEntities: Option<NonNull<c_void>>,
    pub DrawNormalTriangles: Option<NonNull<c_void>>,
    pub DrawTransparentTriangles: Option<unsafe extern "C" fn()>,
    pub StudioEvent: Option<NonNull<c_void>>,
    pub PostRunCmd: Option<
        unsafe extern "C" fn(
            from: *mut c_void,
            to: *mut c_void,
            cmd: *mut usercmd_s,
            runfuncs: c_int,
            time: c_double,
            random_seed: c_uint,
        ),
    >,
    pub Shutdown: Option<unsafe extern "C" fn()>,
    pub TxferLocalOverrides: Option<NonNull<c_void>>,
    pub ProcessPlayerState: Option<NonNull<c_void>>,
    pub TxferPredictionData: Option<NonNull<c_void>>,
    pub ReadDemoBuffer: Option<NonNull<c_void>>,
    pub ConnectionlessPacket: Option<NonNull<c_void>>,
    pub GetHullBounds: Option<NonNull<c_void>>,
    pub HudFrame: Option<NonNull<c_void>>,
    pub KeyEvent: Option<NonNull<c_void>>,
    pub TempEntUpdate: Option<NonNull<c_void>>,
    pub GetUserEntity: Option<NonNull<c_void>>,
    pub VoiceStatus: Option<NonNull<c_void>>,
    pub DirectorMessage: Option<NonNull<c_void>>,
    pub StudioInterface: Option<NonNull<c_void>>,
    pub ChatInputPosition: Option<NonNull<c_void>>,
    pub GetPlayerTeam: Option<NonNull<c_void>>,
    pub ClientFactory: Option<NonNull<c_void>>,
}

#[cfg(unix)]
#[derive(Clone, Copy)]
#[repr(C)]
pub struct Rect {
    pub left: i32,
    pub right: i32,
    pub top: i32,
    pub bottom: i32,
}
#[cfg(windows)]
#[derive(Clone, Copy)]
#[repr(C)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct portable_samplepair_t {
    pub left: c_int,
    pub right: c_int,
}

#[repr(C)]
pub struct dma_t {
    pub gamealive: c_int,
    pub soundalive: c_int,
    pub splitbuffer: c_int,
    pub channels: c_int,
    pub samples: c_int,
    pub submission_chunk: c_int,
    pub samplepos: c_int,
    pub samplebits: c_int,
    pub speed: c_int,
    pub dmaspeed: c_int,
    pub buffer: *mut c_uchar,
}

#[repr(C)]
pub struct cl_entity_s {
    pub index: c_int,
}

#[repr(C)]
pub struct cl_entity_s_viewmodel {
    pub origin: [c_float; 3],
    pub angles: [c_float; 3],
}

#[repr(C)]
pub struct client_state_t {
    pub max_edicts: c_int,
}

#[repr(C)]
pub struct client_static_s {
    pub state: c_int,
}

#[repr(C)]
pub struct client_static_s_demos {
    pub demonum: c_int,
    pub demos: [[c_char; 16]; 32],
    pub demorecording: c_int,
    pub demoplayback: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct lightstyle_t {
    pub length: c_int,
    pub map: [c_char; 64],
}

#[repr(C)]
pub struct movevars_s {
    pub gravity: c_float,
    pub stopspeed: c_float,
    pub maxspeed: c_float,
    pub spectatormaxspeed: c_float,
    pub accelerate: c_float,
    pub airaccelerate: c_float,
    pub wateraccelerate: c_float,
    pub friction: c_float,
    pub edgefriction: c_float,
    pub waterfriction: c_float,
    pub entgravity: c_float,
    pub bounce: c_float,
    pub stepsize: c_float,
    pub maxvelocity: c_float,
    pub zmax: c_float,
    pub waveHeight: c_float,
    pub footsteps: c_int,
    pub skyName: [c_char; 32],
    pub rollangle: c_float,
    pub rollspeed: c_float,
    pub skycolor_r: c_float,
    pub skycolor_g: c_float,
    pub skycolor_b: c_float,
    pub skyvec_x: c_float,
    pub skyvec_y: c_float,
    pub skyvec_z: c_float,
}

#[repr(C)]
pub struct server_static_s {
    pub dll_initialized: c_int,
    pub clients: *mut c_void,
    pub num_clients: c_int,
}

#[allow(clippy::upper_case_acronyms)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SCREENINFO {
    pub iSize: c_int,
    pub iWidth: c_int,
    pub iHeight: c_int,
    pub iFlags: c_int,
    pub iCharHeight: c_int,
    pub charWidths: [c_short; 256],
}

#[repr(C)]
pub struct ref_params_s {
    pub vieworg: [c_float; 3],
    pub viewangles: [c_float; 3],
    pub forward: [c_float; 3],
    pub right: [c_float; 3],
    pub up: [c_float; 3],
    pub frametime: c_float,
    pub time: c_float,
    pub intermission: c_int,
    pub paused: c_int,
    pub spectator: c_int,
    pub onground: c_int,
    pub waterlevel: c_int,
}

impl SCREENINFO {
    pub const fn zeroed() -> Self {
        Self {
            iSize: 0,
            iWidth: 0,
            iHeight: 0,
            iFlags: 0,
            iCharHeight: 0,
            charWidths: [0; 256],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RngState {
    pub idum: c_int,
    pub iy: c_int,
    pub iv: [c_int; 32],
}

// `FromStr` and `Display` implementations are for parsing and printing a console command argument.
impl FromStr for RngState {
    type Err = ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut rv = RngState::default();

        let mut iter = s.split_ascii_whitespace();
        rv.idum = iter.next().unwrap_or_default().parse()?;
        rv.iy = iter.next().unwrap_or_default().parse()?;
        for x in rv.iv.iter_mut() {
            *x = iter.next().unwrap_or_default().parse()?;
        }

        Ok(rv)
    }
}

impl fmt::Display for RngState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.idum, self.iy)?;
        for x in self.iv {
            write!(f, " {x}")?;
        }
        Ok(())
    }
}

/// Returns the non-shared RNG state.
///
/// If the required pointers are missing, returns `None`.
pub fn rng_state(marker: MainThreadMarker) -> Option<RngState> {
    // Safety: these are all global buffers which are always valid.
    unsafe {
        Some(RngState {
            idum: *idum.get_opt(marker)?,
            iy: *ran1_iy.get_opt(marker)?,
            iv: *ran1_iv.get_opt(marker)?,
        })
    }
}

/// Sets the non-shared RNG state.
///
/// # Panics
///
/// Panics if any of the RNG state pointers are missing.
pub fn set_rng_state(marker: MainThreadMarker, rng_state: RngState) {
    // Safety: these are all global buffers which are always valid.
    unsafe {
        *idum.get(marker) = rng_state.idum;
        *ran1_iy.get(marker) = rng_state.iy;
        *ran1_iv.get(marker) = rng_state.iv;
    }
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

/// Prepends the command to the engine command buffer.
///
/// If `command` contains null-bytes, up to the first null-byte will be inserted.
///
/// # Panics
///
/// Panics if `Cbuf_InsertText` was not found.
pub fn prepend_command(marker: MainThreadMarker, command: &str) {
    let command = match CString::new(command) {
        Ok(command) => command,
        Err(nul_error) => {
            let nul_position = nul_error.nul_position();
            let mut bytes = nul_error.into_vec();
            bytes.truncate(nul_position);
            CString::new(bytes).unwrap()
        }
    };

    // Safety: Cbuf_InsertText() uses a global buffer which is zeroed by default. It means that
    // before it is initialized its max size equals to 0, which will trigger the error condition in
    // Cbuf_InsertText() early. The error condition calls Con_Printf(), which is also safe (see
    // safety comment in [`con_print()`]).
    unsafe {
        Cbuf_InsertText.get(marker)(command.as_ptr());
    }
}

/// Returns the current game resolution (width, height).
pub unsafe fn get_resolution(marker: MainThreadMarker) -> (i32, i32) {
    let should_use_window_rect = !VideoMode_IsWindowed.is_set(marker)
        || !VideoMode_GetCurrentVideoMode.is_set(marker)
        || VideoMode_IsWindowed.get(marker)() != 0;

    if should_use_window_rect {
        let rect = *window_rect.get(marker);
        (rect.right - rect.left, rect.bottom - rect.top)
    } else {
        let mut width = 0;
        let mut height = 0;
        VideoMode_GetCurrentVideoMode.get(marker)(&mut width, &mut height, null_mut());
        (width, height)
    }
}

pub unsafe fn player_edict(marker: MainThreadMarker) -> Option<NonNull<edict_s>> {
    // SAFETY: we're not calling any engine functions while the reference is alive.
    let offset = client_s_edict_offset.get(marker)?;
    let svs_ = &*svs.get_opt(marker)?;
    if svs_.num_clients == 0 || svs_.clients.is_null() {
        None
    } else {
        NonNull::new(*svs_.clients.add(offset).cast())
    }
}

/// # Safety
///
/// [`reset_pointers()`] must be called before hw is unloaded so the pointers don't go stale.
#[cfg(unix)]
#[instrument(name = "engine::find_pointers", skip(marker))]
unsafe fn find_pointers(marker: MainThreadMarker) {
    use libc::{RTLD_NOLOAD, RTLD_NOW};
    use libloading::os::unix::Library;

    let engine = Library::open(Some("hw.so"), RTLD_NOW | RTLD_NOLOAD).unwrap();
    let bxt = Library::open(Some("libBunnymodXT.so"), RTLD_NOW | RTLD_NOLOAD);

    for pointer in POINTERS {
        // Search in BXT first. If a function exists in BXT we want to call the BXT version so BXT
        // can run its hooks too and then dispatch to the engine function.
        let ptr = if let Ok(ref bxt) = bxt {
            bxt.get(pointer.symbol())
                .ok()
                .and_then(|sym| NonNull::new(*sym))
        } else {
            None
        };

        let ptr = ptr.or_else(|| {
            engine
                .get(pointer.symbol())
                .ok()
                .and_then(|sym| NonNull::new(*sym))
        });

        pointer.set(marker, ptr);
    }

    cl_stats.set(marker, cl.offset(marker, 174892));
    cl_viewent.set(marker, cl.offset(marker, 1717500));
    cl_viewent_viewmodel.set(marker, cl_viewent.offset(marker, 2888));
    cls_demoframecount.set(marker, cls.offset(marker, 16776));
    cls_demos.set(marker, cls.offset(marker, 15960));
    frametime_remainder.set(marker, CL_Move.by_offset(marker, 452));
    idum.set(marker, ran1.by_offset(marker, 2));
    ran1_iy.set(marker, ran1.by_offset(marker, 13));
    ran1_iv.set(marker, ran1.by_offset(marker, 116));
    r_refdef_vieworg.set(marker, r_refdef.offset(marker, 112));
    r_refdef_viewangles.set(marker, r_refdef_vieworg.offset(marker, 12));
    client_s_edict_offset.set(marker, Some(19076));

    for pointer in POINTERS {
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
#[instrument(skip(marker))]
pub unsafe fn find_pointers(marker: MainThreadMarker, base: *mut c_void, size: usize) {
    use std::slice;

    use rayon::prelude::*;

    // Find all pattern-based pointers.
    {
        let memory: &[u8] = slice::from_raw_parts(base.cast(), size);
        POINTERS.par_iter().for_each(|pointer| {
            // SAFETY: This is not the main thread, but the accesses here are all disjoint and
            // synchronized to the main thread.
            let marker = unsafe { MainThreadMarker::new() };
            let base: *mut c_void = memory.as_ptr().cast_mut().cast();
            if let Some((offset, index)) = pointer.patterns().find(memory) {
                pointer.set_with_index(
                    marker,
                    NonNull::new_unchecked(base.add(offset)),
                    Some(index),
                );
            }
        });
    }

    // Find all offset-based pointers.
    let ptr = &CL_GameDir_f;
    match ptr.pattern_index(marker) {
        // 6153
        Some(0) => com_gamedir.set(marker, ptr.by_offset(marker, 11)),
        // CoF-5936
        Some(1) => com_gamedir.set(marker, ptr.by_offset(marker, 14)),
        _ => (),
    }

    let ptr = &CL_Parse_LightStyle;
    match ptr.pattern_index(marker) {
        // 8684
        Some(0) => cl_lightstyle.set(marker, ptr.by_offset(marker, 75)),
        _ => (),
    }

    let ptr = &ClientDLL_Init;
    match ptr.pattern_index(marker) {
        // 6153
        Some(0) => cl_funcs.set(marker, ptr.by_offset(marker, 187)),
        _ => (),
    }

    let ptr = &CL_Move;
    match ptr.pattern_index(marker) {
        // 6153
        Some(0) => frametime_remainder.set(marker, ptr.by_offset(marker, 451)),
        _ => (),
    }

    let ptr = &CL_PlayDemo_f;
    match ptr.pattern_index(marker) {
        // 8684
        Some(0) => cls_demoframecount.set(marker, ptr.by_offset(marker, 735)),
        _ => (),
    }

    let ptr = &Cmd_AddMallocCommand;
    match ptr.pattern_index(marker) {
        // 6153
        Some(0) => cmd_functions.set(marker, ptr.by_offset(marker, 43)),
        // 4554
        Some(1) => cmd_functions.set(marker, ptr.by_offset(marker, 40)),
        // CoF-5936
        Some(2) => cmd_functions.set(marker, ptr.by_offset(marker, 46)),
        _ => (),
    }

    let ptr = &Cvar_RegisterVariable;
    match ptr.pattern_index(marker) {
        // 6153
        Some(0) => cvar_vars.set(marker, ptr.by_offset(marker, 124)),
        // 4554
        Some(1) => cvar_vars.set(marker, ptr.by_offset(marker, 122)),
        // CoF-5936
        Some(2) => cvar_vars.set(marker, ptr.by_offset(marker, 183)),
        _ => (),
    }

    let ptr = &Host_InitializeGameDLL;
    match ptr.pattern_index(marker) {
        // 6153
        Some(0) => {
            svs.set(marker, ptr.by_offset(marker, 26));
            LoadEntityDLLs.set_if_empty(marker, ptr.by_relative_call(marker, 69));
            gEntityInterface.set(marker, ptr.by_offset(marker, 75));
        }
        // CoF-5936
        Some(2) => {
            svs.set(marker, ptr.by_offset(marker, 74));
            LoadEntityDLLs.set_if_empty(marker, ptr.by_relative_call(marker, 114));
            gEntityInterface.set(marker, ptr.by_offset(marker, 123));
        }
        _ => (),
    }

    let ptr = &Host_FilterTime;
    match ptr.pattern_index(marker) {
        // 6153
        Some(0) => {
            host_frametime.set(marker, ptr.by_offset(marker, 64));
            realtime.set(marker, ptr.by_offset(marker, 70));
        }
        // 4554
        Some(1) => {
            host_frametime.set(marker, ptr.by_offset(marker, 65));
            realtime.set(marker, ptr.by_offset(marker, 71));
        }
        // 3248
        Some(2) => {
            host_frametime.set(marker, ptr.by_offset(marker, 67));
            realtime.set(marker, ptr.by_offset(marker, 73));
        }
        // 1712
        Some(3) => {
            host_frametime.set(marker, ptr.by_offset(marker, 363));
            realtime.set(marker, ptr.by_offset(marker, 14));
        }
        // CoF-5936
        Some(4) => {
            host_frametime.set(marker, ptr.by_offset(marker, 57));
            realtime.set(marker, ptr.by_offset(marker, 63));
        }
        _ => (),
    }

    let ptr = &Host_NextDemo;
    match ptr.pattern_index(marker) {
        // 6153
        Some(0) => {
            Cbuf_InsertText.set(marker, ptr.by_relative_call(marker, 140));
            cls_demos.set(marker, ptr.by_offset(marker, 11));
        }
        // 4554
        Some(1) => {
            Cbuf_InsertText.set(marker, ptr.by_relative_call(marker, 137));
            cls_demos.set(marker, ptr.by_offset(marker, 1));
        }
        // 1712
        Some(2) => {
            Cbuf_InsertText.set(marker, ptr.by_relative_call(marker, 132));
            cls_demos.set(marker, ptr.by_offset(marker, 1));
        }
        // CoF-5936
        Some(3) => {
            Cbuf_InsertText.set(marker, ptr.by_relative_call(marker, 165));
            cls_demos.set(marker, ptr.by_offset(marker, 11));
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
        // 4554
        Some(1) => {
            Cmd_Argc.set(marker, ptr.by_relative_call(marker, 25));
            Cmd_Argv.set(marker, ptr.by_relative_call(marker, 144));
        }
        // 3248
        Some(2) => {
            Cmd_Argc.set(marker, ptr.by_relative_call(marker, 24));
            Cmd_Argv.set(marker, ptr.by_relative_call(marker, 143));
        }
        // 1712
        Some(3) => {
            Cmd_Argc.set(marker, ptr.by_relative_call(marker, 25));
            Cmd_Argv.set(marker, ptr.by_relative_call(marker, 151));
        }
        // CoF-5936
        Some(4) => {
            Cmd_Argc.set(marker, ptr.by_relative_call(marker, 26));
            Cmd_Argv.set(marker, ptr.by_relative_call(marker, 180));
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
        // CoF-5936
        Some(1) => {
            sv.set(marker, ptr.by_offset(marker, 50));
            cls.set(marker, ptr.by_offset(marker, 105));
            Con_Printf.set_if_empty(marker, ptr.by_relative_call(marker, 34));
        }
        _ => (),
    }

    let ptr = &GL_BeginRendering;
    match ptr.pattern_index(marker) {
        // 6153
        Some(0) => {
            VideoMode_IsWindowed.set_if_empty(marker, ptr.by_relative_call(marker, 24));
            VideoMode_GetCurrentVideoMode.set_if_empty(marker, ptr.by_relative_call(marker, 79));
            window_rect.set(marker, ptr.by_offset(marker, 43));
        }
        // 4554
        Some(1) => {
            window_rect.set(marker, ptr.by_offset(marker, 31));
        }
        // CoF-5936
        Some(2) => {
            window_rect.set(marker, ptr.by_offset(marker, 29));
        }
        _ => (),
    }

    let ptr = &Key_Event;
    match ptr.pattern_index(marker) {
        // 6153
        Some(0) => {
            Cbuf_AddText.set_if_empty(marker, ptr.by_relative_call(marker, 462));
        }
        // 4554
        Some(1) => {
            Cbuf_AddText.set_if_empty(marker, ptr.by_relative_call(marker, 475));
        }
        // 1712
        Some(2) => {
            Cbuf_AddText.set_if_empty(marker, ptr.by_relative_call(marker, 301));
        }
        // CoF-5936
        Some(3) => {
            Cbuf_AddText.set_if_empty(marker, ptr.by_relative_call(marker, 528));
        }
        _ => (),
    }

    let ptr = &R_LoadSkys;
    match ptr.pattern_index(marker) {
        // 8684
        Some(0) => {
            gLoadSky.set(marker, ptr.by_offset(marker, 7));
            movevars.set(
                marker,
                ptr.by_offset(marker, 395)
                    .and_then(|ptr| NonNull::new(ptr.as_ptr().sub(68))),
            );
        }
        _ => (),
    }

    let ptr = &R_DrawViewModel;
    match ptr.pattern_index(marker) {
        // 8684
        Some(0) => {
            cl_stats.set(marker, ptr.by_offset(marker, 129));
        }
        _ => (),
    }

    let ptr = &R_RenderView;
    match ptr.pattern_index(marker) {
        // 8684
        Some(0) => {
            r_refdef_vieworg.set(
                marker,
                ptr.by_offset(marker, 129)
                    .and_then(|ptr| NonNull::new(ptr.as_ptr().sub(28))),
            );

            r_refdef_viewangles.set(
                marker,
                ptr.by_offset(marker, 129)
                    .and_then(|ptr| NonNull::new(ptr.as_ptr().sub(16))),
            );
        }
        _ => (),
    }

    let ptr = &R_SetFrustum;
    match ptr.pattern_index(marker) {
        // 6153
        Some(0) => {
            scr_fov_value.set(marker, ptr.by_offset(marker, 13));
        }
        // 4554
        Some(1) => {
            scr_fov_value.set(marker, ptr.by_offset(marker, 10));
        }
        // CoF-5936
        Some(2) => {
            scr_fov_value.set(marker, ptr.by_offset(marker, 7));
        }
        _ => (),
    }

    let ptr = &ran1;
    match ptr.pattern_index(marker) {
        // 6153
        Some(0) => {
            idum.set(marker, ptr.by_offset(marker, 2));
            ran1_iy.set(marker, ptr.by_offset(marker, 13));
            ran1_iv.set(marker, ptr.by_offset(marker, 97));
        }
        // CoF-5936
        Some(1) => {
            idum.set(marker, ptr.by_offset(marker, 8));
            ran1_iy.set(marker, ptr.by_offset(marker, 17));
            ran1_iv.set(marker, ptr.by_offset(marker, 197));
        }
        _ => (),
    }

    let ptr = &ReleaseEntityDlls;
    match ptr.pattern_index(marker) {
        // 6153
        Some(0) => {
            svs.set(marker, ptr.by_offset(marker, 23));
        }
        // CoF-5936
        Some(1) => {
            svs.set(marker, ptr.by_offset(marker, 31));
        }
        _ => (),
    }

    let ptr = &S_PaintChannels;
    match ptr.pattern_index(marker) {
        // 6153
        Some(0) => {
            paintedtime.set(marker, ptr.by_offset(marker, 4));
            paintbuffer.set(marker, ptr.by_offset(marker, 60));
        }
        // 4554
        Some(1) => {
            paintedtime.set(marker, ptr.by_offset(marker, 1));
            paintbuffer.set(marker, ptr.by_offset(marker, 56));
        }
        // CoF-5936
        Some(2) => {
            paintedtime.set(marker, ptr.by_offset(marker, 7));
            paintbuffer.set(marker, ptr.by_offset(marker, 78));
        }
        _ => (),
    }

    let ptr = &S_TransferStereo16;
    match ptr.pattern_index(marker) {
        // 6153
        Some(0) => {
            shm.set(marker, ptr.by_offset(marker, 337));
        }
        // 4554
        Some(1) => {
            shm.set(marker, ptr.by_offset(marker, 308));
        }
        // 3248
        Some(2) => {
            shm.set(marker, ptr.by_offset(marker, 307));
        }
        // CoF-5936
        Some(3) => {
            shm.set(marker, ptr.by_offset(marker, 347));
        }
        _ => (),
    }

    let ptr = &SV_ExecuteClientMessage;
    match ptr.pattern_index(marker) {
        // 8684
        Some(0) => {
            client_s_edict_offset.set(
                marker,
                ptr.by_offset(marker, 144).map(|ptr| ptr.as_ptr() as usize),
            );
            pmove.set(marker, ptr.by_offset(marker, 171));
            g_svmove.set(marker, ptr.by_offset(marker, 175));
        }
        // CoF-5936
        Some(1) => {
            client_s_edict_offset.set(
                marker,
                ptr.by_offset(marker, 188).map(|ptr| ptr.as_ptr() as usize),
            );
            pmove.set(marker, ptr.by_offset(marker, 212));
            g_svmove.set(marker, ptr.by_offset(marker, 216));
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
        // CoF-5936
        Some(1) => {
            sv.set(marker, ptr.by_offset(marker, 5));
            host_frametime.set(marker, ptr.by_offset(marker, 16));
        }
        _ => (),
    }

    let ptr = &SV_RunCmd;
    match ptr.pattern_index(marker) {
        // 8684
        Some(0) => {
            sv_areanodes.set(marker, ptr.by_offset(marker, 2488));
            SV_AddLinksToPM.set(marker, ptr.by_relative_call(marker, 2493));
        }
        // 4554
        Some(1) => {
            sv_areanodes.set(marker, ptr.by_offset(marker, 2484));
            SV_AddLinksToPM.set(marker, ptr.by_relative_call(marker, 2489));
        }
        // CoF-5936
        Some(2) => {
            sv_areanodes.set(marker, ptr.by_offset(marker, 2450));
            SV_AddLinksToPM.set(marker, ptr.by_relative_call(marker, 2455));
        }
        _ => (),
    }

    let ptr = &V_RenderView;
    match ptr.pattern_index(marker) {
        // 8684
        Some(0) => {
            cl_viewent.set(marker, ptr.by_offset(marker, 146));
            cl_viewent_viewmodel.set(marker, cl_viewent.offset(marker, 2888));
        }
        _ => (),
    }

    let ptr = &ClientDLL_DrawTransparentTriangles;
    match ptr.pattern_index(marker) {
        // 8684
        Some(0) => {
            tri.set(
                marker,
                ptr.by_offset(marker, 15)
                    .and_then(|ptr| NonNull::new(ptr.as_ptr().sub(4))),
            );
        }
        // CoF-5936
        Some(1) => {
            tri.set(
                marker,
                ptr.by_offset(marker, 22)
                    .and_then(|ptr| NonNull::new(ptr.as_ptr().sub(4))),
            );
        }
        _ => (),
    }

    for pointer in POINTERS {
        pointer.log(marker);
    }

    debug!(
        "{:?}: client_s_edict_offset",
        client_s_edict_offset.get(marker)
    );

    // Hook only Memory_Init() and the rest later, for BXT compatibility.
    maybe_hook(marker, &Memory_Init);
}

#[cfg(windows)]
unsafe fn maybe_hook(marker: MainThreadMarker, pointer: &dyn PointerTrait) {
    use minhook_sys::*;

    if !pointer.is_set(marker) {
        return;
    }

    let hook_fn = pointer.hook_fn();
    if hook_fn.is_null() {
        return;
    }

    let original = pointer.get_raw(marker);
    let mut trampoline = null_mut();
    assert_eq!(
        MH_CreateHook(original.as_ptr(), hook_fn, &mut trampoline),
        MH_OK
    );

    ORIGINAL_FUNCTIONS
        .borrow_mut(marker)
        .push(original.as_ptr());

    pointer.set_with_index(
        marker,
        NonNull::new_unchecked(trampoline),
        pointer.pattern_index(marker),
    );

    assert_eq!(MH_EnableHook(original.as_ptr()), MH_OK);
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
            assert_eq!(unsafe { MH_RemoveHook(function) }, MH_OK);
        }
    }
}

use exported::*;

/// Functions exported for `LD_PRELOAD` hooking.
pub mod exported {
    #![allow(clippy::missing_safety_doc)]

    use super::*;
    use crate::gl;
    use crate::hooks::client;

    #[export_name = "Memory_Init"]
    pub unsafe extern "C" fn my_Memory_Init(buf: *mut c_void, size: c_int) -> c_int {
        abort_on_panic(move || {
            let marker = MainThreadMarker::new();

            // This is the first function that we hook called on Linux, so do due initialization.
            ensure_logging_hooks();

            #[cfg(unix)]
            find_pointers(marker);

            #[cfg(windows)]
            {
                // Hook all found pointers on Windows.
                for &pointer in POINTERS {
                    if pointer.symbol() == Memory_Init.symbol() {
                        // Memory_Init() is already hooked.
                        continue;
                    }

                    maybe_hook(marker, pointer);
                }
            }

            // GL_SetMode() happens before Memory_Init(), which means the OpenGL context has already
            // been created and made current.
            sdl::find_pointers(marker);
            #[cfg(windows)]
            opengl32::find_pointers(marker);
            bxt::find_pointers(marker);

            let rv = Memory_Init.get(marker)(buf, size);

            cvars::register_all_cvars(marker);
            commands::register_all_commands(marker);
            cvars::deregister_disabled_module_cvars(marker);
            commands::deregister_disabled_module_commands(marker);

            tas_optimizer::maybe_start_client_connection_thread(marker);
            tas_studio::maybe_start_client_connection_thread(marker);

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

            #[cfg(windows)]
            opengl32::reset_pointers(marker);
            sdl::reset_pointers(marker);
            reset_pointers(marker);
        })
    }

    #[export_name = "Mod_LeafPVS"]
    pub unsafe extern "C" fn my_Mod_LeafPVS(
        leaf: *mut mleaf_s,
        model: *mut model_s,
    ) -> *mut c_void {
        abort_on_panic(move || {
            let marker = MainThreadMarker::new();

            if novis::is_active(marker) {
                Mod_LeafPVS.get(marker)((*model).leafs, model)
            } else {
                Mod_LeafPVS.get(marker)(leaf, model)
            }
        })
    }

    #[export_name = "R_DrawSequentialPoly"]
    pub unsafe extern "C" fn my_R_DrawSequentialPoly(
        surf: *mut c_void,
        face: *mut c_int,
    ) -> *mut c_void {
        abort_on_panic(move || {
            let marker = MainThreadMarker::new();

            // R_DrawSequentialPoly is used instead of some top-level drawing functions because we
            // want NPCs to remain opaque, to make them more visible. This function draws the
            // worldspawn and other brush entities but not studio models (NPCs).
            wallhack::with_wallhack(marker, move || R_DrawSequentialPoly.get(marker)(surf, face))
        })
    }

    #[export_name = "R_Clear"]
    pub unsafe extern "C" fn my_R_Clear() -> *mut c_void {
        abort_on_panic(move || {
            let marker = MainThreadMarker::new();

            // Half-Life normally doesn't clear the screen every frame, which is a problem with
            // wallhack as there's no solid background. Removing the skybox also removes the solid
            // background. Finally, while in the TAS editor we're frequently out of bounds, so we
            // want to clear to make it easier to see.
            if wallhack::is_active(marker)
                || skybox_remove::is_active(marker)
                || tas_studio::should_clear(marker)
            {
                if let Some(gl) = gl::GL.borrow(marker).as_ref() {
                    gl.ClearColor(0., 0., 0., 1.);
                    gl.Clear(gl::COLOR_BUFFER_BIT);
                }
            }

            R_Clear.get(marker)()
        })
    }

    #[export_name = "R_DrawSkyBox"]
    pub unsafe extern "C" fn my_R_DrawSkyBox() {
        abort_on_panic(move || {
            let marker = MainThreadMarker::new();

            if skybox_remove::is_active(marker) {
                return;
            }

            R_DrawSkyBox.get(marker)();
        })
    }

    #[export_name = "R_DrawViewModel"]
    pub unsafe extern "C" fn my_R_DrawViewModel() {
        abort_on_panic(move || {
            let marker = MainThreadMarker::new();

            if viewmodel_remove::is_removed(marker) {
                return;
            }

            R_DrawViewModel.get(marker)();
        })
    }

    #[export_name = "R_LoadSkys"]
    pub unsafe extern "C" fn my_R_LoadSkys() {
        abort_on_panic(move || {
            let marker = MainThreadMarker::new();

            skybox_change::with_changed_name(marker, move || R_LoadSkys.get(marker)());
        })
    }

    #[export_name = "R_PreDrawViewModel"]
    pub unsafe extern "C" fn my_R_PreDrawViewModel() {
        abort_on_panic(move || {
            let marker = MainThreadMarker::new();

            if viewmodel_remove::is_removed(marker) {
                return;
            }

            R_PreDrawViewModel.get(marker)();
        })
    }

    #[export_name = "V_ApplyShake"]
    pub unsafe extern "C" fn my_V_ApplyShake(
        origin: *mut [f32; 3],
        angles: *mut [f32; 3],
        factor: c_float,
    ) {
        abort_on_panic(move || {
            let marker = MainThreadMarker::new();

            if shake_remove::is_active(marker) {
                return;
            }

            V_ApplyShake.get(marker)(origin, angles, factor);
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

    #[export_name = "V_RenderView"]
    pub unsafe extern "C" fn my_V_RenderView() {
        abort_on_panic(move || {
            let marker = MainThreadMarker::new();

            campath::capture_motion(marker);

            V_RenderView.get(marker)()
        })
    }

    #[export_name = "CL_ViewDemo_f"]
    pub unsafe extern "C" fn my_CL_ViewDemo_f() {
        abort_on_panic(move || {
            let marker = MainThreadMarker::new();

            campath::load(marker);

            CL_ViewDemo_f.get(marker)();
        })
    }

    #[export_name = "SCR_DrawLoading"]
    pub unsafe extern "C" fn my_SCR_DrawLoading() {
        abort_on_panic(move || {
            let marker = MainThreadMarker::new();

            if disable_loading_text::is_active(marker) {
                return;
            }

            SCR_DrawLoading.get(marker)();
        })
    }

    #[export_name = "SCR_DrawPause"]
    pub unsafe extern "C" fn my_SCR_DrawPause() {
        abort_on_panic(move || {
            let marker = MainThreadMarker::new();

            if !tas_studio::should_draw_pause(marker) {
                return;
            }

            SCR_DrawPause.get(marker)();
        })
    }

    #[export_name = "SV_Frame"]
    pub unsafe extern "C" fn my_SV_Frame() {
        abort_on_panic(move || {
            let marker = MainThreadMarker::new();

            tas_logging::begin_physics_frame(marker);
            tas_recording::on_sv_frame_start(marker);

            SV_Frame.get(marker)();

            tas_recording::on_sv_frame_end(marker);
            tas_logging::end_physics_frame(marker);
        })
    }

    #[export_name = "_Z18Sys_VID_FlipScreenv"]
    pub unsafe extern "C" fn my_Sys_VID_FlipScreen() {
        abort_on_panic(move || {
            let marker = MainThreadMarker::new();

            capture::capture_frame(marker);

            Sys_VID_FlipScreen.get(marker)();

            #[cfg(feature = "tracy-client")]
            if let Some(client) = tracy_client::Client::running() {
                client.frame_mark();
            }
        })
    }

    pub unsafe extern "system" fn my_Sys_VID_FlipScreen_old(hdc: *mut c_void) {
        abort_on_panic(move || {
            let marker = MainThreadMarker::new();

            capture::capture_frame(marker);

            Sys_VID_FlipScreen_old.get(marker)(hdc);
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

    #[export_name = "S_PaintChannels"]
    pub unsafe extern "C" fn my_S_PaintChannels(end_time: c_int) {
        abort_on_panic(move || {
            let marker = MainThreadMarker::new();

            if capture::skip_paint_channels(marker) {
                return;
            }

            S_PaintChannels.get(marker)(end_time);
        })
    }

    #[export_name = "S_TransferStereo16"]
    pub unsafe extern "C" fn my_S_TransferStereo16(end: c_int) {
        abort_on_panic(move || {
            let marker = MainThreadMarker::new();

            capture::on_s_transfer_stereo_16(marker, end);

            S_TransferStereo16.get(marker)(end);
        })
    }

    #[export_name = "Host_FilterTime"]
    pub unsafe extern "C" fn my_Host_FilterTime(time: c_float) -> c_int {
        abort_on_panic(move || {
            let marker = MainThreadMarker::new();

            let skip = capture::on_host_filter_time(marker);

            let rv = if skip {
                1
            } else {
                Host_FilterTime.get(marker)(time)
            };

            if rv != 0 {
                if capture_skip_non_gameplay::should_record_current_frame(marker) {
                    capture::time_passed(marker);
                }

                tas_optimizer::update_client_connection_condition(marker);
                tas_optimizer::maybe_receive_messages_from_remote_server(marker);

                tas_studio::update_client_connection_condition(marker);
                tas_studio::maybe_receive_messages_from_remote_server(marker);
            }

            rv
        })
    }

    #[export_name = "CL_Disconnect"]
    pub unsafe extern "C" fn my_CL_Disconnect() {
        abort_on_panic(move || {
            let marker = MainThreadMarker::new();

            capture_skip_non_gameplay::on_cl_disconnect(marker);

            if !capture_video_per_demo::on_cl_disconnect(marker) {
                capture::on_cl_disconnect(marker);
            }

            campath::on_cl_disconnect(marker);
            viewmodel_sway::on_cl_disconnnect(marker);

            CL_Disconnect.get(marker)();
        })
    }

    #[export_name = "Key_Event"]
    pub unsafe extern "C" fn my_Key_Event(key: c_int, down: c_int) {
        abort_on_panic(move || {
            let marker = MainThreadMarker::new();

            capture::on_key_event_start(marker);
            tas_recording::on_key_event_start(marker);

            Key_Event.get(marker)(key, down);

            tas_recording::on_key_event_end(marker);
            capture::on_key_event_end(marker);
        })
    }

    #[export_name = "Con_ToggleConsole_f"]
    pub unsafe extern "C" fn my_Con_ToggleConsole_f() {
        abort_on_panic(move || {
            let marker = MainThreadMarker::new();

            if !capture::prevent_toggle_console(marker) {
                Con_ToggleConsole_f.get(marker)();
            }
        })
    }

    #[export_name = "Host_NextDemo"]
    pub unsafe extern "C" fn my_Host_NextDemo() {
        abort_on_panic(move || {
            let marker = MainThreadMarker::new();

            Host_NextDemo.get(marker)();

            demo_playback::set_next_demo(marker);
        })
    }

    #[export_name = "R_RenderView"]
    pub unsafe extern "C" fn my_R_RenderView() {
        abort_on_panic(move || {
            let marker = MainThreadMarker::new();

            campath::override_view(marker);
            tas_studio::tas_playback_rendered_views(marker);

            R_RenderView.get(marker)();
        })
    }

    #[export_name = "R_SetFrustum"]
    pub unsafe extern "C" fn my_R_SetFrustum() {
        abort_on_panic(move || {
            let marker = MainThreadMarker::new();

            if let Some(fov) = force_fov::fov(marker) {
                *scr_fov_value.get(marker) = fov;
            }

            fix_widescreen::fix_widescreen_fov(marker);

            R_SetFrustum.get(marker)();
        })
    }

    #[export_name = "hudGetScreenInfo"]
    pub unsafe extern "C" fn my_hudGetScreenInfo(info: *mut SCREENINFO) -> c_int {
        abort_on_panic(move || {
            let marker = MainThreadMarker::new();

            let rv = hudGetScreenInfo.get(marker)(info);

            if rv != 0 {
                hud_scale::maybe_scale_screen_info(marker, &mut *info);
            }

            hud::update_screen_info(marker, *info);

            rv
        })
    }

    #[export_name = "ClientDLL_Init"]
    pub unsafe extern "C" fn my_ClientDLL_Init() {
        abort_on_panic(move || {
            let marker = MainThreadMarker::new();

            ClientDLL_Init.get(marker)();

            client::hook_client_interface(marker);
        })
    }

    #[export_name = "DrawCrosshair"]
    pub unsafe extern "C" fn my_DrawCrosshair(x: c_int, y: c_int) {
        abort_on_panic(move || {
            let marker = MainThreadMarker::new();

            hud_scale::with_scaled_projection_matrix(marker, move || {
                let scale = hud_scale::scale(marker).unwrap_or(1.);

                DrawCrosshair.get(marker)((x as f32 / scale) as i32, (y as f32 / scale) as i32)
            });
        })
    }

    #[export_name = "CL_Move"]
    pub unsafe extern "C" fn my_CL_Move() {
        abort_on_panic(move || {
            let marker = MainThreadMarker::new();

            tas_recording::on_cl_move(marker);

            CL_Move.get(marker)();
        })
    }

    #[export_name = "CL_Parse_LightStyle"]
    pub unsafe extern "C" fn my_CL_Parse_LightStyle() {
        abort_on_panic(move || {
            let marker = MainThreadMarker::new();

            CL_Parse_LightStyle.get(marker)();

            lightstyle::on_cl_parse_lightstyle(marker);
        })
    }

    #[export_name = "CL_PlayDemo_f"]
    pub unsafe extern "C" fn my_CL_PlayDemo_f() {
        abort_on_panic(move || {
            let marker = MainThreadMarker::new();

            capture_video_per_demo::on_before_cl_playdemo_f(marker);
            campath::load(marker);

            CL_PlayDemo_f.get(marker)();

            capture_video_per_demo::on_after_cl_playdemo_f(marker);
        })
    }

    #[export_name = "Cbuf_AddText"]
    pub unsafe extern "C" fn my_Cbuf_AddText(text: *const c_char) {
        abort_on_panic(move || {
            let marker = MainThreadMarker::new();

            let text = comment_overflow_fix::strip_prefix_comments(text);
            let text = scoreboard_remove::strip_showscores(marker, text);

            if tas_studio::should_skip_command(marker, text) {
                return;
            }

            tas_recording::on_cbuf_addtext(marker, text);

            Cbuf_AddText.get(marker)(text);
        })
    }

    #[export_name = "Cbuf_AddFilteredText"]
    pub unsafe extern "C" fn my_Cbuf_AddFilteredText(text: *const c_char) {
        abort_on_panic(move || {
            let marker = MainThreadMarker::new();

            let text = comment_overflow_fix::strip_prefix_comments(text);
            let text = scoreboard_remove::strip_showscores(marker, text);

            Cbuf_AddFilteredText.get(marker)(text);
        })
    }

    #[export_name = "Cbuf_AddTextToBuffer"]
    pub unsafe extern "C" fn my_Cbuf_AddTextToBuffer(text: *const c_char, buffer: *mut c_void) {
        abort_on_panic(move || {
            let marker = MainThreadMarker::new();

            let text = comment_overflow_fix::strip_prefix_comments(text);
            let text = scoreboard_remove::strip_showscores(marker, text);

            Cbuf_AddTextToBuffer.get(marker)(text, buffer);
        })
    }

    #[export_name = "SV_AddLinksToPM_"]
    pub unsafe extern "C" fn my_SV_AddLinksToPM_(
        node: *mut c_void,
        mins: *mut [f32; 3],
        maxs: *mut [f32; 3],
    ) {
        abort_on_panic(move || {
            let marker = MainThreadMarker::new();

            player_movement_tracing::maybe_adjust_distance_limit(marker, mins, maxs);

            SV_AddLinksToPM_.get(marker)(node, mins, maxs);
        })
    }

    #[export_name = "_ZN7CBaseUI10HideGameUIEv"]
    pub unsafe extern "fastcall" fn my_CBaseUI__HideGameUI(this: *mut c_void) {
        abort_on_panic(move || {
            let marker = MainThreadMarker::new();

            tas_studio::maybe_prevent_unpause(marker, || {
                CBaseUI__HideGameUI.get(marker)(this);
            });
        })
    }
}
