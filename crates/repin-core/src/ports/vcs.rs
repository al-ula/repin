use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BranchInfo {
    pub name: String,
    pub head_commit: String,
    pub is_detached: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VcsChangeSet {
    pub base_revision: String,
    pub modified_files: Vec<String>,
    pub added_files: Vec<String>,
    pub deleted_files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum VcsError {
    #[error("not a git repository: {0}")]
    NotRepository(String),
    #[error("vcs command execution failed: {0}")]
    CommandFailed(String),
    #[error("timeout waiting for vcs operation")]
    Timeout,
}

pub trait Vcs: Send + Sync {
    fn current_branch(&self, root_path: &str) -> Result<BranchInfo, VcsError>;
    fn changed_files_since(
        &self,
        root_path: &str,
        revision: &str,
    ) -> Result<VcsChangeSet, VcsError>;
    fn head_revision(&self, root_path: &str) -> Result<String, VcsError>;
    fn status(&self, root_path: &str) -> Result<VcsChangeSet, VcsError>;
}
