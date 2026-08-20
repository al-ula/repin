use crate::lease::FileLease;
use repin_core::config::RepinConfig;
use repin_engine::{Engine, EngineOptions};
use std::fs;
use std::path::{Path, PathBuf};

pub struct ProjectContext {
    canonical_db_path: PathBuf,
    project_root: PathBuf,
    engine: Engine,
    writer_lease: Option<FileLease>,
    config: RepinConfig,
}

impl ProjectContext {
    pub fn open(canonical_db_path: PathBuf) -> Result<Self, String> {
        let repin_dir = canonical_db_path.parent().ok_or("invalid db path")?;
        let project_root = repin_dir
            .parent()
            .ok_or("invalid project root")?
            .to_path_buf();

        let lock_path = repin_dir.join("writer.lock");
        let writer_lease = FileLease::try_acquire(&lock_path).ok();

        let mut config = RepinConfig::default();
        let meta_config = repin_dir.join("config.toml");
        let root_config = project_root.join("config.toml");

        if meta_config.is_file() {
            if let Ok(content) = fs::read_to_string(&meta_config) {
                let _ = config.merge_toml_str(&content);
            }
        } else if root_config.is_file()
            && let Ok(content) = fs::read_to_string(&root_config)
        {
            let _ = config.merge_toml_str(&content);
        }

        let engine = Engine::open(EngineOptions {
            root_id: "root".to_string(),
            root_path: project_root.clone(),
            db_path: Some(canonical_db_path.clone()),
        })?;

        Ok(Self {
            canonical_db_path,
            project_root,
            engine,
            writer_lease,
            config,
        })
    }

    pub fn is_writer(&self) -> bool {
        self.writer_lease.is_some()
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    pub fn canonical_db_path(&self) -> &Path {
        &self.canonical_db_path
    }

    pub fn project_root(&self) -> &Path {
        &self.project_root
    }

    pub fn config(&self) -> &RepinConfig {
        &self.config
    }
}
