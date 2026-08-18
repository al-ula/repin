//! Disposable Linux-only F8 runtime daemon and project-context experiment.
//!
//! This binary is evidence code. It starts the same binary as detached daemon
//! candidates, elects one candidate with an OS-backed flock, serves a bounded
//! pathname Unix-domain socket, and exercises isolated project contexts. It is
//! deliberately not production runtime code.

use libc::{EAGAIN, LOCK_EX, LOCK_NB, LOCK_UN, flock};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};
use std::env;
use std::error::Error;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tempfile::tempdir;

type AppResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

const RUN_ID: &str = "foundation-f8-runtime-20260819";
const PROTOCOL_VERSION: u64 = 1;
const MAX_FRAME: usize = 8 * 1024;
const IDLE_ADVANCE_MS: u64 = 600_000;

#[derive(Debug, Serialize)]
struct Manifest {
    run_id: &'static str,
    experiment: &'static str,
    lifecycle_stage: &'static str,
    platform_scope: &'static str,
    target: String,
    os: String,
    architecture: String,
    rustc: String,
    cargo: String,
    fixture_seed: &'static str,
    protocol_version: u64,
    max_frame_bytes: usize,
    virtual_idle_advance_ms: u64,
    runtime_layout: BTreeMap<String, String>,
    source_revision: String,
}

#[derive(Debug, Serialize)]
struct CaseObservation {
    id: String,
    expected: String,
    observed: String,
    outcome: String,
    details: Value,
}

#[derive(Debug, Serialize)]
struct Report {
    experiment: &'static str,
    run_id: &'static str,
    status: String,
    overall_outcome: String,
    decision_status: &'static str,
    hard_blocker: bool,
    cases: Vec<CaseObservation>,
    case_ids: Vec<String>,
    measurements: Vec<Value>,
    notes: Vec<String>,
    artifacts: Vec<String>,
}

#[derive(Debug)]
struct Lease {
    file: File,
}

impl Lease {
    fn acquire(path: &Path) -> AppResult<Option<Self>> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
        set_private(path, 0o600)?;
        let result = unsafe { flock(file.as_raw_fd(), LOCK_EX | LOCK_NB) };
        if result == 0 {
            let mut file = file;
            file.set_len(0)?;
            writeln!(file, "pid={}", std::process::id())?;
            file.flush()?;
            Ok(Some(Self { file }))
        } else {
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(EAGAIN) {
                Ok(None)
            } else {
                Err(error.into())
            }
        }
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        unsafe {
            let _ = flock(self.file.as_raw_fd(), LOCK_UN);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
struct FileIdentity {
    device: u64,
    inode: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum GraphStatus {
    Valid,
    Invalid,
    Newer,
}

impl GraphStatus {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Valid => "VALID",
            Self::Invalid => "PROJECT_STATE_INVALID",
            Self::Newer => "PROJECT_STATE_NEWER",
        }
    }
}

#[derive(Debug)]
struct ProjectInfo {
    root: PathBuf,
    state_dir: PathBuf,
    spelled_graph: PathBuf,
    database: PathBuf,
    identity: FileIdentity,
    graph_status: GraphStatus,
}

#[derive(Debug)]
struct Context {
    id: String,
    root: PathBuf,
    state_dir: PathBuf,
    database: PathBuf,
    identity: FileIdentity,
    graph_status: GraphStatus,
    revision: u64,
    clients: usize,
    authoritative: bool,
    watcher_registered: bool,
    idle_since: Option<u64>,
    closed: bool,
    writer_lease: Option<Lease>,
}

#[derive(Debug)]
struct DaemonState {
    now_ms: u64,
    next_context_id: u64,
    active_connections: usize,
    had_context: bool,
    shutdown_requested: bool,
    contexts: HashMap<String, Context>,
}

fn main() -> AppResult<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("daemon") => daemon_main(&args),
        Some("hold-lock") => hold_lock_main(&args),
        Some("run") => run_experiment(&required_arg(&args, "--output")?),
        _ => {
            eprintln!(
                "usage: repin-f8-spike run --output DIR | daemon --runtime DIR | hold-lock --path FILE"
            );
            Err("invalid command".into())
        }
    }
}

fn required_arg(args: &[String], name: &str) -> AppResult<String> {
    let index = args
        .iter()
        .position(|arg| arg == name)
        .ok_or_else(|| format!("missing {name}"))?;
    args.get(index + 1)
        .cloned()
        .ok_or_else(|| format!("missing value for {name}").into())
}

fn set_private(path: &Path, mode: u32) -> AppResult<()> {
    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(mode);
    fs::set_permissions(path, permissions)?;
    Ok(())
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_vec_pretty(value)?)?;
    Ok(())
}

fn command_version(program: &str) -> String {
    Command::new(program)
        .arg("--version")
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .unwrap_or_else(|_| "unavailable".into())
}

fn source_revision() -> String {
    let output = Command::new("git")
        .args(["status", "--short", "--branch"])
        .output()
        .map(|output| output.stdout)
        .unwrap_or_default();
    let digest = blake3::hash(&output);
    digest.to_hex().to_string()
}

fn manifest(runtime: &Path) -> Manifest {
    let mut layout = BTreeMap::new();
    layout.insert(
        "socket".into(),
        runtime.join("daemon.sock").display().to_string(),
    );
    layout.insert(
        "daemon_lease".into(),
        runtime.join("daemon.lock").display().to_string(),
    );
    layout.insert(
        "candidate_events".into(),
        runtime.join("candidates").display().to_string(),
    );
    Manifest {
        run_id: RUN_ID,
        experiment: "F8",
        lifecycle_stage: "experimentation",
        platform_scope: "Linux x86_64/glibc PoC",
        target: env::var("TARGET").unwrap_or_else(|_| env::consts::ARCH.into()),
        os: env::consts::OS.into(),
        architecture: env::consts::ARCH.into(),
        rustc: command_version("rustc"),
        cargo: command_version("cargo"),
        fixture_seed: "repin-f8-runtime-1",
        protocol_version: PROTOCOL_VERSION,
        max_frame_bytes: MAX_FRAME,
        virtual_idle_advance_ms: IDLE_ADVANCE_MS,
        runtime_layout: layout,
        source_revision: source_revision(),
    }
}

fn file_identity(path: &Path) -> AppResult<FileIdentity> {
    let metadata = fs::metadata(path)?;
    Ok(FileIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    })
}

fn graph_status(path: &Path) -> AppResult<GraphStatus> {
    let mut file = File::open(path)?;
    let mut bytes = Vec::new();
    Read::by_ref(&mut file).take(256).read_to_end(&mut bytes)?;
    let text = String::from_utf8_lossy(&bytes);
    if text.starts_with("schema=999") {
        Ok(GraphStatus::Newer)
    } else if text.starts_with("INVALID") {
        Ok(GraphStatus::Invalid)
    } else if text.starts_with("schema=1") {
        Ok(GraphStatus::Valid)
    } else {
        Ok(GraphStatus::Invalid)
    }
}

fn read_revision(path: &Path) -> u64 {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| {
            text.lines()
                .find_map(|line| line.strip_prefix("revision=")?.parse().ok())
        })
        .unwrap_or(0)
}

fn graph_pair(root: &Path) -> Result<ProjectInfo, String> {
    let root = fs::canonicalize(root).map_err(|_| "PROJECT_NOT_INITIALIZED".to_string())?;
    let state_dir = root.join(".repin");
    let state_metadata =
        fs::symlink_metadata(&state_dir).map_err(|_| "PROJECT_NOT_INITIALIZED".to_string())?;
    if !state_metadata.is_dir() || state_metadata.file_type().is_symlink() {
        return Err("PROJECT_NOT_INITIALIZED".into());
    }
    let spelled_graph = state_dir.join("graph.redb");
    let graph_metadata =
        fs::symlink_metadata(&spelled_graph).map_err(|_| "PROJECT_NOT_INITIALIZED".to_string())?;
    if !graph_metadata.is_file() && !graph_metadata.file_type().is_symlink() {
        return Err("PROJECT_NOT_INITIALIZED".into());
    }
    let database =
        fs::canonicalize(&spelled_graph).map_err(|_| "PROJECT_STATE_INVALID".to_string())?;
    if !fs::metadata(&database)
        .map_err(|_| "PROJECT_STATE_INVALID".to_string())?
        .is_file()
    {
        return Err("PROJECT_STATE_INVALID".into());
    }
    let identity = file_identity(&database).map_err(|_| "PROJECT_STATE_INVALID".to_string())?;
    let status = graph_status(&database).map_err(|_| "PROJECT_STATE_INVALID".to_string())?;
    Ok(ProjectInfo {
        root,
        state_dir,
        spelled_graph,
        database,
        identity,
        graph_status: status,
    })
}

fn discover_path(path: &Path) -> Result<ProjectInfo, String> {
    let supplied = fs::canonicalize(path).map_err(|_| "PROJECT_NOT_INITIALIZED".to_string())?;
    let mut current = if fs::metadata(&supplied)
        .map_err(|_| "PROJECT_NOT_INITIALIZED".to_string())?
        .is_file()
    {
        supplied
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| "PROJECT_NOT_INITIALIZED".to_string())?
    } else {
        supplied
    };
    loop {
        match graph_pair(&current) {
            Ok(info) => return Ok(info),
            Err(error) if error == "PROJECT_NOT_INITIALIZED" => {}
            Err(error) => return Err(error),
        }
        if !current.pop() {
            return Err("PROJECT_NOT_INITIALIZED".into());
        }
    }
}

fn selector_info(selector: &Value) -> Result<ProjectInfo, String> {
    match selector.get("kind").and_then(Value::as_str) {
        Some("discover") => selector
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| "PROJECT_NOT_INITIALIZED".to_string())
            .and_then(|path| discover_path(Path::new(path))),
        Some("root") => selector
            .get("root")
            .and_then(Value::as_str)
            .ok_or_else(|| "PROJECT_NOT_INITIALIZED".to_string())
            .and_then(|root| graph_pair(Path::new(root))),
        _ => Err("PROJECT_NOT_INITIALIZED".into()),
    }
}

fn safe_relative(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let path = Path::new(relative);
    if path.is_absolute() {
        return Err("DIRECT_RETRIEVAL_REJECTED".into());
    }
    for component in path.components() {
        if !matches!(component, Component::Normal(_)) {
            return Err("DIRECT_RETRIEVAL_REJECTED".into());
        }
    }
    let candidate = root.join(path);
    let canonical =
        fs::canonicalize(&candidate).map_err(|_| "DIRECT_RETRIEVAL_REJECTED".to_string())?;
    if !canonical.starts_with(root) {
        return Err("DIRECT_RETRIEVAL_REJECTED".into());
    }
    Ok(canonical)
}

fn record_candidate(runtime: &Path, outcome: &str, details: Value) -> AppResult<()> {
    let path = runtime
        .join("candidates")
        .join(format!("{}-{}.json", std::process::id(), outcome));
    write_json(
        &path,
        &json!({
            "pid": std::process::id(),
            "outcome": outcome,
            "details": details,
        }),
    )
}

#[derive(Debug, PartialEq, Eq)]
enum SocketProbe {
    Live,
    Stale,
    Malformed,
}

fn probe_socket(path: &Path) -> SocketProbe {
    let mut stream = match UnixStream::connect(path) {
        Ok(stream) => stream,
        Err(_) => return SocketProbe::Stale,
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(250)));
    if send_frame(
        &mut stream,
        &json!({"op":"ready","version":PROTOCOL_VERSION}),
    )
    .is_err()
    {
        return SocketProbe::Malformed;
    }
    match read_frame(&mut stream) {
        Ok(Some(frame)) => match serde_json::from_str::<Value>(&frame) {
            Ok(value) if value.get("ready") == Some(&Value::Bool(true)) => SocketProbe::Live,
            _ => SocketProbe::Malformed,
        },
        _ => SocketProbe::Malformed,
    }
}

fn daemon_main(args: &[String]) -> AppResult<()> {
    let runtime = PathBuf::from(required_arg(args, "--runtime")?);
    fs::create_dir_all(&runtime)?;
    set_private(&runtime, 0o700)?;
    fs::create_dir_all(runtime.join("candidates"))?;
    let socket = runtime.join("daemon.sock");
    let lease_path = runtime.join("daemon.lock");
    let lease = match Lease::acquire(&lease_path)? {
        Some(lease) => lease,
        None => {
            record_candidate(&runtime, "lease_unavailable", json!({}))?;
            return Ok(());
        }
    };

    if socket.exists() {
        match probe_socket(&socket) {
            SocketProbe::Live => {
                record_candidate(&runtime, "live_daemon", json!({}))?;
                drop(lease);
                return Ok(());
            }
            SocketProbe::Stale => {
                fs::remove_file(&socket)?;
                record_candidate(&runtime, "stale_socket_repaired", json!({}))?;
            }
            SocketProbe::Malformed => {
                fs::remove_file(&socket)?;
                record_candidate(&runtime, "malformed_socket_repaired", json!({}))?;
            }
        }
    }

    let listener = UnixListener::bind(&socket)?;
    set_private(&socket, 0o600)?;
    if args.iter().any(|arg| arg == "--die-before-ready") {
        record_candidate(&runtime, "died_before_ready", json!({}))?;
        drop(listener);
        drop(lease);
        return Ok(());
    }
    listener.set_nonblocking(true)?;
    record_candidate(&runtime, "winner", json!({"socket":socket}))?;
    let shared = Arc::new(Mutex::new(DaemonState {
        now_ms: 0,
        next_context_id: 1,
        active_connections: 0,
        had_context: false,
        shutdown_requested: false,
        contexts: HashMap::new(),
    }));

    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                let state = Arc::clone(&shared);
                thread::spawn(move || handle_connection(stream, state));
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(5));
            }
            Err(error) => return Err(error.into()),
        }
        let should_exit = {
            let state = shared.lock().map_err(|_| "daemon state poisoned")?;
            (state.shutdown_requested || state.had_context)
                && state.contexts.is_empty()
                && state.active_connections == 0
        };
        if should_exit {
            break;
        }
    }
    drop(listener);
    let _ = fs::remove_file(&socket);
    record_candidate(&runtime, "exit", json!({}))?;
    drop(lease);
    Ok(())
}

fn hold_lock_main(args: &[String]) -> AppResult<()> {
    let path = PathBuf::from(required_arg(args, "--path")?);
    let lease = Lease::acquire(&path)?.ok_or("PROJECT_LEASE_UNAVAILABLE")?;
    fs::write(
        path.with_extension("held"),
        format!("pid={}\n", std::process::id()),
    )?;
    let _lease = lease;
    loop {
        thread::sleep(Duration::from_secs(60));
    }
}

fn send_frame(stream: &mut UnixStream, value: &Value) -> io::Result<()> {
    let mut bytes = serde_json::to_vec(value).map_err(io::Error::other)?;
    if bytes.len() > MAX_FRAME {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "FRAME_TOO_LARGE",
        ));
    }
    bytes.push(b'\n');
    stream.write_all(&bytes)
}

fn read_frame<R: Read>(reader: &mut R) -> io::Result<Option<String>> {
    let mut bytes = Vec::with_capacity(256);
    loop {
        let mut byte = [0_u8; 1];
        let count = reader.read(&mut byte)?;
        if count == 0 {
            if bytes.is_empty() {
                return Ok(None);
            }
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "incomplete frame",
            ));
        }
        if byte[0] == b'\n' {
            return String::from_utf8(bytes)
                .map(Some)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid utf8"));
        }
        if bytes.len() >= MAX_FRAME {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "FRAME_TOO_LARGE",
            ));
        }
        bytes.push(byte[0]);
    }
}

fn send_error(stream: &mut UnixStream, request_id: Option<&Value>, error: &str) {
    let mut response = json!({"ok":false,"error":error});
    if let Some(request_id) = request_id {
        response["request_id"] = request_id.clone();
    }
    let _ = send_frame(stream, &response);
}

fn request_id(value: &Value) -> Option<&Value> {
    value.get("request_id")
}

fn context_snapshot(context: &Context) -> Value {
    json!({
        "context_id": context.id,
        "root": context.root,
        "state_dir": context.state_dir,
        "database": context.database,
        "revision": context.revision,
        "clients": context.clients,
        "authoritative": context.authoritative,
        "watcher_registered": context.watcher_registered,
        "idle_since": context.idle_since,
        "closed": context.closed,
        "graph_status": context.graph_status.as_str(),
    })
}

fn snapshot(state: &DaemonState) -> Value {
    let mut contexts: Vec<Value> = state.contexts.values().map(context_snapshot).collect();
    contexts.sort_by(|left, right| left["database"].as_str().cmp(&right["database"].as_str()));
    json!({
        "now_ms": state.now_ms,
        "active_connections": state.active_connections,
        "contexts": contexts,
    })
}

fn handle_connection(mut stream: UnixStream, shared: Arc<Mutex<DaemonState>>) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    if let Ok(mut state) = shared.lock() {
        state.active_connections += 1;
    }
    let mut bound_database: Option<String> = None;
    loop {
        let frame = match read_frame(&mut stream) {
            Ok(Some(frame)) => frame,
            Ok(None) => break,
            Err(error) => {
                let code = if error.to_string() == "FRAME_TOO_LARGE" {
                    "FRAME_TOO_LARGE"
                } else {
                    "PROTOCOL_MISMATCH"
                };
                send_error(&mut stream, None, code);
                break;
            }
        };
        let value = match serde_json::from_str::<Value>(&frame) {
            Ok(value) => value,
            Err(_) => {
                send_error(&mut stream, None, "PROTOCOL_MISMATCH");
                break;
            }
        };
        let keep_open = if let Some(database) = bound_database.as_ref() {
            handle_bound_request(&mut stream, &shared, database, &value)
        } else {
            handle_unbound_request(&mut stream, &shared, &value, &mut bound_database)
        };
        if !keep_open {
            break;
        }
    }
    if let Ok(mut state) = shared.lock() {
        let now_ms = state.now_ms;
        if let Some(database) = bound_database
            && let Some(context) = state.contexts.get_mut(&database)
        {
            context.clients = context.clients.saturating_sub(1);
            if context.clients == 0 {
                context.idle_since = Some(now_ms);
            }
        }
        state.active_connections = state.active_connections.saturating_sub(1);
    }
}

fn handle_unbound_request(
    stream: &mut UnixStream,
    shared: &Arc<Mutex<DaemonState>>,
    value: &Value,
    bound_database: &mut Option<String>,
) -> bool {
    match value.get("op").and_then(Value::as_str) {
        Some("ready") => {
            if value.get("version").and_then(Value::as_u64) != Some(PROTOCOL_VERSION) {
                send_error(stream, None, "PROTOCOL_MISMATCH");
            } else {
                let _ = send_frame(stream, &json!({"ready":true,"version":PROTOCOL_VERSION}));
            }
            true
        }
        Some("handshake") => {
            if value.get("version").and_then(Value::as_u64) != Some(PROTOCOL_VERSION) {
                send_error(stream, None, "PROTOCOL_MISMATCH");
                return false;
            }
            let selector = match value.get("selector") {
                Some(selector) => selector,
                None => {
                    send_error(stream, None, "PROJECT_NOT_INITIALIZED");
                    return false;
                }
            };
            let info = match selector_info(selector) {
                Ok(info) => info,
                Err(error) => {
                    send_error(stream, None, &error);
                    return false;
                }
            };
            match attach_context(shared, info) {
                Ok((database, response)) => {
                    *bound_database = Some(database);
                    let _ = send_frame(stream, &response);
                    true
                }
                Err(error) => {
                    send_error(stream, None, &error);
                    false
                }
            }
        }
        Some("init") => {
            let root = match value.get("root").and_then(Value::as_str) {
                Some(root) => PathBuf::from(root),
                None => {
                    send_error(stream, None, "PROJECT_NOT_INITIALIZED");
                    return false;
                }
            };
            match initialize_project(&root) {
                Ok(response) => {
                    let _ = send_frame(stream, &response);
                    true
                }
                Err(error) => {
                    send_error(stream, None, &error);
                    false
                }
            }
        }
        Some("admin") => handle_admin(stream, shared, value),
        _ => {
            send_error(stream, None, "PROTOCOL_MISMATCH");
            false
        }
    }
}

fn initialize_project(root: &Path) -> Result<Value, String> {
    let root = fs::canonicalize(root).map_err(|_| "PROJECT_NOT_INITIALIZED".to_string())?;
    let state_dir = root.join(".repin");
    fs::create_dir_all(&state_dir).map_err(|_| "PROJECT_STATE_INVALID".to_string())?;
    set_private(&state_dir, 0o700).map_err(|_| "PROJECT_STATE_INVALID".to_string())?;
    let lock_path = state_dir.join("writer.lock");
    let lease = Lease::acquire(&lock_path)
        .map_err(|_| "PROJECT_STATE_INVALID".to_string())?
        .ok_or_else(|| "PROJECT_LEASE_UNAVAILABLE".to_string())?;
    let graph = state_dir.join("graph.redb");
    if graph.exists() {
        drop(lease);
        return Err("PROJECT_ALREADY_INITIALIZED".into());
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&graph)
        .map_err(|_| "PROJECT_STATE_INVALID".to_string())?;
    file.write_all(b"schema=1\nrevision=0\n")
        .map_err(|_| "PROJECT_STATE_INVALID".to_string())?;
    file.flush()
        .map_err(|_| "PROJECT_STATE_INVALID".to_string())?;
    set_private(&graph, 0o600).map_err(|_| "PROJECT_STATE_INVALID".to_string())?;
    drop(lease);
    Ok(json!({"ok":true,"root":root,"database":graph}))
}

fn attach_context(
    shared: &Arc<Mutex<DaemonState>>,
    info: ProjectInfo,
) -> Result<(String, Value), String> {
    let key = info.database.to_string_lossy().to_string();
    let mut state = shared
        .lock()
        .map_err(|_| "PROJECT_STATE_INVALID".to_string())?;
    if info.spelled_graph != info.database && state.contexts.contains_key(&key) {
        return Err("PROJECT_STATE_ALIAS".into());
    }
    if let Some(existing) = state.contexts.get(&key) {
        if existing.identity != info.identity || existing.closed {
            return Err("PROJECT_STATE_ALIAS".into());
        }
        if info.spelled_graph != info.database {
            return Err("PROJECT_STATE_ALIAS".into());
        }
    }
    if state
        .contexts
        .values()
        .any(|context| context.identity == info.identity && context.database != info.database)
    {
        return Err("PROJECT_STATE_ALIAS".into());
    }
    if let Some(context) = state.contexts.get_mut(&key) {
        context.clients += 1;
        context.idle_since = None;
        let response = json!({
            "ok":true,
            "context_id":context.id,
            "root":context.root,
            "database":context.database,
            "graph_status":context.graph_status.as_str(),
            "revision":context.revision,
            "authoritative":context.authoritative,
            "shared":true,
        });
        return Ok((key, response));
    }
    let writer_path = info.state_dir.join("writer.lock");
    let writer_lease =
        Lease::acquire(&writer_path).map_err(|_| "PROJECT_STATE_INVALID".to_string())?;
    let authoritative = writer_lease.is_some();
    let id = format!("context-{}", state.next_context_id);
    state.next_context_id += 1;
    state.had_context = true;
    let revision = read_revision(&info.database);
    let context = Context {
        id: id.clone(),
        root: info.root,
        state_dir: info.state_dir,
        database: info.database,
        identity: info.identity,
        graph_status: info.graph_status,
        revision,
        clients: 1,
        authoritative,
        watcher_registered: true,
        idle_since: None,
        closed: false,
        writer_lease,
    };
    let response = json!({
        "ok":true,
        "context_id":context.id,
        "root":context.root,
        "database":context.database,
        "graph_status":context.graph_status.as_str(),
        "revision":context.revision,
        "authoritative":context.authoritative,
        "shared":false,
    });
    state.contexts.insert(key.clone(), context);
    Ok((key, response))
}

fn evict_idle(state: &mut DaemonState) -> Vec<String> {
    let now = state.now_ms;
    let candidates: Vec<String> = state
        .contexts
        .iter()
        .filter_map(|(key, context)| {
            (context.clients == 0
                && context
                    .idle_since
                    .is_some_and(|since| now.saturating_sub(since) >= IDLE_ADVANCE_MS))
            .then_some(key.clone())
        })
        .collect();
    let mut unloaded = Vec::new();
    for key in candidates {
        if let Some(mut context) = state.contexts.remove(&key) {
            context.watcher_registered = false;
            context.writer_lease.take();
            unloaded.push(context.id);
        }
    }
    unloaded
}

fn handle_admin(stream: &mut UnixStream, shared: &Arc<Mutex<DaemonState>>, value: &Value) -> bool {
    if value.get("token").and_then(Value::as_str) != Some("f8-experiment") {
        send_error(stream, None, "PROTOCOL_MISMATCH");
        return false;
    }
    match value.get("command").and_then(Value::as_str) {
        Some("snapshot") => {
            let response = match shared.lock() {
                Ok(state) => json!({"ok":true,"snapshot":snapshot(&state)}),
                Err(_) => json!({"ok":false,"error":"PROJECT_STATE_INVALID"}),
            };
            let _ = send_frame(stream, &response);
            true
        }
        Some("advance") => {
            let milliseconds = value
                .get("milliseconds")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            let response = match shared.lock() {
                Ok(mut state) => {
                    state.now_ms = state.now_ms.saturating_add(milliseconds);
                    let unloaded = evict_idle(&mut state);
                    json!({"ok":true,"now_ms":state.now_ms,"unloaded":unloaded,"snapshot":snapshot(&state)})
                }
                Err(_) => json!({"ok":false,"error":"PROJECT_STATE_INVALID"}),
            };
            let _ = send_frame(stream, &response);
            true
        }
        Some("shutdown") => {
            if let Ok(mut state) = shared.lock() {
                state.shutdown_requested = true;
            }
            let _ = send_frame(stream, &json!({"ok":true}));
            true
        }
        _ => {
            send_error(stream, None, "PROTOCOL_MISMATCH");
            false
        }
    }
}

fn validate_context<'a>(
    state: &'a mut DaemonState,
    database: &str,
) -> Result<&'a mut Context, String> {
    let context = state
        .contexts
        .get_mut(database)
        .ok_or_else(|| "PROJECT_STATE_INVALID".to_string())?;
    if context.closed {
        return Err("PROJECT_STATE_ALIAS".into());
    }
    match file_identity(&context.database) {
        Ok(identity) if identity == context.identity => Ok(context),
        Ok(_) | Err(_) => {
            context.closed = true;
            Err("PROJECT_STATE_ALIAS".into())
        }
    }
}

fn response_with_id(value: &Value, mut body: Value) -> Value {
    if let Some(request_id) = request_id(value) {
        body["request_id"] = request_id.clone();
    }
    body
}

fn handle_bound_request(
    stream: &mut UnixStream,
    shared: &Arc<Mutex<DaemonState>>,
    database: &str,
    value: &Value,
) -> bool {
    if value.get("path").is_some() || value.get("root").is_some() {
        let response = response_with_id(value, json!({"ok":false,"error":"PROJECT_BOUND"}));
        let _ = send_frame(stream, &response);
        return true;
    }
    let operation = value.get("op").and_then(Value::as_str).unwrap_or("");
    match operation {
        "close" => {
            let response = response_with_id(value, json!({"ok":true,"closed":true}));
            let _ = send_frame(stream, &response);
            false
        }
        "revision" => {
            let response = match shared.lock() {
                Ok(mut state) => match validate_context(&mut state, database) {
                    Ok(context) => response_with_id(
                        value,
                        json!({"ok":true,"revision":context.revision,"context_id":context.id}),
                    ),
                    Err(error) => response_with_id(value, json!({"ok":false,"error":error})),
                },
                Err(_) => {
                    response_with_id(value, json!({"ok":false,"error":"PROJECT_STATE_INVALID"}))
                }
            };
            let _ = send_frame(stream, &response);
            true
        }
        "graph" => {
            let response = match shared.lock() {
                Ok(mut state) => match validate_context(&mut state, database) {
                    Ok(context) if context.graph_status == GraphStatus::Valid => {
                        response_with_id(value, json!({"ok":true,"revision":context.revision}))
                    }
                    Ok(context) => response_with_id(
                        value,
                        json!({"ok":false,"error":context.graph_status.as_str()}),
                    ),
                    Err(error) => response_with_id(value, json!({"ok":false,"error":error})),
                },
                Err(_) => {
                    response_with_id(value, json!({"ok":false,"error":"PROJECT_STATE_INVALID"}))
                }
            };
            let _ = send_frame(stream, &response);
            true
        }
        "commit" => {
            let response = match shared.lock() {
                Ok(mut state) => match validate_context(&mut state, database) {
                    Ok(context) if context.graph_status != GraphStatus::Valid => response_with_id(
                        value,
                        json!({"ok":false,"error":context.graph_status.as_str()}),
                    ),
                    Ok(context) if !context.authoritative => response_with_id(
                        value,
                        json!({"ok":false,"error":"PROJECT_LEASE_UNAVAILABLE"}),
                    ),
                    Ok(context) => {
                        context.revision += 1;
                        let body = format!("schema=1\nrevision={}\n", context.revision);
                        let result = fs::write(&context.database, body);
                        if result.is_err() {
                            response_with_id(
                                value,
                                json!({"ok":false,"error":"PROJECT_STATE_INVALID"}),
                            )
                        } else {
                            response_with_id(value, json!({"ok":true,"revision":context.revision}))
                        }
                    }
                    Err(error) => response_with_id(value, json!({"ok":false,"error":error})),
                },
                Err(_) => {
                    response_with_id(value, json!({"ok":false,"error":"PROJECT_STATE_INVALID"}))
                }
            };
            let _ = send_frame(stream, &response);
            true
        }
        "retrieve" => {
            let relative = value.get("relative").and_then(Value::as_str).unwrap_or("");
            let (root, graph_status) = match shared.lock() {
                Ok(mut state) => match validate_context(&mut state, database) {
                    Ok(context) => (context.root.clone(), context.graph_status.clone()),
                    Err(error) => {
                        let response = response_with_id(value, json!({"ok":false,"error":error}));
                        let _ = send_frame(stream, &response);
                        return true;
                    }
                },
                Err(_) => {
                    send_error(stream, request_id(value), "PROJECT_STATE_INVALID");
                    return true;
                }
            };
            let path = match safe_relative(&root, relative) {
                Ok(path) => path,
                Err(error) => {
                    let response = response_with_id(value, json!({"ok":false,"error":error}));
                    let _ = send_frame(stream, &response);
                    return true;
                }
            };
            let bytes = match fs::read(&path) {
                Ok(bytes) if bytes.len() <= 64 * 1024 => bytes,
                Ok(_) => {
                    let response = response_with_id(
                        value,
                        json!({"ok":false,"error":"DIRECT_RETRIEVAL_BOUNDED"}),
                    );
                    let _ = send_frame(stream, &response);
                    return true;
                }
                Err(_) => {
                    let response = response_with_id(
                        value,
                        json!({"ok":false,"error":"DIRECT_RETRIEVAL_REJECTED"}),
                    );
                    let _ = send_frame(stream, &response);
                    return true;
                }
            };
            let _ = send_frame(
                stream,
                &json!({"event":"progress","request_id":request_id(value),"completed":1,"total":1}),
            );
            let response = response_with_id(
                value,
                json!({
                    "ok":true,
                    "bytes":String::from_utf8_lossy(&bytes),
                    "graph_status":graph_status.as_str(),
                }),
            );
            let _ = send_frame(stream, &response);
            true
        }
        "slow" => {
            let response = if value.get("cancelled").and_then(Value::as_bool) == Some(true) {
                response_with_id(value, json!({"ok":false,"error":"CANCELLED"}))
            } else if value
                .get("deadline_ms")
                .and_then(Value::as_u64)
                .is_some_and(|deadline| deadline < 10)
            {
                response_with_id(value, json!({"ok":false,"error":"DEADLINE_EXCEEDED"}))
            } else {
                thread::sleep(Duration::from_millis(10));
                response_with_id(value, json!({"ok":true,"completed":true}))
            };
            let _ = send_frame(stream, &response);
            true
        }
        _ => {
            let response = response_with_id(value, json!({"ok":false,"error":"PROTOCOL_MISMATCH"}));
            let _ = send_frame(stream, &response);
            true
        }
    }
}

struct Client {
    stream: UnixStream,
    context_id: String,
    database: String,
    root: String,
}

impl Client {
    fn request(&mut self, value: Value) -> AppResult<Vec<Value>> {
        send_frame(&mut self.stream, &value)?;
        let mut frames = Vec::new();
        loop {
            let frame = read_frame(&mut self.stream)?.ok_or("connection closed")?;
            let response: Value = serde_json::from_str(&frame)?;
            let terminal = response.get("ok").is_some() || response.get("error").is_some();
            frames.push(response);
            if terminal {
                return Ok(frames);
            }
        }
    }

    fn request_one(&mut self, value: Value) -> AppResult<Value> {
        self.request(value)?
            .pop()
            .ok_or_else(|| "empty response".into())
    }

    fn close_gracefully(mut self) -> AppResult<()> {
        let response = self.request_one(json!({"op":"close","request_id":"close"}))?;
        if response.get("ok") != Some(&Value::Bool(true)) {
            return Err(format!("close failed: {response}").into());
        }
        Ok(())
    }
}

fn connect_ready(socket: &Path) -> AppResult<bool> {
    let mut stream = UnixStream::connect(socket)?;
    stream.set_read_timeout(Some(Duration::from_millis(500)))?;
    send_frame(
        &mut stream,
        &json!({"op":"ready","version":PROTOCOL_VERSION}),
    )?;
    let response = read_frame(&mut stream)?.ok_or("ready connection closed")?;
    let value: Value = serde_json::from_str(&response)?;
    Ok(value.get("ready") == Some(&Value::Bool(true)))
}

fn wait_ready(socket: &Path, timeout: Duration) -> AppResult<()> {
    let started = Instant::now();
    loop {
        if socket.exists() && connect_ready(socket).unwrap_or(false) {
            return Ok(());
        }
        if started.elapsed() > timeout {
            return Err("DAEMON_START_FAILED".into());
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn raw_request(socket: &Path, value: Value) -> AppResult<Vec<Value>> {
    let mut stream = UnixStream::connect(socket)?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    send_frame(&mut stream, &value)?;
    let mut frames = Vec::new();
    loop {
        let frame = read_frame(&mut stream)?.ok_or("connection closed")?;
        let response: Value = serde_json::from_str(&frame)?;
        let terminal = response.get("ok").is_some() || response.get("error").is_some();
        frames.push(response);
        if terminal {
            return Ok(frames);
        }
    }
}

fn admin_request(socket: &Path, command: &str, fields: Value) -> AppResult<Value> {
    let mut request = json!({"op":"admin","token":"f8-experiment","command":command});
    if let Value::Object(extra) = fields
        && let Value::Object(body) = &mut request
    {
        for (key, value) in extra {
            body.insert(key, value);
        }
    }
    raw_request(socket, request)?
        .pop()
        .ok_or_else(|| "empty admin response".into())
}

fn handshake_response(socket: &Path, selector: Value, version: u64) -> AppResult<Value> {
    raw_request(
        socket,
        json!({"op":"handshake","version":version,"selector":selector}),
    )?
    .pop()
    .ok_or_else(|| "empty handshake response".into())
}

fn connect_project(socket: &Path, selector: Value) -> AppResult<Client> {
    let mut stream = UnixStream::connect(socket)?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    send_frame(
        &mut stream,
        &json!({"op":"handshake","version":PROTOCOL_VERSION,"selector":selector}),
    )?;
    let frame = read_frame(&mut stream)?.ok_or("handshake connection closed")?;
    let response: Value = serde_json::from_str(&frame)?;
    if response.get("ok") != Some(&Value::Bool(true)) {
        return Err(response
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("handshake failed")
            .to_string()
            .into());
    }
    Ok(Client {
        stream,
        context_id: response["context_id"].as_str().unwrap_or_default().into(),
        database: response["database"].as_str().unwrap_or_default().into(),
        root: response["root"].as_str().unwrap_or_default().into(),
    })
}

fn spawn_candidate(runtime: &Path, die_before_ready: bool) -> AppResult<Child> {
    let executable = env::current_exe()?;
    let mut command = Command::new(&executable);
    command
        .args(["daemon", "--runtime"])
        .arg(runtime)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if die_before_ready {
        command.arg("--die-before-ready");
    }
    command
        .spawn()
        .map_err(|error| format!("spawn {}: {error}", executable.display()).into())
}

fn wait_child(child: &mut Child, timeout: Duration) -> AppResult<()> {
    let started = Instant::now();
    loop {
        if child.try_wait()?.is_some() {
            return Ok(());
        }
        if started.elapsed() > timeout {
            child.kill()?;
            let _ = child.wait();
            return Err("daemon child did not exit".into());
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn stop_daemon(socket: &Path, child: &mut Child) -> AppResult<()> {
    if socket.exists() {
        let _ = admin_request(socket, "shutdown", json!({}));
    }
    wait_child(child, Duration::from_secs(3))
}

fn candidate_events(runtime: &Path) -> Vec<Value> {
    let mut events: Vec<Value> = fs::read_dir(runtime.join("candidates"))
        .ok()
        .into_iter()
        .flat_map(|entries| entries.flatten())
        .filter_map(|entry| fs::read(entry.path()).ok())
        .filter_map(|bytes| serde_json::from_slice(&bytes).ok())
        .collect();
    events.sort_by_key(|event: &Value| event["outcome"].as_str().unwrap_or_default().to_string());
    events
}

fn selector_discover(path: &Path) -> Value {
    json!({"kind":"discover","path":path})
}

fn selector_root(root: &Path) -> Value {
    json!({"kind":"root","root":root})
}

fn init_fixture(root: &Path, graph: &str, working: &str) -> AppResult<()> {
    fs::create_dir_all(root.join(".repin"))?;
    fs::write(root.join(".repin/graph.redb"), graph)?;
    fs::write(root.join("working.txt"), working)?;
    set_private(&root.join(".repin"), 0o700)?;
    set_private(&root.join(".repin/graph.redb"), 0o600)?;
    Ok(())
}

fn make_project(parent: &Path, name: &str, revision: u64) -> AppResult<PathBuf> {
    let root = parent.join(name);
    fs::create_dir_all(&root)?;
    init_fixture(
        &root,
        &format!("schema=1\nrevision={revision}\n"),
        &format!("working-{name}\n"),
    )?;
    Ok(root)
}

fn case(
    cases: &mut Vec<CaseObservation>,
    id: &str,
    expected: &str,
    observed: impl Into<String>,
    passed: bool,
    details: Value,
) {
    cases.push(CaseObservation {
        id: id.into(),
        expected: expected.into(),
        observed: observed.into(),
        outcome: if passed { "pass" } else { "fail" }.into(),
        details,
    });
}

fn error_is(value: &Value, error: &str) -> bool {
    value.get("error").and_then(Value::as_str) == Some(error)
}

fn response_ok(value: &Value) -> bool {
    value.get("ok") == Some(&Value::Bool(true))
}

fn response_last(frames: &[Value]) -> Option<&Value> {
    frames.last()
}

fn snapshot_contexts(value: &Value) -> Vec<Value> {
    value
        .get("snapshot")
        .and_then(|snapshot| snapshot.get("contexts"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn has_context_root(value: &Value, root: &Path) -> bool {
    let root = root.to_string_lossy();
    snapshot_contexts(value)
        .iter()
        .any(|context| context.get("root").and_then(Value::as_str) == Some(root.as_ref()))
}

fn event_exists(events: &[Value], outcome: &str) -> bool {
    events
        .iter()
        .any(|event| event.get("outcome").and_then(Value::as_str) == Some(outcome))
}

fn malformed_listener(socket: &Path) -> AppResult<thread::JoinHandle<()>> {
    let listener = UnixListener::bind(socket)?;
    Ok(thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let _ = read_frame(&mut stream);
            let _ = stream.write_all(b"not-a-readiness-frame\n");
            let _ = stream.flush();
            thread::sleep(Duration::from_millis(50));
        }
    }))
}

fn run_experiment(output_arg: &str) -> AppResult<()> {
    let output = PathBuf::from(output_arg);
    fs::create_dir_all(&output)?;
    let temp = tempdir()?;
    let projects = temp.path().join("projects");
    fs::create_dir_all(&projects)?;
    let runtime = temp.path().join("runtime");
    fs::create_dir_all(&runtime)?;
    let run_manifest = manifest(&runtime);
    write_json(&output.join("manifest.json"), &run_manifest)?;
    let mut cases = Vec::new();

    let discovery_outer = make_project(&projects, "discovery-outer", 3)?;
    let discovery_incomplete = discovery_outer.join("incomplete/child");
    fs::create_dir_all(discovery_incomplete.join(".repin"))?;
    fs::write(discovery_incomplete.join("working.txt"), "incomplete\n")?;
    let discovery_nested = make_project(&discovery_outer, "nested", 7)?;
    fs::create_dir_all(discovery_nested.join("deep/leaf"))?;
    let graph_without_dir = discovery_outer.join("graph-without-repin/graph.redb");
    fs::create_dir_all(graph_without_dir.parent().ok_or("missing fixture parent")?)?;
    fs::write(&graph_without_dir, b"schema=1\nrevision=99\n")?;
    let physical_parent = projects.join("physical-parent");
    let physical_project = make_project(&physical_parent, "physical-project", 11)?;
    fs::create_dir_all(physical_project.join("inside"))?;
    let linked_parent = projects.join("linked-parent");
    symlink(&physical_parent, &linked_parent)?;
    let empty_project = projects.join("empty");
    fs::create_dir_all(&empty_project)?;

    let init_empty = projects.join("init-empty");
    fs::create_dir_all(&init_empty)?;
    let init_partial = projects.join("init-partial");
    fs::create_dir_all(init_partial.join(".repin"))?;
    let init_existing = make_project(&projects, "init-existing", 4)?;
    let shared = make_project(&projects, "shared", 0)?;
    let invalid = projects.join("invalid");
    init_fixture(&invalid, "INVALID\noriginal-bytes\n", "invalid-working\n")?;
    let newer = projects.join("newer");
    init_fixture(&newer, "schema=999\nrevision=12\n", "newer-working\n")?;
    let observer = make_project(&projects, "observer", 0)?;
    let crash_project = make_project(&projects, "crash", 0)?;
    let idle_a = make_project(&projects, "idle-a", 0)?;
    let idle_b = make_project(&projects, "idle-b", 0)?;
    let protocol_project = make_project(&projects, "protocol", 0)?;

    // Cold-start election: all candidates use this same executable and race
    // for the same kernel-backed lease before any client connects.
    let mut candidates = Vec::new();
    for _ in 0..8 {
        candidates.push(spawn_candidate(&runtime, false)?);
    }
    wait_ready(&runtime.join("daemon.sock"), Duration::from_secs(3))?;
    thread::sleep(Duration::from_millis(100));
    let events = candidate_events(&runtime);
    let winners: Vec<&Value> = events
        .iter()
        .filter(|event| event.get("outcome").and_then(Value::as_str) == Some("winner"))
        .collect();
    let winner_pid = winners
        .first()
        .and_then(|event| event.get("pid"))
        .and_then(Value::as_u64)
        .ok_or("cold-start winner was not recorded")? as u32;
    let mut daemon = None;
    for mut candidate in candidates {
        if candidate.id() == winner_pid {
            daemon = Some(candidate);
        } else {
            wait_child(&mut candidate, Duration::from_secs(2))?;
        }
    }
    let mut daemon = daemon.ok_or("cold-start daemon child was not retained")?;
    case(
        &mut cases,
        "F8-COLD-START-RACE",
        "exactly one winner and all candidates use one socket",
        format!(
            "{} winner(s), {} candidate events",
            winners.len(),
            events.len()
        ),
        winners.len() == 1 && event_exists(&events, "lease_unavailable"),
        json!({"winner_count":winners.len(),"events":events}),
    );

    // A candidate facing a live socket cannot steal the live daemon lease.
    let mut live_candidate = spawn_candidate(&runtime, false)?;
    wait_child(&mut live_candidate, Duration::from_secs(2))?;
    let live_events = candidate_events(&runtime);
    case(
        &mut cases,
        "F8-LIVE-SOCKET",
        "candidate exits without disturbing a live daemon",
        if event_exists(&live_events, "lease_unavailable") {
            "lease_unavailable"
        } else {
            "missing lease_unavailable event"
        },
        event_exists(&live_events, "lease_unavailable"),
        json!({"events":live_events}),
    );

    // Initialization is daemon-mediated and never overwrites a graph.
    let init_response = raw_request(
        &runtime.join("daemon.sock"),
        json!({"op":"init","root":init_empty}),
    )?
    .pop()
    .ok_or("initialization response missing")?;
    let initialized_mode = fs::metadata(init_empty.join(".repin"))?
        .permissions()
        .mode()
        & 0o777;
    let initialized_graph = fs::read(init_empty.join(".repin/graph.redb"))?;
    let repeat_init = raw_request(
        &runtime.join("daemon.sock"),
        json!({"op":"init","root":init_empty}),
    )?
    .pop()
    .ok_or("repeat initialization response missing")?;
    let existing_init = raw_request(
        &runtime.join("daemon.sock"),
        json!({"op":"init","root":init_existing}),
    )?
    .pop()
    .ok_or("existing initialization response missing")?;
    let partial_init = raw_request(
        &runtime.join("daemon.sock"),
        json!({"op":"init","root":init_partial}),
    )?
    .pop()
    .ok_or("partial initialization response missing")?;
    case(
        &mut cases,
        "F8-INITIALIZATION",
        "private creation, incomplete marker repair, and no overwrite",
        format!("mode={initialized_mode:o}"),
        response_ok(&init_response)
            && initialized_mode & 0o077 == 0
            && error_is(&repeat_init, "PROJECT_ALREADY_INITIALIZED")
            && error_is(&existing_init, "PROJECT_ALREADY_INITIALIZED")
            && response_ok(&partial_init)
            && initialized_graph == b"schema=1\nrevision=0\n",
        json!({
            "first":init_response,
            "repeat":repeat_init,
            "existing":existing_init,
            "partial":partial_init,
            "mode":initialized_mode,
        }),
    );

    // Discovery canonicalizes the supplied parent spelling and walks only
    // initialized marker pairs; explicit root selection bypasses the walk.
    let incomplete_client = connect_project(
        &runtime.join("daemon.sock"),
        selector_discover(&discovery_incomplete),
    )?;
    let incomplete_root = incomplete_client.root.clone();
    incomplete_client.close_gracefully()?;
    let nested_client = connect_project(
        &runtime.join("daemon.sock"),
        selector_discover(&discovery_nested.join("deep/leaf")),
    )?;
    let nested_root = nested_client.root.clone();
    nested_client.close_gracefully()?;
    let explicit_client = connect_project(
        &runtime.join("daemon.sock"),
        selector_root(&discovery_outer),
    )?;
    let explicit_root = explicit_client.root.clone();
    explicit_client.close_gracefully()?;
    let symlink_parent_client = connect_project(
        &runtime.join("daemon.sock"),
        selector_discover(&linked_parent.join("physical-project/inside")),
    )?;
    let symlink_parent_root = symlink_parent_client.root.clone();
    symlink_parent_client.close_gracefully()?;
    let graphless_child = graph_without_dir
        .parent()
        .ok_or("missing graphless fixture parent")?
        .join("child");
    fs::create_dir_all(&graphless_child)?;
    let graphless_client = connect_project(
        &runtime.join("daemon.sock"),
        selector_discover(&graphless_child),
    )?;
    let graphless_root = graphless_client.root.clone();
    graphless_client.close_gracefully()?;
    let missing = handshake_response(
        &runtime.join("daemon.sock"),
        selector_discover(&empty_project),
        PROTOCOL_VERSION,
    )?;
    case(
        &mut cases,
        "F8-DISCOVERY",
        "nearest initialized ancestor, explicit override, and canonical parent",
        format!("incomplete={incomplete_root}, nested={nested_root}"),
        incomplete_root == fs::canonicalize(&discovery_outer)?.display().to_string()
            && nested_root == fs::canonicalize(&discovery_nested)?.display().to_string()
            && explicit_root == fs::canonicalize(&discovery_outer)?.display().to_string()
            && symlink_parent_root == fs::canonicalize(&physical_project)?.display().to_string()
            && error_is(&missing, "PROJECT_NOT_INITIALIZED")
            && graphless_root == fs::canonicalize(&discovery_outer)?.display().to_string(),
        json!({
            "incomplete_root":incomplete_root,
            "nested_root":nested_root,
            "explicit_root":explicit_root,
            "symlink_parent_root":symlink_parent_root,
            "graphless_root":graphless_root,
            "missing":missing,
            "graph_without_dir":graph_without_dir,
        }),
    );

    // Two clients share a context and revision stream; a copied database does
    // not, even though its graph bytes begin identical.
    let mut shared_a = connect_project(&runtime.join("daemon.sock"), selector_root(&shared))?;
    let mut shared_b = connect_project(&runtime.join("daemon.sock"), selector_root(&shared))?;
    let shared_context = shared_a.context_id == shared_b.context_id;
    let shared_commit = shared_a.request_one(json!({"op":"commit","request_id":1}))?;
    let shared_revision = shared_b.request_one(json!({"op":"revision","request_id":2}))?;
    let copied = projects.join("shared-copy");
    fs::create_dir_all(copied.join(".repin"))?;
    fs::copy(
        shared.join(".repin/graph.redb"),
        copied.join(".repin/graph.redb"),
    )?;
    fs::copy(shared.join("working.txt"), copied.join("working.txt"))?;
    let mut copied_client = connect_project(&runtime.join("daemon.sock"), selector_root(&copied))?;
    let copied_context = copied_client.context_id != shared_a.context_id;
    let copied_commit = copied_client.request_one(json!({"op":"commit","request_id":3}))?;
    let original_after_copy = shared_b.request_one(json!({"op":"revision","request_id":4}))?;
    case(
        &mut cases,
        "F8-CONTEXT-REGISTRY",
        "same canonical database shares one context; copied path is isolated",
        format!("shared_context={shared_context}, copied_context={copied_context}"),
        shared_context
            && copied_context
            && response_ok(&shared_commit)
            && shared_revision["revision"] == 1
            && response_ok(&copied_commit)
            && original_after_copy["revision"] == 1,
        json!({
            "shared_a":shared_a.context_id,
            "shared_b":shared_b.context_id,
            "copied":copied_client.context_id,
            "shared_database":shared_a.database,
            "shared_commit":shared_commit,
            "shared_revision":shared_revision,
            "copied_commit":copied_commit,
            "original_after_copy":original_after_copy,
        }),
    );
    shared_a.close_gracefully()?;
    copied_client.close_gracefully()?;

    // Active filesystem identity prevents hard-link and symlink aliases from
    // creating another context, and a replaced active path fails closed.
    let alias_hard = projects.join("alias-hard");
    fs::create_dir_all(alias_hard.join(".repin"))?;
    fs::hard_link(
        shared.join(".repin/graph.redb"),
        alias_hard.join(".repin/graph.redb"),
    )?;
    let alias_symlink = projects.join("alias-symlink");
    fs::create_dir_all(alias_symlink.join(".repin"))?;
    symlink(
        shared.join(".repin/graph.redb"),
        alias_symlink.join(".repin/graph.redb"),
    )?;
    let hard_alias = handshake_response(
        &runtime.join("daemon.sock"),
        selector_root(&alias_hard),
        PROTOCOL_VERSION,
    )?;
    let symlink_alias = handshake_response(
        &runtime.join("daemon.sock"),
        selector_root(&alias_symlink),
        PROTOCOL_VERSION,
    )?;
    fs::rename(
        shared.join(".repin/graph.redb"),
        shared.join(".repin/graph.moved"),
    )?;
    let replaced_context = shared_b.request_one(json!({"op":"revision","request_id":5}))?;
    case(
        &mut cases,
        "F8-ALIAS-GUARD",
        "active physical aliases reject and replacement fails closed",
        format!("hard={:?}, symlink={:?}", hard_alias, symlink_alias),
        error_is(&hard_alias, "PROJECT_STATE_ALIAS")
            && error_is(&symlink_alias, "PROJECT_STATE_ALIAS")
            && error_is(&replaced_context, "PROJECT_STATE_ALIAS"),
        json!({"hard":hard_alias,"symlink":symlink_alias,"replaced":replaced_context}),
    );
    shared_b.close_gracefully()?;

    // Invalid and newer graph state remains attachable for bounded direct
    // retrieval but cannot be used for graph operations or writes.
    let invalid_handshake = handshake_response(
        &runtime.join("daemon.sock"),
        selector_root(&invalid),
        PROTOCOL_VERSION,
    )?;
    let newer_handshake = handshake_response(
        &runtime.join("daemon.sock"),
        selector_root(&newer),
        PROTOCOL_VERSION,
    )?;
    let mut invalid_client =
        connect_project(&runtime.join("daemon.sock"), selector_root(&invalid))?;
    let invalid_frames = invalid_client.request(
        json!({"op":"retrieve","relative":"working.txt","request_id":"invalid-retrieve"}),
    )?;
    let invalid_graph =
        invalid_client.request_one(json!({"op":"graph","request_id":"invalid-graph"}))?;
    let invalid_commit =
        invalid_client.request_one(json!({"op":"commit","request_id":"invalid-commit"}))?;
    let mut newer_client = connect_project(&runtime.join("daemon.sock"), selector_root(&newer))?;
    let newer_frames = newer_client
        .request(json!({"op":"retrieve","relative":"working.txt","request_id":"newer-retrieve"}))?;
    let newer_graph = newer_client.request_one(json!({"op":"graph","request_id":"newer-graph"}))?;
    let newer_commit =
        newer_client.request_one(json!({"op":"commit","request_id":"newer-commit"}))?;
    let invalid_retrieve =
        response_last(&invalid_frames).ok_or("invalid retrieval response missing")?;
    let newer_retrieve = response_last(&newer_frames).ok_or("newer retrieval response missing")?;
    case(
        &mut cases,
        "F8-STATE-DEGRADATION",
        "invalid/newer graph attaches with direct retrieval and precise graph errors",
        format!(
            "invalid={}, newer={}",
            invalid_handshake["graph_status"], newer_handshake["graph_status"]
        ),
        invalid_handshake["graph_status"] == "PROJECT_STATE_INVALID"
            && newer_handshake["graph_status"] == "PROJECT_STATE_NEWER"
            && response_ok(invalid_retrieve)
            && invalid_retrieve["bytes"] == "invalid-working\n"
            && error_is(&invalid_graph, "PROJECT_STATE_INVALID")
            && error_is(&invalid_commit, "PROJECT_STATE_INVALID")
            && response_ok(newer_retrieve)
            && newer_retrieve["bytes"] == "newer-working\n"
            && error_is(&newer_graph, "PROJECT_STATE_NEWER")
            && error_is(&newer_commit, "PROJECT_STATE_NEWER"),
        json!({
            "invalid_handshake":invalid_handshake,
            "newer_handshake":newer_handshake,
            "invalid_frames":invalid_frames,
            "newer_frames":newer_frames,
            "invalid_graph":invalid_graph,
            "newer_graph":newer_graph,
        }),
    );
    invalid_client.close_gracefully()?;
    newer_client.close_gracefully()?;

    // An external writer owner yields observer mode, direct retrieval, and a
    // precise lease error for graph mutation.
    let held_marker = observer.join(".repin/writer.held");
    let mut holder = Command::new(env::current_exe()?)
        .args(["hold-lock", "--path"])
        .arg(observer.join(".repin/writer.lock"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let held_started = Instant::now();
    while !held_marker.exists() {
        if held_started.elapsed() > Duration::from_secs(2) {
            holder.kill()?;
            let _ = holder.wait();
            return Err("observer lock holder did not start".into());
        }
        thread::sleep(Duration::from_millis(10));
    }
    let observer_handshake = handshake_response(
        &runtime.join("daemon.sock"),
        selector_root(&observer),
        PROTOCOL_VERSION,
    )?;
    let mut observer_client =
        connect_project(&runtime.join("daemon.sock"), selector_root(&observer))?;
    let observer_retrieve = observer_client.request(
        json!({"op":"retrieve","relative":"working.txt","request_id":"observer-retrieve"}),
    )?;
    let observer_commit =
        observer_client.request_one(json!({"op":"commit","request_id":"observer-commit"}))?;
    let observer_retrieve_response =
        response_last(&observer_retrieve).ok_or("observer retrieval missing")?;
    case(
        &mut cases,
        "F8-OBSERVER-ATTACHMENT",
        "external writer lock produces observer attachment",
        format!("authoritative={}", observer_handshake["authoritative"]),
        observer_handshake["authoritative"] == false
            && response_ok(observer_retrieve_response)
            && observer_retrieve_response["bytes"] == "working-observer\n"
            && error_is(&observer_commit, "PROJECT_LEASE_UNAVAILABLE"),
        json!({"handshake":observer_handshake,"retrieve":observer_retrieve,"commit":observer_commit}),
    );
    observer_client.close_gracefully()?;
    holder.kill()?;
    let _ = holder.wait();
    let _ = fs::remove_file(held_marker);

    // Protocol negotiation, frame bounds, project binding, progress, request
    // IDs, deadlines, cancellation, and client detachment.
    let mismatch = handshake_response(
        &runtime.join("daemon.sock"),
        selector_root(&protocol_project),
        PROTOCOL_VERSION + 1,
    )?;
    let mut oversized = UnixStream::connect(runtime.join("daemon.sock"))?;
    oversized.set_read_timeout(Some(Duration::from_secs(2)))?;
    let mut oversized_frame = vec![b'x'; MAX_FRAME + 1];
    oversized_frame.push(b'\n');
    oversized.write_all(&oversized_frame)?;
    let oversized_response: Value =
        serde_json::from_str(&read_frame(&mut oversized)?.ok_or("oversized response missing")?)?;
    let mut protocol_client = connect_project(
        &runtime.join("daemon.sock"),
        selector_root(&protocol_project),
    )?;
    let bound_path = protocol_client.request_one(json!({
        "op":"revision",
        "path":projects.join("other-project"),
        "request_id":"bound",
    }))?;
    let progress_frames = protocol_client.request(json!({
        "op":"retrieve",
        "relative":"working.txt",
        "request_id":"progress",
    }))?;
    let deadline = protocol_client
        .request_one(json!({"op":"slow","deadline_ms":1,"request_id":"deadline"}))?;
    let cancelled =
        protocol_client.request_one(json!({"op":"slow","cancelled":true,"request_id":"cancel"}))?;
    let has_progress = progress_frames
        .first()
        .and_then(|frame| frame.get("event"))
        .and_then(Value::as_str)
        == Some("progress");
    case(
        &mut cases,
        "F8-PROTOCOL-BOUNDS",
        "protocol mismatch, bounded frames, binding, progress, deadlines, cancellation",
        format!("mismatch={mismatch:?}, oversized={oversized_response:?}"),
        error_is(&mismatch, "PROTOCOL_MISMATCH")
            && error_is(&oversized_response, "FRAME_TOO_LARGE")
            && error_is(&bound_path, "PROJECT_BOUND")
            && has_progress
            && response_last(&progress_frames).is_some_and(response_ok)
            && error_is(&deadline, "DEADLINE_EXCEEDED")
            && error_is(&cancelled, "CANCELLED"),
        json!({
            "mismatch":mismatch,
            "oversized":oversized_response,
            "bound":bound_path,
            "progress":progress_frames,
            "deadline":deadline,
            "cancelled":cancelled,
        }),
    );
    let detached_peer = connect_project(
        &runtime.join("daemon.sock"),
        selector_root(&protocol_project),
    )?;
    let detached_context = detached_peer.context_id.clone();
    drop(detached_peer);
    thread::sleep(Duration::from_millis(50));
    let remaining_revision =
        protocol_client.request_one(json!({"op":"revision","request_id":"remaining"}))?;
    case(
        &mut cases,
        "F8-CLIENT-DETACH",
        "one client termination does not cancel another bound client",
        detached_context,
        response_ok(&remaining_revision),
        json!({"remaining":remaining_revision}),
    );
    protocol_client.close_gracefully()?;

    // Kill the daemon while it owns a project writer handle. The kernel must
    // release the lock, and the next same-binary candidate repairs the stale
    // rendezvous socket before attaching to the preserved revision.
    let mut crash_client =
        connect_project(&runtime.join("daemon.sock"), selector_root(&crash_project))?;
    let crash_commit =
        crash_client.request_one(json!({"op":"commit","request_id":"crash-commit"}))?;
    drop(crash_client);
    daemon.kill()?;
    let _ = daemon.wait();
    let mut restarted = spawn_candidate(&runtime, false)?;
    wait_ready(&runtime.join("daemon.sock"), Duration::from_secs(3))?;
    thread::sleep(Duration::from_millis(50));
    let restart_events = candidate_events(&runtime);
    let mut restarted_client =
        connect_project(&runtime.join("daemon.sock"), selector_root(&crash_project))?;
    let recovered_revision =
        restarted_client.request_one(json!({"op":"revision","request_id":"recovered"}))?;
    case(
        &mut cases,
        "F8-CRASH-RESTART",
        "daemon death releases project lock and repairs stale socket",
        format!("recovered={recovered_revision:?}"),
        response_ok(&crash_commit)
            && event_exists(&restart_events, "stale_socket_repaired")
            && recovered_revision["revision"] == 1,
        json!({"commit":crash_commit,"events":restart_events,"recovered":recovered_revision}),
    );
    restarted_client.close_gracefully()?;

    // Malformed readiness and death before readiness are repaired only after
    // a candidate owns the singleton lease.
    let malformed_runtime = temp.path().join("malformed-runtime");
    fs::create_dir_all(&malformed_runtime)?;
    let malformed_handle = malformed_listener(&malformed_runtime.join("daemon.sock"))?;
    let mut malformed_candidate = spawn_candidate(&malformed_runtime, false)?;
    wait_ready(
        &malformed_runtime.join("daemon.sock"),
        Duration::from_secs(3),
    )?;
    malformed_handle
        .join()
        .map_err(|_| "malformed listener panicked")?;
    thread::sleep(Duration::from_millis(50));
    let malformed_events = candidate_events(&malformed_runtime);
    let malformed_case = event_exists(&malformed_events, "malformed_socket_repaired");
    stop_daemon(
        &malformed_runtime.join("daemon.sock"),
        &mut malformed_candidate,
    )?;
    let die_runtime = temp.path().join("die-runtime");
    fs::create_dir_all(&die_runtime)?;
    let mut dying_candidate = spawn_candidate(&die_runtime, true)?;
    wait_child(&mut dying_candidate, Duration::from_secs(2))?;
    let mut repair_candidate = spawn_candidate(&die_runtime, false)?;
    wait_ready(&die_runtime.join("daemon.sock"), Duration::from_secs(3))?;
    thread::sleep(Duration::from_millis(50));
    let die_events = candidate_events(&die_runtime);
    let die_case = event_exists(&die_events, "died_before_ready")
        && event_exists(&die_events, "stale_socket_repaired");
    stop_daemon(&die_runtime.join("daemon.sock"), &mut repair_candidate)?;
    case(
        &mut cases,
        "F8-STARTUP-REPAIR",
        "malformed readiness and pre-readiness death are recoverable",
        format!("malformed={malformed_case}, die={die_case}"),
        malformed_case && die_case,
        json!({"malformed":malformed_events,"died_before_ready":die_events}),
    );

    // Virtual time makes the ten-minute lifecycle deterministic while still
    // exercising the exact 600,000 ms threshold and sibling isolation.
    let idle_client_a = connect_project(&runtime.join("daemon.sock"), selector_root(&idle_a))?;
    let idle_client_b = connect_project(&runtime.join("daemon.sock"), selector_root(&idle_b))?;
    idle_client_a.close_gracefully()?;
    let first_advance = admin_request(
        &runtime.join("daemon.sock"),
        "advance",
        json!({"milliseconds":IDLE_ADVANCE_MS}),
    )?;
    let sibling_alive = has_context_root(&first_advance, &idle_b);
    let target_unloaded = !has_context_root(&first_advance, &idle_a);
    case(
        &mut cases,
        "F8-IDLE-EVICTION",
        "exact ten-minute idle eviction leaves active sibling untouched",
        format!("target_unloaded={target_unloaded}, sibling_alive={sibling_alive}"),
        target_unloaded && sibling_alive,
        json!({"first_advance":first_advance}),
    );
    idle_client_b.close_gracefully()?;
    let final_advance = admin_request(
        &runtime.join("daemon.sock"),
        "advance",
        json!({"milliseconds":IDLE_ADVANCE_MS}),
    )?;
    let final_empty = snapshot_contexts(&final_advance).is_empty();
    let daemon_exit = wait_child(&mut restarted, Duration::from_secs(3)).is_ok();
    case(
        &mut cases,
        "F8-DAEMON-LIFECYCLE",
        "final context unload closes socket and daemon exits",
        format!("final_empty={final_empty}, daemon_exit={daemon_exit}"),
        final_empty && daemon_exit,
        json!({"final_advance":final_advance,"socket_exists":runtime.join("daemon.sock").exists()}),
    );

    let all_pass = cases.iter().all(|item| item.outcome == "pass");
    let report = Report {
        experiment: "F8",
        run_id: RUN_ID,
        status: if all_pass { "complete" } else { "complete_with_gaps" }.into(),
        overall_outcome: if all_pass { "pass" } else { "inconclusive" }.into(),
        decision_status: "deferred",
        hard_blocker: false,
        case_ids: cases.iter().map(|item| item.id.clone()).collect(),
        cases,
        measurements: Vec::new(),
        notes: vec![
            "Linux x86_64/glibc runtime experiment; project and daemon state are disposable fixtures.".into(),
            "The idle threshold uses a virtual clock but retains the normative 600,000 ms duration.".into(),
            "The protocol includes an experiment-only admin channel for clock advancement and snapshots; it is not a public cross-project API.".into(),
            "This harness is evidence code and does not implement the production Repin daemon.".into(),
        ],
        artifacts: vec!["manifest.json".into(), "F8.json".into(), "F8-report.json".into()],
    };
    write_json(&output.join("F8.json"), &report)?;
    write_json(&output.join("F8-report.json"), &report)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
