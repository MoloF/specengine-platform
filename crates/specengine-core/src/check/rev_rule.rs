//! The rule `rev-not-raised` and `spec bump`'s rows (canon
//! `node-revision`, "Raising"; task spec `rev-rule`, slice 4b-2): the
//! pure half. Nothing is read, written or hashed here: the store hands in
//! both trees' parses, both locks' bytes and the hash function
//! (`specengine_store::b3_hash`), and adds the findings to the checked
//! run's report ([`super::add_findings`]).
//!
//! - **Changed** ([`changed_nodes`]): the nodes with a canonical ID both at
//!   `HEAD` and in the checked tree (the k-th holder of an ID at `HEAD`
//!   paired with its k-th holder in the checked tree, each in path and
//!   source order), whose checked meaning hash is non-null and differs
//!   from `HEAD`'s. A file shared by both sides, at the same path with the
//!   same bytes, changes none of its nodes.
//! - **Locks** ([`rule_lock`]): read only when a node changed, by core's
//!   `parse_lock` without the build's recipes; its `norm_recipe` must be
//!   this build's, `code_recipe` is never compared.
//! - **The rule** ([`judge`]): a changed node whose revision is not above
//!   `HEAD`'s (or is unreadable) is a row; `required` when `HEAD`'s lock
//!   holds an entry vouching for it ([`vouches`]: at `HEAD`'s revision, its
//!   `spec_hash` `HEAD`'s meaning hash), or a `[[tested]]` block so (one
//!   entry in each count), and the checked lock does not carry the new
//!   hash on each such entry and block (the same key or node, at the same
//!   revision), which is the finding; else `suggested`.
//! - **The raise** a row advises: `HEAD`'s revision plus one, written as
//!   an apply writes it ([`raised_line`]); none, and only the
//!   `--editorial` way out, where an apply refuses it: `HEAD`'s revision
//!   unreadable or at [`REV_MAX`], a document of a closed class whose
//!   contract admits no `rev` ([`closed_against_rev`]), a line that cannot
//!   be written. A row's line and raise are computed only for a finding,
//!   or for every row when `spec bump` asks for the rows.
//!
//! Genre-neutral: canonical IDs, revisions, meaning inputs and the lock
//! only; no prefix, kind, path or file of a project.

use std::collections::BTreeMap;

use serde::Serialize;
use specengine_model::{IdScheme, ParsedFile, Severity};

use super::config::Classes;
use super::input::CheckFile;
use super::report::{Finding, Fix};
use super::resolve::{Resolver, canonical_id};
use super::text::FileText;
use crate::Paths;
use crate::lock::{LOCK_FILE, Lock, LockError, REV_MAX, parse_lock, vouches};
use crate::norm::{NORM_RECIPE, norm_input};
use crate::raise::{closed_against_rev, raised, raised_line};
use crate::revision::{Revision, revision};

/// The rule's code.
pub const REV_NOT_RAISED: &str = "rev-not-raised";

/// A node whose meaning the checked tree changed against `HEAD`.
#[derive(Debug, Clone)]
pub struct Changed<'a> {
    /// The canonical ID (`slug/ID` when feature-scoped).
    pub id: String,
    /// Its file in the checked tree.
    pub path: String,
    /// 0 the document, then its sections in source order.
    pub ord: usize,
    pub head_rev: Revision,
    /// Its revision in the checked tree.
    pub rev: Revision,
    /// `HEAD`'s meaning hash; `None` when null.
    pub head_hash: Option<String>,
    /// The checked meaning hash.
    pub hash: String,
    /// Its holder in the checked tree: its line and its raise are read
    /// from it only when [`judge`] needs them.
    at: Holder<'a>,
}

impl Changed<'_> {
    /// The checked revision is readable and above `HEAD`'s readable one.
    pub fn raised(&self) -> bool {
        matches!(
            (self.head_rev.number(), self.rev.number()),
            (Some(head), Some(now)) if now > head
        )
    }

    /// The raise an apply writes: `HEAD`'s revision plus one ([`raised`])
    /// and the line it writes in the checked file ([`raised_line`]); `None`
    /// when an apply refuses it: `HEAD`'s revision unreadable or at
    /// [`REV_MAX`], a document of a closed class whose contract in
    /// `classes` admits no `rev` ([`closed_against_rev`]), or a line that
    /// cannot be written.
    fn raise(&self, scheme: &IdScheme, classes: &Classes) -> Option<(u32, Fix)> {
        let to = raised(self.head_rev).ok()?;
        let at = self.at;
        if at.ord == 0 && closed_against_rev(at.parsed, classes).is_some() {
            return None;
        }
        let (span, text) =
            raised_line(&at.file.path, &at.file.bytes, at.parsed, scheme, at.ord, to)?;
        Some((to, Fix { span, text }))
    }
}

/// One holder of a canonical ID on one side.
#[derive(Debug, Clone, Copy)]
struct Holder<'a> {
    file: &'a CheckFile,
    parsed: &'a ParsedFile,
    ord: usize,
}

/// Every node with an ID of `files`, by canonical ID, each ID's holders in
/// path and source order. A file not read or not parsed holds none.
fn holders<'a>(
    files: &[&'a CheckFile],
    scheme: &IdScheme,
    paths: &Paths,
) -> BTreeMap<String, Vec<Holder<'a>>> {
    let mut sorted: Vec<&'a CheckFile> = files.to_vec();
    sorted.sort_by(|a, b| a.path.cmp(&b.path));
    let resolver = Resolver::of_sorted(&sorted, scheme, &paths.features);
    let mut by_id: BTreeMap<String, Vec<Holder<'a>>> = BTreeMap::new();
    for file in sorted {
        let Some(parsed) = file.parsed.as_ref().filter(|_| file.read_error.is_none()) else {
            continue;
        };
        for (ord, node) in parsed.nodes.iter().enumerate() {
            if let Some(id) = &node.id {
                by_id
                    .entry(canonical_id(&resolver, scheme, &file.path, id, None))
                    .or_default()
                    .push(Holder { file, parsed, ord });
            }
        }
    }
    by_id
}

/// The nodes `checked` changed against `head` (see the module
/// documentation), by canonical ID; `hash` turns a meaning input into its
/// `norm_hash`.
pub fn changed_nodes<'a>(
    head: &[&'a CheckFile],
    checked: &[&'a CheckFile],
    scheme: &IdScheme,
    paths: &Paths,
    hash: &dyn Fn(&[u8]) -> String,
) -> Vec<Changed<'a>> {
    let at_head = holders(head, scheme, paths);
    let now = holders(checked, scheme, paths);
    let mut changed = Vec::new();
    for (id, holders) in &now {
        let Some(before) = at_head.get(id) else {
            continue;
        };
        for (was, is) in before.iter().zip(holders) {
            let same_file = std::ptr::eq(was.file, is.file)
                || (was.file.path == is.file.path && was.file.bytes == is.file.bytes);
            if same_file && was.ord == is.ord {
                continue;
            }
            let Some(input) = norm_input(is.parsed, &is.file.bytes, is.ord) else {
                continue;
            };
            let head_input = norm_input(was.parsed, &was.file.bytes, was.ord);
            if head_input.as_deref() == Some(input.as_slice()) {
                continue;
            }
            changed.push(Changed {
                id: id.clone(),
                path: is.file.path.clone(),
                ord: is.ord,
                head_rev: revision(was.parsed, was.ord),
                rev: revision(is.parsed, is.ord),
                head_hash: head_input.map(|input| hash(&input)),
                hash: hash(&input),
                at: *is,
            });
        }
    }
    changed
}

/// The line of a holder's node in `text`, its file's: its heading's, or
/// its document's `id:` key's (1 when not found).
fn node_line(text: &FileText<'_>, holder: &Holder<'_>) -> usize {
    if holder.ord == 0 {
        return text.key_line("id").unwrap_or(1);
    }
    holder.parsed.nodes[holder.ord]
        .heading
        .map_or(1, |span| text.line(span.start))
}

/// A lock as the rule reads it: UTF-8, any TOML layout core's `parse_lock`
/// reads, its `norm_recipe` this build's ([`NORM_RECIPE`]); `code_recipe`
/// is not compared. The error's message names neither the file nor the
/// line: the caller names both.
pub fn rule_lock(bytes: &[u8]) -> Result<Lock, LockError> {
    let text = std::str::from_utf8(bytes).map_err(|_| LockError {
        line: None,
        message: "not UTF-8: SpecEngine writes it".to_owned(),
    })?;
    let lock = parse_lock(text, None)?;
    if lock.recipes.norm != NORM_RECIPE {
        return Err(LockError {
            line: None,
            message: format!(
                "its norm_recipe is `{}`, this build's `{NORM_RECIPE}`: its spec_hash values are \
                 not this build's",
                lock.recipes.norm
            ),
        });
    }
    Ok(lock)
}

/// Whether the rule finds a row's node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Need {
    /// The rule's finding: `HEAD`'s lock vouches for its old text.
    Required,
    /// No entry vouches for it, or each was refreshed.
    Suggested,
}

impl Need {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Required => "required",
            Self::Suggested => "suggested",
        }
    }
}

/// One node changed without a raise: `spec bump`'s row, serialized in this
/// key order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BumpRow {
    pub need: Need,
    pub id: String,
    pub path: String,
    pub line: usize,
    /// `HEAD`'s revision; `None` when unreadable.
    pub rev: Option<u32>,
    /// The revision a raise writes; `None` when no raise is advised: `rev`
    /// unreadable or the largest, a document of a closed class that admits
    /// no `rev`, a line that cannot be written.
    pub to: Option<u32>,
    /// `HEAD`'s lock entries vouching for its old text, its `[[tested]]`
    /// block counted as one.
    pub lock_entries: usize,
    /// The raised line, without its ending; `None` exactly when `to` is.
    pub write: Option<String>,
}

/// The rule's findings and `spec bump`'s rows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Judged {
    /// One per `required` row.
    pub findings: Vec<Finding>,
    /// By path, line and ID; empty unless asked for.
    pub rows: Vec<BumpRow>,
}

/// The rule over `changed` ([`changed_nodes`]): `head_lock` is `HEAD`'s
/// `spec.lock`, `checked_lock` the checked tree's (`None`: absent);
/// `scheme` and `classes` the checked tree's, which judge a raise as an
/// apply does; `rows` asks for `spec bump`'s rows, else only the findings
/// are built.
pub fn judge(
    changed: &[Changed<'_>],
    head_lock: Option<&Lock>,
    checked_lock: Option<&Lock>,
    scheme: &IdScheme,
    classes: &Classes,
    rows: bool,
) -> Judged {
    let mut judged = Judged::default();
    let mut texts: BTreeMap<&str, FileText<'_>> = BTreeMap::new();
    for node in changed.iter().filter(|node| !node.raised()) {
        let head_rev = node.head_rev.number();
        let (vouching, block): (Vec<_>, bool) =
            match (head_lock, head_rev, node.head_hash.as_deref()) {
                (Some(lock), Some(rev), Some(old)) => (
                    lock.entries_of(&node.id)
                        .filter(|(key, verified)| vouches(key, verified, &node.id, rev, old))
                        .map(|(key, _)| key)
                        .collect(),
                    // A `[[tested]]` block vouches as one entry does.
                    lock.tested
                        .get(&node.id)
                        .is_some_and(|block| block.vouches(rev, old)),
                ),
                _ => (Vec::new(), false),
            };
        let refreshed = vouching.iter().all(|key| {
            checked_lock
                .and_then(|lock| lock.entries.get(*key))
                .is_some_and(|now| Some(now.rev) == head_rev && now.spec_hash == node.hash)
        }) && (!block
            || checked_lock
                .and_then(|lock| lock.tested.get(&node.id))
                .is_some_and(|now| Some(now.rev) == head_rev && now.spec_hash == node.hash));
        let vouchers = vouching.len() + usize::from(block);
        let need = if vouchers == 0 || refreshed {
            Need::Suggested
        } else {
            Need::Required
        };
        if need == Need::Suggested && !rows {
            continue;
        }
        let file = node.at.file;
        let text = texts
            .entry(file.path.as_str())
            .or_insert_with(|| FileText::new(&file.bytes));
        let line = node_line(text, &node.at);
        let (to, fix) = node.raise(scheme, classes).unzip();
        if let (Need::Required, Some(rev)) = (need, head_rev) {
            judged.findings.push(Finding {
                code: REV_NOT_RAISED.to_owned(),
                severity: Severity::Error,
                path: node.path.clone(),
                line,
                subject: node.id.clone(),
                message: message(node, rev, to),
                fix: fix.clone(),
                debt: None,
                introduced: None,
            });
        }
        if rows {
            judged.rows.push(BumpRow {
                need,
                id: node.id.clone(),
                path: node.path.clone(),
                line,
                rev: head_rev,
                to,
                lock_entries: vouchers,
                write: fix.map(|fix| fix.text.trim_end_matches(['\n', '\r']).to_owned()),
            });
        }
    }
    judged.rows.sort_by(|a, b| {
        (a.path.as_bytes(), a.line, a.id.as_bytes()).cmp(&(
            b.path.as_bytes(),
            b.line,
            b.id.as_bytes(),
        ))
    });
    judged
}

/// The finding's message: its node, `HEAD`'s revision, its ways out: the
/// raise to `to` and `--editorial`, or, with no raise advised, only
/// `--editorial` (at [`REV_MAX`] naming it the largest revision).
fn message(node: &Changed<'_>, rev: u32, to: Option<u32>) -> String {
    let id = &node.id;
    match to {
        Some(to) => {
            let written = if node.ord == 0 {
                format!("rev: {to}")
            } else {
                format!("rev={to}")
            };
            format!(
                "`{id}` changed at rev {rev}, which {LOCK_FILE} vouches for: raise it ({written}), \
                 or have the owner approve it through the queue with --editorial"
            )
        }
        None => {
            let largest = if rev >= REV_MAX {
                ", the largest revision"
            } else {
                ""
            };
            format!(
                "`{id}` changed at rev {rev}{largest}, which {LOCK_FILE} vouches for: have the \
                 owner approve it through the queue with --editorial"
            )
        }
    }
}
