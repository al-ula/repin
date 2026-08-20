use crate::envelope::ResultEnvelope;
use crate::errors::ErrorCode;
use repin_core::model::provenance::Revision;
use repin_core::ports::fs::FileChange;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum IpcRequest {
    Handshake {
        client_version: String,
        project_db_path: String,
    },
    Status,
    IndexAll,
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
    Context {
        query: String,
        budget_bytes: Option<usize>,
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
    },
    Eval,
    Shutdown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum IpcResponse {
    HandshakeOk {
        protocol_version: u32,
        daemon_version: String,
        is_writer: bool,
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
    SearchResult(ResultEnvelope<serde_json::Value>),
    InspectResult(ResultEnvelope<serde_json::Value>),
    PositionResult(ResultEnvelope<serde_json::Value>),
    EntityResult(ResultEnvelope<serde_json::Value>),
    NeighborsResult(ResultEnvelope<serde_json::Value>),
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
