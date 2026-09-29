//! Byte spans into the original file.

use std::ops::Range;

use serde::{Deserialize, Serialize};

/// A half-open byte range `[start, end)` into the file's original bytes,
/// BOM included and line endings untouched; serialised as `[start, end]`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(from = "(usize, usize)", into = "(usize, usize)")]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub const fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    pub const fn is_empty(&self) -> bool {
        self.end <= self.start
    }

    pub const fn range(&self) -> Range<usize> {
        self.start..self.end
    }

    /// The span moved right by `offset` bytes.
    pub const fn shifted(&self, offset: usize) -> Self {
        Self::new(self.start + offset, self.end + offset)
    }

    /// `other` lies inside this span.
    pub const fn contains(&self, other: Span) -> bool {
        self.start <= other.start && other.end <= self.end
    }
}

impl From<Range<usize>> for Span {
    fn from(range: Range<usize>) -> Self {
        Self::new(range.start, range.end)
    }
}

impl From<(usize, usize)> for Span {
    fn from((start, end): (usize, usize)) -> Self {
        Self::new(start, end)
    }
}

impl From<Span> for (usize, usize) {
    fn from(span: Span) -> Self {
        (span.start, span.end)
    }
}
