use crate::client::DaemonClient;
use repin_protocol::ipc::{IpcRequest, IpcResponse};
use std::time::Duration;

pub fn execute_watch(client: &mut DaemonClient, poll_interval_ms: u64) -> Result<(), String> {
    println!(
        "Watching repository for changes (interval: {}ms)... Press Ctrl+C to stop.",
        poll_interval_ms
    );

    loop {
        std::thread::sleep(Duration::from_millis(poll_interval_ms));
        if let Ok(resp) = client.send_request(IpcRequest::SyncVcs)
            && let IpcResponse::UpdateOk { revision } = resp
            && revision.0 > 0
        {
            println!("[watch] Updated graph to Revision: {}", revision.0);
        }
    }
}
