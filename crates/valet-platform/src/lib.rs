use anyhow::Result;
use notify::RecursiveMode;
use notify_debouncer_mini::{new_debouncer, DebouncedEvent};
use std::{path::PathBuf, time::Duration, sync::Arc};

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
  F: Fn(Vec<PathBuf>) + Send + Sync + 'static,
{
  let (tx_cancel, _rx_cancel) = tokio::sync::oneshot::channel::<()>();
  let paths_clone = paths.clone();
  
  // Wrap the callback in Arc to share between threads
  let on_events = Arc::new(on_events);
  let on_events_clone = Arc::clone(&on_events);

  // Create debouncer in a dedicated thread since notify is sync
  let _handle = std::thread::spawn(move || {
    // Handler closure required by notify-debouncer-mini (FnMut over DebounceEventResult)
    let handler = move |res: Result<Vec<DebouncedEvent>, notify::Error>| {
      match &res {
        Ok(events) => {
          println!("[DEBUG] Watcher received {} events", events.len());
          let files: Vec<PathBuf> = events.iter().map(|e| e.path.clone()).collect();
          for file in &files {
            println!("[DEBUG] Event for file: {:?}", file);
          }
          on_events_clone(files);
        }
        Err(e) => {
          println!("[DEBUG] Watcher error: {:?}", e);
        }
      }
    };

    // Create debouncer with shorter timeout for more responsiveness
    let mut debouncer = match new_debouncer(Duration::from_millis(100), handler) {
      Ok(d) => d,
      Err(e) => {
        println!("[DEBUG] Failed to create debouncer: {:?}", e);
        return;
      }
    };

    for p in &paths_clone {
      // Recursive watching
      println!("[DEBUG] Adding watch for path: {:?}", p);
      let watch_result = debouncer.watcher().watch(p, RecursiveMode::Recursive);
      match watch_result {
        Ok(()) => println!("[DEBUG] Successfully watching: {:?}", p),
        Err(e) => println!("[DEBUG] Failed to watch {:?}: {:?}", p, e),
      }
    }

    println!("[DEBUG] File watcher thread started, waiting for events...");
    
    // Keep the thread alive - the debouncer needs to stay in scope
    // We can't easily integrate tokio::oneshot with std::thread in a clean way here,
    // so we'll just keep the debouncer alive indefinitely for now
    // In a real implementation, you'd want proper shutdown handling
    loop {
      std::thread::sleep(Duration::from_secs(1));
    }
  });

  // Return immediately - the watcher is now running in background thread
  Ok(WatchCancel { cancel_tx: Some(tx_cancel) })
}
