//! Bridging to a `.hltas` on disk.

use std::fs::{self, File, OpenOptions};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};

use color_eyre::eyre::{self, Context};
use hltas::HLTAS;

use super::watcher::Watcher;

pub struct Bridge {
    path: PathBuf,
    watcher: Option<Watcher>,

    write_thread_handle: Option<JoinHandle<()>>,
    cvar_pair: Arc<(Mutex<Option<Request>>, Condvar)>,
    ignore_next: Arc<AtomicBool>,
}

enum Request {
    Stop,
    Write(HLTAS),
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

// Writing the file happens on a separate thread because for larger scripts it will block for
// noticeable amounts of time.
fn write_thread(
    path: PathBuf,
    cvar_pair: Arc<(Mutex<Option<Request>>, Condvar)>,
    ignore_next: Arc<AtomicBool>,
) {
    // Instead of writing directly to the bridged script, we write to a temp file and then rename.
    // This is because for larger scripts the writing takes long enough that the filesystem watcher
    // detects it twice, which we don't want. The rename strategy avoids that.
    let mut tmp_path = path.clone();
    let mut tmp_file_name = path.file_name().unwrap().to_owned();
    tmp_file_name.push("-temp");
    tmp_path.set_file_name(&tmp_file_name);

    let (lock, cvar) = &*cvar_pair;

    let mut request = lock.lock().unwrap();
    loop {
        match request.take() {
            None => {
                request = cvar.wait(request).unwrap();
            }
            Some(Request::Stop) => break,
            Some(Request::Write(script)) => {
                let file = match File::create(&tmp_path) {
                    Ok(file) => file,
                    Err(err) => {
                        warn!("Error opening temp bridged .hltas: {err:?}");
                        return;
                    }
                };

                if let Err(err) = script.to_writer(file) {
                    warn!("Error writing updated .hltas to temp file: {err:?}");
                }

                // Set ignore_next right before renaming. If we set it earlier, then with larger
                // scripts it's very easy to make two write requests in quick succession which will
                // be detected by the filesystem watcher, but only one of them will be ignored,
                // instead of both of them.
                ignore_next.store(true, Ordering::SeqCst);

                if let Err(err) = fs::rename(&tmp_path, &path) {
                    warn!("Error renaming temp .hltas to bridged file: {err:?}");
                }
            }
        }
    }
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

        let cvar_pair = Arc::new((Mutex::new(None), Condvar::new()));
        let ignore_next = Arc::new(AtomicBool::new(false));
        let write_thread_handle = {
            let path = path.clone();
            let cvar_pair = cvar_pair.clone();
            let ignore_next = ignore_next.clone();
            thread::Builder::new()
                .name(format!(
                    "HLTAS Bridge Writer for {}",
                    path.to_string_lossy()
                ))
                .spawn(move || write_thread(path, cvar_pair, ignore_next))
                .unwrap()
        };

        Self {
            path,
            watcher,

            write_thread_handle: Some(write_thread_handle),
            cvar_pair,
            ignore_next,
        }
    }

    pub fn new_script(&mut self) -> Option<HLTAS> {
        if !self.watcher.as_ref()?.has_changed() {
            return None;
        }

        if self.ignore_next.swap(false, Ordering::SeqCst) {
            return None;
        }

        info!("reading updated bridged .hltas");

        let _span = info_span!("bridge reading script").entered();

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

    pub fn update_on_disk(&mut self, new_script: HLTAS) {
        if self.watcher.is_none() {
            return;
        }

        let (lock, cvar) = &*self.cvar_pair;
        let mut request = lock.lock().unwrap();
        *request = Some(Request::Write(new_script));
        cvar.notify_one();
    }
}

impl Drop for Bridge {
    fn drop(&mut self) {
        // Ask the write thread to exit and wait for it. So that if we close the TAS editor and
        // reopen the same file, there's no possibility of an awkward race condition and non-working
        // bridge.
        let (lock, cvar) = &*self.cvar_pair;
        {
            let mut request = lock.lock().unwrap();
            *request = Some(Request::Stop);
            cvar.notify_one();
        }
        self.write_thread_handle.take().unwrap().join().unwrap();
    }
}
