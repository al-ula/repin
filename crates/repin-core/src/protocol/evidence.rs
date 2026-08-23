use crate::line_index::Range;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Evidence {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<Range>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_at: Option<u64>,
}

impl Evidence {
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            range: None,
            preview: None,
            observed_at: None,
        }
    }

    pub fn with_range(mut self, range: Range) -> Self {
        self.range = Some(range);
        self
    }

    pub fn with_preview(mut self, preview: impl Into<String>) -> Self {
        self.preview = Some(preview.into());
        self
    }
}
