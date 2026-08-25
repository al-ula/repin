use crate::cli::client::DaemonClient;
use crate::product::RuntimeLayout;
use repin_core::config::RepinConfig;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub fn execute_daemon_run(
    runtime_dir: Option<PathBuf>,
    idle_timeout: Option<u64>,
) -> Result<(), String> {
    let rt_dir = runtime_dir.unwrap_or_else(DaemonClient::default_runtime_dir);
    println!(
        "Starting Repin daemon in foreground on {}",
        rt_dir.display()
    );
    let override_timeout = idle_timeout.map(|s| {
        if s == 0 {
            None
        } else {
            Some(Duration::from_secs(s))
        }
    });
    let server = crate::daemon::DaemonServer::bind(rt_dir, override_timeout)
        .map_err(|e| format!("Failed to bind daemon: {e}"))?;
    server
        .run_loop()
        .map_err(|e| format!("Daemon error: {e}"))?;
    Ok(())
}

pub fn execute_daemon_stop(runtime_dir: Option<&Path>) -> Result<(), String> {
    DaemonClient::stop_daemon(runtime_dir)
}

pub fn execute_daemon_restart(
    runtime_dir: Option<&Path>,
    db_path: &Path,
    resolved_config: &RepinConfig,
) -> Result<(), String> {
    DaemonClient::restart_daemon(runtime_dir, db_path, resolved_config)?;
    Ok(())
}

pub fn execute_daemon_status(runtime_dir: Option<&Path>) -> Result<(), String> {
    let rt_dir = runtime_dir
        .map(|p| p.to_path_buf())
        .unwrap_or_else(DaemonClient::default_runtime_dir);
    let socket_path = RuntimeLayout::at_base(&rt_dir).socket_path;

    if socket_path.exists() {
        match DaemonClient::connect_existing(Some(&rt_dir)) {
            Ok(_) => {
                println!(
                    "Repin daemon is RUNNING (active socket at {})",
                    socket_path.display()
                );
            }
            Err(e) => {
                println!(
                    "Socket exists at {} but connection failed: {e}",
                    socket_path.display()
                );
            }
        }
    } else {
        println!(
            "Repin daemon is NOT RUNNING (no socket at {})",
            socket_path.display()
        );
    }

    Ok(())
}
