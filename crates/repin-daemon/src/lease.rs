use fs4::fs_std::FileExt;
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum LeaseError {
    #[error("failed to open lock file at {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("lease is already held by another process")]
    AlreadyHeld,
}

pub struct FileLease {
    path: PathBuf,
    file: Option<File>,
}

impl FileLease {
    pub fn try_acquire<P: AsRef<Path>>(path: P) -> Result<Self, LeaseError> {
        let path_buf = path.as_ref().to_path_buf();
        if let Some(parent) = path_buf.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path_buf)
            .map_err(|e| LeaseError::Io {
                path: path_buf.display().to_string(),
                source: e,
            })?;

        match file.try_lock_exclusive() {
            Ok(()) => Ok(Self {
                path: path_buf,
                file: Some(file),
            }),
            Err(_) => Err(LeaseError::AlreadyHeld),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for FileLease {
    fn drop(&mut self) {
        if let Some(file) = self.file.take() {
            let _ = file.unlock();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_file_lease_exclusive() {
        let dir = tempdir().unwrap();
        let lock_path = dir.path().join("test.lock");

        let lease1 = FileLease::try_acquire(&lock_path).unwrap();
        let lease2 = FileLease::try_acquire(&lock_path);
        assert!(matches!(lease2, Err(LeaseError::AlreadyHeld)));

        drop(lease1);

        let lease3 = FileLease::try_acquire(&lock_path);
        assert!(lease3.is_ok());
    }
}
