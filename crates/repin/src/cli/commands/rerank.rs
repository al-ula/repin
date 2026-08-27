use crate::cli::client::DaemonClient;
use repin_core::protocol::ipc::{IpcRequest, IpcResponse};
use repin_runtime::ranking::RankedCandidate;

pub fn execute_rerank(
    client: &mut DaemonClient,
    query: &str,
    candidates: Vec<String>,
    agent_cmd: &str,
    top_n: Option<usize>,
    deadline_ms: Option<u64>,
) -> Result<(), String> {
    let trimmed_cmd = agent_cmd.trim();
    if trimmed_cmd.is_empty() {
        return Err(
            "An agent callback command is required for reranking. Provide one via --agent-cmd (e.g. --agent-cmd \"agy -p\" or --agent-cmd \"my_script\")."
                .to_string(),
        );
    }

    if candidates.is_empty() {
        println!(
            "Auto-retrieving top candidate symbols for query: \"{}\" and reranking via shell callback `{}`...",
            query, trimmed_cmd
        );
    } else {
        println!(
            "Reranking {} explicit candidate(s) for query: \"{}\" via shell callback `{}`...",
            candidates.len(),
            query,
            trimmed_cmd
        );
    }

    let resp = client.send_request(IpcRequest::Rerank {
        query: query.to_string(),
        candidates,
        agent_cmd: trimmed_cmd.to_string(),
        top_n,
        deadline_ms,
    })?;

    match resp {
        IpcResponse::RerankResult(env) => {
            let reordered: Vec<RankedCandidate> =
                serde_json::from_value(env.data).unwrap_or_default();

            println!("Status: {:?}", env.status);
            for w in &env.warnings {
                eprintln!("Warning: [{:?}] {}", w.code, w.message);
            }

            println!("\nReranked Results (Descending Relevance):");
            for (idx, c) in reordered.iter().enumerate() {
                let loc = if let Some(r) = &c.node.range {
                    format!("{}:{}:{}", c.node.path, r.start.line, r.start.column)
                } else {
                    c.node.path.clone()
                };

                let signals: Vec<String> = c
                    .explanation
                    .reasons
                    .iter()
                    .map(|r| format!("{}:{:.2}", r.signal, r.score))
                    .collect();

                println!(
                    "  [{}] {} ({}) in {} [Score: {:.3} | Signals: {}]",
                    idx + 1,
                    c.node.name,
                    c.node.kind.as_str(),
                    loc,
                    c.explanation.total_score,
                    signals.join(", ")
                );
            }
            Ok(())
        }
        IpcResponse::Error { code, message } => {
            Err(format!("Reranking failed: {:?}: {}", code, message))
        }
        _ => Err("Unexpected response".to_string()),
    }
}
