use assert_cmd::Command;
use serde_json::Value;
use tempfile::tempdir;

fn spike_command() -> Command {
    Command::cargo_bin("repin-foundation-spike").expect("foundation spike binary")
}

fn normalize_manifest(mut value: Value) -> Value {
    let object = value.as_object_mut().expect("manifest object");
    for key in [
        "source_revision",
        "rustc",
        "cargo",
        "target",
        "os",
        "architecture",
    ] {
        object.remove(key);
    }
    object.insert(
        "active_features".into(),
        Value::Array(vec![Value::String("default".into())]),
    );
    value
}

#[test]
fn q003_preflight_snapshot_is_reviewable_and_stable() {
    let output = spike_command()
        .arg("preflight")
        .output()
        .expect("run preflight");

    assert!(output.status.success(), "preflight failed: {output:?}");
    assert!(output.stderr.is_empty(), "preflight wrote stderr");

    let manifest: Value = serde_json::from_slice(&output.stdout).expect("preflight JSON");
    insta::assert_json_snapshot!("q003_preflight_manifest", normalize_manifest(manifest));
}

#[test]
fn q003_f7_command_returns_json_on_stdout_only() {
    let output_dir = tempdir().expect("temporary F7 output directory");
    let output = spike_command()
        .args([
            "run",
            "F7",
            "--output",
            output_dir.path().to_str().expect("UTF-8 temporary path"),
        ])
        .output()
        .expect("run F7");

    assert!(output.status.success(), "F7 failed: {output:?}");
    assert!(output.stderr.is_empty(), "F7 wrote stderr");

    let report: Value = serde_json::from_slice(&output.stdout).expect("F7 JSON");
    assert_eq!(report["experiment"], "F7");
    assert!(
        report["cases"]
            .as_array()
            .is_some_and(|cases| !cases.is_empty())
    );
    assert!(output_dir.path().join("F7-report.json").is_file());
}

#[test]
fn q003_invalid_command_has_nonzero_exit_and_diagnostic() {
    let output = spike_command()
        .arg("not-a-command")
        .output()
        .expect("run invalid command");

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("usage:"), "unexpected stderr: {stderr}");
}
