use crate::config::types::{
    ContextConfig, DaemonConfig, ExtractionConfig, IndexingConfig, IntelligenceConfig,
    ProjectConfig, RepinConfig, RetrievalConfig, RerankCapabilityConfig, SemanticCapabilityConfig,
    SimpleCapabilityConfig, StorageConfig,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PartialProjectConfig {
    pub name: Option<String>,
    pub roots: Option<Vec<String>>,
    pub languages: Option<Vec<String>>,
}

impl PartialProjectConfig {
    pub fn apply_to(&self, target: &mut ProjectConfig) {
        if let Some(name) = &self.name {
            target.name = Some(name.clone());
        }
        if let Some(roots) = &self.roots {
            target.roots = roots.clone();
        }
        if let Some(languages) = &self.languages {
            target.languages = languages.clone();
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartialIndexingConfig {
    pub exclude_paths: Option<Vec<String>>,
    pub exclude_extensions: Option<Vec<String>>,
    pub max_file_size_bytes: Option<usize>,
    pub respect_gitignore: Option<bool>,
    pub index_docs: Option<bool>,
    pub index_config: Option<bool>,
}

impl PartialIndexingConfig {
    pub fn apply_to(&self, target: &mut IndexingConfig) {
        if let Some(exclude_paths) = &self.exclude_paths {
            for p in exclude_paths {
                if !target.exclude_paths.contains(p) {
                    target.exclude_paths.push(p.clone());
                }
            }
        }
        if let Some(exclude_extensions) = &self.exclude_extensions {
            for ext in exclude_extensions {
                if !target.exclude_extensions.contains(ext) {
                    target.exclude_extensions.push(ext.clone());
                }
            }
        }
        if let Some(max_file_size_bytes) = self.max_file_size_bytes {
            target.max_file_size_bytes = max_file_size_bytes;
        }
        if let Some(respect_gitignore) = self.respect_gitignore {
            target.respect_gitignore = respect_gitignore;
        }
        if let Some(index_docs) = self.index_docs {
            target.index_docs = index_docs;
        }
        if let Some(index_config) = self.index_config {
            target.index_config = index_config;
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartialExtractionConfig {
    pub tree_sitter_fallback: Option<bool>,
}

impl PartialExtractionConfig {
    pub fn apply_to(&self, target: &mut ExtractionConfig) {
        if let Some(tree_sitter_fallback) = self.tree_sitter_fallback {
            target.tree_sitter_fallback = tree_sitter_fallback;
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PartialRetrievalConfig {
    pub default_mode: Option<String>,
    pub default_limit: Option<usize>,
    pub centrality_boost: Option<f64>,
    pub regex_size_limit_bytes: Option<usize>,
}

impl PartialRetrievalConfig {
    pub fn apply_to(&self, target: &mut RetrievalConfig) {
        if let Some(default_mode) = &self.default_mode {
            target.default_mode = default_mode.clone();
        }
        if let Some(default_limit) = self.default_limit {
            target.default_limit = default_limit;
        }
        if let Some(centrality_boost) = self.centrality_boost {
            target.centrality_boost = centrality_boost;
        }
        if let Some(regex_size_limit_bytes) = self.regex_size_limit_bytes {
            target.regex_size_limit_bytes = regex_size_limit_bytes;
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartialContextConfig {
    pub default_token_budget: Option<usize>,
    pub padding_lines: Option<usize>,
    pub include_blast_radius: Option<bool>,
    pub include_verbatim_source: Option<bool>,
}

impl PartialContextConfig {
    pub fn apply_to(&self, target: &mut ContextConfig) {
        if let Some(budget) = self.default_token_budget {
            target.default_token_budget = budget;
        }
        if let Some(padding) = self.padding_lines {
            target.padding_lines = padding;
        }
        if let Some(blast) = self.include_blast_radius {
            target.include_blast_radius = blast;
        }
        if let Some(verbatim) = self.include_verbatim_source {
            target.include_verbatim_source = verbatim;
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartialStorageConfig {
    pub wal_checkpoint_mode: Option<String>,
    pub checkpoint_interval: Option<usize>,
}

impl PartialStorageConfig {
    pub fn apply_to(&self, target: &mut StorageConfig) {
        if let Some(mode) = &self.wal_checkpoint_mode {
            target.wal_checkpoint_mode = mode.clone();
        }
        if let Some(interval) = self.checkpoint_interval {
            target.checkpoint_interval = interval;
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartialDaemonConfig {
    pub watch_debounce_ms: Option<u64>,
    pub idle_timeout_secs: Option<u64>,
}

impl PartialDaemonConfig {
    pub fn apply_to(&self, target: &mut DaemonConfig) {
        if let Some(debounce) = self.watch_debounce_ms {
            target.watch_debounce_ms = debounce;
        }
        if let Some(timeout) = self.idle_timeout_secs {
            target.idle_timeout_secs = timeout;
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartialSimpleCapabilityConfig {
    pub enabled: Option<bool>,
}

impl PartialSimpleCapabilityConfig {
    pub fn apply_to(&self, target: &mut SimpleCapabilityConfig) {
        if let Some(enabled) = self.enabled {
            target.enabled = enabled;
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartialSemanticCapabilityConfig {
    pub enabled: Option<bool>,
    pub provider: Option<String>,
}

impl PartialSemanticCapabilityConfig {
    pub fn apply_to(&self, target: &mut SemanticCapabilityConfig) {
        if let Some(enabled) = self.enabled {
            target.enabled = enabled;
        }
        if let Some(provider) = &self.provider {
            target.provider = provider.clone();
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartialRerankCapabilityConfig {
    pub enabled: Option<bool>,
    pub agent_cmd: Option<String>,
}

impl PartialRerankCapabilityConfig {
    pub fn apply_to(&self, target: &mut RerankCapabilityConfig) {
        if let Some(enabled) = self.enabled {
            target.enabled = enabled;
        }
        if let Some(cmd) = &self.agent_cmd {
            target.agent_cmd = cmd.clone();
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartialIntelligenceConfig {
    pub lexical: Option<PartialSimpleCapabilityConfig>,
    pub graph: Option<PartialSimpleCapabilityConfig>,
    pub semantic: Option<PartialSemanticCapabilityConfig>,
    pub rerank: Option<PartialRerankCapabilityConfig>,
}

impl PartialIntelligenceConfig {
    pub fn apply_to(&self, target: &mut IntelligenceConfig) {
        if let Some(lexical) = &self.lexical {
            lexical.apply_to(&mut target.lexical);
        }
        if let Some(graph) = &self.graph {
            graph.apply_to(&mut target.graph);
        }
        if let Some(semantic) = &self.semantic {
            semantic.apply_to(&mut target.semantic);
        }
        if let Some(rerank) = &self.rerank {
            rerank.apply_to(&mut target.rerank);
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PartialRepinConfig {
    pub schema_version: Option<u32>,
    pub project: Option<PartialProjectConfig>,
    pub indexing: Option<PartialIndexingConfig>,
    pub extraction: Option<PartialExtractionConfig>,
    pub retrieval: Option<PartialRetrievalConfig>,
    pub context: Option<PartialContextConfig>,
    pub storage: Option<PartialStorageConfig>,
    pub daemon: Option<PartialDaemonConfig>,
    pub intelligence: Option<PartialIntelligenceConfig>,
}

impl PartialRepinConfig {
    pub fn apply_to(&self, target: &mut RepinConfig) {
        if let Some(version) = self.schema_version {
            target.schema_version = version;
        }
        if let Some(project) = &self.project {
            project.apply_to(&mut target.project);
        }
        if let Some(indexing) = &self.indexing {
            indexing.apply_to(&mut target.indexing);
        }
        if let Some(extraction) = &self.extraction {
            extraction.apply_to(&mut target.extraction);
        }
        if let Some(retrieval) = &self.retrieval {
            retrieval.apply_to(&mut target.retrieval);
        }
        if let Some(context) = &self.context {
            context.apply_to(&mut target.context);
        }
        if let Some(storage) = &self.storage {
            storage.apply_to(&mut target.storage);
        }
        if let Some(daemon) = &self.daemon {
            daemon.apply_to(&mut target.daemon);
        }
        if let Some(intelligence) = &self.intelligence {
            intelligence.apply_to(&mut target.intelligence);
        }
    }
}
