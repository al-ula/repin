use regex::bytes::{Regex, RegexBuilder};

#[derive(Debug, thiserror::Error)]
pub enum DirectRegexError {
    #[error("invalid regex query: {0}")]
    CompileError(String),
    #[error("regex pattern exceeded size limits")]
    SizeLimitExceeded,
}

pub struct DirectRegex {
    inner: Regex,
}

impl DirectRegex {
    pub const DEFAULT_SIZE_LIMIT: usize = 10 * 1024 * 1024; // 10MB compiled limit

    pub fn compile(pattern: &str, is_regex: bool) -> Result<Self, DirectRegexError> {
        let pattern_str = if is_regex {
            pattern.to_string()
        } else {
            regex::escape(pattern)
        };

        let regex = RegexBuilder::new(&pattern_str)
            .size_limit(Self::DEFAULT_SIZE_LIMIT)
            .multi_line(true)
            .build()
            .map_err(|e| DirectRegexError::CompileError(e.to_string()))?;

        Ok(Self { inner: regex })
    }

    pub fn find_iter<'a>(&'a self, text: &'a [u8]) -> impl Iterator<Item = (usize, usize)> + 'a {
        self.inner.find_iter(text).map(|m| (m.start(), m.end()))
    }
}
