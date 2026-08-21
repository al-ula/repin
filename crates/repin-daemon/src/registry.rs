use crate::context_handle::{ProjectContext, WriterLease};
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
            // A cached context is reusable only while its database still has
            // the physical identity recorded at open time (ADR-026). A removed
            // or replaced database fails that context closed and a fresh
            // activation cycle runs against the current file.
            if ctx.is_usable() {
                self.idle_since.lock().unwrap().remove(&canonical);
                return Ok(ctx.clone());
            }
            ctx.mark_closed();
            lock.remove(&canonical);
            self.idle_since.lock().unwrap().remove(&canonical);
        }

        let ctx = Arc::new(ProjectContext::open(canonical.clone())?);
        lock.insert(canonical, ctx.clone());
        Ok(ctx)
    }

    /// Publish a context for a project just created under `lease`, reusing an
    /// equivalent live context if one already exists. The caller's lease is
    /// carried into the new context so writer ownership is continuous from
    /// creation through activation (ADR-026).
    pub fn publish_created<P: AsRef<Path>>(
        &self,
        db_path: P,
        lease: WriterLease,
    ) -> Result<Arc<ProjectContext>, String> {
        let canonical = db_path
            .as_ref()
            .canonicalize()
            .unwrap_or_else(|_| db_path.as_ref().to_path_buf());

        let mut lock = self.contexts.lock().unwrap();
        if let Some(ctx) = lock.get(&canonical) {
            if ctx.is_usable() {
                // Another connection already activated this project; its
                // context owns the authoritative handle. Release ours rather
                // than holding a second lease for the same database.
                drop(lease);
                self.idle_since.lock().unwrap().remove(&canonical);
                return Ok(ctx.clone());
            }
            ctx.mark_closed();
            lock.remove(&canonical);
            self.idle_since.lock().unwrap().remove(&canonical);
        }

        let ctx = Arc::new(ProjectContext::open_with_lease(canonical.clone(), lease)?);
        lock.insert(canonical, ctx.clone());
        Ok(ctx)
    }

    /// Number of connections attached to the context for this path, excluding
    /// the registry's own reference. Zero means the context is detached and
    /// may be unloaded without affecting a live client.
    pub fn attached_count<P: AsRef<Path>>(&self, db_path: P) -> usize {
        let canonical = db_path
            .as_ref()
            .canonicalize()
            .unwrap_or_else(|_| db_path.as_ref().to_path_buf());
        let contexts = self.contexts.lock().unwrap();
        contexts
            .get(&canonical)
            .map(|ctx| Arc::strong_count(ctx).saturating_sub(1))
            .unwrap_or(0)
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
        match lock.remove(&canonical) {
            Some(ctx) => {
                ctx.mark_closed();
                true
            }
            None => false,
        }
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

    #[test]
    fn replaced_database_is_not_served_from_the_cached_context() {
        let dir = tempdir().unwrap();
        let repin_dir = dir.path().join(".repin");
        fs::create_dir_all(&repin_dir).unwrap();
        let db = repin_dir.join("graph.sqlite3");
        let registry = ContextRegistry::new();

        let first = registry.get_or_load(&db).unwrap();
        let first_identity = first.identity();
        assert!(first_identity.is_some());
        assert!(first.is_usable());

        // Simulate `uninit` followed by `init`: the canonical path is the same
        // but the inode is not.
        fs::remove_dir_all(&repin_dir).unwrap();
        assert!(!first.is_usable());
        assert!(first.is_closed());

        fs::create_dir_all(&repin_dir).unwrap();
        let second = registry.get_or_load(&db).unwrap();
        assert_ne!(second.identity(), first_identity);
        assert!(second.is_usable());
        assert_eq!(registry.active_count(), 1);
    }

    #[test]
    fn unload_closes_the_context_and_reports_attachment() {
        let dir = tempdir().unwrap();
        let repin_dir = dir.path().join(".repin");
        fs::create_dir_all(&repin_dir).unwrap();
        let db = repin_dir.join("graph.sqlite3");
        let registry = ContextRegistry::new();

        let attached = registry.get_or_load(&db).unwrap();
        assert_eq!(registry.attached_count(&db), 1);
        drop(attached);
        assert_eq!(registry.attached_count(&db), 0);

        assert!(registry.unload(&db));
        assert_eq!(registry.active_count(), 0);
        assert!(!registry.unload(&db));
    }
}
