use crate::ports::vcs::{BranchInfo, Vcs, VcsChangeSet, VcsError};
use std::process::Command;

pub struct GitVcs;

impl Default for GitVcs {
    fn default() -> Self {
        Self
    }
}

impl GitVcs {
    pub fn new() -> Self {
        Self
    }
}

impl Vcs for GitVcs {
    fn current_branch(&self, root_path: &str) -> Result<BranchInfo, VcsError> {
        let output = Command::new("git")
            .arg("-C")
            .arg(root_path)
            .arg("branch")
            .arg("--show-current")
            .output()
            .map_err(|e| VcsError::CommandFailed(e.to_string()))?;

        if !output.status.success() {
            return Err(VcsError::NotRepository(root_path.to_string()));
        }

        let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let is_detached = name.is_empty();

        let head = self.head_revision(root_path)?;

        Ok(BranchInfo {
            name: if is_detached {
                "HEAD".to_string()
            } else {
                name
            },
            head_commit: head,
            is_detached,
        })
    }

    fn head_revision(&self, root_path: &str) -> Result<String, VcsError> {
        let output = Command::new("git")
            .arg("-C")
            .arg(root_path)
            .arg("rev-parse")
            .arg("HEAD")
            .output()
            .map_err(|e| VcsError::CommandFailed(e.to_string()))?;

        if !output.status.success() {
            return Err(VcsError::NotRepository(root_path.to_string()));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    fn changed_files_since(
        &self,
        root_path: &str,
        revision: &str,
    ) -> Result<VcsChangeSet, VcsError> {
        let output = Command::new("git")
            .arg("-C")
            .arg(root_path)
            .arg("diff")
            .arg("--name-status")
            .arg(revision)
            .output()
            .map_err(|e| VcsError::CommandFailed(e.to_string()))?;

        if !output.status.success() {
            return Err(VcsError::CommandFailed("git diff failed".to_string()));
        }

        let mut modified_files = Vec::new();
        let mut added_files = Vec::new();
        let mut deleted_files = Vec::new();

        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let status = parts[0];
                let file = parts[1].to_string();
                if status.starts_with('M') {
                    modified_files.push(file);
                } else if status.starts_with('A') {
                    added_files.push(file);
                } else if status.starts_with('D') {
                    deleted_files.push(file);
                }
            }
        }

        Ok(VcsChangeSet {
            base_revision: revision.to_string(),
            modified_files,
            added_files,
            deleted_files,
        })
    }

    fn status(&self, root_path: &str) -> Result<VcsChangeSet, VcsError> {
        let output = Command::new("git")
            .arg("-C")
            .arg(root_path)
            .arg("status")
            .arg("--porcelain")
            .output()
            .map_err(|e| VcsError::CommandFailed(e.to_string()))?;

        if !output.status.success() {
            return Err(VcsError::CommandFailed("git status failed".to_string()));
        }

        let mut modified_files = Vec::new();
        let mut added_files = Vec::new();
        let mut deleted_files = Vec::new();

        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if line.len() >= 4 {
                let status_code = &line[..2];
                let file = line[3..].trim().to_string();
                if status_code.contains('M') {
                    modified_files.push(file);
                } else if status_code.contains('?') || status_code.contains('A') {
                    added_files.push(file);
                } else if status_code.contains('D') {
                    deleted_files.push(file);
                }
            }
        }

        let head = self
            .head_revision(root_path)
            .unwrap_or_else(|_| "HEAD".to_string());

        Ok(VcsChangeSet {
            base_revision: head,
            modified_files,
            added_files,
            deleted_files,
        })
    }
}
