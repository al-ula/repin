use crate::client::DaemonClient;
use repin_core::protocol::ipc::{IpcRequest, IpcResponse};
use repin_core::runtime::eval::EvalReport;

pub fn execute_eval(client: &mut DaemonClient) -> Result<(), String> {
    println!("Running Precision-at-N Benchmark Evaluation on indexed graph...");
    let resp = client.send_request(IpcRequest::Eval)?;

    match resp {
        IpcResponse::EvalResult(env) => {
            let report: EvalReport =
                serde_json::from_value(env.data).unwrap_or_else(|_| EvalReport {
                    total_queries: 0,
                    precision_at_1: 0.0,
                    precision_at_5: 0.0,
                    mean_reciprocal_rank: 0.0,
                    query_results: Vec::new(),
                });

            println!("\n=== Precision-at-N Retrieval Evaluation ===");
            println!("Total Benchmark Queries: {}", report.total_queries);
            println!(
                "Precision@1:            {:.2}%",
                report.precision_at_1 * 100.0
            );
            println!(
                "Precision@5:            {:.2}%",
                report.precision_at_5 * 100.0
            );
            println!("Mean Reciprocal Rank:   {:.4}", report.mean_reciprocal_rank);
            println!("\nQuery Breakdown:");
            for q in &report.query_results {
                let rank_str = q
                    .found_rank
                    .map(|r| r.to_string())
                    .unwrap_or_else(|| "Not found".to_string());
                println!(
                    "  Query \"{}\" -> Expected '{}' @ Rank {}",
                    q.query, q.expected_symbol, rank_str
                );
            }
            Ok(())
        }
        IpcResponse::Error { code, message } => {
            Err(format!("Evaluation failed: {:?}: {}", code, message))
        }
        _ => Err("Unexpected response".to_string()),
    }
}
