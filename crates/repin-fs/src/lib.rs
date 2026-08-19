pub mod cap_fs;
pub mod exclusions;
pub mod git_vcs;

pub use cap_fs::{CapabilityFs, FsError};
pub use exclusions::{ExclusionFilter, classify_artifact};
pub use git_vcs::GitVcs;

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_capability_fs_containment() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, b"hello repin").unwrap();

        let cap = CapabilityFs::open("root", dir.path()).unwrap();
        let snapshot = cap.read_snapshot("test.txt").unwrap();
        assert_eq!(snapshot.content, b"hello repin");
        assert_eq!(snapshot.root, "root");
        assert_eq!(snapshot.path, "test.txt");
    }
}
