use crate::client::DaemonClient;
use repin_protocol::ipc::{IpcRequest, IpcResponse};

pub fn execute_init(project_dir: &std::path::Path) -> Result<(), String> {
    let repin_dir = project_dir.join(".repin");
    std::fs::create_dir_all(&repin_dir)
        .map_err(|e| format!("Failed to create .repin directory: {e}"))?;

    let gitignore_path = repin_dir.join(".gitignore");
    if !gitignore_path.exists() {
        std::fs::write(&gitignore_path, "*\n")
            .map_err(|e| format!("Failed to create .repin/.gitignore: {e}"))?;
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execute_init_creates_dir_and_gitignore() {
        let temp_dir = tempfile::tempdir().unwrap();
        let project_path = temp_dir.path();

        let res = execute_init(project_path);
        assert!(res.is_ok());

        let repin_dir = project_path.join(".repin");
        assert!(repin_dir.exists());
        assert!(repin_dir.is_dir());

        let gitignore_path = repin_dir.join(".gitignore");
        assert!(gitignore_path.exists());
        let content = std::fs::read_to_string(&gitignore_path).unwrap();
        assert_eq!(content, "*\n");
    }

    #[test]
    fn test_execute_init_preserves_existing_gitignore() {
        let temp_dir = tempfile::tempdir().unwrap();
        let project_path = temp_dir.path();
        let repin_dir = project_path.join(".repin");
        std::fs::create_dir_all(&repin_dir).unwrap();
        let gitignore_path = repin_dir.join(".gitignore");
        std::fs::write(&gitignore_path, "# custom\n*.db\n").unwrap();

        let res = execute_init(project_path);
        assert!(res.is_ok());

        let content = std::fs::read_to_string(&gitignore_path).unwrap();
        assert_eq!(content, "# custom\n*.db\n");
    }
}

