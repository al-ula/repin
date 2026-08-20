pub mod agent;
pub mod context;
pub mod engine;
pub mod eval;
pub mod inspect;
pub mod invalidation;
pub mod ranking;
pub mod traversal;
pub mod vector;

pub use agent::AgentReranker;
pub use context::{AssembledContext, ContextBuilder, ContextSnippet};
pub use engine::{Engine, EngineOptions};
pub use eval::{BenchmarkHarness, EvalReport};
pub use inspect::{FileOutline, Inspector, SymbolSummary};
pub use invalidation::{BlastRadius, InvalidationCoordinator};
pub use ranking::{DeterministicRanker, RankExplanation, RankReason, RankedCandidate};
pub use traversal::{GraphTraversal, NeighborItem, NeighborsData};
pub use vector::{ExactVectorIndex, VectorHit};

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_engine_direct_search_indexless() {
        let dir = tempdir().unwrap();
        let src_dir = dir.path().join("src");
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(
            src_dir.join("main.rs"),
            b"pub fn start_engine() {\n    println!(\"ready\");\n}\n",
        )
        .unwrap();

        let engine = Engine::open(EngineOptions {
            root_id: "root".to_string(),
            root_path: dir.path().to_path_buf(),
            db_path: None,
        })
        .unwrap();

        let res = engine.search_direct("start_engine", false, 10);
        assert_eq!(res.status, repin_protocol::envelope::Status::Ok);
        assert_eq!(res.evidence.len(), 1);
        assert_eq!(res.evidence[0].path, "src/main.rs");
    }

    #[test]
    fn test_engine_graph_indexing_and_search() {
        let dir = tempdir().unwrap();
        let src_dir = dir.path().join("src");
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(src_dir.join("lib.rs"), b"pub fn compute_sum() {}\n").unwrap();

        let db_path = dir.path().join(".repin/graph.sqlite3");
        let engine = Engine::open(EngineOptions {
            root_id: "root".to_string(),
            root_path: dir.path().to_path_buf(),
            db_path: Some(db_path),
        })
        .unwrap();

        let indexed = engine.index_all_worktree().unwrap();
        assert_eq!(indexed, 1);

        let res = engine.search_graph("compute_sum", 10);
        assert_eq!(res.status, repin_protocol::envelope::Status::Ok);
        assert_eq!(res.data.len(), 1);
        assert_eq!(res.data[0].node.name, "compute_sum");

        let outline = engine.inspect_file("src/lib.rs");
        assert_eq!(outline.data.symbols.len(), 1);
        assert_eq!(outline.data.symbols[0].name, "compute_sum");
    }
}
