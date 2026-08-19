use repin_conformance::{
    ReplayHarness, test_fs_containment_conformance, test_store_commit_atomicity,
    test_store_rollback_safety,
};
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
