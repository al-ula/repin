use crate::lease::FileLease;
use crate::registry::ContextRegistry;
use repin_core::ports::store::Store;
use repin_protocol::errors::ErrorCode;
use repin_protocol::ipc::{IpcMessage, IpcRequest, IpcResponse, IpcResponseEnvelope};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

pub struct DaemonServer {
    socket_path: PathBuf,
    lease: FileLease,
    registry: ContextRegistry,
    running: Arc<AtomicBool>,
}

impl DaemonServer {
    pub fn bind<P: AsRef<Path>>(runtime_dir: P) -> Result<Self, String> {
        let runtime_dir_buf = runtime_dir.as_ref().to_path_buf();
        let _ = std::fs::create_dir_all(&runtime_dir_buf);

        let lock_path = runtime_dir_buf.join("daemon.lock");
        let lease = FileLease::try_acquire(&lock_path)
            .map_err(|e| format!("failed to acquire daemon singleton lease: {e}"))?;

        let socket_path = runtime_dir_buf.join("daemon.sock");
        let _ = std::fs::remove_file(&socket_path);

        Ok(Self {
            socket_path,
            lease,
            registry: ContextRegistry::new(),
            running: Arc::new(AtomicBool::new(true)),
        })
    }

    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    pub fn lease(&self) -> &FileLease {
        &self.lease
    }

    pub fn run_loop(&self) -> Result<(), String> {
        let listener = UnixListener::bind(&self.socket_path)
            .map_err(|e| format!("failed to bind unix socket: {e}"))?;
        listener
            .set_nonblocking(true)
            .map_err(|e| format!("failed to set non-blocking: {e}"))?;

        while self.running.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((stream, _)) => {
                    let registry = self.registry.clone();
                    let running = self.running.clone();
                    thread::spawn(move || {
                        let _ = Self::handle_connection(stream, registry, running);
                    });
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(std::time::Duration::from_millis(50));
                }
                Err(e) => {
                    if !self.running.load(Ordering::SeqCst) {
                        break;
                    }
                    tracing::error!("error accepting connection: {e}");
                }
            }
        }

        let _ = std::fs::remove_file(&self.socket_path);
        Ok(())
    }

    fn handle_connection(
        stream: UnixStream,
        registry: ContextRegistry,
        running: Arc<AtomicBool>,
    ) -> Result<(), String> {
        let mut reader = BufReader::new(stream.try_clone().map_err(|e| e.to_string())?);
        let mut writer = stream;

        let mut bound_context = None;
        let mut line = String::new();

        while reader.read_line(&mut line).map_err(|e| e.to_string())? > 0 {
            let msg: IpcMessage = match serde_json::from_str(line.trim()) {
                Ok(m) => m,
                Err(e) => {
                    let resp = IpcResponseEnvelope {
                        request_id: 0,
                        body: IpcResponse::Error {
                            code: ErrorCode::InvalidQuery,
                            message: format!("malformed JSON frame: {e}"),
                        },
                    };
                    let resp_str = serde_json::to_string(&resp).unwrap();
                    let _ = writeln!(writer, "{resp_str}");
                    line.clear();
                    continue;
                }
            };

            let req_id = msg.request_id;
            let resp_body = match msg.body {
                IpcRequest::Handshake {
                    client_version: _,
                    project_db_path,
                } => match registry.get_or_load(project_db_path) {
                    Ok(ctx) => {
                        let is_writer = ctx.is_writer();
                        bound_context = Some(ctx);
                        IpcResponse::HandshakeOk {
                            protocol_version: 1,
                            daemon_version: env!("CARGO_PKG_VERSION").to_string(),
                            is_writer,
                        }
                    }
                    Err(e) => IpcResponse::Error {
                        code: ErrorCode::ProjectLeaseUnavailable,
                        message: e,
                    },
                },
                IpcRequest::Shutdown => {
                    running.store(false, Ordering::SeqCst);
                    IpcResponse::StatusOk {
                        graph_revision: repin_core::model::provenance::Revision::INITIAL,
                        node_count: 0,
                        edge_count: 0,
                    }
                }
                other => {
                    if let Some(ref ctx) = bound_context {
                        Self::dispatch_project_request(ctx, other)
                    } else {
                        IpcResponse::Error {
                            code: ErrorCode::CapabilityUnavailable,
                            message:
                                "connection must first complete handshake with project db path"
                                    .to_string(),
                        }
                    }
                }
            };

            let resp_env = IpcResponseEnvelope {
                request_id: req_id,
                body: resp_body,
            };

            let resp_str = serde_json::to_string(&resp_env).map_err(|e| e.to_string())?;
            writeln!(writer, "{resp_str}").map_err(|e| e.to_string())?;
            line.clear();
        }

        Ok(())
    }

    fn dispatch_project_request(
        ctx: &crate::context_handle::ProjectContext,
        req: IpcRequest,
    ) -> IpcResponse {
        let engine = ctx.engine();
        match req {
            IpcRequest::Status => {
                let (rev, node_count, edge_count) = if let Some(store) = engine.store() {
                    if let Ok(view) = store.read_view() {
                        (
                            view.revision()
                                .unwrap_or(repin_core::model::provenance::Revision::INITIAL),
                            view.node_count().unwrap_or(0),
                            view.edge_count().unwrap_or(0),
                        )
                    } else {
                        (repin_core::model::provenance::Revision::INITIAL, 0, 0)
                    }
                } else {
                    (repin_core::model::provenance::Revision::INITIAL, 0, 0)
                };

                IpcResponse::StatusOk {
                    graph_revision: rev,
                    node_count,
                    edge_count,
                }
            }
            IpcRequest::IndexAll => match engine.index_all_worktree() {
                Ok(count) => {
                    let rev = engine
                        .store()
                        .and_then(|s| s.read_view().ok())
                        .and_then(|v| v.revision().ok())
                        .unwrap_or(repin_core::model::provenance::Revision::INITIAL);
                    IpcResponse::IndexAllOk {
                        files_indexed: count,
                        revision: rev,
                    }
                }
                Err(e) => IpcResponse::Error {
                    code: ErrorCode::InternalError,
                    message: e,
                },
            },
            IpcRequest::SearchDirect {
                pattern,
                is_regex,
                paths: _,
                max_results,
            } => {
                let limit = max_results.unwrap_or(50);
                let env = engine.search_direct(&pattern, is_regex, limit);
                let val = serde_json::to_value(&env).unwrap_or_default();
                let deserialized = serde_json::from_value(val).unwrap();
                IpcResponse::SearchResult(deserialized)
            }
            IpcRequest::SearchGraph { query, max_results } => {
                let limit = max_results.unwrap_or(50);
                let env = engine.search_graph(&query, limit);
                let val = serde_json::to_value(&env).unwrap_or_default();
                let deserialized = serde_json::from_value(val).unwrap();
                IpcResponse::SearchResult(deserialized)
            }
            IpcRequest::SearchHybrid { query, max_results } => {
                let limit = max_results.unwrap_or(50);
                let env = engine.search_hybrid(&query, limit);
                let val = serde_json::to_value(&env).unwrap_or_default();
                let deserialized = serde_json::from_value(val).unwrap();
                IpcResponse::SearchResult(deserialized)
            }
            IpcRequest::InspectFile { path } => {
                let env = engine.inspect_file(&path);
                let val = serde_json::to_value(&env).unwrap_or_default();
                let deserialized = serde_json::from_value(val).unwrap();
                IpcResponse::InspectResult(deserialized)
            }
            IpcRequest::AtPosition { path, line, column } => {
                let pos = repin_core::line_index::Position::new(line, column);
                let env = engine.at_position(&path, pos);
                let val = serde_json::to_value(&env).unwrap_or_default();
                let deserialized = serde_json::from_value(val).unwrap();
                IpcResponse::PositionResult(deserialized)
            }
            IpcRequest::Entity { name_or_id } => {
                let env = engine.lookup_entity(&name_or_id);
                let val = serde_json::to_value(&env).unwrap_or_default();
                let deserialized = serde_json::from_value(val).unwrap();
                IpcResponse::EntityResult(deserialized)
            }
            IpcRequest::Neighbors {
                name_or_id,
                max_depth,
            } => {
                let depth = max_depth.unwrap_or(1);
                let env = engine.lookup_neighbors(&name_or_id, depth);
                let val = serde_json::to_value(&env).unwrap_or_default();
                let deserialized = serde_json::from_value(val).unwrap();
                IpcResponse::NeighborsResult(deserialized)
            }
            IpcRequest::Context {
                query,
                budget_bytes,
            } => {
                let budget =
                    budget_bytes.unwrap_or(repin_engine::ContextBuilder::DEFAULT_BYTE_BUDGET);
                let env = engine.assemble_context(&query, budget);
                let val = serde_json::to_value(&env).unwrap_or_default();
                let deserialized = serde_json::from_value(val).unwrap();
                IpcResponse::ContextResult(deserialized)
            }
            IpcRequest::ReviewContext {
                changed_since,
                budget_bytes,
            } => {
                let budget =
                    budget_bytes.unwrap_or(repin_engine::ContextBuilder::DEFAULT_BYTE_BUDGET);
                let env = engine.review_context(changed_since, budget);
                let val = serde_json::to_value(&env).unwrap_or_default();
                let deserialized = serde_json::from_value(val).unwrap();
                IpcResponse::ReviewResult(deserialized)
            }
            IpcRequest::SyncVcs => match engine.sync_vcs() {
                Ok(summary) => IpcResponse::UpdateOk {
                    revision: summary.revision,
                },
                Err(e) => IpcResponse::Error {
                    code: ErrorCode::InternalError,
                    message: e,
                },
            },
            IpcRequest::Eval => {
                let env = engine.evaluate_precision();
                let val = serde_json::to_value(&env).unwrap_or_default();
                let deserialized = serde_json::from_value(val).unwrap();
                IpcResponse::EvalResult(deserialized)
            }
            IpcRequest::Rerank {
                query,
                candidates,
                agent_cmd,
            } => {
                let env = engine.rerank_candidates(&query, &candidates, &agent_cmd);
                let val = serde_json::to_value(&env).unwrap_or_default();
                let deserialized = serde_json::from_value(val).unwrap();
                IpcResponse::RerankResult(deserialized)
            }
            IpcRequest::UpdateFiles { changes: _ } => {
                let _ = engine.index_all_worktree();
                IpcResponse::UpdateOk {
                    revision: repin_core::model::provenance::Revision(1),
                }
            }
            _ => IpcResponse::Error {
                code: ErrorCode::CapabilityUnsupported,
                message: "unsupported operation".to_string(),
            },
        }
    }
}
