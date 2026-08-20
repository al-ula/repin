use crate::client::DaemonClient;
use repin_core::model::node::Node;
use repin_engine::traversal::{ImpactData, NeighborsData, PathTraceData};
use repin_protocol::ipc::{IpcRequest, IpcResponse};
use std::collections::BTreeMap;

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

pub fn execute_impact(
    client: &mut DaemonClient,
    name_or_id: &str,
    max_depth: usize,
    json: bool,
) -> Result<(), String> {
    let resp = client.send_request(IpcRequest::Impact {
        name_or_id: name_or_id.to_string(),
        max_depth: Some(max_depth),
    })?;

    match resp {
        IpcResponse::ImpactResult(env) => {
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&env).map_err(|e| e.to_string())?
                );
                return Ok(());
            }

            let data_opt: Option<ImpactData> = serde_json::from_value(env.data).unwrap_or(None);
            if let Some(data) = data_opt {
                println!(
                    "Blast Radius for '{}' ({} in {}) [Max Depth: {}]:",
                    data.target.name,
                    data.target.kind.as_str(),
                    data.target.path,
                    data.max_depth
                );
                println!("Total Impacted Symbols: {}", data.total_impacted);

                // Group by depth
                let mut by_depth: BTreeMap<usize, Vec<_>> = BTreeMap::new();
                for item in data.items {
                    by_depth.entry(item.depth).or_default().push(item);
                }

                for (depth, items) in by_depth {
                    let label = if depth == 1 {
                        "Direct Referrers / Callers".to_string()
                    } else {
                        format!("Transitive Level {}", depth)
                    };
                    println!("\n  Level {} ({}):", depth, label);
                    for item in items {
                        println!(
                            "    <- [{}] {} ({}) in {}",
                            item.via_edge_kind,
                            item.node.name,
                            item.node.kind.as_str(),
                            item.node.path
                        );
                    }
                }
            } else {
                println!("Entity not found for impact analysis: {}", name_or_id);
            }
            Ok(())
        }
        IpcResponse::Error { code, message } => {
            Err(format!("Impact analysis failed: {:?}: {}", code, message))
        }
        _ => Err("Unexpected response".to_string()),
    }
}

pub fn execute_path(
    client: &mut DaemonClient,
    from: &str,
    to: &str,
    max_depth: usize,
    json: bool,
) -> Result<(), String> {
    let resp = client.send_request(IpcRequest::Path {
        from: from.to_string(),
        to: to.to_string(),
        max_depth: Some(max_depth),
    })?;

    match resp {
        IpcResponse::PathResult(env) => {
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&env).map_err(|e| e.to_string())?
                );
                return Ok(());
            }

            let data_opt: Option<PathTraceData> = serde_json::from_value(env.data).unwrap_or(None);
            if let Some(data) = data_opt {
                if data.paths.is_empty() {
                    println!(
                        "No dependency path found connecting '{}' -> '{}' within depth limit {}.",
                        data.from.name, data.to.name, data.max_depth
                    );
                    return Ok(());
                }

                println!(
                    "Found {} path(s) connecting '{}' -> '{}' [Max Depth: {}]:",
                    data.paths.len(),
                    data.from.name,
                    data.to.name,
                    data.max_depth
                );

                for (p_idx, path) in data.paths.iter().enumerate() {
                    println!("\n--- Path [{}] ({} hops) ---", p_idx + 1, path.length - 1);
                    for (s_idx, segment) in path.segments.iter().enumerate() {
                        if let Some(ref edge) = segment.edge_to_next {
                            println!(
                                "  [{}] {} ({}) in {}",
                                s_idx + 1,
                                segment.node.name,
                                segment.node.kind.as_str(),
                                segment.node.path
                            );
                            println!("      └──[{}]──►", edge.kind.as_str());
                        } else {
                            println!(
                                "  [{}] {} ({}) in {}",
                                s_idx + 1,
                                segment.node.name,
                                segment.node.kind.as_str(),
                                segment.node.path
                            );
                        }
                    }
                }
            } else {
                println!("One or both entities not found: '{}' -> '{}'", from, to);
            }
            Ok(())
        }
        IpcResponse::Error { code, message } => {
            Err(format!("Path trace failed: {:?}: {}", code, message))
        }
        _ => Err("Unexpected response".to_string()),
    }
}
