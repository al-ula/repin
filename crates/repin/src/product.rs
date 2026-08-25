//! Repin's product-specific path policy.
//!
//! The capability crates accept explicit paths and roots. This crate supplies
//! the concrete layout used by the Repin CLI and daemon, plus host-default
//! selection for those entry points. Layout construction does not touch the
//! filesystem.

use std::env;
use std::fmt;
use std::path::{Path, PathBuf};

pub const PRODUCT_DIR: &str = "repin";
pub const STATE_DIR: &str = ".repin";
pub const GRAPH_DB_FILE: &str = "graph.sqlite3";
pub const PROJECT_CONFIG_FILE: &str = "config.toml";
pub const COMPATIBILITY_CONFIG_FILE: &str = "repin.toml";
pub const WRITER_LOCK_FILE: &str = "writer.lock";
pub const IGNORE_MARKER_FILE: &str = ".gitignore";
pub const DAEMON_SOCKET_FILE: &str = "daemon.sock";
pub const DAEMON_LOCK_FILE: &str = "daemon.lock";
pub const MODEL_ROOT_DIR: &str = "models";
pub const GITHUB_BASE: &str = "https://github.com/al-ula/repin";
pub const GITHUB_API_LATEST_RELEASE: &str =
    "https://api.github.com/repos/al-ula/repin/releases/latest";
pub const BINARY_NAME: &str = "repin";
pub const DOCS_DIR_NAME: &str = "docs";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectLayout {
    pub project_root: PathBuf,
    pub state_dir: PathBuf,
    pub db_path: PathBuf,
    pub project_config: PathBuf,
    pub root_config: PathBuf,
    pub compatibility_config: PathBuf,
    pub writer_lock: PathBuf,
    pub ignore_marker: PathBuf,
}

impl ProjectLayout {
    #[must_use]
    pub fn at_root(root: impl AsRef<Path>) -> Self {
        let project_root = root.as_ref().to_path_buf();
        let state_dir = project_root.join(STATE_DIR);
        Self {
            project_root: project_root.clone(),
            db_path: state_dir.join(GRAPH_DB_FILE),
            project_config: state_dir.join(PROJECT_CONFIG_FILE),
            root_config: project_root.join(PROJECT_CONFIG_FILE),
            compatibility_config: project_root.join(COMPATIBILITY_CONFIG_FILE),
            writer_lock: state_dir.join(WRITER_LOCK_FILE),
            ignore_marker: state_dir.join(IGNORE_MARKER_FILE),
            state_dir,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeLayout {
    pub base: PathBuf,
    pub socket_path: PathBuf,
    pub daemon_lock: PathBuf,
}

impl RuntimeLayout {
    #[must_use]
    pub fn at_base(base: impl AsRef<Path>) -> Self {
        let base = base.as_ref().to_path_buf();
        Self {
            socket_path: base.join(DAEMON_SOCKET_FILE),
            daemon_lock: base.join(DAEMON_LOCK_FILE),
            base,
        }
    }
}

#[must_use]
pub fn default_runtime_layout() -> RuntimeLayout {
    let base = env::var_os("XDG_RUNTIME_DIR")
        .map(|dir| PathBuf::from(dir).join(PRODUCT_DIR))
        .unwrap_or_else(|| env::temp_dir().join("repin-runtime"));
    RuntimeLayout::at_base(base)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserLayout {
    pub config_base: PathBuf,
    pub cache_base: PathBuf,
    pub data_base: PathBuf,
    pub bin_base: PathBuf,
    pub global_config: PathBuf,
    pub model_root: PathBuf,
    pub install_dir: PathBuf,
    pub install_bin: PathBuf,
    pub install_docs: PathBuf,
    pub bin_link: PathBuf,
}

impl UserLayout {
    #[must_use]
    pub fn at_bases(
        config_base: impl AsRef<Path>,
        cache_base: impl AsRef<Path>,
        data_base: impl AsRef<Path>,
        bin_base: impl AsRef<Path>,
    ) -> Self {
        let config_base = config_base.as_ref().to_path_buf();
        let cache_base = cache_base.as_ref().to_path_buf();
        let data_base = data_base.as_ref().to_path_buf();
        let bin_base = bin_base.as_ref().to_path_buf();
        let install_dir = data_base.join(PRODUCT_DIR);
        Self {
            global_config: config_base.join(PRODUCT_DIR).join(PROJECT_CONFIG_FILE),
            model_root: cache_base.join(PRODUCT_DIR).join(MODEL_ROOT_DIR),
            install_bin: install_dir.join(BINARY_NAME),
            install_docs: install_dir.join(DOCS_DIR_NAME),
            bin_link: bin_base.join(BINARY_NAME),
            install_dir,
            config_base,
            cache_base,
            data_base,
            bin_base,
        }
    }

    pub fn from_home(home: impl AsRef<Path>) -> Self {
        let home = home.as_ref();
        Self::at_bases(
            home.join(".config"),
            home.join(".cache"),
            home.join(".local").join("share"),
            home.join(".local").join("bin"),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingHome;

impl fmt::Display for MissingHome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("HOME environment variable not set")
    }
}

impl std::error::Error for MissingHome {}

pub fn default_user_layout() -> Result<UserLayout, MissingHome> {
    let home = env::var_os("HOME").ok_or(MissingHome)?;
    let home_path = PathBuf::from(home);

    let config_base = env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_path.join(".config"));
    let cache_base = env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_path.join(".cache"));
    let data_base = env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_path.join(".local").join("share"));
    let bin_base = env::var_os("XDG_BIN_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_path.join(".local").join("bin"));

    Ok(UserLayout::at_bases(
        config_base,
        cache_base,
        data_base,
        bin_base,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_layout_centralizes_product_files() {
        let layout = ProjectLayout::at_root("/project");
        assert_eq!(layout.state_dir, Path::new("/project/.repin"));
        assert_eq!(layout.db_path, Path::new("/project/.repin/graph.sqlite3"));
        assert_eq!(
            layout.project_config,
            Path::new("/project/.repin/config.toml")
        );
        assert_eq!(layout.root_config, Path::new("/project/config.toml"));
        assert_eq!(
            layout.compatibility_config,
            Path::new("/project/repin.toml")
        );
        assert_eq!(layout.writer_lock, Path::new("/project/.repin/writer.lock"));
        assert_eq!(
            layout.ignore_marker,
            Path::new("/project/.repin/.gitignore")
        );
    }

    #[test]
    fn runtime_and_user_layouts_use_explicit_bases() {
        let runtime = RuntimeLayout::at_base("/run/repin");
        assert_eq!(runtime.socket_path, Path::new("/run/repin/daemon.sock"));
        assert_eq!(runtime.daemon_lock, Path::new("/run/repin/daemon.lock"));

        let user = UserLayout::from_home("/home/tester");
        assert_eq!(
            user.global_config,
            Path::new("/home/tester/.config/repin/config.toml")
        );
        assert_eq!(
            user.model_root,
            Path::new("/home/tester/.cache/repin/models")
        );
        assert_eq!(
            user.install_dir,
            Path::new("/home/tester/.local/share/repin")
        );
        assert_eq!(
            user.install_bin,
            Path::new("/home/tester/.local/share/repin/repin")
        );
        assert_eq!(
            user.install_docs,
            Path::new("/home/tester/.local/share/repin/docs")
        );
        assert_eq!(user.bin_link, Path::new("/home/tester/.local/bin/repin"));
    }
}
