use repin_product::ProjectLayout;
use std::process::Command;
use tempfile::tempdir;

/// Isolate each end-to-end case behind its own runtime directory so the test
/// drives a dedicated daemon instead of the developer's user-wide one.
fn repin(runtime_dir: &std::path::Path, project: &std::path::Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_repin"));
    command
        .env("XDG_RUNTIME_DIR", runtime_dir)
        .arg("--project")
        .arg(project);
    command
}

#[test]
fn test_version_uses_package_and_commit_identity() {
    let output = Command::new(env!("CARGO_BIN_EXE_repin"))
        .arg("--version")
        .output()
        .expect("Failed to execute repin --version");

    assert!(output.status.success());
    let version = String::from_utf8_lossy(&output.stdout);
    let prefix = format!("repin v{}-", env!("CARGO_PKG_VERSION"));
    assert!(
        version.starts_with(&prefix),
        "unexpected version: {version}"
    );
    let suffix = version.trim().strip_prefix(&prefix).unwrap();
    assert!(suffix == "unknown" || (1..=12).contains(&suffix.len()));
}

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

    assert!(
        !output.status.success(),
        "Expected repin index to fail on uninitialized project"
    );
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

    assert!(
        index_output.status.success(),
        "repin index should succeed on initialized project"
    );
}

/// docs/runtime.md §4 and §9(11): `uninit` is daemon-mediated, so a project
/// re-initialized at the same canonical path must not serve the removed graph.
#[test]
fn test_uninit_then_reinit_does_not_serve_the_removed_graph() {
    let runtime = tempdir().unwrap();
    let project = tempdir().unwrap();
    let project_path = project.path();
    std::fs::write(project_path.join("a.rs"), b"fn alpha_marker() {}\n").unwrap();

    let init = repin(runtime.path(), project_path)
        .arg("init")
        .output()
        .expect("repin init");
    assert!(init.status.success(), "repin init should succeed");

    let found = repin(runtime.path(), project_path)
        .args(["search", "--graph", "alpha_marker"])
        .output()
        .expect("repin search");
    let found_stdout = String::from_utf8_lossy(&found.stdout);
    assert!(
        found_stdout.contains("alpha_marker"),
        "indexed symbol should be in the graph, got: {found_stdout}"
    );

    let uninit = repin(runtime.path(), project_path)
        .args(["uninit", "-f"])
        .output()
        .expect("repin uninit");
    assert!(uninit.status.success(), "repin uninit should succeed");
    assert!(!ProjectLayout::at_root(project_path).state_dir.exists());

    let reinit = repin(runtime.path(), project_path)
        .args(["init", "--no-index"])
        .output()
        .expect("repin init");
    assert!(reinit.status.success(), "repin re-init should succeed");

    let stale = repin(runtime.path(), project_path)
        .args(["search", "--graph", "alpha_marker"])
        .output()
        .expect("repin search");
    let stale_stdout = String::from_utf8_lossy(&stale.stdout);

    let _ = repin(runtime.path(), project_path).arg("stop").output();

    assert!(
        !stale_stdout.contains("alpha_marker (function)"),
        "re-initialized empty graph must not serve the removed database, got: {stale_stdout}"
    );
}
