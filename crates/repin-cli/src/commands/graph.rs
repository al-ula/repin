use crate::client::DaemonClient;
use repin_core::model::node::Node;
use repin_engine::traversal::NeighborsData;
use repin_protocol::ipc::{IpcRequest, IpcResponse};

pub fn execute_entity(client: &mut DaemonClient, name_or_id: &str) -> Result<(), String> {
    let resp = client.send_request(IpcRequest::Entity {
        name_or_id: name_or_id.to_string(),
    })?;

    match resp {
        IpcResponse::EntityResult(env) => {
            let node_opt: Option<Node> = serde_json::from_value(env.data).unwrap_or(None);

            if let Some(node) = node_opt {
                println!("Entity: {} ({})", node.name, node.kind.as_str());
                println!("  Node ID: {}", node.id);
                println!("  Path: {}", node.path);
                if let Some(ref q) = node.qualified_name {
                    println!("  Qualified Name: {}", q);
                }
                if let Some(r) = node.range {
                    println!(
                        "  Range: {}:{} - {}:{}",
                        r.start.line, r.start.column, r.end.line, r.end.column
                    );
                }
                if !node.attributes.is_empty() {
                    println!(
                        "  Attributes: {}",
                        serde_json::to_string(&node.attributes).unwrap_or_default()
                    );
                }
            } else {
                println!("Entity not found: {}", name_or_id);
            }
            Ok(())
        }
        IpcResponse::Error { code, message } => {
            Err(format!("Entity lookup failed: {:?}: {}", code, message))
        }
        _ => Err("Unexpected response".to_string()),
    }
}

pub fn execute_neighbors(
    client: &mut DaemonClient,
    name_or_id: &str,
    max_depth: usize,
) -> Result<(), String> {
    let resp = client.send_request(IpcRequest::Neighbors {
        name_or_id: name_or_id.to_string(),
        max_depth: Some(max_depth),
    })?;

    match resp {
        IpcResponse::NeighborsResult(env) => {
            let data_opt: Option<NeighborsData> = serde_json::from_value(env.data).unwrap_or(None);

            if let Some(data) = data_opt {
                println!(
                    "Neighbors for {} ({} in {}):",
                    data.target.name,
                    data.target.kind.as_str(),
                    data.target.path
                );
                println!(
                    "  Incoming Relations (referrers/callers): {}",
                    data.incoming.len()
                );
                for item in &data.incoming {
                    if let Some(ref from_node) = item.node {
                        println!(
                            "    <- [{}] {} ({}) in {}",
                            item.edge.kind.as_str(),
                            from_node.name,
                            from_node.kind.as_str(),
                            from_node.path
                        );
                    } else {
                        println!("    <- [{}] (external/unresolved)", item.edge.kind.as_str());
                    }
                }

                println!(
                    "  Outgoing Relations (dependencies/callees): {}",
                    data.outgoing.len()
                );
                for item in &data.outgoing {
                    if let Some(ref to_node) = item.node {
                        println!(
                            "    -> [{}] {} ({}) in {}",
                            item.edge.kind.as_str(),
                            to_node.name,
                            to_node.kind.as_str(),
                            to_node.path
                        );
                    } else {
                        println!("    -> [{}] (external/unresolved)", item.edge.kind.as_str());
                    }
                }
            } else {
                println!("Entity not found for neighbor lookup: {}", name_or_id);
            }
            Ok(())
        }
        IpcResponse::Error { code, message } => {
            Err(format!("Neighbors lookup failed: {:?}: {}", code, message))
        }
        _ => Err("Unexpected response".to_string()),
    }
}
