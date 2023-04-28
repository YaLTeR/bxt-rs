//! Bridging to a `.hltas` on disk.

use std::fs::{self, File, OpenOptions};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use color_eyre::eyre::{self, Context};
use hltas::HLTAS;

use super::watcher::Watcher;

pub struct Bridge {
    path: PathBuf,
    watcher: Option<Watcher>,
    ignore_next: bool,
}

fn write_first_version(path: &Path, script: &HLTAS) -> eyre::Result<()> {
    // Create the target .hltas file. If the file already exists, rename it, and try again.
    let file = match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(file) => file,
        Err(err) if err.kind() == ErrorKind::AlreadyExists => {
            let mut stem = path.file_stem().unwrap_or_default().to_owned();
            stem.push("-backup.hltas");
            let backup_name = path.with_file_name(stem);

            debug!(
                "{} already exists, renaming it to {}",
                path.to_string_lossy(),
                backup_name.to_string_lossy(),
            );

            fs::rename(path, &backup_name).context("could not rename {path} to {backup_name}")?;

            OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .context("could not open {path}")?
        }
        Err(err) => return Err(err).context("could not open {path}"),
    };

    script
        .to_writer(file)
        .context("could not write HLTAS to file")?;

    Ok(())
}

impl Bridge {
    pub fn with_project_path(project_path: &Path, script: &HLTAS) -> Self {
        let mut stem = project_path.file_stem().unwrap_or_default().to_owned();
        stem.push("-bridged.hltas");
        let path = project_path.with_file_name(stem);

        let watcher = match write_first_version(&path, script) {
            Ok(()) => {
                // There are several non-atomicity issues with this code, i.e. if the file is
                // changed between the write above and when the watcher starts up below, it will not
                // get caught. However, that is okay. Even if we somehow miss an actual user write,
                // they can just save the file again.
                Some(Watcher::new(path.clone()))
            }
            Err(err) => {
                warn!("Error creating bridged .hltas, disabling bridge: {err:?}");
                None
            }
        };

        Self {
            path,
            watcher,
            ignore_next: false,
        }
    }

    pub fn new_script(&mut self) -> Option<HLTAS> {
        if !self.watcher.as_ref()?.has_changed() {
            return None;
        }

        if self.ignore_next {
            self.ignore_next = false;
            return None;
        }

        info!("reading updated bridged .hltas");

        match fs::read_to_string(&self.path) {
            Ok(input) => match HLTAS::from_str(&input) {
                Ok(script) => Some(script),
                Err(err) => {
                    warn!("Error parsing bridged .hltas: {err:?}");
                    None
                }
            },
            Err(err) => {
                warn!("Error reading bridged .hltas: {err:?}");
                None
            }
        }
    }

    pub fn update_on_disk(&mut self, new_script: &HLTAS) {
        if self.watcher.is_none() {
            return;
        }

        self.ignore_next = true;

        let file = match File::create(&self.path) {
            Ok(file) => file,
            Err(err) => {
                warn!("Error opening bridged .hltas: {err:?}");
                return;
            }
        };

        if let Err(err) = new_script.to_writer(file) {
            warn!("Error writing updated .hltas to file: {err:?}");
        }
    }
}
