//! Watching files for changes made by other programs.
//!
//! Folders are watched rather than files: editors often save by writing a
//! new file and renaming it over the old one, which a watch on the file
//! itself would lose. Changes are debounced: a path is reported once it has
//! been quiet for a while, so a save that arrives as several events (truncate,
//! write, rename) reloads once.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use notify::{RecommendedWatcher, RecursiveMode, Watcher as _};

/// How long a path must be quiet before it is reported.
pub const QUIET: Duration = Duration::from_millis(120);

/// Watches folders and reports files that changed in them.
pub struct Watcher {
    inner: Option<RecommendedWatcher>,
    events: Receiver<notify::Result<notify::Event>>,
    folders: HashSet<PathBuf>,
    pending: HashMap<PathBuf, Instant>,
}

impl std::fmt::Debug for Watcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Watcher")
            .field("folders", &self.folders)
            .field("pending", &self.pending.len())
            .finish_non_exhaustive()
    }
}

impl Default for Watcher {
    fn default() -> Watcher {
        Watcher::new()
    }
}

impl Watcher {
    /// A watcher; without the platform's notifications (rare) it reports
    /// nothing.
    pub fn new() -> Watcher {
        let (tx, events) = channel();
        let inner = notify::recommended_watcher(move |e| {
            let _ = tx.send(e);
        })
        .ok();
        Watcher { inner, events, folders: HashSet::new(), pending: HashMap::new() }
    }

    /// Whether changes can be reported at all.
    pub fn available(&self) -> bool {
        self.inner.is_some()
    }

    /// Watches the folder a file is in (or a folder).
    pub fn watch(&mut self, path: &Path) {
        let folder = if path.is_dir() { path } else { path.parent().unwrap_or(path) };
        if self.folders.contains(folder) {
            return;
        }
        if let Some(w) = &mut self.inner
            && w.watch(folder, RecursiveMode::NonRecursive).is_ok()
        {
            self.folders.insert(folder.to_path_buf());
        }
    }

    /// Stops watching a folder.
    pub fn unwatch(&mut self, folder: &Path) {
        if self.folders.remove(folder)
            && let Some(w) = &mut self.inner
        {
            let _ = w.unwatch(folder);
        }
    }

    /// The watched folders.
    pub fn folders(&self) -> impl Iterator<Item = &Path> {
        self.folders.iter().map(PathBuf::as_path)
    }

    /// Files changed (written, created, removed, renamed) that have been
    /// quiet for [`QUIET`] by `now`.
    pub fn poll(&mut self, now: Instant) -> Vec<PathBuf> {
        while let Ok(event) = self.events.try_recv() {
            let Ok(event) = event else { continue };
            if matches!(event.kind, notify::EventKind::Access(_)) {
                continue;
            }
            for p in event.paths {
                self.pending.insert(p, Instant::now());
            }
        }
        let mut ready: Vec<PathBuf> = self
            .pending
            .iter()
            .filter(|(_, t)| now.saturating_duration_since(**t) >= QUIET)
            .map(|(p, _)| p.clone())
            .collect();
        for p in &ready {
            self.pending.remove(p);
        }
        ready.sort();
        ready
    }

    /// Whether changes are waiting to settle (so the caller keeps polling).
    pub fn busy(&self) -> bool {
        !self.pending.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_a_change_once_it_settles() {
        let dir = std::env::temp_dir().join(format!("mgv-watch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("a.mdl");
        std::fs::write(&file, "one").unwrap();
        let mut w = Watcher::new();
        if !w.available() {
            eprintln!("skipped: no file notifications on this system");
            return;
        }
        w.watch(&file);
        std::fs::write(&file, "two").unwrap();
        let start = Instant::now();
        let mut seen = Vec::new();
        while start.elapsed() < Duration::from_secs(5) {
            seen = w.poll(Instant::now());
            if !seen.is_empty() {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let file = file.canonicalize().unwrap();
        assert!(seen.iter().any(|p| p.canonicalize().ok().as_ref() == Some(&file)), "{seen:?}");
        // Reported once.
        std::thread::sleep(QUIET * 2);
        assert!(w.poll(Instant::now()).is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
