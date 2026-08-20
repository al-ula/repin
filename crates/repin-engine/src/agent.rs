use crate::ranking::{RankReason, RankedCandidate};
use std::io::Write;
use std::process::{Command, Stdio};

pub struct AgentReranker;

impl AgentReranker {
    pub fn rerank_with_shell_callback(
        query: &str,
        candidates: Vec<RankedCandidate>,
        shell_cmd: &str,
    ) -> Result<Vec<RankedCandidate>, String> {
        if candidates.is_empty() {
            return Ok(candidates);
        }

        let trimmed_cmd = shell_cmd.trim();
        if trimmed_cmd.is_empty() {
            return Err("Agent shell callback command cannot be empty".to_string());
        }

        // Construct structured prompt for the agent
        let mut prompt = format!(
            "You are an expert code intelligence reranker. Given a user query and a list of code candidates, rerank them in descending order of relevance.\nQuery: \"{}\"\n\nCandidates:\n",
            query
        );

        for (idx, c) in candidates.iter().enumerate() {
            let doc_summary = c
                .node
                .attributes
                .get("docSummary")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            prompt.push_str(&format!(
                "[{}] {} ({}) in {} {}\n",
                idx,
                c.node.name,
                c.node.kind.as_str(),
                c.node.path,
                if doc_summary.is_empty() {
                    "".to_string()
                } else {
                    format!("- {}", doc_summary)
                }
            ));
        }

        prompt.push_str("\nRespond ONLY with a JSON array of 0-based candidate indices in order of relevance, for example: [1, 0, 2]. Do not include any other text.");

        // Spawn the shell command callback with prompt piped via stdin
        let mut child = Command::new("sh")
            .arg("-c")
            .arg(trimmed_cmd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn shell callback '{}': {e}", trimmed_cmd))?;

        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(prompt.as_bytes());
        }

        let output = child
            .wait_with_output()
            .map_err(|e| format!("Error waiting on shell callback: {e}"))?;

        if !output.status.success() {
            let stderr_msg = String::from_utf8_lossy(&output.stderr);
            return Err(format!(
                "Agent shell callback exited with error code {:?}: {}",
                output.status.code(),
                stderr_msg.trim()
            ));
        }

        let stdout_str = String::from_utf8_lossy(&output.stdout).to_string();

        // Parse JSON array of indices from stdout
        let start_idx = stdout_str.find('[');
        let end_idx = stdout_str.rfind(']');

        if let (Some(start), Some(end)) = (start_idx, end_idx)
            && end > start
        {
            let json_str = &stdout_str[start..=end];
            if let Ok(indices) = serde_json::from_str::<Vec<usize>>(json_str) {
                let mut reordered = Vec::new();
                let mut seen = std::collections::HashSet::new();

                for idx in indices {
                    if idx < candidates.len() && seen.insert(idx) {
                        let mut item = candidates[idx].clone();
                        item.explanation.reasons.push(RankReason {
                            signal: "agent_rerank".to_string(),
                            score: 0.35,
                            detail: Some(format!("reranked via callback (`{}`)", trimmed_cmd)),
                        });
                        item.explanation.total_score += 0.35;
                        reordered.push(item);
                    }
                }

                // Add any candidates that weren't included in the response
                for (i, c) in candidates.into_iter().enumerate() {
                    if seen.insert(i) {
                        reordered.push(c);
                    }
                }

                return Ok(reordered);
            }
        }

        Err(format!(
            "Failed to parse JSON index array from agent output: {}",
            stdout_str.trim()
        ))
    }
}
