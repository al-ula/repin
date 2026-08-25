use crossbeam_channel::Sender;
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use repin_core::runtime::Engine;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

pub struct ProjectWatcher {
    _watcher: RecommendedWatcher,
    shutdown_tx: Sender<()>,
    thread_handle: Option<JoinHandle<()>>,
}

impl ProjectWatcher {
    pub fn start(
        project_root: &Path,
        engine: Arc<Engine>,
        closed: Arc<AtomicBool>,
        debounce_ms: u64,
    ) -> Result<Self, String> {
        let (event_tx, event_rx) = crossbeam_channel::unbounded::<notify::Result<Event>>();
        let (shutdown_tx, shutdown_rx) = crossbeam_channel::bounded::<()>(1);

        let mut watcher = RecommendedWatcher::new(
            move |res: notify::Result<Event>| {
                if let Ok(event) = &res
                    && event.kind.is_access()
                {
                    return;
                }
                let _ = event_tx.send(res);
            },
            Config::default(),
        )
        .map_err(|err| format!("failed to initialize file watcher: {err}"))?;

        watcher
            .watch(project_root, RecursiveMode::Recursive)
            .map_err(|err| format!("failed to watch project root: {err}"))?;

        let root = project_root.to_path_buf();
        let debounce_duration = Duration::from_millis(debounce_ms.max(10));

        let thread_handle = std::thread::Builder::new()
            .name("repin-watcher".to_string())
            .spawn(move || {
                let mut pending_paths = HashSet::new();

                loop {
                    if closed.load(Ordering::SeqCst) {
                        break;
                    }

                    if pending_paths.is_empty() {
                        crossbeam_channel::select! {
                            recv(shutdown_rx) -> _ => break,
                            recv(event_rx) -> msg => {
                                match msg {
                                    Ok(Ok(event)) => {
                                        collect_event_paths(&root, &event, &mut pending_paths);
                                    }
                                    Ok(Err(_)) => {}
                                    Err(_) => break,
                                }
                            }
                        }
                    } else {
                        let timeout = crossbeam_channel::after(debounce_duration);
                        crossbeam_channel::select! {
                            recv(shutdown_rx) -> _ => break,
                            recv(event_rx) -> msg => {
                                match msg {
                                    Ok(Ok(event)) => {
                                        collect_event_paths(&root, &event, &mut pending_paths);
                                    }
                                    Ok(Err(_)) => {}
                                    Err(_) => break,
                                }
                            }
                            recv(timeout) -> _ => {
                                if closed.load(Ordering::SeqCst) {
                                    break;
                                }
                                for rel_path in pending_paths.drain() {
                                    if closed.load(Ordering::SeqCst) {
                                        break;
                                    }
                                    let full_path = root.join(&rel_path);
                                    if full_path.is_dir() {
                                        continue;
                                    }
                                    if let Some(path_str) = rel_path.to_str() {
                                        match engine.update_file(path_str) {
                                            Ok(Some(summary)) => {
                                                tracing::debug!(
                                                    path = %path_str,
                                                    revision = %summary.revision,
                                                    "watcher updated file"
                                                );
                                            }
                                            Ok(None) => {}
                                            Err(err) => {
                                                tracing::debug!(
                                                    path = %path_str,
                                                    error = %err,
                                                    "watcher failed to update file"
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            })
            .map_err(|err| format!("failed to spawn watcher worker thread: {err}"))?;

        Ok(Self {
            _watcher: watcher,
            shutdown_tx,
            thread_handle: Some(thread_handle),
        })
    }
}

impl Drop for ProjectWatcher {
    fn drop(&mut self) {
        let _ = self.shutdown_tx.send(());
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}

fn collect_event_paths(root: &Path, event: &Event, pending: &mut HashSet<PathBuf>) {
    if event.kind.is_access() {
        return;
    }

    for path in &event.paths {
        let rel_path = match path.strip_prefix(root) {
            Ok(rel) => rel.to_path_buf(),
            Err(_) => {
                if path.is_relative() {
                    path.clone()
                } else {
                    continue;
                }
            }
        };

        if rel_path.as_os_str().is_empty() {
            continue;
        }

        if should_ignore_path(&rel_path) {
            continue;
        }

        pending.insert(rel_path);
    }
}

fn should_ignore_path(rel_path: &Path) -> bool {
    for component in rel_path.components() {
        let name = component.as_os_str().to_string_lossy();
        if name == ".git" || name == ".repin" || name.starts_with(".repin-") {
            return true;
        }
    }
    false
}
