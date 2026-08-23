use crate::client::DaemonClient;
use repin_core::protocol::ipc::{IpcRequest, IpcResponse};

pub fn execute_status(client: &mut DaemonClient) -> Result<(), String> {
    let resp = client.send_request(IpcRequest::Status)?;

    match resp {
        IpcResponse::StatusOk {
            graph_revision,
            node_count,
            edge_count,
        } => {
            println!("Repin Daemon Status: Active");
            println!("  Graph Revision: {}", graph_revision);
            println!("  Nodes: {}", node_count);
            println!("  Edges: {}", edge_count);
            Ok(())
        }
        IpcResponse::Error { code, message } => {
            Err(format!("Status failed: {:?}: {}", code, message))
        }
        _ => Err("Unexpected response".to_string()),
    }
}
