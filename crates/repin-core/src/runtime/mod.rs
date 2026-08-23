//! Repin's default in-process composition root.
//!
//! `Runtime` owns concrete adapter selection and high-level result
//! normalization. The reusable capability modules remain independently usable;
//! this module is the convenience composition used by the product and by
//! embedded callers that want Repin's defaults.

pub mod agent;
pub mod engine;
pub mod eval;
pub mod inspect;
pub mod intelligence;

pub mod context {
    pub use crate::context::*;
}
pub mod invalidation {
    pub use crate::indexing::{BlastRadius, IndexingCoordinator, InvalidationScope};
    pub type InvalidationCoordinator = IndexingCoordinator;
}
pub mod ranking {
    pub use crate::retrieval::ranking::*;
}
pub mod traversal {
    pub use crate::retrieval::traversal::*;
}
pub mod vector {
    pub use crate::retrieval::vector::*;
}

pub use agent::AgentReranker;
pub use context::{AssembledContext, ContextBuilder, ContextSnippet};
pub use engine::{Runtime, RuntimeOptions};
pub use eval::{BenchmarkHarness, EvalReport};
pub use inspect::{FileOutline, Inspector, SymbolSummary};
pub use intelligence::{
    AgentRunnerReranker, EmbeddedOnnxModel, EmbeddedOnnxReranker, GoogleGeminiProvider,
    IntelligenceRegistry, OllamaProvider, OpenAiProvider, ensure_hf_model_assets,
    list_cached_models, normalize_l2,
};
pub use invalidation::{BlastRadius, InvalidationCoordinator};
pub use ranking::{DeterministicRanker, RankExplanation, RankReason, RankedCandidate};
pub use traversal::{
    GraphTraversal, ImpactData, ImpactItem, NeighborItem, NeighborsData, PathSegment, PathTrace,
    PathTraceData,
};
pub use vector::{ExactVectorIndex, VectorHit};

/// Descriptive name for the default composition root. `Engine` remains the
/// concrete compatibility name in the runtime module.
pub type Engine = Runtime;
pub type EngineOptions = RuntimeOptions;

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn direct_retrieval_survives_store_initialization_failure() {
        let directory = tempdir().unwrap();
        fs::write(directory.path().join("main.rs"), "fn answer() {}\n").unwrap();
        let blocked_parent = directory.path().join("blocked");
        fs::write(&blocked_parent, "not a directory").unwrap();

        let runtime = Runtime::open(RuntimeOptions {
            root_id: "root".to_string(),
            root_path: directory.path().to_path_buf(),
            db_path: Some(blocked_parent.join("state.sqlite3")),
        })
        .unwrap();
        let result = runtime.search_direct("answer", false, 10);
        assert_eq!(result.evidence.len(), 1);
        assert!(runtime.store().is_none());
    }
}
