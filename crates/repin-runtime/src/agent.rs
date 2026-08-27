use repin_retrieval::ranking::{RankReason, RankedCandidate};
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

#[derive(Debug, Default, Clone, Copy)]
pub struct AgentReranker;

impl AgentReranker {
    pub fn rerank_with_shell_callback(
        query: &str,
        candidates: Vec<RankedCandidate>,
        shell_cmd: &str,
        deadline_ms: Option<u64>,
    ) -> Result<Vec<RankedCandidate>, String> {
        if candidates.is_empty() {
            return Ok(candidates);
        }
        let trimmed_cmd = shell_cmd.trim();
        if trimmed_cmd.is_empty() {
            return Err("Agent shell callback command cannot be empty".to_string());
        }

        let mut prompt = format!(
            "You are an expert code intelligence reranker. Given a user query and a list of code candidates, rerank them in descending order of relevance.\nQuery: \"{query}\"\n\nCandidates:\n"
        );
        for (index, candidate) in candidates.iter().enumerate() {
            let doc_summary = candidate
                .node
                .attributes
                .get("docSummary")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            prompt.push_str(&format!(
                "[{index}] {} ({}) in {}{}\n",
                candidate.node.name,
                candidate.node.kind.as_str(),
                candidate.node.path,
                if doc_summary.is_empty() {
                    String::new()
                } else {
                    format!(" - {doc_summary}")
                }
            ));
        }
        prompt.push_str(
            "\nRespond ONLY with a JSON array of 0-based candidate indices in order of relevance, for example: [1, 0, 2]. Do not include any other text.",
        );

        let mut child = Command::new("sh")
            .arg("-c")
            .arg(trimmed_cmd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("failed to spawn shell callback '{trimmed_cmd}': {error}"))?;
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(prompt.as_bytes());
        }
        let output = if let Some(deadline) = deadline_ms {
            let (tx, rx) = mpsc::channel();
            let _waiter = std::thread::spawn(move || tx.send(child.wait_with_output()));
            match rx.recv_timeout(Duration::from_millis(deadline)) {
                Ok(Ok(out)) => out,
                Ok(Err(error)) => return Err(format!("error waiting on shell callback: {error}")),
                Err(_) => {
                    return Err(format!(
                        "agent shell callback exceeded deadline of {deadline} ms"
                    ));
                }
            }
        } else {
            child
                .wait_with_output()
                .map_err(|error| format!("error waiting on shell callback: {error}"))?
        };
        if !output.status.success() {
            return Err(format!(
                "agent shell callback exited with error code {:?}: {}",
                output.status.code(),
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let (Some(start), Some(end)) = (stdout.find('['), stdout.rfind(']')) else {
            return Err(format!(
                "failed to parse JSON index array from agent output: {}",
                stdout.trim()
            ));
        };
        if end <= start {
            return Err(format!(
                "failed to parse JSON index array from agent output: {}",
                stdout.trim()
            ));
        }
        let indices = serde_json::from_str::<Vec<usize>>(&stdout[start..=end])
            .map_err(|error| format!("failed to parse agent ranking: {error}"))?;
        let mut reordered = Vec::with_capacity(candidates.len());
        let mut seen = std::collections::HashSet::new();
        for index in indices {
            if index < candidates.len() && seen.insert(index) {
                let mut item = candidates[index].clone();
                item.explanation.reasons.push(RankReason {
                    signal: "agent_rerank".to_string(),
                    score: 0.35,
                    detail: Some("reranked via caller-supplied callback".to_string()),
                });
                item.explanation.total_score += 0.35;
                reordered.push(item);
            }
        }
        for (index, candidate) in candidates.into_iter().enumerate() {
            if seen.insert(index) {
                reordered.push(candidate);
            }
        }
        Ok(reordered)
    }
}
