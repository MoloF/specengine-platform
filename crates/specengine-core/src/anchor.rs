//! The pairing of a renamed or moved binding with its old lock entry (canon
//! `spec-verify-anchors`, "The pairing"): pure, no I/O, no node, relation
//! or code type. An entry whose key no marker gives any more and a marker
//! binding of its group with no entry each come as a [`Print`]: an opaque
//! group key, a file and three fingerprint strings compared only for
//! equality (`body`, `skeleton`, `quote`), and `body_bytes`, its size. The
//! caller names the triviality threshold; a body under it on either side
//! never takes part. One [`Follow`] per entry, by a mutual unique match:
//!
//! 1. C0(e): every binding of e's group with an equal `body`; none: every
//!    one in e's file with an equal `skeleton`.
//! 2. C(e): C0(e); when it holds several, those with an equal `quote`
//!    instead, only when exactly one is left.
//! 3. R(b): the entries whose C holds b; when one of them matched b by
//!    `body`, only the `body` matches stay; then step 2's quote rule.
//! 4. MOVED iff C(e) = {b} and R(b) = {e}; else AMBIGUOUS when C(e) is not
//!    empty (its candidates), else LOST.
//!
//! Over sets: neither input order nor anything but the prints goes in.

use std::collections::BTreeMap;

/// One side's fingerprints of an entry or a binding: `group` the opaque
/// key both sides must share, `file` where it lies (the `skeleton` step
/// stays in the entry's file), `body`, `skeleton` and `quote` compared for
/// equality only, `body_bytes` its body's size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Print<'a, G> {
    pub group: G,
    pub file: &'a str,
    pub body: &'a str,
    pub skeleton: &'a str,
    pub quote: &'a str,
    pub body_bytes: u32,
}

/// What became of an entry: followed to one binding, contested or matched
/// by several (its candidates), or lost. Indexes into the bindings given,
/// ascending.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Follow {
    Moved(usize),
    Ambiguous(Vec<usize>),
    Lost,
}

/// One [`Follow`] per entry of `entries`, in their order, against
/// `bindings`; a print whose `body_bytes` is under `trivial` is never
/// followed (an entry so: `Lost`) and never a candidate.
pub fn pair<G: Ord>(
    entries: &[Print<'_, G>],
    bindings: &[Print<'_, G>],
    trivial: u32,
) -> Vec<Follow> {
    let mut by_group: BTreeMap<&G, Vec<usize>> = BTreeMap::new();
    for (index, binding) in bindings.iter().enumerate() {
        if binding.body_bytes >= trivial {
            by_group.entry(&binding.group).or_default().push(index);
        }
    }

    // C(e), per entry; empty for a trivial one.
    let candidates: Vec<Vec<usize>> = entries
        .iter()
        .map(|entry| {
            if entry.body_bytes < trivial {
                return Vec::new();
            }
            let Some(group) = by_group.get(&entry.group) else {
                return Vec::new();
            };
            let by_body: Vec<usize> = group
                .iter()
                .copied()
                .filter(|&index| bindings[index].body == entry.body)
                .collect();
            let first = if by_body.is_empty() {
                group
                    .iter()
                    .copied()
                    .filter(|&index| {
                        let binding = &bindings[index];
                        binding.file == entry.file && binding.skeleton == entry.skeleton
                    })
                    .collect()
            } else {
                by_body
            };
            by_quote(first, entry.quote, |index| bindings[index].quote)
        })
        .collect();

    // R(b), per binding named by some C(e).
    let mut reached: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (entry, found) in candidates.iter().enumerate() {
        for &binding in found {
            reached.entry(binding).or_default().push(entry);
        }
    }
    let reverse: BTreeMap<usize, Vec<usize>> = reached
        .into_iter()
        .map(|(binding, from)| {
            let body = bindings[binding].body;
            let by_body: Vec<usize> = from
                .iter()
                .copied()
                .filter(|&entry| entries[entry].body == body)
                .collect();
            let first = if by_body.is_empty() { from } else { by_body };
            let quote = bindings[binding].quote;
            (
                binding,
                by_quote(first, quote, |entry| entries[entry].quote),
            )
        })
        .collect();

    candidates
        .into_iter()
        .enumerate()
        .map(|(entry, found)| match found.as_slice() {
            [] => Follow::Lost,
            [binding] if reverse.get(binding).map(Vec::as_slice) == Some(&[entry][..]) => {
                Follow::Moved(*binding)
            }
            _ => Follow::Ambiguous(found),
        })
        .collect()
}

/// Step 2's quote rule: several left, those whose quote equals `quote`
/// instead, only when exactly one is; ascending.
fn by_quote<'a>(
    mut found: Vec<usize>,
    quote: &str,
    quote_of: impl Fn(usize) -> &'a str,
) -> Vec<usize> {
    found.sort_unstable();
    if found.len() > 1 {
        let equal: Vec<usize> = found
            .iter()
            .copied()
            .filter(|&index| quote_of(index) == quote)
            .collect();
        if equal.len() == 1 {
            return equal;
        }
    }
    found
}
