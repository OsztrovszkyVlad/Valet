use anyhow::Result;
use notify::RecursiveMode;
use notify_debouncer_mini::{new_debouncer, DebouncedEvent};
use std::{path::PathBuf, time::Duration};

pub struct WatchCancel {
  cancel_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

impl WatchCancel {
  pub fn cancel(mut self) {
    if let Some(tx) = self.cancel_tx.take() {
      let _ = tx.send(());
    }
  }
}

pub async fn watch_paths<F>(paths: Vec<PathBuf>, on_events: F) -> Result<WatchCancel>
where
  F: Fn(Vec<PathBuf>) + Send + 'static,
{
  let (tx_cancel, rx_cancel) = tokio::sync::oneshot::channel::<()>();
  let mut paths_clone = paths.clone();

  // Handler closure required by notify-debouncer-mini (FnMut over DebounceEventResult)
  let handler = move |res: Result<Vec<DebouncedEvent>, notify::Error>| {
    if let Ok(events) = res {
      let files: Vec<PathBuf> = events.into_iter().map(|e| e.path).collect();
      on_events(files);
    }
  };

  // Keep the debouncer alive on a blocking thread
  tokio::task::spawn_blocking(move || {
    // 300ms debounce window; tune later
    let mut debouncer = new_debouncer(Duration::from_millis(300), handler)
      .expect("debouncer");

    for p in paths_clone.drain(..) {
      // Recursive watching
      let _ = debouncer.watcher().watch(&p, RecursiveMode::Recursive);
    }

    // Block this thread until cancelled
    let _ = rx_cancel.blocking_recv();
  });

  Ok(WatchCancel { cancel_tx: Some(tx_cancel) })
}
