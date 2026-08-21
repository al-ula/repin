pub mod context_handle;
pub mod lease;
pub mod registry;
pub mod server;
pub mod state;

pub use context_handle::{DatabaseIdentity, ProjectContext};
pub use lease::{FileLease, LeaseError};
pub use registry::ContextRegistry;
pub use server::DaemonServer;
pub use state::{
    GRAPH_DB_FILE, InitializedState, RemovedState, STATE_DIR, StateLayout, discover_state_layout,
    initialize_state, uninitialize_state,
};

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_daemon_binding_and_lease() {
        let dir = tempdir().unwrap();
        let daemon = DaemonServer::bind(dir.path()).unwrap();
        assert!(daemon.socket_path().ends_with("daemon.sock"));
    }
}
