use anyhow::Result;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

/// Watch a directory recursively, calling `on_change` with 250ms debounce.
pub async fn watch_recursive(
    root: &Path,
    mut cancel: tokio::sync::watch::Receiver<bool>,
    on_change: impl Fn() + Send + 'static,
) -> Result<()> {
    let (tx, mut rx) = mpsc::channel::<()>(16);
    let on_change = Arc::new(on_change);

    let mut watcher = RecommendedWatcher::new(
        move |res: notify::Result<notify::Event>| {
            if res.is_ok() {
                let _ = tx.blocking_send(());
            }
        },
        Config::default(),
    )?;

    watcher.watch(root, RecursiveMode::Recursive)?;

    let debounce = Duration::from_millis(250);

    loop {
        tokio::select! {
            _ = cancel.changed() => {
                break;
            }
            msg = rx.recv() => {
                if msg.is_none() {
                    break;
                }
                tokio::time::sleep(debounce).await;
                while rx.try_recv().is_ok() {}
                on_change();
            }
        }
    }

    drop(watcher);
    Ok(())
}
