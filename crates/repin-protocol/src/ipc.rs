use crate::envelope::ResultEnvelope;
use crate::errors::ErrorCode;
use repin_core::config::RepinConfig;
use repin_core::model::provenance::Revision;
use repin_core::ports::fs::FileChange;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BootstrapHandshake {
    pub bootstrap_version: u32,
    pub protocol_min: u32,
    pub protocol_max: u32,
    pub client_package_version: String,
    pub client_build_id: Option<String>,
    #[serde(default)]
    pub replacement_request: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BootstrapHandshakeOk {
    pub bootstrap_version: u32,
    pub selected_protocol: u32,
    pub daemon_protocol_min: u32,
    pub daemon_protocol_max: u32,
    pub daemon_package_version: String,
    pub daemon_build_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BootstrapRejected {
    pub code: ErrorCode,
    pub bootstrap_version: u32,
    pub daemon_protocol_min: u32,
    pub daemon_protocol_max: u32,
    pub daemon_package_version: String,
    pub daemon_build_id: Option<String>,
    pub replacement_allowed: bool,
    pub message: String,
}

pub fn select_protocol(
    client_min: u32,
    client_max: u32,
    daemon_min: u32,
    daemon_max: u32,
) -> Option<u32> {
    let min = client_min.max(daemon_min);
    let max = client_max.min(daemon_max);
    (min <= max).then_some(max)
}

pub fn replacement_allowed(
    client_protocol_min: u32,
    daemon_protocol_max: u32,
    full_idle: bool,
) -> bool {
    client_protocol_min > daemon_protocol_max && full_idle
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RebuildTarget {
    Graph,
    Lexical,
    Vector,
    All,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum IpcRequest {
    Bootstrap(BootstrapHandshake),
    RequestReplacement,
    Handshake {
        client_version: String,
        project_db_path: String,
        #[serde(default)]
        resolved_config: Option<RepinConfig>,
    },
    /// Daemon-mediated creation of durable project state (ADR-026). Issued
    /// before project binding; a successful response binds the connection.
    InitializeProject {
        project_root: String,
        #[serde(default)]
        resolved_config: Option<RepinConfig>,
    },
    /// Daemon-mediated removal of durable project state (ADR-026). Issued
    /// before project binding and refused while another connection is
    /// attached to that project's context.
    UninitializeProject {
        project_root: String,
    },
    Status,
    IndexAll,
    Rebuild {
        target: RebuildTarget,
    },
    SearchDirect {
        pattern: String,
        is_regex: bool,
        paths: Option<Vec<String>>,
        max_results: Option<usize>,
    },
    SearchGraph {
        query: String,
        max_results: Option<usize>,
    },
    SearchHybrid {
        query: String,
        max_results: Option<usize>,
        #[serde(default)]
        centrality_boost: Option<f64>,
    },
    InspectFile {
        path: String,
    },
    AtPosition {
        path: String,
        line: u32,
        column: u32,
    },
    Entity {
        name_or_id: String,
    },
    Neighbors {
        name_or_id: String,
        max_depth: Option<usize>,
    },
    Impact {
        name_or_id: String,
        max_depth: Option<usize>,
    },
    Path {
        from: String,
        to: String,
        max_depth: Option<usize>,
    },
    Context {
        query: String,
        budget_bytes: Option<usize>,
        #[serde(default)]
        padding_lines: Option<usize>,
        #[serde(default)]
        include_blast_radius: Option<bool>,
        #[serde(default)]
        include_verbatim_source: Option<bool>,
    },
    ReviewContext {
        changed_since: Option<Revision>,
        budget_bytes: Option<usize>,
    },
    UpdateFiles {
        changes: Vec<FileChange>,
    },
    SyncVcs,
    Rerank {
        query: String,
        candidates: Vec<String>,
        agent_cmd: String,
        #[serde(default)]
        top_n: Option<usize>,
        #[serde(default)]
        deadline_ms: Option<u64>,
    },
    Eval,
    Shutdown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum IpcResponse {
    BootstrapOk(BootstrapHandshakeOk),
    BootstrapRejected(BootstrapRejected),
    ReplacementAccepted,
    HandshakeOk {
        protocol_version: u32,
        daemon_version: String,
        is_writer: bool,
    },
    InitializeProjectOk {
        project_root: String,
        db_path: String,
        created: bool,
        is_writer: bool,
    },
    UninitializeProjectOk {
        project_root: String,
        removed: bool,
    },
    StatusOk {
        graph_revision: Revision,
        node_count: usize,
        edge_count: usize,
    },
    IndexAllOk {
        files_indexed: usize,
        revision: Revision,
    },
    RebuildOk {
        target: RebuildTarget,
        files_indexed: usize,
        revision: Revision,
    },
    SearchResult(ResultEnvelope<serde_json::Value>),
    InspectResult(ResultEnvelope<serde_json::Value>),
    PositionResult(ResultEnvelope<serde_json::Value>),
    EntityResult(ResultEnvelope<serde_json::Value>),
    NeighborsResult(ResultEnvelope<serde_json::Value>),
    ImpactResult(ResultEnvelope<serde_json::Value>),
    PathResult(ResultEnvelope<serde_json::Value>),
    ContextResult(ResultEnvelope<serde_json::Value>),
    ReviewResult(ResultEnvelope<serde_json::Value>),
    RerankResult(ResultEnvelope<serde_json::Value>),
    EvalResult(ResultEnvelope<serde_json::Value>),
    UpdateOk {
        revision: Revision,
    },
    Error {
        code: ErrorCode,
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IpcMessage {
    pub request_id: u64,
    pub body: IpcRequest,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IpcResponseEnvelope {
    pub request_id: u64,
    pub body: IpcResponse,
}
