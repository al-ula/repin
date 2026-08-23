use crate::context_handle::WriterLease;
use crate::registry::ContextRegistry;
use repin_core::ports::store::StoreError;
use repin_core::protocol::errors::ErrorCode;
use repin_core::store::SqliteStore;
pub use repin_product::{GRAPH_DB_FILE, STATE_DIR};
use std::fs;
use std::path::{Path, PathBuf};

pub type StateLayout = repin_product::ProjectLayout;

/// Resolve the state directory that an uninitialize request addresses. The
/// supplied root wins when it carries a state directory; otherwise the nearest
/// ancestor with one is selected, matching `DiscoverFrom` in docs/runtime.md.
/// Returns `None` when no ancestor carries a state directory.
pub fn discover_state_layout(start: &Path) -> Option<StateLayout> {
    let mut current = if start.is_file() {
        start.parent()?.to_path_buf()
    } else {
        start.to_path_buf()
    };

    loop {
        let layout = StateLayout::at_root(&current);
        if layout.state_dir.is_dir() {
            return Some(layout);
        }
        current = current.parent()?.to_path_buf();
    }
}

#[derive(Debug)]
pub struct InitializedState {
    pub layout: StateLayout,
    /// False when the state directory and database already existed.
    pub created: bool,
    /// The lease taken before creation, carried to context activation so
    /// ownership is never dropped and reacquired in between (ADR-026).
    pub lease: WriterLease,
}

/// Map a store failure onto the state classes in docs/runtime.md §4. A newer
/// schema is distinct from invalid or corrupt state, and neither is reported
/// as an uninitialized project.
fn classify_store_error(error: &StoreError) -> (ErrorCode, String) {
    match error {
        StoreError::SchemaVersionMismatch { found, supported } => (
            ErrorCode::ProjectStateNewer,
            format!("graph schema version {found} is newer than supported version {supported}"),
        ),
        other => (
            ErrorCode::ProjectStateInvalid,
            format!("graph state is unusable: {other}"),
        ),
    }
}

/// Create `.repin` with owner-only permissions, the ignore marker, and the
/// graph database. The writer lease is taken before the database is created and
/// is returned to the caller for context activation. An existing database is
/// never overwritten, and state that cannot be activated is reported with its
/// state class rather than as a successful initialization (ADR-026).
pub fn initialize_state(project_root: &Path) -> Result<InitializedState, (ErrorCode, String)> {
    let canonical_root = project_root.canonicalize().map_err(|error| {
        (
            ErrorCode::StatePermissions,
            format!("failed to resolve project root: {error}"),
        )
    })?;
    let layout = StateLayout::at_root(&canonical_root);

    // A state entry that exists but is not a regular file is not a marker under
    // docs/runtime.md §3 and can never be activated.
    if layout.db_path.exists() && !layout.db_path.is_file() {
        return Err((
            ErrorCode::ProjectStateInvalid,
            format!(
                "{} exists but is not a regular file",
                layout.db_path.display()
            ),
        ));
    }
    let already_initialized = layout.db_path.is_file();

    fs::create_dir_all(&layout.state_dir).map_err(|error| {
        (
            ErrorCode::StatePermissions,
            format!("failed to create state directory: {error}"),
        )
    })?;
    apply_private_permissions(&layout.state_dir)?;

    if !layout.ignore_marker.exists() {
        fs::write(&layout.ignore_marker, "*\n").map_err(|error| {
            (
                ErrorCode::StatePermissions,
                format!("failed to create ignore marker: {error}"),
            )
        })?;
    }

    // Lease before create: the handle guarding creation is the handle the
    // published context keeps for its lifetime.
    let lease = WriterLease::acquire(&layout.writer_lock);

    // Creating the store stamps the application id and schema version, and
    // classifies an existing database without overwriting it. Only the lease
    // owner writes; an observer leaves creation to the owning process.
    if lease.is_owned() {
        let store = SqliteStore::open(&layout.db_path).map_err(|error| {
            let (code, message) = classify_store_error(&error);
            (code, message)
        })?;
        drop(store);
    } else if !already_initialized {
        return Err((
            ErrorCode::ProjectLeaseUnavailable,
            "another process owns this project's writer lock; cannot create graph state"
                .to_string(),
        ));
    }

    Ok(InitializedState {
        layout,
        created: !already_initialized,
        lease,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemovedState {
    pub project_root: PathBuf,
    /// False when there was no state directory to remove.
    pub removed: bool,
}

/// Unload the project's context before deleting its state directory, so the
/// graph store is closed and the writer lease released while the durable
/// files still exist. Refuses removal while another connection is attached.
pub fn uninitialize_state(
    registry: &ContextRegistry,
    start: &Path,
) -> Result<RemovedState, (ErrorCode, String)> {
    let Some(layout) = discover_state_layout(start) else {
        return Ok(RemovedState {
            project_root: start.to_path_buf(),
            removed: false,
        });
    };

    if registry.attached_count(&layout.db_path) > 0 {
        return Err((
            ErrorCode::ProjectLeaseUnavailable,
            "project has attached clients; close them before uninitializing".to_string(),
        ));
    }

    registry.unload(&layout.db_path);

    fs::remove_dir_all(&layout.state_dir).map_err(|error| {
        (
            ErrorCode::StatePermissions,
            format!("failed to remove state directory: {error}"),
        )
    })?;

    Ok(RemovedState {
        project_root: layout.project_root,
        removed: true,
    })
}

fn apply_private_permissions(state_dir: &Path) -> Result<(), (ErrorCode, String)> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = fs::metadata(state_dir)
        .map_err(|error| {
            (
                ErrorCode::StatePermissions,
                format!("failed to read state directory metadata: {error}"),
            )
        })?
        .permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(state_dir, permissions).map_err(|error| {
        (
            ErrorCode::StatePermissions,
            format!("failed to apply private state permissions: {error}"),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context_handle::ProjectContext;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

    #[test]
    fn initialize_creates_private_state_with_ignore_marker() {
        let dir = tempdir().unwrap();
        let state = initialize_state(dir.path()).unwrap();

        assert!(state.created);
        assert!(state.layout.state_dir.is_dir());
        assert_eq!(
            fs::read_to_string(&state.layout.ignore_marker).unwrap(),
            "*\n"
        );
        let mode = fs::metadata(&state.layout.state_dir)
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o700);
    }

    #[test]
    fn initialize_preserves_an_existing_database_and_marker() {
        let dir = tempdir().unwrap();
        let layout = StateLayout::at_root(dir.path());

        // Create a real, valid database and record its identity.
        let first = initialize_state(dir.path()).unwrap();
        assert!(first.created);
        fs::write(&layout.ignore_marker, "# custom\n").unwrap();
        let before = fs::metadata(&layout.db_path).unwrap().len();
        drop(first);

        let state = initialize_state(dir.path()).unwrap();

        assert!(!state.created);
        assert_eq!(fs::metadata(&layout.db_path).unwrap().len(), before);
        assert_eq!(
            fs::read_to_string(&layout.ignore_marker).unwrap(),
            "# custom\n"
        );
    }

    /// docs/runtime.md §4: initialization must not report success for state it
    /// cannot activate. A file that is not a database is `PROJECT_STATE_INVALID`,
    /// not a successful init and not an uninitialized project.
    #[test]
    fn initialize_rejects_a_corrupt_database_instead_of_reporting_success() {
        let dir = tempdir().unwrap();
        let layout = StateLayout::at_root(dir.path());
        fs::create_dir_all(&layout.state_dir).unwrap();
        fs::write(&layout.db_path, b"not a database").unwrap();

        let error = initialize_state(dir.path()).unwrap_err();

        assert_eq!(error.0, ErrorCode::ProjectStateInvalid);
        // The unusable bytes are preserved, never silently replaced.
        assert_eq!(fs::read(&layout.db_path).unwrap(), b"not a database");
    }

    /// The state entry must be a regular file to be a marker (docs/runtime.md §3).
    #[test]
    fn initialize_rejects_a_non_regular_state_entry() {
        let dir = tempdir().unwrap();
        let layout = StateLayout::at_root(dir.path());
        fs::create_dir_all(&layout.db_path).unwrap();

        let error = initialize_state(dir.path()).unwrap_err();

        assert_eq!(error.0, ErrorCode::ProjectStateInvalid);
        assert!(error.1.contains("not a regular file"));
    }

    /// Lease-before-create: the handle that guarded creation is handed to the
    /// context, so ownership is continuous across activation (ADR-026).
    #[test]
    fn initialize_takes_the_lease_before_creating_and_hands_it_to_the_context() {
        let dir = tempdir().unwrap();
        let state = initialize_state(dir.path()).unwrap();

        assert!(state.lease.is_owned(), "creator must own the writer lease");
        assert!(state.layout.db_path.is_file());

        let context =
            ProjectContext::open_with_lease(state.layout.db_path.clone(), state.lease).unwrap();
        assert!(
            context.is_writer(),
            "the creating lease must carry into the published context"
        );
        assert!(context.is_usable());
    }

    #[test]
    fn uninitialize_unloads_the_context_then_removes_state() {
        let dir = tempdir().unwrap();
        let registry = ContextRegistry::new();
        let state = initialize_state(dir.path()).unwrap();
        let context = registry.get_or_load(&state.layout.db_path).unwrap();
        drop(context);

        let removed = uninitialize_state(&registry, dir.path()).unwrap();

        assert!(removed.removed);
        assert!(!state.layout.state_dir.exists());
        assert_eq!(registry.active_count(), 0);
    }

    #[test]
    fn uninitialize_is_refused_while_a_client_is_attached() {
        let dir = tempdir().unwrap();
        let registry = ContextRegistry::new();
        let state = initialize_state(dir.path()).unwrap();
        let _attached = registry.get_or_load(&state.layout.db_path).unwrap();

        let error = uninitialize_state(&registry, dir.path()).unwrap_err();

        assert_eq!(error.0, ErrorCode::ProjectLeaseUnavailable);
        assert!(state.layout.state_dir.is_dir());
    }

    #[test]
    fn uninitialize_from_a_subdirectory_selects_the_nearest_ancestor() {
        let dir = tempdir().unwrap();
        let registry = ContextRegistry::new();
        let state = initialize_state(dir.path()).unwrap();
        let nested = dir.path().join("src").join("nested");
        fs::create_dir_all(&nested).unwrap();

        let removed = uninitialize_state(&registry, &nested).unwrap();

        assert!(removed.removed);
        assert!(!state.layout.state_dir.exists());
    }

    #[test]
    fn uninitialize_reports_nothing_removed_without_state() {
        let dir = tempdir().unwrap();
        let registry = ContextRegistry::new();

        let removed = uninitialize_state(&registry, dir.path()).unwrap();

        assert!(!removed.removed);
    }
}
