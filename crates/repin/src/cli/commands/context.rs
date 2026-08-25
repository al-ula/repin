use crate::cli::client::DaemonClient;
use repin_core::model::provenance::Revision;
use repin_core::protocol::ipc::{IpcRequest, IpcResponse};
use repin_core::runtime::context::AssembledContext;

pub fn execute_context(
    client: &mut DaemonClient,
    query: &str,
    budget_bytes: usize,
    context_override: Option<repin_core::config::ContextConfig>,
) -> Result<(), String> {
    let resp = client.send_request(IpcRequest::Context {
        query: query.to_string(),
        budget_bytes: Some(budget_bytes),
        padding_lines: context_override.as_ref().map(|c| c.padding_lines),
        include_blast_radius: context_override.as_ref().map(|c| c.include_blast_radius),
        include_verbatim_source: context_override.as_ref().map(|c| c.include_verbatim_source),
    })?;

    match resp {
        IpcResponse::ContextResult(env) => {
            let assembled: AssembledContext =
                serde_json::from_value(env.data).unwrap_or_else(|_| AssembledContext {
                    snippets: Vec::new(),
                    total_bytes: 0,
                    truncated: false,
                });

            println!("Assembled Context for \"{}\":", query);
            println!(
                "Total bytes: {} (Budget: {}, Truncated: {})",
                assembled.total_bytes, budget_bytes, assembled.truncated
            );
            println!("Snippets included: {}", assembled.snippets.len());
            for (idx, snip) in assembled.snippets.iter().enumerate() {
                println!(
                    "\n--- Snippet [{}] {}:{}..{} ---",
                    idx + 1,
                    snip.path,
                    snip.start_line,
                    snip.end_line
                );
                println!("{}", snip.content);
            }
            Ok(())
        }
        IpcResponse::Error { code, message } => {
            Err(format!("Context assembly failed: {:?}: {}", code, message))
        }
        _ => Err("Unexpected response".to_string()),
    }
}

pub fn execute_review_context(
    client: &mut DaemonClient,
    since_rev: Option<u64>,
    budget_bytes: usize,
) -> Result<(), String> {
    let changed_since = since_rev.map(Revision);
    let resp = client.send_request(IpcRequest::ReviewContext {
        changed_since,
        budget_bytes: Some(budget_bytes),
    })?;

    match resp {
        IpcResponse::ReviewResult(env) => {
            let assembled: AssembledContext =
                serde_json::from_value(env.data).unwrap_or_else(|_| AssembledContext {
                    snippets: Vec::new(),
                    total_bytes: 0,
                    truncated: false,
                });

            println!("Assembled Review Context (ADR-016):");
            println!(
                "Total bytes: {} (Budget: {}, Truncated: {})",
                assembled.total_bytes, budget_bytes, assembled.truncated
            );
            println!("Context snippets: {}", assembled.snippets.len());
            for (idx, snip) in assembled.snippets.iter().enumerate() {
                println!(
                    "\n--- Review Snippet [{}] {}:{}..{} ---",
                    idx + 1,
                    snip.path,
                    snip.start_line,
                    snip.end_line
                );
                println!("{}", snip.content);
            }
            Ok(())
        }
        IpcResponse::Error { code, message } => {
            Err(format!("Review context failed: {:?}: {}", code, message))
        }
        _ => Err("Unexpected response".to_string()),
    }
}
