use crate::config::types::RepinConfig;

pub trait Merge {
    fn merge(&mut self, higher: Self);
}

impl Merge for RepinConfig {
    fn merge(&mut self, higher: Self) {
        if higher.schema_version != 0 {
            self.schema_version = higher.schema_version;
        }
        if higher.project.name.is_some() {
            self.project.name = higher.project.name;
        }
        if !higher.project.roots.is_empty() && higher.project.roots != vec![".".to_string()] {
            self.project.roots = higher.project.roots;
        }
        if !higher.project.languages.is_empty() {
            self.project.languages = higher.project.languages;
        }

        // Indexing: merge exclusions (union)
        for p in higher.indexing.exclude_paths {
            if !self.indexing.exclude_paths.contains(&p) {
                self.indexing.exclude_paths.push(p);
            }
        }
        for ext in higher.indexing.exclude_extensions {
            if !self.indexing.exclude_extensions.contains(&ext) {
                self.indexing.exclude_extensions.push(ext);
            }
        }
        self.indexing.max_file_size_bytes = higher.indexing.max_file_size_bytes;
        self.indexing.respect_gitignore = higher.indexing.respect_gitignore;
        self.indexing.index_docs = higher.indexing.index_docs;
        self.indexing.index_config = higher.indexing.index_config;

        self.extraction.tree_sitter_fallback = higher.extraction.tree_sitter_fallback;

        self.retrieval.default_mode = higher.retrieval.default_mode;
        self.retrieval.default_limit = higher.retrieval.default_limit;
        self.retrieval.centrality_boost = higher.retrieval.centrality_boost;
        self.retrieval.regex_size_limit_bytes = higher.retrieval.regex_size_limit_bytes;

        self.context.default_token_budget = higher.context.default_token_budget;
        self.context.padding_lines = higher.context.padding_lines;
        self.context.include_blast_radius = higher.context.include_blast_radius;
        self.context.include_verbatim_source = higher.context.include_verbatim_source;

        self.storage.wal_checkpoint_mode = higher.storage.wal_checkpoint_mode;
        self.storage.checkpoint_interval = higher.storage.checkpoint_interval;

        self.daemon.watch_debounce_ms = higher.daemon.watch_debounce_ms;
        self.daemon.idle_timeout_secs = higher.daemon.idle_timeout_secs;

        self.intelligence.lexical = higher.intelligence.lexical;
        self.intelligence.graph = higher.intelligence.graph;
        for (k, v) in higher.intelligence.providers {
            self.intelligence.providers.insert(k, v);
        }
        self.intelligence.embedding = higher.intelligence.embedding;
        self.intelligence.rerank = higher.intelligence.rerank;
        self.intelligence.enrichment = higher.intelligence.enrichment;
    }
}
