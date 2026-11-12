//! `spec card …` (canon `cards`; task spec `card-store`): a card is the
//! lighter record of the work of any trade, a row of the project's queue
//! beside its tasks, never a file or a commit. The project's `[card]` gives
//! each discipline its columns, its moves and who makes them; no code here
//! reads a column's name.
//!
//! - `new --title T … [--reviewer N]` → `created C-0001: plain/backlog`;
//!   `list [--parent C]` → a line per flow, then per card; `show C
//!   [--history] [--marks] [--context [--budget N]]` → `key: value` lines,
//!   with `--context` its parent's goal and one bundle of its targets and
//!   its own applied records, the body last, in the room the lines leave
//!   under the ceiling (task spec `card-context` CC1-CC5). With `--marks`,
//!   `show` and `list` say what changed above an open subtask, its own key
//!   beside the mark and never folded into it (`crate::upstream`, task spec
//!   `upstream-signal` US-4, US-8).
//! - `move C [--to K] [--before C] [--note T]`, `edit C …`, `assign C …`,
//!   `cancel C --reason T` → `<verb> C-0001: <flow>/<column>`; JSON `{id,
//!   flow, column, position, revision, allowed, notes}`, a refusal too; no
//!   such card `{id, reason}`. In `edit`, `assign` a value `-` clears.
//! - A hand-off is a child card (`--parent`) in another trade's flow, its
//!   reviewer the asking side; a move into a flow's `return` column from
//!   another one says why (`--note`), and no write touches another card
//!   (canon `card-flows`, "The rule").
//! - `accept C --as N (--briefed | --departs REF... --summary T --observed T
//!   --documented T [--gap-type G])`, with a `Consent`: a person accepts, on
//!   a terminal (`main` refuses a stdin that is no terminal before anything
//!   is read) or the local page, and no agent tool does. Either verdict
//!   freezes the snapshot again, a correction's also that of the card it
//!   corrects. `--departs` makes the discrepancy in the accept's transaction,
//!   its hits named by the accept itself, so the dedup never refuses one.
//! - `clear C --no-effect|--needs-correction --reason T --as N` and `clear
//!   --group REF --no-effect --reason T --as N`, with a `Consent`: a person
//!   answers a suspect mark, and no agent tool does. A mark is computed per
//!   read (core's `trace`); only what it is computed from is written.
//! - `edit --estimate E --reason T` on a card holding an estimate, `-` for
//!   `E` too: `card.reestimated`, `new` null when cleared, no other field
//!   beside it, so the record keeps `old`.
//!
//! `--as N` is self-declared, recorded and compared with a move's `by`, never
//! verified; absent: `local`. A tool acts as `agent`. The daemon's page (task
//! spec `board-ui`) writes through these same calls: each `Card*Body` is a
//! route's JSON, decoded here, its `as` as self-declared as the flag. A
//! person is core's `card_person_problem`: any script, inner spaces, trimmed
//! wherever it is stored or compared; no grammar of a task or a proposal. A
//! hub's handlers set `as` to the login first; its group clear also gives the
//! `[team]` entry whose cover is judged, and the count the page showed, both
//! here, the set known, before the consent. A terminal gives neither.
//!
//! Exits as the queue's: 0 done; 1 refused (no such card, a move outside its
//! flow, a closed card, a cap, a declined prompt, a stale `--revision`); 2
//! cannot run (`C` in `[ids]`, a look-alike ID, a bad `[card]`, no terminal,
//! a corrupt row, nothing asked). Nothing here opens git, so any root works,
//! but for `--departs`, which binds its discrepancy to a place as `propose
//! update` does and reads git to do it: reads only, exit 2 outside a
//! worktree; and `show` of a card that links `T-` tasks (`edit --task T…`),
//! which reads each now as `spec task list` does, a task it cannot read a
//! note, never an exit; and `show --context`, whose bundle reads the corpus
//! and refreshes the index as `spec bundle` does, a bundle it cannot make a
//! note, never an exit. Beside a card's own items, only the upstream read of
//! an open subtask reads proposals: the applied questions, discrepancies and
//! decisions of every repository, no git run. Nothing else reads a proposal,
//! a lock or a task, no rule reads a link (canon `card-tasks`, "The rule"),
//! and nothing is written under the root: the queue is in the data
//! directory.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::slice;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use specengine_core::TeamEntry;
use specengine_core::card::{
    AGENT_ACTOR, AllowedMove, CLEAR, CardIdError, EVIDENCE_ENTRIES_MAX, HISTORY_MAX, LOCAL_ACTOR,
    accept_refusal, allowed_moves, card_look_alike_message, card_move as decide_move, card_number,
    card_person, card_person_problem, card_prefix_clash, closed_refusal, due_problem,
    estimate_problem, mandate_problem, parse_card_id, parse_evidence, reviewer_problem,
    split_mandates, stale_refusal, task_links_problem,
};
use specengine_core::card_split::{ScopeNode, SplitPlan, TargetsFrom, split_plan};
use specengine_core::card_toml::{CardConfig, CardFlow, name_problem};
use specengine_core::check::{
    CheckInput, NodeAt, Resolver, SpecGraph, bundle_task_from_toml, canonical_id,
};
use specengine_core::intake::{
    AuthorInput, DECISION_KIND, DecisionInput, GapType, IntakeSeverity, QUESTION_KIND,
    discrepancy_problem,
};
use specengine_core::proposal::{Author, prefix_clash};
use specengine_core::task::{
    CRITERIA_MAX, CRITERION_MAX, DEFAULT_TASK_BUDGET, GOAL_MAX, NODES_MAX, NOTE_MAX, TITLE_MAX,
    over_bytes, over_items, task_prefix_clash,
};
use specengine_core::trace::{
    CardMark, Departure, MarkFacts, TargetNorms, card_mark, clear_refusal, clearer_problem,
    correction_refusal, correction_title, departure_input, freeze, mark_notes, suspect_on,
};
use specengine_core::upstream::Upstream;
use specengine_model::{
    Card, CardAcceptance, CardCancellation, CardEvidence, CardFrozen, Mandate, TaskStatus,
};
use specengine_store::{
    CardAccept, CardChange, CardClear, CardClearing, CardDeparture, CardEdit, Event, GitEnv,
    IntakeResult, NewCard, NewIntake, NewSplit, NewSplitChild, Place, Proposal, ProposalStatus,
    QueueError, SplitParent, SqliteQueue, WorktreeGit,
};

use crate::apply::Consent;
use crate::bundle::{Bundle, BundleOutcome, BundleRequest, budget_of, bundle_within};
use crate::cap::{OUTPUT_CAP_CHARS, View, lines_within};
use crate::corpus::{Admission, LeftOut, indexed_in, locate};
use crate::intake::{
    DecisionRequest, IntakeOutcome, IntakeTarget, QuestionRequest, departing_hits,
    departure_intake, prefixed, propose_decision_as, propose_question_as,
};
use crate::location::{open_index, prepared_data_dir};
use crate::project::{ProjectRoot, config_error, discover_with_text};
use crate::proposals::{checked_now, escaped_error, queue_cannot, require_root_config};
use crate::propose::resolved_node;
use crate::search::notes;
use crate::show::classify;
use crate::task::{LinkPlace, LinkedTask, linked_tasks, resolved_list, written_task};
use crate::trace::{
    QueueRead, correction_of, facts_of, fresh_norms, fresh_norms_reach, group_marks,
    held_documents, norms_read, settled_of, target_norms, target_places,
};
use crate::tree::walked;
use crate::upstream::{
    Judge, Reaches, UpstreamFacts, judged_parent, no_anchor, parent_unread, reach_of,
    upstream_lines,
};
use crate::{
    CliError, Env, Exit, Globals, Message, SPLIT_DRY_RUN_FLAG, escape_controls, one_line,
    process_git,
};

/// How a line prints an absent field.
const ABSENT: &str = "-";

/// Why a group's `@rev` is ignored.
const GROUP_REV_IGNORED: &str = "`spec card clear --group` reads the current files";

/// Who a tool acts as (MCP's card tools).
pub const TOOL_ACTOR: &str = AGENT_ACTOR;

/// How deep under each parent target a split's scope reads: a document's
/// Inside (task spec `department-split` S2).
const SCOPE_DEPTH: usize = 2;

/// `spec card new` options.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CardNewRequest {
    pub title: String,
    pub goal: Option<String>,
    pub discipline: Option<String>,
    /// `--target REF…`, resolved as `task new --nodes`; none: a manual card.
    pub targets: Vec<String>,
    pub criteria: Vec<String>,
    pub assignee: Option<String>,
    /// `--reviewer N`: who accepts its result, as `assign` names one; a
    /// hand-off's asking side (canon `card-flows`, "The rule").
    pub reviewer: Option<String>,
    /// `--mandate M`: who decides what its brief leaves open; needs a
    /// reviewer (canon `card-mandate`, "The rule").
    pub mandate: Option<Mandate>,
    pub estimate: Option<String>,
    pub due: Option<String>,
    pub parent: Option<String>,
    pub corrects: Option<String>,
    /// `--as N`; `None`: `local`.
    pub actor: Option<String>,
    pub now: String,
}

/// `spec card list` options: filters ANDed, exact.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CardListRequest {
    pub flow: Option<String>,
    pub column: Option<String>,
    pub discipline: Option<String>,
    pub assignee: Option<String>,
    /// `--no-target`: manual cards only.
    pub no_target: bool,
    /// `--with-cancelled`: cancelled cards too, last in their column.
    pub with_cancelled: bool,
    /// `--marks`: each card's mark over all its targets.
    pub marks: bool,
    /// `--parent C`: only the cards whose `parent` is exactly `C`, a card
    /// ID (a look-alike, or no ID at all, exits 2).
    pub parent: Option<String>,
}

/// `spec card show C [--history] [--marks] [--context [--budget N]]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardShowRequest {
    pub id: String,
    /// The card's last `card.*` events.
    pub history: bool,
    /// `--marks`: its mark over all its targets, as its `list --marks` row.
    pub marks: bool,
    /// What changed above it, an open subtask ([`Upstream`]): the CLI's
    /// `--marks`, MCP's `get_card` and the daemon's card route; `false`:
    /// nothing read for it (task spec `upstream-signal` US-8, US-12).
    pub upstream: bool,
    /// `--context`: its parent's goal and one bundle of its targets and
    /// its applied records ([`CardContext`]); `false`: none read.
    pub context: bool,
    /// `--budget N`: the context's bundle budget, estimated tokens, 1 to
    /// `u32::MAX`; `None`: `[budgets] bundle_task`, else
    /// [`DEFAULT_TASK_BUDGET`]. Needs `context`.
    pub budget: Option<i64>,
    /// The reading repository's git: where a linked task's state is read
    /// (`TaskListRequest`'s), on a card that links one alone.
    pub git: GitEnv,
    /// This queue holds the project's `T-` tasks: true for the CLI, the
    /// stdio MCP and a local daemon; false on a hub, which reads no task
    /// (canon `card-tasks`, "The rule").
    pub tasks_here: bool,
}

/// `spec card move C [--to K] [--before C] [--note T]`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CardMoveRequest {
    pub id: String,
    /// A column of its flow; `None`: its own, with `before`.
    pub to: Option<String>,
    /// The card whose place it takes; `None`: last.
    pub before: Option<String>,
    /// `--note T`: why it moves, kept on its `card.moved`; required into
    /// the flow's `return` column from another column.
    pub note: Option<String>,
    /// `--revision R`: a compare-and-set on what the caller read.
    pub revision: Option<String>,
    /// `--as N`; `None`: `local`.
    pub actor: Option<String>,
    pub now: String,
}

/// `spec card edit C …`: a value `-` clears; `targets`, `criteria` replace,
/// `evidence` appends. No `Default`: [`CardEditRequest::tasks_here`] is each
/// caller's to state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardEditRequest {
    pub id: String,
    pub title: Option<String>,
    pub goal: Option<String>,
    pub targets: Vec<String>,
    pub criteria: Vec<String>,
    pub estimate: Option<String>,
    pub due: Option<String>,
    pub parent: Option<String>,
    pub corrects: Option<String>,
    /// Each `KIND=VALUE`.
    pub evidence: Vec<String>,
    /// `--task T…`: the `T-` tasks it links, replacing the list in the
    /// order given; a lone `-` clears it.
    pub tasks: Vec<String>,
    /// `--mandate M`: the mandate it takes, only from the card's reviewer
    /// (canon `card-mandate`, "The rule"); `None` leaves it.
    pub mandate: Option<Mandate>,
    /// `--reason T`: why an estimate the card holds changes.
    pub reason: Option<String>,
    pub revision: Option<String>,
    pub actor: Option<String>,
    pub now: String,
    /// This queue holds the project's `T-` tasks, so each linked one must
    /// name a task row of it; false on a hub, which keeps the IDs as written
    /// (canon `card-tasks`, "The rule").
    pub tasks_here: bool,
}

/// `spec card assign C [--assignee N] [--reviewer N]`: a value `-` clears.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CardAssignRequest {
    pub id: String,
    pub assignee: Option<String>,
    pub reviewer: Option<String>,
    pub revision: Option<String>,
    pub actor: Option<String>,
    pub now: String,
}

/// `spec card cancel C --reason T`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CardCancelRequest {
    pub id: String,
    pub reason: String,
    pub revision: Option<String>,
    pub actor: Option<String>,
    pub now: String,
}

/// `spec card accept C --as N (--briefed | --departs REF... --summary T
/// --observed T --documented T [--gap-type G]) [--note T]`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CardAcceptRequest {
    pub id: String,
    /// `--as N`, required: the card's reviewer.
    pub actor: String,
    pub note: Option<String>,
    pub revision: Option<String>,
    pub now: String,
    /// `--briefed`: the result is what the brief asked.
    pub briefed: bool,
    /// `--departs REF...`: direct targets the result departs from.
    pub departs: Vec<String>,
    pub summary: Option<String>,
    pub observed: Option<String>,
    pub documented: Option<String>,
    /// `--gap-type G`; `None`: `contradicts`.
    pub gap_type: Option<String>,
    /// A hub's login: a departing accept's `Author.login`, the one place a
    /// proposal names a person. `None` everywhere else.
    pub login: Option<String>,
}

/// What a clear says of a mark.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CardClearOutcome {
    /// The change does not touch the work.
    #[default]
    NoEffect,
    /// The accepted work needs a correction card.
    NeedsCorrection,
}

/// `spec card clear C (--no-effect | --needs-correction --estimate E [--title
/// T]) --reason T --as N`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CardClearRequest {
    pub id: String,
    pub outcome: CardClearOutcome,
    pub reason: String,
    /// The correction's estimate.
    pub estimate: Option<String>,
    /// The correction's title; `None`: `Correct <title>`.
    pub title: Option<String>,
    pub revision: Option<String>,
    /// `--as N`, required: a person, never `local` or `agent`.
    pub actor: Option<String>,
    pub now: String,
}

/// `spec card clear --group REF --no-effect --reason T --as N`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CardClearGroupRequest {
    /// `REF` as given.
    pub group: String,
    pub reason: String,
    /// `--as N`, required: a person, never `local` or `agent`.
    pub actor: Option<String>,
    pub now: String,
    /// The number of cards the page showed: another number is refused.
    /// `None`: not compared.
    pub count: Option<u64>,
    /// A hub's `[team]` entry of the login: every node the clear re-freezes
    /// must lie in its cover. `None`: nothing judged.
    pub entry: Option<TeamEntry>,
}

/// `bytes` as the JSON of one route's body, `shape` its keys; else exit 2
/// naming the defect, nothing read or changed.
fn decoded<T: DeserializeOwned>(bytes: &[u8], shape: &str) -> Result<T, CliError> {
    serde_json::from_slice(bytes).map_err(|error| {
        escaped_error(CliError::spec(format!(
            "the card's body is not `{{{shape}}}` as JSON: {error}; nothing changed"
        )))
    })
}

/// The persons a body names, judged as the flags judge them: a person's
/// typing is the daemon's 400 with the CLI's line, read before any call,
/// whether the flag's exit is 2 (`as`) or 1 (each of `named`, a key and its
/// name).
fn persons_named(actor: Option<&str>, named: &[(&str, Option<&str>)]) -> Result<(), CliError> {
    actor_of(actor)
        .and_then(|_| {
            named
                .iter()
                .find_map(|(key, name)| name.and_then(|name| person_problem(key, name)))
                .map_or(Ok(()), |problem| Err(CliError::spec(problem)))
        })
        .map_err(escaped_error)
}

/// The body of `POST …/cards`: `create_card`'s keys and `as`. Any other key,
/// or no `title`, is refused.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CardNewBody {
    pub title: String,
    pub goal: Option<String>,
    pub discipline: Option<String>,
    /// Nodes it answers; none: a manual card.
    pub target: Option<Vec<String>>,
    pub criterion: Option<Vec<String>>,
    pub assignee: Option<String>,
    /// Who accepts its result.
    pub reviewer: Option<String>,
    /// `binding` or `open`; `null` or absent: none.
    pub mandate: Option<Mandate>,
    pub estimate: Option<String>,
    pub due: Option<String>,
    pub parent: Option<String>,
    pub corrects: Option<String>,
    /// The acting name; `null`: `local`.
    #[serde(rename = "as")]
    pub actor: Option<String>,
}

impl CardNewBody {
    /// `bytes` decoded; else exit 2 naming the defect, nothing read or
    /// changed.
    pub fn from_json(bytes: &[u8]) -> Result<Self, CliError> {
        let body: Self = decoded(
            bytes,
            "\"title\",\"goal\",\"discipline\",\"target\",\"criterion\",\"assignee\",\
             \"reviewer\",\"mandate\",\"estimate\",\"due\",\"parent\",\"corrects\",\"as\"",
        )?;
        persons_named(
            body.actor.as_deref(),
            &[
                ("assignee", body.assignee.as_deref()),
                ("reviewer", body.reviewer.as_deref()),
            ],
        )?;
        Ok(body)
    }

    /// The request at `now`.
    pub fn request(self, now: String) -> CardNewRequest {
        CardNewRequest {
            title: self.title,
            goal: self.goal,
            discipline: self.discipline,
            targets: self.target.unwrap_or_default(),
            criteria: self.criterion.unwrap_or_default(),
            assignee: self.assignee,
            reviewer: self.reviewer,
            mandate: self.mandate,
            estimate: self.estimate,
            due: self.due,
            parent: self.parent,
            corrects: self.corrects,
            actor: self.actor,
            now,
        }
    }
}

/// The body of `POST …/cards/<id>/move`: `move_card`'s keys but the path's
/// `card_id`, and `as`. Any other key is refused.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CardMoveBody {
    pub to: Option<String>,
    pub before: Option<String>,
    /// Why it moves; required into the flow's `return` column.
    pub note: Option<String>,
    pub revision: Option<String>,
    /// The acting name; `null`: `local`.
    #[serde(rename = "as")]
    pub actor: Option<String>,
}

impl CardMoveBody {
    /// `bytes` decoded; else exit 2 naming the defect, nothing read or
    /// changed.
    pub fn from_json(bytes: &[u8]) -> Result<Self, CliError> {
        let body: Self = decoded(bytes, "\"to\",\"before\",\"note\",\"revision\",\"as\"")?;
        persons_named(body.actor.as_deref(), &[])?;
        Ok(body)
    }

    /// The request for `id` at `now`.
    pub fn request(self, id: String, now: String) -> CardMoveRequest {
        CardMoveRequest {
            id,
            to: self.to,
            before: self.before,
            note: self.note,
            revision: self.revision,
            actor: self.actor,
            now,
        }
    }
}

/// The body of `POST …/cards/<id>/edit`: `edit_card`'s keys but the path's
/// `card_id`, and `as`; `-` clears, as the flags. Any other key is refused.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CardEditBody {
    pub title: Option<String>,
    pub goal: Option<String>,
    pub target: Option<Vec<String>>,
    pub criterion: Option<Vec<String>>,
    pub estimate: Option<String>,
    pub due: Option<String>,
    pub parent: Option<String>,
    pub corrects: Option<String>,
    /// Each `KIND=VALUE`; appended.
    pub evidence: Option<Vec<String>>,
    /// The `T-` tasks it links, replacing the list; `["-"]` clears it.
    pub task: Option<Vec<String>>,
    /// `binding` or `open`, only from the card's reviewer; `null` or
    /// absent: unchanged.
    pub mandate: Option<Mandate>,
    /// Why an estimate the card holds changes.
    pub reason: Option<String>,
    pub revision: Option<String>,
    /// The acting name; `null`: `local`.
    #[serde(rename = "as")]
    pub actor: Option<String>,
}

impl CardEditBody {
    /// `bytes` decoded; else exit 2 naming the defect, nothing read or
    /// changed.
    pub fn from_json(bytes: &[u8]) -> Result<Self, CliError> {
        let body: Self = decoded(
            bytes,
            "\"title\",\"goal\",\"target\",\"criterion\",\"estimate\",\"due\",\"parent\",\
             \"corrects\",\"evidence\",\"task\",\"mandate\",\"reason\",\"revision\",\"as\"",
        )?;
        persons_named(body.actor.as_deref(), &[])?;
        Ok(body)
    }

    /// The request for `id` at `now`; `tasks_here` as
    /// [`CardEditRequest::tasks_here`].
    pub fn request(self, id: String, now: String, tasks_here: bool) -> CardEditRequest {
        CardEditRequest {
            id,
            title: self.title,
            goal: self.goal,
            targets: self.target.unwrap_or_default(),
            criteria: self.criterion.unwrap_or_default(),
            estimate: self.estimate,
            due: self.due,
            parent: self.parent,
            corrects: self.corrects,
            evidence: self.evidence.unwrap_or_default(),
            tasks: self.task.unwrap_or_default(),
            mandate: self.mandate,
            reason: self.reason,
            revision: self.revision,
            actor: self.actor,
            now,
            tasks_here,
        }
    }
}

/// The body of `POST …/cards/<id>/assign`; `-` clears, as the flags. Any
/// other key is refused.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CardAssignBody {
    pub assignee: Option<String>,
    pub reviewer: Option<String>,
    pub revision: Option<String>,
    /// The acting name; `null`: `local`.
    #[serde(rename = "as")]
    pub actor: Option<String>,
}

impl CardAssignBody {
    /// `bytes` decoded; else exit 2 naming the defect, nothing read or
    /// changed.
    pub fn from_json(bytes: &[u8]) -> Result<Self, CliError> {
        let body: Self = decoded(bytes, "\"assignee\",\"reviewer\",\"revision\",\"as\"")?;
        persons_named(
            body.actor.as_deref(),
            &[
                ("assignee", body.assignee.as_deref()),
                ("reviewer", body.reviewer.as_deref()),
            ],
        )?;
        Ok(body)
    }

    /// The request for `id` at `now`.
    pub fn request(self, id: String, now: String) -> CardAssignRequest {
        CardAssignRequest {
            id,
            assignee: self.assignee,
            reviewer: self.reviewer,
            revision: self.revision,
            actor: self.actor,
            now,
        }
    }
}

/// The body of `POST …/cards/<id>/cancel`. Any other key, or no `reason`, is
/// refused.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CardCancelBody {
    pub reason: String,
    pub revision: Option<String>,
    /// The acting name; `null`: `local`.
    #[serde(rename = "as")]
    pub actor: Option<String>,
}

impl CardCancelBody {
    /// `bytes` decoded; else exit 2 naming the defect, nothing read or
    /// changed.
    pub fn from_json(bytes: &[u8]) -> Result<Self, CliError> {
        let body: Self = decoded(bytes, "\"reason\",\"revision\",\"as\"")?;
        persons_named(body.actor.as_deref(), &[])?;
        Ok(body)
    }

    /// The request for `id` at `now`.
    pub fn request(self, id: String, now: String) -> CardCancelRequest {
        CardCancelRequest {
            id,
            reason: self.reason,
            revision: self.revision,
            actor: self.actor,
            now,
        }
    }
}

/// The body of `POST …/cards/<id>/accept`. Any other key is refused.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CardAcceptBody {
    /// The acting name; `null`: `local`, who can accept nothing.
    #[serde(rename = "as")]
    pub actor: Option<String>,
    pub note: Option<String>,
    pub revision: Option<String>,
    /// As briefed; else `departs` and its three texts.
    pub briefed: Option<bool>,
    /// The direct targets the result departs from.
    pub departs: Option<Vec<String>>,
    pub summary: Option<String>,
    pub observed: Option<String>,
    pub documented: Option<String>,
    pub gap_type: Option<String>,
}

impl CardAcceptBody {
    /// `bytes` decoded; else exit 2 naming the defect, nothing read or
    /// changed.
    pub fn from_json(bytes: &[u8]) -> Result<Self, CliError> {
        let body: Self = decoded(
            bytes,
            "\"as\",\"note\",\"revision\",\"briefed\",\"departs\",\"summary\",\"observed\",\
             \"documented\",\"gap_type\"",
        )?;
        persons_named(body.actor.as_deref(), &[])?;
        Ok(body)
    }

    /// The request for `id` at `now`.
    pub fn request(self, id: String, now: String) -> CardAcceptRequest {
        CardAcceptRequest {
            id,
            actor: self.actor.unwrap_or_else(|| LOCAL_ACTOR.to_owned()),
            note: self.note,
            revision: self.revision,
            now,
            briefed: self.briefed.unwrap_or(false),
            departs: self.departs.unwrap_or_default(),
            summary: self.summary,
            observed: self.observed,
            documented: self.documented,
            gap_type: self.gap_type,
            login: None,
        }
    }
}

/// The body of `POST …/cards/<id>/clear`. Any other key, or no `outcome` or
/// `reason`, is refused.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CardClearBody {
    pub outcome: CardClearOutcome,
    pub reason: String,
    pub estimate: Option<String>,
    pub title: Option<String>,
    pub revision: Option<String>,
    /// The acting name; `null`: `local`, who can clear nothing.
    #[serde(rename = "as")]
    pub actor: Option<String>,
}

impl CardClearBody {
    /// `bytes` decoded; else exit 2 naming the defect, nothing read or
    /// changed.
    pub fn from_json(bytes: &[u8]) -> Result<Self, CliError> {
        let body: Self = decoded(
            bytes,
            "\"outcome\",\"reason\",\"estimate\",\"title\",\"revision\",\"as\"",
        )?;
        persons_named(body.actor.as_deref(), &[])?;
        Ok(body)
    }

    /// The request for `id` at `now`.
    pub fn request(self, id: String, now: String) -> CardClearRequest {
        CardClearRequest {
            id,
            outcome: self.outcome,
            reason: self.reason,
            estimate: self.estimate,
            title: self.title,
            revision: self.revision,
            actor: self.actor,
            now,
        }
    }
}

/// The body of `POST …/trace/clear`. Any other key, or no `group` or
/// `reason`, is refused.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CardClearGroupBody {
    pub group: String,
    pub reason: String,
    /// The acting name; `null`: `local`, who can clear nothing.
    #[serde(rename = "as")]
    pub actor: Option<String>,
    /// The number of cards the page showed, sent on a hub alone.
    pub count: Option<u64>,
}

impl CardClearGroupBody {
    /// `bytes` decoded; else exit 2 naming the defect, nothing read or
    /// changed.
    pub fn from_json(bytes: &[u8]) -> Result<Self, CliError> {
        let body: Self = decoded(bytes, "\"group\",\"reason\",\"as\",\"count\"")?;
        persons_named(body.actor.as_deref(), &[])?;
        Ok(body)
    }

    /// The request at `now`.
    pub fn request(self, now: String) -> CardClearGroupRequest {
        CardClearGroupRequest {
            group: self.group,
            reason: self.reason,
            actor: self.actor,
            now,
            count: self.count,
            entry: None,
        }
    }
}

/// What a card raises from the page: a question, or a decision taken
/// (task spec `card-questions` L3); no other kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardRaiseKind {
    Question,
    Decision,
}

/// The body of `POST …/cards/<id>/items`: a question's or a decision's
/// fields, its optional `why`, and `as`; no `proposed_patch`, `task_id` or
/// author field, since a person raises it. Any other key, or a required one
/// missing, is refused.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CardRaiseBody {
    /// `question` or `decision`.
    pub kind: String,
    pub node_ids: Vec<String>,
    /// The question, or what had to be decided.
    pub text: String,
    /// The answer worked on, or the decision taken.
    pub working_answer: String,
    pub price_of_other: String,
    /// A decision's why; `null`: none.
    pub why: Option<String>,
    /// Absent or `null`: `normal`.
    pub severity: Option<IntakeSeverity>,
    pub distinct_from: Option<Vec<String>>,
    /// The acting name; `null`: `local`.
    #[serde(rename = "as")]
    pub actor: Option<String>,
}

impl CardRaiseBody {
    /// `bytes` decoded, its kind one a card raises and `as` a card person
    /// (trimmed); else exit 2 naming the defect, nothing read or changed.
    pub fn from_json(bytes: &[u8]) -> Result<Self, CliError> {
        let body: Self = decoded(
            bytes,
            "\"kind\",\"node_ids\",\"text\",\"working_answer\",\"price_of_other\",\"why\",\
             \"severity\",\"distinct_from\",\"as\"",
        )?;
        let kind = body.raise_kind()?;
        if kind == CardRaiseKind::Question && body.why.is_some() {
            return Err(CliError::spec("`why` goes with a `decision`"));
        }
        persons_named(body.actor.as_deref(), &[])?;
        // Trimmed as a stage's `as` is, before a hub binds it to its login.
        let body = Self {
            actor: body.actor.map(|actor| card_person(&actor).to_owned()),
            ..body
        };
        Ok(body)
    }

    /// Its `kind`, one a card raises; else exit 2.
    fn raise_kind(&self) -> Result<CardRaiseKind, CliError> {
        match self.kind.as_str() {
            QUESTION_KIND => Ok(CardRaiseKind::Question),
            DECISION_KIND => Ok(CardRaiseKind::Decision),
            _ => Err(escaped_error(CliError::spec(
                "`kind`: a card raises a `question` or a `decision`",
            ))),
        }
    }

    /// The request for the card `id` (the path's, as written) at `now`;
    /// `login` a hub's, `None` locally.
    pub fn request(
        self,
        id: String,
        now: String,
        git: GitEnv,
        login: Option<String>,
    ) -> Result<CardRaiseRequest, CliError> {
        Ok(CardRaiseRequest {
            id,
            kind: self.raise_kind()?,
            node_ids: self.node_ids,
            text: self.text,
            working_answer: self.working_answer,
            price_of_other: self.price_of_other,
            why: self.why,
            severity: self.severity,
            distinct_from: self.distinct_from.unwrap_or_default(),
            actor: self.actor,
            login,
            now,
            git,
        })
    }
}

/// [`card_raise`]'s options: a question or a decision raised on the card
/// `id` by a person.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardRaiseRequest {
    /// The card as written.
    pub id: String,
    pub kind: CardRaiseKind,
    pub node_ids: Vec<String>,
    pub text: String,
    pub working_answer: String,
    pub price_of_other: String,
    /// A decision's why.
    pub why: Option<String>,
    pub severity: Option<IntakeSeverity>,
    pub distinct_from: Vec<String>,
    /// The acting name; `None`: `local`. Its `proposal.created`'s `as`.
    pub actor: Option<String>,
    /// A hub's login: the stored author's `login`. `None` locally.
    pub login: Option<String>,
    /// The injected clock: `YYYY-MM-DDTHH:MM:SSZ`.
    pub now: String,
    pub git: GitEnv,
}

/// What [`card_raise`] answered: the intake's document, stored or not; or,
/// before any intake rule, why there is no such card (exit 1, `{id,
/// reason}`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardRaiseOutcome {
    /// The intake's answer; `None` when there is no such card.
    pub intake: Option<Box<IntakeOutcome>>,
    /// The card asked for, when there is none (`null` when no card ID was
    /// written).
    pub id: Option<String>,
    /// Why there is none.
    pub reason: Option<String>,
    /// The no-card answer's own: none.
    pub messages: Vec<Message>,
}

impl CardRaiseOutcome {
    /// The intake's exit; 1 when there is no such card.
    pub fn exit(&self) -> Exit {
        match &self.intake {
            Some(intake) => intake.exit(),
            None => Exit::NotFound,
        }
    }

    /// No such card: the card's read refused it before any intake rule.
    pub fn no_card(&self) -> bool {
        self.intake.is_none()
    }

    fn missing(id: Option<String>, reason: String) -> Self {
        Self {
            intake: None,
            id,
            reason: Some(reason),
            messages: Vec::new(),
        }
    }
}

impl Serialize for CardRaiseOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match &self.intake {
            Some(intake) => intake.serialize(serializer),
            None => NoCardJson {
                id: self.id.as_deref(),
                reason: self.reason.as_deref(),
            }
            .serialize(serializer),
        }
    }
}

/// What a card-changing command did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CardAction {
    New,
    Move,
    Edit,
    Assign,
    Cancel,
    Accept,
    Clear,
}

impl CardAction {
    /// The past tense its line opens with.
    const fn verb(&self) -> &'static str {
        match self {
            Self::New => "created",
            Self::Move => "moved",
            Self::Edit => "edited",
            Self::Assign => "assigned",
            Self::Cancel => "cancelled",
            Self::Accept => "accepted",
            Self::Clear => "cleared",
        }
    }
}

/// Why a command was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardCause {
    /// The request asks nothing: exit 2, no card read. The document is `{id,
    /// reason}`; the daemon answers a 400 with the line, and the terminal
    /// prints the line alone ([`CardOutcome::or_usage_error`]).
    Usage,
    /// There is no such card: the document is `{id, reason}`.
    Unknown,
    /// A rule, a cap, a stale revision, a declined prompt.
    Refused,
    /// A hub's `[team]` entry does not cover what the act re-freezes: the
    /// daemon's 403.
    Outside,
}

/// What a card-changing command answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardOutcome {
    pub action: CardAction,
    /// The card's ID; `null` when what was given is none, or none was made.
    pub id: Option<String>,
    /// The card after the command (as read, when refused).
    pub card: Option<Box<Card>>,
    /// What it may do next.
    pub allowed: Vec<AllowedMove>,
    /// What a reader should know; a refusal's reason last.
    pub notes: Vec<String>,
    /// Why the command was refused (exit 1).
    pub refusal: Option<String>,
    pub cause: Option<CardCause>,
    pub messages: Vec<Message>,
}

impl CardOutcome {
    /// 0 done; 2 a usage defect; 1 unknown or refused.
    pub fn exit(&self) -> Exit {
        match self.cause {
            None => Exit::Answered,
            Some(CardCause::Usage) => Exit::CannotRun,
            Some(CardCause::Unknown | CardCause::Refused | CardCause::Outside) => Exit::NotFound,
        }
    }

    /// A usage defect as the terminal gives it, a [`CliError`]: its one line
    /// on stderr, stdout empty, `--json` too. Any other outcome as it is.
    pub fn or_usage_error(self) -> Result<Self, CliError> {
        match (self.cause, &self.refusal) {
            (Some(CardCause::Usage), Some(reason)) => Err(CliError::spec(reason)),
            _ => Ok(self),
        }
    }

    fn done(action: CardAction, card: Card, config: &CardConfig, messages: Vec<Message>) -> Self {
        let allowed = allowed_moves(config.flow(&card.flow), &card);
        Self {
            action,
            id: Some(card.id.clone()),
            card: Some(Box::new(card)),
            allowed,
            notes: notes(&messages),
            refusal: None,
            cause: None,
            messages,
        }
    }

    fn refused(
        action: CardAction,
        id: Option<&str>,
        card: Option<(&Card, &CardConfig)>,
        reason: &str,
        messages: Vec<Message>,
    ) -> Self {
        let reason = one_line(reason);
        let mut listed = notes(&messages);
        listed.push(reason.clone());
        Self {
            action,
            id: id.map(str::to_owned),
            card: card.map(|(card, _)| Box::new(card.clone())),
            allowed: card
                .map(|(card, config)| allowed_moves(config.flow(&card.flow), card))
                .unwrap_or_default(),
            notes: listed,
            refusal: Some(reason),
            cause: Some(CardCause::Refused),
            messages,
        }
    }

    fn unknown(action: CardAction, id: Option<&str>, reason: String) -> Self {
        Self {
            cause: Some(CardCause::Unknown),
            ..Self::refused(action, id, None, &reason, Vec::new())
        }
    }

    /// `line`, without the `spec: ` of the stderr line: what the request
    /// did wrong.
    fn usage(action: CardAction, line: &str) -> Self {
        Self {
            cause: Some(CardCause::Usage),
            ..Self::refused(action, None, None, line, Vec::new())
        }
    }
}

#[derive(Serialize)]
struct CardWriteJson<'a> {
    id: Option<&'a str>,
    flow: Option<&'a str>,
    column: Option<&'a str>,
    position: Option<u64>,
    revision: Option<String>,
    allowed: &'a [AllowedMove],
    notes: &'a [String],
}

#[derive(Serialize)]
struct NoCardJson<'a> {
    id: Option<&'a str>,
    reason: Option<&'a str>,
}

/// `{id, flow, column, position, revision, allowed, notes}`, else exactly
/// `{id, reason}`: no such card, or a usage defect.
impl Serialize for CardOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if matches!(self.cause, Some(CardCause::Unknown | CardCause::Usage)) {
            return NoCardJson {
                id: self.id.as_deref(),
                reason: self.refusal.as_deref(),
            }
            .serialize(serializer);
        }
        let card = self.card.as_deref();
        CardWriteJson {
            id: self.id.as_deref(),
            flow: card.map(|card| card.flow.as_str()),
            column: card.map(|card| card.column.as_str()),
            position: card.map(|card| card.position),
            revision: card.map(|card| card.revision.to_string()),
            allowed: &self.allowed,
            notes: &self.notes,
        }
        .serialize(serializer)
    }
}

/// What `spec card clear --group` answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardGroupOutcome {
    /// `REF` as given.
    pub group: String,
    /// The cards cleared, by number.
    pub cards: Vec<Card>,
    /// What a reader should know; a refusal's reason last.
    pub notes: Vec<String>,
    /// Why the command was refused (exit 1).
    pub refusal: Option<String>,
    pub cause: Option<CardCause>,
    pub messages: Vec<Message>,
}

impl CardGroupOutcome {
    /// 0 done; 1 unknown or refused.
    pub fn exit(&self) -> Exit {
        match self.cause {
            None => Exit::Answered,
            Some(CardCause::Usage) => Exit::CannotRun,
            Some(CardCause::Unknown | CardCause::Refused | CardCause::Outside) => Exit::NotFound,
        }
    }

    fn refused(group: &str, cause: CardCause, reason: &str, messages: Vec<Message>) -> Self {
        let reason = one_line(reason);
        let mut listed = notes(&messages);
        listed.push(reason.clone());
        Self {
            group: group.to_owned(),
            cards: Vec::new(),
            notes: listed,
            refusal: Some(reason),
            cause: Some(cause),
            messages,
        }
    }
}

#[derive(Serialize)]
struct CardGroupJson<'a> {
    group: &'a str,
    cleared: Vec<&'a str>,
    notes: &'a [String],
}

/// `{group, cleared, notes}`, a refusal's reason the last of `notes`.
impl Serialize for CardGroupOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        CardGroupJson {
            group: &self.group,
            cleared: self.cards.iter().map(|card| card.id.as_str()).collect(),
            notes: &self.notes,
        }
        .serialize(serializer)
    }
}

/// One question, discrepancy or decision a card names, as its read serves
/// it (task spec `card-questions` K1): the row's values, `author` its
/// object, read now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CardItem {
    pub id: String,
    pub kind: &'static str,
    pub status: &'static str,
    /// A question's or a decision's text, a discrepancy's summary.
    pub summary: String,
    /// A question's gist and why it matters, when given; `null` on every
    /// other kind (task spec `plain-questions` PQ10).
    pub gist: Option<String>,
    pub matters: Option<String>,
    pub working_answer: Option<String>,
    /// A decision's why.
    pub why: Option<String>,
    pub author: Author,
    pub created_at: String,
    pub updated_at: String,
    /// When its open stage was staged; `null` when none.
    pub staged_at: Option<String>,
    pub decided_by: Option<String>,
    pub decided_at: Option<String>,
    /// An approval's note, a rejection's reason.
    pub decision_note: Option<String>,
    pub record_id: Option<String>,
    pub record_path: Option<String>,
    pub record_title: Option<String>,
}

impl CardItem {
    /// `proposal`'s keys.
    fn of(proposal: &Proposal) -> Self {
        let intake = proposal.intake.as_ref();
        let record = proposal.record.as_ref();
        Self {
            id: proposal.id.clone(),
            kind: proposal.kind.as_str(),
            status: proposal.status.as_str(),
            summary: intake
                .map(|intake| intake.summary.clone())
                .unwrap_or_default(),
            gist: intake.and_then(|intake| intake.gist.clone()),
            matters: intake.and_then(|intake| intake.matters.clone()),
            working_answer: intake.and_then(|intake| intake.working_answer.clone()),
            why: intake.and_then(|intake| intake.why.clone()),
            author: proposal.author.clone(),
            created_at: proposal.created_at.clone(),
            updated_at: proposal.updated_at.clone(),
            staged_at: proposal.staged.as_ref().map(|staged| staged.at.clone()),
            decided_by: proposal.decided_by.clone(),
            decided_at: proposal.decided_at.clone(),
            decision_note: proposal.decision_note.clone(),
            record_id: record.map(|record| record.id.clone()),
            record_path: record.map(|record| record.path.clone()),
            record_title: record.map(|record| record.title.clone()),
        }
    }
}

/// One question, discrepancy or decision naming one of a card's subtasks
/// (task spec `task-mandate` M6): the child's ID first, then [`CardItem`]'s
/// keys.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CardSubtaskItem {
    pub card_id: String,
    #[serde(flatten)]
    pub item: CardItem,
}

impl CardSubtaskItem {
    /// `proposal`'s keys, its `card_id` the child it names.
    fn of(proposal: &Proposal) -> Self {
        Self {
            card_id: proposal.card_id.clone().unwrap_or_default(),
            item: CardItem::of(proposal),
        }
    }
}

/// What `spec card show` answered: the card, or why there is none.
#[derive(Debug, Clone, PartialEq)]
pub struct CardShowOutcome {
    pub card: Option<Box<Card>>,
    pub allowed: Vec<AllowedMove>,
    /// `--history`'s events, oldest first; `null` without it.
    pub history: Option<Vec<Event>>,
    /// `--marks`' mark; `None` without it and on a cancelled card, which a
    /// read ignores.
    pub mark: Option<CardMark>,
    /// What changed above it; `None` unasked, with no parent, on a closed
    /// card, and when nothing is said (task spec `upstream-signal` US-8).
    pub upstream: Option<Upstream>,
    /// Each task the card links, in link order, read now; a task's notes,
    /// each starting with its ID, follow the card's own in `notes`.
    pub tasks: Vec<LinkedTask>,
    /// The questions, discrepancies and decisions naming it, by ID number,
    /// read now; a row that does not read is left out, its note after the
    /// tasks' (task spec `card-questions` K1).
    pub items: Vec<CardItem>,
    /// The items naming one of its subtasks (`parent` exactly this card),
    /// by ID number, in one read; a row that does not read is left out, its
    /// note after the items'.
    pub subtask_items: Vec<CardSubtaskItem>,
    /// `--context`'s; `None` without it, and when there is no card.
    pub context: Option<CardContext>,
    pub notes: Vec<String>,
    /// The card asked for, when there is none.
    pub id: Option<String>,
    /// Why there is none (exit 1).
    pub reason: Option<String>,
    pub messages: Vec<Message>,
}

/// A card's context (task spec `card-context` CC1, CC5): its direct
/// parent's goal, and one bundle of its targets then its own applied
/// records, fitted into the room its lines leave. Every part is present
/// when asked; a bundle that cannot be made is `None` with a note, never
/// the read's failure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CardContext {
    /// The direct parent, read now; `None` without one, or unread (a note).
    pub parent: Option<CardContextParent>,
    /// `spec bundle`'s outcome, made: `bundle` set, `reason` `None`.
    pub bundle: Option<BundleOutcome>,
    /// Each printed as `note: context: <note>`.
    pub notes: Vec<String>,
}

/// A card's direct parent, as its context reads it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CardContextParent {
    pub id: String,
    pub title: String,
    /// At most [`GOAL_MAX`] bytes: a longer one (a hand edit) is cut on a
    /// character boundary, with a note.
    pub goal: Option<String>,
}

impl CardShowOutcome {
    /// Exit 0, or 1 when there is no such card.
    pub fn exit(&self) -> Exit {
        if self.reason.is_some() {
            Exit::NotFound
        } else {
            Exit::Answered
        }
    }
}

#[derive(Serialize)]
struct EventJson<'a> {
    seq: i64,
    #[serde(rename = "type")]
    event_type: &'a str,
    at: &'a str,
    payload: &'a serde_json::Value,
}

/// The columns in order, less `project`, the four keys of `--marks` and
/// `upstream` right after `targets`, so no cut of a long card's text
/// reaches them, then `allowed`, `history`, `context`, `notes`.
#[derive(Serialize)]
struct CardJson<'a> {
    id: &'a str,
    flow: &'a str,
    column: &'a str,
    position: u64,
    title: &'a str,
    goal: Option<&'a str>,
    discipline: Option<&'a str>,
    targets: &'a [String],
    mark: Option<&'static str>,
    on: Option<&'a str>,
    discrepancy: Option<&'a str>,
    correction: Option<&'a str>,
    upstream: Option<&'a Upstream>,
    snapshot: Option<&'a [CardFrozen]>,
    criteria: &'a [String],
    assignee: Option<&'a str>,
    reviewer: Option<&'a str>,
    mandate: Option<&'static str>,
    estimate: Option<&'a str>,
    due: Option<&'a str>,
    parent: Option<&'a str>,
    corrects: Option<&'a str>,
    evidence: &'a [CardEvidence],
    tasks: &'a [LinkedTask],
    items: &'a [CardItem],
    subtask_items: &'a [CardSubtaskItem],
    acceptance: Option<&'a CardAcceptance>,
    cancellation: Option<&'a CardCancellation>,
    created_by: &'a str,
    created_at: &'a str,
    updated_at: &'a str,
    revision: String,
    allowed: &'a [AllowedMove],
    history: Option<Vec<EventJson<'a>>>,
    context: Option<&'a CardContext>,
    notes: &'a [String],
}

impl Serialize for CardShowOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let Some(card) = self.card.as_deref() else {
            return NoCardJson {
                id: self.id.as_deref(),
                reason: self.reason.as_deref(),
            }
            .serialize(serializer);
        };
        let mark = self.mark.as_ref();
        CardJson {
            id: &card.id,
            flow: &card.flow,
            column: &card.column,
            position: card.position,
            title: &card.title,
            goal: card.goal.as_deref(),
            discipline: card.discipline.as_deref(),
            targets: &card.targets,
            mark: mark.map(CardMark::word),
            on: mark.and_then(CardMark::on),
            discrepancy: mark.and_then(CardMark::discrepancy),
            correction: mark.and_then(CardMark::correction),
            upstream: self.upstream.as_ref(),
            snapshot: card.snapshot.as_deref(),
            criteria: &card.criteria,
            assignee: card.assignee.as_deref(),
            reviewer: card.reviewer.as_deref(),
            mandate: card.mandate.map(Mandate::as_str),
            estimate: card.estimate.as_deref(),
            due: card.due.as_deref(),
            parent: card.parent.as_deref(),
            corrects: card.corrects.as_deref(),
            evidence: &card.evidence,
            tasks: &self.tasks,
            items: &self.items,
            subtask_items: &self.subtask_items,
            acceptance: card.acceptance.as_ref(),
            cancellation: card.cancellation.as_ref(),
            created_by: &card.created_by,
            created_at: &card.created_at,
            updated_at: &card.updated_at,
            revision: card.revision.to_string(),
            allowed: &self.allowed,
            history: self.history.as_ref().map(|events| {
                events
                    .iter()
                    .map(|event| EventJson {
                        seq: event.seq,
                        event_type: &event.event_type,
                        at: &event.at,
                        payload: &event.payload,
                    })
                    .collect()
            }),
            context: self.context.as_ref(),
            notes: &self.notes,
        }
        .serialize(serializer)
    }
}

/// What `spec card list` answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardListOutcome {
    pub flows: Vec<CardFlow>,
    /// `flow <name> | <column> <n> | ...`, one per flow: the cards listed
    /// standing in each column, before the cut.
    pub flow_lines: Vec<String>,
    /// Every card listed, in list order.
    pub cards: Vec<Card>,
    /// `--marks`: by card ID; a cancelled card, which a read ignores, none.
    pub marks: BTreeMap<String, CardMark>,
    /// `--marks`: the open subtasks something changed above, by ID (task
    /// spec `upstream-signal` US-8).
    pub upstream: BTreeSet<String>,
    /// `--marks` was asked: each row's `upstream` is a flag, else `null`.
    pub marked: bool,
    /// How many of `cards` are printed (text and JSON alike).
    pub shown: usize,
    pub notes: Vec<String>,
    pub messages: Vec<Message>,
}

impl CardListOutcome {
    /// The list as `view` bounds it: [`View::Browser`] prints every card
    /// (`truncated` false, `omitted` null, the same keys); else as
    /// [`card_list`] read it.
    #[must_use]
    pub fn in_view(mut self, view: View) -> Self {
        if view == View::Browser {
            self.shown = self.cards.len();
        }
        self
    }

    fn omitted(&self) -> usize {
        self.cards.len().saturating_sub(self.shown)
    }

    fn shown_cards(&self) -> &[Card] {
        &self.cards[..self.shown.min(self.cards.len())]
    }
}

/// A card as `list` gives it.
#[derive(Serialize)]
struct CardRowJson<'a> {
    id: &'a str,
    flow: &'a str,
    column: &'a str,
    position: u64,
    title: &'a str,
    discipline: Option<&'a str>,
    targets: &'a [String],
    assignee: Option<&'a str>,
    reviewer: Option<&'a str>,
    mandate: Option<&'static str>,
    estimate: Option<&'a str>,
    due: Option<&'a str>,
    parent: Option<&'a str>,
    corrects: Option<&'a str>,
    updated_at: &'a str,
    revision: String,
    accepted: bool,
    cancelled: bool,
    mark: Option<&'static str>,
    on: Option<&'a str>,
    discrepancy: Option<&'a str>,
    correction: Option<&'a str>,
    upstream: Option<bool>,
}

#[derive(Serialize)]
struct CardListJson<'a> {
    flows: &'a [CardFlow],
    cards: Vec<CardRowJson<'a>>,
    notes: &'a [String],
    truncated: bool,
    omitted: Option<usize>,
}

/// `{flows, cards, notes, truncated, omitted}`.
impl Serialize for CardListOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let omitted = self.omitted();
        CardListJson {
            flows: &self.flows,
            cards: self
                .shown_cards()
                .iter()
                .map(|card| {
                    let mark = self.marks.get(&card.id);
                    CardRowJson {
                        id: &card.id,
                        flow: &card.flow,
                        column: &card.column,
                        position: card.position,
                        title: &card.title,
                        discipline: card.discipline.as_deref(),
                        targets: &card.targets,
                        assignee: card.assignee.as_deref(),
                        reviewer: card.reviewer.as_deref(),
                        mandate: card.mandate.map(Mandate::as_str),
                        estimate: card.estimate.as_deref(),
                        due: card.due.as_deref(),
                        parent: card.parent.as_deref(),
                        corrects: card.corrects.as_deref(),
                        updated_at: &card.updated_at,
                        revision: card.revision.to_string(),
                        accepted: card.acceptance.is_some(),
                        cancelled: card.cancellation.is_some(),
                        mark: mark.map(CardMark::word),
                        on: mark.and_then(CardMark::on),
                        discrepancy: mark.and_then(CardMark::discrepancy),
                        correction: mark.and_then(CardMark::correction),
                        upstream: self.marked.then(|| self.upstream.contains(&card.id)),
                    }
                })
                .collect(),
            notes: &self.notes,
            truncated: omitted > 0,
            omitted: (omitted > 0).then_some(omitted),
        }
        .serialize(serializer)
    }
}

// ------------------------------------------------------------- context

/// Where `spec card` runs: the project, its flows and its queue.
struct OpenCards {
    project: ProjectRoot,
    slug: String,
    /// The queue's directory.
    data_dir: PathBuf,
    config: CardConfig,
    /// The root's `specengine.toml` as read: a card's context reads its
    /// `[budgets] bundle_task` alone, only when asked.
    config_text: String,
    queue: SqliteQueue,
}

/// The queue's context for a card command, git never asked: `--config` but
/// the root's own, `C` taken by `[ids]` and a bad `[card]` exit 2.
fn open_cards(env: &Env, globals: &Globals) -> Result<OpenCards, CliError> {
    let (project, text) = discover_with_text(env, globals)?;
    require_root_config(env, globals, &project)?;
    let slug = project.slug()?.to_owned();
    if let Some(clash) = card_prefix_clash(&project.config.scheme) {
        return Err(CliError::spec(format!("{}: {clash}", project.config_label)));
    }
    let config = CardConfig::from_toml(&text)
        .map_err(|error| config_error(&project.config_label, &error))?;
    let data_dir = prepared_data_dir(env, &project)?;
    let queue =
        SqliteQueue::open(data_dir.join(format!("{slug}.db")), &slug).map_err(queue_cannot)?;
    Ok(OpenCards {
        project,
        slug,
        data_dir,
        config,
        config_text: text,
        queue,
    })
}

/// The card ID as written: `Some` when it is one, `None` when it is none in
/// any script (exit 1); a look-alike exits 2 naming the Latin form.
fn written_card(written: &str) -> Result<Option<String>, CliError> {
    match parse_card_id(written) {
        Ok(id) => Ok(Some(id)),
        Err(CardIdError::LookAlike { fix }) => Err(CliError::spec(card_look_alike_message(
            written.trim(),
            &fix,
        ))),
        Err(CardIdError::NotAnId) => Ok(None),
    }
}

/// `no card C-0099`.
fn no_card(id: &str) -> String {
    format!("no card {id}")
}

/// The line for what is no card ID: exit 1 where a command names its card
/// or a linked one (`--parent`, `--corrects`, `--before`), exit 2 for `list
/// --parent`.
pub(crate) fn not_a_card(written: &str) -> String {
    format!(
        "no card `{}`: a card ID is `C-` and 4 or more digits, as `spec card list` lists it",
        written.trim()
    )
}

/// `--as N`, trimmed: `local` when absent; off a card person's grammar exit
/// 2.
fn actor_of(written: Option<&str>) -> Result<String, CliError> {
    let actor = written.unwrap_or(LOCAL_ACTOR);
    match card_person_problem(actor) {
        Some(problem) => Err(CliError::spec(format!("--as: {problem}"))),
        None => Ok(card_person(actor).to_owned()),
    }
}

/// `--revision R`, a compare-and-set key: exit 2 for what no revision can be.
fn revision_of(written: Option<&str>) -> Result<Option<u64>, CliError> {
    let Some(written) = written else {
        return Ok(None);
    };
    written
        .parse::<u64>()
        .ok()
        .filter(|revision| *revision >= 1 && revision.to_string() == written)
        .map(Some)
        .ok_or_else(|| {
            CliError::spec(format!(
                "--revision: `{written}` is no revision (1, 2, ...)"
            ))
        })
}

/// A card a field names: the ID, a look-alike exiting 2; `Ok(Err)` the
/// refusal when it is no card ID.
fn linked_card(field: &str, written: &str) -> Result<Result<String, String>, CliError> {
    Ok(match written_card(written)? {
        Some(id) => Ok(id),
        None => Err(format!("{field}: {}", not_a_card(written))),
    })
}

/// A field as `edit` and `assign` take it: absent, cleared (`-`), or a value.
fn field(written: Option<&str>) -> Option<Option<String>> {
    written.map(|value| (value != CLEAR).then(|| value.to_owned()))
}

/// A person as `assign` takes one: [`field`]'s, trimmed.
fn person_field(written: Option<&str>) -> Option<Option<String>> {
    field(written.map(card_person))
}

/// The first problem of the fields `new` and `edit` share.
fn field_problem(
    title: Option<&str>,
    goal: Option<&str>,
    targets: usize,
    criteria: &[String],
    estimate: Option<&str>,
    due: Option<&str>,
) -> Option<String> {
    title
        .filter(|title| title.trim().is_empty())
        .map(|_| "title: blank; a card has a title".to_owned())
        .or_else(|| title.and_then(|title| over_bytes("title", title, TITLE_MAX)))
        .or_else(|| goal.and_then(|goal| over_bytes("goal", goal, GOAL_MAX)))
        .or_else(|| over_items("targets", targets, NODES_MAX))
        .or_else(|| over_items("criteria", criteria.len(), CRITERIA_MAX))
        .or_else(|| {
            criteria
                .iter()
                .enumerate()
                .find_map(|(at, text)| over_bytes(&format!("criteria[{at}]"), text, CRITERION_MAX))
        })
        .or_else(|| estimate.and_then(estimate_problem))
        .or_else(|| due.and_then(due_problem))
}

/// Why `reason` is no reason a card records: blank, or over its cap.
fn reason_problem(reason: &str, why: &str) -> Option<String> {
    reason
        .trim()
        .is_empty()
        .then(|| format!("reason: blank; say {why}"))
        .or_else(|| over_bytes("reason", reason, NOTE_MAX))
}

/// Why `name` cannot be a person a card names.
fn person_problem(key: &str, name: &str) -> Option<String> {
    card_person_problem(name).map(|problem| format!("{key}: {problem}"))
}

/// The card a command names: `Ok(Err)` the refused outcome (exit 1).
fn named_card(
    context: &OpenCards,
    written: &str,
    id: Option<String>,
    action: CardAction,
) -> Result<Result<Card, Box<CardOutcome>>, CliError> {
    let Some(id) = id else {
        return Ok(Err(Box::new(CardOutcome::unknown(
            action,
            None,
            not_a_card(written),
        ))));
    };
    match context.queue.get_card(&id).map_err(queue_cannot)? {
        Some(card) => Ok(Ok(card)),
        None => Ok(Err(Box::new(CardOutcome::unknown(
            action,
            Some(&id),
            no_card(&id),
        )))),
    }
}

/// A store refusal (exit 1) as the outcome, the card as it stands now;
/// anything else exit 2.
fn store_refusal(
    error: QueueError,
    action: CardAction,
    read: &Card,
    context: &OpenCards,
) -> Result<CardOutcome, CliError> {
    let QueueError::CardRefused { reason, .. } = error else {
        return Err(queue_cannot(error));
    };
    let now = context
        .queue
        .get_card(&read.id)
        .map_err(queue_cannot)?
        .unwrap_or_else(|| read.clone());
    Ok(CardOutcome::refused(
        action,
        Some(&now.id),
        Some((&now, &context.config)),
        &reason,
        Vec::new(),
    ))
}

/// One change of `card` through the store, under its lock: the answer.
fn changed(
    context: &mut OpenCards,
    action: CardAction,
    card: &Card,
    seen: Option<u64>,
    change: &CardChange<'_>,
    actor: &str,
    now: &str,
) -> Result<CardOutcome, CliError> {
    match context
        .queue
        .change_card(&card.id, seen, change, actor, now)
    {
        Ok(stored) => Ok(CardOutcome::done(
            action,
            stored,
            &context.config,
            Vec::new(),
        )),
        Err(error) => store_refusal(error, action, card, context),
    }
}

// ---------------------------------------------------------------- new

/// `spec card new`: stores a card, first column of its discipline's flow.
pub fn card_new(
    env: &Env,
    globals: &Globals,
    request: &CardNewRequest,
) -> Result<CardOutcome, CliError> {
    run_new(env, globals, request).map_err(escaped_error)
}

fn run_new(
    env: &Env,
    globals: &Globals,
    request: &CardNewRequest,
) -> Result<CardOutcome, CliError> {
    let refuse = |reason: String, messages: Vec<Message>| {
        Ok(CardOutcome::refused(
            CardAction::New,
            None,
            None,
            &reason,
            messages,
        ))
    };
    let now = checked_now(&request.now)?;
    let actor = actor_of(request.actor.as_deref())?;
    let parent = match request.parent.as_deref() {
        Some(written) => Some(linked_card("parent", written)?),
        None => None,
    };
    let corrects = match request.corrects.as_deref() {
        Some(written) => Some(linked_card("corrects", written)?),
        None => None,
    };
    let mut context = open_cards(env, globals)?;
    let (parent, corrects) = match (parent.transpose(), corrects.transpose()) {
        (Ok(parent), Ok(corrects)) => (parent, corrects),
        (Err(reason), _) | (_, Err(reason)) => return refuse(reason, Vec::new()),
    };
    let assignee = request.assignee.as_deref().map(card_person);
    let reviewer = request.reviewer.as_deref().map(card_person);
    let problem = field_problem(
        Some(&request.title),
        request.goal.as_deref(),
        request.targets.len(),
        &request.criteria,
        request.estimate.as_deref(),
        request.due.as_deref(),
    )
    .or_else(|| {
        request
            .discipline
            .as_deref()
            .and_then(name_problem)
            .map(|problem| format!("discipline: {problem}"))
    })
    .or_else(|| {
        assignee.and_then(|name| {
            (name == CLEAR)
                .then(|| format!("assignee: `{CLEAR}` clears a name: a new card has none"))
                .or_else(|| person_problem("assignee", name))
        })
    })
    .or_else(|| {
        // `assign`'s rule: a person, then core's, so `agent` and `local`
        // are no reviewer.
        reviewer.and_then(|name| {
            (name == CLEAR)
                .then(|| format!("reviewer: `{CLEAR}` clears a name: a new card has none"))
                .or_else(|| person_problem("reviewer", name))
                .or_else(|| reviewer_problem(name).map(|text| format!("reviewer: {text}")))
        })
    })
    // After the reviewer's: a mandate goes with one (canon `card-mandate`,
    // "The rule"); the store asks it again under its lock.
    .or_else(|| mandate_problem(request.mandate, reviewer));
    if let Some(problem) = problem {
        return refuse(problem, Vec::new());
    }
    let flow = match context.config.flow_for(request.discipline.as_deref()) {
        Ok(flow) => flow,
        Err(reason) => return refuse(reason, Vec::new()),
    };
    let (flow, column) = (flow.name.clone(), flow.first_column().to_owned());
    let mut messages = Vec::new();
    let (targets, norms) = match resolved_targets(env, &context, &request.targets, &mut messages)? {
        Ok(resolved) => resolved,
        Err(reason) => return refuse(reason, messages),
    };
    let snapshot = freeze(&targets, &norms);
    let new_card = NewCard {
        flow,
        column,
        title: request.title.clone(),
        goal: request.goal.clone(),
        discipline: request.discipline.clone(),
        targets,
        criteria: request.criteria.clone(),
        assignee: assignee.map(str::to_owned),
        reviewer: reviewer.map(str::to_owned),
        mandate: request.mandate,
        estimate: request.estimate.clone(),
        due: request.due.clone(),
        parent,
        corrects,
        by: actor,
    };
    let made = if new_card.targets.is_empty() {
        context.queue.create_card(&new_card, now)
    } else {
        context.queue.create_card_frozen(&new_card, &snapshot, now)
    };
    match made {
        Ok(card) => Ok(CardOutcome::done(
            CardAction::New,
            card,
            &context.config,
            messages,
        )),
        Err(QueueError::CardRefused { reason, .. }) => refuse(reason, messages),
        Err(error) => Err(queue_cannot(error)),
    }
}

/// The nodes `--target` names, canonical, each once, and what they read now;
/// a manual card reads no index.
fn resolved_targets(
    env: &Env,
    context: &OpenCards,
    written: &[String],
    messages: &mut Vec<Message>,
) -> Result<Result<(Vec<String>, TargetNorms), String>, CliError> {
    if written.is_empty() {
        return Ok(Ok((Vec::new(), TargetNorms::new())));
    }
    let project = &context.project;
    let mut open = open_index(env, project)?;
    let input = indexed_in(&mut open.index, project, messages, false)?;
    let targets = match resolved_list(project, &input, written, "target")? {
        Ok(targets) => targets,
        Err(reason) => return Ok(Err(reason)),
    };
    let graph = SpecGraph::new(&input, &project.config.scheme, &project.config.paths);
    let norms = norms_read(&open.index, &graph, &targets)?;
    Ok(Ok((targets, norms)))
}

// -------------------------------------------------------------- split

/// `spec card split` (task spec `department-split` S3): a new parent on
/// `targets` and `title`, or the stored card `parent`, and one subtask per
/// `children` entry.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CardSplitRequest {
    /// `--target REF…`, a new parent's, resolved as `new` resolves them.
    pub targets: Vec<String>,
    /// `--title T`: required without `parent`, refused with it.
    pub title: Option<String>,
    pub goal: Option<String>,
    /// A new parent's discipline: its flow, as `new` picks one.
    pub discipline: Option<String>,
    /// `--parent C`: a stored card, in any state; only subtasks are made.
    pub parent: Option<String>,
    /// `--child D…`, each a subtask's discipline in this order; `None` (or
    /// empty): none named, which a dry run plans as every discipline.
    pub children: Option<Vec<String>>,
    /// `--mandate D=M…`: a subtask's mandate, `D` one of `children`, each
    /// once; a child no entry names holds none
    /// (canon `card-mandate`, "The rule").
    pub mandate: Vec<String>,
    /// `--reviewer N`; absent, the stored parent's, else the acting name.
    pub reviewer: Option<String>,
    /// `--dry-run`: the plan, nothing written, no person judged.
    pub dry_run: bool,
    /// `--as N`; `None`: `local`.
    pub actor: Option<String>,
    pub now: String,
}

/// The body of `POST …/cards/split`: `split_card`'s keys and `as`, every
/// key optional; the CLI's usage rules judge it after decoding. `child`
/// `null` or `[]` names none, as `mandate` does; `dry_run` absent or `null`
/// is false.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CardSplitBody {
    pub target: Option<Vec<String>>,
    pub title: Option<String>,
    pub goal: Option<String>,
    pub discipline: Option<String>,
    pub parent: Option<String>,
    pub child: Option<Vec<String>>,
    /// Each `D=M`, as `--mandate`.
    pub mandate: Option<Vec<String>>,
    pub reviewer: Option<String>,
    pub dry_run: Option<bool>,
    /// The acting name; `null`: `local`.
    #[serde(rename = "as")]
    pub actor: Option<String>,
}

impl CardSplitBody {
    /// `bytes` decoded, its persons judged as the flags judge them; else
    /// exit 2 naming the defect, nothing read or changed.
    pub fn from_json(bytes: &[u8]) -> Result<Self, CliError> {
        let body: Self = decoded(
            bytes,
            "\"target\",\"title\",\"goal\",\"discipline\",\"parent\",\"child\",\"mandate\",\
             \"reviewer\",\"dry_run\",\"as\"",
        )?;
        persons_named(
            body.actor.as_deref(),
            &[("reviewer", body.reviewer.as_deref())],
        )?;
        Ok(body)
    }

    /// The request at `now`.
    pub fn request(self, now: String) -> CardSplitRequest {
        CardSplitRequest {
            targets: self.target.unwrap_or_default(),
            title: self.title,
            goal: self.goal,
            discipline: self.discipline,
            parent: self.parent,
            children: self.child.filter(|children| !children.is_empty()),
            mandate: self.mandate.unwrap_or_default(),
            reviewer: self.reviewer,
            dry_run: self.dry_run.unwrap_or(false),
            actor: self.actor,
            now,
        }
    }
}

/// A split's parent as its document serves it: made, or as stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CardSplitParent {
    /// `null` in a dry run.
    pub id: Option<String>,
    /// Made by this split; false: a stored card, never written.
    pub created: bool,
    pub title: String,
    pub discipline: Option<String>,
    pub flow: String,
    /// The flow's `title`, else its name.
    pub flow_title: String,
    pub column: String,
    pub targets: Vec<String>,
}

/// One subtask as the split's document serves it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CardSplitChild {
    /// `null` in a dry run.
    pub id: Option<String>,
    pub discipline: String,
    pub flow: String,
    pub flow_title: String,
    pub column: String,
    pub targets: Vec<String>,
    /// `kinds`: the nodes of its rows' kinds; `parent`: the parent's
    /// targets.
    pub from: TargetsFrom,
    /// A `[[card.split]]` row matched in the scope lists its discipline.
    pub suggested: bool,
    /// The `--mandate` naming its discipline; `null` when none does.
    pub mandate: Option<Mandate>,
}

/// What `spec card split` answered: the document and its exit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardSplitOutcome {
    pub dry_run: bool,
    /// `None` when refused.
    pub parent: Option<CardSplitParent>,
    pub children: Vec<CardSplitChild>,
    /// Who accepts each card's result; a dry run's unjudged, `None` when the
    /// chain ends at no name; `None` when refused.
    pub reviewer: Option<String>,
    /// The plan's notes, then a refusal's reason.
    pub notes: Vec<String>,
    /// Why nothing was written (exit 1), or the usage line (exit 2).
    pub refusal: Option<String>,
    pub cause: Option<CardCause>,
    pub messages: Vec<Message>,
}

impl CardSplitOutcome {
    /// 0 written or planned; 2 a usage defect; 1 refused.
    pub fn exit(&self) -> Exit {
        match self.cause {
            None => Exit::Answered,
            Some(CardCause::Usage) => Exit::CannotRun,
            Some(CardCause::Unknown | CardCause::Refused | CardCause::Outside) => Exit::NotFound,
        }
    }

    /// A usage defect as the terminal gives it, a [`CliError`]: its one line
    /// on stderr, stdout empty, `--json` too. Any other outcome as it is.
    pub fn or_usage_error(self) -> Result<Self, CliError> {
        match (self.cause, &self.refusal) {
            (Some(CardCause::Usage), Some(reason)) => Err(CliError::spec(reason)),
            _ => Ok(self),
        }
    }

    fn refused(
        dry_run: bool,
        mut notes: Vec<String>,
        reason: &str,
        messages: Vec<Message>,
    ) -> Self {
        let reason = one_line(reason);
        notes.push(reason.clone());
        Self {
            dry_run,
            parent: None,
            children: Vec::new(),
            reviewer: None,
            notes,
            refusal: Some(reason),
            cause: Some(CardCause::Refused),
            messages,
        }
    }

    fn usage(dry_run: bool, line: &str) -> Self {
        Self {
            cause: Some(CardCause::Usage),
            ..Self::refused(dry_run, Vec::new(), line, Vec::new())
        }
    }
}

#[derive(Serialize)]
struct CardSplitJson<'a> {
    dry_run: bool,
    parent: Option<&'a CardSplitParent>,
    children: &'a [CardSplitChild],
    reviewer: Option<&'a str>,
    notes: &'a [String],
}

/// `{dry_run, parent, children, reviewer, notes}`, every key present.
impl Serialize for CardSplitOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        CardSplitJson {
            dry_run: self.dry_run,
            parent: self.parent.as_ref(),
            children: &self.children,
            reviewer: self.reviewer.as_deref(),
            notes: &self.notes,
        }
        .serialize(serializer)
    }
}

/// The first usage defect of `request` (exit 2; the daemon's 400).
fn split_usage(request: &CardSplitRequest) -> Option<String> {
    let named = |children: &Option<Vec<String>>| children.as_ref().is_some_and(|c| !c.is_empty());
    if request.parent.is_some()
        && (!request.targets.is_empty()
            || request.title.is_some()
            || request.goal.is_some()
            || request.discipline.is_some())
    {
        return Some(
            "--parent C splits under a stored card: give no --target, --title, --goal or \
             --discipline with it"
                .to_owned(),
        );
    }
    if request.parent.is_none() && request.title.is_none() {
        return Some(
            "a split makes a parent card: give --title T, or --parent C to split under a stored \
             card"
                .to_owned(),
        );
    }
    if !named(&request.children) && !request.mandate.is_empty() {
        return Some("--mandate names a subtask: give its --child too".to_owned());
    }
    if !named(&request.children) && !request.dry_run {
        return Some(format!(
            "name each subtask's discipline with --child, or plan with --{SPLIT_DRY_RUN_FLAG}"
        ));
    }
    None
}

/// Why `name`, a split's reviewer, can be none: `assign`'s person grammar,
/// then core's, so `agent` and `local` are no reviewer.
fn split_reviewer_problem(name: &str) -> Option<String> {
    (name == CLEAR)
        .then(|| format!("reviewer: `{CLEAR}` clears a name: a new card has none"))
        .or_else(|| person_problem("reviewer", name))
        .or_else(|| reviewer_problem(name).map(|text| format!("reviewer: {text}")))
}

/// What a split reads of the corpus: the parent's targets, the plan over
/// their scope, and what every planned target reads now (a write's alone).
struct SplitRead {
    targets: Vec<String>,
    plan: SplitPlan,
    norms: TargetNorms,
}

/// `spec card split`: one parent and a subtask per `--child`, all or none
/// (task spec `department-split` S3, S5); `--dry-run` the plan (S6).
pub fn card_split(
    env: &Env,
    globals: &Globals,
    request: &CardSplitRequest,
) -> Result<CardSplitOutcome, CliError> {
    run_split(env, globals, request).map_err(escaped_error)
}

fn run_split(
    env: &Env,
    globals: &Globals,
    request: &CardSplitRequest,
) -> Result<CardSplitOutcome, CliError> {
    let dry_run = request.dry_run;
    let refuse = |reason: String, notes: Vec<String>, messages: Vec<Message>| {
        Ok(CardSplitOutcome::refused(dry_run, notes, &reason, messages))
    };
    let now = checked_now(&request.now)?;
    // 1. Usage, then the config (exit 2).
    if let Some(line) = split_usage(request) {
        return Ok(CardSplitOutcome::usage(dry_run, &line));
    }
    let actor = actor_of(request.actor.as_deref())?;
    let parent = match request.parent.as_deref() {
        Some(written) => Some(linked_card("parent", written)?),
        None => None,
    };
    let mut context = open_cards(env, globals)?;
    // 3. Fields.
    let problem = field_problem(
        request.title.as_deref(),
        request.goal.as_deref(),
        request.targets.len(),
        &[],
        None,
        None,
    )
    .or_else(|| {
        request
            .discipline
            .as_deref()
            .and_then(name_problem)
            .map(|problem| format!("discipline: {problem}"))
    });
    if let Some(problem) = problem {
        return refuse(problem, Vec::new(), Vec::new());
    }
    // Each subtask's mandate, paired by discipline with the `--child` list;
    // a dry run judges it too (task spec `task-mandate` M2).
    let asked: &[String] = request.children.as_deref().unwrap_or_default();
    let mandates = match split_mandates(&request.mandate, asked) {
        Ok(mandates) => mandates,
        Err(reason) => return refuse(reason, Vec::new(), Vec::new()),
    };
    let mandate_of = |discipline: &str| {
        asked
            .iter()
            .zip(&mandates)
            .find(|(child, _)| *child == discipline)
            .and_then(|(_, mandate)| *mandate)
    };
    // 4. The stored parent, in any state.
    let stored = match parent {
        None => None,
        Some(Err(reason)) => return refuse(reason, Vec::new(), Vec::new()),
        Some(Ok(id)) => match context.queue.get_card(&id).map_err(queue_cannot)? {
            Some(card) => Some(card),
            None => return refuse(format!("parent: {}", no_card(&id)), Vec::new(), Vec::new()),
        },
    };
    // 5. The reviewer: given, else the stored parent's, else the acting
    // name; judged for a write alone.
    let reviewer = request
        .reviewer
        .as_deref()
        .map(|name| card_person(name).to_owned())
        .or_else(|| stored.as_ref().and_then(|card| card.reviewer.clone()))
        .unwrap_or_else(|| actor.clone());
    if !dry_run && let Some(problem) = split_reviewer_problem(&reviewer) {
        return refuse(problem, Vec::new(), Vec::new());
    }
    // 6. Targets and the scope; 7. the plan.
    let mut messages = Vec::new();
    let read = match split_read(env, &context, request, stored.as_ref(), &mut messages)? {
        Ok(read) => read,
        Err(reason) => return refuse(reason, Vec::new(), messages),
    };
    let SplitRead {
        targets,
        plan,
        norms,
    } = read;
    let config = &context.config;
    let (title, goal, parent_view) = match &stored {
        Some(card) => (
            card.title.clone(),
            card.goal.clone(),
            CardSplitParent {
                id: None,
                created: false,
                title: card.title.clone(),
                discipline: card.discipline.clone(),
                flow: card.flow.clone(),
                flow_title: config
                    .flow(&card.flow)
                    .map_or_else(|| card.flow.clone(), |flow| flow.title.clone()),
                column: card.column.clone(),
                targets: card.targets.clone(),
            },
        ),
        None => {
            let flow = match config.flow_for(request.discipline.as_deref()) {
                Ok(flow) => flow,
                Err(reason) => return refuse(reason, plan.notes, messages),
            };
            let title = request.title.clone().unwrap_or_default();
            (
                title.clone(),
                request.goal.clone(),
                CardSplitParent {
                    id: None,
                    created: true,
                    title,
                    discipline: request.discipline.clone(),
                    flow: flow.name.clone(),
                    flow_title: flow.title.clone(),
                    column: flow.first_column().to_owned(),
                    targets: targets.clone(),
                },
            )
        }
    };
    let planned: Vec<CardSplitChild> = plan
        .children
        .iter()
        .map(|child| CardSplitChild {
            id: None,
            discipline: child.discipline.clone(),
            flow: child.flow.clone(),
            flow_title: child.flow_title.clone(),
            column: child.column.clone(),
            targets: child.targets.clone(),
            from: child.from,
            suggested: child.suggested,
            mandate: mandate_of(&child.discipline),
        })
        .collect();
    if dry_run {
        return Ok(CardSplitOutcome {
            dry_run,
            parent: Some(parent_view),
            children: planned,
            reviewer: (!reviewer.is_empty()).then_some(reviewer),
            notes: plan.notes,
            refusal: None,
            cause: None,
            messages,
        });
    }
    // 8. The store: one transaction, all cards or none. A new parent holds
    // no mandate; each subtask the one its discipline was given.
    let card_of =
        |flow: &str, column: &str, discipline: Option<&str>, targets: &[String]| NewCard {
            flow: flow.to_owned(),
            column: column.to_owned(),
            title: title.clone(),
            goal: goal.clone(),
            discipline: discipline.map(str::to_owned),
            targets: targets.to_vec(),
            criteria: Vec::new(),
            assignee: None,
            reviewer: Some(reviewer.clone()),
            mandate: discipline.and_then(mandate_of),
            estimate: None,
            due: None,
            parent: None,
            corrects: None,
            by: actor.clone(),
        };
    let split = NewSplit {
        parent: match &stored {
            Some(card) => SplitParent::Stored(card.id.clone()),
            None => SplitParent::New {
                card: Box::new(NewCard {
                    mandate: None,
                    ..card_of(
                        &parent_view.flow,
                        &parent_view.column,
                        request.discipline.as_deref(),
                        &targets,
                    )
                }),
                snapshot: freeze(&targets, &norms),
            },
        },
        children: plan
            .children
            .iter()
            .map(|child| NewSplitChild {
                card: card_of(
                    &child.flow,
                    &child.column,
                    Some(&child.discipline),
                    &child.targets,
                ),
                snapshot: freeze(&child.targets, &norms),
            })
            .collect(),
    };
    match context.queue.split_cards(&split, now) {
        Ok(done) => Ok(CardSplitOutcome {
            dry_run,
            parent: Some(CardSplitParent {
                id: Some(done.parent.id.clone()),
                created: done.created,
                ..parent_view
            }),
            children: planned
                .into_iter()
                .zip(&done.children)
                .map(|(child, card)| CardSplitChild {
                    id: Some(card.id.clone()),
                    ..child
                })
                .collect(),
            reviewer: Some(reviewer),
            notes: plan.notes,
            refusal: None,
            cause: None,
            messages,
        }),
        Err(QueueError::CardRefused { reason, .. }) => refuse(reason, plan.notes, messages),
        Err(error) => Err(queue_cannot(error)),
    }
}

/// The parent's targets (`--target` resolved as `new` resolves them, or the
/// stored parent's as stored), the plan over their scope, and, for a write,
/// what every target of the split reads now. No target: no index is read,
/// the scope is empty and every subtask a manual card.
fn split_read(
    env: &Env,
    context: &OpenCards,
    request: &CardSplitRequest,
    stored: Option<&Card>,
    messages: &mut Vec<Message>,
) -> Result<Result<SplitRead, String>, CliError> {
    let config = &context.config;
    let chosen = request
        .children
        .as_deref()
        .filter(|children| !children.is_empty());
    let written = stored.map_or(request.targets.as_slice(), |card| card.targets.as_slice());
    if written.is_empty() {
        return Ok(split_plan(config, &[], &[], chosen).map(|plan| SplitRead {
            targets: Vec::new(),
            plan,
            norms: TargetNorms::new(),
        }));
    }
    let project = &context.project;
    let mut open = open_index(env, project)?;
    let input = indexed_in(&mut open.index, project, messages, false)?;
    let targets = match stored {
        Some(card) => card.targets.clone(),
        None => match resolved_list(project, &input, written, "target")? {
            Ok(targets) => targets,
            Err(reason) => return Ok(Err(reason)),
        },
    };
    let graph = SpecGraph::new(&input, &project.config.scheme, &project.config.paths);
    let scope = split_scope(project, &input, &graph, &targets);
    let plan = match split_plan(config, &scope, &targets, chosen) {
        Ok(plan) => plan,
        Err(reason) => return Ok(Err(reason)),
    };
    let norms = if request.dry_run {
        TargetNorms::new()
    } else {
        let wanted: BTreeSet<&String> = targets
            .iter()
            .chain(plan.children.iter().flat_map(|child| &child.targets))
            .collect();
        norms_read(&open.index, &graph, wanted.iter().copied())?
    };
    Ok(Ok(SplitRead {
        targets,
        plan,
        norms,
    }))
}

/// The scope of a split: for each parent target in its order, the rows `spec
/// tree <target> --depth 2` lists, in its order, the target first; each node
/// once, at its first place, named as a card stores a target (its canonical
/// ID, else its document's path); an ID-less section is passed over. A target
/// that no longer names a node adds nothing.
fn split_scope(
    project: &ProjectRoot,
    input: &CheckInput,
    graph: &SpecGraph<'_>,
    targets: &[String],
) -> Vec<ScopeNode> {
    let scheme = &project.config.scheme;
    let resolver = Resolver::new(input, scheme, &project.config.paths);
    let places = target_places(graph, targets);
    let mut seen = BTreeSet::new();
    let mut scope = Vec::new();
    for target in targets {
        let roots = places.get(target).map_or(&[][..], Vec::as_slice);
        let admission = Admission::new(graph, false, roots.iter().map(|root| root.file));
        let mut left_out = LeftOut::default();
        for (at, _, _) in walked(graph, &admission, roots, Some(SCOPE_DEPTH), &mut left_out) {
            let node = graph.node(at);
            let path = graph.paths()[at.file];
            let name = if roots.contains(&at) {
                target.clone()
            } else {
                match node.and_then(|node| node.id.as_deref()) {
                    Some(id) => canonical_id(&resolver, scheme, path, id, None),
                    None if at.ord == 0 => path.to_owned(),
                    None => continue,
                }
            };
            if seen.insert(name.clone()) {
                scope.push(ScopeNode {
                    target: name,
                    kind: node.and_then(|node| node.kind.clone()),
                });
            }
        }
    }
    scope
}

// -------------------------------------------------------------- reads

/// `spec card show C [--history] [--marks] [--context [--budget N]]`.
pub fn card_show(
    env: &Env,
    globals: &Globals,
    request: &CardShowRequest,
) -> Result<CardShowOutcome, CliError> {
    run_show(env, globals, request).map_err(escaped_error)
}

fn run_show(
    env: &Env,
    globals: &Globals,
    request: &CardShowRequest,
) -> Result<CardShowOutcome, CliError> {
    // `--budget` is judged before anything is read (task spec
    // `card-context` CC4).
    if request.budget.is_some() && !request.context {
        return Err(CliError::spec(BUDGET_NEEDS_CONTEXT));
    }
    let flag_budget = budget_of(request.budget)?;
    let missing = |id: Option<String>, reason: String| CardShowOutcome {
        card: None,
        allowed: Vec::new(),
        history: None,
        mark: None,
        upstream: None,
        tasks: Vec::new(),
        items: Vec::new(),
        subtask_items: Vec::new(),
        context: None,
        notes: Vec::new(),
        id,
        reason: Some(reason),
        messages: Vec::new(),
    };
    let id = written_card(&request.id)?;
    let context = open_cards(env, globals)?;
    let Some(id) = id else {
        return Ok(missing(None, not_a_card(&request.id)));
    };
    let Some(card) = context.queue.get_card(&id).map_err(queue_cannot)? else {
        return Ok(missing(Some(id.clone()), no_card(&id)));
    };
    // Its items, read now; no card rule reads them
    // (canon `card-questions`, "The rule").
    let read = context.queue.card_items(&id).map_err(queue_cannot)?;
    let items: Vec<CardItem> = read.items.iter().map(CardItem::of).collect();
    // Its subtasks' items, one read; nothing reads a mandate from them
    // (canon `card-mandate`, "The rule").
    let below = context.queue.subtask_items(&id).map_err(queue_cannot)?;
    let subtask_items: Vec<CardSubtaskItem> = below.items.iter().map(CardSubtaskItem::of).collect();
    let item_notes: Vec<String> = read
        .unreadable
        .iter()
        .chain(&below.unreadable)
        .map(|row| {
            one_line(&format!(
                "{}: its row does not read ({}: {}); not listed",
                row.id, row.column, row.reason
            ))
        })
        .collect();
    // The card's last events and its items' last, each list capped on its
    // own, merged by `seq`.
    let history = if request.history {
        let mut events = context
            .queue
            .card_history(&id, HISTORY_MAX)
            .map_err(queue_cannot)?;
        let ids: Vec<&str> = items.iter().map(|item| item.id.as_str()).collect();
        events.extend(
            context
                .queue
                .item_history(&ids, HISTORY_MAX)
                .map_err(queue_cannot)?,
        );
        events.sort_by_key(|event| event.seq);
        Some(events)
    } else {
        None
    };
    let mut notes: Vec<String> = stranded(&context.config, &card)
        .map(|why| format!("{}: {why}", card.id))
        .into_iter()
        .collect();
    let mut messages = Vec::new();
    let (mark, upstream) = show_marks(env, &context, &card, request, &mut notes, &mut messages)?;
    let allowed = allowed_moves(context.config.flow(&card.flow), &card);
    // The context's queue and config reads, only when asked, before the
    // linked tasks' read takes the queue (task spec `card-context` CC2).
    let asked = request.context.then(|| ContextAsked {
        parent: context_parent(&context.queue, &card),
        records: context_refs(&card.targets, &items),
        budget: context_budget(flag_budget, &context),
    });
    // A card that links no task reads no task and runs no git.
    let tasks = if card.tasks.is_empty() {
        Vec::new()
    } else {
        let place = request.tasks_here.then(|| LinkPlace {
            project: context.project,
            slug: context.slug,
            data_dir: context.data_dir,
            queue: context.queue,
        });
        let (tasks, task_notes) =
            linked_tasks(env, place, &card.tasks, &request.git, &mut messages);
        notes.extend(task_notes);
        tasks
    };
    notes.extend(item_notes);
    messages.extend(notes.iter().cloned().map(Message::Note));
    let mut outcome = CardShowOutcome {
        allowed,
        card: Some(Box::new(card)),
        history,
        mark,
        upstream,
        tasks,
        items,
        subtask_items,
        context: None,
        messages,
        notes,
        id: None,
        reason: None,
    };
    if let Some(asked) = asked {
        add_context(env, globals, &mut outcome, asked, &request.git);
    }
    Ok(outcome)
}

/// `--budget` without `--context` (task spec `card-context` CC4).
const BUDGET_NEEDS_CONTEXT: &str = "`--budget` needs `--context`";

/// The `context.bundle` line's characters as made, its line end included:
/// the key, `: `, `b3:` and 64 hex digits (task spec `card-context` CC3).
const BUNDLE_LINE_CHARS: usize = "context.bundle: ".len() + "b3:".len() + 64 + 1;

/// What a card's context read before its bundle: the parent and its notes,
/// the REFs and the cap's note, the budget or its config error.
struct ContextAsked {
    parent: (Option<CardContextParent>, Vec<String>),
    records: (Vec<String>, Option<String>),
    budget: Result<u32, String>,
}

/// The card's direct parent (task spec `card-context` CC5), read now:
/// none without one; one that does not read is `None` with a note; a goal
/// over [`GOAL_MAX`] bytes cut on a character boundary, with a note.
fn context_parent(queue: &SqliteQueue, card: &Card) -> (Option<CardContextParent>, Vec<String>) {
    let Some(id) = card.parent.as_deref() else {
        return (None, Vec::new());
    };
    let parent = match queue.get_card(id) {
        Ok(Some(parent)) => parent,
        Ok(None) => return (None, vec![format!("parent {id} not read: {}", no_card(id))]),
        Err(error) => {
            return (
                None,
                vec![one_line(&format!("parent {id} not read: {error}"))],
            );
        }
    };
    let mut notes = Vec::new();
    let goal = parent.goal.map(|goal| {
        if goal.len() <= GOAL_MAX {
            return goal;
        }
        notes.push(format!("parent {id}'s goal cut at {GOAL_MAX} bytes"));
        let mut end = GOAL_MAX;
        while !goal.is_char_boundary(end) {
            end -= 1;
        }
        goal[..end].to_owned()
    });
    (
        Some(CardContextParent {
            id: parent.id,
            title: parent.title,
            goal,
        }),
        notes,
    )
}

/// A card context's REFs (task spec `card-context` CC6): its targets as
/// stored, then the `record_id` of each of its own items that is `applied`
/// and holds one, in `items` order, at most [`NODES_MAX`] records; past
/// them, the note `not bundled: <k> more records, from <ID>`. A subtask's
/// items are never its own.
fn context_refs(targets: &[String], items: &[CardItem]) -> (Vec<String>, Option<String>) {
    let applied = ProposalStatus::Applied.as_str();
    let records: Vec<&str> = items
        .iter()
        .filter(|item| item.status == applied)
        .filter_map(|item| item.record_id.as_deref())
        .collect();
    let note = records.get(NODES_MAX).map(|first| {
        format!(
            "not bundled: {} more records, from {first}",
            records.len() - NODES_MAX
        )
    });
    let refs = targets
        .iter()
        .cloned()
        .chain(
            records
                .iter()
                .take(NODES_MAX)
                .map(|&record| record.to_owned()),
        )
        .collect();
    (refs, note)
}

/// The context's budget (task spec `card-context` CC1, CC4): `--budget N`,
/// else the root's `[budgets] bundle_task`, read alone, else
/// [`DEFAULT_TASK_BUDGET`]; `bundle_node` is never read. A bad
/// `bundle_task` is its line, a note's, never an exit.
fn context_budget(flag: Option<u32>, cards: &OpenCards) -> Result<u32, String> {
    if let Some(budget) = flag {
        return Ok(budget);
    }
    match bundle_task_from_toml(&cards.config_text) {
        Ok(Some(found)) => Ok(found.tokens),
        Ok(None) => Ok(DEFAULT_TASK_BUDGET),
        Err(error) => Err(error.at(&cards.project.config_label)),
    }
}

/// The context onto a read card (task spec `card-context` CC1-CC4): its
/// parent, its notes in order, and one bundle in the room the card's lines
/// leave, else `None` and a `no bundle:` note. The notes print after the
/// read's own messages, then the bundle's own messages.
fn add_context(
    env: &Env,
    globals: &Globals,
    outcome: &mut CardShowOutcome,
    asked: ContextAsked,
    git: &GitEnv,
) {
    let ContextAsked {
        parent: (parent, mut notes),
        records: (refs, records_note),
        budget,
    } = asked;
    notes.extend(records_note);
    outcome.context = Some(CardContext {
        parent,
        bundle: None,
        notes: Vec::new(),
    });
    let room = context_room(outcome);
    let (made, bundle_messages) = context_bundle(env, globals, refs, budget, room, git);
    let bundle = match made {
        Ok(made) => Some(made),
        Err(why) => {
            notes.push(one_line(&format!("no bundle: {why}")));
            None
        }
    };
    outcome.messages.extend(
        notes
            .iter()
            .map(|note| Message::Note(format!("context: {note}"))),
    );
    outcome.messages.extend(bundle_messages);
    if let Some(context) = outcome.context.as_mut() {
        context.bundle = bundle;
        context.notes = notes;
    }
}

/// The characters left for the context's bundle (task spec `card-context`
/// CC3): [`OUTPUT_CAP_CHARS`] less every line the text prints before the
/// body, each with its line end, the `context.bundle` line at its made
/// length; `None` when nothing is left, so the card's lines are never cut
/// under a bundle.
fn context_room(outcome: &CardShowOutcome) -> Option<usize> {
    let (Some(card), Some(context)) = (outcome.card.as_deref(), outcome.context.as_ref()) else {
        return None;
    };
    let cost = |line: &String| line.chars().count() + 1;
    let printed: usize = show_lines(card, outcome).iter().map(cost).sum();
    let used = printed.saturating_sub(cost(&bundle_line(context))) + BUNDLE_LINE_CHARS;
    OUTPUT_CAP_CHARS.checked_sub(used).filter(|&room| room > 0)
}

/// The context's bundle, or why there is none (its `no bundle:` note's
/// text), and the bundle's own messages, as `spec bundle 2>&1` prints them
/// but its exit-1 reason and exit-2 line, which are the note.
fn context_bundle(
    env: &Env,
    globals: &Globals,
    refs: Vec<String>,
    budget: Result<u32, String>,
    room: Option<usize>,
    git: &GitEnv,
) -> (Result<BundleOutcome, String>, Vec<Message>) {
    if refs.is_empty() {
        return (
            Err("no targets and no recorded decision".to_owned()),
            Vec::new(),
        );
    }
    let Some(chars_max) = room else {
        return (
            Err(format!(
                "the card's lines leave no room under the {OUTPUT_CAP_CHARS}-character ceiling"
            )),
            Vec::new(),
        );
    };
    let budget = match budget {
        Ok(budget) => budget,
        Err(line) => return (Err(line), Vec::new()),
    };
    // As a task's bundle: the budget passed as a flag, so `bundle_node`
    // stays unread and a minimum's line names `--budget N`.
    let request = BundleRequest {
        references: refs,
        budget: Some(i64::from(budget)),
        git: git.clone(),
    };
    match bundle_within(env, globals, &request, chars_max) {
        Ok(made) => match made.reason.clone() {
            None => {
                let messages = made.messages.clone();
                (Ok(made), messages)
            }
            Some(reason) => (Err(reason), made.messages),
        },
        Err(error) => {
            let line = error
                .message
                .strip_prefix("spec: ")
                .unwrap_or(&error.message)
                .to_owned();
            (Err(line), Vec::new())
        }
    }
}

// ------------------------------------------------------------- raises

/// The card door's raise (task spec `card-questions` L3): the card read
/// first, before any intake rule (no such card: exit 1 `{id, reason}`, no
/// intake run); then the question's or the decision's intake, its rules
/// all, with `card_id` the card and the acting name (`local` when absent)
/// trailing its `proposal.created` as `as`, never a column. The author is a
/// human, with a hub's login. Only the queue is written; the card is not:
/// no card rule reads the item (canon `card-questions`, "The rule").
pub fn card_raise(
    env: &Env,
    globals: &Globals,
    request: &CardRaiseRequest,
) -> Result<CardRaiseOutcome, CliError> {
    run_raise(env, globals, request).map_err(escaped_error)
}

fn run_raise(
    env: &Env,
    globals: &Globals,
    request: &CardRaiseRequest,
) -> Result<CardRaiseOutcome, CliError> {
    checked_now(&request.now)?;
    let actor = actor_of(request.actor.as_deref())?;
    let id = written_card(&request.id)?;
    let context = open_cards(env, globals)?;
    let Some(id) = id else {
        return Ok(CardRaiseOutcome::missing(None, not_a_card(&request.id)));
    };
    if context.queue.get_card(&id).map_err(queue_cannot)?.is_none() {
        return Ok(CardRaiseOutcome::missing(Some(id.clone()), no_card(&id)));
    }
    // The queue is opened again by the intake, under its own lock.
    drop(context);
    let intake = match request.kind {
        CardRaiseKind::Question => propose_question_as(
            env,
            globals,
            &QuestionRequest {
                node_ids: request.node_ids.clone(),
                text: request.text.clone(),
                working_answer: request.working_answer.clone(),
                price_of_other: request.price_of_other.clone(),
                // A person's raise sends no lead: stored as before (task
                // spec `plain-questions` PQ3).
                gist: None,
                matters: None,
                options: None,
                recommendation: None,
                severity: request.severity,
                distinct_from: request.distinct_from.clone(),
                card: Some(id),
                author_role: None,
                author_model: None,
                run: None,
                login: request.login.clone(),
                now: request.now.clone(),
                git: request.git.clone(),
            },
            &actor,
        )?,
        CardRaiseKind::Decision => propose_decision_as(
            env,
            globals,
            &DecisionRequest {
                input: DecisionInput {
                    node_ids: request.node_ids.clone(),
                    text: request.text.clone(),
                    working_answer: request.working_answer.clone(),
                    price_of_other: request.price_of_other.clone(),
                    why: request.why.clone(),
                    severity: request.severity,
                    proposed_patch: None,
                    distinct_from: Some(request.distinct_from.clone()),
                },
                card: Some(id),
                author_role: None,
                author_model: None,
                run: None,
                login: request.login.clone(),
                now: request.now.clone(),
                git: request.git.clone(),
            },
            &actor,
        )?,
    };
    Ok(CardRaiseOutcome {
        intake: Some(Box::new(intake)),
        id: None,
        reason: None,
        messages: Vec::new(),
    })
}

/// `--marks`: `card`'s mark as its `list --marks` row reads it, over every
/// card's live correction and its own discrepancy, so the notes are its own;
/// a cancelled card has none. `upstream`: an open subtask's parent is read
/// and marked alike, from the same refresh, and what changed above it judged
/// (task spec `upstream-signal` US-7 to US-9): one list of the cards, one
/// refresh of the index, one read of the events and one of the applied
/// items, each only when something asks it, so a card with no parent asked
/// no mark reads none. An index that cannot be read is the read's error
/// ([`list_marks`]), never a clean card.
fn show_marks(
    env: &Env,
    context: &OpenCards,
    card: &Card,
    request: &CardShowRequest,
    notes: &mut Vec<String>,
    messages: &mut Vec<Message>,
) -> Result<(Option<CardMark>, Option<Upstream>), CliError> {
    let marked = request.marks && card.cancellation.is_none();
    let parent = match request.upstream.then(|| judged_parent(card)).flatten() {
        Some(id) => match context.queue.get_card(id) {
            Ok(Some(parent)) => Some(parent),
            Ok(None) | Err(_) => {
                notes.push(parent_unread(&card.id, id));
                None
            }
        },
        None => None,
    };
    if !marked && parent.is_none() {
        return Ok((None, None));
    }
    let listed = context.queue.list_cards().map_err(queue_cannot)?;
    // The read's notes are the card's own: the project's rows that do not
    // read under `--marks` alone, never on an upstream read without it (the
    // daemon's card route), and the parent's discrepancy never.
    if marked {
        notes.extend(
            listed
                .unreadable
                .iter()
                .map(|row| one_line(&format!("{row}; not read"))),
        );
    }
    let mut facts = QueueRead::default();
    for other in listed
        .cards
        .iter()
        .filter(|other| other.cancellation.is_none())
    {
        correction_of(other, &mut facts);
    }
    if marked {
        settled_of(&context.queue, card, &mut facts);
        notes.append(&mut facts.notes);
    }
    let live_parent = parent
        .as_ref()
        .filter(|parent| parent.cancellation.is_none());
    if let Some(parent) = live_parent {
        settled_of(&context.queue, parent, &mut facts);
        facts.notes.clear();
    }
    let shown = if marked { slice::from_ref(card) } else { &[] };
    let reached: BTreeSet<&String> = parent
        .iter()
        .flat_map(|parent| card.targets.iter().chain(&parent.targets))
        .collect();
    let also: Vec<&Card> = live_parent.into_iter().collect();
    let mut read = list_marks(env, context, shown, &also, &reached, &facts, messages)?;
    notes.append(&mut read.notes);
    let upstream = match &parent {
        Some(parent) => {
            let facts = UpstreamFacts::read(&context.queue)?;
            let judge = Judge::new(&facts);
            if !judge.anchored(&card.id) {
                notes.push(no_anchor(&card.id));
            }
            // Applied rows that do not decode cannot be judged: one note
            // whatever their count, never read as clean (task spec
            // `upstream-signal` US-8).
            notes.extend(facts.unread_note(&card.id));
            let reach = reach_of(card.targets.iter().chain(&parent.targets), &read.reaches);
            judge.judge(card, parent, read.marks.get(&parent.id), &reach)
        }
        None => None,
    };
    let mark = if marked {
        read.marks.remove(&card.id)
    } else {
        None
    };
    Ok((mark, upstream))
}

/// Why `[card]` no longer places `card`: its flow or its column is gone.
fn stranded(config: &CardConfig, card: &Card) -> Option<String> {
    match config.flow(&card.flow) {
        None => Some(format!(
            "the flow `{}` is no longer in `[card]`: served last, no move open",
            card.flow
        )),
        Some(flow) if !flow.holds(&card.column) => Some(format!(
            "the column `{}` is no longer in the flow `{}`: served last",
            card.column, card.flow
        )),
        Some(_) => None,
    }
}

/// One note per (flow, column) `[card]` no longer places, its cards counted
/// and the first named: `cards` in list order, so a pair's cards stand
/// together.
fn stranded_notes(config: &CardConfig, cards: &[Card]) -> Vec<String> {
    cards
        .chunk_by(|one, next| one.flow == next.flow && one.column == next.column)
        .filter_map(|group| {
            let first = group.first()?;
            stranded(config, first).map(|why| {
                format!(
                    "{} card(s) in {}/{}, from {}: {why}",
                    group.len(),
                    first.flow,
                    first.column,
                    first.id
                )
            })
        })
        .collect()
}

/// `spec card list`: by flow, column (config order), position; a card
/// `[card]` no longer places last, cancelled ones last in their column.
pub fn card_list(
    env: &Env,
    globals: &Globals,
    request: &CardListRequest,
) -> Result<CardListOutcome, CliError> {
    run_list(env, globals, request).map_err(escaped_error)
}

fn run_list(
    env: &Env,
    globals: &Globals,
    request: &CardListRequest,
) -> Result<CardListOutcome, CliError> {
    let parent = match request.parent.as_deref() {
        Some(written) => Some(
            written_card(written)?
                .ok_or_else(|| CliError::spec(format!("--parent: {}", not_a_card(written))))?,
        ),
        None => None,
    };
    let context = open_cards(env, globals)?;
    let listed = context.queue.list_cards().map_err(queue_cannot)?;
    let mut notes: Vec<String> = listed
        .unreadable
        .iter()
        .map(|row| one_line(&format!("{row}; not listed")))
        .collect();
    // Over every card: a filter must not hide a live correction.
    let mut facts = QueueRead::default();
    if request.marks {
        for card in listed
            .cards
            .iter()
            .filter(|card| card.cancellation.is_none())
        {
            facts_of(&context.queue, card, &mut facts);
        }
    }
    let exact = |asked: &Option<String>, value: Option<&str>| {
        asked.as_deref().is_none_or(|asked| Some(asked) == value)
    };
    // The cards a filter leaves out are kept aside: a listed subtask's
    // parent is read among them too (task spec `upstream-signal` US-8).
    let (mut cards, others): (Vec<Card>, Vec<Card>) = listed.cards.into_iter().partition(|card| {
        exact(&request.flow, Some(&card.flow))
            && exact(&request.column, Some(&card.column))
            && exact(&request.discipline, card.discipline.as_deref())
            && exact(&request.assignee, card.assignee.as_deref())
            && exact(&parent, card.parent.as_deref())
            && (!request.no_target || card.targets.is_empty())
            && (request.with_cancelled || card.cancellation.is_none())
    });
    let config = &context.config;
    cards.sort_by_cached_key(|card| list_key(config, card));
    notes.extend(stranded_notes(config, &cards));
    let mut messages = Vec::new();
    let (marks, upstream) = if request.marks {
        notes.append(&mut facts.notes);
        list_upstream(
            env,
            &context,
            &cards,
            &others,
            &facts,
            &mut notes,
            &mut messages,
        )?
    } else {
        (BTreeMap::new(), BTreeSet::new())
    };
    let flow_lines: Vec<String> = config
        .flows
        .iter()
        .map(|flow| flow_line(flow, &cards))
        .collect();
    let used = flow_lines
        .iter()
        .map(|line| line.chars().count() + 1)
        .sum::<usize>();
    let (shown, _) = lines_within(
        cards.iter().map(|card| {
            card_line(card, marks.get(&card.id), upstream.contains(&card.id))
                .chars()
                .count()
                + 1
        }),
        used,
        false,
    );
    messages.extend(notes.iter().cloned().map(Message::Note));
    Ok(CardListOutcome {
        flows: config.flows.clone(),
        flow_lines,
        cards,
        marks,
        upstream,
        marked: request.marks,
        shown,
        messages,
        notes,
    })
}

/// `--marks`: each listed card's mark ([`list_marks`]) and the listed open
/// subtasks something changed above (task spec `upstream-signal` US-8,
/// US-9): each one's parent found among every readable card, listed or not
/// (`others`), and marked from the same refresh; one read of the events and
/// one of the applied items for them all, none when no open subtask is
/// listed. A parent that does not read leaves its row out of the set, its
/// row's own note already given.
fn list_upstream(
    env: &Env,
    context: &OpenCards,
    cards: &[Card],
    others: &[Card],
    facts: &QueueRead,
    notes: &mut Vec<String>,
    messages: &mut Vec<Message>,
) -> Result<(BTreeMap<String, CardMark>, BTreeSet<String>), CliError> {
    let every: BTreeMap<&str, &Card> = cards
        .iter()
        .chain(others)
        .map(|card| (card.id.as_str(), card))
        .collect();
    let subtasks: Vec<(&Card, &Card)> = cards
        .iter()
        .filter_map(|card| Some((card, *every.get(judged_parent(card)?)?)))
        .collect();
    // A listed parent is marked as a listed card, once; the others beside.
    let mut seen: BTreeSet<&str> = cards.iter().map(|card| card.id.as_str()).collect();
    let parents: Vec<&Card> = subtasks
        .iter()
        .map(|(_, parent)| *parent)
        .filter(|parent| parent.cancellation.is_none() && seen.insert(parent.id.as_str()))
        .collect();
    let reached: BTreeSet<&String> = subtasks
        .iter()
        .flat_map(|(child, parent)| child.targets.iter().chain(&parent.targets))
        .collect();
    let read = list_marks(env, context, cards, &parents, &reached, facts, messages)?;
    notes.extend(read.notes);
    let mut upstream = BTreeSet::new();
    if !subtasks.is_empty() {
        let queue_facts = UpstreamFacts::read(&context.queue)?;
        let judge = Judge::new(&queue_facts);
        for (child, parent) in &subtasks {
            let reach = reach_of(child.targets.iter().chain(&parent.targets), &read.reaches);
            if judge
                .judge(child, parent, read.marks.get(&parent.id), &reach)
                .is_some()
            {
                upstream.insert(child.id.clone());
            }
        }
    }
    let marks = cards
        .iter()
        .filter_map(|card| Some((card.id.clone(), read.marks.get(&card.id)?.clone())))
        .collect();
    Ok((marks, upstream))
}

/// What one refresh of the index says of the cards a read marks.
struct MarksRead {
    /// Each live card's mark, listed or asked beside them, by ID.
    marks: BTreeMap<String, CardMark>,
    /// What each target asked reaches ([`Reaches`]).
    reaches: Reaches,
    /// What a read cannot compare of a listed card.
    notes: Vec<String>,
}

/// `--marks`: each listed card's mark over all its targets, core's, and what
/// a read cannot compare of it; the marks of `also` (a subtask's parents
/// not among `cards`) and what each of `reached` reaches, from the same
/// refresh. A cancelled card, which every read ignores, has none. No index
/// to read the targets: the read's error, so a board never draws a clean
/// tile it could not vouch for.
fn list_marks(
    env: &Env,
    context: &OpenCards,
    cards: &[Card],
    also: &[&Card],
    reached: &BTreeSet<&String>,
    facts: &QueueRead,
    messages: &mut Vec<Message>,
) -> Result<MarksRead, CliError> {
    let live = || cards.iter().filter(|card| card.cancellation.is_none());
    let marked: Vec<&Card> = live()
        .chain(
            also.iter()
                .copied()
                .filter(|card| card.cancellation.is_none()),
        )
        .collect();
    let mut targets: BTreeSet<&String> = marked.iter().flat_map(|card| &card.targets).collect();
    targets.extend(reached.iter().copied());
    let (norms, reaches) = fresh_norms_reach(env, &context.project, &targets, reached, messages)?;
    let facts = MarkFacts {
        norms: &norms,
        settled: &facts.settled,
        corrections: &facts.corrections,
    };
    let marks = marked
        .iter()
        .map(|card| (card.id.clone(), card_mark(card, &card.targets, &facts)))
        .collect();
    let notes = live().flat_map(|card| mark_notes(card, &norms)).collect();
    Ok(MarksRead {
        marks,
        reaches,
        notes,
    })
}

/// Where a name stands in `names`: its place, or past them all by name.
fn place_of<'n>(names: impl Iterator<Item = &'n str>, name: &str) -> (usize, String) {
    let mut count = 0;
    for known in names {
        if known == name {
            return (count, String::new());
        }
        count += 1;
    }
    (count, name.to_owned())
}

/// A card's place in the list. Ties by ID, by number.
fn list_key(config: &CardConfig, card: &Card) -> impl Ord + use<> {
    let flow = config.flow(&card.flow);
    let flow_place = place_of(
        config.flows.iter().map(|flow| flow.name.as_str()),
        &card.flow,
    );
    let column_place = match flow {
        Some(flow) => place_of(
            flow.columns.iter().map(|column| column.id.as_str()),
            &card.column,
        ),
        None => (0, card.column.clone()),
    };
    (
        flow_place,
        column_place,
        card.cancellation.is_some(),
        card.position,
        card_number(&card.id),
    )
}

/// `flow <name> | <column> <n> | ...`: the cards listed standing in each
/// column (a cancelled card has left it).
fn flow_line(flow: &CardFlow, cards: &[Card]) -> String {
    let mut line = format!("flow {}", flow.name);
    for column in &flow.columns {
        let count = cards
            .iter()
            .filter(|card| {
                card.flow == flow.name && card.column == column.id && card.cancellation.is_none()
            })
            .count();
        line.push_str(&format!(" | {} {count}", column.id));
    }
    line
}

/// `<id> | <flow>/<column> | <position> | <title> | <discipline> | <assignee>
/// | <due> | <targets>[ | mandate <m>][ | cancelled][ | mark <mark>][ |
/// upstream]`, no line end: a card with no mandate prints as before it, and
/// `upstream` only when something changed above it (task spec
/// `upstream-signal` US-8).
fn card_line(card: &Card, mark: Option<&CardMark>, upstream: bool) -> String {
    let optional = |value: &Option<String>| value.as_deref().map_or(ABSENT.to_owned(), one_line);
    let targets = if card.targets.is_empty() {
        ABSENT.to_owned()
    } else {
        one_line(&card.targets.join(", "))
    };
    let mut line = format!(
        "{} | {}/{} | {} | {} | {} | {} | {} | {targets}",
        card.id,
        card.flow,
        card.column,
        card.position,
        one_line(&card.title),
        optional(&card.discipline),
        optional(&card.assignee),
        optional(&card.due),
    );
    if let Some(mandate) = card.mandate {
        line.push_str(&format!(" | mandate {}", mandate.as_str()));
    }
    if card.cancellation.is_some() {
        line.push_str(" | cancelled");
    }
    if let Some(mark) = mark {
        line.push_str(&format!(" | mark {}", one_line(&mark.text())));
    }
    if upstream {
        line.push_str(" | upstream");
    }
    line
}

// ------------------------------------------------------------- writes

/// `spec card move C [--to K] [--before C] [--note T]`: the flow's rule, a
/// reorder within a column none; last, or at `--before`'s place. A note
/// rides on `card.moved`; into the flow's `return` column from another one
/// it is required.
pub fn card_move(
    env: &Env,
    globals: &Globals,
    request: &CardMoveRequest,
) -> Result<CardOutcome, CliError> {
    run_move(env, globals, request).map_err(escaped_error)
}

fn run_move(
    env: &Env,
    globals: &Globals,
    request: &CardMoveRequest,
) -> Result<CardOutcome, CliError> {
    const ACTION: CardAction = CardAction::Move;
    let now = checked_now(&request.now)?;
    if request.to.is_none() && request.before.is_none() {
        return Ok(CardOutcome::usage(
            ACTION,
            "`spec card move` asks nothing: give --to K, --before C or both",
        ));
    }
    let actor = actor_of(request.actor.as_deref())?;
    let seen = revision_of(request.revision.as_deref())?;
    let id = written_card(&request.id)?;
    let before = match request.before.as_deref() {
        Some(written) => Some(linked_card("--before", written)?),
        None => None,
    };
    let mut context = open_cards(env, globals)?;
    let card = match named_card(&context, &request.id, id, ACTION)? {
        Ok(card) => card,
        Err(refused) => return Ok(*refused),
    };
    let refuse = |reason: &str, context: &OpenCards| {
        Ok(CardOutcome::refused(
            ACTION,
            Some(&card.id),
            Some((&card, &context.config)),
            reason,
            Vec::new(),
        ))
    };
    let before = match before.transpose() {
        Ok(before) => before,
        Err(reason) => return refuse(&reason, &context),
    };
    let asked = MoveAsked {
        to: request.to.as_deref(),
        note: request.note.as_deref(),
        actor: &actor,
    };
    if let Some(reason) = move_refusal(&context, &card, seen, &asked) {
        return refuse(&reason, &context);
    }
    let config = context.config.clone();
    let change = CardChange::Move {
        flow: config.flow(&card.flow),
        to: request.to.clone(),
        before,
        note: request.note.clone(),
    };
    changed(&mut context, ACTION, &card, seen, &change, &actor, now)
}

/// What a move asks: the column, why, and who.
struct MoveAsked<'a> {
    to: Option<&'a str>,
    note: Option<&'a str>,
    actor: &'a str,
}

/// Why `note` is no note a move records: `cancel --reason`'s grammar,
/// blank or over [`NOTE_MAX`].
fn note_problem(note: &str) -> Option<String> {
    note.trim()
        .is_empty()
        .then(|| "note: blank; say why it moves".to_owned())
        .or_else(|| over_bytes("note", note, NOTE_MAX))
}

/// What the flow says of this move before the store says it again under
/// its lock, in the store's order: a closed card, a stale revision, core's
/// decision, a `return` column's note among it. The note's grammar, asked
/// here alone, comes after the closed card, as `cancel`'s reason does.
fn move_refusal(
    context: &OpenCards,
    card: &Card,
    seen: Option<u64>,
    asked: &MoveAsked<'_>,
) -> Option<String> {
    if let Some(reason) = closed_refusal(card) {
        return Some(reason);
    }
    if let Some(problem) = asked.note.and_then(note_problem) {
        return Some(problem);
    }
    if seen.is_some_and(|seen| seen != card.revision) {
        return Some(stale_refusal(card));
    }
    let flow = context.config.flow(&card.flow)?;
    let to = asked.to.unwrap_or(&card.column);
    decide_move(flow, &card.column, to, asked.actor, asked.note)
        .err()
        .map(|refusal| refusal.message(&card.id, &card.column, to, asked.actor))
}

/// `spec card edit C …`.
pub fn card_edit(
    env: &Env,
    globals: &Globals,
    request: &CardEditRequest,
) -> Result<CardOutcome, CliError> {
    run_edit(env, globals, request).map_err(escaped_error)
}

/// `--target -` or `--criterion -` alone: the list cleared.
fn clears(values: &[String]) -> bool {
    matches!(values, [value] if value == CLEAR)
}

fn run_edit(
    env: &Env,
    globals: &Globals,
    request: &CardEditRequest,
) -> Result<CardOutcome, CliError> {
    const ACTION: CardAction = CardAction::Edit;
    let now = checked_now(&request.now)?;
    let asked = request.title.is_some()
        || request.goal.is_some()
        || !request.targets.is_empty()
        || !request.criteria.is_empty()
        || request.estimate.is_some()
        || request.due.is_some()
        || request.parent.is_some()
        || request.corrects.is_some()
        || !request.evidence.is_empty()
        || !request.tasks.is_empty()
        || request.mandate.is_some();
    let beside_estimate = request.title.is_some()
        || request.goal.is_some()
        || !request.targets.is_empty()
        || !request.criteria.is_empty()
        || request.due.is_some()
        || request.parent.is_some()
        || request.corrects.is_some()
        || !request.evidence.is_empty()
        || !request.tasks.is_empty()
        || request.mandate.is_some();
    if !asked {
        return Ok(CardOutcome::usage(
            ACTION,
            "`spec card edit` asks nothing: give a field to change or --evidence",
        ));
    }
    let actor = actor_of(request.actor.as_deref())?;
    let seen = revision_of(request.revision.as_deref())?;
    let id = written_card(&request.id)?;
    let parent = link_field("parent", request.parent.as_deref())?;
    let corrects = link_field("corrects", request.corrects.as_deref())?;
    let tasks = written_tasks(&request.tasks)?;
    let mut context = open_cards(env, globals)?;
    if tasks.is_some()
        && let Some(clash) = task_prefix_clash(&context.project.config.scheme)
    {
        return Err(CliError::spec(format!(
            "{}: {clash}",
            context.project.config_label
        )));
    }
    let card = match named_card(&context, &request.id, id, ACTION)? {
        Ok(card) => card,
        Err(refused) => return Ok(*refused),
    };
    let mut messages = Vec::new();
    let refuse = |reason: &str, context: &OpenCards, messages: Vec<Message>| {
        Ok(CardOutcome::refused(
            ACTION,
            Some(&card.id),
            Some((&card, &context.config)),
            reason,
            messages,
        ))
    };
    if let Some(reason) = closed_refusal(&card) {
        return refuse(&reason, &context, messages);
    }
    // A change of an estimate the card holds, a clear too, stands alone and
    // says why.
    let reestimate = request.estimate.is_some() && card.estimate.is_some();
    if reestimate && beside_estimate {
        return Ok(CardOutcome::usage(
            ACTION,
            &format!(
                "`spec card edit`: {} holds an estimate: --estimate takes --reason and no other \
                 field",
                card.id
            ),
        ));
    }
    if !reestimate && request.reason.is_some() {
        return Ok(CardOutcome::usage(
            ACTION,
            "`spec card edit`: --reason goes with --estimate on a card holding an estimate",
        ));
    }
    if reestimate {
        let problem = match &request.reason {
            Some(reason) => reason_problem(reason, "why the estimate changes"),
            None => Some(format!(
                "{} holds an estimate: --reason says why it changes; nothing changed",
                card.id
            )),
        };
        if let Some(problem) = problem {
            return refuse(&problem, &context, messages);
        }
    }
    let (parent, corrects) = match (parent.transpose(), corrects.transpose()) {
        (Ok(parent), Ok(corrects)) => (parent, corrects),
        (Err(reason), _) | (_, Err(reason)) => return refuse(&reason, &context, messages),
    };
    let criteria = if clears(&request.criteria) {
        &[][..]
    } else {
        &request.criteria[..]
    };
    let goal = field(request.goal.as_deref());
    let estimate = field(request.estimate.as_deref());
    let due = field(request.due.as_deref());
    let cleared_title = request.title.as_deref() == Some(CLEAR);
    let problem = cleared_title
        .then(|| "title: a card has one; `-` clears no title".to_owned())
        .or_else(|| {
            field_problem(
                request.title.as_deref(),
                goal.as_ref().and_then(Option::as_deref),
                request.targets.len(),
                criteria,
                estimate.as_ref().and_then(Option::as_deref),
                due.as_ref().and_then(Option::as_deref),
            )
        })
        .or_else(|| over_items("evidence", request.evidence.len(), EVIDENCE_ENTRIES_MAX));
    if let Some(problem) = problem {
        return refuse(&problem, &context, messages);
    }
    let mut evidence = Vec::with_capacity(request.evidence.len());
    for written in &request.evidence {
        match parse_evidence(written) {
            Ok(entry) => evidence.push(entry),
            Err(reason) => return refuse(&reason, &context, messages),
        }
    }
    if let Some(tasks) = &tasks {
        if let Some(problem) = task_links_problem(tasks) {
            return refuse(&problem, &context, messages);
        }
        // Locally each names a task row of the project, any repository's,
        // any state's, before the write; a hub keeps the IDs as written
        // (canon `card-tasks`, "The rule").
        if request.tasks_here
            && !tasks.is_empty()
            && let Some(missing) = context.queue.missing_task(tasks).map_err(queue_cannot)?
        {
            return refuse(
                &format!("task: no task {missing}; nothing changed"),
                &context,
                messages,
            );
        }
    }
    let (targets, norms) = if clears(&request.targets) {
        (Some(Vec::new()), Some(TargetNorms::new()))
    } else if request.targets.is_empty() {
        (None, None)
    } else {
        match resolved_targets(env, &context, &request.targets, &mut messages)? {
            Ok((targets, norms)) => (Some(targets), Some(norms)),
            Err(reason) => return refuse(&reason, &context, messages),
        }
    };
    let edit = CardEdit {
        title: request.title.clone(),
        goal,
        targets,
        criteria: (!request.criteria.is_empty()).then(|| criteria.to_vec()),
        estimate,
        due,
        parent,
        corrects,
        evidence,
        reason: request.reason.clone(),
        norms,
        tasks,
        // Judged by the store against the row under its lock, whatever else
        // the edit holds: only the card's reviewer changes it (canon
        // `card-mandate`, "The rule").
        mandate: request.mandate,
    };
    let change = CardChange::Edit(Box::new(edit));
    let mut answer = changed(&mut context, ACTION, &card, seen, &change, &actor, now)?;
    answer.notes.splice(0..0, notes(&messages));
    answer.messages.splice(0..0, messages);
    Ok(answer)
}

/// `--task` as written: `None` when not given, none when a lone `-` clears
/// the list, else each entry a task ID as `spec task` takes it, a look-alike
/// exiting 2 naming its Latin form; an entry that is no task ID in any
/// script is kept as written, for core's grammar to refuse.
fn written_tasks(written: &[String]) -> Result<Option<Vec<String>>, CliError> {
    if written.is_empty() {
        return Ok(None);
    }
    if clears(written) {
        return Ok(Some(Vec::new()));
    }
    let mut tasks = Vec::with_capacity(written.len());
    for entry in written {
        tasks.push(written_task(entry)?.unwrap_or_else(|| entry.clone()));
    }
    Ok(Some(tasks))
}

/// A card field of `edit` as written: absent, cleared, a card; a look-alike
/// exits 2, `Err` inside the refusal when it is no card ID.
fn link_field(
    key: &str,
    written: Option<&str>,
) -> Result<Option<Result<Option<String>, String>>, CliError> {
    match field(written) {
        None => Ok(None),
        Some(None) => Ok(Some(Ok(None))),
        Some(Some(value)) => Ok(Some(linked_card(key, &value)?.map(Some))),
    }
}

/// `spec card assign C [--assignee N] [--reviewer N]`.
pub fn card_assign(
    env: &Env,
    globals: &Globals,
    request: &CardAssignRequest,
) -> Result<CardOutcome, CliError> {
    run_assign(env, globals, request).map_err(escaped_error)
}

fn run_assign(
    env: &Env,
    globals: &Globals,
    request: &CardAssignRequest,
) -> Result<CardOutcome, CliError> {
    const ACTION: CardAction = CardAction::Assign;
    let now = checked_now(&request.now)?;
    if request.assignee.is_none() && request.reviewer.is_none() {
        return Ok(CardOutcome::usage(
            ACTION,
            "`spec card assign` asks nothing: give --assignee N, --reviewer N or both",
        ));
    }
    let actor = actor_of(request.actor.as_deref())?;
    let seen = revision_of(request.revision.as_deref())?;
    let id = written_card(&request.id)?;
    let mut context = open_cards(env, globals)?;
    let card = match named_card(&context, &request.id, id, ACTION)? {
        Ok(card) => card,
        Err(refused) => return Ok(*refused),
    };
    let assignee = person_field(request.assignee.as_deref());
    let reviewer = person_field(request.reviewer.as_deref());
    let problem = closed_refusal(&card)
        .or_else(|| {
            assignee
                .as_ref()
                .and_then(Option::as_deref)
                .and_then(|name| person_problem("assignee", name))
        })
        .or_else(|| {
            reviewer
                .as_ref()
                .and_then(Option::as_deref)
                .and_then(|name| {
                    person_problem("reviewer", name)
                        .or_else(|| reviewer_problem(name).map(|text| format!("reviewer: {text}")))
                })
        });
    if let Some(problem) = problem {
        return Ok(CardOutcome::refused(
            ACTION,
            Some(&card.id),
            Some((&card, &context.config)),
            &problem,
            Vec::new(),
        ));
    }
    let change = CardChange::Assign { assignee, reviewer };
    changed(&mut context, ACTION, &card, seen, &change, &actor, now)
}

/// `spec card cancel C --reason T`: the card leaves its column.
pub fn card_cancel(
    env: &Env,
    globals: &Globals,
    request: &CardCancelRequest,
) -> Result<CardOutcome, CliError> {
    run_cancel(env, globals, request).map_err(escaped_error)
}

fn run_cancel(
    env: &Env,
    globals: &Globals,
    request: &CardCancelRequest,
) -> Result<CardOutcome, CliError> {
    const ACTION: CardAction = CardAction::Cancel;
    let now = checked_now(&request.now)?;
    let actor = actor_of(request.actor.as_deref())?;
    let seen = revision_of(request.revision.as_deref())?;
    let id = written_card(&request.id)?;
    let mut context = open_cards(env, globals)?;
    let card = match named_card(&context, &request.id, id, ACTION)? {
        Ok(card) => card,
        Err(refused) => return Ok(*refused),
    };
    let problem = closed_refusal(&card)
        .or_else(|| {
            request
                .reason
                .trim()
                .is_empty()
                .then(|| "reason: blank; say why the card is cancelled".to_owned())
        })
        .or_else(|| over_bytes("reason", &request.reason, NOTE_MAX));
    if let Some(problem) = problem {
        return Ok(CardOutcome::refused(
            ACTION,
            Some(&card.id),
            Some((&card, &context.config)),
            &problem,
            Vec::new(),
        ));
    }
    let change = CardChange::Cancel {
        reason: request.reason.clone(),
    };
    changed(&mut context, ACTION, &card, seen, &change, &actor, now)
}

/// `spec card accept C --as N` with its verdict: the card in `accept.from`,
/// `N` its reviewer and not its assignee, after the person's consent; to
/// `accept.to`, which nothing else reaches. Its snapshot is frozen anew, a
/// correction's accept also that of the card it corrects; `--departs` makes
/// the discrepancy in the same transaction, refused by no hit.
pub fn card_accept(
    env: &Env,
    globals: &Globals,
    request: &CardAcceptRequest,
    consent: Consent<'_>,
) -> Result<CardOutcome, CliError> {
    run_accept(env, globals, request, consent).map_err(escaped_error)
}

/// The verdict as asked: one of the two, a departure with its three texts
/// and nothing of one beside `--briefed`; else the usage line.
fn verdict_usage(request: &CardAcceptRequest) -> Option<&'static str> {
    let departing = !request.departs.is_empty();
    let texts = [&request.summary, &request.observed, &request.documented];
    let any_text = texts.iter().any(|text| text.is_some()) || request.gap_type.is_some();
    if request.briefed && (departing || any_text) {
        return Some("`spec card accept`: --briefed takes no --departs, nor its texts");
    }
    if !request.briefed && !departing {
        return Some("`spec card accept` asks one of --briefed and --departs REF...");
    }
    (departing && !texts.iter().all(|text| text.is_some()))
        .then_some("`spec card accept`: --departs takes --summary, --observed and --documented")
}

/// `--gap-type G`, off its four words a usage line.
fn gap_type_of(request: &CardAcceptRequest) -> Result<Option<GapType>, String> {
    let Some(written) = request.gap_type.as_deref() else {
        return Ok(None);
    };
    GapType::parse(written).map(Some).ok_or_else(|| {
        let words: Vec<&str> = GapType::ALL.iter().map(|gap| gap.as_str()).collect();
        format!("--gap-type: `{written}` is none of {}", words.join(", "))
    })
}

/// Where a departure is raised and what it reads of the root: the item as
/// the queue stores it, its corpus hits named as `distinct_from` takes them,
/// and what the card's targets read now. `Err` the refusal (exit 1); git is
/// read here alone, reads only, exit 2 outside a worktree.
struct Departing {
    intake: NewIntake,
    corpus_hits: Vec<String>,
    departs: Vec<String>,
    norms: TargetNorms,
    messages: Vec<Message>,
}

fn departing(
    env: &Env,
    context: &OpenCards,
    card: &Card,
    request: &CardAcceptRequest,
    gap_type: Option<GapType>,
    targets: &BTreeSet<&String>,
) -> Result<Result<Departing, (String, Vec<Message>)>, CliError> {
    let project = &context.project;
    let mut messages = Vec::new();
    let text = |value: &Option<String>| value.clone().unwrap_or_default();
    let (summary, observed, documented) = (
        text(&request.summary),
        text(&request.observed),
        text(&request.documented),
    );
    let input = departure_input(&Departure {
        card: &card.id,
        node_ids: &request.departs,
        summary: &summary,
        observed: &observed,
        documented: &documented,
        gap_type,
    });
    let author = Author {
        login: request.login.clone(),
        ..Author::human()
    };
    let anonymous = AuthorInput {
        role: None,
        model: None,
        run: None,
    };
    if let Some(problem) = discrepancy_problem(&input, anonymous) {
        return Ok(Err((problem.to_string(), messages)));
    }
    let place = departure_place(env, project)?;
    let mut open = open_index(env, project)?;
    let read = indexed_in(&mut open.index, project, &mut messages, false)?;
    let mut nodes: Vec<IntakeTarget> = Vec::with_capacity(input.node_ids.len());
    for (index, written) in input.node_ids.iter().enumerate() {
        let field = format!("departs[{index}]");
        let (id, path) = match resolved_node(project, &read, written)
            .map_err(|error| prefixed(error, &field))?
        {
            Ok(found) => found,
            Err(reason) => return Ok(Err((format!("{field}: {reason}"), messages))),
        };
        if !card.targets.contains(&id) {
            return Ok(Err((
                format!(
                    "{field}: `{id}` is no direct target of {}; nothing changed",
                    card.id
                ),
                messages,
            )));
        }
        if nodes.iter().any(|node| node.id == id) {
            return Ok(Err((
                format!("{field}: `{id}` is named twice; name each node once"),
                messages,
            )));
        }
        nodes.push(IntakeTarget { id, path });
    }
    let graph = SpecGraph::new(&read, &project.config.scheme, &project.config.paths);
    let corpus_hits = departing_hits(&graph, &nodes);
    let norms = norms_read(&open.index, &graph, targets.iter().copied())?;
    let departs: Vec<String> = nodes.iter().map(|node| node.id.clone()).collect();
    let intake = departure_intake(&input, &nodes, place, author, &card.id);
    Ok(Ok(Departing {
        intake,
        corpus_hits,
        departs,
        norms,
        messages,
    }))
}

/// The place a departure binds, `propose update`'s step 5: `[ids]` taking
/// `PR`, no worktree, a detached or unborn `HEAD` exit 2. Git is only read.
fn departure_place(env: &Env, project: &ProjectRoot) -> Result<Place, CliError> {
    if let Some(clash) = prefix_clash(&project.config.scheme) {
        return Err(CliError::spec(format!("{}: {clash}", project.config_label)));
    }
    let git = WorktreeGit::new(&project.root, &process_git(env))
        .map_err(|error| CliError::spec(format!("cannot run git: {error}")))?;
    git.place(&project.root).map_err(|error| {
        CliError::spec(format!(
            "the project root {} cannot be bound to a discrepancy: {error}; nothing changed",
            project.root.display()
        ))
    })
}

fn run_accept(
    env: &Env,
    globals: &Globals,
    request: &CardAcceptRequest,
    consent: Consent<'_>,
) -> Result<CardOutcome, CliError> {
    const ACTION: CardAction = CardAction::Accept;
    let now = checked_now(&request.now)?;
    if let Some(line) = verdict_usage(request) {
        return Ok(CardOutcome::usage(ACTION, line));
    }
    let gap_type = match gap_type_of(request) {
        Ok(gap_type) => gap_type,
        Err(line) => return Ok(CardOutcome::usage(ACTION, &line)),
    };
    let actor = actor_of(Some(&request.actor))?;
    let seen = revision_of(request.revision.as_deref())?;
    let id = written_card(&request.id)?;
    let mut context = open_cards(env, globals)?;
    let card = match named_card(&context, &request.id, id, ACTION)? {
        Ok(card) => card,
        Err(refused) => return Ok(*refused),
    };
    let refuse = |reason: &str, context: &OpenCards, messages: Vec<Message>| {
        Ok(CardOutcome::refused(
            ACTION,
            Some(&card.id),
            Some((&card, &context.config)),
            reason,
            messages,
        ))
    };
    let config = context.config.clone();
    let flow = config.flow(&card.flow);
    let problem = accept_refusal(&card, flow, &actor)
        .or_else(|| {
            seen.filter(|seen| *seen != card.revision)
                .map(|_| stale_refusal(&card))
        })
        .or_else(|| {
            request
                .note
                .as_deref()
                .and_then(|note| over_bytes("note", note, NOTE_MAX))
        });
    if let Some(problem) = problem {
        return refuse(&problem, &context, Vec::new());
    }
    // The card it corrects is frozen again too, so its targets are read.
    let original = card
        .corrects
        .as_deref()
        .map(|original| context.queue.get_card(original))
        .transpose()
        .ok()
        .flatten()
        .flatten();
    let targets: BTreeSet<&String> = card
        .targets
        .iter()
        .chain(original.iter().flat_map(|original| &original.targets))
        .collect();
    let mut messages = Vec::new();
    let departure = if request.departs.is_empty() {
        None
    } else {
        match departing(env, &context, &card, request, gap_type, &targets)? {
            Ok(departure) => Some(departure),
            Err((reason, messages)) => return refuse(&reason, &context, messages),
        }
    };
    let norms = match &departure {
        Some(departure) => departure.norms.clone(),
        None => fresh_norms(env, &context.project, &targets, &mut messages)?,
    };
    let question = format!("accept {} ({}) as {actor}? [y/N]", card.id, card.title);
    if !consent(&escape_controls(&one_line(&question))) {
        return refuse(
            &format!(
                "`{}` not accepted: the answer was not `y`; nothing changed",
                card.id
            ),
            &context,
            messages,
        );
    }
    let accept = CardAccept {
        flow,
        note: request.note.as_deref(),
        norms: &norms,
    };
    let seen = Some(card.revision);
    let accepted = match &departure {
        None => context
            .queue
            .accept_card(&card.id, seen, &accept, &actor, now),
        Some(departure) => context.queue.accept_departing(
            &card.id,
            seen,
            &accept,
            &CardDeparture {
                intake: &departure.intake,
                corpus_hits: &departure.corpus_hits,
                departs: &departure.departs,
            },
            &actor,
            now,
        ),
    };
    let accepted = match accepted {
        Ok(accepted) => accepted,
        Err(error) => return store_refusal(error, ACTION, &card, &context),
    };
    if let Some(departure) = &departure {
        messages.extend(departure.messages.iter().cloned());
        messages.extend(departed_notes(
            &accepted.card.id,
            accepted.intake.as_ref(),
            &departure.corpus_hits,
        ));
    }
    if let Some(original) = &accepted.corrected {
        messages.push(Message::Note(format!(
            "{original}: frozen again by its correction {}",
            accepted.card.id
        )));
    }
    Ok(CardOutcome::done(
        ACTION,
        accepted.card,
        &context.config,
        messages,
    ))
}

/// What a departure says after its commit: the discrepancy, then every hit
/// the item names itself, none hidden, whatever `distinct_from` could hold.
fn departed_notes(
    card: &str,
    result: Option<&IntakeResult>,
    corpus_hits: &[String],
) -> Vec<Message> {
    let Some(result) = result else {
        return Vec::new();
    };
    let made = result.created.as_ref().map_or("-", |item| item.id.as_str());
    let mut lines = vec![format!("{card} departs: its discrepancy is {made}")];
    let hits: Vec<&str> = corpus_hits
        .iter()
        .map(String::as_str)
        .chain(result.hits.iter().map(|hit| hit.id.as_str()))
        .collect();
    if !hits.is_empty() {
        lines.push(one_line(&format!(
            "{made} is distinct from {} hit(s), named by this accept: {}",
            hits.len(),
            hits.join(", ")
        )));
    }
    lines.into_iter().map(Message::Note).collect()
}

/// Why `actor` cannot clear with `reason`: a person's act, under a name and
/// for a reason.
fn clear_problem(actor: &str, reason: &str) -> Option<String> {
    reason_problem(reason, "why the mark clears")
        .or_else(|| clearer_problem(actor).map(|problem| format!("--as: {problem}")))
}

/// `spec card clear C`: a person answers a suspect card, after their consent:
/// no effect freezes each suspect entry again, a needed correction makes a
/// new card and leaves the original's row. Exit 1 on a card not suspect.
pub fn card_clear(
    env: &Env,
    globals: &Globals,
    request: &CardClearRequest,
    consent: Consent<'_>,
) -> Result<CardOutcome, CliError> {
    run_clear(env, globals, request, consent).map_err(escaped_error)
}

fn clear_usage(request: &CardClearRequest) -> Option<&'static str> {
    match request.outcome {
        CardClearOutcome::NeedsCorrection if request.estimate.is_none() => {
            Some("`spec card clear`: --needs-correction takes --estimate E")
        }
        CardClearOutcome::NoEffect if request.estimate.is_some() || request.title.is_some() => {
            Some("`spec card clear`: --no-effect takes no --estimate and no --title")
        }
        _ => None,
    }
}

/// The new card a correction is: the original's targets, flow and discipline,
/// first in that flow; `Err` the refusal.
fn correction_card(
    config: &CardConfig,
    card: &Card,
    request: &CardClearRequest,
    actor: &str,
) -> Result<NewCard, String> {
    let Some(flow) = config.flow(&card.flow) else {
        return Err(format!(
            "{} is in the flow `{}`, which `[card]` no longer holds: no correction stands in \
             it; nothing changed",
            card.id, card.flow
        ));
    };
    let title = request
        .title
        .clone()
        .unwrap_or_else(|| correction_title(&card.title));
    if let Some(problem) = field_problem(
        Some(&title),
        None,
        card.targets.len(),
        &[],
        request.estimate.as_deref(),
        None,
    ) {
        return Err(problem);
    }
    Ok(NewCard {
        flow: flow.name.clone(),
        column: flow.first_column().to_owned(),
        title,
        goal: None,
        discipline: card.discipline.clone(),
        targets: card.targets.clone(),
        criteria: Vec::new(),
        assignee: None,
        reviewer: None,
        mandate: None,
        estimate: request.estimate.clone(),
        due: None,
        parent: None,
        corrects: Some(card.id.clone()),
        by: actor.to_owned(),
    })
}

fn run_clear(
    env: &Env,
    globals: &Globals,
    request: &CardClearRequest,
    consent: Consent<'_>,
) -> Result<CardOutcome, CliError> {
    const ACTION: CardAction = CardAction::Clear;
    let now = checked_now(&request.now)?;
    if let Some(line) = clear_usage(request) {
        return Ok(CardOutcome::usage(ACTION, line));
    }
    let actor = actor_of(request.actor.as_deref())?;
    let seen = revision_of(request.revision.as_deref())?;
    let id = written_card(&request.id)?;
    let mut context = open_cards(env, globals)?;
    let card = match named_card(&context, &request.id, id, ACTION)? {
        Ok(card) => card,
        Err(refused) => return Ok(*refused),
    };
    let refuse = |reason: &str, context: &OpenCards, messages: Vec<Message>| {
        Ok(CardOutcome::refused(
            ACTION,
            Some(&card.id),
            Some((&card, &context.config)),
            reason,
            messages,
        ))
    };
    let mut messages = Vec::new();
    if let Some(problem) = clear_problem(&actor, &request.reason) {
        return refuse(&problem, &context, messages);
    }
    let targets: BTreeSet<&String> = card.targets.iter().collect();
    let norms = fresh_norms(env, &context.project, &targets, &mut messages)?;
    let held = context
        .queue
        .live_correction(&card.id)
        .map_err(queue_cannot)?;
    let suspect = !suspect_on(&card, &norms).is_empty();
    let correcting = request.outcome == CardClearOutcome::NeedsCorrection;
    let problem = clear_refusal(&card, suspect, held.as_deref())
        .or_else(|| {
            seen.filter(|seen| *seen != card.revision)
                .map(|_| stale_refusal(&card))
        })
        .or_else(|| correcting.then(|| correction_refusal(&card)).flatten());
    if let Some(problem) = problem {
        return refuse(&problem, &context, messages);
    }
    let correction = if correcting {
        match correction_card(&context.config, &card, request, &actor) {
            Ok(correction) => Some(correction),
            Err(reason) => return refuse(&reason, &context, messages),
        }
    } else {
        None
    };
    let (clearing, words) = match &correction {
        Some(correction) => (
            CardClearing::NeedsCorrection(correction),
            "needs correction",
        ),
        None => (CardClearing::NoEffect, "no effect"),
    };
    let question = format!(
        "clear {} ({}) as {actor}: {words}? [y/N]",
        card.id, card.title
    );
    if !consent(&escape_controls(&one_line(&question))) {
        return refuse(
            &format!(
                "`{}` not cleared: the answer was not `y`; nothing changed",
                card.id
            ),
            &context,
            messages,
        );
    }
    let clear = CardClear {
        reason: &request.reason,
        norms: &norms,
        actor: &actor,
    };
    let done = context
        .queue
        .clear_card(&card.id, Some(card.revision), &clearing, &clear, now);
    let done = match done {
        Ok(done) => done,
        Err(error) => return store_refusal(error, ACTION, &card, &context),
    };
    if let Some(made) = &done.correction {
        messages.push(Message::Note(format!(
            "{} corrects {}: {}/{}",
            made.id, card.id, made.flow, made.column
        )));
    }
    Ok(CardOutcome::done(
        ACTION,
        done.card,
        &context.config,
        messages,
    ))
}

/// `spec card clear --group REF`: every card suspect on a target in `within`
/// of `REF` or of a document under it by `parent:`, no effect only, one
/// transaction, one event each, after one consent. Only those entries are
/// frozen again: a mark outside the group stays. The read's word decides: a
/// card is taken when a row of `spec trace REF` reads it `suspect`, so one a
/// live correction holds is passed over, and one reading `departed` is too,
/// with a note naming it: it keeps its reading, and a person clears it alone
/// once its discrepancy is settled. Exit 1 when `REF` names nothing or none is
/// suspect.
pub fn card_clear_group(
    env: &Env,
    globals: &Globals,
    request: &CardClearGroupRequest,
    consent: Consent<'_>,
) -> Result<CardGroupOutcome, CliError> {
    run_clear_group(env, globals, request, consent).map_err(escaped_error)
}

/// The first path, by text, of every node of the `targets` the clear
/// re-freezes that `entry` does not cover. `owner` covers all of it.
fn outside_cover(
    entry: &TeamEntry,
    graph: &SpecGraph<'_>,
    places: &BTreeMap<String, Vec<NodeAt>>,
    targets: impl Iterator<Item = String>,
) -> Option<String> {
    let judged: BTreeSet<&str> = targets
        .flat_map(|target| &places[&target])
        .map(|at| graph.paths()[at.file])
        .collect();
    judged
        .into_iter()
        .find(|path| !entry.covers(path))
        .map(str::to_owned)
}

fn run_clear_group(
    env: &Env,
    globals: &Globals,
    request: &CardClearGroupRequest,
    consent: Consent<'_>,
) -> Result<CardGroupOutcome, CliError> {
    let now = checked_now(&request.now)?;
    let actor = actor_of(request.actor.as_deref())?;
    let mut context = open_cards(env, globals)?;
    let group = request.group.trim();
    let mut messages = Vec::new();
    if let Some(problem) = clear_problem(&actor, &request.reason) {
        return Ok(CardGroupOutcome::refused(
            group,
            CardCause::Refused,
            &problem,
            messages,
        ));
    }
    let project = &context.project;
    let scheme = &project.config.scheme;
    let target = match classify(group, scheme, &mut messages, GROUP_REV_IGNORED)? {
        Ok(target) => target,
        Err(reason) => {
            return Ok(CardGroupOutcome::refused(
                group,
                CardCause::Unknown,
                &reason,
                messages,
            ));
        }
    };
    let mut open = open_index(env, project)?;
    let input = indexed_in(&mut open.index, project, &mut messages, false)?;
    let graph = SpecGraph::new(&input, scheme, &project.config.paths);
    let roots = match locate(&graph, &target, group)? {
        Ok(roots) => roots,
        Err(reason) => {
            return Ok(CardGroupOutcome::refused(
                group,
                CardCause::Unknown,
                &reason,
                messages,
            ));
        }
    };
    let mut scope = BTreeSet::new();
    for &root in &roots {
        scope.extend(graph.within(root));
        for document in held_documents(&graph, root) {
            scope.extend(graph.within(document));
        }
    }
    let listed = context.queue.list_cards().map_err(queue_cannot)?;
    messages.extend(
        listed
            .unreadable
            .iter()
            .map(|row| Message::Note(one_line(&format!("{row}; not read")))),
    );
    // What `spec trace` reads, so the words below are its own.
    let mut read = QueueRead::default();
    for card in listed.cards {
        if card.cancellation.is_none() {
            facts_of(&context.queue, &card, &mut read);
            read.cards.push(card);
        }
    }
    messages.extend(read.notes.iter().cloned().map(Message::Note));
    let places = target_places(&graph, read.cards.iter().flat_map(|card| &card.targets));
    let stored = open.index.node_norms().map_err(crate::store_error)?;
    let norms = target_norms(&graph, &stored, &places);
    let inside: BTreeSet<String> = places
        .iter()
        .filter(|(_, nodes)| nodes.iter().any(|node| scope.contains(node)))
        .map(|(target, _)| target.clone())
        .collect();
    let frozen_again = |card: &Card| -> Vec<String> {
        suspect_on(card, &norms)
            .into_iter()
            .filter(|target| inside.contains(*target))
            .map(str::to_owned)
            .collect()
    };
    let marks = group_marks(&graph, &read, &norms, &places, &roots);
    let mut chosen: Vec<&Card> = Vec::new();
    for card in &read.cards {
        let rows = marks.get(&card.id).map_or(&[][..], Vec::as_slice);
        if rows
            .iter()
            .any(|mark| matches!(mark, CardMark::Suspect { .. }))
        {
            chosen.push(card);
        } else if !frozen_again(card).is_empty()
            && let Some(departed) = rows
                .iter()
                .find(|mark| matches!(mark, CardMark::Departed { .. }))
        {
            messages.push(Message::Note(format!(
                "{} reads {}: passed over; clear it alone once that is settled",
                card.id,
                departed.text()
            )));
        }
    }
    if chosen.is_empty() {
        return Ok(CardGroupOutcome::refused(
            group,
            CardCause::Refused,
            &format!("no suspect card under `{group}`: there is no mark to clear; nothing changed"),
            messages,
        ));
    }
    if let Some(entry) = &request.entry {
        let targets = chosen.iter().flat_map(|card| frozen_again(card));
        if let Some(outside) = outside_cover(entry, &graph, &places, targets) {
            let reason = format!(
                "refused: {} leads {}; {outside} lies outside them",
                entry.login,
                entry.zones_text()
            );
            return Ok(CardGroupOutcome::refused(
                group,
                CardCause::Outside,
                &reason,
                messages,
            ));
        }
    }
    if let Some(count) = request.count.filter(|count| *count != chosen.len() as u64) {
        return Ok(CardGroupOutcome::refused(
            group,
            CardCause::Refused,
            &format!(
                "`{group}` holds {} suspect cards, not {count}; nothing changed",
                chosen.len()
            ),
            messages,
        ));
    }
    let cards: Vec<(String, u64)> = chosen
        .iter()
        .map(|card| (card.id.clone(), card.revision))
        .collect();
    let question = format!(
        "clear {} cards of {group} as {actor}: no effect? [y/N]",
        cards.len()
    );
    if !consent(&escape_controls(&one_line(&question))) {
        return Ok(CardGroupOutcome::refused(
            group,
            CardCause::Refused,
            &format!("`{group}` not cleared: the answer was not `y`; nothing changed"),
            messages,
        ));
    }
    let clear = CardClear {
        reason: &request.reason,
        norms: &norms,
        actor: &actor,
    };
    match context.queue.clear_cards(&cards, &inside, &clear, now) {
        Ok(cards) => Ok(CardGroupOutcome {
            group: group.to_owned(),
            cards,
            notes: notes(&messages),
            refusal: None,
            cause: None,
            messages,
        }),
        Err(QueueError::CardRefused { reason, .. }) => Ok(CardGroupOutcome::refused(
            group,
            CardCause::Refused,
            &reason,
            messages,
        )),
        Err(error) => Err(queue_cannot(error)),
    }
}

// -------------------------------------------------------------- text

/// `<verb> C-0001: <flow>/<column>`; `assigned C-0001: <assignee>, reviewer
/// <reviewer>`; nothing when refused.
pub(crate) fn render_text(outcome: &CardOutcome) -> String {
    let (None, Some(card)) = (&outcome.refusal, outcome.card.as_deref()) else {
        return String::new();
    };
    let verb = outcome.action.verb();
    let name = |value: &Option<String>| value.as_deref().unwrap_or(ABSENT).to_owned();
    let line = if outcome.action == CardAction::Assign {
        format!(
            "{verb} {}: {}, reviewer {}\n",
            card.id,
            name(&card.assignee),
            name(&card.reviewer)
        )
    } else {
        format!("{verb} {}: {}/{}\n", card.id, card.flow, card.column)
    };
    escape_controls(&line)
}

/// A split, one line each (task spec `department-split` S3): `split C-0001
/// new|stored: <flow>/<column> | <title> | <targets> | reviewer <name>`,
/// then `subtask C-0002: <flow>/<column> | <discipline> | <targets> | by
/// kind|the task's targets[, suggested]` each, then `note: <note>` each; a
/// dry run `plan`, `-` for each ID, and `dry run: nothing written` last.
/// `-` for none; the body cut at [`crate::OUTPUT_CAP_CHARS`] with a tail.
/// Nothing when refused.
pub(crate) fn render_split_text(outcome: &CardSplitOutcome) -> String {
    let Some(parent) = &outcome.parent else {
        return String::new();
    };
    let id = |id: &Option<String>| id.as_deref().unwrap_or(ABSENT).to_owned();
    let listed = |targets: &[String]| {
        if targets.is_empty() {
            ABSENT.to_owned()
        } else {
            targets.join(", ")
        }
    };
    let mut lines = vec![format!(
        "{} {} {}: {}/{} | {} | {} | reviewer {}",
        if outcome.dry_run { "plan" } else { "split" },
        id(&parent.id),
        if parent.created { "new" } else { "stored" },
        parent.flow,
        parent.column,
        parent.title,
        listed(&parent.targets),
        outcome.reviewer.as_deref().unwrap_or(ABSENT)
    )];
    for child in &outcome.children {
        let from = match child.from {
            TargetsFrom::Kinds => "by kind",
            TargetsFrom::Parent => "the task's targets",
        };
        let mandate = child.mandate.map_or_else(String::new, |mandate| {
            format!(" | mandate {}", mandate.as_str())
        });
        lines.push(format!(
            "subtask {}: {}/{} | {} | {} | {from}{}{mandate}",
            id(&child.id),
            child.flow,
            child.column,
            child.discipline,
            listed(&child.targets),
            if child.suggested { ", suggested" } else { "" }
        ));
    }
    lines.extend(outcome.notes.iter().map(|note| format!("note: {note}")));
    let lines: Vec<String> = lines.iter().map(|line| one_line(line)).collect();
    let last = outcome.dry_run.then_some("dry run: nothing written");
    let reserved = last.map_or(0, |line| line.chars().count() + 1);
    let (shown, _) = lines_within(
        lines.iter().map(|line| line.chars().count() + 1),
        reserved,
        true,
    );
    let mut out = String::new();
    for line in &lines[..shown] {
        out.push_str(line);
        out.push('\n');
    }
    if shown < lines.len() {
        out.push_str(&format!(
            "[truncated: {} of {} lines not shown]\n",
            lines.len() - shown,
            lines.len()
        ));
    }
    if let Some(line) = last {
        out.push_str(line);
        out.push('\n');
    }
    escape_controls(&out)
}

/// `cleared C-0001: <flow>/<column>`, a line per card; nothing when refused.
pub(crate) fn render_group_text(outcome: &CardGroupOutcome) -> String {
    let mut out = String::new();
    for card in &outcome.cards {
        out.push_str(&format!(
            "{} {}: {}/{}\n",
            CardAction::Clear.verb(),
            card.id,
            card.flow,
            card.column
        ));
    }
    escape_controls(&out)
}

/// The `key: value` lines of a card, cut at [`OUTPUT_CAP_CHARS`], then its
/// context's bundle body when one was made; nothing when there is no card.
pub(crate) fn render_show_text(outcome: &CardShowOutcome) -> String {
    let Some(card) = outcome.card.as_deref() else {
        return String::new();
    };
    let lines = show_lines(card, outcome);
    let (shown, _) = lines_within(lines.iter().map(|line| line.chars().count() + 1), 0, false);
    let mut out = String::new();
    for line in &lines[..shown] {
        out.push_str(line);
        out.push('\n');
    }
    if shown < lines.len() {
        out.push_str(&format!(
            "[truncated: {} of {} lines not shown]\n",
            lines.len() - shown,
            lines.len()
        ));
    }
    // The context's body, last, in the room the lines left (task spec
    // `card-context` CC3).
    if let Some(bundle) = outcome.context.as_ref().and_then(made_bundle) {
        out.push_str(&bundle.body);
    }
    escape_controls(&out)
}

/// The context's bundle, when one was made.
fn made_bundle(context: &CardContext) -> Option<&Bundle> {
    context
        .bundle
        .as_ref()
        .and_then(|outcome| outcome.bundle.as_ref())
}

/// `context.parent: <id> | <title>` and `context.parent.goal: <goal>`,
/// each `-` when absent.
fn parent_lines(context: &CardContext) -> [String; 2] {
    let parent = context.parent.as_ref();
    [
        format!(
            "context.parent: {}",
            parent.map_or_else(
                || ABSENT.to_owned(),
                |parent| format!("{} | {}", parent.id, one_line(&parent.title))
            )
        ),
        format!(
            "context.parent.goal: {}",
            parent
                .and_then(|parent| parent.goal.as_deref())
                .map_or_else(|| ABSENT.to_owned(), one_line)
        ),
    ]
}

/// `context.bundle: <bundle_hash>`, `-` when none was made.
fn bundle_line(context: &CardContext) -> String {
    format!(
        "context.bundle: {}",
        made_bundle(context).map_or(ABSENT, |bundle| bundle.bundle_hash.as_str())
    )
}

fn show_lines(card: &Card, outcome: &CardShowOutcome) -> Vec<String> {
    let optional = |value: &Option<String>| value.as_deref().map_or(ABSENT.to_owned(), one_line);
    let mut lines = vec![
        format!("id: {}", card.id),
        format!("flow: {}", card.flow),
        format!("column: {}", card.column),
        format!("position: {}", card.position),
        format!("title: {}", one_line(&card.title)),
        format!("goal: {}", optional(&card.goal)),
        format!("discipline: {}", optional(&card.discipline)),
    ];
    let list = |lines: &mut Vec<String>, key: &str, values: Vec<String>| {
        if values.is_empty() {
            lines.push(format!("{key}: {ABSENT}"));
        }
        lines.extend(values.iter().map(|value| format!("{key}: {value}")));
    };
    list(
        &mut lines,
        "targets",
        card.targets.iter().map(|text| one_line(text)).collect(),
    );
    let mark = outcome.mark.as_ref();
    let held = |text: Option<&str>| text.map_or(ABSENT.to_owned(), one_line);
    lines.extend([
        format!("mark: {}", mark.map_or(ABSENT, CardMark::word)),
        format!("on: {}", held(mark.and_then(CardMark::on))),
        format!(
            "discrepancy: {}",
            held(mark.and_then(CardMark::discrepancy))
        ),
        format!("correction: {}", held(mark.and_then(CardMark::correction))),
    ]);
    // Among the first lines, so the cut never reaches them (task spec
    // `upstream-signal` US-8).
    lines.extend(upstream_lines(outcome.upstream.as_ref()));
    list(
        &mut lines,
        "snapshot",
        card.snapshot
            .iter()
            .flatten()
            .map(|entry| {
                format!(
                    "{} rev {} {}",
                    one_line(&entry.id),
                    entry
                        .rev
                        .map_or_else(|| ABSENT.to_owned(), |rev| rev.to_string()),
                    entry.norm_hash.as_deref().unwrap_or(ABSENT)
                )
            })
            .collect(),
    );
    list(
        &mut lines,
        "criteria",
        card.criteria.iter().map(|text| one_line(text)).collect(),
    );
    lines.extend([
        format!("assignee: {}", optional(&card.assignee)),
        format!("reviewer: {}", optional(&card.reviewer)),
        format!("mandate: {}", card.mandate.map_or(ABSENT, Mandate::as_str)),
        format!("estimate: {}", optional(&card.estimate)),
        format!("due: {}", optional(&card.due)),
        format!("parent: {}", optional(&card.parent)),
        format!("corrects: {}", optional(&card.corrects)),
    ]);
    list(
        &mut lines,
        "evidence",
        card.evidence
            .iter()
            .map(|entry| {
                format!(
                    "{} {} ({}, {})",
                    entry.kind.as_str(),
                    one_line(&entry.value),
                    entry.by,
                    entry.at
                )
            })
            .collect(),
    );
    list(
        &mut lines,
        "tasks",
        outcome
            .tasks
            .iter()
            .map(|task| {
                let stale = if task.stale == Some(true) {
                    " | stale"
                } else {
                    ""
                };
                format!(
                    "{} | {} | {}{stale}",
                    task.id,
                    task.status.map_or(ABSENT, TaskStatus::as_str),
                    held(task.title.as_deref())
                )
            })
            .collect(),
    );
    let item_line = |item: &CardItem| {
        let record = item
            .record_id
            .as_deref()
            .map_or_else(String::new, |record| format!(" | record {record}"));
        // The gist leads, else the text.
        format!(
            "{} | {} | {} | {}{record}",
            item.id,
            item.kind,
            item.status,
            one_line(item.gist.as_deref().unwrap_or(&item.summary))
        )
    };
    list(
        &mut lines,
        "items",
        outcome.items.iter().map(item_line).collect(),
    );
    list(
        &mut lines,
        "subtask_items",
        outcome
            .subtask_items
            .iter()
            .map(|below| format!("{} | {}", below.card_id, item_line(&below.item)))
            .collect(),
    );
    lines.push(format!(
        "acceptance: {}",
        card.acceptance.as_ref().map_or(ABSENT.to_owned(), |done| {
            let departs = done.discrepancy.as_ref().map_or_else(String::new, |item| {
                format!(" departs {} {item}", done.departs.join(", "))
            });
            match &done.note {
                Some(note) => format!("{} {}{departs}: {}", done.by, done.at, one_line(note)),
                None => format!("{} {}{departs}", done.by, done.at),
            }
        })
    ));
    lines.push(format!(
        "cancellation: {}",
        card.cancellation
            .as_ref()
            .map_or(ABSENT.to_owned(), |done| {
                format!("{} {}: {}", done.by, done.at, one_line(&done.reason))
            })
    ));
    lines.extend([
        format!("created_by: {}", card.created_by),
        format!("created_at: {}", card.created_at),
        format!("updated_at: {}", card.updated_at),
        format!("revision: {}", card.revision),
    ]);
    list(
        &mut lines,
        "allowed",
        outcome
            .allowed
            .iter()
            .map(|allowed| match allowed.by.as_slice() {
                [] => allowed.to.clone(),
                by => format!("{} (by {})", allowed.to, by.join(", ")),
            })
            .collect(),
    );
    list(
        &mut lines,
        "history",
        outcome
            .history
            .iter()
            .flatten()
            .map(|event| {
                format!(
                    "{} {} {} {}",
                    event.seq, event.event_type, event.at, event.payload
                )
            })
            .collect(),
    );
    // `context: -`, or its lines, the bundle's line last (task spec
    // `card-context` CC2, CC3).
    match outcome.context.as_ref() {
        None => lines.push(format!("context: {ABSENT}")),
        Some(context) => {
            lines.extend(parent_lines(context));
            lines.push(bundle_line(context));
        }
    }
    lines
}

/// The flow lines, then a line per card printed, then the tail.
pub(crate) fn render_list_text(outcome: &CardListOutcome) -> String {
    let mut out = String::new();
    for line in &outcome.flow_lines {
        out.push_str(line);
        out.push('\n');
    }
    for card in outcome.shown_cards() {
        out.push_str(&card_line(
            card,
            outcome.marks.get(&card.id),
            outcome.upstream.contains(&card.id),
        ));
        out.push('\n');
    }
    let omitted = outcome.omitted();
    if omitted > 0 {
        out.push_str(&format!(
            "[truncated: {omitted} of {} cards not shown]\n",
            outcome.cards.len()
        ));
    }
    escape_controls(&out)
}

#[cfg(test)]
mod context_tests {
    //! Task spec `card-context` CC6, the cap: the selection of a context's
    //! REFs from a card's targets and its own items alone, `context_refs`.
    //! The items are built here, as `run_show` reads them; the integration
    //! half is the crate's `card_context` test.

    use specengine_core::proposal::Author;

    use super::{CardItem, NODES_MAX, context_refs};

    /// An item `PR-<n>` of `status`, holding `record` when given.
    fn item(n: usize, status: &'static str, record: Option<String>) -> CardItem {
        CardItem {
            id: format!("PR-{n:04}"),
            kind: "decision",
            status,
            summary: format!("Decision {n}"),
            gist: None,
            matters: None,
            working_answer: Some("Yes.".to_owned()),
            why: None,
            author: Author::human(),
            created_at: "2026-11-12T09:00:00Z".to_owned(),
            updated_at: "2026-11-12T09:00:00Z".to_owned(),
            staged_at: None,
            decided_by: None,
            decided_at: None,
            decision_note: None,
            record_path: record.as_ref().map(|id| format!("notes/{id}.md")),
            record_title: record.as_ref().map(|id| format!("Record {id}")),
            record_id: record,
        }
    }

    fn record(n: usize) -> String {
        format!("REC-{n:04}")
    }

    fn targets() -> Vec<String> {
        vec!["NODE-A".to_owned(), "NODE-B".to_owned()]
    }

    /// 66 applied records give 64 REFs after the targets, the first 64 by
    /// `items` order, and the note naming the 65th. M: no cap.
    #[test]
    fn sixty_six_records_bundle_sixty_four_and_name_the_first_left_out() {
        // Records numbered against the items' order, so the order kept is
        // `items`', never the records'.
        let items: Vec<CardItem> = (1..=66)
            .map(|n| item(n, "applied", Some(record(200 - n))))
            .collect();
        let (refs, note) = context_refs(&targets(), &items);
        assert_eq!(NODES_MAX, 64);
        assert_eq!(refs.len(), 2 + 64, "M: no cap");
        assert_eq!(refs[..2], targets()[..]);
        let wanted: Vec<String> = (1..=64).map(|n| record(200 - n)).collect();
        assert_eq!(refs[2..], wanted[..]);
        assert_eq!(
            note.as_deref(),
            Some(format!("not bundled: 2 more records, from {}", record(200 - 65)).as_str())
        );
    }

    /// Exactly 64 records: all of them, no note.
    #[test]
    fn sixty_four_records_are_all_bundled() {
        let items: Vec<CardItem> = (1..=64)
            .map(|n| item(n, "applied", Some(record(n))))
            .collect();
        let (refs, note) = context_refs(&targets(), &items);
        assert_eq!(refs.len(), 66);
        assert_eq!(note, None);
    }

    /// Only an applied item holding a record counts: an open one, a staged
    /// one, an approved one holding its record (a killed run's), a rejected
    /// one and an applied one with none are left out; the order is `items`'.
    /// M: open items taken.
    #[test]
    fn only_applied_items_holding_a_record() {
        let items = vec![
            item(1, "open", None),
            item(2, "applied", Some(record(30))),
            item(3, "staged", None),
            item(4, "approved", Some(record(31))),
            item(5, "rejected", None),
            item(6, "applied", None),
            item(7, "applied", Some(record(24))),
        ];
        let (refs, note) = context_refs(&targets(), &items);
        assert_eq!(
            refs,
            ["NODE-A", "NODE-B", "REC-0030", "REC-0024"],
            "M: open items taken"
        );
        assert_eq!(note, None);
        // A manual card with no records: no REF at all.
        assert_eq!(context_refs(&[], &items[..1]), (Vec::new(), None));
    }
}
