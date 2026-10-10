//! A node's revision (ADR-0040, task spec `norm-hash` "Revision"): a
//! section's is the `rev=N` attribute of its own `{#ID ...}` heading, a
//! document's its front-matter `rev:`; never inherited. None written is
//! revision 1; one written but unreadable (`bad-rev`, or a front-matter
//! that failed to parse) is unknown, never 1.

use specengine_model::ParsedFile;

use crate::check::front_matter_failure;

/// The key a revision is written under: the heading attribute `rev=N` and
/// the front-matter `rev:`.
pub(crate) const REV_KEY: &str = "rev";

/// The largest revision: nine digits.
pub(crate) const MAX_REVISION: u32 = 999_999_999;

/// What a node's own revision is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Revision {
    /// Written and read: 1 to 999 999 999.
    Written(u32),
    /// None written: revision 1.
    Unwritten,
    /// Written but unreadable, or no node at that position: unknown.
    Unreadable,
}

impl Revision {
    /// The revision's number: `Unwritten` is 1, `Unreadable` none.
    pub const fn number(self) -> Option<u32> {
        match self {
            Self::Written(rev) => Some(rev),
            Self::Unwritten => Some(1),
            Self::Unreadable => None,
        }
    }
}

/// The revision of node `ord` of `parsed` (0 the document, then its
/// sections in source order).
///
/// A section: its `Node.rev`; `rev` among its heading's `attrs` without
/// one is unreadable. A document: unreadable when its front-matter failed
/// ([`front_matter_failure`]) or its `extra` holds `rev` (a mistyped, a
/// repeated or an out-of-range value), else its `Node.rev`. A number
/// outside 1 to 999 999 999 (a parse of another format) is unreadable.
pub fn revision(parsed: &ParsedFile, ord: usize) -> Revision {
    let Some(node) = parsed.nodes.get(ord) else {
        return Revision::Unreadable;
    };
    if ord == 0 {
        let unread = front_matter_failure(parsed).is_some()
            || node
                .extra
                .iter()
                .flatten()
                .any(|entry| entry.key == REV_KEY);
        if unread {
            return Revision::Unreadable;
        }
    }
    match node.rev {
        Some(rev) if (1..=MAX_REVISION).contains(&rev) => Revision::Written(rev),
        Some(_) => Revision::Unreadable,
        // A section's `rev` the parser could not read.
        None if node.attrs.iter().any(|(key, _)| key == REV_KEY) => Revision::Unreadable,
        None => Revision::Unwritten,
    }
}
