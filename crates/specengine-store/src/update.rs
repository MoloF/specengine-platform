//! A proposal's `update` over a worktree (task spec `proposal-apply`,
//! creation steps 2–4, apply steps 4–6): core's pure half
//! ([`specengine_core::patch`]) with the parser's panics caught, the span
//! hash, and the validation of a patched file over a [`Source`]'s fresh
//! parse. Nothing is written.

use std::fmt;
use std::panic::{self, AssertUnwindSafe};

use specengine_core::patch::{PatchCheck, StructureError, Update, span_bytes};
use specengine_model::{IdScheme, Node, ParsedFile};

use crate::b3_hash;
use crate::check::{CheckSetup, check_input};
use crate::queue::ProposalFinding;
use crate::source::Source;

/// `b3:` of `node`'s span bytes in `bytes` (the file it was parsed from):
/// `spec show`'s `span_hash`, a proposal's `base_hash`.
pub fn span_hash(bytes: &[u8], node: &Node) -> String {
    b3_hash(span_bytes(bytes, node))
}

/// Why an update of one file was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateError {
    /// The structure check ([`specengine_core::patch::check_structure`]).
    Structure(StructureError),
    /// The spec parser panicked on the patched bytes.
    ParserPanicked,
}

impl fmt::Display for UpdateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Structure(error) => error.fmt(f),
            Self::ParserPanicked => f.write_str("the spec parser failed on the edited file"),
        }
    }
}

impl std::error::Error for UpdateError {}

/// [`specengine_core::patch::update`]: node `ord` of `parsed` (the parse
/// of `bytes`, named `path`) replaced by `new_text` and checked; a parser
/// panic is [`UpdateError::ParserPanicked`].
pub fn update_file(
    path: &str,
    bytes: &[u8],
    parsed: &ParsedFile,
    ord: usize,
    new_text: &str,
    scheme: &IdScheme,
) -> Result<Update, UpdateError> {
    panic::catch_unwind(AssertUnwindSafe(|| {
        specengine_core::patch::update(path, bytes, parsed, ord, new_text, scheme)
    }))
    .map_err(|_| UpdateError::ParserPanicked)?
    .map_err(UpdateError::Structure)
}

/// The findings an edit of `path` to `patched` introduces in `source`'s
/// tree (creation step 4): the tree read and parsed once
/// ([`check_input`]), checked as it is and with the file replaced, under
/// `setup` and `today` ([`PatchCheck::introduced`]). Never a refusal: a
/// parser panic on the patched bytes gives no findings.
pub fn introduced_findings(
    source: &dyn Source,
    setup: &CheckSetup,
    today: &str,
    path: &str,
    patched: Vec<u8>,
) -> Vec<ProposalFinding> {
    let input = check_input(source, &setup.project.scheme);
    let check = PatchCheck {
        scheme: &setup.project.scheme,
        paths: &setup.project.paths,
        config: &setup.config,
        baseline: &setup.baseline,
        today,
    };
    panic::catch_unwind(AssertUnwindSafe(|| check.introduced(&input, path, patched)))
        .map(|findings| findings.iter().map(ProposalFinding::from).collect())
        .unwrap_or_default()
}
