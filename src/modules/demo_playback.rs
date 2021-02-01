//! Playing multiple demos at once.

use std::{
    ffi::{CStr, OsStr},
    path::{Path, PathBuf},
};

use byte_slice_cast::AsSliceOf;

use super::{commands::Command, Module};
use crate::{
    handler,
    hooks::engine::{self, con_print, prepend_command},
    utils::*,
};

pub struct DemoPlayback;
impl Module for DemoPlayback {
    fn name(&self) -> &'static str {
        "Multiple demo playback"
    }

    fn commands(&self) -> &'static [&'static Command] {
        static COMMANDS: &[&Command] = &[&BXT_PLAY_RUN];
        &COMMANDS
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        engine::cls_demos.is_set(marker)
            && engine::com_gamedir.is_set(marker)
            && engine::Cbuf_InsertText.is_set(marker)
            && engine::Host_NextDemo.is_set(marker)
    }
}

static DEMOS: MainThreadRefCell<Vec<Vec<u8>>> = MainThreadRefCell::new(Vec::new());

static BXT_PLAY_RUN: Command = Command::new(
    b"bxt_play_run\0",
    handler!(
        "Usage: bxt_play_run <name>\n \
          Plays back all name_N.dem demos in order.\n",
        play_run as fn(_, _)
    ),
);

fn play_run(marker: MainThreadMarker, prefix: PathBuf) {
    if !DemoPlayback.is_enabled(marker) {
        return;
    }

    let game_dir = Path::new(
        unsafe { CStr::from_ptr(engine::com_gamedir.get(marker).cast()) }
            .to_str()
            .unwrap(),
    );
    let mut full_prefix: PathBuf = game_dir.into();
    full_prefix.push(&prefix);

    let mut demos = DEMOS.borrow_mut(marker);
    demos.clear();

    let paths = match find_demos(full_prefix) {
        Ok(paths) => paths,
        Err(err) => {
            con_print(marker, &format!("Error: {}.\n", err));
            return;
        }
    };

    if paths.is_empty() {
        con_print(marker, "Error: no demos found.\n");
        return;
    }

    for (path, _) in paths.into_iter().rev() {
        let mut path = path
            .strip_prefix(game_dir)
            .map(|path| path.into())
            .unwrap_or(path);

        // find_demos() already returns files with .dem extensions. We can strip them, but only if
        // there's no second extension: in that case, playdemo won't append .dem for us.
        if Path::new(path.file_stem().unwrap()).extension().is_none() {
            path.set_extension("");
        }

        debug!("Demo filename: {:?}", path);

        let demo = path.into_os_string().into_string().unwrap();

        // Since we're using only a single cls.demos entry at a time, it should be possible to use
        // the whole cls.demos storage to significantly increase the character limit, if needed.
        if demo.len() >= 16 {
            con_print(
                marker,
                &format!("Error: filename {} is longer than 15 characters.\n", demo),
            );
            return;
        }

        let mut demo = demo.into_bytes();
        demo.push(0); // Add a trailing null-byte.
        demos.push(demo);
    }

    con_print(marker, &format!("Playing {} demos.\n", demos.len()));

    drop(demos);
    set_next_demo(marker);

    prepend_command(marker, "demos\n");
}

fn find_demos(prefix: PathBuf) -> Result<Vec<(PathBuf, usize)>, String> {
    let name_prefix = match prefix.file_name() {
        Some(prefix) => {
            format!(
                "{}_",
                prefix
                    .to_str()
                    .ok_or_else(|| "the name contains invalid characters".to_string())?
            )
        }
        None => "".into(),
    };

    let dir = prefix.parent().unwrap();
    let mut demos: Vec<_> = dir
        .read_dir()
        .map_err(|_| format!("could not open directory {}", dir.to_string_lossy()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().map(|ext| ext == "dem").unwrap_or(false))
        .filter_map(|path| {
            path.file_stem()
                .and_then(OsStr::to_str)
                .filter(|name| name.starts_with(&name_prefix))
                .and_then(|name| name[name_prefix.len()..].parse::<usize>().ok())
                .map(|number| (path, number))
        })
        .collect();

    demos.sort_unstable_by_key(|(_, number)| *number);

    Ok(demos)
}

pub fn set_next_demo(marker: MainThreadMarker) {
    let mut demos = DEMOS.borrow_mut(marker);

    unsafe {
        // Safety: no engine functions are called while the reference is active.
        let cls_demos = &mut *engine::cls_demos.get(marker);

        match demos.pop() {
            Some(demo) => {
                // Replace the first startdemos entry with the next demo and set the next demo as
                // the first one.
                cls_demos.demonum = 0;

                let demo = demo.as_slice_of().unwrap();
                cls_demos.demos[0][..demo.len()].copy_from_slice(demo);
                cls_demos.demos[1][0] = 0;
            }
            None => {
                cls_demos.demonum = -1;
                cls_demos.demos[0][0] = 0;
            }
        }
    }
}
