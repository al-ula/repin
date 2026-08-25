use crate::cli::client::DaemonClient;
use repin_core::protocol::ipc::{IpcRequest, IpcResponse};
use std::time::Duration;

pub fn execute_watch(client: &mut DaemonClient, poll_interval_ms: u64) -> Result<(), String> {
    println!(
        "Watching repository for changes (interval: {}ms)... Press Ctrl+C to stop.",
        poll_interval_ms
    );

    let mut last_revision = match client.send_request(IpcRequest::Status) {
        Ok(IpcResponse::StatusOk { graph_revision, .. }) => graph_revision,
        _ => repin_core::model::provenance::Revision::INITIAL,
    };

    loop {
        std::thread::sleep(Duration::from_millis(poll_interval_ms));
        if let Ok(resp) = client.send_request(IpcRequest::Status)
            && let IpcResponse::StatusOk { graph_revision, .. } = resp
            && graph_revision > last_revision
        {
            println!("[watch] Updated graph to Revision: {}", graph_revision.0);
            last_revision = graph_revision;
        }
    }
}
