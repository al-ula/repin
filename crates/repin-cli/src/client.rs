use repin_protocol::ipc::{IpcMessage, IpcRequest, IpcResponse, IpcResponseEnvelope};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};

pub struct DaemonClient {
    stream: UnixStream,
    reader: BufReader<UnixStream>,
    next_req_id: u64,
}

impl DaemonClient {
    pub fn default_runtime_dir() -> PathBuf {
        if let Ok(xdg) = std::env::var("XDG_RUNTIME_DIR") {
            PathBuf::from(xdg).join("repin")
        } else {
            std::env::temp_dir().join("repin-runtime")
        }
    }

    pub fn connect_existing(runtime_dir: Option<&Path>) -> Result<Self, String> {
        let rt_dir = runtime_dir
            .map(|p| p.to_path_buf())
            .unwrap_or_else(Self::default_runtime_dir);
        let socket_path = rt_dir.join("daemon.sock");

        if !socket_path.exists() {
            return Err(format!(
                "No running Repin daemon found (socket not present at {})",
                socket_path.display()
            ));
        }

        let stream = UnixStream::connect(&socket_path)
            .map_err(|e| format!("Failed to connect to daemon socket: {e}"))?;

        let reader_stream = stream.try_clone().map_err(|e| e.to_string())?;
        Ok(Self {
            stream,
            reader: BufReader::new(reader_stream),
            next_req_id: 1,
        })
    }

    pub fn connect_or_start(db_path: &Path) -> Result<Self, String> {
        let runtime_dir = Self::default_runtime_dir();
        let socket_path = runtime_dir.join("daemon.sock");

        let stream = match UnixStream::connect(&socket_path) {
            Ok(s) => s,
            Err(_) => {
                let _ = std::fs::remove_file(&socket_path);

                // Spawn persistent background daemon subprocess
                let current_exe =
                    std::env::current_exe().unwrap_or_else(|_| PathBuf::from("repin"));
                let _ = std::process::Command::new(&current_exe)
                    .arg("daemon")
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn();

                // Bounded wait for socket
                let mut connected = None;
                for _ in 0..30 {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    if let Ok(s) = UnixStream::connect(&socket_path) {
                        connected = Some(s);
                        break;
                    }
                }
                connected.ok_or_else(|| "timed out connecting to repin daemon".to_string())?
            }
        };

        let reader_stream = stream.try_clone().map_err(|e| e.to_string())?;
        let mut client = Self {
            stream,
            reader: BufReader::new(reader_stream),
            next_req_id: 1,
        };

        // Handshake
        let resp = client.send_request(IpcRequest::Handshake {
            client_version: env!("CARGO_PKG_VERSION").to_string(),
            project_db_path: db_path.display().to_string(),
        })?;

        match resp {
            IpcResponse::HandshakeOk { .. } => Ok(client),
            IpcResponse::Error { code, message } => {
                Err(format!("handshake failed: {:?}: {}", code, message))
            }
            _ => Err("unexpected handshake response".to_string()),
        }
    }

    pub fn stop_daemon(runtime_dir: Option<&Path>) -> Result<(), String> {
        let rt_dir = runtime_dir
            .map(|p| p.to_path_buf())
            .unwrap_or_else(Self::default_runtime_dir);
        let socket_path = rt_dir.join("daemon.sock");

        if !socket_path.exists() {
            println!("No active daemon found at {}", socket_path.display());
            return Ok(());
        }

        let mut client = match Self::connect_existing(Some(&rt_dir)) {
            Ok(c) => c,
            Err(_) => {
                let _ = std::fs::remove_file(&socket_path);
                println!("Removed stale daemon socket at {}.", socket_path.display());
                return Ok(());
            }
        };

        println!(
            "Sending shutdown signal to daemon at {}...",
            socket_path.display()
        );
        let _ = client.send_request(IpcRequest::Shutdown);

        // Bounded wait for socket removal
        for _ in 0..40 {
            if !socket_path.exists() {
                println!("Daemon shut down successfully.");
                return Ok(());
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }

        // Clean up socket if still lingering
        let _ = std::fs::remove_file(&socket_path);
        println!("Daemon stopped.");
        Ok(())
    }

    pub fn restart_daemon(runtime_dir: Option<&Path>, db_path: &Path) -> Result<Self, String> {
        let rt_dir = runtime_dir
            .map(|p| p.to_path_buf())
            .unwrap_or_else(Self::default_runtime_dir);

        println!("Stopping existing daemon if active...");
        let _ = Self::stop_daemon(Some(&rt_dir));

        std::thread::sleep(std::time::Duration::from_millis(100));

        println!("Starting new daemon instance...");
        let client = Self::connect_or_start(db_path)?;
        println!("Daemon restarted and connection established.");
        Ok(client)
    }

    pub fn send_request(&mut self, request: IpcRequest) -> Result<IpcResponse, String> {
        let req_id = self.next_req_id;
        self.next_req_id += 1;

        let msg = IpcMessage {
            request_id: req_id,
            body: request,
        };

        let msg_str = serde_json::to_string(&msg).map_err(|e| e.to_string())?;
        writeln!(self.stream, "{msg_str}").map_err(|e| e.to_string())?;

        let mut line = String::new();
        self.reader
            .read_line(&mut line)
            .map_err(|e| e.to_string())?;

        let resp_env: IpcResponseEnvelope =
            serde_json::from_str(line.trim()).map_err(|e| e.to_string())?;

        Ok(resp_env.body)
    }
}
