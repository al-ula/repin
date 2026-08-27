use super::engine::Runtime;
use repin_core::ports::store::Store;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueryEvalResult {
    pub query: String,
    pub expected_symbol: String,
    pub found_rank: Option<usize>,
    pub hit_at_1: bool,
    pub hit_at_5: bool,
    pub reciprocal_rank: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvalReport {
    pub total_queries: usize,
    pub precision_at_1: f64,
    pub precision_at_5: f64,
    pub mean_reciprocal_rank: f64,
    pub query_results: Vec<QueryEvalResult>,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct BenchmarkHarness;

impl BenchmarkHarness {
    pub fn evaluate_engine(engine: &Runtime) -> EvalReport {
        let mut test_queries = Vec::new();
        if let Some(store) = engine.store()
            && let Ok(view) = store.read_view()
        {
            for name in [
                "main",
                "Engine",
                "SqliteStore",
                "CapabilityFs",
                "LineIndex",
                "LanguagePack",
                "NodeId",
                "EdgeId",
            ] {
                if let Ok(nodes) = view.nodes_by_name(name, &Default::default())
                    && !nodes.is_empty()
                {
                    test_queries.push((name.to_string(), name.to_string()));
                }
            }
        }
        if test_queries.is_empty() {
            test_queries.push(("main".to_string(), "main".to_string()));
        }

        let mut results = Vec::new();
        let mut p1_count = 0;
        let mut p5_count = 0;
        let mut mrr_sum = 0.0;
        for (query, expected) in &test_queries {
            let response = engine.search_graph(query, 10);
            let found_rank = response
                .data
                .iter()
                .position(|item| item.node.name.eq_ignore_ascii_case(expected))
                .map(|index| index + 1);
            let (hit_at_1, hit_at_5, reciprocal_rank) = match found_rank {
                Some(1) => {
                    p1_count += 1;
                    p5_count += 1;
                    mrr_sum += 1.0;
                    (true, true, 1.0)
                }
                Some(rank) if rank <= 5 => {
                    p5_count += 1;
                    let reciprocal_rank = 1.0 / rank as f64;
                    mrr_sum += reciprocal_rank;
                    (false, true, reciprocal_rank)
                }
                Some(rank) => {
                    let reciprocal_rank = 1.0 / rank as f64;
                    mrr_sum += reciprocal_rank;
                    (false, false, reciprocal_rank)
                }
                None => (false, false, 0.0),
            };
            results.push(QueryEvalResult {
                query: query.clone(),
                expected_symbol: expected.clone(),
                found_rank,
                hit_at_1,
                hit_at_5,
                reciprocal_rank,
            });
        }
        let total = test_queries.len();
        EvalReport {
            total_queries: total,
            precision_at_1: p1_count as f64 / total as f64,
            precision_at_5: p5_count as f64 / total as f64,
            mean_reciprocal_rank: mrr_sum / total as f64,
            query_results: results,
        }
    }
}
