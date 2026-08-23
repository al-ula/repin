use crate::ports::model::{
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

    fn rerank(
        &self,
        query: &str,
        candidates: &[RerankCandidate],
    ) -> Result<Vec<RerankHit>, ModelError> {
        if candidates.is_empty() {
            return Ok(Vec::new());
        }

        let rpc_request = JsonRpcRequest {
            jsonrpc: "2.0",
            method: "repin/rerank",
            params: RerankParams { query, candidates },
        };
        let request_json =
            serde_json::to_string(&rpc_request).map_err(|error| ModelError::ProviderError {
                provider: "agent".to_string(),
                message: format!("failed to serialize JSON-RPC request: {error}"),
            })?;

        let mut child = Command::new("sh")
            .arg("-c")
            .arg(&self.cmd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| ModelError::ProviderError {
                provider: "agent".to_string(),
                message: format!("failed to spawn agent command '{}': {error}", self.cmd),
            })?;

        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(request_json.as_bytes());
        }

        let (sender, receiver) = std::sync::mpsc::channel();
        let timeout_ms = self.deadline_ms;
        std::thread::spawn(move || {
            let result = child.wait_with_output();
            let _ = sender.send(result);
        });

        let output = receiver
            .recv_timeout(Duration::from_millis(timeout_ms))
            .map_err(|_| ModelError::Timeout { timeout_ms })?
            .map_err(|error| ModelError::ProviderError {
                provider: "agent".to_string(),
                message: format!("error reading agent output: {error}"),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ModelError::ProviderError {
                provider: "agent".to_string(),
                message: format!(
                    "agent exited with code {:?}: {}",
                    output.status.code(),
                    stderr.trim()
                ),
            });
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        if let Ok(response) = serde_json::from_str::<JsonRpcResponse>(&stdout) {
            if let Some(error) = response.error {
                return Err(ModelError::ProviderError {
                    provider: "agent".to_string(),
                    message: format!("agent JSON-RPC returned error: {error}"),
                });
            }
            if let Some(result) = response.result {
                let hits = result
                    .ranked
                    .into_iter()
                    .enumerate()
                    .map(|(rank, hit)| RerankHit {
                        id: hit.id,
                        score: hit
                            .score
                            .unwrap_or_else(|| 1.0 - (rank as f32 * 0.05).min(0.9)),
                        rank,
                    })
                    .collect();
                return Ok(hits);
            }
        }

        if let Ok(indices) = serde_json::from_str::<Vec<usize>>(&stdout) {
            let hits = indices
                .iter()
                .enumerate()
                .filter_map(|(rank, &index)| {
                    candidates.get(index).map(|candidate| RerankHit {
                        id: candidate.id.clone(),
                        score: 1.0 - (rank as f32 * 0.05).min(0.9),
                        rank,
                    })
                })
                .collect();
            return Ok(hits);
        }

        Err(ModelError::ProviderError {
            provider: "agent".to_string(),
            message: format!("unrecognized agent response format: {stdout}"),
        })
    }
}
