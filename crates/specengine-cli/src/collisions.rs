//! `spec collisions [--kind K]... [--any-kind] [--limit N]` and `spec
//! collisions mark --input F|- [--author-role R [--author-model M] [--run
//! ID]]` (task spec `collision-pass` CP1 to CP5): the pairs of spec nodes
//! that may collide, found from the index, and an agent's verdicts on them,
//! kept in the queue DB's ledger. Neither moves, refuses or gates anything:
//! a pair, a mark or a stale mark is advice.
//!
//! The read refreshes the index as the reads do, reads its universe once
//! ([`SpecIndex::collision_input`]) and the ledger once
//! ([`SqliteQueue::collision_marks`], opened with
//! [`SqliteQueue::open_existing`]: no schema step, nothing written; a queue
//! at schema 13 or lower reads no mark), then core's
//! [`collision_pairs`] scores, orders and filters the pairs. No clock is
//! read: one index and ledger state gives one text and one JSON at any root
//! and `HOME`.
//!
//! ```text
//! collisions: <n> pair(s) listed, <left> left, <skipped> node(s) skipped
//! <score> | <a name> | <b name> | <a kind>[/<b kind>] | <reason>
//! ```
//!
//! The head line prints, then each pair whole while the text stays within
//! [`OUTPUT_CAP_CHARS`] (the first always): a pair cut by the cap is
//! counted in `left` and named in a note, and the JSON holds the pairs the
//! text prints. The notes go to stderr as `note:` lines and into the JSON's
//! `notes`; every text line's control characters are escaped, the JSON
//! holds the values raw. Exit 0 with no pair too; exit 2: a `--limit`
//! outside 1 to [`COLLISIONS_LIMIT_MAX`], no project, an index or a queue
//! that cannot be read (a newer build's).
//!
//! The mark runs its rules in order, the first refusal answering with
//! nothing written: the form (exit 2: `--input` not UTF-8 JSON of the
//! shape, an unknown key, a verdict but `distinct` or `reported`); core's
//! caps, then the author (exit 1); each `proposal` in the project's queue;
//! each name a node of the read's universe; then one `Immediate`
//! transaction writes one row per pair ([`SqliteQueue::mark_collisions`]),
//! no event. A side whose norm differs from the index's is a note, never a
//! refusal. Its text is `marked <n> pair(s), <r> replacing an earlier mark`;
//! a refusal's reason is `spec: <reason>` on stderr and the JSON's last
//! note.

use std::fs;
use std::io::Read as _;

use serde::Serialize;
use specengine_core::collisions::{
    COLLISIONS_LIMIT_DEFAULT, COLLISIONS_LIMIT_MAX, CollisionList, CollisionPair, CollisionQuery,
    JudgedPair, PairJudgement, PairJudgements, collision_pairs, judged_notes, judgement_problem,
};
use specengine_core::intake::{AuthorInput, author_problem};
use specengine_core::proposal::Author;
use specengine_store::{
    CollisionMarks, NewCollisionMark, SpecIndex as _, SqliteQueue, WorkingTree,
};

use crate::cap::{OUTPUT_CAP_CHARS, lines_within};
use crate::freshness::step;
use crate::intake::{INTAKE_INPUT_MAX_BYTES, IntakeSource};
use crate::location::open_index;
use crate::project::discover;
use crate::proposals::{escaped_error, queue_cannot, require_root_config};
use crate::search::notes;
use crate::{CliError, Env, Globals, Message, escape_controls, one_line, store_error};

/// `spec collisions` options.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CollisionsRequest {
    /// `--kind`: keep the groups of these kinds; empty: every kind.
    pub kinds: Vec<String>,
    /// `--any-kind`: one group across the kinds kept.
    pub any_kind: bool,
    /// `--limit`; `None`: [`COLLISIONS_LIMIT_DEFAULT`].
    pub limit: Option<i64>,
}

/// What `spec collisions` found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollisionsOutcome {
    /// The project's slug.
    pub project: String,
    /// As asked.
    pub kinds: Vec<String>,
    pub any_kind: bool,
    pub limit: usize,
    /// The pairs listed, in order, those the text prints.
    pub pairs: Vec<CollisionPair>,
    /// The qualifying pairs past them: past the limit, or cut by the cap.
    pub left: usize,
    /// The nodes of the kinds kept without a `norm_hash`, never paired.
    pub skipped: usize,
    /// The walk's notes and warnings, then the pass's notes, in order.
    pub messages: Vec<Message>,
}

impl CollisionsOutcome {
    /// Every message's stderr line, escaped.
    pub(crate) fn stderr_lines(&self) -> Vec<String> {
        self.messages
            .iter()
            .map(|message| escape_controls(&message.line()))
            .collect()
    }
}

/// `{project, kinds, any_kind, limit, pairs, left, skipped, notes}`.
#[derive(Serialize)]
struct CollisionsJson<'o> {
    project: &'o str,
    kinds: &'o [String],
    any_kind: bool,
    limit: usize,
    pairs: &'o [CollisionPair],
    left: usize,
    skipped: usize,
    notes: Vec<String>,
}

impl Serialize for CollisionsOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        CollisionsJson {
            project: &self.project,
            kinds: &self.kinds,
            any_kind: self.any_kind,
            limit: self.limit,
            pairs: &self.pairs,
            left: self.left,
            skipped: self.skipped,
            notes: notes(&self.messages),
        }
        .serialize(serializer)
    }
}

/// `spec collisions`: the index refreshed and its universe read, the ledger
/// read, the pairs computed.
pub fn collisions(
    env: &Env,
    globals: &Globals,
    request: &CollisionsRequest,
) -> Result<CollisionsOutcome, CliError> {
    run_collisions(env, globals, request).map_err(escaped_error)
}

fn run_collisions(
    env: &Env,
    globals: &Globals,
    request: &CollisionsRequest,
) -> Result<CollisionsOutcome, CliError> {
    let limit = collisions_limit(request.limit)?;
    let project = discover(env, globals)?;
    let mut open = open_index(env, &project)?;
    let tree = WorkingTree::new(&project.root, &project.config.paths).map_err(store_error)?;
    let mut messages = step(&mut open.index, &project, &tree)?;
    let rows = open.index.collision_input().map_err(store_error)?;
    let ledger = match SqliteQueue::open_existing(&open.db, &open.slug).map_err(queue_cannot)? {
        Some(queue) => queue.collision_marks().map_err(queue_cannot)?,
        None => CollisionMarks::default(),
    };
    let judged: Vec<JudgedPair> = ledger
        .marks
        .iter()
        .map(|mark| JudgedPair {
            a: mark.a.clone(),
            a_norm: mark.a_norm.clone(),
            b: mark.b.clone(),
            b_norm: mark.b_norm.clone(),
            verdict: mark.verdict,
            proposal: mark.proposal.clone(),
            at: mark.at.clone(),
        })
        .collect();
    let CollisionList {
        mut pairs,
        mut left,
        skipped,
    } = collision_pairs(
        &rows,
        &judged,
        &CollisionQuery {
            kinds: &request.kinds,
            any_kind: request.any_kind,
            limit,
        },
    );
    if skipped > 0 {
        messages.push(Message::Note(format!(
            "{skipped} node(s) without a norm_hash not paired: their files do not parse whole"
        )));
    }
    messages.extend(ledger.unreadable.iter().cloned().map(Message::Note));
    // The cap: the head line at its longest, then each pair whole.
    let longest_head = head_line(pairs.len(), left + pairs.len(), skipped);
    let costs = pairs.iter().map(|pair| pair_line(pair).chars().count() + 1);
    let (shown, _) = lines_within(costs, longest_head.chars().count() + 1, true);
    if shown < pairs.len() {
        let cut = pairs.len() - shown;
        pairs.truncate(shown);
        left += cut;
        messages.push(Message::Note(format!(
            "{cut} pair(s) past the {OUTPUT_CAP_CHARS}-character cap counted in left: judge the \
             pairs listed, then call again"
        )));
    }
    Ok(CollisionsOutcome {
        project: open.slug,
        kinds: request.kinds.clone(),
        any_kind: request.any_kind,
        limit,
        pairs,
        left,
        skipped,
        messages,
    })
}

/// `--limit` as given, else the default; outside 1 to
/// [`COLLISIONS_LIMIT_MAX`]: exit 2.
fn collisions_limit(limit: Option<i64>) -> Result<usize, CliError> {
    match limit {
        None => Ok(COLLISIONS_LIMIT_DEFAULT),
        Some(given) => usize::try_from(given)
            .ok()
            .filter(|limit| (1..=COLLISIONS_LIMIT_MAX).contains(limit))
            .ok_or_else(|| {
                CliError::spec(format!(
                    "--limit {given}: the limit is 1 to {COLLISIONS_LIMIT_MAX}"
                ))
            }),
    }
}

/// `collisions: <n> pair(s) listed, <left> left, <skipped> node(s) skipped`.
fn head_line(listed: usize, left: usize, skipped: usize) -> String {
    format!("collisions: {listed} pair(s) listed, {left} left, {skipped} node(s) skipped")
}

/// `<score> | <a name> | <b name> | <a kind>[/<b kind>] | <reason>`, on one
/// line, escaped.
fn pair_line(pair: &CollisionPair) -> String {
    let kinds = if pair.a.kind == pair.b.kind {
        pair.a.kind.clone()
    } else {
        format!("{}/{}", pair.a.kind, pair.b.kind)
    };
    escape_controls(&one_line(&format!(
        "{} | {} | {} | {kinds} | {}",
        pair.score,
        pair.a.name(),
        pair.b.name(),
        pair.reason
    )))
}

/// The head line, then one line per pair.
pub(crate) fn render_text(outcome: &CollisionsOutcome) -> String {
    let mut out = head_line(outcome.pairs.len(), outcome.left, outcome.skipped);
    out.push('\n');
    for pair in &outcome.pairs {
        out.push_str(&pair_line(pair));
        out.push('\n');
    }
    out
}

/// `spec collisions mark` options: the input read, and the author.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkPairsRequest {
    /// The pairs, as `--input` or the tool gives them.
    pub input: PairJudgements,
    pub author_role: Option<String>,
    pub author_model: Option<String>,
    pub run: Option<String>,
    /// The injected clock: `YYYY-MM-DDTHH:MM:SSZ`.
    pub now: String,
}

/// What `spec collisions mark` wrote, or why it wrote nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairsMarked {
    /// The pairs written.
    pub marked: usize,
    /// Of them, those that replaced an earlier mark of the pair.
    pub replaced: usize,
    /// The sides changed since they were listed, in input order.
    pub notes: Vec<String>,
    /// Why nothing was written (exit 1).
    pub refusal: Option<String>,
    /// The walk's notes and warnings.
    pub messages: Vec<Message>,
}

impl PairsMarked {
    fn refused(reason: String, messages: Vec<Message>) -> Self {
        Self {
            marked: 0,
            replaced: 0,
            notes: Vec::new(),
            refusal: Some(reason),
            messages,
        }
    }

    /// 0 marked; 1 refused.
    pub fn exit(&self) -> crate::Exit {
        if self.refusal.is_some() {
            crate::Exit::NotFound
        } else {
            crate::Exit::Answered
        }
    }

    /// The walk's lines, each note as `note:`, then `spec: <reason>`, each
    /// escaped.
    pub(crate) fn stderr_lines(&self) -> Vec<String> {
        let mut lines: Vec<String> = self.messages.iter().map(Message::line).collect();
        lines.extend(
            self.notes
                .iter()
                .map(|note| format!("note: {}", one_line(note))),
        );
        if let Some(reason) = &self.refusal {
            lines.push(format!("spec: {}", one_line(reason)));
        }
        lines.iter().map(|line| escape_controls(line)).collect()
    }
}

/// `{marked, replaced, notes}`, a refusal's reason the last note.
#[derive(Serialize)]
struct MarkJson<'o> {
    marked: usize,
    replaced: usize,
    notes: Vec<&'o str>,
}

impl Serialize for PairsMarked {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        MarkJson {
            marked: self.marked,
            replaced: self.replaced,
            notes: self
                .notes
                .iter()
                .map(String::as_str)
                .chain(self.refusal.as_deref())
                .collect(),
        }
        .serialize(serializer)
    }
}

/// `marked <n> pair(s), <r> replacing an earlier mark`.
pub(crate) fn render_mark_text(outcome: &PairsMarked) -> String {
    format!(
        "marked {} pair(s), {} replacing an earlier mark\n",
        outcome.marked, outcome.replaced
    )
}

/// `--input F|-` of `spec collisions mark`: UTF-8 JSON `{"pairs": [{a,
/// a_norm, b, b_norm, verdict, proposal}]}`, no other key, at most
/// [`INTAKE_INPUT_MAX_BYTES`]; anything else exits 2, its message's control
/// characters escaped (the path and the parser's quote are agent-written).
pub fn read_pairs_input(env: &Env, source: &IntakeSource) -> Result<PairJudgements, CliError> {
    read_pairs(env, source).map_err(escaped_error)
}

fn read_pairs(env: &Env, source: &IntakeSource) -> Result<PairJudgements, CliError> {
    let read: Vec<u8>;
    let (label, bytes) = match source {
        IntakeSource::Given(bytes) => ("-".to_owned(), bytes.as_slice()),
        IntakeSource::File(path) => {
            let label = path.display().to_string();
            let cannot =
                |error: std::io::Error| CliError::spec(format!("--input {label}: {error}"));
            let file = fs::File::open(env.cwd.join(path)).map_err(cannot)?;
            let mut bytes = Vec::new();
            file.take(INTAKE_INPUT_MAX_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(cannot)?;
            read = bytes;
            (label, read.as_slice())
        }
    };
    if bytes.len() > INTAKE_INPUT_MAX_BYTES {
        return Err(CliError::spec(format!(
            "--input {label}: longer than {INTAKE_INPUT_MAX_BYTES} bytes (8 MiB)"
        )));
    }
    let text = std::str::from_utf8(bytes).map_err(|error| {
        CliError::spec(format!(
            "--input {label}: not UTF-8 (at byte {})",
            error.valid_up_to()
        ))
    })?;
    serde_json::from_str(text).map_err(|error| {
        CliError::spec(format!(
            "--input {label}: not a mark's pairs ({{\"pairs\": [{{a, a_norm, b, b_norm, verdict, \
             proposal}}]}}, verdict distinct or reported): {error}"
        ))
    })
}

/// `spec collisions mark`: the rules in order, then one transaction.
pub fn mark_pairs(
    env: &Env,
    globals: &Globals,
    request: &MarkPairsRequest,
) -> Result<PairsMarked, CliError> {
    run_mark(env, globals, request).map_err(escaped_error)
}

fn run_mark(
    env: &Env,
    globals: &Globals,
    request: &MarkPairsRequest,
) -> Result<PairsMarked, CliError> {
    let pairs = &request.input.pairs;
    // 2. Core's caps, then the author.
    if let Some(problem) = judgement_problem(pairs) {
        return Ok(PairsMarked::refused(problem.to_string(), Vec::new()));
    }
    let written = AuthorInput {
        role: request.author_role.as_deref(),
        model: request.author_model.as_deref(),
        run: request.run.as_deref(),
    };
    if let Some(problem) = author_problem(written) {
        return Ok(PairsMarked::refused(problem.to_string(), Vec::new()));
    }
    let author = Author::new(
        request.author_role.clone(),
        request.author_model.clone(),
        request.run.clone(),
    )
    .map_err(CliError::spec)?;
    let project = discover(env, globals)?;
    require_root_config(env, globals, &project)?;
    let mut open = open_index(env, &project)?;
    // 3. Each proposal in the project's queue, in any state.
    let queue = SqliteQueue::open_existing(&open.db, &open.slug).map_err(queue_cannot)?;
    for (index, pair) in pairs.iter().enumerate() {
        let Some(id) = pair.proposal.as_deref() else {
            continue;
        };
        let held = match &queue {
            Some(queue) => queue.proposal_exists(id).map_err(queue_cannot)?,
            None => false,
        };
        if !held {
            return Ok(PairsMarked::refused(
                format!("pairs[{index}].proposal: no proposal {id}"),
                Vec::new(),
            ));
        }
    }
    drop(queue);
    // 4. Each name a node of the universe, as `spec collisions` names it.
    let tree = WorkingTree::new(&project.root, &project.config.paths).map_err(store_error)?;
    let messages = step(&mut open.index, &project, &tree)?;
    let rows = open.index.collision_input().map_err(store_error)?;
    let notes = match judged_notes(&rows, pairs) {
        Ok(notes) => notes,
        Err(problem) => return Ok(PairsMarked::refused(problem.to_string(), messages)),
    };
    // 5. One `Immediate` transaction, one row per pair.
    let marks: Vec<NewCollisionMark> = pairs.iter().map(new_mark).collect();
    let mut queue = SqliteQueue::open(&open.db, &open.slug).map_err(queue_cannot)?;
    let written = queue
        .mark_collisions(&marks, &author, &request.now)
        .map_err(queue_cannot)?;
    Ok(PairsMarked {
        marked: written.marked,
        replaced: written.replaced,
        notes,
        refusal: None,
        messages,
    })
}

fn new_mark(pair: &PairJudgement) -> NewCollisionMark {
    NewCollisionMark {
        a: pair.a.clone(),
        a_norm: pair.a_norm.clone(),
        b: pair.b.clone(),
        b_norm: pair.b_norm.clone(),
        verdict: pair.verdict,
        proposal: pair.proposal.clone(),
    }
}
