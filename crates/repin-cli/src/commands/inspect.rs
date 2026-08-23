use crate::client::DaemonClient;
use repin_core::model::node::Node;
use repin_core::protocol::ipc::{IpcRequest, IpcResponse};
use repin_core::runtime::inspect::FileOutline;

pub fn execute_inspect(client: &mut DaemonClient, path: &str) -> Result<(), String> {
    let resp = client.send_request(IpcRequest::InspectFile {
        path: path.to_string(),
    })?;

    match resp {
        IpcResponse::InspectResult(env) => {
            let outline: FileOutline =
                serde_json::from_value(env.data).unwrap_or_else(|_| FileOutline {
                    root: "root".to_string(),
                    path: path.to_string(),
                    symbols: Vec::new(),
                });

            println!("File Outline: {}", outline.path);
            println!("Symbols declared: {}", outline.symbols.len());
            for sym in &outline.symbols {
                let range = sym.range_preview.as_deref().unwrap_or("-");
                if let Some(ref q) = sym.qualified_name {
                    println!("  [{}] {} ({}) @ {}", sym.kind, sym.name, q, range);
                } else {
                    println!("  [{}] {} @ {}", sym.kind, sym.name, range);
                }
            }
            Ok(())
        }
        IpcResponse::Error { code, message } => {
            Err(format!("Inspect failed: {:?}: {}", code, message))
        }
        _ => Err("Unexpected response".to_string()),
    }
}

pub fn execute_at_position(
    client: &mut DaemonClient,
    path: &str,
    line: u32,
    column: u32,
) -> Result<(), String> {
    let resp = client.send_request(IpcRequest::AtPosition {
        path: path.to_string(),
        line,
        column,
    })?;

    match resp {
        IpcResponse::PositionResult(env) => {
            let node_opt: Option<Node> = serde_json::from_value(env.data).unwrap_or(None);

            if let Some(node) = node_opt {
                println!(
                    "Found definition at {}:{}:{} -> {} ({})",
                    path,
                    line,
                    column,
                    node.name,
                    node.kind.as_str()
                );
                if let Some(r) = node.range {
                    println!(
                        "  Span: {}:{} - {}:{}",
                        r.start.line, r.start.column, r.end.line, r.end.column
                    );
                }
            } else {
                println!(
                    "No enclosing AST definition found at {}:{}:{}",
                    path, line, column
                );
            }
            Ok(())
        }
        IpcResponse::Error { code, message } => {
            Err(format!("Position lookup failed: {:?}: {}", code, message))
        }
        _ => Err("Unexpected response".to_string()),
    }
}
