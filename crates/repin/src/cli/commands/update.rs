use crate::cli::client::DaemonClient;
use crate::product::{GITHUB_API_LATEST_RELEASE, GITHUB_BASE};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Deserialize, Debug)]
struct GitHubRelease {
    tag_name: String,
    #[serde(default)]
    assets: Vec<GitHubAsset>,
}

#[derive(Deserialize, Debug)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

fn parse_semver_components(ver: &str) -> Vec<u64> {
    let clean = ver.trim_start_matches('v').trim();
    clean
        .split(|c: char| !c.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse::<u64>().ok())
        .collect()
}

fn is_remote_newer(remote: &str, current: &str) -> bool {
    let r = parse_semver_components(remote);
    let c = parse_semver_components(current);
    r > c
}

fn fetch_latest_release() -> Result<GitHubRelease, String> {
    let current_version = env!("CARGO_PKG_VERSION");
    let response = ureq::get(GITHUB_API_LATEST_RELEASE)
        .set("User-Agent", &format!("repin/{current_version}"))
        .set("Accept", "application/vnd.github.v3+json")
        .call()
        .map_err(|e| format!("Failed to check for updates from GitHub: {e}"))?;

    let release: GitHubRelease = response
        .into_json()
        .map_err(|e| format!("Failed to parse release information: {e}"))?;

    Ok(release)
}

fn find_extracted_binary(extract_dir: &Path) -> Option<PathBuf> {
    let direct_bin = extract_dir.join(crate::product::BINARY_NAME);
    if direct_bin.is_file() {
        return Some(direct_bin);
    }

    if let Ok(entries) = fs::read_dir(extract_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let candidate = path.join(crate::product::BINARY_NAME);
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }

    None
}

pub fn execute_update(check_only: bool, force: bool) -> Result<(), String> {
    let current_version = env!("CARGO_PKG_VERSION");
    println!("Checking for updates (current: v{current_version})...");

    let release = fetch_latest_release()?;
    let remote_tag = &release.tag_name;
    let update_available = is_remote_newer(remote_tag, current_version);

    if check_only {
        println!();
        println!("Version Information:");
        println!("  • Installed: v{current_version}");
        println!("  • Latest:    {remote_tag}");
        println!();
        if update_available {
            println!("An update is available ({remote_tag}). Run `repin update` to upgrade.");
        } else {
            println!("Repin is up to date.");
        }
        return Ok(());
    }

    if !update_available && !force {
        println!("Repin is already up to date ({remote_tag}). Use `--force` to reinstall.");
        return Ok(());
    }

    println!("Updating Repin to {remote_tag}...");

    let download_url = release
        .assets
        .iter()
        .find(|asset| asset.name.contains(env!("REPIN_TARGET")) && asset.name.ends_with(".tar.gz"))
        .map(|asset| asset.browser_download_url.clone())
        .unwrap_or_else(|| {
            format!(
                "{GITHUB_BASE}/releases/download/{remote_tag}/repin-{remote_tag}-{}.tar.gz",
                env!("REPIN_TARGET")
            )
        });

    let temp_dir = tempfile::Builder::new()
        .prefix("repin-update-")
        .tempdir()
        .map_err(|e| format!("Failed to create temporary directory: {e}"))?;

    let tarball_path = temp_dir.path().join("repin-update.tar.gz");
    println!("Downloading release archive from {download_url}...");
    let resp = ureq::get(&download_url)
        .set("User-Agent", &format!("repin/{current_version}"))
        .call()
        .map_err(|e| format!("Failed to download update tarball: {e}"))?;

    let mut reader = resp.into_reader();
    let mut out_file = fs::File::create(&tarball_path)
        .map_err(|e| format!("Failed to create local archive file: {e}"))?;
    std::io::copy(&mut reader, &mut out_file)
        .map_err(|e| format!("Failed to save update archive: {e}"))?;
    drop(out_file);

    println!("Extracting update package...");
    let extract_dir = temp_dir.path().join("extracted");
    fs::create_dir_all(&extract_dir)
        .map_err(|e| format!("Failed to create extract directory: {e}"))?;

    let tar_status = std::process::Command::new("tar")
        .args([
            "-xzf",
            tarball_path.to_str().unwrap(),
            "-C",
            extract_dir.to_str().unwrap(),
        ])
        .status()
        .map_err(|e| format!("Failed to run tar command: {e}"))?;

    if !tar_status.success() {
        return Err(format!(
            "Extraction failed with exit code: {:?}",
            tar_status.code()
        ));
    }

    let new_binary = find_extracted_binary(&extract_dir).ok_or_else(|| {
        format!(
            "Could not locate extracted '{}' executable in update package",
            crate::product::BINARY_NAME
        )
    })?;

    println!("Stopping active daemon if running...");
    let _ = DaemonClient::stop_daemon(None);

    println!("Installing new binary and assets...");
    let install_status = std::process::Command::new(&new_binary)
        .arg("install")
        .status()
        .map_err(|e| format!("Failed to execute installer subprocess: {e}"))?;

    if !install_status.success() {
        return Err(format!(
            "Installation subprocess failed with exit code: {:?}",
            install_status.code()
        ));
    }

    println!("\nUpdate completed successfully! Repin is now at {remote_tag}.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_semver_comparison() {
        assert!(is_remote_newer("v0.2.0", "0.1.0"));
        assert!(is_remote_newer("0.1.1", "0.1.0"));
        assert!(is_remote_newer("v1.0.0", "0.9.9"));
        assert!(!is_remote_newer("v0.1.0", "0.1.0"));
        assert!(!is_remote_newer("v0.0.9", "0.1.0"));
    }
}
