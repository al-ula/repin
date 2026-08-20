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
        // Enforce safety floor: project config cannot declare providers or API keys
        RepinConfig::validate_project_toml_str(&content)?;
        config.merge_toml_str(&content)?;
    }

    config.validate()?;

    // 3. Resolve shared provider profiles for enabled capabilities
    resolve_intelligence_providers(&mut config);

    Ok(config)
}

fn resolve_intelligence_providers(config: &mut RepinConfig) {
    let providers = &config.intelligence.providers;

    // Resolve embedding provider settings
    let emb_p = config.intelligence.embedding.provider.as_str();
    if emb_p != "none" && emb_p != "embedded" && emb_p != "agent" && !emb_p.is_empty() {
        if config.intelligence.embedding.endpoint.is_none() {
            if let Some(p) = providers.get(emb_p) {
                config.intelligence.embedding.endpoint = p.endpoint.clone();
            } else if emb_p == "openai" {
                config.intelligence.embedding.endpoint =
                    Some("https://api.openai.com/v1".to_string());
            } else if emb_p == "ollama" {
                config.intelligence.embedding.endpoint = Some("http://localhost:11434".to_string());
            } else if emb_p == "google" {
                config.intelligence.embedding.endpoint =
                    Some("https://generativelanguage.googleapis.com".to_string());
            }
        }
        if config.intelligence.embedding.api_key_env.is_none() {
            if let Some(p) = providers.get(emb_p) {
                config.intelligence.embedding.api_key_env = p.api_key_env.clone();
            } else if emb_p == "openai" {
                config.intelligence.embedding.api_key_env = Some("OPENAI_API_KEY".to_string());
            } else if emb_p == "google" {
                config.intelligence.embedding.api_key_env = Some("GEMINI_API_KEY".to_string());
            }
        }
    }

    // Resolve rerank provider settings
    let rerank_p = config.intelligence.rerank.provider.as_str();
    if rerank_p != "none" && rerank_p != "embedded" && rerank_p != "agent" && !rerank_p.is_empty() {
        if config.intelligence.rerank.endpoint.is_none() {
            if let Some(p) = providers.get(rerank_p) {
                config.intelligence.rerank.endpoint = p.endpoint.clone();
            } else if rerank_p == "openai" {
                config.intelligence.rerank.endpoint = Some("https://api.openai.com/v1".to_string());
            } else if rerank_p == "ollama" {
                config.intelligence.rerank.endpoint = Some("http://localhost:11434".to_string());
            }
        }
        if config.intelligence.rerank.api_key_env.is_none() {
            if let Some(p) = providers.get(rerank_p) {
                config.intelligence.rerank.api_key_env = p.api_key_env.clone();
            } else if rerank_p == "openai" {
                config.intelligence.rerank.api_key_env = Some("OPENAI_API_KEY".to_string());
            }
        }
    }

    // Resolve enrichment provider settings
    let enrich_p = config.intelligence.enrichment.provider.as_str();
    if enrich_p != "none" && enrich_p != "embedded" && enrich_p != "agent" && !enrich_p.is_empty() {
        if config.intelligence.enrichment.endpoint.is_none() {
            if let Some(p) = providers.get(enrich_p) {
                config.intelligence.enrichment.endpoint = p.endpoint.clone();
            } else if enrich_p == "google" {
                config.intelligence.enrichment.endpoint =
                    Some("https://generativelanguage.googleapis.com".to_string());
            } else if enrich_p == "openai" {
                config.intelligence.enrichment.endpoint =
                    Some("https://api.openai.com/v1".to_string());
            } else if enrich_p == "ollama" {
                config.intelligence.enrichment.endpoint =
                    Some("http://localhost:11434".to_string());
            }
        }
        if config.intelligence.enrichment.api_key_env.is_none() {
            if let Some(p) = providers.get(enrich_p) {
                config.intelligence.enrichment.api_key_env = p.api_key_env.clone();
            } else if enrich_p == "google" {
                config.intelligence.enrichment.api_key_env = Some("GEMINI_API_KEY".to_string());
            } else if enrich_p == "openai" {
                config.intelligence.enrichment.api_key_env = Some("OPENAI_API_KEY".to_string());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_discover_project_uninitialized_returns_none() {
        let temp = tempdir().unwrap();
        let discovered = discover_project_from(temp.path());
        assert!(discovered.is_none());
    }

    #[test]
    fn test_discover_project_initialized_returns_some() {
        let temp = tempdir().unwrap();
        let repin_dir = temp.path().join(".repin");
        fs::create_dir_all(&repin_dir).unwrap();
        fs::write(repin_dir.join("graph.sqlite3"), b"").unwrap();

        let discovered = discover_project_from(temp.path());
        assert!(discovered.is_some());
        let proj = discovered.unwrap();
        assert_eq!(proj.root_dir, temp.path());
        assert_eq!(proj.db_path, repin_dir.join("graph.sqlite3"));
    }

    #[test]
    fn test_discover_project_from_subdirectory() {
        let temp = tempdir().unwrap();
        let repin_dir = temp.path().join(".repin");
        fs::create_dir_all(&repin_dir).unwrap();
        fs::write(repin_dir.join("graph.sqlite3"), b"").unwrap();

        let sub_dir = temp.path().join("src").join("commands");
        fs::create_dir_all(&sub_dir).unwrap();

        let discovered = discover_project_from(&sub_dir);
        assert!(discovered.is_some());
        let proj = discovered.unwrap();
        assert_eq!(proj.root_dir, temp.path());
    }

    #[test]
    fn test_discover_project_without_graph_sqlite3_returns_none() {
        let temp = tempdir().unwrap();
        let repin_dir = temp.path().join(".repin");
        fs::create_dir_all(&repin_dir).unwrap();

        let discovered = discover_project_from(temp.path());
        assert!(discovered.is_none());
    }
}
