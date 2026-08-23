use repin_product::default_user_layout;
use std::fs;
use std::path::{Path, PathBuf};

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let dest_path = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_all(&entry.path(), &dest_path)?;
        } else {
            fs::copy(entry.path(), dest_path)?;
        }
    }
    Ok(())
}

pub fn execute_install(source: Option<PathBuf>) -> Result<(), String> {
    let source_dir = if let Some(src) = source {
        if src.is_file() {
            src.parent()
                .ok_or_else(|| "Invalid source path".to_string())?
                .to_path_buf()
        } else {
            src
        }
    } else {
        let current_exe = std::env::current_exe()
            .map_err(|e| format!("Failed to locate current executable: {e}"))?;
        current_exe
            .parent()
            .ok_or_else(|| "Failed to determine current executable directory".to_string())?
            .to_path_buf()
    };

    let source_binary = source_dir.join(repin_product::BINARY_NAME);
    if !source_binary.is_file() {
        return Err(format!(
            "Binary '{}' not found in source directory {}",
            repin_product::BINARY_NAME,
            source_dir.display()
        ));
    }

    let layout = default_user_layout().map_err(|e| e.to_string())?;
    let install_dir = &layout.install_dir;
    let install_bin = &layout.install_bin;
    let install_docs = &layout.install_docs;
    let bin_link = &layout.bin_link;
    let bin_dir = &layout.bin_base;

    let is_same_dir = source_dir
        .canonicalize()
        .ok()
        .zip(install_dir.canonicalize().ok())
        .map(|(a, b)| a == b)
        .unwrap_or(false);

    if is_same_dir {
        return Err(format!(
            "Repin is already installed in {}",
            install_dir.display()
        ));
    }

    if install_dir.exists() {
        fs::remove_dir_all(install_dir)
            .map_err(|e| format!("Failed to remove existing install directory: {e}"))?;
    }

    fs::create_dir_all(install_dir).map_err(|e| {
        format!(
            "Failed to create install directory {}: {e}",
            install_dir.display()
        )
    })?;

    fs::copy(&source_binary, install_bin)
        .map_err(|e| format!("Failed to copy binary to {}: {e}", install_bin.display()))?;

    // Ensure executable permissions on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(install_bin)
            .map_err(|e| {
                format!(
                    "Failed to inspect permissions for {}: {e}",
                    install_bin.display()
                )
            })?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(install_bin, perms).map_err(|e| {
            format!(
                "Failed to set permissions for {}: {e}",
                install_bin.display()
            )
        })?;
    }

    // Copy usage documentation if present
    let source_docs = source_dir.join(repin_product::DOCS_DIR_NAME);
    let mut docs_installed = false;
    if source_docs.is_dir() {
        copy_dir_all(&source_docs, install_docs)
            .map_err(|e| format!("Failed to copy documentation: {e}"))?;
        docs_installed = true;
    }

    // Ensure bin directory exists
    fs::create_dir_all(bin_dir).map_err(|e| {
        format!(
            "Failed to create binary directory {}: {e}",
            bin_dir.display()
        )
    })?;

    // Manage symlink in bin directory
    let symlink_correct = if bin_link.is_symlink() || fs::symlink_metadata(bin_link).is_ok() {
        match fs::read_link(bin_link) {
            Ok(target) => target == *install_bin,
            Err(_) => false,
        }
    } else {
        false
    };

    if !symlink_correct {
        if bin_link.is_symlink() || bin_link.exists() || fs::symlink_metadata(bin_link).is_ok() {
            let _ = fs::remove_file(bin_link);
        }

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(install_bin, bin_link)
                .map_err(|e| format!("Failed to create symlink at {}: {e}", bin_link.display()))?;
        }
    }

    let in_path = std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|p| p == *bin_dir))
        .unwrap_or(false);

    println!("Repin installed successfully:");
    println!("  • Binary:        {}", install_bin.display());
    println!(
        "  • Symlink:       {} -> {}",
        bin_link.display(),
        install_bin.display()
    );
    if docs_installed {
        println!("  • Documentation: {}", install_docs.display());
    }
    if !in_path {
        println!();
        println!("Note: {} is not in your PATH.", bin_dir.display());
        println!(
            "Add it to your shell configuration (e.g., export PATH=\"{}:$PATH\")",
            bin_dir.display()
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_execute_install_from_custom_source() {
        let temp = tempdir().unwrap();
        let source_dir = temp.path().join("source");
        let data_dir = temp.path().join("share");
        let bin_dir = temp.path().join("bin");

        fs::create_dir_all(&source_dir).unwrap();
        fs::write(source_dir.join("repin"), "#!/bin/sh\necho test\n").unwrap();
        let docs_dir = source_dir.join("docs");
        fs::create_dir_all(&docs_dir).unwrap();
        fs::write(docs_dir.join("index.html"), "<h1>Docs</h1>").unwrap();

        unsafe {
            std::env::set_var("HOME", temp.path());
            std::env::set_var("XDG_DATA_HOME", &data_dir);
            std::env::set_var("XDG_BIN_HOME", &bin_dir);
        }

        let result = execute_install(Some(source_dir.clone()));
        assert!(result.is_ok());

        let target_bin = data_dir.join("repin").join("repin");
        let target_docs = data_dir.join("repin").join("docs").join("index.html");
        let target_link = bin_dir.join("repin");

        assert!(target_bin.is_file());
        assert!(target_docs.is_file());
        assert!(target_link.is_symlink());
        assert_eq!(fs::read_link(target_link).unwrap(), target_bin);
    }

    #[test]
    fn test_execute_install_rejects_existing_install_source() {
        let temp = tempdir().unwrap();
        let data_dir = temp.path().join("share");
        let bin_dir = temp.path().join("bin");
        let install_dir = data_dir.join("repin");

        fs::create_dir_all(&install_dir).unwrap();
        fs::write(install_dir.join("repin"), "already installed").unwrap();

        unsafe {
            std::env::set_var("HOME", temp.path());
            std::env::set_var("XDG_DATA_HOME", &data_dir);
            std::env::set_var("XDG_BIN_HOME", &bin_dir);
        }

        let error = execute_install(Some(install_dir.clone())).unwrap_err();
        assert!(
            error.contains("already installed"),
            "unexpected error: {error}"
        );
    }
}
