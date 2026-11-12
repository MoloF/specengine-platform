//! The collision pass, the pure half (task spec `collision-pass`, CP1 to
//! CP5): the index's universe in, the pairs of nodes that may collide out,
//! each scored, ordered and filtered by the verdicts an agent left. Nothing
//! is read, written or stored here, no clock is read and no pair is kept: a
//! call computes its pairs from one state.
//!
//! - **Groups.** Pairs are drawn within a kind, kinds compared byte for
//!   byte as opaque strings (ADR-0008); `any_kind` makes one group of the
//!   whole universe. Every cap counts within the group.
//! - **Signal (a).** The link targets two nodes both hold; a target held by
//!   more than [`COLLISION_TARGET_DEGREE_MAX`] nodes of the group is
//!   skipped.
//! - **Signal (b).** Title trigrams: the title as the queue's dedup
//!   normalizes it ([`normalized_summary`]), padded with one space on each
//!   side, every window of three Unicode scalar values, as a set; `titles`
//!   is Dice over the whole sets, per mille. Candidates come from the
//!   trigrams at most [`COLLISION_TRIGRAM_DF_MAX`] titles of the group hold
//!   (a commoner one gives none, yet counts in Dice) and from word buckets
//!   (a title's words sorted, each once) holding at most as many titles; a
//!   candidate whose sizes cannot reach [`COLLISION_TITLES_MIN`] is never
//!   intersected. In a group where no trigram exceeds the cap, the pairs are
//!   those of every shared trigram.
//! - **Never paired**: two holders of one name (a duplicate ID), and a
//!   document with its own sections.
//! - **Order**: score descending, then `a`'s `(path, ord)`, then `b`'s;
//!   `a` is the side first by `(path, ord)`. Only the best `limit` pairs are
//!   kept, in a bounded heap; the rest are counted. No hash or map order
//!   leaks.
//! - **The verdicts.** A pair is hidden while a verdict for its two names
//!   holds both current norms; one whose norms no longer hold lists the pair
//!   again, `marked`. A verdict blocks, gates or changes nothing (ADR-0012).
//!
//! The marks' own caps ([`judgement_problem`]) and the names they must
//! name ([`judged_notes`]) live here too, so the CLI and MCP judge alike.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap, HashMap};

use serde::{Deserialize, Serialize};

use crate::drift::is_drift_hash;
use crate::intake::{FieldProblem, normalized_summary};
use crate::proposal::proposal_number;

/// The pairs `spec collisions` lists without `--limit`.
pub const COLLISIONS_LIMIT_DEFAULT: usize = 50;
/// The most pairs one call lists (`--limit`'s top).
pub const COLLISIONS_LIMIT_MAX: usize = 200;
/// A link target held by more nodes of the group than this is skipped.
pub const COLLISION_TARGET_DEGREE_MAX: usize = 64;
/// A trigram held by more titles of the group than this, or a word bucket
/// holding more, gives no candidate; the trigram still counts in Dice.
pub const COLLISION_TRIGRAM_DF_MAX: usize = 200;
/// The fewest per mille of title likeness that qualifies a pair alone.
pub const COLLISION_TITLES_MIN: u32 = 500;
/// What one shared target adds to a pair's score.
pub const COLLISION_SHARED_WEIGHT: u32 = 200;
/// The most shared targets a score counts.
pub const COLLISION_SHARED_SCORED_MAX: usize = 5;
/// The most shared targets a pair names.
pub const COLLISION_SHARED_LISTED_MAX: usize = 8;
/// The most pairs one mark takes.
pub const COLLISION_MARKS_MAX: usize = 200;
/// The most bytes of a name a mark takes.
pub const COLLISION_NAME_MAX: usize = 256;

/// One node of the universe: a live document or ID section with a kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollisionNode {
    pub path: String,
    /// 0 the document, then its sections in source order.
    pub ord: usize,
    /// Its canonical ID; `None` for an ID-less document.
    pub id: Option<String>,
    pub kind: String,
    pub title: Option<String>,
    /// `None`: its file does not parse whole; never paired.
    pub norm_hash: Option<String>,
}

impl CollisionNode {
    /// What a pair and a mark name it by: its ID, else its path.
    pub fn name(&self) -> &str {
        self.id.as_deref().unwrap_or(&self.path)
    }

    fn key(&self) -> (&str, usize) {
        (self.path.as_str(), self.ord)
    }
}

/// One resolved link target a node holds: an edge's end (any type but
/// `mentions`, the holder its source) or a document's `parent:`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollisionTarget {
    pub holder_path: String,
    pub holder_ord: usize,
    pub end_path: String,
    pub end_ord: usize,
    /// The end's name: its ID, else its path.
    pub end_name: String,
}

/// The index's universe in one snapshot, by `(path, ord)`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CollisionRows {
    /// Every node of the universe, a `None` norm too.
    pub nodes: Vec<CollisionNode>,
    pub targets: Vec<CollisionTarget>,
    /// The nodes whose `norm_hash` is `None`.
    pub skipped: usize,
}

/// A verdict on a pair: judged apart, or reported to the owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CollisionVerdict {
    /// The two nodes do not collide.
    Distinct,
    /// A collision, filed as the proposal the verdict names.
    Reported,
}

impl CollisionVerdict {
    pub const ALL: [Self; 2] = [Self::Distinct, Self::Reported];

    /// `distinct` or `reported`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Distinct => "distinct",
            Self::Reported => "reported",
        }
    }

    /// The verdict spelled `text`, exactly.
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|verdict| verdict.as_str() == text)
    }
}

/// One stored verdict as the pass reads it: `a` before `b` byte-wise, each
/// norm its name's when it was judged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JudgedPair {
    pub a: String,
    pub a_norm: String,
    pub b: String,
    pub b_norm: String,
    pub verdict: CollisionVerdict,
    pub proposal: Option<String>,
    /// When it was judged, `YYYY-MM-DDTHH:MM:SSZ`.
    pub at: String,
}

/// One side of a listed pair.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PairSide {
    pub id: Option<String>,
    pub path: String,
    pub ord: usize,
    pub kind: String,
    pub title: Option<String>,
    pub norm_hash: String,
}

impl PairSide {
    /// Its name: its ID, else its path.
    pub fn name(&self) -> &str {
        self.id.as_deref().unwrap_or(&self.path)
    }
}

/// The verdict a pair had before either side changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PairMarked {
    pub verdict: CollisionVerdict,
    pub proposal: Option<String>,
    pub at: String,
}

/// One pair that may collide.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CollisionPair {
    /// The side first by `(path, ord)`.
    pub a: PairSide,
    pub b: PairSide,
    /// `titles + 200 × min(shared, 5)`: 200 to 2 000.
    pub score: u32,
    /// Dice of the whole title trigram sets, per mille.
    pub titles: u32,
    /// The targets both hold.
    pub shared: usize,
    /// The first [`COLLISION_SHARED_LISTED_MAX`] shared targets' names, by
    /// `(path, ord)`.
    pub shared_targets: Vec<String>,
    /// A verdict whose norms no longer hold.
    pub marked: Option<PairMarked>,
    /// Its parts joined by `; `.
    pub reason: String,
}

/// What one call asks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CollisionQuery<'a> {
    /// Keep the nodes of these kinds; empty: every kind.
    pub kinds: &'a [String],
    /// One group across the kinds kept.
    pub any_kind: bool,
    /// The most pairs listed.
    pub limit: usize,
}

/// The pairs listed, how many qualifying pairs lie past them, and how many
/// nodes of the kinds kept have no `norm_hash`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CollisionList {
    pub pairs: Vec<CollisionPair>,
    pub left: usize,
    pub skipped: usize,
}

/// The caps one pass runs under: [`CollisionCaps::ENGINE`], core's; a unit
/// test may lower one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CollisionCaps {
    pub target_degree_max: usize,
    pub trigram_df_max: usize,
    pub titles_min: u32,
}

impl CollisionCaps {
    pub(crate) const ENGINE: Self = Self {
        target_degree_max: COLLISION_TARGET_DEGREE_MAX,
        trigram_df_max: COLLISION_TRIGRAM_DF_MAX,
        titles_min: COLLISION_TITLES_MIN,
    };
}

/// The pairs of `rows` that may collide, as `query` asks, the verdicts of
/// `judged` applied: at most `query.limit`, in order, and how many
/// qualifying pairs are left past them.
pub fn collision_pairs(
    rows: &CollisionRows,
    judged: &[JudgedPair],
    query: &CollisionQuery<'_>,
) -> CollisionList {
    collision_pairs_capped(rows, judged, query, &CollisionCaps::ENGINE)
}

/// A qualifying pair before its texts: global node indices, `x` first.
struct Candidate {
    x: usize,
    y: usize,
    score: u32,
    titles: u32,
    /// Indices into `names` of the shared targets, by `(path, ord)`.
    shared: Vec<usize>,
    marked: Option<PairMarked>,
}

/// Better first: score descending, then `x`, then `y` (a pair's two global
/// indices, unique), so the heap's top is the worst pair kept.
impl Ord for Candidate {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .score
            .cmp(&self.score)
            .then_with(|| self.x.cmp(&other.x))
            .then_with(|| self.y.cmp(&other.y))
    }
}

impl PartialOrd for Candidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Candidate {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Candidate {}

/// [`collision_pairs`] under `caps`.
pub(crate) fn collision_pairs_capped(
    rows: &CollisionRows,
    judged: &[JudgedPair],
    query: &CollisionQuery<'_>,
    caps: &CollisionCaps,
) -> CollisionList {
    // The paired nodes, by `(path, ord)`: a kept kind, and a norm.
    let kept = |node: &&CollisionNode| query.kinds.is_empty() || query.kinds.contains(&node.kind);
    let skipped = rows
        .nodes
        .iter()
        .filter(kept)
        .filter(|node| node.norm_hash.is_none())
        .count();
    let mut nodes: Vec<&CollisionNode> = rows
        .nodes
        .iter()
        .filter(kept)
        .filter(|node| node.norm_hash.is_some())
        .collect();
    nodes.sort_by(|a, b| a.key().cmp(&b.key()));
    nodes.dedup_by(|a, b| a.key() == b.key());

    // The groups, each its members' global indices in order.
    let mut groups: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (index, node) in nodes.iter().enumerate() {
        let group = if query.any_kind { "" } else { &node.kind };
        groups.entry(group).or_default().push(index);
    }
    let mut place: Vec<(usize, usize)> = vec![(0, 0); nodes.len()];
    for (group, members) in groups.values().enumerate() {
        for (local, &index) in members.iter().enumerate() {
            place[index] = (group, local);
        }
    }
    let index_of: HashMap<(&str, usize), usize> = nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.key(), index))
        .collect();

    // Every target's name once, by `(path, ord)`, and each group's holders
    // of it.
    let mut ends: BTreeMap<(&str, usize), &str> = BTreeMap::new();
    for target in &rows.targets {
        ends.entry((target.end_path.as_str(), target.end_ord))
            .or_insert(target.end_name.as_str());
    }
    let end_rank: HashMap<(&str, usize), usize> = ends
        .keys()
        .enumerate()
        .map(|(rank, key)| (*key, rank))
        .collect();
    let names: Vec<&str> = ends.values().copied().collect();
    let mut held: Vec<BTreeMap<usize, BTreeSet<usize>>> = vec![BTreeMap::new(); groups.len()];
    for target in &rows.targets {
        let Some(&holder) = index_of.get(&(target.holder_path.as_str(), target.holder_ord)) else {
            continue;
        };
        let Some(&rank) = end_rank.get(&(target.end_path.as_str(), target.end_ord)) else {
            continue;
        };
        let (group, local) = place[holder];
        held[group].entry(rank).or_default().insert(local);
    }

    let ledger: HashMap<(&str, &str), &JudgedPair> = judged
        .iter()
        .map(|pair| ((pair.a.as_str(), pair.b.as_str()), pair))
        .collect();

    // The best `limit` qualifying pairs, the worst on top; the rest counted.
    let mut best: BinaryHeap<Candidate> = BinaryHeap::with_capacity(query.limit.saturating_add(1));
    let mut qualifying = 0_usize;
    for (group, members) in groups.values().enumerate() {
        // Signal (a): local pair → the shared targets' ranks, ascending.
        let mut shared: BTreeMap<(usize, usize), Vec<usize>> = BTreeMap::new();
        for (&rank, holders) in &held[group] {
            if holders.len() < 2 || holders.len() > caps.target_degree_max {
                continue;
            }
            let holders: Vec<usize> = holders.iter().copied().collect();
            for (at, &x) in holders.iter().enumerate() {
                for &y in &holders[at + 1..] {
                    shared.entry((x, y)).or_default().push(rank);
                }
            }
        }
        let titles = TitleSets::of(members.iter().map(|&index| nodes[index]), caps);
        // `stamp[y] == x + 1`: `y` is already a partner of `x`.
        let mut stamp = vec![0_usize; members.len()];
        let mut alike: Vec<usize> = Vec::new();
        let mut with_targets: Vec<(usize, Vec<usize>)> = Vec::new();
        for x in 0..members.len() {
            // The pairs of `x` sharing a target, each list moved out: every
            // key below `(x, 0)` is drained already.
            while let Some(entry) = shared.first_entry() {
                if entry.key().0 != x {
                    break;
                }
                let ((_, y), targets) = entry.remove_entry();
                stamp[y] = x + 1;
                with_targets.push((y, targets));
            }
            titles.partners_after(x, &mut stamp, &mut alike);
            let pairs = with_targets
                .drain(..)
                .map(|(y, targets)| (y, titles.dice(x, y), targets))
                .chain(alike.drain(..).filter_map(|y| {
                    // A pair no Dice of these sizes can lift to the floor is
                    // never intersected.
                    if titles.bound(x, y) < caps.titles_min {
                        return None;
                    }
                    let liked = titles.dice(x, y);
                    (liked >= caps.titles_min).then_some((y, liked, Vec::new()))
                }));
            for (y, liked, targets) in pairs {
                if liked < caps.titles_min && targets.is_empty() {
                    continue;
                }
                let (gx, gy) = (members[x], members[y]);
                let (a, b) = (nodes[gx], nodes[gy]);
                if a.name() == b.name() || (a.path == b.path && (a.ord == 0 || b.ord == 0)) {
                    continue;
                }
                let marked = match verdict_on(&ledger, a, b) {
                    Verdict::Holds => continue,
                    Verdict::Changed(marked) => Some(marked),
                    Verdict::None => None,
                };
                qualifying += 1;
                if query.limit == 0 {
                    continue;
                }
                let scored = targets.len().min(COLLISION_SHARED_SCORED_MAX);
                best.push(Candidate {
                    x: gx,
                    y: gy,
                    score: liked + COLLISION_SHARED_WEIGHT * u32::try_from(scored).unwrap_or(0),
                    titles: liked,
                    shared: targets,
                    marked,
                });
                if best.len() > query.limit {
                    best.pop();
                }
            }
        }
    }

    // Best first: score descending, then `(path, ord)`, which global
    // indices follow.
    let left = qualifying - best.len();
    let pairs = best
        .into_sorted_vec()
        .into_iter()
        .map(|candidate| {
            let shared_targets: Vec<String> = candidate
                .shared
                .iter()
                .take(COLLISION_SHARED_LISTED_MAX)
                .map(|&rank| names[rank].to_owned())
                .collect();
            let reason = pair_reason(
                candidate.titles,
                candidate.shared.len(),
                &shared_targets,
                candidate.marked.as_ref(),
            );
            CollisionPair {
                a: side(nodes[candidate.x]),
                b: side(nodes[candidate.y]),
                score: candidate.score,
                titles: candidate.titles,
                shared: candidate.shared.len(),
                shared_targets,
                marked: candidate.marked,
                reason,
            }
        })
        .collect();
    CollisionList {
        pairs,
        left,
        skipped,
    }
}

/// Where a pair stands against the verdicts.
enum Verdict {
    /// A verdict holds both current norms: hidden.
    Holds,
    /// A verdict for its names whose norms no longer hold.
    Changed(PairMarked),
    None,
}

/// The verdict stored for the names of `x` and `y`, byte-wise ordered.
fn verdict_on(
    ledger: &HashMap<(&str, &str), &JudgedPair>,
    x: &CollisionNode,
    y: &CollisionNode,
) -> Verdict {
    let (low, high) = if x.name() <= y.name() { (x, y) } else { (y, x) };
    let Some(row) = ledger.get(&(low.name(), high.name())) else {
        return Verdict::None;
    };
    if low.norm_hash.as_deref() == Some(row.a_norm.as_str())
        && high.norm_hash.as_deref() == Some(row.b_norm.as_str())
    {
        return Verdict::Holds;
    }
    Verdict::Changed(PairMarked {
        verdict: row.verdict,
        proposal: row.proposal.clone(),
        at: row.at.clone(),
    })
}

fn side(node: &CollisionNode) -> PairSide {
    PairSide {
        id: node.id.clone(),
        path: node.path.clone(),
        ord: node.ord,
        kind: node.kind.clone(),
        title: node.title.clone(),
        norm_hash: node.norm_hash.clone().unwrap_or_default(),
    }
}

/// `titles alike 0.736`, `` 1 link target(s) shared: `T` ``, `marked
/// distinct <at>, changed since`, the parts that apply joined by `; `.
fn pair_reason(
    titles: u32,
    shared: usize,
    listed: &[String],
    marked: Option<&PairMarked>,
) -> String {
    let mut parts = Vec::with_capacity(3);
    if titles >= COLLISION_TITLES_MIN {
        parts.push(format!(
            "titles alike {}.{:03}",
            titles / 1000,
            titles % 1000
        ));
    }
    if shared > 0 {
        let mut named = listed
            .iter()
            .map(|name| format!("`{name}`"))
            .collect::<Vec<_>>()
            .join(", ");
        if shared > listed.len() {
            named.push_str(&format!(", and {} more", shared - listed.len()));
        }
        parts.push(format!("{shared} link target(s) shared: {named}"));
    }
    if let Some(marked) = marked {
        parts.push(format!(
            "marked {} {}, changed since",
            marked.verdict.as_str(),
            marked.at
        ));
    }
    parts.join("; ")
}

/// A group's title trigram sets, whole, and where its signal (b) takes
/// candidates from: the postings of the trigrams at most
/// [`CollisionCaps::trigram_df_max`] titles hold, and the word buckets
/// holding at most as many.
struct TitleSets {
    /// Each member's trigrams, every one, as IDs ascending: what Dice reads.
    full: Vec<Vec<usize>>,
    /// Each member's trigrams under the cap, as indices into `postings`.
    rare: Vec<Vec<usize>>,
    /// Each trigram under the cap: its members, ascending.
    postings: Vec<Vec<usize>>,
    /// Each member's word bucket under the cap, an index into `buckets`.
    bucket: Vec<Option<usize>>,
    /// Each bucket under the cap: its members, ascending.
    buckets: Vec<Vec<usize>>,
}

impl TitleSets {
    fn of<'n>(members: impl Iterator<Item = &'n CollisionNode>, caps: &CollisionCaps) -> Self {
        let mut ids: HashMap<[char; 3], usize> = HashMap::new();
        let mut held: Vec<Vec<usize>> = Vec::new();
        let mut keys: HashMap<String, usize> = HashMap::new();
        let mut keyed: Vec<Vec<usize>> = Vec::new();
        let mut full: Vec<Vec<usize>> = Vec::new();
        let mut key_of: Vec<Option<usize>> = Vec::new();
        for (member, node) in members.enumerate() {
            let mut set: Vec<usize> = Vec::new();
            let mut key_id = None;
            if let Some(title) = node.title.as_deref() {
                let normalized = normalized_summary(title);
                let padded: Vec<char> = format!(" {normalized} ").chars().collect();
                for window in padded.windows(3) {
                    let gram = [window[0], window[1], window[2]];
                    let next = ids.len();
                    set.push(*ids.entry(gram).or_insert(next));
                }
                let key = words_key(&normalized);
                if !key.is_empty() {
                    let next = keys.len();
                    let id = *keys.entry(key).or_insert(next);
                    if id == keyed.len() {
                        keyed.push(Vec::new());
                    }
                    keyed[id].push(member);
                    key_id = Some(id);
                }
            }
            set.sort_unstable();
            set.dedup();
            for &id in &set {
                if id == held.len() {
                    held.push(Vec::new());
                }
                held[id].push(member);
            }
            full.push(set);
            key_of.push(key_id);
        }
        // Only the trigrams and buckets under the cap give candidates.
        let mut posted: Vec<Option<usize>> = vec![None; held.len()];
        let mut postings: Vec<Vec<usize>> = Vec::new();
        for (id, holders) in held.into_iter().enumerate() {
            if holders.len() <= caps.trigram_df_max {
                posted[id] = Some(postings.len());
                postings.push(holders);
            }
        }
        let rare = full
            .iter()
            .map(|set| set.iter().filter_map(|&id| posted[id]).collect())
            .collect();
        let mut kept: Vec<Option<usize>> = vec![None; keyed.len()];
        let mut buckets: Vec<Vec<usize>> = Vec::new();
        for (id, holders) in keyed.into_iter().enumerate() {
            if holders.len() >= 2 && holders.len() <= caps.trigram_df_max {
                kept[id] = Some(buckets.len());
                buckets.push(holders);
            }
        }
        let bucket = key_of
            .into_iter()
            .map(|key| key.and_then(|id| kept[id]))
            .collect();
        Self {
            full,
            rare,
            postings,
            bucket,
            buckets,
        }
    }

    /// Pushes onto `out` each later member that shares a trigram under the
    /// cap or the word bucket with `x`, once, and not one `stamp` already
    /// gives `x` (`x + 1`).
    fn partners_after(&self, x: usize, stamp: &mut [usize], out: &mut Vec<usize>) {
        let tag = x + 1;
        let lists = self.rare[x]
            .iter()
            .map(|&gram| &self.postings[gram])
            .chain(self.bucket[x].map(|bucket| &self.buckets[bucket]));
        for holders in lists {
            let start = holders.partition_point(|&member| member <= x);
            for &y in &holders[start..] {
                if stamp[y] != tag {
                    stamp[y] = tag;
                    out.push(y);
                }
            }
        }
    }

    /// The most Dice the sizes of `x`'s and `y`'s sets allow, per mille:
    /// `floor(2000 × min / (|x| + |y|))`, 0 with both sets empty.
    fn bound(&self, x: usize, y: usize) -> u32 {
        let (a, b) = (self.full[x].len(), self.full[y].len());
        per_mille(a.min(b), a + b)
    }

    /// `floor(2000 × |x ∩ y| / (|x| + |y|))` over the whole sets, 0 with
    /// both sets empty.
    fn dice(&self, x: usize, y: usize) -> u32 {
        let (a, b) = (&self.full[x], &self.full[y]);
        let (mut i, mut j, mut common) = (0, 0, 0);
        while i < a.len() && j < b.len() {
            match a[i].cmp(&b[j]) {
                Ordering::Less => i += 1,
                Ordering::Greater => j += 1,
                Ordering::Equal => {
                    common += 1;
                    i += 1;
                    j += 1;
                }
            }
        }
        per_mille(common, a.len() + b.len())
    }
}

/// `floor(2000 × common / total)`, 0 for a total of 0.
fn per_mille(common: usize, total: usize) -> u32 {
    if total == 0 {
        return 0;
    }
    let (common, total) = (
        u64::try_from(common).unwrap_or(u64::MAX),
        u64::try_from(total).unwrap_or(u64::MAX),
    );
    u32::try_from(2000 * common / total).unwrap_or(u32::MAX)
}

/// A title's word bucket key: its normalized form's words, split at Unicode
/// whitespace, sorted byte-wise, each once, joined by one space; empty for
/// a title of no word.
fn words_key(normalized: &str) -> String {
    let mut words: Vec<&str> = normalized.split_whitespace().collect();
    words.sort_unstable();
    words.dedup();
    words.join(" ")
}

/// One pair of a mark as it is given: `{a, a_norm, b, b_norm, verdict,
/// proposal}`, no other key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PairJudgement {
    pub a: String,
    pub a_norm: String,
    pub b: String,
    pub b_norm: String,
    pub verdict: CollisionVerdict,
    /// The `PR-` ID a `reported` pair was filed as; none for `distinct`.
    #[serde(default)]
    pub proposal: Option<String>,
}

/// A mark's input document: `{"pairs": [...]}`, no other key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PairJudgements {
    pub pairs: Vec<PairJudgement>,
}

/// The first problem of a mark's pairs, in input order (task spec
/// `collision-pass` CP5): 1 to [`COLLISION_MARKS_MAX`] pairs; then each
/// pair's `a` and `b` (1 to [`COLLISION_NAME_MAX`] bytes, no control
/// character, not one node twice), its norms (`b3:` and 64 lower-case hex
/// digits), its `proposal` (a `PR-` ID for `reported`, none for
/// `distinct`), and no pair given twice in either order. The author is the
/// caller's step after this one.
pub fn judgement_problem(pairs: &[PairJudgement]) -> Option<FieldProblem> {
    if pairs.is_empty() {
        return Some(problem(
            "pairs",
            format!("none: give 1 to {COLLISION_MARKS_MAX} pairs"),
        ));
    }
    if pairs.len() > COLLISION_MARKS_MAX {
        return Some(problem(
            "pairs",
            format!("{} pairs; at most {COLLISION_MARKS_MAX}", pairs.len()),
        ));
    }
    let mut seen: HashMap<(&str, &str), usize> = HashMap::new();
    for (index, pair) in pairs.iter().enumerate() {
        let field = |name: &str| format!("pairs[{index}].{name}");
        for (name, value) in [("a", &pair.a), ("b", &pair.b)] {
            if let Some(found) = name_problem(value) {
                return Some(problem(field(name), found));
            }
        }
        if pair.a == pair.b {
            return Some(problem(format!("pairs[{index}]"), "a pair of one node"));
        }
        for (name, value) in [("a_norm", &pair.a_norm), ("b_norm", &pair.b_norm)] {
            if !is_drift_hash(value) {
                return Some(problem(
                    field(name),
                    "not `b3:` and 64 lower-case hex digits: give the norm_hash as listed",
                ));
            }
        }
        if let Some(found) = proposal_problem(pair.verdict, pair.proposal.as_deref()) {
            return Some(problem(field("proposal"), found));
        }
        let key = if pair.a <= pair.b {
            (pair.a.as_str(), pair.b.as_str())
        } else {
            (pair.b.as_str(), pair.a.as_str())
        };
        if let Some(first) = seen.get(&key) {
            return Some(problem(
                format!("pairs[{index}]"),
                format!("repeats pairs[{first}]"),
            ));
        }
        seen.insert(key, index);
    }
    None
}

/// Why a name cannot stand in a mark: empty, a control character, past
/// [`COLLISION_NAME_MAX`] bytes.
fn name_problem(name: &str) -> Option<String> {
    if name.is_empty() {
        Some("empty: name a node as get_collisions names it".to_owned())
    } else if name.chars().any(char::is_control) {
        Some("holds a control character".to_owned())
    } else if name.len() > COLLISION_NAME_MAX {
        Some(format!(
            "{} bytes; at most {COLLISION_NAME_MAX}",
            name.len()
        ))
    } else {
        None
    }
}

/// Why a verdict's `proposal` does not fit it: `reported` names a `PR-` ID,
/// `distinct` none.
pub fn proposal_problem(verdict: CollisionVerdict, proposal: Option<&str>) -> Option<String> {
    match (verdict, proposal) {
        (CollisionVerdict::Distinct, None) => None,
        (CollisionVerdict::Distinct, Some(_)) => {
            Some("a distinct pair names no proposal: leave it null".to_owned())
        }
        (CollisionVerdict::Reported, None) => {
            Some("a reported pair names the proposal it was filed as".to_owned())
        }
        (CollisionVerdict::Reported, Some(id)) if proposal_number(id).is_some() => None,
        (CollisionVerdict::Reported, Some(id)) => Some(format!(
            "`{id}` is no proposal ID: give the PR- ID report_discrepancy returned"
        )),
    }
}

/// Each pair's names against the universe of `rows`, as `spec collisions`
/// names its nodes (no alias), in input order: the first name no node holds
/// is `pairs[i].a: no live node `X``; a side whose norm no node of that name
/// holds now adds a note, never a refusal.
pub fn judged_notes(
    rows: &CollisionRows,
    pairs: &[PairJudgement],
) -> Result<Vec<String>, FieldProblem> {
    let mut norms: HashMap<&str, Vec<Option<&str>>> = HashMap::new();
    for node in &rows.nodes {
        norms
            .entry(node.name())
            .or_default()
            .push(node.norm_hash.as_deref());
    }
    let mut notes = Vec::new();
    for (index, pair) in pairs.iter().enumerate() {
        for (field, name, norm) in [("a", &pair.a, &pair.a_norm), ("b", &pair.b, &pair.b_norm)] {
            let Some(held) = norms.get(name.as_str()) else {
                return Err(problem(
                    format!("pairs[{index}].{field}"),
                    format!("no live node `{name}`"),
                ));
            };
            if !held.contains(&Some(norm.as_str())) {
                notes.push(format!(
                    "pairs[{index}]: {name} changed since it was listed; the pair will be listed \
                     again"
                ));
            }
        }
    }
    Ok(notes)
}

fn problem(field: impl Into<String>, problem: impl Into<String>) -> FieldProblem {
    FieldProblem {
        field: field.into(),
        problem: problem.into(),
    }
}

#[cfg(test)]
mod tests {
    //! docs/features/collision-pass.md "Acceptance criteria" CP1 to CP5, the
    //! core's units: the caps at their boundaries (a target of 64 holders
    //! pairs, of 65 not; a trigram of 200 titles is kept, of 201 dropped;
    //! `titles` 500 qualifies), the score's five shared targets and the
    //! eight named, the never-paired rules, the verdicts, `left`, the order
    //! and its independence of the input's order, and the marks' caps. Every
    //! name is invented here (neutral words, no project's prefix or kind).

    use super::{
        COLLISION_MARKS_MAX, COLLISION_SHARED_LISTED_MAX, COLLISION_TARGET_DEGREE_MAX,
        COLLISION_TRIGRAM_DF_MAX, CollisionCaps, CollisionList, CollisionNode, CollisionQuery,
        CollisionRows, CollisionTarget, CollisionVerdict, JudgedPair, PairJudgement,
        collision_pairs, collision_pairs_capped, judged_notes, judgement_problem,
    };

    const KIND: &str = "plot";

    fn norm(n: usize) -> String {
        format!("b3:{n:064x}")
    }

    fn node(path: &str, ord: usize, id: Option<&str>, title: Option<&str>) -> CollisionNode {
        CollisionNode {
            path: path.to_owned(),
            ord,
            id: id.map(str::to_owned),
            kind: KIND.to_owned(),
            title: title.map(str::to_owned),
            norm_hash: Some(norm(path.len() * 1000 + ord)),
        }
    }

    fn target(holder: &CollisionNode, end: &str) -> CollisionTarget {
        CollisionTarget {
            holder_path: holder.path.clone(),
            holder_ord: holder.ord,
            end_path: format!("ends/{end}.md"),
            end_ord: 0,
            end_name: end.to_owned(),
        }
    }

    fn pairs_of(rows: &CollisionRows, judged: &[JudgedPair], limit: usize) -> CollisionList {
        collision_pairs(
            rows,
            judged,
            &CollisionQuery {
                kinds: &[],
                any_kind: false,
                limit,
            },
        )
    }

    /// `count` untitled nodes `p000.md` … each holding `end`.
    fn holders(count: usize, end: &str) -> CollisionRows {
        let nodes: Vec<CollisionNode> = (0..count)
            .map(|n| node(&format!("p{n:03}.md"), 0, None, None))
            .collect();
        let targets = nodes.iter().map(|node| target(node, end)).collect();
        CollisionRows {
            nodes,
            targets,
            skipped: 0,
        }
    }

    /// CP4: a target of 64 holders pairs them all (C(64, 2) = 2 016, 200
    /// listed); of 65, none. M: the degree cap removed.
    #[test]
    fn a_target_past_64_holders_is_skipped() {
        assert_eq!(COLLISION_TARGET_DEGREE_MAX, 64);
        let at = pairs_of(&holders(64, "hub"), &[], 200);
        assert_eq!((at.pairs.len(), at.left), (200, 2_016 - 200));
        assert!(
            at.pairs
                .iter()
                .all(|pair| pair.score == 200 && pair.shared == 1)
        );
        let past = pairs_of(&holders(65, "hub"), &[], 200);
        assert_eq!((past.pairs.len(), past.left), (0, 0));
    }

    /// CP2: 200 titles holding one title's every trigram keep them (every
    /// pair `titles` 1 000); 201 drop them all (no pair). M: the df cap
    /// removed or counted across groups.
    #[test]
    fn a_trigram_past_200_titles_is_dropped() {
        assert_eq!(COLLISION_TRIGRAM_DF_MAX, 200);
        let titled = |count: usize| CollisionRows {
            nodes: (0..count)
                .map(|n| node(&format!("t{n:03}.md"), 0, None, Some("Lamp oil")))
                .collect(),
            targets: Vec::new(),
            skipped: 0,
        };
        let at = pairs_of(&titled(200), &[], 1);
        assert_eq!(at.left, 200 * 199 / 2 - 1);
        assert_eq!(at.pairs[0].titles, 1000);
        assert_eq!(at.pairs[0].reason, "titles alike 1.000");
        let past = pairs_of(&titled(201), &[], 1);
        assert_eq!((past.pairs.len(), past.left), (0, 0));
        // Another kind's 201 titles never count against this group's.
        let mut mixed = titled(200);
        mixed.nodes.extend((0..201).map(|n| CollisionNode {
            kind: "braid".to_owned(),
            ..node(&format!("s{n:03}.md"), 0, None, Some("Lamp oil"))
        }));
        let only = collision_pairs(
            &mixed,
            &[],
            &CollisionQuery {
                kinds: &[KIND.to_owned()],
                any_kind: false,
                limit: 1,
            },
        );
        assert_eq!(
            only.left,
            200 * 199 / 2 - 1,
            "the cap counts within the group"
        );
    }

    /// `titles` and its rows under a trigram cap of 2: the pairs listed.
    fn capped_two(titles: &[&str]) -> CollisionList {
        let rows = CollisionRows {
            nodes: titles
                .iter()
                .enumerate()
                .map(|(n, title)| node(&format!("c{n}.md"), 0, None, Some(title)))
                .collect(),
            targets: Vec::new(),
            skipped: 0,
        };
        collision_pairs_capped(
            &rows,
            &[],
            &CollisionQuery {
                kinds: &[],
                any_kind: false,
                limit: 50,
            },
            &CollisionCaps {
                trigram_df_max: 2,
                ..CollisionCaps::ENGINE
            },
        )
    }

    /// The orchestrator's ruling R1 (iteration 2), a trigram cap of 2: `abc`
    /// and `zbc` share one rare trigram (`bc `, held by the two) among
    /// commoner ones (` ab`, `abc`, ` zb`, `zbc`, each held by three titles
    /// with the fillers): the rare one makes them a candidate, the commoner
    /// ones still count in Dice, 1 of 3 + 3: `titles` 333, no pair (dropped
    /// from the sets they would read 1 of 1 + 1, 1 000). M: the cap drops a
    /// trigram from the score again.
    #[test]
    fn a_capped_trigram_still_counts_in_the_score() {
        let list = capped_two(&["abc", "zbc", "abc1", "abc2", "zbc3", "zbc4"]);
        assert_eq!(list.pairs, Vec::new(), "{list:?}");
        assert_eq!(list.left, 0);
        // Under the engine's cap the same titles score the same Dice.
        let rows = CollisionRows {
            nodes: vec![
                node("c0.md", 0, None, Some("abc")),
                node("c1.md", 0, None, Some("zbc")),
            ],
            targets: Vec::new(),
            skipped: 0,
        };
        assert!(pairs_of(&rows, &[], 50).pairs.is_empty());
    }

    /// R1, a trigram cap of 2: two titles `Lamp oil` whose every trigram the
    /// fillers `Lamp oil x` and `Lamp oil y` make common (four holders each)
    /// pair through their words bucket (`lamp oil`, two titles), `titles`
    /// 1 000; a filler is a candidate of neither (no rare trigram shared, its
    /// bucket its own). M: the bucket dropped.
    #[test]
    fn an_exact_twin_of_capped_trigrams_pairs_through_its_words() {
        let list = capped_two(&["Lamp oil", "lamp  OIL", "Lamp oil x", "Lamp oil y"]);
        let names: Vec<(&str, &str, u32)> = list
            .pairs
            .iter()
            .map(|pair| (pair.a.path.as_str(), pair.b.path.as_str(), pair.titles))
            .collect();
        assert_eq!(names, [("c0.md", "c1.md", 1000)], "{list:?}");
        assert_eq!(list.pairs[0].reason, "titles alike 1.000");
        // `oil lamp`: the same words, another order, one bucket.
        let reordered = capped_two(&["Lamp oil", "oil lamp", "Lamp oil x", "Lamp oil y"]);
        assert_eq!(reordered.pairs.len(), 1, "{reordered:?}");
        assert_eq!(
            (
                reordered.pairs[0].a.path.as_str(),
                reordered.pairs[0].b.path.as_str()
            ),
            ("c0.md", "c1.md")
        );
    }

    /// CP2: ` abcd ` and ` abcx ` share 2 of 4 + 4 trigrams: `titles` 500,
    /// listed; a cap of 501 lists nothing; ` abcd ` and ` abxy ` (1 of 8,
    /// 250) never qualify. M: `titles` > 500 for >=.
    #[test]
    fn titles_of_500_qualify() {
        let rows = |second: &str| CollisionRows {
            nodes: vec![
                node("a.md", 0, None, Some("abcd")),
                node("b.md", 0, None, Some(second)),
            ],
            targets: Vec::new(),
            skipped: 0,
        };
        let listed = pairs_of(&rows("ABCX"), &[], 50);
        assert_eq!(listed.pairs.len(), 1);
        assert_eq!((listed.pairs[0].titles, listed.pairs[0].score), (500, 500));
        assert_eq!(listed.pairs[0].reason, "titles alike 0.500");
        let raised = collision_pairs_capped(
            &rows("ABCX"),
            &[],
            &CollisionQuery {
                kinds: &[],
                any_kind: false,
                limit: 50,
            },
            &CollisionCaps {
                titles_min: 501,
                ..CollisionCaps::ENGINE
            },
        );
        assert!(raised.pairs.is_empty());
        assert!(pairs_of(&rows("abxy"), &[], 50).pairs.is_empty());
    }

    /// CP1: nine shared targets score `200 × 5`, name the first eight by
    /// `(path, ord)` and say `, and 1 more`; one is 200.
    #[test]
    fn five_shared_targets_score_and_eight_are_named() {
        let (x, y) = (node("x.md", 0, None, None), node("y.md", 0, None, None));
        let ends = ["e9", "e1", "e2", "e3", "e4", "e5", "e6", "e7", "e8"];
        let mut targets = Vec::new();
        for end in ends {
            targets.push(target(&x, end));
            targets.push(target(&y, end));
        }
        let rows = CollisionRows {
            nodes: vec![x, y],
            targets,
            skipped: 0,
        };
        let pair = &pairs_of(&rows, &[], 50).pairs[0];
        assert_eq!((pair.shared, pair.score, pair.titles), (9, 1000, 0));
        assert_eq!(COLLISION_SHARED_LISTED_MAX, 8);
        assert_eq!(
            pair.shared_targets,
            ["e1", "e2", "e3", "e4", "e5", "e6", "e7", "e8"]
        );
        assert_eq!(
            pair.reason,
            "9 link target(s) shared: `e1`, `e2`, `e3`, `e4`, `e5`, `e6`, `e7`, `e8`, and 1 more"
        );
    }

    /// W10 and the duplicate ID: a document and its own section with one
    /// title never pair, two sections of it do; two holders of one ID never
    /// pair; a node without a norm is never paired and counts in `skipped`
    /// (its kind's only).
    #[test]
    fn the_never_paired_rules() {
        let rows = CollisionRows {
            nodes: vec![
                node("doc.md", 0, Some("PLT-DOC"), Some("Lamp oil store")),
                node("doc.md", 1, Some("PLT-ONE"), Some("Lamp oil store")),
                node("doc.md", 2, Some("PLT-TWO"), Some("Lamp oil stores")),
                node("twin1.md", 0, Some("PLT-TWIN"), Some("Brass hood")),
                node("twin2.md", 0, Some("PLT-TWIN"), Some("Brass hood")),
                CollisionNode {
                    norm_hash: None,
                    ..node("zz.md", 0, Some("PLT-NULL"), Some("Brass hood"))
                },
                CollisionNode {
                    norm_hash: None,
                    kind: "braid".to_owned(),
                    ..node("zz2.md", 0, Some("PLT-SCENE"), None)
                },
            ],
            targets: Vec::new(),
            skipped: 2,
        };
        let list = pairs_of(&rows, &[], 50);
        let names: Vec<(&str, &str)> = list
            .pairs
            .iter()
            .map(|pair| (pair.a.name(), pair.b.name()))
            .collect();
        assert_eq!(names, [("PLT-ONE", "PLT-TWO")], "{list:?}");
        let only = collision_pairs(
            &rows,
            &[],
            &CollisionQuery {
                kinds: &[KIND.to_owned()],
                any_kind: false,
                limit: 50,
            },
        );
        assert_eq!(only.skipped, 1, "the kinds kept");
        assert_eq!(list.skipped, 2);
    }

    fn judged(a: &str, a_norm: String, b: &str, b_norm: String) -> JudgedPair {
        JudgedPair {
            a: a.to_owned(),
            a_norm,
            b: b.to_owned(),
            b_norm,
            verdict: CollisionVerdict::Distinct,
            proposal: None,
            at: "2026-11-12T09:00:00Z".to_owned(),
        }
    }

    /// CP3: a verdict holding both norms hides the pair (and `left` counts
    /// it no more); either norm changed lists it `marked`, the reason's last
    /// part `marked distinct <at>, changed since`; a verdict keyed by names
    /// alone (the norms another node's) never hides it. M: the ledger keyed
    /// by name alone.
    #[test]
    fn verdicts_hide_a_pair_while_both_norms_hold() {
        let (x, y) = (
            node("x.md", 0, Some("PLT-X"), None),
            node("y.md", 0, Some("PLT-Y"), None),
        );
        let (xn, yn) = (x.norm_hash.clone().unwrap(), y.norm_hash.clone().unwrap());
        let z = node("z.md", 0, Some("PLT-Z"), None);
        let rows = CollisionRows {
            targets: vec![target(&x, "e"), target(&y, "e"), target(&z, "e")],
            nodes: vec![x, y, z],
            skipped: 0,
        };
        let open = pairs_of(&rows, &[], 1);
        assert_eq!((open.pairs.len(), open.left), (1, 2));
        let held = [judged("PLT-X", xn.clone(), "PLT-Y", yn.clone())];
        let hidden = pairs_of(&rows, &held, 1);
        assert_eq!(hidden.left, 1);
        assert!(
            pairs_of(&rows, &held, 50)
                .pairs
                .iter()
                .all(|pair| (pair.a.name(), pair.b.name()) != ("PLT-X", "PLT-Y"))
        );
        for changed in [
            [judged("PLT-X", norm(1), "PLT-Y", yn.clone())],
            [judged("PLT-X", xn.clone(), "PLT-Y", norm(2))],
        ] {
            let list = pairs_of(&rows, &changed, 50);
            let pair = list
                .pairs
                .iter()
                .find(|pair| (pair.a.name(), pair.b.name()) == ("PLT-X", "PLT-Y"))
                .expect("listed again");
            let marked = pair.marked.as_ref().expect("marked");
            assert_eq!(marked.verdict, CollisionVerdict::Distinct);
            assert_eq!(marked.at, "2026-11-12T09:00:00Z");
            assert_eq!(
                pair.reason,
                "1 link target(s) shared: `e`; marked distinct 2026-11-12T09:00:00Z, changed since"
            );
        }
    }

    /// CP4: the order is score, then `a`'s `(path, ord)`, then `b`'s, and
    /// the same whatever order the rows come in. M: the sort removed.
    #[test]
    fn the_order_is_the_rows_whatever_their_input_order() {
        let a = node("a.md", 0, None, Some("Wick trim"));
        let b = node("b.md", 0, None, Some("Wick trims"));
        let c = node("c.md", 0, None, None);
        let d = node("d.md", 1, Some("PLT-D"), None);
        let targets = vec![
            target(&a, "e"),
            target(&c, "e"),
            target(&b, "f"),
            target(&d, "f"),
            target(&c, "g"),
            target(&d, "g"),
        ];
        let rows = CollisionRows {
            nodes: vec![a, b, c, d],
            targets,
            skipped: 0,
        };
        let names = |list: &CollisionList| -> Vec<(String, String, u32)> {
            list.pairs
                .iter()
                .map(|pair| {
                    (
                        pair.a.name().to_owned(),
                        pair.b.name().to_owned(),
                        pair.score,
                    )
                })
                .collect()
        };
        let forward = pairs_of(&rows, &[], 50);
        let titles = forward.pairs[0].titles;
        // ` wick trim ` and ` wick trims ` share 8 of 9 + 10 trigrams:
        // floor(16 000 / 19) = 842; the three 200s by `a`'s path.
        assert_eq!(titles, 842);
        let want: Vec<(String, String, u32)> = [
            ("a.md", "b.md", 842),
            ("a.md", "c.md", 200),
            ("b.md", "PLT-D", 200),
            ("c.md", "PLT-D", 200),
        ]
        .iter()
        .map(|(a, b, score)| ((*a).to_owned(), (*b).to_owned(), *score))
        .collect();
        assert_eq!(names(&forward), want);
        let mut reversed = rows.clone();
        reversed.nodes.reverse();
        reversed.targets.reverse();
        assert_eq!(pairs_of(&reversed, &[], 50), forward, "the input's order");
    }

    fn judgement(a: &str, b: &str) -> PairJudgement {
        PairJudgement {
            a: a.to_owned(),
            a_norm: norm(1),
            b: b.to_owned(),
            b_norm: norm(2),
            verdict: CollisionVerdict::Distinct,
            proposal: None,
        }
    }

    /// CP5, the caps: 1 to 200 pairs; a name of 256 bytes passes, 257 not;
    /// a control character; one node twice; a bad norm; `reported` without a
    /// proposal and `distinct` with one; a pair repeated reversed. Then the
    /// names: an unknown name refuses, a stale norm is a note.
    #[test]
    fn the_marks_caps_and_names() {
        let field = |pairs: &[PairJudgement]| {
            judgement_problem(pairs).map(|problem| (problem.field, problem.problem))
        };
        assert_eq!(field(&[]).map(|f| f.0), Some("pairs".to_owned()));
        let many: Vec<PairJudgement> = (0..=COLLISION_MARKS_MAX)
            .map(|n| judgement(&format!("PLT-{n}"), "PLT-Z"))
            .collect();
        assert_eq!(field(&many).map(|f| f.0), Some("pairs".to_owned()));
        assert_eq!(field(&many[..COLLISION_MARKS_MAX]), None);
        assert_eq!(field(&[judgement(&"a".repeat(256), "PLT-Z")]), None);
        assert_eq!(
            field(&[judgement(&"a".repeat(257), "PLT-Z")]).map(|f| f.0),
            Some("pairs[0].a".to_owned())
        );
        assert_eq!(
            field(&[judgement("PLT-Z", "PLT-\u{7}")]).map(|f| f.0),
            Some("pairs[0].b".to_owned())
        );
        assert_eq!(
            field(&[judgement("PLT-Z", "PLT-Z")]),
            Some(("pairs[0]".to_owned(), "a pair of one node".to_owned()))
        );
        let mut bad = judgement("PLT-Y", "PLT-Z");
        bad.b_norm = format!("b3:{}", "AB".repeat(32));
        assert_eq!(
            field(&[bad]).map(|f| f.0),
            Some("pairs[0].b_norm".to_owned())
        );
        let mut reported = judgement("PLT-Y", "PLT-Z");
        reported.verdict = CollisionVerdict::Reported;
        assert_eq!(
            field(std::slice::from_ref(&reported)).map(|f| f.0),
            Some("pairs[0].proposal".to_owned())
        );
        reported.proposal = Some("PR-0007".to_owned());
        assert_eq!(field(&[reported]), None);
        let mut distinct = judgement("PLT-Y", "PLT-Z");
        distinct.proposal = Some("PR-0007".to_owned());
        assert_eq!(
            field(&[distinct]).map(|f| f.0),
            Some("pairs[0].proposal".to_owned())
        );
        assert_eq!(
            field(&[judgement("PLT-Y", "PLT-Z"), judgement("PLT-Z", "PLT-Y")]),
            Some(("pairs[1]".to_owned(), "repeats pairs[0]".to_owned()))
        );

        let y = CollisionNode {
            norm_hash: Some(norm(1)),
            ..node("y.md", 0, Some("PLT-Y"), None)
        };
        let z = CollisionNode {
            norm_hash: Some(norm(9)),
            ..node("z.md", 0, Some("PLT-Z"), None)
        };
        let rows = CollisionRows {
            nodes: vec![y, z],
            targets: Vec::new(),
            skipped: 0,
        };
        assert_eq!(
            judged_notes(&rows, &[judgement("PLT-Y", "PLT-Z")]),
            Ok(vec![
                "pairs[0]: PLT-Z changed since it was listed; the pair will be listed again"
                    .to_owned()
            ])
        );
        let unknown = judged_notes(&rows, &[judgement("PLT-Y", "PLT-W")]).expect_err("unknown");
        assert_eq!(
            (unknown.field.as_str(), unknown.problem.as_str()),
            ("pairs[0].b", "no live node `PLT-W`")
        );
    }
}
