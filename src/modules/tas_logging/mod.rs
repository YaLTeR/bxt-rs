//! TAS logging.

use std::{
    ffi::{CStr, OsString},
    io,
    path::Path,
};

use super::Module;
use crate::{
    ffi::{edict, playermove::playermove_s, usercmd::usercmd_s},
    handler,
    hooks::{
        engine::{self, Engine},
        server,
    },
    modules::{
        commands::{self, Command},
        cvars::{self, CVar},
    },
    utils::*,
};

mod serializer;
use serializer::Serializer;

pub struct TASLogging;
impl Module for TASLogging {
    fn name(&self) -> &'static str {
        "bxt_taslog"
    }

    fn commands(&self) -> &'static [&'static Command] {
        static COMMANDS: &[&Command] = &[&BXT_TASLOG];
        &COMMANDS
    }

    fn cvars(&self) -> &'static [&'static CVar] {
        static CVARS: &[&CVar] = &[&BXT_TASLOG_FILENAME];
        &CVARS
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        commands::Commands.is_enabled(marker)
            && engine::SV_FRAME.is_set(marker)
            // CmdStart starts a JSON object which PM_Move ends. Therefore for valid JSON they
            // should either both be found, or both not found.
            && (server::CMD_START.is_set(marker) == server::PM_MOVE.is_set(marker))
    }
}

static BXT_TASLOG: Command = Command {
    name: b"bxt_taslog\0",
    function: handler!(
        "Usage: bxt_taslog <0|1>\n \
          Enables or disables TAS logging into the file at bxt_taslog_filename.\n",
        taslog as fn(_, _)
    ),
};

static BXT_TASLOG_FILENAME: CVar = CVar::new(b"bxt_taslog_filename\0", b"taslogger.log\0");

static TAS_LOG: MainThreadRefCell<Option<TASLog>> = MainThreadRefCell::new(None);

fn taslog(engine: &Engine, enabled: i32) {
    let marker = engine.marker();

    if !TASLogging.is_enabled(marker) {
        return;
    }

    let mut tas_log = TAS_LOG.borrow_mut(marker);

    if enabled == 0 {
        if let Some(tas_log) = tas_log.take() {
            if let Err(err) = tas_log.close() {
                engine.print(&format!("TAS logging stopped with an error: {}\n", err));
            } else {
                engine.print("TAS logging stopped.\n");
            }
        }

        return;
    }

    if tas_log.is_some() {
        // Already logging.
        return;
    }

    let filename = if cvars::CVars.is_enabled(marker) {
        BXT_TASLOG_FILENAME.to_os_string(marker)
    } else {
        OsString::from("taslogger.log")
    };

    let build_number = engine::BUILD_NUMBER.get_opt(marker).map(|f| unsafe { f() });

    // Safety: the reference does not outlive this command handler, and com_gamedir can only be
    // modified at engine start and while setting the HD models or the addon folder.
    let game_dir = engine::COM_GAMEDIR
        .get_opt(marker)
        .map(|dir| unsafe { CStr::from_ptr(dir.cast()).to_string_lossy() });

    match TASLog::new(&filename, "bxt-rs 0.1", build_number, game_dir.as_deref()) {
        Ok(tas_log_new) => {
            engine.print(&format!(
                "Started TAS logging into {}\n",
                filename.to_string_lossy()
            ));

            *tas_log = Some(tas_log_new)
        }
        Err(err) => engine.print(&format!("Unable to start TAS logging: {}\n", err)),
    }
}

/// # Safety
///
/// This function must only be called right before `SV_Frame()`.
pub unsafe fn on_sv_frame_start(engine: &Engine) {
    let marker = engine.marker();

    if let Some(tas_log) = TAS_LOG.borrow_mut(marker).as_mut() {
        let frame_time = engine::HOST_FRAMETIME
            .get_opt(marker)
            .map(|frame_time| *frame_time);
        let client_state = engine::CLS.get_opt(marker).map(|cls| *cls.cast());
        let is_paused = engine::SV.get_opt(marker).map(|sv| *sv.offset(4).cast());

        // TODO: command_buffer
        if let Err(err) = tas_log.begin_physics_frame(frame_time, client_state, is_paused, None) {
            engine.print(&format!("Error writing to the TAS log: {}", err));
        }
    }
}

/// # Safety
///
/// This function must only be called right after `SV_Frame()`.
pub unsafe fn on_sv_frame_end(engine: &Engine) {
    let marker = engine.marker();

    if let Some(tas_log) = TAS_LOG.borrow_mut(marker).as_mut() {
        if let Err(err) = tas_log.end_physics_frame() {
            engine.print(&format!("Error writing to the TAS log: {}", err));
        }
    }
}

/// # Safety
///
/// This function must only be called right before `CmdStart()`.
pub unsafe fn on_cmd_start(engine: &Engine, cmd: usercmd_s, random_seed: u32) {
    let marker = engine.marker();

    if let Some(tas_log) = TAS_LOG.borrow_mut(marker).as_mut() {
        if let Err(err) = tas_log.begin_cmd_frame(None, None, &cmd, random_seed) {
            engine.print(&format!("Error writing to the TAS log: {}", err));
        }
    }
}

/// # Safety
///
/// This function must only be called right before serverside `PM_Move()`.
pub unsafe fn on_pm_move_start(engine: &Engine, ppmove: *const playermove_s) {
    let marker = engine.marker();

    if let Some(tas_log) = TAS_LOG.borrow_mut(marker).as_mut() {
        if let Err(err) = tas_log.write_pre_pm_state(&*ppmove) {
            engine.print(&format!("Error writing to the TAS log: {}", err));
        }
    }
}

/// # Safety
///
/// This function must only be called right after serverside `PM_Move()`.
pub unsafe fn on_pm_move_end(engine: &Engine, ppmove: *const playermove_s) {
    let marker = engine.marker();

    if let Some(tas_log) = TAS_LOG.borrow_mut(marker).as_mut() {
        if let Err(err) = tas_log.write_post_pm_state(&*ppmove) {
            engine.print(&format!("Error writing to the TAS log: {}", err));
        }
        if let Err(err) = tas_log.end_cmd_frame() {
            engine.print(&format!("Error writing to the TAS log: {}", err));
        }
    }
}

struct TASLog {
    ser: Serializer,
}

impl TASLog {
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
