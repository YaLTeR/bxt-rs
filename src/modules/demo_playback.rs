//! Playing multiple demos at once.

use std::ffi::{CStr, OsStr};
use std::path::{Path, PathBuf};

use byte_slice_cast::AsSliceOf;

use super::commands::Command;
use super::Module;
use crate::handler;
use crate::hooks::engine::{self, con_print, prepend_command};
use crate::utils::*;

pub struct DemoPlayback;
impl Module for DemoPlayback {
    fn name(&self) -> &'static str {
        "Multiple demo playback"
    }

    fn description(&self) -> &'static str {
        "Playing multiple demos at once."
    }

    fn commands(&self) -> &'static [&'static Command] {
        static COMMANDS: &[&Command] = &[&BXT_PLAY_RUN, &BXT_PLAY_FOLDER];
        COMMANDS
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
        "bxt_play_run <name>

Plays back all `name_N.dem` demos in order.",
        play_run as fn(_, _)
    ),
);

static BXT_PLAY_FOLDER: Command = Command::new(
    b"bxt_play_folder\0",
    handler!(
        "bxt_play_folder <folder>
        
Plays back all demos in the folder in alphabetic order.",
        play_folder as fn(_, _)
    ),
);

fn play_run(marker: MainThreadMarker, prefix: PathBuf) {
    play(marker, &prefix, run_demos_by_number);
}

fn play_folder(marker: MainThreadMarker, folder: PathBuf) {
    play(marker, &folder, demos_alphabetically);
}

fn play<F, I>(marker: MainThreadMarker, name: &Path, enumerate_demos: F)
where
    F: FnOnce(&Path) -> Result<I, String>,
    I: DoubleEndedIterator<Item = PathBuf>,
{
    if !DemoPlayback.is_enabled(marker) {
        return;
    }

    DEMOS.borrow_mut(marker).clear();

    let game_dir = Path::new(
        unsafe { CStr::from_ptr(engine::com_gamedir.get(marker).cast()) }
            .to_str()
            .unwrap(),
    );
    let mut full_prefix: PathBuf = game_dir.into();
    full_prefix.push(name);

    let paths = match enumerate_demos(&full_prefix) {
        Ok(paths) => paths,
        Err(err) => {
            con_print(marker, &format!("Error: {}.\n", err));
            return;
        }
    };

    let paths = paths.map(|path| {
        path.strip_prefix(game_dir)
            .map(|path| path.to_owned())
            .unwrap_or(path)
    });

    queue_for_playing(marker, paths);
}

fn demos_in_folder(dir: &Path) -> Result<impl Iterator<Item = PathBuf>, String> {
    let files = dir
        .read_dir()
        .map_err(|_| format!("could not open directory {}", dir.to_string_lossy()))?;
    Ok(files
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().map(|ext| ext == "dem").unwrap_or(false)))
}

fn run_demos_by_number(prefix: &Path) -> Result<impl DoubleEndedIterator<Item = PathBuf>, String> {
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
    let mut demos: Vec<_> = demos_in_folder(dir)?
        .filter_map(|path| {
            path.file_stem()
                .and_then(OsStr::to_str)
                .filter(|name| name.starts_with(&name_prefix))
                .and_then(|name| name[name_prefix.len()..].parse::<usize>().ok())
                .map(|number| (path, number))
        })
        .collect();

    demos.sort_unstable_by_key(|(_, number)| *number);

    Ok(demos.into_iter().map(|(path, _)| path))
}

fn demos_alphabetically(dir: &Path) -> Result<impl DoubleEndedIterator<Item = PathBuf>, String> {
    let mut demos: Vec<PathBuf> = demos_in_folder(dir)?.collect();
    demos.sort_unstable();
    Ok(demos.into_iter())
}

pub fn queue_for_playing(
    marker: MainThreadMarker,
    paths: impl DoubleEndedIterator<Item = PathBuf>,
) {
    let mut demos = DEMOS.borrow_mut(marker);

    for mut path in paths.rev() {
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
                &format!("Error: filename {demo} is longer than 15 characters.\n"),
            );
            return;
        }

        let mut demo = demo.into_bytes();
        demo.push(0); // Add a trailing null-byte.
        demos.push(demo);
    }

    if demos.is_empty() {
        con_print(marker, "Error: no demos found.\n");
        return;
    }

    con_print(marker, &format!("Playing {} demos.\n", demos.len()));

    drop(demos);
    set_next_demo(marker);

    prepend_command(marker, "demos\n");
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
