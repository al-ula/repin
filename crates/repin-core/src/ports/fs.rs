use crate::hash::ContentHash;
use crate::line_index::ByteSpan;
use crate::model::provenance::FactOwner;
use crate::model::registries::ArtifactClass;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeOrigin {
    Watcher,
    Host,
    Cli,
    Scan,
    Vcs,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FileChange {
    Create {
        root: String,
        path: String,
        origin: ChangeOrigin,
        content: Option<Vec<u8>>,
    },
    Modify {
        root: String,
        path: String,
        origin: ChangeOrigin,
        content: Option<Vec<u8>>,
    },
    Delete {
        root: String,
        path: String,
        origin: ChangeOrigin,
    },
    Rename {
        root: String,
        from: String,
        to: String,
        origin: ChangeOrigin,
        content: Option<Vec<u8>>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileSnapshot {
    pub root: String,
    pub path: String,
    pub content: Vec<u8>,
    pub content_hash: ContentHash,
    pub artifact_class: ArtifactClass,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Skip {
    pub root: String,
    pub path: String,
    pub reason: String,
    pub owner: FactOwner,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub root: String,
    pub path: String,
    pub message: String,
    pub span: Option<ByteSpan>,
    pub owner: FactOwner,
}

/// Errors returned by the reusable root-relative source contract.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SourceError {
    #[error("I/O error at {path}: {message}")]
    Io { path: String, message: String },
    #[error("path {0} is outside root containment boundary")]
    Containment(String),
    #[error("path {0} is excluded by safety rules")]
    Excluded(String),
    #[error("source operation cancelled")]
    Cancelled,
    #[error("source limit exceeded: {0}")]
    LimitExceeded(String),
    #[error("source operation failed: {0}")]
    Other(String),
}

/// Minimal filesystem contract required by reusable retrieval, indexing, and
/// context algorithms.
pub trait SourceFs: Send + Sync {
    fn read_snapshot(&self, relative_path: &str) -> Result<FileSnapshot, SourceError>;

    fn walk_files(
        &self,
        callback: &mut dyn FnMut(FileSnapshot) -> Result<(), SourceError>,
    ) -> Result<(), SourceError>;
}
