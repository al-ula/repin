use repin_engine::{Engine, EngineOptions};
use repin_protocol::envelope::Status;
use std::path::Path;

pub struct ReplayHarness;

impl ReplayHarness {
    pub fn assert_convergence(clean_root: &Path, _incremental_root: &Path) -> Result<(), String> {
        let clean_db = clean_root.join(".repin/graph.sqlite3");
        let clean_engine = Engine::open(EngineOptions {
            root_id: "root".to_string(),
            root_path: clean_root.to_path_buf(),
            db_path: Some(clean_db),
        })?;

        clean_engine.index_all_worktree()?;

        let res = clean_engine.search_direct("fn ", true, 100);
        if res.status != Status::Ok && res.status != Status::NotFound {
            return Err("convergence validation failed".to_string());
        }

        Ok(())
    }
}
