use std::env;

fn main() {
    println!("cargo:rerun-if-env-changed=REPIN_GIT_COMMIT");

    let package_version = env::var("CARGO_PKG_VERSION")
        .expect("Cargo must provide CARGO_PKG_VERSION to the CLI build script");
    let commit = env::var("REPIN_GIT_COMMIT")
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_string());
    let display_commit: String = commit.chars().take(12).collect();

    println!("cargo:rustc-env=REPIN_DISPLAY_VERSION=v{package_version}-{display_commit}");
}
