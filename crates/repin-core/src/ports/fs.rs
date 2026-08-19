use crate::hash::ContentHash;
use crate::line_index::ByteSpan;
use crate::model::provenance::FactOwner;
use crate::model::registries::ArtifactClass;
use serde::{Deserialize, Serialize};

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
