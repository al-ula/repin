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
            .flatten()
        {
            if entry.file_type().is_some_and(|ft| ft.is_file()) {
                let path = entry.path();
                if let Ok(rel) = path.strip_prefix(self.root_path()) {
                    let rel_str = rel.to_string_lossy();
                    if !self.filter().is_excluded(&rel_str) {
                        let snapshot = self.read_snapshot(&rel_str).map_err(source_error)?;
                        callback(snapshot)?;
                    }
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
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_capability_fs_containment() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        let mut file = File::create(&file_path).unwrap();
        writeln!(file, "hello").unwrap();

        let fs = CapabilityFs::open("root1", dir.path()).unwrap();
        let snapshot = fs.read_snapshot("test.txt").unwrap();
        assert_eq!(snapshot.content, b"hello\n");

        assert!(fs.read_snapshot("../escape.txt").is_err());
    }

    #[test]
    fn source_contract_preserves_containment_and_walks_snapshots() {
        let dir = tempdir().unwrap();
        let src_dir = dir.path().join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::write(src_dir.join("main.rs"), b"fn main() {}\n").unwrap();
        std::fs::write(dir.path().join(".env"), b"SECRET=1\n").unwrap();

        let fs = CapabilityFs::open("root1", dir.path()).unwrap();
        let source_fs: &dyn SourceFs = &fs;

        let snapshot = source_fs.read_snapshot("src/main.rs").unwrap();
        assert_eq!(snapshot.root, "root1");
        assert_eq!(snapshot.path, "src/main.rs");
        assert_eq!(snapshot.content, b"fn main() {}\n");

        assert_eq!(
            source_fs.read_snapshot("../outside.rs").unwrap_err(),
            SourceError::Containment("../outside.rs".to_string())
        );
        assert_eq!(
            source_fs.read_snapshot(".env").unwrap_err(),
            SourceError::Excluded(".env".to_string())
        );

        let mut observed = Vec::new();
        source_fs
            .walk_files(&mut |item| {
                observed.push(item.path);
                Ok(())
            })
            .unwrap();

        assert_eq!(observed, vec!["src/main.rs".to_string()]);
    }
}
