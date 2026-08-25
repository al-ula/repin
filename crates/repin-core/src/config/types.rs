use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default = "default_roots")]
    pub roots: Vec<String>,
    #[serde(default)]
    pub languages: Vec<String>,
}

fn default_roots() -> Vec<String> {
    vec![".".to_string()]
}

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            name: None,
            roots: default_roots(),
            languages: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexingConfig {
    #[serde(default)]
    pub exclude_paths: Vec<String>,
    #[serde(default)]
    pub exclude_extensions: Vec<String>,
    #[serde(default = "default_max_file_size_bytes")]
    pub max_file_size_bytes: usize,
    #[serde(default = "default_true")]
    pub respect_gitignore: bool,
    #[serde(default = "default_true")]
    pub index_docs: bool,
    #[serde(default = "default_true")]
    pub index_config: bool,
}

fn default_max_file_size_bytes() -> usize {
    2 * 1024 * 1024 // 2 MB
}

fn default_true() -> bool {
    true
}

impl Default for IndexingConfig {
    fn default() -> Self {
        Self {
            exclude_paths: Vec::new(),
            exclude_extensions: Vec::new(),
            max_file_size_bytes: default_max_file_size_bytes(),
            respect_gitignore: true,
            index_docs: true,
            index_config: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractionConfig {
    #[serde(default = "default_true")]
    pub tree_sitter_fallback: bool,
}

impl Default for ExtractionConfig {
    fn default() -> Self {
        Self {
            tree_sitter_fallback: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetrievalConfig {
    #[serde(default = "default_mode")]
    pub default_mode: String,
    #[serde(default = "default_limit")]
    pub default_limit: usize,
    #[serde(default = "default_centrality_boost")]
    pub centrality_boost: f64,
    #[serde(default = "default_regex_size_limit_bytes")]
    pub regex_size_limit_bytes: usize,
}

fn default_mode() -> String {
    "hybrid".to_string()
}

fn default_limit() -> usize {
    50
}

fn default_centrality_boost() -> f64 {
    0.15
}

fn default_regex_size_limit_bytes() -> usize {
    10 * 1024 * 1024 // 10 MB
}

impl Default for RetrievalConfig {
    fn default() -> Self {
        Self {
            default_mode: default_mode(),
            default_limit: default_limit(),
            centrality_boost: default_centrality_boost(),
            regex_size_limit_bytes: default_regex_size_limit_bytes(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextConfig {
    #[serde(default = "default_token_budget")]
    pub default_token_budget: usize,
    #[serde(default = "default_padding_lines")]
    pub padding_lines: usize,
    #[serde(default = "default_true")]
    pub include_blast_radius: bool,
    #[serde(default = "default_true")]
    pub include_verbatim_source: bool,
}

fn default_token_budget() -> usize {
    8192
}

fn default_padding_lines() -> usize {
    2
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            default_token_budget: default_token_budget(),
            padding_lines: default_padding_lines(),
            include_blast_radius: true,
            include_verbatim_source: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageConfig {
    #[serde(default = "default_wal_checkpoint_mode")]
    pub wal_checkpoint_mode: String,
    #[serde(default = "default_checkpoint_interval")]
    pub checkpoint_interval: usize,
}

fn default_wal_checkpoint_mode() -> String {
    "truncate".to_string()
}

fn default_checkpoint_interval() -> usize {
    1000
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            wal_checkpoint_mode: default_wal_checkpoint_mode(),
            checkpoint_interval: default_checkpoint_interval(),
        }
    }
}

pub const DEFAULT_IDLE_TIMEOUT_SECS: u64 = 600;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DaemonConfig {
    #[serde(default = "default_watch_debounce_ms")]
    pub watch_debounce_ms: u64,
    #[serde(default = "default_idle_timeout_secs")]
    pub idle_timeout_secs: u64,
}

fn default_watch_debounce_ms() -> u64 {
    150
}

fn default_idle_timeout_secs() -> u64 {
    DEFAULT_IDLE_TIMEOUT_SECS
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            watch_debounce_ms: default_watch_debounce_ms(),
            idle_timeout_secs: default_idle_timeout_secs(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimpleCapabilityConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl Default for SimpleCapabilityConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_env: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    #[serde(default = "default_provider_none")]
    pub provider: String,
    #[serde(default = "default_embedding_model")]
    pub model: String,
    #[serde(default = "default_embedding_dimension")]
    pub dimension: Option<usize>,
    #[serde(default = "default_true")]
    pub auto_download: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_env: Option<String>,
}

fn default_provider_none() -> String {
    "none".to_string()
}

fn default_embedding_model() -> String {
    "Alibaba-NLP/gte-modernbert-base".to_string()
}

fn default_embedding_dimension() -> Option<usize> {
    Some(256)
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            provider: default_provider_none(),
            model: default_embedding_model(),
            dimension: default_embedding_dimension(),
            auto_download: true,
            endpoint: None,
            api_key_env: None,
        }
    }
}

impl EmbeddingConfig {
    pub fn is_enabled(&self) -> bool {
        !self.provider.is_empty() && self.provider != "none"
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RerankConfig {
    #[serde(default = "default_provider_none")]
    pub provider: String,
    #[serde(default = "default_rerank_model")]
    pub model: String,
    #[serde(default = "default_rerank_top_n")]
    pub top_n: usize,
    #[serde(default = "default_rerank_deadline_ms")]
    pub deadline_ms: u64,
    #[serde(default)]
    pub agent_cmd: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_env: Option<String>,
}

fn default_rerank_model() -> String {
    "Alibaba-NLP/gte-reranker-modernbert-base".to_string()
}

fn default_rerank_top_n() -> usize {
    50
}

fn default_rerank_deadline_ms() -> u64 {
    100
}

impl Default for RerankConfig {
    fn default() -> Self {
        Self {
            provider: default_provider_none(),
            model: default_rerank_model(),
            top_n: default_rerank_top_n(),
            deadline_ms: default_rerank_deadline_ms(),
            agent_cmd: String::new(),
            endpoint: None,
            api_key_env: None,
        }
    }
}

impl RerankConfig {
    pub fn is_enabled(&self) -> bool {
        !self.provider.is_empty() && self.provider != "none"
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnrichmentConfig {
    #[serde(default = "default_provider_none")]
    pub provider: String,
    #[serde(default = "default_enrichment_model")]
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_env: Option<String>,
}

fn default_enrichment_model() -> String {
    "gemini-2.5-flash".to_string()
}

impl Default for EnrichmentConfig {
    fn default() -> Self {
        Self {
            provider: default_provider_none(),
            model: default_enrichment_model(),
            endpoint: None,
            api_key_env: None,
        }
    }
}

impl EnrichmentConfig {
    pub fn is_enabled(&self) -> bool {
        !self.provider.is_empty() && self.provider != "none"
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntelligenceConfig {
    #[serde(default)]
    pub lexical: SimpleCapabilityConfig,
    #[serde(default)]
    pub graph: SimpleCapabilityConfig,
    #[serde(default)]
    pub providers: HashMap<String, ProviderProfile>,
    #[serde(default)]
    pub embedding: EmbeddingConfig,
    #[serde(default)]
    pub rerank: RerankConfig,
    #[serde(default)]
    pub enrichment: EnrichmentConfig,
}

impl Default for IntelligenceConfig {
    fn default() -> Self {
        Self {
            lexical: SimpleCapabilityConfig { enabled: true },
            graph: SimpleCapabilityConfig { enabled: true },
            providers: HashMap::new(),
            embedding: EmbeddingConfig::default(),
            rerank: RerankConfig::default(),
            enrichment: EnrichmentConfig::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RepinConfig {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub project: ProjectConfig,
    #[serde(default)]
    pub indexing: IndexingConfig,
    #[serde(default)]
    pub extraction: ExtractionConfig,
    #[serde(default)]
    pub retrieval: RetrievalConfig,
    #[serde(default)]
    pub context: ContextConfig,
    #[serde(default)]
    pub storage: StorageConfig,
    #[serde(default)]
    pub daemon: DaemonConfig,
    #[serde(default)]
    pub intelligence: IntelligenceConfig,
}

fn default_schema_version() -> u32 {
    1
}

impl Default for RepinConfig {
    fn default() -> Self {
        Self {
            schema_version: default_schema_version(),
            project: ProjectConfig::default(),
            indexing: IndexingConfig::default(),
            extraction: ExtractionConfig::default(),
            retrieval: RetrievalConfig::default(),
            context: ContextConfig::default(),
            storage: StorageConfig::default(),
            daemon: DaemonConfig::default(),
            intelligence: IntelligenceConfig::default(),
        }
    }
}
