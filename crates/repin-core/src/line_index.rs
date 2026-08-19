use serde::{Deserialize, Serialize};
use std::fmt;

pub const CHECKPOINT_STRIDE: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Position {
    pub line: u32,
    pub column: u32,
}

impl Position {
    pub fn new(line: u32, column: u32) -> Self {
        Self { line, column }
    }
}

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ByteSpan {
    pub start: usize,
    pub end: usize,
}

impl ByteSpan {
    pub fn new(start: usize, end: usize) -> Self {
        assert!(start <= end, "start must not exceed end");
        Self { start, end }
    }

    pub fn len(&self) -> usize {
        self.end - self.start
    }

    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Range {
    pub span: ByteSpan,
    pub start: Position,
    pub end: Position,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScalarCheckpoint {
    pub byte_delta: u32,
    pub scalar_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecialLineIndex {
    pub line_index: usize,
    pub checkpoints: Vec<ScalarCheckpoint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineIndex {
    pub content_length: usize,
    pub line_starts: Vec<usize>,
    pub special_lines: Vec<SpecialLineIndex>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum LineIndexError {
    #[error("offset {offset} exceeds content length {content_length}")]
    OffsetOutOfBounds {
        offset: usize,
        content_length: usize,
    },
    #[error("offset {0} lands inside a multi-byte Unicode scalar or invalid UTF-8 sequence")]
    MidScalarOffset(usize),
}

impl LineIndex {
    pub fn build(bytes: &[u8]) -> Self {
        let content_length = bytes.len();
        let mut line_starts = vec![0];
        let mut special_lines = Vec::new();

        let mut i = 0;
        let mut current_line_start = 0;
        let mut current_line_idx = 0;
        let mut current_line_is_ascii = true;

        while i < bytes.len() {
            let b = bytes[i];
            if b >= 0x80 {
                current_line_is_ascii = false;
            }

            if b == b'\n' {
                let next_start = i + 1;
                if !current_line_is_ascii {
                    let line_slice = &bytes[current_line_start..i];
                    let checkpoints = Self::compute_checkpoints(line_slice);
                    special_lines.push(SpecialLineIndex {
                        line_index: current_line_idx,
                        checkpoints,
                    });
                }
                line_starts.push(next_start);
                current_line_start = next_start;
                current_line_idx += 1;
                current_line_is_ascii = true;
                i += 1;
            } else if b == b'\r' && i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
                let next_start = i + 2;
                if !current_line_is_ascii {
                    let line_slice = &bytes[current_line_start..i];
                    let checkpoints = Self::compute_checkpoints(line_slice);
                    special_lines.push(SpecialLineIndex {
                        line_index: current_line_idx,
                        checkpoints,
                    });
                }
                line_starts.push(next_start);
                current_line_start = next_start;
                current_line_idx += 1;
                current_line_is_ascii = true;
                i += 2;
            } else {
                i += 1;
            }
        }

        if !current_line_is_ascii {
            let line_slice = &bytes[current_line_start..bytes.len()];
            let checkpoints = Self::compute_checkpoints(line_slice);
            special_lines.push(SpecialLineIndex {
                line_index: current_line_idx,
                checkpoints,
            });
        }

        Self {
            content_length,
            line_starts,
            special_lines,
        }
    }

    fn compute_checkpoints(line_bytes: &[u8]) -> Vec<ScalarCheckpoint> {
        let mut checkpoints = Vec::new();
        let mut byte_offset = 0;
        let mut scalar_count = 0;
        let mut next_checkpoint_boundary = CHECKPOINT_STRIDE;

        while byte_offset < line_bytes.len() {
            if byte_offset >= next_checkpoint_boundary {
                checkpoints.push(ScalarCheckpoint {
                    byte_delta: byte_offset as u32,
                    scalar_count,
                });
                next_checkpoint_boundary = byte_offset + CHECKPOINT_STRIDE;
            }

            let slice = &line_bytes[byte_offset..];
            match std::str::from_utf8(slice) {
                Ok(s) => {
                    for ch in s.chars() {
                        let ch_len = ch.len_utf8();
                        if byte_offset >= next_checkpoint_boundary {
                            checkpoints.push(ScalarCheckpoint {
                                byte_delta: byte_offset as u32,
                                scalar_count,
                            });
                            next_checkpoint_boundary = byte_offset + CHECKPOINT_STRIDE;
                        }
                        byte_offset += ch_len;
                        scalar_count += 1;
                    }
                    break;
                }
                Err(err) => {
                    let valid_len = err.valid_up_to();
                    if valid_len > 0 {
                        let valid_str =
                            std::str::from_utf8(&slice[..valid_len]).unwrap_or_default();
                        for ch in valid_str.chars() {
                            let ch_len = ch.len_utf8();
                            if byte_offset >= next_checkpoint_boundary {
                                checkpoints.push(ScalarCheckpoint {
                                    byte_delta: byte_offset as u32,
                                    scalar_count,
                                });
                                next_checkpoint_boundary = byte_offset + CHECKPOINT_STRIDE;
                            }
                            byte_offset += ch_len;
                            scalar_count += 1;
                        }
                    }

                    if let Some(error_len) = err.error_len() {
                        byte_offset += error_len;
                        scalar_count += 1;
                    } else {
                        byte_offset += slice.len() - valid_len;
                        scalar_count += 1;
                    }
                }
            }
        }

        checkpoints
    }

    pub fn offset_to_position(
        &self,
        bytes: &[u8],
        offset: usize,
    ) -> Result<Position, LineIndexError> {
        if offset > self.content_length {
            return Err(LineIndexError::OffsetOutOfBounds {
                offset,
                content_length: self.content_length,
            });
        }

        let line_idx = match self.line_starts.binary_search(&offset) {
            Ok(idx) => idx,
            Err(idx) => idx.saturating_sub(1),
        };

        let line_start = self.line_starts[line_idx];
        let byte_delta = offset - line_start;

        let special = self
            .special_lines
            .binary_search_by_key(&line_idx, |s| s.line_index)
            .ok()
            .map(|idx| &self.special_lines[idx]);

        let col_1based = if let Some(special) = special {
            let mut start_byte_delta = 0;
            let mut start_scalar_count = 0;

            for cp in &special.checkpoints {
                if (cp.byte_delta as usize) <= byte_delta {
                    start_byte_delta = cp.byte_delta as usize;
                    start_scalar_count = cp.scalar_count;
                } else {
                    break;
                }
            }

            let slice = &bytes[line_start + start_byte_delta..offset];
            let remaining_scalars = match std::str::from_utf8(slice) {
                Ok(s) => s.chars().count() as u32,
                Err(_) => {
                    let mut count = 0;
                    let mut cur = slice;
                    while !cur.is_empty() {
                        match std::str::from_utf8(cur) {
                            Ok(s) => {
                                count += s.chars().count() as u32;
                                break;
                            }
                            Err(e) => {
                                let valid = e.valid_up_to();
                                if valid > 0 {
                                    count += std::str::from_utf8(&cur[..valid])
                                        .map(|s| s.chars().count() as u32)
                                        .unwrap_or(0);
                                }
                                if let Some(elen) = e.error_len() {
                                    count += 1;
                                    cur = &cur[valid + elen..];
                                } else {
                                    count += 1;
                                    break;
                                }
                            }
                        }
                    }
                    count
                }
            };

            start_scalar_count + remaining_scalars + 1
        } else {
            (byte_delta as u32) + 1
        };

        Ok(Position {
            line: (line_idx as u32) + 1,
            column: col_1based,
        })
    }

    pub fn span_to_range(&self, bytes: &[u8], span: ByteSpan) -> Result<Range, LineIndexError> {
        let start = self.offset_to_position(bytes, span.start)?;
        let end = self.offset_to_position(bytes, span.end)?;
        Ok(Range { span, start, end })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ascii_line_index() {
        let bytes = b"hello\nworld\nfoo";
        let index = LineIndex::build(bytes);
        assert_eq!(index.line_starts, vec![0, 6, 12]);
        assert!(index.special_lines.is_empty());

        let pos = index.offset_to_position(bytes, 0).unwrap();
        assert_eq!(pos, Position::new(1, 1));

        let pos = index.offset_to_position(bytes, 5).unwrap();
        assert_eq!(pos, Position::new(1, 6));

        let pos = index.offset_to_position(bytes, 6).unwrap();
        assert_eq!(pos, Position::new(2, 1));

        let pos = index.offset_to_position(bytes, 12).unwrap();
        assert_eq!(pos, Position::new(3, 1));
    }

    #[test]
    fn test_crlf_line_index() {
        let bytes = b"abc\r\ndef\r\n";
        let index = LineIndex::build(bytes);
        assert_eq!(index.line_starts, vec![0, 5, 10]);

        let pos = index.offset_to_position(bytes, 5).unwrap();
        assert_eq!(pos, Position::new(2, 1));
    }

    #[test]
    fn test_unicode_scalar_column() {
        let s = "fn 🦀_run() {}\n";
        let bytes = s.as_bytes();
        let index = LineIndex::build(bytes);
        assert_eq!(index.special_lines.len(), 1);

        let crab_start = s.find('🦀').unwrap();
        let pos_before_crab = index.offset_to_position(bytes, crab_start).unwrap();
        assert_eq!(pos_before_crab, Position::new(1, 4));

        let after_crab = crab_start + '🦀'.len_utf8();
        let pos_after_crab = index.offset_to_position(bytes, after_crab).unwrap();
        assert_eq!(pos_after_crab, Position::new(1, 5));
    }
}
