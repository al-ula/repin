use crate::client::DaemonClient;
use repin_engine::FileOutline;
use repin_protocol::envelope::ResultEnvelope;
use repin_protocol::ipc::{IpcRequest, IpcResponse};

pub fn execute_inspect(client: &mut DaemonClient, path: &str) -> Result<(), String> {
    let resp = client.send_request(IpcRequest::InspectFile {
        path: path.to_string(),
    })?;

    match resp {
        IpcResponse::InspectResult(env) => {
            let outline: ResultEnvelope<FileOutline> = serde_json::from_value(env.data)
                .unwrap_or_else(|_| {
                    ResultEnvelope::ok(FileOutline {
                        root: "root".to_string(),
                        path: path.to_string(),
                        symbols: Vec::new(),
                    })
                });

            println!("File Outline for: {}", outline.data.path);
            for sym in &outline.data.symbols {
                println!(
                    "  {} [{}] {}",
                    sym.name,
                    sym.kind,
                    sym.range_preview.as_deref().unwrap_or("")
                );
            }
            Ok(())
        }
        IpcResponse::Error { code, message } => {
            Err(format!("Inspect failed: {:?}: {}", code, message))
        }
        _ => Err("Unexpected response".to_string()),
    }
}
