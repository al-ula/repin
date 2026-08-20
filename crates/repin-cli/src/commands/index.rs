use crate::client::DaemonClient;
use repin_protocol::ipc::{IpcRequest, IpcResponse};

pub fn execute_init(project_dir: &std::path::Path) -> Result<(), String> {
    let repin_dir = project_dir.join(".repin");
    std::fs::create_dir_all(&repin_dir)
        .map_err(|e| format!("Failed to create .repin directory: {e}"))?;
    println!(
        "Initialized empty Repin workspace in {}",
        repin_dir.display()
    );
    Ok(())
}

pub fn execute_index(client: &mut DaemonClient) -> Result<(), String> {
    println!("Indexing workspace files...");
    let resp = client.send_request(IpcRequest::IndexAll)?;

    match resp {
        IpcResponse::IndexAllOk {
            files_indexed,
            revision,
        } => {
            println!(
                "Successfully indexed {} files into graph (Revision: {})",
                files_indexed, revision.0
            );
            Ok(())
        }
        IpcResponse::Error { code, message } => {
            Err(format!("Index failed: {:?}: {}", code, message))
        }
        _ => Err("Unexpected response".to_string()),
    }
}
