use crate::cli::client::DaemonClient;
use crate::product::ProjectLayout;
use repin_core::config::RepinConfig;
use repin_core::protocol::ipc::{IpcRequest, IpcResponse};

/// Daemon-mediated project initialization (ADR-026). The daemon creates the
/// state directory, database, and writer lease, then binds this connection to
/// the initialized project so indexing can follow on the same connection.
pub fn execute_init(
    project_dir: &std::path::Path,
    resolved_config: Option<RepinConfig>,
) -> Result<DaemonClient, String> {
    std::fs::create_dir_all(project_dir)
        .map_err(|e| format!("Failed to create project directory: {e}"))?;
    let canonical = project_dir
        .canonicalize()
        .map_err(|e| format!("Failed to resolve project directory: {e}"))?;

    let mut client = DaemonClient::connect_or_start_unbound()
        .map_err(|e| format!("Failed to connect to daemon: {e}"))?;
    require_state_lifecycle(&client)?;

    match client.send_request(IpcRequest::InitializeProject {
        project_root: canonical.display().to_string(),
        resolved_config,
    })? {
        IpcResponse::InitializeProjectOk {
            project_root,
            created,
            ..
        } => {
            let state_dir = ProjectLayout::at_root(&project_root).state_dir;
            if created {
                println!(
                    "Initialized empty Repin workspace in {}",
                    state_dir.display()
                );
            } else {
                println!(
                    "Repin workspace already initialized in {}",
                    state_dir.display()
                );
            }
            Ok(client)
        }
        IpcResponse::Error { code, message } => Err(format!("Init failed: {code:?}: {message}")),
        _ => Err("Unexpected init response".to_string()),
    }
}

/// Daemon-mediated project removal (ADR-026). The daemon unloads the project
/// context — closing the store and releasing the writer lease — before the
/// state directory is deleted. With no reachable daemon there is no context to
/// unload, so the unattached state directory is removed locally.
pub fn execute_uninit(project_dir: &std::path::Path, force: bool) -> Result<(), String> {
    let layout = crate::daemon::discover_state_layout(project_dir);
    let Some(layout) = layout else {
        println!("No Repin workspace found in {}", project_dir.display());
        return Ok(());
    };

    if !force && !confirm_removal(&layout.state_dir)? {
        println!("Uninit aborted.");
        return Ok(());
    }

    match DaemonClient::connect_existing_unbound(None) {
        Ok(mut client) => {
            require_state_lifecycle(&client)?;
            match client.send_request(IpcRequest::UninitializeProject {
                project_root: layout.project_root.display().to_string(),
            })? {
                IpcResponse::UninitializeProjectOk {
                    project_root,
                    removed,
                } => {
                    if removed {
                        let state_dir = ProjectLayout::at_root(&project_root).state_dir;
                        println!("Uninitialized Repin workspace in {}", state_dir.display());
                    } else {
                        println!("No Repin workspace found in {project_root}");
                    }
                    Ok(())
                }
                IpcResponse::Error { code, message } => {
                    Err(format!("Uninit failed: {code:?}: {message}"))
                }
                _ => Err("Unexpected uninit response".to_string()),
            }
        }
        Err(_) => {
            // No daemon owns this state; remove it directly.
            std::fs::remove_dir_all(&layout.state_dir)
                .map_err(|e| format!("Failed to remove {}: {e}", layout.state_dir.display()))?;
            println!(
                "Uninitialized Repin workspace in {}",
                layout.state_dir.display()
            );
            Ok(())
        }
    }
}

/// State lifecycle requests exist only from protocol 2 (ADR-026). A daemon
/// still serving protocol 1 negotiates successfully but cannot mediate them,
/// and creating or deleting state behind its back is the stale-context fault
/// the decision closes. Refuse with the bounded recovery instead.
fn require_state_lifecycle(client: &DaemonClient) -> Result<(), String> {
    if client.supports_state_lifecycle() {
        return Ok(());
    }
    Err(format!(
        "PROTOCOL_MISMATCH: the running daemon negotiated protocol {} and cannot mediate project state. Run `repin daemon restart` and retry.",
        client.selected_protocol()
    ))
}

fn confirm_removal(state_dir: &std::path::Path) -> Result<bool, String> {
    use std::io::{self, Write};
    print!(
        "Are you sure you want to uninitialize Repin workspace in {}? [y/N]: ",
        state_dir.display()
    );
    io::stdout().flush().map_err(|e| e.to_string())?;

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|e| format!("Failed to read confirmation: {e}"))?;

    let trimmed = input.trim().to_lowercase();
    Ok(trimmed == "y" || trimmed == "yes")
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
