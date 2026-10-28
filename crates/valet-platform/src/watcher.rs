//! Recursive file watcher: `notify` + `notify-debouncer-mini`, 2 s debounce.
//!
//! The callback receives batches of changed paths. A watched root itself in a
//! batch means "events may have been lost under this root, rescan all of it":
//! that is sent when the OS reports a queue overflow or the watcher reports an
//! error.

use anyhow::{Context, Result};
use notify::{
    Event, EventHandler, EventKind, RecommendedWatcher, RecursiveMode, Watcher, WatcherKind,
};
use notify_debouncer_mini::{Config, DebounceEventResult, Debouncer, new_debouncer_opt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

/// Quiet period before a batch of changes is delivered.
pub const DEBOUNCE: Duration = Duration::from_secs(2);

/// Handle returned by [`watch_paths`].
///
/// Call [`WatchCancel::cancel`] to stop watching. Dropping the handle without
/// calling `cancel` leaves the watcher running until the process exits.
pub struct WatchCancel {
    stop_tx: Option<mpsc::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
    active: Arc<AtomicBool>,
}

impl WatchCancel {
    /// Stops watching and waits for the watcher thread to exit. After this
    /// returns no new callback starts; one already running may still finish.
    pub fn cancel(mut self) {
        self.active.store(false, Ordering::SeqCst);
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Watches every path in `paths` recursively and calls `on_events` with each
/// debounced batch of changed paths (sorted, no duplicates).
///
/// Returns an error if the watcher cannot be created or any path cannot be
/// watched; in that case nothing is watched.
pub async fn watch_paths<F>(paths: Vec<PathBuf>, on_events: F) -> Result<WatchCancel>
where
    F: Fn(Vec<PathBuf>) + Send + Sync + 'static,
{
    let active = Arc::new(AtomicBool::new(true));
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel::<Result<()>>();
    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    let thread_active = Arc::clone(&active);

    // Setting up recursive watches can walk large trees (inotify), so it runs
    // on its own thread instead of blocking the async executor.
    let thread = std::thread::Builder::new()
        .name("valet-watcher".into())
        .spawn(move || {
            let debouncer = match start(&paths, on_events, thread_active) {
                Ok(d) => {
                    let _ = ready_tx.send(Ok(()));
                    d
                }
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                    return;
                }
            };
            match stop_rx.recv() {
                // cancel(): dropping the debouncer stops its thread and the OS watches.
                Ok(()) => drop(debouncer),
                // Handle dropped without cancel(): keep watching for the life of the process.
                Err(_) => std::mem::forget(debouncer),
            }
        })
        .context("spawn watcher thread")?;

    ready_rx
        .await
        .context("watcher thread exited during setup")??;

    Ok(WatchCancel {
        stop_tx: Some(stop_tx),
        thread: Some(thread),
        active,
    })
}

fn start<F>(
    paths: &[PathBuf],
    on_events: F,
    active: Arc<AtomicBool>,
) -> Result<Debouncer<RescanWatcher>>
where
    F: Fn(Vec<PathBuf>) + Send + Sync + 'static,
{
    let roots = paths.to_vec();
    let handler = move |res: DebounceEventResult| {
        if active.load(Ordering::SeqCst) {
            deliver(res, &roots, &on_events);
        }
    };
    let config = Config::default().with_timeout(DEBOUNCE);
    let mut debouncer =
        new_debouncer_opt::<_, RescanWatcher>(config, handler).context("create file watcher")?;
    for p in paths {
        debouncer
            .watcher()
            .watch(p, RecursiveMode::Recursive)
            .with_context(|| format!("watch {}", p.display()))?;
    }
    Ok(debouncer)
}

/// Turns one debouncer result into one callback call.
fn deliver<F>(res: DebounceEventResult, roots: &[PathBuf], on_events: &F)
where
    F: Fn(Vec<PathBuf>),
{
    match res {
        Ok(events) => {
            let mut files: Vec<PathBuf> = events.into_iter().map(|e| e.path).collect();
            files.sort();
            files.dedup();
            if !files.is_empty() {
                on_events(files);
            }
        }
        // A watcher error can mean lost events: ask for a rescan of every root.
        Err(_) => {
            if !roots.is_empty() {
                on_events(roots.to_vec());
            }
        }
    }
}

/// `RecommendedWatcher` that replaces a "rescan needed" event (for example an
/// inotify queue overflow), which carries no path and would be dropped by the
/// debouncer, with one event per watched root.
struct RescanWatcher {
    inner: RecommendedWatcher,
    roots: Arc<Mutex<Vec<PathBuf>>>,
}

fn forward<H: EventHandler>(
    res: notify::Result<Event>,
    roots: &Mutex<Vec<PathBuf>>,
    handler: &mut H,
) {
    match res {
        Ok(ev) if ev.need_rescan() => {
            let roots = roots.lock().map(|r| r.clone()).unwrap_or_default();
            for root in roots {
                handler.handle_event(Ok(Event::new(EventKind::Other).add_path(root)));
            }
        }
        other => handler.handle_event(other),
    }
}

impl Watcher for RescanWatcher {
    fn new<F: EventHandler>(mut event_handler: F, config: notify::Config) -> notify::Result<Self> {
        let roots = Arc::new(Mutex::new(Vec::new()));
        let handler_roots = Arc::clone(&roots);
        let inner = RecommendedWatcher::new(
            move |res: notify::Result<Event>| forward(res, &handler_roots, &mut event_handler),
            config,
        )?;
        Ok(Self { inner, roots })
    }

    fn watch(&mut self, path: &Path, recursive_mode: RecursiveMode) -> notify::Result<()> {
        self.inner.watch(path, recursive_mode)?;
        if let Ok(mut roots) = self.roots.lock() {
            roots.push(path.to_path_buf());
        }
        Ok(())
    }

    fn unwatch(&mut self, path: &Path) -> notify::Result<()> {
        self.inner.unwatch(path)?;
        if let Ok(mut roots) = self.roots.lock() {
            roots.retain(|p| p != path);
        }
        Ok(())
    }

    fn configure(&mut self, option: notify::Config) -> notify::Result<bool> {
        self.inner.configure(option)
    }

    fn kind() -> WatcherKind {
        RecommendedWatcher::kind()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::Flag;
    use std::time::Instant;

    #[test]
    fn rescan_event_becomes_one_event_per_root() {
        let roots = Mutex::new(vec![PathBuf::from("/r1"), PathBuf::from("/r2")]);
        let got: Arc<Mutex<Vec<Event>>> = Arc::default();
        let sink = Arc::clone(&got);
        let mut h = move |res: notify::Result<Event>| sink.lock().unwrap().push(res.unwrap());
        let rescan = Event::new(EventKind::Other).set_flag(Flag::Rescan);
        forward(Ok(rescan), &roots, &mut h);
        let plain = Event::new(EventKind::Any).add_path(PathBuf::from("/r1/f"));
        forward(Ok(plain), &roots, &mut h);
        let events = std::mem::take(&mut *got.lock().unwrap());
        let paths: Vec<PathBuf> = events.into_iter().flat_map(|e| e.paths).collect();
        assert_eq!(
            paths,
            vec![
                PathBuf::from("/r1"),
                PathBuf::from("/r2"),
                PathBuf::from("/r1/f")
            ]
        );
    }

    #[test]
    fn watcher_error_requests_rescan_of_all_roots() {
        let roots = vec![PathBuf::from("/r1"), PathBuf::from("/r2")];
        let got = Mutex::new(Vec::new());
        deliver(Err(notify::Error::generic("overflow")), &roots, &|p| {
            got.lock().unwrap().push(p)
        });
        assert_eq!(got.into_inner().unwrap(), vec![roots]);
    }

    fn wait_for(rx: &mpsc::Receiver<Vec<PathBuf>>, name: &str, limit: Duration) -> bool {
        let deadline = Instant::now() + limit;
        while let Some(left) = deadline.checked_duration_since(Instant::now()) {
            match rx.recv_timeout(left) {
                Ok(batch) if batch.iter().any(|p| p.ends_with(name)) => return true,
                Ok(_) => {}
                Err(_) => return false,
            }
        }
        false
    }

    #[tokio::test]
    async fn delivers_events_and_stops_on_cancel() {
        let dir = tempfile::tempdir().unwrap();
        let (tx, rx) = mpsc::channel();
        let tx = Mutex::new(tx);
        let handle = watch_paths(vec![dir.path().to_path_buf()], move |batch| {
            let _ = tx.lock().unwrap().send(batch);
        })
        .await
        .unwrap();

        std::fs::write(dir.path().join("first.txt"), b"1").unwrap();
        assert!(wait_for(&rx, "first.txt", Duration::from_secs(10)));

        handle.cancel();
        std::fs::write(dir.path().join("second.txt"), b"2").unwrap();
        // Stopped means the debouncer thread exited and dropped the callback,
        // which closes the channel. No batch may arrive before that.
        match rx.recv_timeout(DEBOUNCE + Duration::from_secs(3)) {
            Ok(batch) => panic!("callback after cancel: {batch:?}"),
            Err(mpsc::RecvTimeoutError::Disconnected) => {}
            Err(mpsc::RecvTimeoutError::Timeout) => panic!("watcher still alive after cancel"),
        }
    }

    #[tokio::test]
    async fn missing_root_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let r = watch_paths(vec![dir.path().join("missing")], |_| {}).await;
        assert!(r.is_err());
    }
}
