use crate::context_handle::ProjectContext;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Default, Clone)]
pub struct ContextRegistry {
    contexts: Arc<Mutex<HashMap<PathBuf, Arc<ProjectContext>>>>,
}

impl ContextRegistry {
    pub fn new() -> Self {
        Self {
            contexts: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn get_or_load<P: AsRef<Path>>(&self, db_path: P) -> Result<Arc<ProjectContext>, String> {
        let canonical = db_path
            .as_ref()
            .canonicalize()
            .unwrap_or_else(|_| db_path.as_ref().to_path_buf());

        let mut lock = self.contexts.lock().unwrap();
        if let Some(ctx) = lock.get(&canonical) {
            return Ok(ctx.clone());
        }

        let ctx = Arc::new(ProjectContext::open(canonical.clone())?);
        lock.insert(canonical, ctx.clone());
        Ok(ctx)
    }

    pub fn unload<P: AsRef<Path>>(&self, db_path: P) -> bool {
        let canonical = db_path
            .as_ref()
            .canonicalize()
            .unwrap_or_else(|_| db_path.as_ref().to_path_buf());

        let mut lock = self.contexts.lock().unwrap();
        lock.remove(&canonical).is_some()
    }

    pub fn active_count(&self) -> usize {
        self.contexts.lock().unwrap().len()
    }
}
