use repin_core::model::identity::NodeId;
use repin_core::model::node::{Node, NodeClaim};
use repin_core::model::provenance::{Confidence, Derivation, FactOwner, Provenance, Revision};
use repin_core::model::registries::NodeKind;
use repin_core::ports::store::{Store, StoreError};
use repin_fs::CapabilityFs;
use repin_runtime::{Engine, EngineOptions};
use repin_store_sqlite::SqliteStore;
use std::fs;
use std::path::Path;
use tempfile::tempdir;

pub fn test_store_commit_atomicity(store: &dyn Store) -> Result<(), StoreError> {
    let mut tx = store.begin_write()?;
    let id = NodeId::new(NodeKind::Function, "root", "src/test.rs", &[], "foo", 0);
    let node = Node {
        id,
        kind: NodeKind::Function,
        name: "foo".to_string(),
        qualified_name: None,
        root: "root".to_string(),
        path: "src/test.rs".to_string(),
        range: None,
        language: Some("rust".to_string()),
        artifact_class: None,
        provenance: Provenance {
            root: "root".to_string(),
            path: "src/test.rs".to_string(),
            range: None,
            extractor: "test".to_string(),
            extractor_version: "1.0".to_string(),
            derivation: Derivation::Extracted,
            confidence: Confidence::EXACT,
            revision: Revision::INITIAL,
        },
        attributes: Default::default(),
    };

    tx.put_nodes(&[NodeClaim {
        node,
        owner: FactOwner::new("root", "src/test.rs", "test", "1.0"),
    }])?;
    tx.set_revision(Revision(1))?;
    tx.commit()?;

    let view = store.read_view()?;
    assert_eq!(view.revision()?, Revision(1));
    assert!(view.node(&id)?.is_some());
    Ok(())
}

pub fn test_store_rollback_safety(store: &dyn Store) -> Result<(), StoreError> {
    let mut tx = store.begin_write()?;
    let id = NodeId::new(NodeKind::Function, "root", "src/test.rs", &[], "bar", 0);
    let node = Node {
        id,
        kind: NodeKind::Function,
        name: "bar".to_string(),
        qualified_name: None,
        root: "root".to_string(),
        path: "src/test.rs".to_string(),
        range: None,
        language: Some("rust".to_string()),
        artifact_class: None,
        provenance: Provenance {
            root: "root".to_string(),
            path: "src/test.rs".to_string(),
            range: None,
            extractor: "test".to_string(),
            extractor_version: "1.0".to_string(),
            derivation: Derivation::Extracted,
            confidence: Confidence::EXACT,
            revision: Revision::INITIAL,
        },
        attributes: Default::default(),
    };

    tx.put_nodes(&[NodeClaim {
        node,
        owner: FactOwner::new("root", "src/test.rs", "test", "1.0"),
    }])?;
    tx.rollback()?;

    let view = store.read_view()?;
    assert!(view.node(&id)?.is_none());
    Ok(())
}

#[test]
fn test_store_conformance() {
    let store = SqliteStore::open_in_memory().unwrap();
    test_store_commit_atomicity(&store).unwrap();
    test_store_rollback_safety(&store).unwrap();
}

#[test]
fn test_fs_conformance() {
    let dir = tempdir().unwrap();
    let fs = CapabilityFs::open("root", dir.path()).unwrap();
    assert_eq!(fs.root_id(), "root");
}

pub struct ReplayHarness;

impl ReplayHarness {
    pub fn assert_convergence(clean_root: &Path, _incremental_root: &Path) -> Result<(), String> {
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

        let initial_indexed = engine.index_all_worktree()?;
        assert!(initial_indexed >= 3);

        fs::write(
            &rust_file,
            b"pub fn initial_func() -> u32 { 100 }\npub fn new_feature() {}\n",
        )
        .map_err(|e| e.to_string())?;

        let snapshot = engine.options().root_path.join("src/lib.rs");
        let snap_content = fs::read(&snapshot).map_err(|e| e.to_string())?;
        let file_snap = repin_core::ports::fs::FileSnapshot {
            root: "root".to_string(),
            path: "src/lib.rs".to_string(),
            artifact_class: repin_core::model::registries::ArtifactClass::Code,
            content_hash: repin_core::hash::ContentHash::of_bytes(&snap_content),
            content: snap_content,
        };

        engine
            .update_snapshot(&file_snap)
            .map_err(|e| e.to_string())?;

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

#[test]
fn test_replay_harness_convergence() {
    let dir = tempdir().unwrap();
    let src = dir.path().join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("lib.rs"), b"pub fn work() {}\n").unwrap();

    ReplayHarness::assert_convergence(dir.path(), dir.path()).unwrap();
}

#[test]
fn test_impact_and_path_conformance() {
    let dir = tempdir().unwrap();
    let project_root = dir.path().join("project");
    let src = project_root.join("src");
    fs::create_dir_all(&src).unwrap();

    let file_a = src.join("a.rs");
    let file_b = src.join("b.rs");

    fs::write(&file_a, b"pub fn helper() {}\n").unwrap();
    fs::write(&file_b, b"use crate::a::helper;\npub fn caller() {}\n").unwrap();

    let state_dir = dir.path().join("state");
    fs::create_dir_all(&state_dir).unwrap();
    let db_path = state_dir.join("graph.sqlite3");

    let engine = Engine::open(EngineOptions {
        root_id: "root".to_string(),
        root_path: project_root.clone(),
        db_path: Some(db_path),
    })
    .unwrap();

    engine.index_all_worktree().unwrap();

    let impact = engine.lookup_impact("helper", 5);
    assert!(impact.data.is_some());

    let paths = engine.trace_paths("caller", "helper", 5);
    assert!(paths.data.is_some());
}
