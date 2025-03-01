//! TAS logging.

use std::ffi::{CStr, OsString};
use std::io;
use std::path::Path;

use git_version::git_version;

use super::Module;
use crate::ffi::edict;
use crate::ffi::playermove::{playermove_s, usercmd_s};
use crate::handler;
use crate::hooks::engine::{self, con_print, RngState};
use crate::hooks::server;
use crate::modules::commands::{self, Command};
use crate::modules::cvars::{self, CVar};
use crate::utils::*;

mod serializer;
use serializer::Serializer;

pub struct TasLogging;
impl Module for TasLogging {
    fn name(&self) -> &'static str {
        "bxt_tas_log"
    }

    fn description(&self) -> &'static str {
        "Logging the player state during TAS playback.\n\n\
        This is a useful subset of `bxt_tas_log`, including RNG state dumping, \
        when you can't use the one from the original Bunnymod XT."
    }

    fn commands(&self) -> &'static [&'static Command] {
        static COMMANDS: &[&Command] = &[&BXT_TAS_LOG];
        COMMANDS
    }

    fn cvars(&self) -> &'static [&'static CVar] {
        static CVARS: &[&CVar] = &[&BXT_TAS_LOG_FILENAME, &BXT_TAS_LOG_WRITE_FULL_RNG_STATE];
        CVARS
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        commands::Commands.is_enabled(marker) && engine::SV_Frame.is_set(marker)
    }
}

static BXT_TAS_LOG: Command = Command::new(
    b"bxt_tas_log\0",
    handler!(
        "bxt_tas_log <0|1>

Enables (`1`) or disables (`0`) TAS logging into the file at `bxt_tas_log_filename`.",
        tas_log as fn(_, _)
    ),
);

static BXT_TAS_LOG_FILENAME: CVar = CVar::new(
    b"bxt_tas_log_filename\0",
    b"taslogger.log\0",
    "Filename of the log file to write.",
);
static BXT_TAS_LOG_WRITE_FULL_RNG_STATE: CVar = CVar::new(
    b"_bxt_tas_log_write_full_rng_state\0",
    b"0\0",
    "\
Set to `1` to write the full engine RNG state every frame.

This massively increases the log file size while being seldom needed, so it's not enabled by \
default.",
);

static TAS_LOG: MainThreadRefCell<Option<TasLog>> = MainThreadRefCell::new(None);

fn tas_log(marker: MainThreadMarker, enabled: i32) {
    if !TasLogging.is_enabled(marker) {
        return;
    }

    let mut tas_log = TAS_LOG.borrow_mut(marker);

    if enabled == 0 {
        if let Some(tas_log) = tas_log.take() {
            if let Err(err) = tas_log.close() {
                con_print(
                    marker,
                    &format!("TAS logging stopped with an error: {err}\n"),
                );
            } else {
                con_print(marker, "TAS logging stopped.\n");
            }
        }

        return;
    }

    if tas_log.is_some() {
        // Already logging.
        return;
    }

    let filename = if cvars::CVars.is_enabled(marker) {
        BXT_TAS_LOG_FILENAME.to_os_string(marker)
    } else {
        OsString::from("taslogger.log")
    };

    let build_number = engine::build_number.get_opt(marker).map(|f| unsafe { f() });

    // Safety: the reference does not outlive this command handler, and com_gamedir can only be
    // modified at engine start and while setting the HD models or the addon folder.
    let game_dir = engine::com_gamedir
        .get_opt(marker)
        .map(|dir| unsafe { CStr::from_ptr(dir.cast()).to_string_lossy() });

    match TasLog::new(
        &filename,
        &format!(
            "{} {}",
            env!("CARGO_PKG_NAME"),
            git_version!(cargo_prefix = "cargo:", fallback = "unknown")
        ),
        build_number,
        game_dir.as_deref(),
    ) {
        Ok(tas_log_new) => {
            con_print(
                marker,
                &format!("Started TAS logging into {}\n", filename.to_string_lossy()),
            );

            *tas_log = Some(tas_log_new)
        }
        Err(err) => con_print(marker, &format!("Unable to start TAS logging: {err}\n")),
    }
}

/// # Safety
///
/// `host_frametime`, `cls` and `sv` must be valid to read from.
pub unsafe fn begin_physics_frame(marker: MainThreadMarker) {
    if let Some(tas_log) = TAS_LOG.borrow_mut(marker).as_mut() {
        let frame_time = engine::host_frametime
            .get_opt(marker)
            .map(|frame_time| *frame_time);
        let client_state = engine::cls.get_opt(marker).map(|cls| (*cls).state);
        let is_paused = engine::sv.get_opt(marker).map(|sv| *sv.offset(4).cast());

        // TODO: command_buffer
        if let Err(err) = tas_log.begin_physics_frame(
            frame_time,
            client_state,
            is_paused,
            None,
            engine::rng_state(marker),
            BXT_TAS_LOG_WRITE_FULL_RNG_STATE.as_bool(marker),
        ) {
            con_print(marker, &format!("Error writing to the TAS log: {err}"));
        }
    }
}

pub fn end_physics_frame(marker: MainThreadMarker) {
    if let Some(tas_log) = TAS_LOG.borrow_mut(marker).as_mut() {
        if let Err(err) = tas_log.end_physics_frame() {
            con_print(marker, &format!("Error writing to the TAS log: {err}"));
        }
    }
}

pub fn begin_cmd_frame(marker: MainThreadMarker, cmd: usercmd_s, random_seed: u32) {
    // PM_Move is required because it ends the cmd frame JSON object.
    if !server::PM_Move.is_set(marker) {
        return;
    }

    if let Some(tas_log) = TAS_LOG.borrow_mut(marker).as_mut() {
        if let Err(err) = tas_log.begin_cmd_frame(None, None, &cmd, random_seed) {
            con_print(marker, &format!("Error writing to the TAS log: {err}"));
        }
    }
}

/// # Safety
///
/// `ppmove` must be valid to read from.
pub unsafe fn write_pre_pm_state(marker: MainThreadMarker, ppmove: *const playermove_s) {
    // CmdStart is required because it starts the cmd frame JSON object.
    if !server::CmdStart.is_set(marker) {
        return;
    }

    if let Some(tas_log) = TAS_LOG.borrow_mut(marker).as_mut() {
        if let Err(err) = tas_log.write_pre_pm_state(&*ppmove) {
            con_print(marker, &format!("Error writing to the TAS log: {err}"));
        }
    }
}

/// # Safety
///
/// `ppmove` must be valid to read from.
pub unsafe fn write_post_pm_state(marker: MainThreadMarker, ppmove: *const playermove_s) {
    // CmdStart is required because it starts the cmd frame JSON object.
    if !server::CmdStart.is_set(marker) {
        return;
    }

    if let Some(tas_log) = TAS_LOG.borrow_mut(marker).as_mut() {
        if let Err(err) = tas_log.write_post_pm_state(&*ppmove) {
            con_print(marker, &format!("Error writing to the TAS log: {err}"));
        }
    }
}

pub fn end_cmd_frame(marker: MainThreadMarker) {
    // CmdStart is required because it starts the cmd frame JSON object.
    if !server::CmdStart.is_set(marker) {
        return;
    }

    if let Some(tas_log) = TAS_LOG.borrow_mut(marker).as_mut() {
        if let Err(err) = tas_log.end_cmd_frame() {
            con_print(marker, &format!("Error writing to the TAS log: {err}"));
        }
    }
}

struct TasLog {
    ser: Serializer,
}

impl TasLog {
    fn new<P: AsRef<Path>>(
        path: P,
        tool_version: &str,
        build_number: Option<i32>,
        game_dir: Option<&str>,
    ) -> Result<Self, io::Error> {
        let mut ser = Serializer::new(path)?;

        ser.begin_object()?;
        ser.entry("tool_ver", tool_version)?;

        if let Some(build_number) = build_number {
            ser.entry("build", &build_number)?;
        }
        if let Some(game_dir) = game_dir {
            ser.entry("mod", game_dir)?;
        }

        ser.key("pf")?;
        ser.begin_object_value()?;
        ser.begin_array()?;

        Ok(Self { ser })
    }

    fn close(mut self) -> Result<(), io::Error> {
        self.ser.end_array()?;
        self.ser.end_object_value()?;
        self.ser.end_object()?;
        Ok(())
    }

    fn begin_physics_frame(
        &mut self,
        frame_time: Option<f64>,
        client_state: Option<i32>,
        is_paused: Option<bool>,
        command_buffer: Option<&str>,
        rng_state: Option<RngState>,
        write_full_rng_state: bool,
    ) -> Result<(), io::Error> {
        self.ser.begin_array_value()?;
        self.ser.begin_object()?;

        if let Some(frame_time) = frame_time {
            self.ser.entry("ft", &frame_time)?;
        }

        if let Some(client_state) = client_state {
            if client_state != 5 {
                self.ser.entry("cls", &client_state)?;
            }
        }

        if let Some(is_paused) = is_paused {
            if is_paused {
                self.ser.entry("p", &is_paused)?;
            }
        }

        if let Some(command_buffer) = command_buffer {
            self.ser.entry("cbuf", command_buffer)?;
        }

        if let Some(rng_state) = rng_state {
            self.ser.key("rng")?;
            self.ser.begin_object_value()?;
            self.ser.begin_object()?;
            self.ser.entry("idum", &rng_state.idum)?;

            if write_full_rng_state {
                self.ser.entry("iy", &rng_state.iy)?;
                self.ser.entry("iv", &rng_state.iv)?;
            }

            self.ser.end_object()?;
            self.ser.end_object_value()?;
        }

        self.ser.key("cf")?;
        self.ser.begin_object_value()?;
        self.ser.begin_array()?;

        Ok(())
    }

    fn end_physics_frame(&mut self) -> Result<(), io::Error> {
        self.ser.end_array()?;
        self.ser.end_object_value()?;

        // TODO: console messages, damage, object move.

        self.ser.end_object()?;
        self.ser.end_array_value()?;
        Ok(())
    }

    fn begin_cmd_frame(
        &mut self,
        frame_bulk_id: Option<usize>,
        frame_time_remainder: Option<f64>,
        cmd: &usercmd_s,
        shared_seed: u32,
    ) -> Result<(), io::Error> {
        self.ser.begin_array_value()?;
        self.ser.begin_object()?;

        if let Some(frame_bulk_id) = frame_bulk_id {
            self.ser.entry("bid", &frame_bulk_id)?;
        }
        if let Some(frame_time_remainder) = frame_time_remainder {
            self.ser.entry("rem", &frame_time_remainder)?;
        }

        self.ser.entry("ms", &cmd.msec)?;
        self.ser.entry("btns", &cmd.buttons)?;
        self.ser.entry("impls", &cmd.impulse)?;
        self.ser
            .entry("fsu", &[cmd.forwardmove, cmd.sidemove, cmd.upmove])?;
        self.ser.entry(
            "view",
            &[cmd.viewangles[1], cmd.viewangles[0], cmd.viewangles[2]],
        )?;

        self.ser.entry("ss", &shared_seed)?;

        // TODO: health, armor.

        Ok(())
    }

    fn end_cmd_frame(&mut self) -> Result<(), io::Error> {
        self.ser.end_object()?;
        self.ser.end_array_value()?;
        Ok(())
    }

    fn write_pre_pm_state(&mut self, pmove: &playermove_s) -> Result<(), io::Error> {
        if pmove.friction != 1. {
            self.ser.entry("efric", &pmove.friction)?;
        }
        if pmove.gravity != 1. {
            self.ser.entry("egrav", &pmove.gravity)?;
        }
        if pmove.punchangle.iter().any(|x| *x != 0.) {
            self.ser.entry(
                "pview",
                &[
                    pmove.punchangle[1],
                    pmove.punchangle[0],
                    pmove.punchangle[2],
                ],
            )?;
        }

        self.ser.key("prepm")?;
        self.ser.begin_object_value()?;
        self.ser.begin_object()?;

        self.write_pm_state(pmove)?;

        self.ser.end_object()?;
        self.ser.end_object_value()?;

        Ok(())
    }

    fn write_post_pm_state(&mut self, pmove: &playermove_s) -> Result<(), io::Error> {
        self.ser.key("postpm")?;
        self.ser.begin_object_value()?;
        self.ser.begin_object()?;

        self.write_pm_state(pmove)?;

        self.ser.end_object()?;
        self.ser.end_object_value()?;

        Ok(())
    }

    fn write_pm_state(&mut self, pmove: &playermove_s) -> Result<(), io::Error> {
        self.ser.entry("pos", &pmove.origin)?;
        self.ser.entry("vel", &pmove.velocity)?;
        self.ser.entry("og", &(pmove.onground != -1))?;
        if pmove.basevelocity.iter().any(|x| *x != 0.) {
            self.ser.entry("bvel", &pmove.basevelocity)?;
        }
        if pmove.waterlevel != 0 {
            self.ser.entry("wlvl", &pmove.waterlevel)?;
        }
        if pmove.flags.contains(edict::Flags::FL_DUCKING) {
            self.ser.entry("dst", &2)?;
        } else if pmove.bInDuck != 0 {
            self.ser.entry("dst", &1)?;
        }

        // TODO: ladder

        Ok(())
    }
}
