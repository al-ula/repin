use repin_core::ports::model::{
    ModelError, ModelIdentity, ModelLocation, RerankCandidate, RerankHit, Reranker,
};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct AgentRunnerReranker {
    pub cmd: String,
    pub deadline_ms: u64,
}

impl AgentRunnerReranker {
    pub fn new(cmd: impl Into<String>, deadline_ms: u64) -> Self {
        Self {
            cmd: cmd.into(),
            deadline_ms: if deadline_ms == 0 { 5000 } else { deadline_ms },
        }
    }
}

#[derive(Serialize)]
struct JsonRpcRequest<'a> {
    jsonrpc: &'static str,
    method: &'static str,
    params: RerankParams<'a>,
}

#[derive(Serialize)]
struct RerankParams<'a> {
    query: &'a str,
    candidates: &'a [RerankCandidate],
}

#[derive(Deserialize)]
struct JsonRpcResponse {
    result: Option<JsonRpcResult>,
    error: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct JsonRpcResult {
    ranked: Vec<JsonRpcRankedHit>,
}

#[derive(Deserialize)]
struct JsonRpcRankedHit {
    id: String,
    score: Option<f32>,
}

impl Reranker for AgentRunnerReranker {
    fn identity(&self) -> ModelIdentity {
        ModelIdentity {
            provider: "agent".to_string(),
            model: self.cmd.clone(),
            version: None,
            location: ModelLocation::HostSupplied,
        }
    }

    fn rerank(&self, query: &str, candidates: &[RerankCandidate]) -> Result<Vec<RerankHit>, ModelError> {
        if candidates.is_empty() {
            return Ok(Vec::new());
        }

        let rpc_req = JsonRpcRequest {
            jsonrpc: "2.0",
            method: "repin/rerank",
            params: RerankParams { query, candidates },
        };

        let req_json = serde_json::to_string(&rpc_req)
            .map_err(|e| ModelError::ProviderError {
                provider: "agent".to_string(),
                message: format!("failed to serialize JSON-RPC request: {e}"),
            })?;

        let mut child = Command::new("sh")
            .arg("-c")
            .arg(&self.cmd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| ModelError::ProviderError {
                provider: "agent".to_string(),
                message: format!("failed to spawn agent command '{}': {e}", self.cmd),
            })?;

        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(req_json.as_bytes());
        }

        // Bounded wait with deadline timeout
        let (tx, rx) = std::sync::mpsc::channel();
        let timeout_ms = self.deadline_ms;

        std::thread::spawn(move || {
            let res = child.wait_with_output();
            let _ = tx.send(res);
        });

        let output = rx
            .recv_timeout(Duration::from_millis(timeout_ms))
            .map_err(|_| ModelError::Timeout { timeout_ms })?
            .map_err(|e| ModelError::ProviderError {
                provider: "agent".to_string(),
                message: format!("error reading agent output: {e}"),
            })?;

        if !output.status.success() {
            let stderr_msg = String::from_utf8_lossy(&output.stderr);
            return Err(ModelError::ProviderError {
                provider: "agent".to_string(),
                message: format!(
                    "agent exited with code {:?}: {}",
                    output.status.code(),
                    stderr_msg.trim()
                ),
            });
        }

        let stdout_str = String::from_utf8_lossy(&output.stdout);

        // Try parsing standard JSON-RPC response
        if let Ok(rpc_resp) = serde_json::from_str::<JsonRpcResponse>(&stdout_str) {
            if let Some(err) = rpc_resp.error {
                return Err(ModelError::ProviderError {
                    provider: "agent".to_string(),
                    message: format!("agent JSON-RPC returned error: {err}"),
                });
            }
            if let Some(res) = rpc_resp.result {
                let mut hits = Vec::new();
                for (rank, hit) in res.ranked.into_iter().enumerate() {
                    hits.push(RerankHit {
                        id: hit.id,
                        score: hit.score.unwrap_or(1.0 - (rank as f32 * 0.05).min(0.9)),
                        rank,
                    });
                }
                return Ok(hits);
            }
        }

        // Fallback: parse array of indices or IDs directly if returned
        if let Ok(indices) = serde_json::from_str::<Vec<usize>>(&stdout_str) {
            let mut hits = Vec::new();
            for (rank, &idx) in indices.iter().enumerate() {
                if idx < candidates.len() {
                    hits.push(RerankHit {
                        id: candidates[idx].id.clone(),
                        score: 1.0 - (rank as f32 * 0.05).min(0.9),
                        rank,
                    });
                }
            }
            return Ok(hits);
        }

        Err(ModelError::ProviderError {
            provider: "agent".to_string(),
            message: format!("unrecognized agent response format: {stdout_str}"),
        })
    }
}
