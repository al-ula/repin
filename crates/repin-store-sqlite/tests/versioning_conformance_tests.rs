use repin_store_sqlite::{STORE_APPLICATION_ID, STORE_SCHEMA_VERSION, SqliteStore};
use tempfile::tempdir;

#[test]
fn sqlite_creation_and_inspection_are_versioned_before_activation() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("graph.sqlite3");
    let before = SqliteStore::inspect(&path).unwrap();
    assert_eq!(before.application_id, 0);
    assert_eq!(before.schema_version, 0);
    assert!(!before.has_user_tables);

    let _store = SqliteStore::open(&path).unwrap();
    let after = SqliteStore::inspect(&path).unwrap();
    assert_eq!(after.application_id, STORE_APPLICATION_ID);
    assert_eq!(after.schema_version, STORE_SCHEMA_VERSION);
    assert!(after.has_user_tables);
}
