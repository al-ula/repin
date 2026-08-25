use crate::daemon::context_handle::{ProjectContext, WriterLease};
use repin_core::config::RepinConfig;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

struct RegistryEntry {
    context: Arc<ProjectContext>,
    last_detached: Option<Instant>,
    idle_timeout: Option<Duration>,
}

#[derive(Clone)]
pub struct ContextRegistry {
    entries: Arc<Mutex<HashMap<PathBuf, RegistryEntry>>>,
    override_idle_timeout: Arc<Mutex<Option<Option<Duration>>>>,
    has_ever_activated: Arc<AtomicBool>,
}

impl Default for ContextRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ContextRegistry {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(Mutex::new(HashMap::new())),
            override_idle_timeout: Arc::new(Mutex::new(None)),
            has_ever_activated: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Override the idle reap threshold (ADR-027: `repin daemon run --idle-timeout`).
    /// If `override_timeout` is `None`, per-context config is used.
    /// If `override_timeout` is `Some(None)`, idle eviction is disabled for all contexts (`--idle-timeout 0`).
    /// If `override_timeout` is `Some(Some(d))`, `d` is used as the idle timeout for all contexts.
    pub fn set_override_idle_timeout(&self, override_timeout: Option<Option<Duration>>) {
        *self.override_idle_timeout.lock().unwrap() = override_timeout;
        if let Some(override_val) = override_timeout {
            let mut entries = self.entries.lock().unwrap();
            for entry in entries.values_mut() {
                entry.idle_timeout = override_val;
            }
        }
    }

    pub fn set_idle_timeout(&mut self, timeout: Duration) {
        self.set_override_idle_timeout(Some(Some(timeout)));
    }

    fn compute_effective_idle_timeout(
        override_timeout: Option<Option<Duration>>,
        config: &RepinConfig,
    ) -> Option<Duration> {
        match override_timeout {
            Some(override_val) => override_val,
            None => {
                if config.daemon.idle_timeout_secs == 0 {
                    None
                } else {
                    Some(Duration::from_secs(config.daemon.idle_timeout_secs))
                }
            }
        }
    }

    pub fn get_or_load<P: AsRef<Path>>(&self, db_path: P) -> Result<Arc<ProjectContext>, String> {
        self.get_or_load_with_config(db_path, RepinConfig::default())
    }

    pub fn get_or_load_with_config<P: AsRef<Path>>(
        &self,
        db_path: P,
        config: RepinConfig,
    ) -> Result<Arc<ProjectContext>, String> {
        let canonical = db_path
            .as_ref()
            .canonicalize()
            .unwrap_or_else(|_| db_path.as_ref().to_path_buf());

        let override_timeout = *self.override_idle_timeout.lock().unwrap();
        let effective_idle_timeout =
            Self::compute_effective_idle_timeout(override_timeout, &config);

        let mut lock = self.entries.lock().unwrap();
        if let Some(entry) = lock.get_mut(&canonical) {
            // A cached context is reusable only while its database still has
            // the physical identity recorded at open time (ADR-026). A removed
            // or replaced database fails that context closed and a fresh
            // activation cycle runs against the current file.
            if entry.context.is_usable() && entry.context.config() == &config {
                entry.last_detached = None;
                entry.idle_timeout = effective_idle_timeout;
                self.has_ever_activated.store(true, Ordering::SeqCst);
                return Ok(entry.context.clone());
            }
            if entry.context.is_usable() && Arc::strong_count(&entry.context) > 1 {
                return Err(
                    "project is attached with a different resolved configuration; close clients before reconnecting with another configuration".to_string(),
                );
            }
            entry.context.mark_closed();
            lock.remove(&canonical);
        }

        let ctx = Arc::new(ProjectContext::open_with_config(canonical.clone(), config)?);
        let entry = RegistryEntry {
            context: ctx.clone(),
            last_detached: None,
            idle_timeout: effective_idle_timeout,
        };
        lock.insert(canonical, entry);
        self.has_ever_activated.store(true, Ordering::SeqCst);
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
        self.publish_created_with_config(db_path, lease, RepinConfig::default())
    }

    pub fn publish_created_with_config<P: AsRef<Path>>(
        &self,
        db_path: P,
        lease: WriterLease,
        config: RepinConfig,
    ) -> Result<Arc<ProjectContext>, String> {
        let canonical = db_path
            .as_ref()
            .canonicalize()
            .unwrap_or_else(|_| db_path.as_ref().to_path_buf());

        let override_timeout = *self.override_idle_timeout.lock().unwrap();
        let effective_idle_timeout =
            Self::compute_effective_idle_timeout(override_timeout, &config);

        let mut lock = self.entries.lock().unwrap();
        if let Some(entry) = lock.get_mut(&canonical) {
            if entry.context.is_usable() && entry.context.config() == &config {
                // Another connection already activated this project; its
                // context owns the authoritative handle. Release ours rather
                // than holding a second lease for the same database.
                drop(lease);
                entry.last_detached = None;
                entry.idle_timeout = effective_idle_timeout;
                self.has_ever_activated.store(true, Ordering::SeqCst);
                return Ok(entry.context.clone());
            }
            if entry.context.is_usable() {
                drop(lease);
                return Err(
                    "project is already active with a different resolved configuration".to_string(),
                );
            }
            entry.context.mark_closed();
            lock.remove(&canonical);
        }

        let ctx = Arc::new(ProjectContext::open_with_lease_and_config(
            canonical.clone(),
            lease,
            config,
        )?);
        let entry = RegistryEntry {
            context: ctx.clone(),
            last_detached: None,
            idle_timeout: effective_idle_timeout,
        };
        lock.insert(canonical, entry);
        self.has_ever_activated.store(true, Ordering::SeqCst);
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
        let entries = self.entries.lock().unwrap();
        entries
            .get(&canonical)
            .map(|entry| Arc::strong_count(&entry.context).saturating_sub(1))
            .unwrap_or(0)
    }

    pub fn mark_detached<P: AsRef<Path>>(&self, db_path: P) {
        let canonical = db_path
            .as_ref()
            .canonicalize()
            .unwrap_or_else(|_| db_path.as_ref().to_path_buf());
        let mut entries = self.entries.lock().unwrap();
        if let Some(entry) = entries.get_mut(&canonical)
            && Arc::strong_count(&entry.context) == 1
            && entry.last_detached.is_none()
        {
            entry.last_detached = Some(Instant::now());
        }
    }

    pub fn reap_idle(&self) {
        let now = Instant::now();
        let mut entries = self.entries.lock().unwrap();
        for entry in entries.values_mut() {
            if Arc::strong_count(&entry.context) == 1 && entry.last_detached.is_none() {
                entry.last_detached = Some(now);
            }
        }
        let candidates: Vec<PathBuf> = entries
            .iter()
            .filter_map(|(path, entry)| {
                if Arc::strong_count(&entry.context) == 1
                    && let Some(since) = entry.last_detached
                    && let Some(timeout) = entry.idle_timeout
                    && now.duration_since(since) >= timeout
                {
                    return Some(path.clone());
                }
                None
            })
            .collect();

        for path in candidates {
            if let Some(entry) = entries.remove(&path) {
                entry.context.mark_closed();
            }
        }
    }

    pub fn reap_idle_after(&self, idle_timeout: Duration) {
        let now = Instant::now();
        let mut entries = self.entries.lock().unwrap();
        let candidates: Vec<PathBuf> = entries
            .iter()
            .filter_map(|(path, entry)| {
                if Arc::strong_count(&entry.context) == 1 {
                    if let Some(since) = entry.last_detached {
                        if now.duration_since(since) >= idle_timeout {
                            return Some(path.clone());
                        }
                    } else if idle_timeout == Duration::ZERO {
                        return Some(path.clone());
                    }
                }
                None
            })
            .collect();

        for path in candidates {
            if let Some(entry) = entries.remove(&path) {
                entry.context.mark_closed();
            }
        }
    }

    pub fn unload<P: AsRef<Path>>(&self, db_path: P) -> bool {
        let canonical = db_path
            .as_ref()
            .canonicalize()
            .unwrap_or_else(|_| db_path.as_ref().to_path_buf());

        let mut lock = self.entries.lock().unwrap();
        match lock.remove(&canonical) {
            Some(entry) => {
                entry.context.mark_closed();
                true
            }
            None => false,
        }
    }

    pub fn active_count(&self) -> usize {
        self.entries.lock().unwrap().len()
    }

    pub fn has_active_contexts(&self) -> bool {
        !self.entries.lock().unwrap().is_empty()
    }

    pub fn has_ever_activated(&self) -> bool {
        self.has_ever_activated.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::product::ProjectLayout;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn detached_context_is_reaped_only_after_idle_timeout() {
        let dir = tempdir().unwrap();
        let layout = ProjectLayout::at_root(dir.path());
        fs::create_dir_all(&layout.state_dir).unwrap();
        let db = layout.db_path;
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
        let layout = ProjectLayout::at_root(dir.path());
        fs::create_dir_all(&layout.state_dir).unwrap();
        let db = layout.db_path;
        let registry = ContextRegistry::new();

        let first = registry.get_or_load(&db).unwrap();
        let first_identity = first.identity();
        assert!(first_identity.is_some());
        assert!(first.is_usable());

        // Simulate `uninit` followed by `init`: the canonical path is the same
        // but the inode is not.
        fs::remove_dir_all(&layout.state_dir).unwrap();
        assert!(!first.is_usable());
        assert!(first.is_closed());

        fs::create_dir_all(&layout.state_dir).unwrap();
        let second = registry.get_or_load(&db).unwrap();
        assert_ne!(second.identity(), first_identity);
        assert!(second.is_usable());
        assert_eq!(registry.active_count(), 1);
    }

    #[test]
    fn unload_closes_the_context_and_reports_attachment() {
        let dir = tempdir().unwrap();
        let layout = ProjectLayout::at_root(dir.path());
        fs::create_dir_all(&layout.state_dir).unwrap();
        let db = layout.db_path;
        let registry = ContextRegistry::new();

        let attached = registry.get_or_load(&db).unwrap();
        assert_eq!(registry.attached_count(&db), 1);
        drop(attached);
        assert_eq!(registry.attached_count(&db), 0);

        assert!(registry.unload(&db));
        assert_eq!(registry.active_count(), 0);
        assert!(!registry.unload(&db));
    }

    #[test]
    fn resolved_configuration_is_injected_and_conflicts_are_scoped_to_attachments() {
        let dir = tempdir().unwrap();
        let layout = ProjectLayout::at_root(dir.path());
        fs::create_dir_all(&layout.state_dir).unwrap();
        let db = layout.db_path;
        let registry = ContextRegistry::new();

        let mut config = RepinConfig::default();
        config
            .indexing
            .exclude_paths
            .push("generated/**".to_string());
        let context = registry
            .get_or_load_with_config(&db, config.clone())
            .unwrap();
        assert_eq!(context.config(), &config);

        let conflicting = RepinConfig::default();
        assert!(
            registry
                .get_or_load_with_config(&db, conflicting.clone())
                .is_err()
        );

        drop(context);
        registry.mark_detached(&db);
        let replacement = registry
            .get_or_load_with_config(&db, conflicting.clone())
            .unwrap();
        assert_eq!(replacement.config(), &conflicting);
    }

    #[test]
    fn test_detached_context_idle_timeout_seconds() {
        let dir = tempdir().unwrap();
        let layout = ProjectLayout::at_root(dir.path());
        fs::create_dir_all(&layout.state_dir).unwrap();
        let db = layout.db_path;
        let registry = ContextRegistry::new();

        let mut config = RepinConfig::default();
        config.daemon.idle_timeout_secs = 1;

        let context = registry.get_or_load_with_config(&db, config).unwrap();
        assert_eq!(registry.active_count(), 1);
        drop(context);
        registry.mark_detached(&db);

        registry.reap_idle();
        assert_eq!(registry.active_count(), 1);

        std::thread::sleep(Duration::from_millis(1100));
        registry.reap_idle();
        assert_eq!(registry.active_count(), 0);
    }

    #[test]
    fn test_zero_idle_timeout_is_persistent() {
        let dir = tempdir().unwrap();
        let layout = ProjectLayout::at_root(dir.path());
        fs::create_dir_all(&layout.state_dir).unwrap();
        let db = layout.db_path;
        let registry = ContextRegistry::new();

        let mut config = RepinConfig::default();
        config.daemon.idle_timeout_secs = 0;

        let context = registry.get_or_load_with_config(&db, config).unwrap();
        assert_eq!(registry.active_count(), 1);
        drop(context);
        registry.mark_detached(&db);

        registry.reap_idle();
        assert_eq!(registry.active_count(), 1);

        std::thread::sleep(Duration::from_millis(100));
        registry.reap_idle();
        assert_eq!(registry.active_count(), 1);
    }

    #[test]
    fn test_cli_override_idle_timeout_precedence() {
        let dir = tempdir().unwrap();
        let layout = ProjectLayout::at_root(dir.path());
        fs::create_dir_all(&layout.state_dir).unwrap();
        let db = layout.db_path;
        let registry = ContextRegistry::new();

        registry.set_override_idle_timeout(Some(Some(Duration::from_millis(50))));

        let mut config = RepinConfig::default();
        config.daemon.idle_timeout_secs = 600;

        let context = registry.get_or_load_with_config(&db, config).unwrap();
        drop(context);
        registry.mark_detached(&db);

        std::thread::sleep(Duration::from_millis(70));
        registry.reap_idle();
        assert_eq!(registry.active_count(), 0);

        // Also test CLI override 0 (disable) overriding config timeout
        let context2 = registry.get_or_load(&db).unwrap();
        registry.set_override_idle_timeout(Some(None));
        drop(context2);
        registry.mark_detached(&db);
        std::thread::sleep(Duration::from_millis(50));
        registry.reap_idle();
        assert_eq!(registry.active_count(), 1);
    }

    #[test]
    fn test_reattachment_cancels_idle_timer() {
        let dir = tempdir().unwrap();
        let layout = ProjectLayout::at_root(dir.path());
        fs::create_dir_all(&layout.state_dir).unwrap();
        let db = layout.db_path;
        let registry = ContextRegistry::new();
        registry.set_override_idle_timeout(Some(Some(Duration::from_millis(100))));

        let context = registry.get_or_load(&db).unwrap();
        drop(context);
        registry.mark_detached(&db);

        std::thread::sleep(Duration::from_millis(60));
        // Reattach
        let context = registry.get_or_load(&db).unwrap();
        drop(context);
        registry.mark_detached(&db);

        // Sleep 60ms more: 120ms since first detach, but only 60ms since second
        std::thread::sleep(Duration::from_millis(60));
        registry.reap_idle();
        assert_eq!(registry.active_count(), 1);

        // Sleep 50ms more: 110ms since second detach -> now reaped
        std::thread::sleep(Duration::from_millis(50));
        registry.reap_idle();
        assert_eq!(registry.active_count(), 0);
    }

    #[test]
    fn test_attached_clients_never_reaped() {
        let dir = tempdir().unwrap();
        let layout = ProjectLayout::at_root(dir.path());
        fs::create_dir_all(&layout.state_dir).unwrap();
        let db = layout.db_path;
        let registry = ContextRegistry::new();
        registry.set_override_idle_timeout(Some(Some(Duration::from_millis(50))));

        let context = registry.get_or_load(&db).unwrap();
        std::thread::sleep(Duration::from_millis(70));
        registry.reap_idle();
        assert_eq!(registry.active_count(), 1);
        assert_eq!(registry.attached_count(&db), 1);
        drop(context);
    }

    #[test]
    fn test_has_ever_activated_tracks_lifecycle() {
        let dir = tempdir().unwrap();
        let layout = ProjectLayout::at_root(dir.path());
        fs::create_dir_all(&layout.state_dir).unwrap();
        let db = layout.db_path;
        let registry = ContextRegistry::new();

        assert!(!registry.has_ever_activated());
        assert!(!registry.has_active_contexts());

        let context = registry.get_or_load(&db).unwrap();
        assert!(registry.has_ever_activated());
        assert!(registry.has_active_contexts());

        drop(context);
        registry.unload(&db);
        assert!(registry.has_ever_activated());
        assert!(!registry.has_active_contexts());
    }
}
