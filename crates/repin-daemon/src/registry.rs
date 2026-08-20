use crate::context_handle::ProjectContext;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Default, Clone)]
pub struct ContextRegistry {
    contexts: Arc<Mutex<HashMap<PathBuf, Arc<ProjectContext>>>>,
    idle_since: Arc<Mutex<HashMap<PathBuf, Instant>>>,
}

impl ContextRegistry {
    pub fn new() -> Self {
        Self {
            contexts: Arc::new(Mutex::new(HashMap::new())),
            idle_since: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn get_or_load<P: AsRef<Path>>(&self, db_path: P) -> Result<Arc<ProjectContext>, String> {
        let canonical = db_path
            .as_ref()
            .canonicalize()
            .unwrap_or_else(|_| db_path.as_ref().to_path_buf());

        let mut lock = self.contexts.lock().unwrap();
        if let Some(ctx) = lock.get(&canonical) {
            self.idle_since.lock().unwrap().remove(&canonical);
            return Ok(ctx.clone());
        }

        let ctx = Arc::new(ProjectContext::open(canonical.clone())?);
        lock.insert(canonical, ctx.clone());
        Ok(ctx)
    }

    pub fn mark_detached<P: AsRef<Path>>(&self, db_path: P) {
        let canonical = db_path
            .as_ref()
            .canonicalize()
            .unwrap_or_else(|_| db_path.as_ref().to_path_buf());
        let contexts = self.contexts.lock().unwrap();
        if let Some(ctx) = contexts.get(&canonical)
            && Arc::strong_count(ctx) == 1
        {
            self.idle_since
                .lock()
                .unwrap()
                .entry(canonical)
                .or_insert_with(Instant::now);
        }
    }

    pub fn reap_idle(&self) {
        const IDLE_TIMEOUT: Duration = Duration::from_millis(600_000);
        self.reap_idle_after(IDLE_TIMEOUT);
    }

    fn reap_idle_after(&self, idle_timeout: Duration) {
        let now = Instant::now();
        let candidates: Vec<PathBuf> = self
            .idle_since
            .lock()
            .unwrap()
            .iter()
            .filter_map(|(path, since)| {
                (now.duration_since(*since) >= idle_timeout).then_some(path.clone())
            })
            .collect();
        let mut contexts = self.contexts.lock().unwrap();
        let mut idle_since = self.idle_since.lock().unwrap();
        for path in candidates {
            if let Some(ctx) = contexts.get(&path)
                && Arc::strong_count(ctx) == 1
            {
                contexts.remove(&path);
                idle_since.remove(&path);
            }
        }
    }

    pub fn unload<P: AsRef<Path>>(&self, db_path: P) -> bool {
        let canonical = db_path
            .as_ref()
            .canonicalize()
            .unwrap_or_else(|_| db_path.as_ref().to_path_buf());

        let mut lock = self.contexts.lock().unwrap();
        self.idle_since.lock().unwrap().remove(&canonical);
        lock.remove(&canonical).is_some()
    }

    pub fn active_count(&self) -> usize {
        self.contexts.lock().unwrap().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn detached_context_is_reaped_only_after_idle_timeout() {
        let dir = tempdir().unwrap();
        let repin_dir = dir.path().join(".repin");
        fs::create_dir_all(&repin_dir).unwrap();
        let db = repin_dir.join("graph.sqlite3");
        let registry = ContextRegistry::new();
        let context = registry.get_or_load(&db).unwrap();
        assert_eq!(registry.active_count(), 1);
        drop(context);
        registry.mark_detached(&db);
        registry.reap_idle_after(Duration::from_secs(60));
        assert_eq!(registry.active_count(), 1);
        registry.reap_idle_after(Duration::ZERO);
        assert_eq!(registry.active_count(), 0);
    }
}
