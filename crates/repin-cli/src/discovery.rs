use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredProject {
    pub root_dir: PathBuf,
    pub db_path: PathBuf,
}

pub fn discover_project_from<P: AsRef<Path>>(start_path: P) -> Option<DiscoveredProject> {
    let mut current = if start_path.as_ref().is_file() {
        start_path.as_ref().parent()?.to_path_buf()
    } else {
        start_path.as_ref().to_path_buf()
    };

    loop {
        let repin_dir = current.join(".repin");
        let db_path = repin_dir.join("graph.sqlite3");
        if repin_dir.is_dir() && db_path.is_file() {
            return Some(DiscoveredProject {
                root_dir: current,
                db_path,
            });
        }

        if let Some(parent) = current.parent() {
            current = parent.to_path_buf();
        } else {
            break;
        }
    }

    None
}
