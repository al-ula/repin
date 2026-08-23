use crate::client::DaemonClient;
use repin_core::protocol::ipc::{IpcRequest, IpcResponse, RebuildTarget};

pub fn execute_rebuild(client: &mut DaemonClient, target: RebuildTarget) -> Result<(), String> {
    let response = client.send_request(IpcRequest::Rebuild { target })?;
    match response {
        IpcResponse::RebuildOk {
            target,
            files_indexed,
            revision,
        } => {
            println!(
                "Rebuild target {:?} completed for {} files (revision: {})",
                target, files_indexed, revision.0
            );
            Ok(())
        }
        IpcResponse::Error { code, message } => Err(format!("rebuild failed: {code:?}: {message}")),
        other => Err(format!("unexpected rebuild response: {other:?}")),
    }
}
