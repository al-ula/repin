use repin_core::config::{ConfigError, RepinConfig};
use std::fs;
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

pub fn find_project_config_path(root_dir: &Path) -> Option<PathBuf> {
    let meta_config = root_dir.join(".repin").join("config.toml");
    if meta_config.is_file() {
        return Some(meta_config);
    }
    let root_config = root_dir.join("config.toml");
    if root_config.is_file() {
        return Some(root_config);
    }
    None
}

pub fn load_effective_config(
    root_dir: &Path,
    explicit_config_path: Option<&Path>,
) -> Result<RepinConfig, ConfigError> {
    let mut config = RepinConfig::default();

    // 1. User global config (~/.config/repin/config.toml)
    if let Some(home_dir) = std::env::var_os("HOME").map(PathBuf::from) {
        let user_config = home_dir.join(".config").join("repin").join("config.toml");
        if user_config.is_file()
            && let Ok(content) = fs::read_to_string(&user_config)
        {
            let _ = config.merge_toml_str(&content);
        }
    }

    // 2. Project config or explicit override
    if let Some(explicit_path) = explicit_config_path {
        let content = fs::read_to_string(explicit_path)?;
        config.merge_toml_str(&content)?;
    } else if let Some(project_config_path) = find_project_config_path(root_dir) {
        let content = fs::read_to_string(project_config_path)?;
        config.merge_toml_str(&content)?;
    }

    config.validate()?;
    Ok(config)
}
