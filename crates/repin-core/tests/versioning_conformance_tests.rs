use repin_core::protocol::ipc::RebuildTarget;
use repin_core::protocol::{
    BOOTSTRAP_VERSION, PROTOCOL_MAX, PROTOCOL_MIN, replacement_allowed, select_protocol,
};
use repin_core::store::{STORE_APPLICATION_ID, STORE_SCHEMA_VERSION, SqliteStore};
use tempfile::tempdir;

#[test]
fn protocol_selection_is_highest_common_and_build_identity_independent() {
    assert_eq!(select_protocol(1, 3, 2, 4), Some(3));
    assert_eq!(select_protocol(1, 1, 2, 2), None);
    assert_eq!(BOOTSTRAP_VERSION, 1);
    const { assert!(PROTOCOL_MIN <= PROTOCOL_MAX) };
    assert!(!replacement_allowed(PROTOCOL_MIN, PROTOCOL_MAX, true));
    assert!(replacement_allowed(PROTOCOL_MAX + 1, PROTOCOL_MAX, true));
    assert!(!replacement_allowed(PROTOCOL_MAX + 1, PROTOCOL_MAX, false));

    for target in [
        RebuildTarget::Graph,
        RebuildTarget::Lexical,
        RebuildTarget::Vector,
        RebuildTarget::All,
    ] {
        let encoded = serde_json::to_string(&target).unwrap();
        assert_eq!(
            serde_json::from_str::<RebuildTarget>(&encoded).unwrap(),
            target
        );
    }
}

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
