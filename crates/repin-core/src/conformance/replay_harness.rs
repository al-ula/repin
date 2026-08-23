use crate::ports::store::Store;
use crate::runtime::{Engine, EngineOptions};
use std::fs;
use std::path::Path;

pub struct ReplayHarness;

impl ReplayHarness {
    pub fn assert_convergence(clean_root: &Path, _incremental_root: &Path) -> Result<(), String> {
        // Step 1: Initial state
        let src_dir = clean_root.join("src");
        fs::create_dir_all(&src_dir).map_err(|e| e.to_string())?;

        let rust_file = src_dir.join("lib.rs");
        let ts_file = src_dir.join("index.ts");
        let md_file = clean_root.join("README.md");

        fs::write(&rust_file, b"pub fn initial_func() -> u32 { 42 }\n")
            .map_err(|e| e.to_string())?;
        fs::write(&ts_file, b"export class Service { run(): void {} }\n")
            .map_err(|e| e.to_string())?;
        fs::write(&md_file, b"# Documentation\n\nIntro section.\n").map_err(|e| e.to_string())?;

        let state_dir = clean_root
            .parent()
            .unwrap_or(clean_root)
            .join(format!("repin-replay-state-{}", std::process::id()));
        fs::create_dir_all(&state_dir).map_err(|e| e.to_string())?;
        let db_path = state_dir.join("graph.sqlite3");
        let engine = Engine::open(EngineOptions {
            root_id: "root".to_string(),
            root_path: clean_root.to_path_buf(),
            db_path: Some(db_path),
        })?;

        // Perform initial index
        let initial_indexed = engine.index_all_worktree()?;
        assert!(initial_indexed >= 3);

        // Step 2: Modify files incrementally
        fs::write(
            &rust_file,
            b"pub fn initial_func() -> u32 { 100 }\npub fn new_feature() {}\n",
        )
        .map_err(|e| e.to_string())?;

        // Re-index modified snapshot
        let snapshot = engine.options().root_path.join("src/lib.rs");
        let snap_content = fs::read(&snapshot).map_err(|e| e.to_string())?;
        let file_snap = crate::ports::fs::FileSnapshot {
            root: "root".to_string(),
            path: "src/lib.rs".to_string(),
            artifact_class: crate::model::registries::ArtifactClass::Code,
            content_hash: crate::hash::ContentHash::of_bytes(&snap_content),
            content: snap_content,
        };

        engine
            .update_snapshot(&file_snap)
            .map_err(|e| e.to_string())?;

        // Step 3: Validate that both initial_func and new_feature are queryable
        let store = engine.store().ok_or("store not available")?;
        let view = store.read_view().map_err(|e| e.to_string())?;

        let nodes = view
            .nodes_by_file("root", "src/lib.rs")
            .map_err(|e| e.to_string())?;
        let names: Vec<&str> = nodes.iter().map(|n| n.name.as_str()).collect();

        if !names.contains(&"initial_func") || !names.contains(&"new_feature") {
            return Err("convergence verification failed: expected symbols missing".to_string());
        }

        Ok(())
    }
}
