//! File modification watcher.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

pub struct Watcher {
    has_changed: Arc<AtomicBool>,
    should_stop: Arc<AtomicBool>,
}

impl Drop for Watcher {
    fn drop(&mut self) {
        self.should_stop.store(true, Ordering::SeqCst);
    }
}

impl Watcher {
    pub fn new(path: PathBuf) -> Self {
        let has_changed = Arc::new(AtomicBool::new(false));
        let should_stop = Arc::new(AtomicBool::new(false));

        {
            let has_changed = has_changed.clone();
            let should_stop = should_stop.clone();
            thread::Builder::new()
                .name(format!("Filesystem Watcher for {}", path.to_string_lossy()))
                .spawn(move || {
                    let mut last_mtime = path.metadata().and_then(|meta| meta.modified()).ok();

                    loop {
                        thread::sleep(Duration::from_millis(500));

                        if should_stop.load(Ordering::SeqCst) {
                            trace!("Exiting watcher thread for {}", path.to_string_lossy());
                            break;
                        }

                        if let Ok(mtime) = path.metadata().and_then(|meta| meta.modified()) {
                            if last_mtime != Some(mtime) {
                                has_changed.store(true, Ordering::SeqCst);
                                last_mtime = Some(mtime);
                                trace!("file changed: {}", path.to_string_lossy());
                            }
                        }
                    }
                })
                .unwrap();
        }

        Self {
            has_changed,
            should_stop,
        }
    }

    pub fn has_changed(&self) -> bool {
        self.has_changed.swap(false, Ordering::SeqCst)
    }
}
