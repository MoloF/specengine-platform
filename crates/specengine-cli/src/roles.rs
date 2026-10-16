//! Framework roles as the reads see them (task spec `bevy-roles`, "Links",
//! "spec index"; L1, G1, J1): each stored role row's state, computed per
//! read from the store's candidates ([`state`]), and the counts
//! `spec index` prints ([`role_counts`]).
//!
//! - **Link** (L1): a row with a subject takes its candidates at the best
//!   tier; of those, the ones in its own file's package if any, else all:
//!   its own package breaks a tie within the tier, never beats a better
//!   one. One: [`RoleState::Linked`]; several: [`RoleState::Ambiguous`] on
//!   each, never a picked one; none: [`RoleState::Unlinked`], counted only.
//!   A row without a subject is [`RoleState::Certain`] or
//!   [`RoleState::Uncertain`] by its certainty, on its `symbol_ord` or on
//!   nothing.
//! - **Counts** (G1): per word of the listed frameworks (the code crate's
//!   data, byte order, zeros too), the symbols holding a row of the word
//!   certain, linked or ambiguous; the rows with a subject by state; the
//!   uncertain rows. Over the stored rows, a gap of the walk too.
//!
//! Which words exist, which rows carry a subject and which item kinds a
//! word's subject may name are the code crate's
//! (`specengine_code::frameworks`); this module names none of them.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use specengine_code::frameworks;
use specengine_store::{RoleCandidate, SqliteIndex, StoredRole};

use crate::{CliError, store_error};

/// A role row's state at one read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RoleState<'r> {
    /// Its subject names this one symbol.
    Linked(&'r RoleCandidate),
    /// Its subject names each of these symbols at the same best tier.
    Ambiguous(Vec<&'r RoleCandidate>),
    /// Its subject names no symbol.
    Unlinked,
    /// No subject, read exactly: it sits on its `symbol_ord`, if any.
    Certain,
    /// No subject, not read exactly: its category says why.
    Uncertain,
}

/// The state of `role` from its stored candidates (L1).
pub(crate) fn state(role: &StoredRole) -> RoleState<'_> {
    if role.subject.is_none() {
        return if role.certainty == frameworks::UNCERTAIN {
            RoleState::Uncertain
        } else {
            RoleState::Certain
        };
    }
    let Some(best) = role.candidates.iter().map(|candidate| candidate.tier).min() else {
        return RoleState::Unlinked;
    };
    let at_best: Vec<&RoleCandidate> = role
        .candidates
        .iter()
        .filter(|candidate| candidate.tier == best)
        .collect();
    let own: Vec<&RoleCandidate> = at_best
        .iter()
        .copied()
        .filter(|candidate| candidate.package == role.package)
        .collect();
    let mut pool = if own.is_empty() { at_best } else { own };
    if pool.len() == 1 {
        RoleState::Linked(pool.remove(0))
    } else {
        RoleState::Ambiguous(pool)
    }
}

/// The rows with a subject, by state.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct SubjectCounts {
    pub total: usize,
    pub linked: usize,
    pub ambiguous: usize,
    pub unlinked: usize,
}

/// `spec index`'s role counts: JSON `code.roles`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct RoleCounts {
    /// Each word of the listed frameworks: the symbols holding a row of it
    /// certain, linked or ambiguous.
    pub words: BTreeMap<String, usize>,
    pub subjects: SubjectCounts,
    /// The rows without a subject that are uncertain.
    pub uncertain: usize,
}

/// The role counts of the worktree's stored rows under the listed
/// `frameworks` (links computed now, one read transaction); all zero
/// when no code is indexed.
pub fn role_counts(index: &SqliteIndex, frameworks: &[String]) -> Result<RoleCounts, CliError> {
    let kinds = frameworks::item_kinds(frameworks);
    let roles = index
        .code_roles(&kinds)
        .map_err(store_error)?
        .unwrap_or_default();
    Ok(count(frameworks, &roles))
}

/// [`role_counts`] over rows already read.
pub(crate) fn count(frameworks: &[String], roles: &[StoredRole]) -> RoleCounts {
    // Each word's holders, as (path, ord) of a symbol.
    let mut holders: BTreeMap<&str, BTreeSet<(&str, u32)>> = frameworks::words(frameworks)
        .into_iter()
        .map(|word| (word, BTreeSet::new()))
        .collect();
    let mut counts = RoleCounts::default();
    for role in roles {
        let mut held: Vec<(&str, u32)> = Vec::new();
        match state(role) {
            RoleState::Certain => {
                held.extend(role.symbol_ord.map(|ord| (role.path.as_str(), ord)));
            }
            RoleState::Uncertain => counts.uncertain += 1,
            RoleState::Linked(candidate) => {
                counts.subjects.linked += 1;
                held.push((candidate.symbol.path.as_str(), candidate.symbol.ord));
            }
            RoleState::Ambiguous(candidates) => {
                counts.subjects.ambiguous += 1;
                held.extend(
                    candidates
                        .iter()
                        .map(|candidate| (candidate.symbol.path.as_str(), candidate.symbol.ord)),
                );
            }
            RoleState::Unlinked => counts.subjects.unlinked += 1,
        }
        if role.subject.is_some() {
            counts.subjects.total += 1;
        }
        if let Some(symbols) = holders.get_mut(role.word.as_str()) {
            symbols.extend(held);
        }
    }
    counts.words = holders
        .into_iter()
        .map(|(word, symbols)| (word.to_owned(), symbols.len()))
        .collect();
    counts
}

/// `spec index`'s line: `roles <slug>: <word> <n>, …; subjects <n> (linked
/// <n>, ambiguous <n>, unlinked <n>), uncertain <n>`.
pub(crate) fn render_line(slug: &str, counts: &RoleCounts) -> String {
    let words: Vec<String> = counts
        .words
        .iter()
        .map(|(word, count)| format!("{word} {count}"))
        .collect();
    let subjects = &counts.subjects;
    format!(
        "roles {slug}: {}; subjects {} (linked {}, ambiguous {}, unlinked {}), uncertain {}",
        words.join(", "),
        subjects.total,
        subjects.linked,
        subjects.ambiguous,
        subjects.unlinked,
        counts.uncertain
    )
}
