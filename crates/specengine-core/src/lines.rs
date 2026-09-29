//! Byte offset → 1-based line, counting `\n` (a CRLF ending counts once).

pub(crate) struct LineIndex {
    /// Offsets of every `\n`.
    newlines: Vec<usize>,
}

impl LineIndex {
    pub fn new(bytes: &[u8]) -> Self {
        let newlines = bytes
            .iter()
            .enumerate()
            .filter_map(|(offset, &byte)| (byte == b'\n').then_some(offset))
            .collect();
        Self { newlines }
    }

    /// The 1-based line holding the byte at `offset`.
    pub fn line(&self, offset: usize) -> usize {
        self.newlines.partition_point(|&newline| newline < offset) + 1
    }
}
