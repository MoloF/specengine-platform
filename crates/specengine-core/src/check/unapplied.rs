//! The unapplied amendments of one walk (task spec `health-left`, "Rules
//! and edge cases"; canon `proposal-amendment`, "Unapplied"): every walked
//! document that is live ([`is_live`]) and names a node it amends
//! ([`amended_refs`] non-empty), whatever raised it, its class or how its
//! links fare. Nothing here names a class, a status, a kind or a link
//! type: the live rule and the amended nodes come from their one
//! predicate each.
//!
//! A file whose front-matter does not read ([`front_matter_failure`]) is
//! not judged: its `links:` cannot be read, so it is reported with the
//! parser's finding, never listed and never taken for none. A file that
//! was not read or parsed is no document here: the walk's gap
//! ([`super::walk_gap`]) names it, and a caller stops on it first.
//!
//! It reads nothing and writes nothing; the result depends only on the
//! input, whatever the order of `input.files`: both lists are by path,
//! byte order. It never blocks: what it lists is no finding.

use specengine_model::Diagnostic;

use super::input::{CheckFile, CheckInput};
use super::render::is_live;
use super::text::front_matter_failure;
use crate::create::amended_refs;

/// What [`unapplied_amendments`] finds in one walk.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Unapplied<'a> {
    /// The live documents that amend a node, by path.
    pub amendments: Vec<&'a CheckFile>,
    /// The files whose front-matter does not read, by path, each with the
    /// parser's first finding that failed it: not judged.
    pub not_judged: Vec<(&'a CheckFile, &'a Diagnostic)>,
}

/// The unapplied amendments of `input`, and the files the rule could not
/// judge.
pub fn unapplied_amendments(input: &CheckInput) -> Unapplied<'_> {
    let mut files: Vec<&CheckFile> = input.files.iter().collect();
    files.sort_by(|a, b| a.path.cmp(&b.path));
    let mut found = Unapplied::default();
    for file in files {
        let Some(parsed) = &file.parsed else {
            continue;
        };
        if let Some(failure) = front_matter_failure(parsed) {
            found.not_judged.push((file, failure));
        } else if is_live(parsed) && !amended_refs(parsed).is_empty() {
            found.amendments.push(file);
        }
    }
    found
}
