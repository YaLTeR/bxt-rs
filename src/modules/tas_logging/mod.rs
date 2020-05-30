//! TAS logging.

use std::{
    ffi::{CStr, OsString},
    io,
    path::Path,
};

use super::Module;
use crate::{
    handler,
    hooks::engine::{self, Engine},
    modules::{
        commands::{self, Command},
        cvars::{self, CVar},
    },
    utils::{MainThreadMarker, MainThreadRefCell},
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

    let build_number = if engine::BUILD_NUMBER.is_set(marker) {
        unsafe { engine::BUILD_NUMBER.get(marker)() }
    } else {
        -1
    };

    let game_dir = if engine::COM_GAMEDIR.is_set(marker) {
        // Safety: the reference does not outlive this command handler, and com_gamedir can only be
        // modified at engine start and while setting the HD models or the addon folder.
        unsafe { CStr::from_ptr(engine::COM_GAMEDIR.get(marker).as_ptr().cast()).to_string_lossy() }
    } else {
        "".into()
    };

    match TASLog::new(&filename, "bxt-rs 0.1", build_number, &game_dir) {
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

struct TASLog {
    ser: Serializer,
}

impl TASLog {
    fn new<P: AsRef<Path>>(
        path: P,
        tool_version: &str,
        build_number: i32,
        game_dir: &str,
    ) -> Result<Self, io::Error> {
        let mut ser = Serializer::new(path)?;

        ser.begin_object()?;
        ser.entry("tool_ver", tool_version)?;
        ser.entry("build", &build_number)?;
        ser.entry("game_dir", game_dir)?;

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
}
