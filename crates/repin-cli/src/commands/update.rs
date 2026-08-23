use crate::client::DaemonClient;
use repin_core::protocol::ipc::{IpcRequest, IpcResponse};

pub fn execute_update(client: &mut DaemonClient) -> Result<(), String> {
    println!("Checking for VCS worktree changes and updating incrementally...");
    let resp = client.send_request(IpcRequest::SyncVcs)?;

    match resp {
        IpcResponse::UpdateOk { revision } => {
            println!(
                "Incremental update complete. Graph advanced to Revision: {}",
                revision.0
            );
            Ok(())
        }
        IpcResponse::Error { code, message } => {
            Err(format!("Update failed: {:?}: {}", code, message))
        }
        _ => Err("Unexpected response".to_string()),
    }
}
