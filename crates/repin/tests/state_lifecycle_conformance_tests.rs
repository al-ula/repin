use repin::cli::client::DaemonClient;
use repin::{
    ContextRegistry, DaemonServer, FileLease, RuntimeLayout, initialize_state, uninitialize_state,
};
use repin_core::config::RepinConfig;
use repin_core::protocol::errors::ErrorCode;
use repin_core::protocol::ipc::{IpcRequest, IpcResponse};
use repin_core::protocol::{PROTOCOL_MAX, PROTOCOL_MIN, PROTOCOL_STATE_LIFECYCLE, select_protocol};
use std::fs;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;
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

/// docs/runtime.md §7: detached context idle timeout in seconds and persistence when set to 0.
#[test]
fn detached_context_idle_timeout_and_zero_persistence_conformance() {
    let dir = tempdir().unwrap();
    let registry = ContextRegistry::new();
    let state = initialize_state(dir.path()).unwrap();

    let mut config = RepinConfig::default();
    config.daemon.idle_timeout_secs = 1;

    let context = registry
        .get_or_load_with_config(&state.layout.db_path, config)
        .unwrap();
    assert_eq!(registry.active_count(), 1);
    drop(context);
    registry.mark_detached(&state.layout.db_path);

    // Immediately checking idle reap does not evict the context
    registry.reap_idle();
    assert_eq!(registry.active_count(), 1);

    // After 1.1 seconds, idle reap removes the context
    std::thread::sleep(Duration::from_millis(1100));
    registry.reap_idle();
    assert_eq!(registry.active_count(), 0);

    // Context with idle_timeout_secs = 0 is persistent
    let mut persistent_config = RepinConfig::default();
    persistent_config.daemon.idle_timeout_secs = 0;

    let persistent_ctx = registry
        .get_or_load_with_config(&state.layout.db_path, persistent_config)
        .unwrap();
    assert_eq!(registry.active_count(), 1);
    drop(persistent_ctx);
    registry.mark_detached(&state.layout.db_path);

    std::thread::sleep(Duration::from_millis(100));
    registry.reap_idle();
    assert_eq!(registry.active_count(), 1, "0 must disable idle eviction");
}

/// docs/runtime.md §7: CLI --idle-timeout override takes precedence over per-context configuration.
#[test]
fn cli_idle_timeout_override_conformance() {
    let dir = tempdir().unwrap();
    let registry = ContextRegistry::new();
    registry.set_override_idle_timeout(Some(Some(Duration::from_millis(50))));
    let state = initialize_state(dir.path()).unwrap();

    let mut config = RepinConfig::default();
    config.daemon.idle_timeout_secs = 600;

    let context = registry
        .get_or_load_with_config(&state.layout.db_path, config)
        .unwrap();
    assert_eq!(registry.active_count(), 1);
    drop(context);
    registry.mark_detached(&state.layout.db_path);

    std::thread::sleep(Duration::from_millis(70));
    registry.reap_idle();
    assert_eq!(
        registry.active_count(),
        0,
        "CLI override must take precedence over config"
    );
}

/// docs/runtime.md §7 & §9(6): daemon auto-exits once all active contexts are unloaded and zero connections remain.
#[test]
fn daemon_server_auto_exits_when_final_context_unloads() {
    let rt_dir = tempdir().unwrap();
    let project_dir = tempdir().unwrap();
    let state = initialize_state(project_dir.path()).unwrap();
    let rt_layout = RuntimeLayout::at_base(rt_dir.path());

    let server = Arc::new(
        DaemonServer::bind(rt_dir.path(), Some(Some(Duration::from_millis(100)))).unwrap(),
    );
    let server_clone = server.clone();
    let server_handle = std::thread::spawn(move || server_clone.run_loop());

    // Verify daemon did not immediately auto-exit on startup before any context activation
    std::thread::sleep(Duration::from_millis(150));
    assert!(
        server.running().load(Ordering::SeqCst),
        "daemon must not exit before first context activation"
    );
    assert!(rt_layout.socket_path.exists());

    // Connect a client and bind context
    let mut client = DaemonClient::connect_existing(Some(rt_dir.path())).unwrap();
    let handshake_resp = client
        .send_request(IpcRequest::Handshake {
            client_version: env!("CARGO_PKG_VERSION").to_string(),
            project_db_path: state.layout.db_path.display().to_string(),
            resolved_config: Some(RepinConfig::default()),
        })
        .unwrap();
    match handshake_resp {
        IpcResponse::HandshakeOk { .. } => {}
        other => panic!("expected HandshakeOk, got {other:?}"),
    }

    assert!(server.registry().has_ever_activated());
    assert_eq!(server.registry().active_count(), 1);

    // Disconnect client
    drop(client);

    // Wait for auto-exit (idle timeout 100ms + drain)
    let join_result = server_handle.join().expect("server thread panicked");
    assert!(join_result.is_ok(), "server run_loop exited cleanly");

    // Socket file removed and lock released
    assert!(
        !rt_layout.socket_path.exists(),
        "socket must be cleaned up on auto-exit"
    );
    drop(server);
    let lease = FileLease::try_acquire(&rt_layout.daemon_lock);
    assert!(lease.is_ok(), "daemon lock must be released on exit");
}

/// docs/runtime.md §7 & §9(6): daemon auto-exits after search request and disconnect.
#[test]
fn daemon_server_auto_exits_after_search_request() {
    let rt_dir = tempdir().unwrap();
    let project_dir = tempdir().unwrap();
    let state = initialize_state(project_dir.path()).unwrap();
    let rt_layout = RuntimeLayout::at_base(rt_dir.path());

    let server = Arc::new(
        DaemonServer::bind(rt_dir.path(), Some(Some(Duration::from_millis(100)))).unwrap(),
    );
    let server_clone = server.clone();
    let server_handle = std::thread::spawn(move || server_clone.run_loop());
    for _ in 0..50 {
        if rt_layout.socket_path.exists() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    let mut client = DaemonClient::connect_existing(Some(rt_dir.path())).unwrap();
    let handshake_resp = client
        .send_request(IpcRequest::Handshake {
            client_version: env!("CARGO_PKG_VERSION").to_string(),
            project_db_path: state.layout.db_path.display().to_string(),
            resolved_config: Some(RepinConfig::default()),
        })
        .unwrap();
    match handshake_resp {
        IpcResponse::HandshakeOk { .. } => {}
        other => panic!("expected HandshakeOk, got {other:?}"),
    }

    let search_resp = client
        .send_request(IpcRequest::SearchHybrid {
            query: "test".to_string(),
            max_results: Some(10),
            centrality_boost: None,
        })
        .unwrap();
    match search_resp {
        IpcResponse::SearchResult(_) => {}
        other => panic!("expected SearchResult, got {other:?}"),
    }

    drop(client);

    let join_result = server_handle.join().expect("server thread panicked");
    assert!(join_result.is_ok(), "server run_loop exited cleanly");
    assert!(!rt_layout.socket_path.exists());
}

/// docs/runtime.md §4: daemon auto-watches repository files and advances graph revision
/// in the background without requiring synchronous VCS sync requests.
#[test]
fn daemon_auto_watches_file_changes_and_advances_revision() {
    let rt_dir = tempdir().unwrap();
    let project_dir = tempdir().unwrap();
    let src_dir = project_dir.path().join("src");
    fs::create_dir_all(&src_dir).unwrap();
    let state = initialize_state(project_dir.path()).unwrap();
    let db_path = state.layout.db_path.display().to_string();
    drop(state);
    let rt_layout = RuntimeLayout::at_base(rt_dir.path());
    let mut config = RepinConfig::default();
    config.daemon.watch_debounce_ms = 20;

    let server = Arc::new(
        DaemonServer::bind(rt_dir.path(), Some(Some(Duration::from_millis(150)))).unwrap(),
    );
    let server_clone = server.clone();
    let server_handle = std::thread::spawn(move || server_clone.run_loop());
    for _ in 0..50 {
        if rt_layout.socket_path.exists() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    let mut client = DaemonClient::connect_existing(Some(rt_dir.path())).unwrap();
    let handshake_resp = client
        .send_request(IpcRequest::Handshake {
            client_version: env!("CARGO_PKG_VERSION").to_string(),
            project_db_path: db_path,
            resolved_config: Some(config),
        })
        .unwrap();
    match handshake_resp {
        IpcResponse::HandshakeOk { .. } => {}
        other => panic!("expected HandshakeOk, got {other:?}"),
    }

    // Check initial status
    let status = client.send_request(IpcRequest::Status).unwrap();
    let (initial_rev, initial_nodes) = match status {
        IpcResponse::StatusOk {
            graph_revision,
            node_count,
            ..
        } => (graph_revision, node_count),
        other => panic!("expected StatusOk, got {other:?}"),
    };
    // 1. Create a new source file in the project
    let file_path = src_dir.join("foo.rs");
    fs::write(&file_path, "pub fn foo_fn() -> i32 { 42 }\n").unwrap();

    // Poll until daemon watcher picks up file and advances revision
    let mut updated_rev = initial_rev;
    let mut updated_nodes = initial_nodes;
    for _ in 0..60 {
        std::thread::sleep(Duration::from_millis(50));
        if let Ok(IpcResponse::StatusOk {
            graph_revision,
            node_count,
            ..
        }) = client.send_request(IpcRequest::Status)
            && graph_revision > initial_rev
        {
            updated_rev = graph_revision;
            updated_nodes = node_count;
            break;
        }
    }
    assert!(
        updated_rev > initial_rev,
        "daemon auto-watcher failed to advance revision on file creation: rev {initial_rev:?} -> {updated_rev:?}"
    );
    assert!(
        updated_nodes > initial_nodes,
        "node count should increase on adding source file with a symbol"
    );

    // 2. Modify the source file on disk
    fs::write(
        &file_path,
        "pub fn foo_fn() -> i32 { 42 }\npub fn bar_fn() -> i32 { 100 }\n",
    )
    .unwrap();

    let mut modified_rev = updated_rev;
    let mut modified_nodes = updated_nodes;
    for _ in 0..60 {
        std::thread::sleep(Duration::from_millis(50));
        if let Ok(IpcResponse::StatusOk {
            graph_revision,
            node_count,
            ..
        }) = client.send_request(IpcRequest::Status)
            && graph_revision > updated_rev
        {
            modified_rev = graph_revision;
            modified_nodes = node_count;
            break;
        }
    }
    assert!(
        modified_rev > updated_rev,
        "daemon auto-watcher failed to advance revision on file modification: rev {updated_rev:?} -> {modified_rev:?}"
    );
    assert!(
        modified_nodes > updated_nodes,
        "node count should increase on adding second symbol"
    );

    // 3. Delete the source file from disk
    fs::remove_file(&file_path).unwrap();

    let mut deleted_rev = modified_rev;
    let mut deleted_nodes = modified_nodes;
    for _ in 0..60 {
        std::thread::sleep(Duration::from_millis(50));
        if let Ok(IpcResponse::StatusOk {
            graph_revision,
            node_count,
            ..
        }) = client.send_request(IpcRequest::Status)
            && graph_revision > modified_rev
        {
            deleted_rev = graph_revision;
            deleted_nodes = node_count;
            break;
        }
    }
    assert!(
        deleted_rev > modified_rev,
        "daemon auto-watcher failed to advance revision on file deletion: rev {modified_rev:?} -> {deleted_rev:?}"
    );
    assert!(
        deleted_nodes < modified_nodes,
        "node count should decrease on deleting file"
    );

    // Clean up
    drop(client);
    let join_result = server_handle.join().expect("server thread panicked");
    assert!(join_result.is_ok(), "server run_loop exited cleanly");
    assert!(!rt_layout.socket_path.exists());
}
