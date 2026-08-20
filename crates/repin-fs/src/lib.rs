pub mod cap_fs;
pub mod exclusions;
pub mod git_vcs;

pub use cap_fs::{CapabilityFs, FsError};
pub use exclusions::{ExclusionFilter, classify_artifact};
pub use git_vcs::GitVcs;

use repin_core::ports::fs::{FileSnapshot, SourceError, SourceFs};

impl SourceFs for CapabilityFs {
    fn read_snapshot(&self, relative_path: &str) -> Result<FileSnapshot, SourceError> {
        CapabilityFs::read_snapshot(self, relative_path).map_err(source_error)
    }

    fn walk_files(
        &self,
        callback: &mut dyn FnMut(FileSnapshot) -> Result<(), SourceError>,
    ) -> Result<(), SourceError> {
        for entry in ignore::WalkBuilder::new(self.root_path())
            .hidden(false)
            .git_ignore(true)
            .build()
        {
            let entry = entry.map_err(|error| SourceError::Io {
                path: self.root_path().display().to_string(),
                message: error.to_string(),
            })?;
            if entry
                .file_type()
                .is_some_and(|file_type| file_type.is_file())
                && let Ok(relative) = entry.path().strip_prefix(self.root_path())
            {
                let relative_path = relative.to_string_lossy();
                match self.read_snapshot(&relative_path) {
                    Ok(snapshot) => callback(snapshot)?,
                    Err(FsError::Excluded(_)) => {}
                    Err(error) => return Err(source_error(error)),
                }
            }
        }
        Ok(())
    }
}

fn source_error(error: FsError) -> SourceError {
    match error {
        FsError::Io { path, source } => SourceError::Io {
            path,
            message: source.to_string(),
        },
        FsError::Escape(path) => SourceError::Containment(path),
        FsError::Excluded(path) => SourceError::Excluded(path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use repin_core::ports::fs::{SourceError, SourceFs};
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

    #[test]
    fn source_contract_preserves_containment_and_walks_snapshots() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("test.txt"), b"hello").unwrap();
        let cap = CapabilityFs::open("root", dir.path()).unwrap();
        let source: &dyn SourceFs = &cap;

        assert!(matches!(
            source.read_snapshot("../outside"),
            Err(SourceError::Containment(_))
        ));
        assert!(matches!(
            source.read_snapshot(r"..\outside"),
            Err(SourceError::Containment(_))
        ));
        assert!(matches!(
            source.read_snapshot(r"C:\outside"),
            Err(SourceError::Containment(_))
        ));
        assert!(matches!(
            source.read_snapshot(".env"),
            Err(SourceError::Excluded(_))
        ));

        let mut paths = Vec::new();
        source
            .walk_files(&mut |snapshot| {
                paths.push(snapshot.path);
                Ok(())
            })
            .unwrap();
        assert_eq!(paths, vec!["test.txt"]);

        let error = source.walk_files(&mut |_| Err(SourceError::Cancelled));
        assert!(matches!(error, Err(SourceError::Cancelled)));
    }
}
