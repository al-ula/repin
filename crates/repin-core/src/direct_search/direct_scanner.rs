use super::direct_regex::{DirectRegex, DirectRegexError};
use crate::line_index::{ByteSpan, LineIndex};
use crate::ports::fs::FileSnapshot;
use crate::protocol::evidence::Evidence;

pub struct DirectScanner;

impl DirectScanner {
    pub fn scan_snapshot(
        regex: &DirectRegex,
        snapshot: &FileSnapshot,
        max_matches: usize,
    ) -> Result<Vec<Evidence>, DirectRegexError> {
        let bytes = &snapshot.content;
        let mut results = Vec::new();

        let line_index = LineIndex::build(bytes);

        for (start, end) in regex.find_iter(bytes) {
            if results.len() >= max_matches {
                break;
            }

            let span = ByteSpan::new(start, end);
            if let Ok(range) = line_index.span_to_range(bytes, span) {
                let line_start_offset = line_index.line_starts[(range.start.line as usize) - 1];
                let line_end_offset = line_index
                    .line_starts
                    .get(range.end.line as usize)
                    .copied()
                    .unwrap_or(bytes.len());

                let preview_bytes = &bytes[line_start_offset..line_end_offset];
                let preview_text = String::from_utf8_lossy(preview_bytes).trim().to_string();

                let redacted = Self::redact_sensitive(&preview_text);

                results.push(
                    Evidence::new(&snapshot.path)
                        .with_range(range)
                        .with_preview(redacted),
                );
            }
        }

        Ok(results)
    }

    fn redact_sensitive(text: &str) -> String {
        let sensitive_keys = ["api_key", "secret", "password", "token", "auth"];
        let mut out = text.to_string();
        for key in &sensitive_keys {
            if out.to_lowercase().contains(key) {
                // If it contains an equals or colon, redact after
                if let Some(idx) = out.find('=') {
                    out.replace_range(idx + 1.., " [REDACTED]");
                } else if let Some(idx) = out.find(':') {
                    out.replace_range(idx + 1.., " [REDACTED]");
                }
            }
        }
        out
    }
}
