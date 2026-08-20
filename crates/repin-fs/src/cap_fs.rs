use crate::exclusions::{ExclusionFilter, classify_artifact};
use cap_std::fs::Dir;
use repin_core::hash::ContentHash;
use repin_core::ports::fs::FileSnapshot;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum FsError {
    #[error("I/O error at {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("path {0} is outside root containment boundary")]
    Escape(String),
    #[error("path {0} is excluded by safety rules")]
    Excluded(String),
}

pub struct CapabilityFs {
    root_id: String,
    root_path: PathBuf,
    dir: Dir,
    filter: ExclusionFilter,
}

impl CapabilityFs {
    pub fn open(root_id: impl Into<String>, root_path: impl AsRef<Path>) -> Result<Self, FsError> {
        let root_path_buf = root_path.as_ref().to_path_buf();
        let canonical_root = root_path_buf.canonicalize().map_err(|e| FsError::Io {
            path: root_path_buf.display().to_string(),
            source: e,
        })?;

        let dir =
            Dir::open_ambient_dir(&canonical_root, cap_std::ambient_authority()).map_err(|e| {
                FsError::Io {
                    path: canonical_root.display().to_string(),
                    source: e,
                }
            })?;

        Ok(Self {
            root_id: root_id.into(),
            root_path: canonical_root,
            dir,
            filter: ExclusionFilter::default(),
        })
    }

    pub fn root_id(&self) -> &str {
        &self.root_id
    }

    pub fn root_path(&self) -> &Path {
        &self.root_path
    }

    pub fn read_snapshot(&self, relative_path: &str) -> Result<FileSnapshot, FsError> {
        if !is_root_relative(relative_path) {
            return Err(FsError::Escape(relative_path.to_string()));
        }
        if self.filter.is_excluded(relative_path) {
            return Err(FsError::Excluded(relative_path.to_string()));
        }

        let content = self.dir.read(relative_path).map_err(|e| FsError::Io {
            path: relative_path.to_string(),
            source: e,
        })?;

        let content_hash = ContentHash::of_bytes(&content);
        let artifact_class = classify_artifact(relative_path);

        Ok(FileSnapshot {
            root: self.root_id.clone(),
            path: relative_path.to_string(),
            content,
            content_hash,
            artifact_class,
        })
    }

    pub fn walk_files<F>(&self, mut callback: F) -> Result<(), FsError>
    where
        F: FnMut(FileSnapshot) -> Result<(), FsError>,
    {
        for entry in ignore::WalkBuilder::new(&self.root_path)
            .hidden(false)
            .git_ignore(true)
            .build()
            .flatten()
        {
            if entry.file_type().is_some_and(|ft| ft.is_file()) {
                let path = entry.path();
                if let Ok(rel) = path.strip_prefix(&self.root_path) {
                    let rel_str = rel.to_string_lossy();
                    if !self.filter.is_excluded(&rel_str)
                        && let Ok(snapshot) = self.read_snapshot(&rel_str)
                    {
                        callback(snapshot)?;
                    }
                }
            }
        }
        Ok(())
    }
}

fn is_root_relative(path: &str) -> bool {
    if path.is_empty() {
        return false;
    }
    let path = Path::new(path);
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return false;
    }

    let normalized = path.to_string_lossy().replace('\\', "/");
    !normalized.starts_with('/')
        && !normalized
            .split('/')
            .any(|component| component == ".." || component.ends_with(':'))
}
