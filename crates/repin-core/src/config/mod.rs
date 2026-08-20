pub mod merger;
pub mod partial;
pub mod types;

pub use merger::Merge;
pub use partial::*;
pub use types::*;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to parse TOML configuration: {0}")]
    ParseError(#[from] toml::de::Error),
    #[error("failed to serialize TOML configuration: {0}")]
    SerializeError(#[from] toml::ser::Error),
    #[error("unsupported schema version {found}, expected {expected}")]
    UnsupportedSchemaVersion { found: u32, expected: u32 },
    #[error("I/O error reading configuration: {0}")]
    IoError(#[from] std::io::Error),
    #[error("validation error: {0}")]
    ValidationError(String),
}

impl RepinConfig {
    /// Parse a complete or partial TOML string into a merged `RepinConfig`.
    pub fn from_toml_str(content: &str) -> Result<Self, ConfigError> {
        let mut config = Self::default();
        config.merge_toml_str(content)?;
        Ok(config)
    }

    /// Merge a partial or complete TOML string into this `RepinConfig` instance.
    pub fn merge_toml_str(&mut self, content: &str) -> Result<(), ConfigError> {
        let partial: PartialRepinConfig = toml::from_str(content)?;
        if let Some(version) = partial.schema_version
            && version != 1
        {
            return Err(ConfigError::UnsupportedSchemaVersion {
                found: version,
                expected: 1,
            });
        }
        partial.apply_to(self);
        self.validate()?;
        Ok(())
    }

    /// Serialize the active configuration to a formatted TOML string.
    pub fn to_toml_string(&self) -> Result<String, ConfigError> {
        Ok(toml::to_string_pretty(self)?)
    }

    /// Validate configuration constraints.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.schema_version != 1 {
            return Err(ConfigError::UnsupportedSchemaVersion {
                found: self.schema_version,
                expected: 1,
            });
        }
        if self.project.roots.is_empty() {
            return Err(ConfigError::ValidationError(
                "project roots list cannot be empty".to_string(),
            ));
        }
        for root in &self.project.roots {
            if root.starts_with("../") || root.contains("/../") {
                return Err(ConfigError::ValidationError(format!(
                    "project root '{}' escapes the repository boundary",
                    root
                )));
            }
        }
        if self.retrieval.regex_size_limit_bytes == 0 {
            return Err(ConfigError::ValidationError(
                "retrieval.regex_size_limit_bytes must be greater than 0".to_string(),
            ));
        }
        if self.indexing.max_file_size_bytes == 0 {
            return Err(ConfigError::ValidationError(
                "indexing.max_file_size_bytes must be greater than 0".to_string(),
            ));
        }
        let valid_modes = ["hybrid", "graph", "direct", "regex"];
        if !valid_modes.contains(&self.retrieval.default_mode.as_str()) {
            return Err(ConfigError::ValidationError(format!(
                "invalid retrieval.default_mode '{}', expected one of: {:?}",
                self.retrieval.default_mode, valid_modes
            )));
        }
        let valid_wal_modes = ["truncate", "passive", "full"];
        if !valid_wal_modes.contains(&self.storage.wal_checkpoint_mode.as_str()) {
            return Err(ConfigError::ValidationError(format!(
                "invalid storage.wal_checkpoint_mode '{}', expected one of: {:?}",
                self.storage.wal_checkpoint_mode, valid_wal_modes
            )));
        }
        Ok(())
    }

    /// Return a well-commented starter configuration template.
    pub fn starter_template() -> &'static str {
        r#"# Repin Repository Intelligence Configuration
schema_version = 1

[project]
# name = "my-project"
roots = ["."]
# languages = ["rust", "typescript"]

[indexing]
# Additional file or folder glob patterns to exclude
exclude_paths = [
  "**/build/**",
  "**/dist/**",
  "vendor/**"
]
# File extensions to ignore (without leading dot)
exclude_extensions = ["min.js", "bundle.js"]
# Maximum file size to index in bytes (default 2 MB)
max_file_size_bytes = 2097152
# Respect .gitignore rules
respect_gitignore = true
# Index markdown and documentation into the graph & lexical index
index_docs = true
# Extract configuration files into config_key entities
index_config = true

[extraction]
# Fallback to Tree-sitter when native grammars are unavailable
tree_sitter_fallback = true

[retrieval]
# Default search mode: 'hybrid' | 'graph' | 'direct' | 'regex'
default_mode = "hybrid"
# Default candidate limit
default_limit = 50
# Graph degree centrality signal weight in deterministic rank fusion
centrality_boost = 0.15
# Direct regex search compiled size limit in bytes (10 MB)
regex_size_limit_bytes = 10485760

[context]
# Default context packing token budget
default_token_budget = 8192
# Padding lines around symbol definitions in verbatim snippets
padding_lines = 2
# Include blast radius (caller & dependency counts) in output
include_blast_radius = true
# Include verbatim source code snippets in neighborhood context
include_verbatim_source = true

[storage]
# SQLite WAL checkpoint mode: 'truncate' | 'passive' | 'full'
wal_checkpoint_mode = "truncate"
# Batch update interval before auto-checkpointing
checkpoint_interval = 1000

[daemon]
# File watcher debounce delay in milliseconds
watch_debounce_ms = 150
# Idle daemon context timeout in seconds (0 = persistent)
idle_timeout_secs = 3600

[intelligence.lexical]
enabled = true

[intelligence.graph]
enabled = true

[intelligence.semantic]
enabled = false
provider = ""

[intelligence.rerank]
enabled = false
agent_cmd = ""
"#
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_is_valid() {
        let config = RepinConfig::default();
        assert_eq!(config.schema_version, 1);
        assert_eq!(config.retrieval.default_mode, "hybrid");
        assert_eq!(config.context.default_token_budget, 8192);
        assert_eq!(config.storage.wal_checkpoint_mode, "truncate");
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_partial_toml_merging() {
        let toml_str = r#"
            [indexing]
            exclude_paths = ["custom/**"]
            max_file_size_bytes = 1048576

            [retrieval]
            default_limit = 20
        "#;

        let config = RepinConfig::from_toml_str(toml_str).expect("parse should succeed");
        assert_eq!(config.indexing.exclude_paths, vec!["custom/**"]);
        assert_eq!(config.indexing.max_file_size_bytes, 1048576);
        assert_eq!(config.retrieval.default_limit, 20);
        // Defaults preserved
        assert_eq!(config.retrieval.default_mode, "hybrid");
        assert_eq!(config.context.default_token_budget, 8192);
        assert_eq!(config.storage.wal_checkpoint_mode, "truncate");
    }

    #[test]
    fn test_invalid_schema_version() {
        let toml_str = "schema_version = 99";
        let err = RepinConfig::from_toml_str(toml_str).unwrap_err();
        match err {
            ConfigError::UnsupportedSchemaVersion { found, expected } => {
                assert_eq!(found, 99);
                assert_eq!(expected, 1);
            }
            _ => panic!("unexpected error type: {:?}", err),
        }
    }

    #[test]
    fn test_root_traversal_validation_fails() {
        let toml_str = r#"
            [project]
            roots = ["../../outside"]
        "#;
        let err = RepinConfig::from_toml_str(toml_str).unwrap_err();
        match err {
            ConfigError::ValidationError(msg) => {
                assert!(msg.contains("escapes the repository boundary"));
            }
            _ => panic!("unexpected error type: {:?}", err),
        }
    }

    #[test]
    fn test_starter_template_is_valid() {
        let template = RepinConfig::starter_template();
        let config = RepinConfig::from_toml_str(template).expect("starter template should be valid");
        assert_eq!(config.schema_version, 1);
        assert_eq!(config.indexing.exclude_paths, vec!["**/build/**", "**/dist/**", "vendor/**"]);
    }
}
