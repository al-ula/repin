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

    pub fn connect_or_start(db_path: &Path) -> Result<Self, String> {
        let runtime_dir = Self::default_runtime_dir();
        let socket_path = runtime_dir.join("daemon.sock");

        let stream = match UnixStream::connect(&socket_path) {
            Ok(s) => s,
            Err(_) => {
                // Spawn daemon in background or in-process thread for PoC
                let rt_clone = runtime_dir.clone();
                std::thread::spawn(move || {
                    if let Ok(server) = repin_daemon::DaemonServer::bind(rt_clone) {
                        let _ = server.run_loop();
                    }
                });

                // Bounded wait for socket
                let mut connected = None;
                for _ in 0..20 {
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
