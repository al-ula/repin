use repin_core::protocol::errors::ErrorCode;
use repin_core::protocol::ipc::{IpcRequest, IpcResponse};
use repin_core::protocol::{PROTOCOL_MAX, PROTOCOL_MIN, PROTOCOL_STATE_LIFECYCLE, select_protocol};
use repin_daemon::{ContextRegistry, initialize_state, uninitialize_state};
use std::fs;
use tempfile::tempdir;

/// docs/runtime.md §4: state creation and removal are daemon-mediated, and the
/// protocol carrying them is negotiated, not assumed.
#[test]
fn state_lifecycle_requests_are_carried_by_the_negotiated_protocol() {
    const { assert!(PROTOCOL_STATE_LIFECYCLE <= PROTOCOL_MAX) };
    const { assert!(PROTOCOL_MIN < PROTOCOL_STATE_LIFECYCLE) };
    // An old daemon still overlaps at protocol 1, which excludes lifecycle.
    assert_eq!(select_protocol(PROTOCOL_MIN, PROTOCOL_MAX, 1, 1), Some(1));
    assert_eq!(
        select_protocol(PROTOCOL_MIN, PROTOCOL_MAX, PROTOCOL_MIN, PROTOCOL_MAX),
        Some(PROTOCOL_MAX)
    );

    for request in [
        IpcRequest::InitializeProject {
            project_root: "/tmp/project".to_string(),
            resolved_config: None,
        },
        IpcRequest::UninitializeProject {
            project_root: "/tmp/project".to_string(),
        },
    ] {
        let encoded = serde_json::to_string(&request).unwrap();
        assert_eq!(
            serde_json::from_str::<IpcRequest>(&encoded).unwrap(),
            request
        );
    }

    for response in [
        IpcResponse::InitializeProjectOk {
            project_root: "/tmp/project".to_string(),
            db_path: "/tmp/project/.repin/graph.sqlite3".to_string(),
            created: true,
            is_writer: true,
        },
        IpcResponse::UninitializeProjectOk {
            project_root: "/tmp/project".to_string(),
            removed: true,
        },
    ] {
        let encoded = serde_json::to_string(&response).unwrap();
        assert_eq!(
            serde_json::from_str::<IpcResponse>(&encoded).unwrap(),
            response
        );
    }
}

/// docs/runtime.md §4: a `.repin/graph.sqlite3` entry that is not a regular
/// file can never be activated, so init fails with PROJECT_STATE_INVALID
/// instead of reporting created success.
#[test]
fn initialization_fails_when_the_graph_database_is_a_directory() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join(".repin").join("graph.sqlite3");
    fs::create_dir_all(&db_path).unwrap();

    let error = initialize_state(dir.path()).unwrap_err();

    assert_eq!(error.0, ErrorCode::ProjectStateInvalid);
    assert!(error.1.contains("not a regular file"));
    assert!(db_path.is_dir(), "the unusable entry must be preserved");
}

/// docs/runtime.md §4: initialization creates private state and never
/// overwrites an existing database.
#[test]
fn initialization_is_private_and_never_overwrites_state() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempdir().unwrap();
    let first = initialize_state(dir.path()).unwrap();
    assert!(first.created);
    assert_eq!(
        fs::metadata(&first.layout.state_dir)
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );

    fs::write(&first.layout.db_path, b"authoritative").unwrap();
    let second = initialize_state(dir.path()).unwrap();
    assert!(!second.created);
    assert_eq!(fs::read(&second.layout.db_path).unwrap(), b"authoritative");
}

/// docs/runtime.md §4 and §9(10): removal unloads the context and releases the
/// writer lease before the state directory disappears, and is refused while a
/// client is attached.
#[test]
fn removal_unloads_the_context_and_is_refused_while_attached() {
    let dir = tempdir().unwrap();
    let registry = ContextRegistry::new();
    let state = initialize_state(dir.path()).unwrap();

    let attached = registry.get_or_load(&state.layout.db_path).unwrap();
    let refusal = uninitialize_state(&registry, dir.path()).unwrap_err();
    assert_eq!(refusal.0, ErrorCode::ProjectLeaseUnavailable);
    assert!(state.layout.state_dir.is_dir());

    drop(attached);
    let removed = uninitialize_state(&registry, dir.path()).unwrap();
    assert!(removed.removed);
    assert!(!state.layout.state_dir.exists());
    assert_eq!(registry.active_count(), 0);

    // Idempotent outcome: removing absent state succeeds and reports nothing.
    assert!(!uninitialize_state(&registry, dir.path()).unwrap().removed);
}

/// docs/runtime.md §3 and §9(11): a database that changes physical identity
/// fails its context closed, so a re-initialized project at the same canonical
/// path never serves the previous graph.
#[test]
fn replaced_state_fails_its_context_closed_instead_of_serving_stale_graph() {
    let dir = tempdir().unwrap();
    let registry = ContextRegistry::new();
    let state = initialize_state(dir.path()).unwrap();

    let context = registry.get_or_load(&state.layout.db_path).unwrap();
    let stale_identity = context.identity();
    assert!(stale_identity.is_some());
    assert!(context.is_usable());

    // Out-of-band removal, as an external `rm -rf .repin` would do.
    fs::remove_dir_all(&state.layout.state_dir).unwrap();
    assert!(!context.is_usable());
    assert!(context.is_closed());
    drop(context);

    let reinitialized = initialize_state(dir.path()).unwrap();
    assert!(reinitialized.created);
    let fresh = registry.get_or_load(&reinitialized.layout.db_path).unwrap();
    assert_ne!(fresh.identity(), stale_identity);
    assert!(fresh.is_usable());
    assert_eq!(registry.active_count(), 1);
}
