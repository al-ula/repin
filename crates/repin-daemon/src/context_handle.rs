use crate::lease::FileLease;
use repin_core::config::RepinConfig;
use repin_engine::{Engine, EngineOptions};
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// Physical identity of the graph database backing a context. Recorded when
/// the store is opened and revalidated before reuse or dispatch (ADR-026).
/// It is an active safety guard only and is never exposed as a project id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DatabaseIdentity {
    device: u64,
    inode: u64,
}

impl DatabaseIdentity {
    pub fn read(db_path: &Path) -> Option<Self> {
        let metadata = fs::metadata(db_path).ok()?;
        if !metadata.is_file() {
            return None;
        }
        Some(Self {
            device: metadata.dev(),
            inode: metadata.ino(),
        })
    }
}

/// Outcome of attempting to take a project's writer lease.
#[derive(Debug)]
pub enum WriterLease {
    /// This process owns the lease and may serve authoritative writes.
    Owned(FileLease),
    /// Another process owns it; attach as an observer per docs/runtime.md §6.
    Observer,
}

impl WriterLease {
    /// Acquire the lease for a project state directory. Non-blocking: a lease
    /// held elsewhere yields `Observer` rather than an error, because observer
    /// attachment is a conforming outcome.
    pub fn acquire(repin_dir: &Path) -> Self {
        match FileLease::try_acquire(repin_dir.join("writer.lock")) {
            Ok(lease) => Self::Owned(lease),
            Err(_) => Self::Observer,
        }
    }

    pub fn is_owned(&self) -> bool {
        matches!(self, Self::Owned(_))
    }

    fn into_handle(self) -> Option<FileLease> {
        match self {
            Self::Owned(lease) => Some(lease),
            Self::Observer => None,
        }
    }
}

pub struct ProjectContext {
    canonical_db_path: PathBuf,
    project_root: PathBuf,
    engine: Engine,
    writer_lease: Option<FileLease>,
    config: RepinConfig,
    identity: Option<DatabaseIdentity>,
    closed: AtomicBool,
}

impl ProjectContext {
    pub fn open(canonical_db_path: PathBuf) -> Result<Self, String> {
        let repin_dir = canonical_db_path.parent().ok_or("invalid db path")?;
        let lease = WriterLease::acquire(repin_dir);
        Self::open_with_lease(canonical_db_path, lease)
    }

    /// Activate a context around a lease the caller already holds. Initialization
    /// creates the database under its lease and then publishes the context with
    /// that same handle, so ownership is never dropped and reacquired between
    /// creation and activation (ADR-026).
    pub fn open_with_lease(canonical_db_path: PathBuf, lease: WriterLease) -> Result<Self, String> {
        let repin_dir = canonical_db_path.parent().ok_or("invalid db path")?;
        let project_root = repin_dir
            .parent()
            .ok_or("invalid project root")?
            .to_path_buf();

        let writer_lease = lease.into_handle();

        let mut config = RepinConfig::default();
        let meta_config = repin_dir.join("config.toml");
        let root_config = project_root.join("config.toml");

        if meta_config.is_file() {
            if let Ok(content) = fs::read_to_string(&meta_config) {
                let _ = config.merge_toml_str(&content);
            }
        } else if root_config.is_file()
            && let Ok(content) = fs::read_to_string(&root_config)
        {
            let _ = config.merge_toml_str(&content);
        }

        let engine = Engine::open(EngineOptions {
            root_id: "root".to_string(),
            root_path: project_root.clone(),
            db_path: Some(canonical_db_path.clone()),
        })?;

        let identity = DatabaseIdentity::read(&canonical_db_path);

        Ok(Self {
            canonical_db_path,
            project_root,
            engine,
            writer_lease,
            config,
            identity,
            closed: AtomicBool::new(false),
        })
    }

    pub fn is_writer(&self) -> bool {
        self.writer_lease.is_some()
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    pub fn canonical_db_path(&self) -> &Path {
        &self.canonical_db_path
    }

    pub fn project_root(&self) -> &Path {
        &self.project_root
    }

    pub fn config(&self) -> &RepinConfig {
        &self.config
    }

    pub fn identity(&self) -> Option<DatabaseIdentity> {
        self.identity
    }

    /// True when the backing database still has the identity recorded at open
    /// time. A missing file or a different device/inode pair means the durable
    /// state was removed or replaced underneath this context.
    pub fn identity_is_current(&self) -> bool {
        DatabaseIdentity::read(&self.canonical_db_path) == self.identity
    }

    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    /// Fail this context closed. A closed context serves no further graph
    /// reads or writes and is never rebound to a new file.
    pub fn mark_closed(&self) {
        self.closed.store(true, Ordering::SeqCst);
    }

    /// Revalidate before serving work: a stale identity marks the context
    /// closed and reports it as unusable.
    pub fn is_usable(&self) -> bool {
        if self.is_closed() {
            return false;
        }
        if !self.identity_is_current() {
            self.mark_closed();
            return false;
        }
        true
    }
}
