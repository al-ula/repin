use crate::discovery::{find_project_config_path, load_effective_config};
use anyhow::{Context, Result};
use repin_core::config::RepinConfig;
use std::fs;
use std::path::PathBuf;

pub fn execute_config_init(project_path: Option<PathBuf>, force: bool) -> Result<()> {
    let root_dir = project_path
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    let repin_dir = root_dir.join(".repin");
    if !repin_dir.exists() {
        fs::create_dir_all(&repin_dir)
            .with_context(|| format!("failed to create directory {:?}", repin_dir))?;
    }

    let target_file = repin_dir.join("config.toml");
    if target_file.exists() && !force {
        eprintln!(
            "Configuration file already exists at {:?}. Use --force to overwrite.",
            target_file
        );
        return Ok(());
    }

    let template = RepinConfig::starter_template();
    fs::write(&target_file, template)
        .with_context(|| format!("failed to write configuration to {:?}", target_file))?;

    println!("Initialized Repin configuration at {:?}", target_file);
    Ok(())
}

pub fn execute_config_show(
    project_path: Option<PathBuf>,
    explicit_config: Option<PathBuf>,
) -> Result<()> {
    let root_dir = project_path
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    let resolved_config = load_effective_config(&root_dir, explicit_config.as_deref())
        .map_err(|e| anyhow::anyhow!("Configuration error: {}", e))?;

    let toml_str = resolved_config.to_toml_string()
        .map_err(|e| anyhow::anyhow!("Serialization error: {}", e))?;

    println!("{}", toml_str);
    Ok(())
}

pub fn execute_config_validate(
    project_path: Option<PathBuf>,
    explicit_config: Option<PathBuf>,
) -> Result<()> {
    let root_dir = project_path
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    let config_path = explicit_config
        .clone()
        .or_else(|| find_project_config_path(&root_dir));

    match load_effective_config(&root_dir, explicit_config.as_deref()) {
        Ok(config) => {
            if let Some(path) = config_path {
                println!("✓ Configuration at {:?} is valid (schema_version = {}).", path, config.schema_version);
            } else {
                println!("✓ No project config.toml found; built-in defaults are valid (schema_version = {}).", config.schema_version);
            }
            Ok(())
        }
        Err(err) => {
            eprintln!("✗ Configuration validation failed: {}", err);
            Err(anyhow::anyhow!("Configuration validation failed: {}", err))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_config_init_creates_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().to_path_buf();

        execute_config_init(Some(path.clone()), false).expect("init should succeed");
        let config_file = path.join(".repin").join("config.toml");
        assert!(config_file.is_file());

        let content = fs::read_to_string(&config_file).unwrap();
        assert!(content.contains("schema_version = 1"));
        assert!(content.contains("[indexing]"));
    }

    #[test]
    fn test_config_validate_valid_and_invalid() {
        let dir = tempdir().unwrap();
        let path = dir.path().to_path_buf();

        execute_config_init(Some(path.clone()), false).expect("init should succeed");
        assert!(execute_config_validate(Some(path.clone()), None).is_ok());

        // Write invalid schema version
        let config_file = path.join(".repin").join("config.toml");
        fs::write(&config_file, "schema_version = 999").unwrap();
        assert!(execute_config_validate(Some(path), None).is_err());
    }
}
