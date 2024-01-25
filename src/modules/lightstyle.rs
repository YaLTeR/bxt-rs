//! `bxt_lightstyle`

use byte_slice_cast::AsSliceOf;

use super::commands::Command;
use super::Module;
use crate::handler;
use crate::hooks::engine;
use crate::modules::cvars::CVar;
use crate::utils::*;

pub struct LightStyle;
impl Module for LightStyle {
    fn name(&self) -> &'static str {
        "bxt_lightstyle"
    }

    fn description(&self) -> &'static str {
        "Change rendering light styles."
    }

    fn commands(&self) -> &'static [&'static Command] {
        static COMMANDS: &[&Command] = &[&BXT_LIGHTSTYLE_APPLY];
        COMMANDS
    }

    fn cvars(&self) -> &'static [&'static CVar] {
        static CVARS: &[&CVar] = &[&BXT_LIGHTSTYLE_CUSTOM, &BXT_LIGHTSTYLE];
        CVARS
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        engine::cl_lightstyle.is_set(marker) || engine::CL_Parse_LightStyle.is_set(marker)
    }
}

static BXT_LIGHTSTYLE: CVar = CVar::new(
    b"bxt_lightstyle\0",
    b"0\0",
    "\
Preset controls. Must invoke apply command to take effects. Persists across level changes.

0: Off
1: Maximum brightness
2: Full bright
3: Maximum darkness
4: Mildy darker",
);
static BXT_LIGHTSTYLE_CUSTOM: CVar = CVar::new(
    b"bxt_lightstyle_custom\0",
    b"\0",
    "\
Custom controls. Takes precedence over preset when using bxt_lightstyle_apply.

First value is effect. Second value is amount.
E.g.: bxt_lightstyle_custom \"1 nomnomnom\".
",
);

static BXT_LIGHTSTYLE_APPLY: Command = Command::new(
    b"bxt_lightstyle_apply\0",
    handler!(
        "Apply lightstyle changes. Takes an optional argument for instantly applying a preset.",
        apply_from_cvars as fn(_),
        apply_preset as fn(_, _)
    ),
);

pub fn apply_from_cvars(marker: MainThreadMarker) {
    if !BXT_LIGHTSTYLE_CUSTOM.to_string(marker).is_empty() {
        // 0 and "m" is default normal
        let s = BXT_LIGHTSTYLE_CUSTOM.to_string(marker);
        let mut args = s.split_ascii_whitespace();
        let index = args.next().unwrap_or("0").parse().unwrap_or(0);
        let lightinfo = args.next().unwrap_or("m");
        apply(marker, index, lightinfo);
    } else {
        apply_preset(marker, BXT_LIGHTSTYLE.as_u64(marker))
    }
}

fn apply_preset(marker: MainThreadMarker, preset: u64) {
    let lightinfo = match preset {
        0 => "m",
        1 => "z",
        2 => "#",
        3 => "a",
        4 => "g", // from someone else's personal preference
        _ => "m",
    };
    apply(marker, 0, lightinfo);
}

fn apply(marker: MainThreadMarker, index: u64, lightinfo: &str) {
    if !LightStyle.is_enabled(marker) {
        return;
    }

    if index > 63 {
        return;
    }

    if lightinfo.len() > 64 {
        return;
    }

    unsafe {
        let cl_lightstyle = &mut *engine::cl_lightstyle.get(marker);

        cl_lightstyle[index as usize].map[..lightinfo.len()]
            .copy_from_slice(lightinfo.as_slice_of().unwrap());
        cl_lightstyle[index as usize].length = lightinfo.len() as i32;
    }
}
