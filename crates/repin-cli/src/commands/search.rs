use crate::client::DaemonClient;
use repin_protocol::envelope::ResultEnvelope;
use repin_protocol::evidence::Evidence;
use repin_protocol::ipc::{IpcRequest, IpcResponse};

pub fn execute_search(
    client: &mut DaemonClient,
    pattern: &str,
    is_regex: bool,
    max_results: usize,
) -> Result<(), String> {
    let resp = client.send_request(IpcRequest::SearchDirect {
        pattern: pattern.to_string(),
        is_regex,
        paths: None,
        max_results: Some(max_results),
    })?;

    match resp {
        IpcResponse::SearchResult(env) => {
            let direct_env: ResultEnvelope<Vec<Evidence>> =
                serde_json::from_value(env.data).unwrap_or_else(|_| ResultEnvelope::ok(Vec::new()));

            println!("Status: {:?}", env.status);
            println!("Found {} matches:", direct_env.data.len());
            for ev in &direct_env.data {
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
            Ok(())
        }
        IpcResponse::Error { code, message } => {
            Err(format!("Search failed: {:?}: {}", code, message))
        }
        _ => Err("Unexpected response".to_string()),
    }
}
