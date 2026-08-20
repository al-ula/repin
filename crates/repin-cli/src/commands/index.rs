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

    let db_path = repin_dir.join("graph.sqlite3");
    if !db_path.exists() {
        let _ = repin_engine::Engine::open(repin_engine::EngineOptions {
            root_id: "root".to_string(),
            root_path: project_dir.to_path_buf(),
            db_path: Some(db_path),
        });
    }

    println!(
        "Initialized empty Repin workspace in {}",
        repin_dir.display()
    );
    Ok(())
}

pub fn execute_uninit(project_dir: &std::path::Path, force: bool) -> Result<(), String> {
    let repin_dir = if project_dir.join(".repin").exists() {
        project_dir.join(".repin")
    } else {
        let mut ancestor = project_dir.to_path_buf();
        let mut found = None;
        while let Some(parent) = ancestor.parent() {
            let candidate = parent.join(".repin");
            if candidate.is_dir() {
                found = Some(candidate);
                break;
            }
            ancestor = parent.to_path_buf();
        }
        found.unwrap_or_else(|| project_dir.join(".repin"))
    };

    if !repin_dir.exists() {
        println!("No Repin workspace found in {}", project_dir.display());
        return Ok(());
    }

    if !force {
        use std::io::{self, Write};
        print!(
            "Are you sure you want to uninitialize Repin workspace in {}? [y/N]: ",
            repin_dir.display()
        );
        io::stdout().flush().map_err(|e| e.to_string())?;

        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .map_err(|e| format!("Failed to read confirmation: {e}"))?;

        let trimmed = input.trim().to_lowercase();
        if trimmed != "y" && trimmed != "yes" {
            println!("Uninit aborted.");
            return Ok(());
        }
    }

    std::fs::remove_dir_all(&repin_dir)
        .map_err(|e| format!("Failed to remove {}: {e}", repin_dir.display()))?;

    println!("Uninitialized Repin workspace in {}", repin_dir.display());
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

        let db_path = repin_dir.join("graph.sqlite3");
        assert!(db_path.exists());
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

    #[test]
    fn test_execute_uninit_removes_dir() {
        let temp_dir = tempfile::tempdir().unwrap();
        let project_path = temp_dir.path();

        execute_init(project_path).unwrap();
        let repin_dir = project_path.join(".repin");
        assert!(repin_dir.exists());

        let res = execute_uninit(project_path, true);
        assert!(res.is_ok());
        assert!(!repin_dir.exists());
    }

    #[test]
    fn test_execute_uninit_when_not_initialized() {
        let temp_dir = tempfile::tempdir().unwrap();
        let project_path = temp_dir.path();

        let res = execute_uninit(project_path, true);
        assert!(res.is_ok());
    }

    #[test]
    fn test_execute_uninit_from_subdirectory() {
        let temp_dir = tempfile::tempdir().unwrap();
        let project_path = temp_dir.path();

        execute_init(project_path).unwrap();
        let subdir = project_path.join("src").join("nested");
        std::fs::create_dir_all(&subdir).unwrap();

        let repin_dir = project_path.join(".repin");
        assert!(repin_dir.exists());

        let res = execute_uninit(&subdir, true);
        assert!(res.is_ok());
        assert!(!repin_dir.exists());
    }
}
