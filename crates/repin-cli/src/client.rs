use repin_core::config::RepinConfig;
use repin_core::protocol::ipc::{
    BootstrapHandshake, IpcMessage, IpcRequest, IpcResponse, IpcResponseEnvelope,
};
use repin_core::protocol::{
    BOOTSTRAP_VERSION, PROTOCOL_MAX, PROTOCOL_MIN, PROTOCOL_STATE_LIFECYCLE,
};
use repin_product::{RuntimeLayout, default_runtime_layout};
use std::io::{BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};

pub struct DaemonClient {
    stream: UnixStream,
    reader: BufReader<UnixStream>,
    next_req_id: u64,
    selected_protocol: u32,
}

fn read_bounded_frame<R: Read>(reader: &mut R, max_bytes: usize) -> Result<Vec<u8>, String> {
    let mut frame = Vec::new();
    let mut byte = [0_u8; 1];
    loop {
        let read = reader.read(&mut byte).map_err(|e| e.to_string())?;
        if read == 0 {
            return Err("daemon closed an incomplete IPC frame".to_string());
        }
        if frame.len() >= max_bytes {
            return Err("daemon response exceeds configured frame limit".to_string());
        }
        frame.push(byte[0]);
        if byte[0] == b'\n' {
            return Ok(frame);
        }
    }
}

impl DaemonClient {
    pub fn default_runtime_dir() -> PathBuf {
        default_runtime_layout().base
    }

    pub fn connect_existing(runtime_dir: Option<&Path>) -> Result<Self, String> {
        let rt_dir = runtime_dir
            .map(|p| p.to_path_buf())
            .unwrap_or_else(Self::default_runtime_dir);
        let socket_path = RuntimeLayout::at_base(&rt_dir).socket_path;

        if !socket_path.exists() {
            return Err(format!(
                "No running Repin daemon found (socket not present at {})",
                socket_path.display()
            ));
        }

        let stream = UnixStream::connect(&socket_path)
            .map_err(|e| format!("Failed to connect to daemon socket: {e}"))?;

        let reader_stream = stream.try_clone().map_err(|e| e.to_string())?;
        stream
            .set_read_timeout(Some(std::time::Duration::from_millis(
                repin_core::protocol::BOOTSTRAP_DEADLINE_MS,
            )))
            .map_err(|e| e.to_string())?;
        let mut client = Self {
            stream,
            reader: BufReader::new(reader_stream),
            next_req_id: 1,
            selected_protocol: PROTOCOL_MIN,
        };
        client.negotiate_bootstrap()?;
        Ok(client)
    }

    /// Negotiated but unbound connection to an already-running daemon. Used by
    /// state lifecycle requests, which precede project binding (ADR-026).
    pub fn connect_existing_unbound(runtime_dir: Option<&Path>) -> Result<Self, String> {
        Self::connect_existing(runtime_dir)
    }

    /// Negotiated but unbound connection, starting a daemon when none is
    /// listening. State lifecycle requests are issued before project binding.
    pub fn connect_or_start_unbound() -> Result<Self, String> {
        let runtime_dir = Self::default_runtime_dir();
        let socket_path = RuntimeLayout::at_base(&runtime_dir).socket_path;

        let stream = match UnixStream::connect(&socket_path) {
            Ok(s) => s,
            Err(_) => {
                let current_exe =
                    std::env::current_exe().unwrap_or_else(|_| PathBuf::from("repin"));
                let _ = std::process::Command::new(&current_exe)
                    .arg("daemon")
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn();

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

        stream
            .set_read_timeout(Some(std::time::Duration::from_millis(
                repin_core::protocol::BOOTSTRAP_DEADLINE_MS,
            )))
            .map_err(|e| e.to_string())?;
        let reader_stream = stream.try_clone().map_err(|e| e.to_string())?;
        let mut client = Self {
            stream,
            reader: BufReader::new(reader_stream),
            next_req_id: 1,
            selected_protocol: PROTOCOL_MIN,
        };
        client.negotiate_bootstrap()?;
        Ok(client)
    }

    pub fn connect_or_start(db_path: &Path, resolved_config: &RepinConfig) -> Result<Self, String> {
        let mut client = Self::connect_or_start_unbound()?;

        // Project binding handshake follows successful bootstrap negotiation.
        let resp = client.send_request(IpcRequest::Handshake {
            client_version: env!("CARGO_PKG_VERSION").to_string(),
            project_db_path: db_path.display().to_string(),
            resolved_config: Some(resolved_config.clone()),
        })?;

        match resp {
            IpcResponse::HandshakeOk { .. } => Ok(client),
            IpcResponse::Error { code, message } => {
                Err(format!("handshake failed: {:?}: {}", code, message))
            }
            _ => Err("unexpected handshake response".to_string()),
        }
    }

    fn negotiate_bootstrap(&mut self) -> Result<(), String> {
        let bootstrap = self.send_request(IpcRequest::Bootstrap(BootstrapHandshake {
            bootstrap_version: BOOTSTRAP_VERSION,
            protocol_min: PROTOCOL_MIN,
            protocol_max: PROTOCOL_MAX,
            client_package_version: env!("CARGO_PKG_VERSION").to_string(),
            client_build_id: option_env!("REPIN_BUILD_ID").map(str::to_owned),
            replacement_request: false,
        }))?;
        match bootstrap {
            IpcResponse::BootstrapOk(ok) => {
                self.selected_protocol = ok.selected_protocol;
            }
            IpcResponse::BootstrapRejected(rejected) => {
                return Err(format!(
                    "bootstrap negotiation failed: {} (daemon supports {}..={}, replacement_allowed={})",
                    rejected.message,
                    rejected.daemon_protocol_min,
                    rejected.daemon_protocol_max,
                    rejected.replacement_allowed
                ));
            }
            other => return Err(format!("bootstrap negotiation failed: {other:?}")),
        }
        self.stream
            .set_read_timeout(None)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn selected_protocol(&self) -> u32 {
        self.selected_protocol
    }

    /// Whether the negotiated protocol carries the daemon-mediated state
    /// lifecycle requests (ADR-026). An older daemon still overlaps at
    /// protocol 1, so a client must check before sending them.
    pub fn supports_state_lifecycle(&self) -> bool {
        self.selected_protocol >= PROTOCOL_STATE_LIFECYCLE
    }

    pub fn stop_daemon(runtime_dir: Option<&Path>) -> Result<(), String> {
        let rt_dir = runtime_dir
            .map(|p| p.to_path_buf())
            .unwrap_or_else(Self::default_runtime_dir);
        let socket_path = RuntimeLayout::at_base(&rt_dir).socket_path;

        if !socket_path.exists() {
            println!("No active daemon found at {}", socket_path.display());
            return Ok(());
        }

        let mut client = match Self::connect_existing(Some(&rt_dir)) {
            Ok(c) => c,
            Err(_) => {
                println!(
                    "Daemon socket at {} is unavailable; leaving it for a lease-owning candidate to clean up.",
                    socket_path.display()
                );
                return Ok(());
            }
        };

        println!(
            "Sending shutdown signal to daemon at {}...",
            socket_path.display()
        );
        let _ = client.send_request(IpcRequest::Shutdown);

        for _ in 0..40 {
            if !socket_path.exists() {
                println!("Daemon shut down successfully.");
                return Ok(());
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }

        let _ = std::fs::remove_file(&socket_path);
        println!("Daemon stopped.");
        Ok(())
    }

    pub fn restart_daemon(
        runtime_dir: Option<&Path>,
        db_path: &Path,
        resolved_config: &RepinConfig,
    ) -> Result<Self, String> {
        let rt_dir = runtime_dir
            .map(|p| p.to_path_buf())
            .unwrap_or_else(Self::default_runtime_dir);

        println!("Stopping existing daemon if active...");
        let _ = Self::stop_daemon(Some(&rt_dir));

        std::thread::sleep(std::time::Duration::from_millis(100));

        println!("Starting new daemon instance...");
        let client = Self::connect_or_start(db_path, resolved_config)?;
        println!("Daemon restarted and connection established.");
        Ok(client)
    }

    pub fn request_replacement(&mut self) -> Result<(), String> {
        match self.send_request(IpcRequest::RequestReplacement)? {
            IpcResponse::ReplacementAccepted => Ok(()),
            IpcResponse::Error { code, message } => {
                Err(format!("daemon replacement rejected: {code:?}: {message}"))
            }
            response => Err(format!("unexpected replacement response: {response:?}")),
        }
    }

    /// Request replacement through the stable bootstrap envelope when this
    /// client advertises a strictly newer protocol range.
    pub fn request_incompatible_replacement(
        runtime_dir: Option<&Path>,
        protocol_min: u32,
        protocol_max: u32,
    ) -> Result<(), String> {
        let rt_dir = runtime_dir
            .map(Path::to_path_buf)
            .unwrap_or_else(Self::default_runtime_dir);
        let socket_path = RuntimeLayout::at_base(&rt_dir).socket_path;
        let stream = UnixStream::connect(&socket_path)
            .map_err(|e| format!("failed to connect to daemon endpoint: {e}"))?;
        stream
            .set_read_timeout(Some(std::time::Duration::from_millis(
                repin_core::protocol::BOOTSTRAP_DEADLINE_MS,
            )))
            .map_err(|e| e.to_string())?;
        let reader_stream = stream.try_clone().map_err(|e| e.to_string())?;
        let mut client = Self {
            stream,
            reader: BufReader::new(reader_stream),
            next_req_id: 1,
            selected_protocol: PROTOCOL_MIN,
        };
        let accepted = match client.send_request(IpcRequest::Bootstrap(BootstrapHandshake {
            bootstrap_version: BOOTSTRAP_VERSION,
            protocol_min,
            protocol_max,
            client_package_version: env!("CARGO_PKG_VERSION").to_string(),
            client_build_id: option_env!("REPIN_BUILD_ID").map(str::to_owned),
            replacement_request: true,
        }))? {
            IpcResponse::ReplacementAccepted => true,
            IpcResponse::BootstrapRejected(rejected) => {
                return Err(format!(
                    "daemon replacement rejected: {} (replacement_allowed={})",
                    rejected.message, rejected.replacement_allowed
                ));
            }
            response => return Err(format!("unexpected replacement response: {response:?}")),
        };
        debug_assert!(accepted);

        // Dropping the acknowledged bootstrap connection lets the old daemon
        // complete its bounded drain before releasing the singleton lease.
        drop(client);
        for _ in 0..40 {
            if !socket_path.exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        if socket_path.exists() {
            return Err(
                "old daemon did not release its socket after replacement acknowledgement"
                    .to_string(),
            );
        }

        let current_exe = std::env::current_exe()
            .map_err(|e| format!("failed to locate current executable: {e}"))?;
        std::process::Command::new(current_exe)
            .arg("daemon")
            .arg("--runtime-dir")
            .arg(&rt_dir)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| format!("failed to launch successor daemon: {e}"))?;

        for _ in 0..40 {
            if socket_path.exists() && UnixStream::connect(&socket_path).is_ok() {
                return Ok(());
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        Err("successor daemon did not become ready within the bounded retry budget".to_string())
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

        let frame = read_bounded_frame(&mut self.reader, repin_core::protocol::MAX_FRAME_BYTES)?;
        let resp_env: IpcResponseEnvelope =
            serde_json::from_slice(&frame).map_err(|e| e.to_string())?;

        Ok(resp_env.body)
    }
}
