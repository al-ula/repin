use crate::cli::client::DaemonClient;
use repin_core::protocol::evidence::Evidence;
use repin_core::protocol::ipc::{IpcRequest, IpcResponse};
use repin_runtime::ranking::RankedCandidate;

pub fn execute_search(
    client: &mut DaemonClient,
    pattern: &str,
    is_regex: bool,
    use_graph: bool,
    use_hybrid: bool,
    limit: usize,
    centrality_boost: Option<f64>,
) -> Result<(), String> {
    let req = if is_regex {
        IpcRequest::SearchDirect {
            pattern: pattern.to_string(),
            is_regex: true,
            paths: None,
            max_results: Some(limit),
        }
    } else if use_graph && !use_hybrid {
        IpcRequest::SearchGraph {
            query: pattern.to_string(),
            max_results: Some(limit),
        }
    } else {
        // Deterministic multi-channel hybrid search (FTS5 + Symbol graph)
        IpcRequest::SearchHybrid {
            query: pattern.to_string(),
            max_results: Some(limit),
            centrality_boost,
        }
    };

    let resp = client.send_request(req)?;

    match resp {
        IpcResponse::SearchResult(env) => {
            if is_regex {
                let direct_evidence: Vec<Evidence> =
                    serde_json::from_value(env.data).unwrap_or_default();

                println!("Status: {:?}", env.status);
                println!("Found {} direct text matches:", direct_evidence.len());
                for ev in &direct_evidence {
                    if let Some(r) = &ev.range {
                        println!(
                            "  {}:{} - {}",
                            ev.path,
                            r.start,
                            ev.preview.as_deref().unwrap_or("")
                        );
                    } else {
                        println!("  {} - {}", ev.path, ev.preview.as_deref().unwrap_or(""));
                    }
                }
            } else {
                let candidates: Vec<RankedCandidate> =
                    serde_json::from_value(env.data).unwrap_or_default();

                println!("Status: {:?}", env.status);
                println!("Found {} ranked symbol matches:", candidates.len());
                for (idx, c) in candidates.iter().enumerate() {
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
            }
            Ok(())
        }
        IpcResponse::Error { code, message } => {
            Err(format!("Search failed: {:?}: {}", code, message))
        }
        _ => Err("Unexpected response".to_string()),
    }
}
