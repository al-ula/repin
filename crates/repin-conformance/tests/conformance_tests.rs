use repin_conformance::{
    ReplayHarness, test_fs_containment_conformance, test_store_commit_atomicity,
    test_store_rollback_safety,
};
use repin_engine::{Engine, EngineOptions};
use repin_store_sqlite::SqliteStore;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_store_conformance() {
    let store = SqliteStore::open_in_memory().unwrap();
    test_store_commit_atomicity(&store).unwrap();
    test_store_rollback_safety(&store).unwrap();
}

#[test]
fn test_fs_conformance() {
    let dir = tempdir().unwrap();
    test_fs_containment_conformance(dir.path()).unwrap();
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
    let src = dir.path().join("src");
    fs::create_dir_all(&src).unwrap();

    fs::write(src.join("a.rs"), b"pub fn helper_a() {}\n").unwrap();
    fs::write(
        src.join("b.rs"),
        b"mod a;\npub fn middle_b() { a::helper_a(); }\n",
    )
    .unwrap();
    fs::write(
        src.join("c.rs"),
        b"mod b;\npub fn top_c() { b::middle_b(); }\n",
    )
    .unwrap();

    let db_path = dir.path().join(".repin/graph.sqlite3");
    let engine = Engine::open(EngineOptions {
        root_id: "root".to_string(),
        root_path: dir.path().to_path_buf(),
        db_path: Some(db_path),
    })
    .unwrap();

    engine.index_all_worktree().unwrap();

    // 1. Impact analysis on leaf helper_a
    let impact_env = engine.lookup_impact("helper_a", 3);
    assert_eq!(impact_env.status, repin_protocol::envelope::Status::Ok);
    let impact_data = impact_env.data.expect("impact data should be present");
    assert_eq!(impact_data.target.name, "helper_a");
    assert!(impact_data.total_impacted >= 1);

    // 2. Shortest path trace from top_c to helper_a
    let path_env = engine.trace_paths("src/c.rs", "helper_a", 5);
    assert_eq!(path_env.status, repin_protocol::envelope::Status::Ok);

    // 3. Disconnected path query
    let disconnected_env = engine.trace_paths("helper_a", "nonexistent_symbol", 5);
    assert_eq!(
        disconnected_env.status,
        repin_protocol::envelope::Status::Ok
    );
    assert!(disconnected_env.data.is_none());
}
