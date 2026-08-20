use std::process::Command;
use tempfile::tempdir;

#[test]
fn test_index_on_uninitialized_project_fails() {
    let temp_dir = tempdir().unwrap();
    let uninit_path = temp_dir.path();

    // Run the repin binary directly with index command on the uninitialized directory
    let bin_path = env!("CARGO_BIN_EXE_repin");
    let output = Command::new(bin_path)
        .arg("--project")
        .arg(uninit_path)
        .arg("index")
        .output()
        .expect("Failed to execute repin binary");

    assert!(!output.status.success(), "Expected repin index to fail on uninitialized project");
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{stdout}\n{stderr}");

    assert!(
        combined.contains("PROJECT_NOT_INITIALIZED") || combined.contains("not initialized"),
        "Error message should mention that project is not initialized, got: {combined}"
    );
}

#[test]
fn test_init_then_index_succeeds() {
    let temp_dir = tempdir().unwrap();
    let project_path = temp_dir.path();

    let bin_path = env!("CARGO_BIN_EXE_repin");

    // Initialize the project with --no-index
    let init_output = Command::new(bin_path)
        .arg("--project")
        .arg(project_path)
        .arg("init")
        .arg("--no-index")
        .output()
        .expect("Failed to execute repin init");

    assert!(init_output.status.success(), "repin init should succeed");

    // Now index the initialized project
    let index_output = Command::new(bin_path)
        .arg("--project")
        .arg(project_path)
        .arg("index")
        .output()
        .expect("Failed to execute repin index");

    assert!(index_output.status.success(), "repin index should succeed on initialized project");
}
