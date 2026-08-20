use crate::engine::Engine;
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

pub struct BenchmarkHarness;

impl BenchmarkHarness {
    pub fn evaluate_engine(engine: &Engine) -> EvalReport {
        // Collect known symbols from store if available
        let mut test_queries = Vec::new();

        if let Some(store) = engine.store()
            && let Ok(view) = store.read_view()
        {
            // Sample some symbols
            let sample_names = [
                "main",
                "Engine",
                "SqliteStore",
                "CapabilityFs",
                "LineIndex",
                "LanguagePack",
                "NodeId",
                "EdgeId",
            ];
            for name in sample_names {
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
            let res = engine.search_graph(query, 10);
            let mut found_rank = None;

            for (idx, item) in res.data.iter().enumerate() {
                if item.node.name.eq_ignore_ascii_case(expected) {
                    found_rank = Some(idx + 1);
                    break;
                }
            }

            let (hit1, hit5, rr) = match found_rank {
                Some(1) => {
                    p1_count += 1;
                    p5_count += 1;
                    mrr_sum += 1.0;
                    (true, true, 1.0)
                }
                Some(r) if r <= 5 => {
                    p5_count += 1;
                    let rr = 1.0 / (r as f64);
                    mrr_sum += rr;
                    (false, true, rr)
                }
                Some(r) => {
                    let rr = 1.0 / (r as f64);
                    mrr_sum += rr;
                    (false, false, rr)
                }
                None => (false, false, 0.0),
            };

            results.push(QueryEvalResult {
                query: query.clone(),
                expected_symbol: expected.clone(),
                found_rank,
                hit_at_1: hit1,
                hit_at_5: hit5,
                reciprocal_rank: rr,
            });
        }

        let total = test_queries.len();
        EvalReport {
            total_queries: total,
            precision_at_1: if total > 0 {
                p1_count as f64 / total as f64
            } else {
                0.0
            },
            precision_at_5: if total > 0 {
                p5_count as f64 / total as f64
            } else {
                0.0
            },
            mean_reciprocal_rank: if total > 0 {
                mrr_sum / total as f64
            } else {
                0.0
            },
            query_results: results,
        }
    }
}
