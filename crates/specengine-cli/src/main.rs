//! `spec`: parses the arguments, runs one command of the library and prints
//! its answer. stdout carries results only (with `--json`, one document for
//! exit 0 and 1, nothing for exit 2 but `spec check`'s cannot-check
//! report); stderr carries `note:`, `warning:` and error lines. No colour,
//! no timing.
//!
//! The one check that stays here: `spec approve`, `spec reject`, `spec
//! import-state`, `spec round answer`, the owner's `spec task
//! approve|changes|cancel` and `spec verify --tests` run only when stdin is
//! a terminal (else exit 2, nothing read or logged, `import-state`'s and
//! `round answer`'s file not opened, no test run); the
//! library asks its question through a callback, which prints it on stderr
//! as `... [y/N] ` and reads one line: only `y` or `yes` consents. The clock
//! (`now`, UTC) and the process's variables for git are passed in as well.
//!
//! `spec gate` runs apart (canon `gate`, "Verdict"): its deadline counts from
//! the start of `main`; it never exits 1 (a usage error, a panic or the
//! deadline is exit 2, `specengine gate: no verdict (…)` on stderr); with
//! `--stdin` it prints nothing on stdout, whatever the verdict.
//!
//! `spec serve` runs apart too (task spec `spec-serve`): the library
//! decides; a running daemon's lines are printed, or this process is
//! replaced by the daemon (`exec`), never started beside it.

use std::ffi::OsString;
use std::io::{BufRead as _, IsTerminal as _, Read as _, Write as _};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use clap::builder::PossibleValuesParser;
use clap::error::{ContextKind, ContextValue, ErrorKind};
use clap::{ArgGroup, ColorChoice, CommandFactory as _, Parser, Subcommand};
use specengine_cli::{
    AmendmentRequest, ApproveFlags, ApproveRequest, BumpRequest, BundleRequest, CheckRequest,
    CheckedTree, CliError, CreateRequest, DecisionRequest, DiscrepancyRequest, DriftRequest, Env,
    Exit, ExportIndexRequest, ExportStateRequest, FollowUp, GATE_DEADLINE, GateInput, GateProgress,
    GateRequest, GateStage, Globals, GraphRequest, INTAKE_INPUT_MAX_BYTES, ImpactRequest,
    ImportStateRequest, InboxRequest, IndexRequest, InitRequest, IntakeSeverity, IntakeSource,
    InterpretRequest, LockAcceptRequest, LockPrompt, LockRebaseRequest, LockRegenRequest, Outcome,
    ProposeRequest, ProposedText, QuestionRequest, RefsRequest, RejectRequest, ResolveRequest,
    ReviewRequest, RoadmapRequest, RoundAnswerRequest, RoundNewRequest, RoundTerminal,
    SearchRequest, Serve, ServeRequest, ShowRequest, SymbolsForm, SymbolsRequest, TEXT_MAX_BYTES,
    TaskClaimRequest, TaskCompleteRequest, TaskDecisionRequest, TaskListRequest, TaskNewRequest,
    TaskPlanRequest, TaskReportRequest, TaskShowRequest, TaskStatus, TreeRequest, UnmappedRequest,
    VerifiedTree, VerifyFormat, VerifyRequest, VerifyTestsRequest, render_json, render_text,
};
use specengine_store::GitEnv;

#[derive(Parser)]
#[command(
    name = "spec",
    version,
    about = "SpecEngine: find and read a project's spec nodes by ID, over an always-fresh index; check its documents; propose changes and apply them as commits",
    color = ColorChoice::Never
)]
struct Cli {
    /// The project root (no walk up from the current directory).
    #[arg(long, global = true, value_name = "DIR")]
    root: Option<PathBuf>,
    /// The config to read instead of `<root>/specengine.toml`; without
    /// --root the root is the current directory.
    #[arg(long, global = true, value_name = "FILE")]
    config: Option<PathBuf>,
    /// One JSON document on stdout (exit 0 and 1; `check`: every verdict).
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create specengine.toml with a `[project] slug` at the project root.
    Init {
        /// The slug to write; default: derived from the directory name.
        #[arg(long, value_name = "S")]
        slug: Option<String>,
    },
    /// Bring the project's index up to date with its files.
    Index {
        /// Re-parse every file.
        #[arg(long)]
        full: bool,
    },
    /// Full-text search over the spec nodes (terms of 3 or more characters).
    Search {
        #[arg(required = true, num_args = 1.., value_name = "QUERY")]
        query: Vec<String>,
        /// Keep only nodes of this kind (repeatable).
        #[arg(long = "kind", value_name = "K")]
        kinds: Vec<String>,
        /// Hits at most, 1 to 200 (default 20).
        #[arg(long, value_name = "N", allow_negative_numbers = true)]
        limit: Option<i64>,
        /// Include Tier 3 (archived) documents.
        #[arg(long)]
        archive: bool,
    },
    /// Print a node by ID, alias, `slug/ID`, `ID#SECTION` or a root-relative `.md` path.
    Show {
        #[arg(value_name = "REF")]
        reference: String,
        /// Also list each node's links, outgoing and incoming, before its text.
        #[arg(long)]
        links: bool,
        /// With --links: links written in Tier 3 (archived) documents too.
        #[arg(long)]
        archive: bool,
        /// Also list each node's code bindings and its sections', each with its state as `verify --node` judges it.
        #[arg(long)]
        bindings: bool,
    },
    /// Print the containment tree (`parent:` and section nesting) from ROOT, or from the roots under `[paths] spec`.
    Tree {
        // Not named `root`: that argument id is the global --root.
        /// Any form `show` takes.
        #[arg(value_name = "ROOT")]
        start: Option<String>,
        /// Levels below the roots at most (0: the roots only).
        #[arg(long, value_name = "N", allow_negative_numbers = true)]
        depth: Option<i64>,
        /// Keep only the lines of this kind (repeatable).
        #[arg(long = "kind", value_name = "K")]
        kinds: Vec<String>,
        /// Include Tier 3 (archived) documents.
        #[arg(long)]
        archive: bool,
    },
    /// Walk the typed links from REF: what it depends on, or with --impact what an edit of it reaches.
    Graph {
        /// Any form `show` takes.
        #[arg(value_name = "REF")]
        reference: String,
        /// Follow the impact table: depends_on, derived_from, verifies, uses_term back, constrains forward.
        #[arg(long)]
        impact: bool,
        /// Follow only this link type (repeatable): outgoing; with --impact in its table direction, else incoming.
        #[arg(long = "type", value_name = "T")]
        types: Vec<String>,
        /// Distance from REF at most; default unbounded.
        #[arg(long, value_name = "N", allow_negative_numbers = true)]
        depth: Option<i64>,
        /// Include links written in Tier 3 (archived) documents.
        #[arg(long)]
        archive: bool,
    },
    /// Show what a change to a node, a code symbol or the code since a revision reaches: nodes, bound code with its sync, tests, open tasks; read-only.
    Impact {
        /// Any form `show` takes.
        #[arg(value_name = "REF")]
        reference: Option<String>,
        /// Looked up as `spec symbols QUERY`.
        #[arg(long, value_name = "QPATH")]
        symbol: Option<String>,
        /// The code changed since this git revision, against the files on disk.
        #[arg(long, value_name = "REV", allow_hyphen_values = true)]
        since: Option<String>,
        /// Distance from REF at most; default unbounded.
        #[arg(long, value_name = "N", allow_negative_numbers = true)]
        depth: Option<i64>,
    },
    /// Assemble the context of REF within a budget of estimated tokens: the targets, open questions, ancestors, criteria, decisions, neighbours, terms; the rest named by ID.
    Bundle {
        /// Any form `show` takes; several are bundled together.
        #[arg(required = true, num_args = 1.., value_name = "REF")]
        references: Vec<String>,
        /// Estimated tokens at most (default: `[budgets] bundle_node`, else 2000).
        #[arg(long, value_name = "N", allow_negative_numbers = true)]
        budget: Option<i64>,
    },
    /// Check the documents against the convention; exit 0 clean or observed, 1 blocked, 2 cannot check.
    Check {
        /// Check what `git commit` would record: the git index, config and baseline from it unless --config, --baseline; judged against HEAD.
        #[arg(long)]
        staged: bool,
        /// Check the working tree as a plain check does, config and baseline from disk; judged against HEAD.
        #[arg(long, conflicts_with = "staged")]
        changed: bool,
        /// The debt baseline to use instead of the root's .spec-debt.toml (relative to the current directory).
        #[arg(long, value_name = "F")]
        baseline: Option<PathBuf>,
        /// List every finding, stale and new debt entry, not only the blocking ones.
        #[arg(long)]
        debt: bool,
    },
    /// List the nodes whose meaning changed against HEAD while their revision is not above HEAD's: `required` when spec.lock vouches for the old text (the check's `rev-not-raised`), else `suggested`, each with the line a raise writes; read-only, exit 0 whatever is listed, 2 when the check against HEAD cannot run.
    Bump {
        /// List what `git commit` would record (the git index), not the working tree.
        #[arg(long)]
        staged: bool,
    },
    /// List the unapplied amendments: the working tree's live documents that name a node they amend (`links: {amends: [ID]}`), each with those links; read-only, exit 0 whatever is listed, 2 when it cannot run (no project, a config error, an incomplete walk).
    Health,
    /// Print the plan of the `[roadmap]` items: releases in order, each one's open items by chain then priority with this repository's open tasks, range sums, the current release, the pool; read-only, exit 0 whatever is listed, 1 when --release names no release, 2 when it cannot run (no project, a config error, no or a bad `[roadmap]`).
    Roadmap {
        /// Only this release's group.
        #[arg(long, value_name = "R", conflicts_with = "pool")]
        release: Option<String>,
        /// Only the pool (items without a release) and the unplaced items.
        #[arg(long)]
        pool: bool,
    },
    /// List what in the code names REF: the markers (`@implements`, `@verifies`, `@configures`, `@assumes`) and comment mentions under `[code]`, each with its state and binding; read-only, exit 0 whatever is listed, 1 when REF names nothing, 2 when it cannot run (no `[code]`, a config error, an incomplete walk).
    Refs {
        /// Any form `show` takes.
        #[arg(value_name = "REF")]
        reference: String,
    },
    /// Find code items in the code index: by QUERY (a qpath, a suffix of one or a name, matched in tiers exact, suffix, name), bound to the nodes REF names (--node: items, modules, RON paths), in a file (--file) or covering a line, innermost first (--at); each with its markers; read-only, exit 0 whatever is listed, 1 when REF names nothing, 2 when it cannot run (no `[code]`, a config error, a PATH the walk does not read, a bad LINE, an incomplete walk).
    #[command(group(ArgGroup::new("form").required(true).args(["query", "node", "file", "at"])))]
    Symbols {
        /// A qpath, a suffix of one or a name; the words are joined by a space (quote a `#2`: a shell reads it as a comment).
        #[arg(value_name = "QUERY")]
        query: Vec<String>,
        /// Symbols at most, 1 to 200 (default 20); QUERY only.
        #[arg(long, value_name = "N", conflicts_with_all = ["node", "file", "at"])]
        limit: Option<usize>,
        /// The items, modules and RON paths bound to the nodes REF names (any form `show` takes).
        #[arg(long, value_name = "REF")]
        node: Option<String>,
        /// A code file's items in source order (root-relative).
        #[arg(long, value_name = "PATH")]
        file: Option<String>,
        /// The items covering LINE of a code file, the shortest span first.
        #[arg(long, value_name = "PATH:LINE")]
        at: Option<String>,
    },
    /// Show how much of the code the spec maps: per directory and code file, the units (items, RON files), those a marker maps (through the item, an enclosing item or inline mod, or the file's module; its ID resolved, an alias or ambiguous), their percent and those a mention reaches; a code file lists its unmapped items; read-only, exit 0, 2 when it cannot run (no `[code]`, a config error, a PATH the walk does not read, an incomplete walk).
    Unmapped {
        /// A `[code]` root, or a directory or code file under one (default: a row per root).
        #[arg(value_name = "PATH")]
        path: Option<String>,
    },
    /// Resolve the code tree's items with rust-analyzer through the `specengine-ra` executable (beside spec or on PATH; one cold load) and store its answers as the worktree's advisory layer C snapshot, outside the project; prints the counts and the run's timings. Exit 0 stored, 2 when it cannot run, the binary is missing, fails, refuses, overruns or is interrupted, or the tree changed during the run (the previous snapshot is kept).
    Resolve {
        /// The run's budget in seconds, 1 to 3600: past it the run's process group is stopped.
        #[arg(long, value_name = "S", default_value_t = specengine_cli::RESOLVE_TIMEOUT_DEFAULT, value_parser = clap::value_parser!(u64).range(specengine_cli::RESOLVE_TIMEOUT_MIN..=specengine_cli::RESOLVE_TIMEOUT_MAX))]
        timeout: u64,
    },
    /// Show drift between spec.lock and the tree: a state per lock entry and marker binding, findings Fix, Check or pre-existing; read-only. Exit 0 clean or observed, 1 blocked (only under `[verify] mode` enforce-introduced or enforce, on a Fix finding), 2 when a binding or the lock cannot be verified, in every mode. With --task T: only the rows of task T's nodes, the verdict and exit theirs. With --node REF: only the rows of REF's bindings, `ok` too, judged over the files the code index picks, the verdict and exit theirs. With --tests: run the tests `@verifies` binds through the project's `[verify] tests` command, on a terminal after one [y/N], over a clean committed tree; exit 0 verified, 1 not verified, 2 refused, interrupted or when it cannot run.
    Verify {
        /// Judge against HEAD too: a finding HEAD already has is pre-existing.
        #[arg(long)]
        changed: bool,
        /// Judge what `git commit` would record (the git index, its config unless --config) against HEAD: a finding HEAD already has is pre-existing.
        #[arg(long, conflicts_with = "changed")]
        staged: bool,
        /// Run the bound tests (`@verifies`) with the project's `[verify] tests` command, after the owner's [y/N] on a terminal; Unix only.
        #[arg(long, conflicts_with_all = ["changed", "staged"])]
        tests: bool,
        /// Only task T's nodes (its targets, criteria and affected nodes, and the sections they contain): the rows of those nodes, or with --tests their bound tests.
        #[arg(long, value_name = "T", conflicts_with = "staged")]
        task: Option<String>,
        /// Only the rows of REF's bindings (an ID, alias, slug/ID, ID#SECTION or root-relative .md path; never its sections'), judged over the files the code index picks.
        #[arg(long, value_name = "REF", conflicts_with_all = ["changed", "staged", "task", "tests"])]
        node: Option<String>,
        /// With --node: only the rows whose symbol is QPATH, byte for byte (a RON value's `<file>#<path>` too; a moved row's old symbol too).
        #[arg(long, value_name = "QPATH", requires = "node")]
        symbol: Option<String>,
    },
    /// Export generated output (`index`: the registered index generator's document; `state`: a backup of the proposal queue).
    Export {
        #[command(subcommand)]
        what: Export,
    },
    /// Propose a change to one node or a new one, ask the owner a question, report a discrepancy, raise a decision taken during work or queue HEAD's lock drift; no file is touched until the owner approves a change.
    Propose {
        #[command(subcommand)]
        kind: Propose,
    },
    /// List the current repository's open and approved proposals, by ID.
    Inbox {
        /// Every state, applied and rejected included.
        #[arg(long)]
        all: bool,
    },
    /// Print one proposal: its target, place, author, rationale, diff, and what approving it now would do (apply it, or complete it by its own commit already on its branch); a question's, a discrepancy's or a decision's fields.
    Review {
        #[arg(value_name = "PR")]
        id: String,
        /// Leave out the texts, the diff and the conflict, list at most 20 introduced findings, and cut the text at 40000 characters.
        #[arg(long)]
        brief: bool,
    },
    /// Apply an open proposal in its worktree as one commit (a question, a discrepancy or a decision: write its decision record, from the project's [decision_records] template, as one new file and one commit), or complete an open or approved one whose own commit is already on its branch, with no new commit; asks for consent on the terminal (completing an approved one does not ask). Without flags, an approve staged from the UI is shown and confirmed with its flags.
    Approve {
        #[arg(value_name = "PR")]
        id: String,
        /// A note kept with the decision (and in a decision record's `note` slot), at most 4096 bytes.
        #[arg(long, value_name = "T")]
        note: Option<String>,
        /// A discrepancy: the owner's chosen option, from 0.
        #[arg(long, value_name = "N")]
        option: Option<u64>,
        /// A question or a decision: the owner's answer, instead of its working answer.
        #[arg(long, value_name = "T")]
        answer: Option<String>,
        /// A question, a discrepancy or a decision: the section its record governs (ID, ID#SECTION or path#anchor), instead of its first ID.
        #[arg(long, value_name = "REF")]
        canon: Option<String>,
        /// An editorial change (wording, no meaning): keep every revision the change would raise, and set the new spec hash on the spec.lock entries that vouched for the old text, in the same commit; a drift item: its node's `spec lock accept --editorial`.
        #[arg(long)]
        editorial: bool,
    },
    /// Reject an open or approved proposal, never one whose `Proposal:` commit is on its branch (a drift item: one completing it); asks for consent on the terminal.
    Reject {
        #[arg(value_name = "PR")]
        id: String,
        /// Why the proposal is rejected (non-empty, at most 4096 bytes); left out, a reject staged from the UI gives it.
        #[arg(long, value_name = "T")]
        reason: Option<String>,
        /// A drift item only: also make a draft task, `code` (change the bound code to match the spec) or `spec` (propose a spec edit describing the code); `spec task approve` makes it ready.
        #[arg(long = "follow-up", value_name = "KIND", value_parser = PossibleValuesParser::new(FollowUp::ALL.map(FollowUp::as_str)))]
        follow_up: Option<String>,
    },
    /// Restore a dump written by `export state` into the project's empty queue, rows as stored; asks for consent on the terminal.
    ImportState {
        /// The dump, relative to the current directory.
        #[arg(value_name = "FILE")]
        file: PathBuf,
    },
    /// A question round: the open questions, discrepancies and decisions on one sheet, answered in one sitting; each returned answer is staged, then asked again on the terminal, one at a time.
    Round {
        #[command(subcommand)]
        command: RoundCommand,
    },
    /// spec.lock, what the owner verified: a node's spec hash and its bound code's hashes at a commit, written only on the owner's terminal.
    Lock {
        #[command(subcommand)]
        command: LockCommand,
    },
    /// Work on the project's tasks: only the owner moves one to `ready` (approve, on a terminal); an agent reads its package, plans, claims, reports and completes it.
    Task {
        #[command(subcommand)]
        command: TaskCommand,
    },
    /// The task gate: may an agent write FILE? Exit 0 pass, 2 refuse or no verdict; only an in_progress task claimed in the file's worktree, on its branch, opens a controlled path.
    #[command(group(ArgGroup::new("input").required(true).args(["path", "stdin"])))]
    Gate {
        /// The session's directory (relative: to the current one); a relative FILE joins it.
        #[arg(long, value_name = "DIR", requires = "path")]
        worktree: Option<PathBuf>,
        /// The file written.
        #[arg(long, value_name = "FILE", requires = "worktree")]
        path: Option<PathBuf>,
        /// Read Claude Code's PreToolUse hook JSON on stdin; nothing is printed on stdout.
        #[arg(long)]
        stdin: bool,
    },
    /// Run the daemon (specengine-http) in the foreground on 127.0.0.1 for the project, or for each DIR; one per data directory: a daemon already serving them is named instead.
    Serve {
        /// A project root to serve, its own specengine.toml read; repeat for more (default: the project found from the current directory, or --root).
        #[arg(value_name = "DIR")]
        dirs: Vec<PathBuf>,
        /// The port on 127.0.0.1; 0 takes a free one.
        #[arg(long, value_name = "N", default_value_t = specengine_cli::DAEMON_PORT)]
        port: u16,
    },
}

#[derive(Subcommand)]
enum RoundCommand {
    /// Print the current repository's open questions, discrepancies and decisions as a Markdown sheet to answer, by severity, waiting tasks and age; writes nothing.
    New {
        /// Items at most, 1 to 200 (default 20).
        #[arg(long, value_name = "N", allow_negative_numbers = true)]
        limit: Option<i64>,
    },
    /// Read a filled sheet or a reply, stage each answer, then ask each item's own approve or reject question on the terminal ([y/N] per answer; any other key keeps it staged).
    #[command(group(ArgGroup::new("answers").required(true).args(["file", "staged"])))]
    Answer {
        /// The filled sheet or a reply, relative to the current directory: `PR-NNNN: <answer>` lines and the `project:` line.
        #[arg(value_name = "FILE")]
        file: Option<PathBuf>,
        /// No file: ask the open questions, discrepancies and decisions already staged, in round order.
        #[arg(long)]
        staged: bool,
    },
}

#[derive(Subcommand)]
enum LockCommand {
    /// Lock REF's marker bindings over HEAD's committed bytes: its revision and spec hash, each bound item's hash, each file's blob; lists the changes, asks [y/N] on the terminal, commits spec.lock alone. Exit 0 committed or nothing to change, 1 refused (a dirty input, text changed at the revision, nothing lockable, not confirmed), 2 cannot run.
    Accept {
        /// Any form `show` takes; a gone node's old ID drops its entries.
        #[arg(value_name = "REF")]
        reference: String,
        /// Only set the spec hash of REF's entries at its revision to its current text's (an editorial change); nothing else.
        #[arg(long)]
        editorial: bool,
    },
    /// After a merge conflicting in spec.lock: merge its stages per entry (no code read, no hash computed), ask [y/N] on the terminal, write the working-tree file; then git add and commit the merge. Exit 0 written, 1 no conflict or not confirmed, 2 cannot run.
    Regen,
    /// Carry spec.lock into this build's format and code recipe: each entry's hashes made again from its own committed blob, its revision, spec hash, blob, commit and date kept; an entry that cannot be made again is dropped and named. Lists the entries, asks [y/N] on the terminal, commits spec.lock alone. Exit 0 committed or nothing to change, 1 refused (a dirty input, no lock, not confirmed, the commit failed or unverified), 2 cannot run.
    Rebase,
}

#[derive(Subcommand)]
enum TaskCommand {
    /// Make a `draft` task on nodes of the spec.
    New {
        /// The nodes it works on, each an ID, `slug/ID` or a root-relative `.md` path: 1 to 64.
        #[arg(long = "nodes", required = true, num_args = 1.., value_name = "REF")]
        nodes: Vec<String>,
        /// A title, at most 256 bytes.
        #[arg(long, value_name = "T")]
        title: Option<String>,
        /// The goal, at most 4096 bytes.
        #[arg(long, value_name = "T")]
        goal: Option<String>,
        /// The making agent's role (any of the three: an agent's task).
        #[arg(long = "author-role", value_name = "R")]
        author_role: Option<String>,
        /// The making agent's model.
        #[arg(long = "author-model", value_name = "M")]
        author_model: Option<String>,
        /// The making agent's run.
        #[arg(long, value_name = "ID")]
        run: Option<String>,
    },
    /// Print a task's brief (`--json`: its package): goal, criteria, targets, assumptions, open proposals, owner notes, plan, spec changes since approval, runs, code bindings, bundle.
    Show {
        #[arg(
            value_name = "T",
            required_unless_present = "next",
            conflicts_with = "next"
        )]
        id: Option<String>,
        /// The first `ready` task of this repository in `spec roadmap`'s order of the items the tasks execute, then by number (tasks executing none last); without `[roadmap]`, the lowest-numbered. Advisory: nothing waits on it.
        #[arg(long)]
        next: bool,
    },
    /// List this repository's tasks by number.
    List {
        /// Only tasks in this state (repeatable).
        #[arg(long = "status", value_name = "S", value_parser = PossibleValuesParser::new(TaskStatus::ALL.map(TaskStatus::as_str)))]
        statuses: Vec<String>,
    },
    /// The owner's approval: freeze the task's nodes from the files of this worktree and make it `ready`; asks for consent on the terminal.
    Approve {
        #[arg(value_name = "T")]
        id: String,
    },
    /// The owner sends a planned task back with a note; asks for consent on the terminal.
    Changes {
        #[arg(value_name = "T")]
        id: String,
        /// What the plan must change, at most 4096 bytes.
        #[arg(long, value_name = "T")]
        note: String,
    },
    /// The owner cancels a task not done yet; its proposals stay; asks for consent on the terminal.
    Cancel {
        #[arg(value_name = "T")]
        id: String,
    },
    /// Submit a plan for the owner's review.
    Plan {
        #[arg(value_name = "T")]
        id: String,
        /// The plan (Markdown): a file, or `-` for stdin; at most 16384 bytes.
        #[arg(long = "plan-file", value_name = "F")]
        plan_file: PathBuf,
        /// A criterion: one reference (an ID, `slug/ID`, a `.md` path) or free text (repeatable, at most 32).
        #[arg(long = "criterion", value_name = "C")]
        criteria: Vec<String>,
        /// A node the work also touches (repeatable, at most 64).
        #[arg(long = "affected", value_name = "REF")]
        affected: Vec<String>,
    },
    /// Claim a `ready` task in a worktree of this repository: a run opens.
    Claim {
        #[arg(value_name = "T")]
        id: String,
        /// The project's own role name.
        #[arg(long, value_name = "R")]
        role: String,
        /// The worktree the work happens in, on a branch.
        #[arg(long, value_name = "DIR")]
        worktree: PathBuf,
    },
    /// Close the task's open run with its outcome; the state is kept.
    Report {
        #[arg(value_name = "T")]
        id: String,
        /// completed, partial, failed or abandoned.
        #[arg(long, value_name = "O")]
        outcome: String,
        /// What the run did, at most 4096 bytes.
        #[arg(long, value_name = "S")]
        summary: String,
        /// A file the run changed (repeatable, at most 256).
        #[arg(long = "changed", value_name = "FILE")]
        changed: Vec<String>,
    },
    /// Complete a task whose run is reported: `done`.
    Complete {
        #[arg(value_name = "T")]
        id: String,
    },
}

#[derive(Subcommand)]
enum Propose {
    /// Replace the text of node TARGET (a section's heading to its end, or a document's whole file).
    Update {
        /// An ID, `slug/ID`, or a root-relative `.md` path (its document: the whole file).
        #[arg(value_name = "TARGET")]
        target: String,
        /// The span hash the text was written against (`spec show`: `span b3:…`, JSON `span_hash`).
        #[arg(long, value_name = "HASH")]
        base: String,
        /// The new text: a file, or `-` for stdin (UTF-8, at most 1 MiB).
        #[arg(long = "text-file", value_name = "F")]
        text_file: PathBuf,
        /// Why; the commit's body when applied.
        #[arg(long, value_name = "T")]
        rationale: String,
        /// The proposing agent's role (any of the three: an agent's proposal).
        #[arg(long = "author-role", value_name = "R")]
        author_role: Option<String>,
        /// The proposing agent's model.
        #[arg(long = "author-model", value_name = "M")]
        author_model: Option<String>,
        /// The proposing agent's run.
        #[arg(long, value_name = "ID")]
        run: Option<String>,
        /// Answer with the brief review: no texts or diff, at most 20 introduced findings, the text cut at 40000 characters.
        #[arg(long)]
        brief: bool,
        /// The task this is raised for (`T-0001`): one of this repository, not done or cancelled.
        #[arg(long, value_name = "T")]
        task: Option<String>,
    },
    /// Add a node: a new spec file at a free `.md` path (no --base), or new `{#ID}` sections below node TARGET (--base its span hash, the text its span with the sections added); the proposer names each new ID.
    Create {
        /// A root-relative `.md` path naming nothing yet (a new file), or an ID, `slug/ID` or `.md` path of a node to add sections to.
        #[arg(value_name = "TARGET")]
        target: String,
        /// New sections only: the span hash the text was written against (`spec show`: `span b3:…`, JSON `span_hash`).
        #[arg(long, value_name = "HASH")]
        base: Option<String>,
        /// The text: the whole new file, or the node's span with its new sections; a file, or `-` for stdin (UTF-8, at most 1 MiB).
        #[arg(long = "text-file", value_name = "F")]
        text_file: PathBuf,
        /// Why; the commit's body when applied.
        #[arg(long, value_name = "T")]
        rationale: String,
        /// The proposing agent's role (any of the three: an agent's proposal).
        #[arg(long = "author-role", value_name = "R")]
        author_role: Option<String>,
        /// The proposing agent's model.
        #[arg(long = "author-model", value_name = "M")]
        author_model: Option<String>,
        /// The proposing agent's run.
        #[arg(long, value_name = "ID")]
        run: Option<String>,
        /// Answer with the brief review: no texts or diff, at most 20 introduced findings, the text cut at 40000 characters.
        #[arg(long)]
        brief: bool,
        /// The task this is raised for (`T-0001`): one of this repository, not done or cancelled.
        #[arg(long, value_name = "T")]
        task: Option<String>,
    },
    /// Replace the text of node TARGET as `update` does, carrying the accepted decision REF into it: how the decision was applied and why interpretation was needed go with it into the commit.
    Interpretation {
        /// An ID, `slug/ID`, or a root-relative `.md` path (its document: the whole file).
        #[arg(value_name = "TARGET")]
        target: String,
        /// The span hash the text was written against (`spec show`: `span b3:…`, JSON `span_hash`).
        #[arg(long, value_name = "HASH")]
        base: String,
        /// The accepted decision it interprets: an ID, `slug/ID` or a root-relative `.md` path of a live `class: decision` document with `status: accepted`, or of a node inside one.
        #[arg(long, value_name = "REF")]
        decision: String,
        /// How the decision was applied (not blank, at most 2048 bytes).
        #[arg(long, value_name = "T")]
        how: String,
        /// Why interpretation was needed (not blank, at most 2048 bytes).
        #[arg(long, value_name = "T")]
        why: String,
        /// The new text: a file, or `-` for stdin (UTF-8, at most 1 MiB).
        #[arg(long = "text-file", value_name = "F")]
        text_file: PathBuf,
        /// Why; the commit's body when applied.
        #[arg(long, value_name = "T")]
        rationale: String,
        /// The proposing agent's role (any of the three: an agent's proposal).
        #[arg(long = "author-role", value_name = "R")]
        author_role: Option<String>,
        /// The proposing agent's model.
        #[arg(long = "author-model", value_name = "M")]
        author_model: Option<String>,
        /// The proposing agent's run.
        #[arg(long, value_name = "ID")]
        run: Option<String>,
        /// Answer with the brief review: no texts or diff, at most 20 introduced findings, the text cut at 40000 characters.
        #[arg(long)]
        brief: bool,
        /// The task this is raised for (`T-0001`): one of this repository, not done or cancelled.
        #[arg(long, value_name = "T")]
        task: Option<String>,
    },
    /// Add a new spec or decision document at a free `.md` path PATH whose front-matter declares `links: {amends: [ID, ...]}` to live nodes: applied as a new file, the amended nodes' files never written; it stays an unapplied amendment until its document goes to Tier 3 by status.
    Amendment {
        /// A root-relative `.md` path naming nothing yet.
        #[arg(value_name = "PATH")]
        target: String,
        /// The whole new file, its front-matter `class: spec` or `class: decision` naming each amended ID (canonical, live) under `links: {amends: [...]}`; a file, or `-` for stdin (UTF-8, at most 1 MiB).
        #[arg(long = "text-file", value_name = "F")]
        text_file: PathBuf,
        /// Why; the commit's body when applied.
        #[arg(long, value_name = "T")]
        rationale: String,
        /// The proposing agent's role (any of the three: an agent's proposal).
        #[arg(long = "author-role", value_name = "R")]
        author_role: Option<String>,
        /// The proposing agent's model.
        #[arg(long = "author-model", value_name = "M")]
        author_model: Option<String>,
        /// The proposing agent's run.
        #[arg(long, value_name = "ID")]
        run: Option<String>,
        /// Answer with the brief review: no texts or diff, at most 20 introduced findings, the text cut at 40000 characters.
        #[arg(long)]
        brief: bool,
        /// The task this is raised for (`T-0001`): one of this repository, not done or cancelled.
        #[arg(long, value_name = "T")]
        task: Option<String>,
    },
    /// Ask the owner a question about nodes, with the answer worked on meanwhile; stored in the queue unless an accepted decision or a queued question or decision already answers it.
    Question {
        /// The nodes the question is about, each an ID, `slug/ID`, or a root-relative `.md` path (its document): 1 to 16.
        #[arg(value_name = "TARGET")]
        node_ids: Vec<String>,
        /// The question.
        #[arg(long, value_name = "T")]
        text: String,
        /// The answer worked on until the owner answers.
        #[arg(long = "working-answer", value_name = "W")]
        working_answer: String,
        /// What another answer would cost.
        #[arg(long = "price-of-other", value_name = "P")]
        price_of_other: String,
        /// How much it matters (default normal); it never blocks anything.
        #[arg(long, value_name = "S", value_parser = PossibleValuesParser::new(IntakeSeverity::ALL.map(IntakeSeverity::as_str)))]
        severity: Option<String>,
        /// A hit (its ID, else its path) this question is distinct from; name every hit to store it anyway.
        #[arg(long = "distinct-from", value_name = "X")]
        distinct_from: Vec<String>,
        /// The asking agent's role (any of the three: an agent's question).
        #[arg(long = "author-role", value_name = "R")]
        author_role: Option<String>,
        /// The asking agent's model.
        #[arg(long = "author-model", value_name = "M")]
        author_model: Option<String>,
        /// The asking agent's run.
        #[arg(long, value_name = "ID")]
        run: Option<String>,
        /// The task this is raised for (`T-0001`): one of this repository, not done or cancelled.
        #[arg(long, value_name = "T")]
        task: Option<String>,
    },
    /// Report a discrepancy between the spec and what was observed, with evidence and priced options; stored in the queue unless an accepted decision or a queued report already covers it.
    Discrepancy {
        /// A JSON document of the report (`node_ids`, `summary`, `gap_type`, `severity`, `evidence`, `options`, `recommendation`, optional `working_answer`, `proposed_patch`, `distinct_from`), or `-` for stdin; at most 8 MiB.
        #[arg(long, value_name = "F")]
        input: PathBuf,
        /// The reporting agent's role (any of the three: an agent's report).
        #[arg(long = "author-role", value_name = "R")]
        author_role: Option<String>,
        /// The reporting agent's model.
        #[arg(long = "author-model", value_name = "M")]
        author_model: Option<String>,
        /// The reporting agent's run.
        #[arg(long, value_name = "ID")]
        run: Option<String>,
        /// The task this is raised for (`T-0001`): one of this repository, not done or cancelled.
        #[arg(long, value_name = "T")]
        task: Option<String>,
    },
    /// Raise a decision taken during work for the owner to confirm and record, with what another answer would cost and, optionally, the rule's new text as a linked update; stored in the queue unless an accepted decision or a queued question or decision already covers it.
    Decision {
        /// A JSON document of the decision (`node_ids`, `text`: what had to be decided, `working_answer`: the decision, `price_of_other`, optional `severity`, `proposed_patch`, `distinct_from`), or `-` for stdin; at most 8 MiB.
        #[arg(long, value_name = "F")]
        input: PathBuf,
        /// The deciding agent's role (any of the three: an agent's decision).
        #[arg(long = "author-role", value_name = "R")]
        author_role: Option<String>,
        /// The deciding agent's model.
        #[arg(long = "author-model", value_name = "M")]
        author_model: Option<String>,
        /// The deciding agent's run.
        #[arg(long, value_name = "ID")]
        run: Option<String>,
        /// The task this is raised for (`T-0001`): one of this repository, not done or cancelled.
        #[arg(long, value_name = "T")]
        task: Option<String>,
    },
    /// Queue the lock drift of HEAD's commit for the owner, one item per node whose spec.lock pair no longer holds (spec verify's findings), its evidence computed from the commit, never typed; approving one runs the node's `spec lock accept`. Exit 0 whatever was stored, 1 refused, 2 when it cannot run.
    Drift {
        /// The nodes to judge, each an ID, `slug/ID`, `ID#SECTION` or a root-relative `.md` path at HEAD, or a gone ID with spec.lock entries (at most 16); none: every drifting node. A named node is queued also when never locked.
        #[arg(value_name = "REF")]
        references: Vec<String>,
        /// The raising agent's role (any of the three: an agent's item).
        #[arg(long = "author-role", value_name = "R")]
        author_role: Option<String>,
        /// The raising agent's model.
        #[arg(long = "author-model", value_name = "M")]
        author_model: Option<String>,
        /// The raising agent's run.
        #[arg(long, value_name = "ID")]
        run: Option<String>,
        /// The task this is raised for (`T-0001`): one of this repository, not done or cancelled.
        #[arg(long, value_name = "T")]
        task: Option<String>,
    },
}

#[derive(Subcommand)]
enum Export {
    /// Render the outputs of the `[[generators]]` entry with `index = true` and write them: `[paths] index`, its shards and its roadmap view.
    Index {
        /// Print the render instead of writing it.
        #[arg(long, conflicts_with = "json")]
        stdout: bool,
    },
    /// Write the project's whole proposal queue (every row of `proposals` and `events`) as a JSONL dump outside the repository, to restore with `import-state`.
    State {
        /// The new file to write, relative to the current directory (default: `<data dir>/backups/<slug>-<UTC time>.jsonl`).
        #[arg(long, value_name = "PATH")]
        out: Option<PathBuf>,
    },
}

fn main() -> ExitCode {
    let started = Instant::now();
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) if error.use_stderr() && is_gate(std::env::args_os()) => {
            return gate_usage(&error);
        }
        Err(error) if is_stdout_json_conflict(&error) => {
            return usage_error(&stdout_json_conflict());
        }
        Err(error) => return usage_error(&error),
    };
    if let Command::Gate {
        worktree,
        path,
        stdin,
    } = cli.command
    {
        let globals = cli.root.is_some() || cli.config.is_some();
        return run_gate(started, globals, cli.json, worktree, path, stdin);
    }
    if let Command::Serve { dirs, port } = cli.command {
        let globals = Globals {
            root: cli.root,
            config: cli.config,
        };
        return run_serve(&globals, cli.json, dirs, port);
    }
    // `conflicts_with` sees `--json` only after `index`: clap fills global
    // arguments in after the subcommand's conflict check.
    if cli.json
        && matches!(
            cli.command,
            Command::Export {
                what: Export::Index { stdout: true }
            }
        )
    {
        return usage_error(&stdout_json_conflict());
    }
    let globals = Globals {
        root: cli.root,
        config: cli.config,
    };
    let result = Env::from_process().and_then(|env| run(&env, &globals, cli.command, cli.json));
    match result {
        Ok(outcome) => {
            let mut stderr = std::io::stderr().lock();
            for line in outcome.stderr_lines() {
                let _ = writeln!(stderr, "{line}");
            }
            let stdout = if cli.json {
                render_json(&outcome)
            } else {
                render_text(&outcome)
            };
            // A closed stdout (`| head`) is no failure of the command.
            let _ = std::io::stdout().lock().write_all(stdout.as_bytes());
            ExitCode::from(outcome.exit().code())
        }
        Err(error) => {
            let _ = writeln!(std::io::stderr().lock(), "{error}");
            ExitCode::from(error.exit.code())
        }
    }
}

fn run(env: &Env, globals: &Globals, command: Command, json: bool) -> Result<Outcome, CliError> {
    Ok(match command {
        Command::Init { slug } => {
            Outcome::Init(specengine_cli::init(env, globals, &InitRequest { slug })?)
        }
        Command::Index { full } => {
            Outcome::Index(specengine_cli::index(env, globals, &IndexRequest { full })?)
        }
        Command::Search {
            query,
            kinds,
            limit,
            archive,
        } => Outcome::Search(specengine_cli::search(
            env,
            globals,
            &SearchRequest {
                terms: query,
                kinds,
                limit,
                archive,
            },
        )?),
        Command::Show {
            reference,
            links,
            archive,
            bindings,
        } => Outcome::Show(specengine_cli::show(
            env,
            globals,
            &ShowRequest {
                reference,
                links,
                archive,
                bindings,
                git: process_git(env),
            },
        )?),
        Command::Tree {
            start,
            depth,
            kinds,
            archive,
        } => Outcome::Tree(specengine_cli::tree(
            env,
            globals,
            &TreeRequest {
                root: start,
                depth,
                kinds,
                archive,
            },
        )?),
        Command::Graph {
            reference,
            impact,
            types,
            depth,
            archive,
        } => Outcome::Graph(specengine_cli::graph(
            env,
            globals,
            &GraphRequest {
                reference,
                impact,
                types,
                depth,
                archive,
            },
        )?),
        Command::Impact {
            reference,
            symbol,
            since,
            depth,
        } => Outcome::Impact(Box::new(specengine_cli::impact(
            env,
            globals,
            &ImpactRequest {
                reference,
                symbol,
                since,
                depth,
                git: process_git(env),
            },
        )?)),
        Command::Bundle { references, budget } => Outcome::Bundle(specengine_cli::bundle(
            env,
            globals,
            &BundleRequest {
                references,
                budget,
                git: process_git(env),
            },
        )?),
        Command::Check {
            staged,
            changed,
            baseline,
            debt,
        } => {
            // Git's children see the process's variables; relative `GIT_*`
            // paths resolve against the current directory.
            let git = || GitEnv::new(env.cwd.clone(), std::env::vars_os());
            // clap refuses `--staged` with `--changed`.
            let tree = if staged {
                CheckedTree::Staged(git())
            } else if changed {
                CheckedTree::Changed(git())
            } else {
                CheckedTree::WorkingTree
            };
            Outcome::Check(specengine_cli::check(
                env,
                globals,
                &CheckRequest {
                    tree,
                    baseline,
                    debt,
                    json,
                },
            )?)
        }
        Command::Health => Outcome::Health(specengine_cli::health(env, globals)?),
        Command::Roadmap { release, pool } => Outcome::Roadmap(Box::new(specengine_cli::roadmap(
            env,
            globals,
            &RoadmapRequest {
                release,
                pool,
                git: process_git(env),
            },
        )?)),
        Command::Refs { reference } => Outcome::Refs(Box::new(specengine_cli::refs(
            env,
            globals,
            &RefsRequest { reference },
        )?)),
        Command::Symbols {
            query,
            limit,
            node,
            file,
            at,
        } => {
            // clap's group and `--limit`'s conflicts leave exactly one form.
            let form = match (node, file, at) {
                (Some(reference), _, _) => SymbolsForm::Node(reference),
                (_, Some(path), _) => SymbolsForm::File(path),
                (_, _, Some(raw)) => SymbolsForm::At(raw),
                (None, None, None) => SymbolsForm::Query {
                    terms: query,
                    limit,
                },
            };
            Outcome::Symbols(Box::new(specengine_cli::symbols(
                env,
                globals,
                &SymbolsRequest { form },
            )?))
        }
        Command::Unmapped { path } => Outcome::Unmapped(Box::new(specengine_cli::unmapped(
            env,
            globals,
            &UnmappedRequest { path },
        )?)),
        Command::Resolve { timeout } => Outcome::Resolve(Box::new(specengine_cli::resolve(
            env,
            globals,
            &ResolveRequest {
                timeout,
                spec_path: std::env::current_exe().ok(),
                path: std::env::var_os("PATH"),
            },
        )?)),
        Command::Verify {
            tests: true, task, ..
        } => {
            // Off Unix the library refuses before anything; on it, the
            // terminal before any config, task or code read.
            if cfg!(unix) {
                require_terminal("verify --tests")?;
            }
            let mut consent = |text: &str| ask(text);
            Outcome::VerifyTests(Box::new(specengine_cli::verify_tests(
                env,
                globals,
                &VerifyTestsRequest {
                    task,
                    now: now(),
                    git: process_git(env),
                },
                &mut consent,
            )?))
        }
        Command::Verify {
            changed,
            staged,
            task,
            node,
            symbol,
            ..
        } => {
            // clap refuses `--staged` with `--changed` or `--task`, `--node`
            // with either, `--task` or `--tests`, `--symbol` without `--node`.
            let tree = if staged {
                VerifiedTree::Staged
            } else if changed {
                VerifiedTree::Changed
            } else {
                VerifiedTree::WorkingTree
            };
            Outcome::Verify(Box::new(specengine_cli::verify_as(
                env,
                globals,
                &VerifyRequest {
                    tree,
                    task,
                    node,
                    symbol,
                    git: process_git(env),
                },
                if json {
                    VerifyFormat::Json
                } else {
                    VerifyFormat::Text
                },
            )?))
        }
        Command::Bump { staged } => Outcome::Bump(Box::new(specengine_cli::bump(
            env,
            globals,
            &BumpRequest {
                staged,
                git: process_git(env),
            },
        )?)),
        Command::Export {
            what: Export::Index { stdout },
        } => Outcome::Export(specengine_cli::export_index(
            env,
            globals,
            &ExportIndexRequest { stdout },
        )?),
        Command::Export {
            what: Export::State { out },
        } => Outcome::StateExport(specengine_cli::export_state(
            env,
            globals,
            &ExportStateRequest {
                out,
                now: now(),
                git: process_git(env),
            },
        )?),
        Command::Propose {
            kind:
                Propose::Update {
                    target,
                    base,
                    text_file,
                    rationale,
                    author_role,
                    author_model,
                    run,
                    brief,
                    task,
                },
        } => {
            let text = if text_file.as_os_str() == "-" {
                ProposedText::Given(read_stdin(TEXT_MAX_BYTES, "the text")?)
            } else {
                ProposedText::File(text_file)
            };
            let request = ProposeRequest {
                target,
                base,
                text,
                rationale,
                author_role,
                author_model,
                run,
                now: now(),
                git: process_git(env),
            };
            let task = task.as_deref();
            Outcome::Proposal(Box::new(if brief {
                specengine_cli::propose_brief_with_task(env, globals, &request, task)?
            } else {
                specengine_cli::propose_with_task(env, globals, &request, task)?
            }))
        }
        Command::Propose {
            kind:
                Propose::Create {
                    target,
                    base,
                    text_file,
                    rationale,
                    author_role,
                    author_model,
                    run,
                    brief,
                    task,
                },
        } => {
            let text = if text_file.as_os_str() == "-" {
                ProposedText::Given(read_stdin(TEXT_MAX_BYTES, "the text")?)
            } else {
                ProposedText::File(text_file)
            };
            let request = CreateRequest {
                target,
                base,
                text,
                rationale,
                author_role,
                author_model,
                run,
                now: now(),
                git: process_git(env),
            };
            let task = task.as_deref();
            Outcome::Proposal(Box::new(if brief {
                specengine_cli::propose_create_brief_with_task(env, globals, &request, task)?
            } else {
                specengine_cli::propose_create_with_task(env, globals, &request, task)?
            }))
        }
        Command::Propose {
            kind:
                Propose::Interpretation {
                    target,
                    base,
                    decision,
                    how,
                    why,
                    text_file,
                    rationale,
                    author_role,
                    author_model,
                    run,
                    brief,
                    task,
                },
        } => {
            let text = if text_file.as_os_str() == "-" {
                ProposedText::Given(read_stdin(TEXT_MAX_BYTES, "the text")?)
            } else {
                ProposedText::File(text_file)
            };
            let request = InterpretRequest {
                target,
                base,
                decision,
                how,
                why,
                text,
                rationale,
                author_role,
                author_model,
                run,
                now: now(),
                git: process_git(env),
            };
            let task = task.as_deref();
            Outcome::Proposal(Box::new(if brief {
                specengine_cli::propose_interpretation_brief(env, globals, &request, task)?
            } else {
                specengine_cli::propose_interpretation(env, globals, &request, task)?
            }))
        }
        Command::Propose {
            kind:
                Propose::Amendment {
                    target,
                    text_file,
                    rationale,
                    author_role,
                    author_model,
                    run,
                    brief,
                    task,
                },
        } => {
            let text = if text_file.as_os_str() == "-" {
                ProposedText::Given(read_stdin(TEXT_MAX_BYTES, "the text")?)
            } else {
                ProposedText::File(text_file)
            };
            let request = AmendmentRequest {
                target,
                text,
                rationale,
                author_role,
                author_model,
                run,
                now: now(),
                git: process_git(env),
            };
            let task = task.as_deref();
            Outcome::Proposal(Box::new(if brief {
                specengine_cli::propose_amendment_brief(env, globals, &request, task)?
            } else {
                specengine_cli::propose_amendment(env, globals, &request, task)?
            }))
        }
        Command::Propose {
            kind:
                Propose::Question {
                    node_ids,
                    text,
                    working_answer,
                    price_of_other,
                    severity,
                    distinct_from,
                    author_role,
                    author_model,
                    run,
                    task,
                },
        } => Outcome::Intake(Box::new(specengine_cli::propose_question_with_task(
            env,
            globals,
            &QuestionRequest {
                node_ids,
                text,
                working_answer,
                price_of_other,
                // clap took only a listed value.
                severity: severity.as_deref().and_then(IntakeSeverity::parse),
                distinct_from,
                author_role,
                author_model,
                run,
                now: now(),
                git: process_git(env),
            },
            task.as_deref(),
        )?)),
        Command::Propose {
            kind:
                Propose::Discrepancy {
                    input,
                    author_role,
                    author_model,
                    run,
                    task,
                },
        } => {
            let source = if input.as_os_str() == "-" {
                IntakeSource::Given(read_stdin(INTAKE_INPUT_MAX_BYTES, "the input")?)
            } else {
                IntakeSource::File(input)
            };
            let input = specengine_cli::read_discrepancy_input(env, &source)?;
            Outcome::Intake(Box::new(specengine_cli::propose_discrepancy_with_task(
                env,
                globals,
                &DiscrepancyRequest {
                    input,
                    author_role,
                    author_model,
                    run,
                    now: now(),
                    git: process_git(env),
                },
                task.as_deref(),
            )?))
        }
        Command::Propose {
            kind:
                Propose::Decision {
                    input,
                    author_role,
                    author_model,
                    run,
                    task,
                },
        } => {
            let source = if input.as_os_str() == "-" {
                IntakeSource::Given(read_stdin(INTAKE_INPUT_MAX_BYTES, "the input")?)
            } else {
                IntakeSource::File(input)
            };
            let input = specengine_cli::read_decision_input(env, &source)?;
            Outcome::Intake(Box::new(specengine_cli::propose_decision_with_task(
                env,
                globals,
                &DecisionRequest {
                    input,
                    author_role,
                    author_model,
                    run,
                    now: now(),
                    git: process_git(env),
                },
                task.as_deref(),
            )?))
        }
        Command::Propose {
            kind:
                Propose::Drift {
                    references,
                    author_role,
                    author_model,
                    run,
                    task,
                },
        } => Outcome::Drift(Box::new(specengine_cli::propose_drift(
            env,
            globals,
            &DriftRequest {
                references,
                author_role,
                author_model,
                run,
                now: now(),
                git: process_git(env),
            },
            task.as_deref(),
        )?)),
        Command::Inbox { all } => Outcome::Inbox(specengine_cli::inbox(
            env,
            globals,
            &InboxRequest {
                all,
                git: process_git(env),
            },
        )?),
        Command::Review { id, brief } => {
            let request = ReviewRequest {
                id,
                git: process_git(env),
            };
            Outcome::Proposal(Box::new(if brief {
                specengine_cli::review_brief(env, globals, &request)?
            } else {
                specengine_cli::review(env, globals, &request)?
            }))
        }
        Command::Approve {
            id,
            note,
            option,
            answer,
            canon,
            editorial,
        } => {
            require_terminal("approve")?;
            let mut consent = ask;
            let approve = if editorial {
                specengine_cli::approve_editorial
            } else {
                specengine_cli::approve_with
            };
            Outcome::Proposal(Box::new(approve(
                env,
                globals,
                &ApproveRequest {
                    id,
                    note,
                    now: now(),
                    git: process_git(env),
                },
                &ApproveFlags {
                    option,
                    answer,
                    canon,
                },
                &mut consent,
            )?))
        }
        Command::Reject {
            id,
            reason,
            follow_up,
        } => {
            require_terminal("reject")?;
            // Clap's possible values already refuse another one (exit 2).
            let follow_up = follow_up.as_deref().and_then(FollowUp::parse);
            let mut consent = ask;
            let request = RejectRequest {
                id,
                reason,
                now: now(),
                git: process_git(env),
            };
            Outcome::Proposal(Box::new(match follow_up {
                Some(follow_up) => specengine_cli::reject_follow_up(
                    env,
                    globals,
                    &request,
                    follow_up,
                    &mut consent,
                )?,
                None => specengine_cli::reject(env, globals, &request, &mut consent)?,
            }))
        }
        Command::ImportState { file } => {
            require_terminal("import-state")?;
            let mut consent = ask;
            Outcome::StateImport(specengine_cli::import_state(
                env,
                globals,
                &ImportStateRequest { file },
                &mut consent,
            )?)
        }
        Command::Round {
            command: RoundCommand::New { limit },
        } => Outcome::RoundSheet(Box::new(specengine_cli::round_new(
            env,
            globals,
            &RoundNewRequest {
                limit,
                git: process_git(env),
            },
        )?)),
        Command::Round {
            command: RoundCommand::Answer { file, staged: _ },
        } => {
            // Before the file is opened: agents never stage through it
            // (canon `round`, "Run").
            require_terminal("round answer")?;
            let mut terminal = Console { json };
            Outcome::Round(Box::new(specengine_cli::round_answer(
                env,
                globals,
                &RoundAnswerRequest {
                    file,
                    now: now(),
                    git: process_git(env),
                },
                &mut terminal,
            )?))
        }
        Command::Lock {
            command:
                LockCommand::Accept {
                    reference,
                    editorial,
                },
        } => {
            // Before anything is read: the lock is the owner's alone.
            require_terminal("lock accept")?;
            let mut consent = |prompt: &LockPrompt<'_>| show_and_ask(prompt, json);
            Outcome::Lock(Box::new(specengine_cli::lock_accept(
                env,
                globals,
                &LockAcceptRequest {
                    reference,
                    editorial,
                    now: now(),
                    git: process_git(env),
                    proposal: None,
                    branch: None,
                },
                &mut consent,
                &[],
            )?))
        }
        Command::Lock {
            command: LockCommand::Regen,
        } => {
            require_terminal("lock regen")?;
            let mut consent = |prompt: &LockPrompt<'_>| show_and_ask(prompt, json);
            Outcome::LockRegen(Box::new(specengine_cli::lock_regen(
                env,
                globals,
                &LockRegenRequest {
                    git: process_git(env),
                },
                &mut consent,
            )?))
        }
        Command::Lock {
            command: LockCommand::Rebase,
        } => {
            // Before anything is read: the lock is the owner's alone.
            require_terminal("lock rebase")?;
            let mut consent = |prompt: &LockPrompt<'_>| show_and_ask(prompt, json);
            Outcome::LockRebase(Box::new(specengine_cli::lock_rebase(
                env,
                globals,
                &LockRebaseRequest {
                    git: process_git(env),
                },
                &mut consent,
            )?))
        }
        Command::Task { command } => run_task(env, globals, command)?,
        // Run apart, before the environment is read (`run_gate`).
        Command::Gate { .. } => {
            return Err(CliError::cannot(
                "spec: gate runs apart from the other commands",
            ));
        }
        Command::Serve { .. } => {
            return Err(CliError::cannot(
                "spec: serve runs apart from the other commands",
            ));
        }
    })
}

/// `spec gate`: the verdict within [`GATE_DEADLINE`] of `started`; a
/// panic or a stall past it (a blocked file system) is exit 2 too. With
/// `--stdin`, stdout stays empty.
fn run_gate(
    started: Instant,
    globals: bool,
    json: bool,
    worktree: Option<PathBuf>,
    path: Option<PathBuf>,
    stdin: bool,
) -> ExitCode {
    if globals {
        return gate_usage_line(
            "--root and --config are not taken by `gate`: it finds the config from the file",
        );
    }
    if json && stdin {
        return gate_usage_line("--json is not taken with --stdin: the hook reads no stdout");
    }
    let deadline = started + GATE_DEADLINE;
    let env = Env {
        cwd: std::env::current_dir().unwrap_or_default(),
        home: std::env::var_os("HOME"),
        xdg_data_home: std::env::var_os("XDG_DATA_HOME"),
    };
    // The session's project: `CLAUDE_PROJECT_DIR`, else the current
    // directory.
    let session = std::env::var_os("CLAUDE_PROJECT_DIR")
        .filter(|dir| !dir.is_empty())
        .map(|dir| env.cwd.join(dir))
        .or_else(|| env.cwd.is_absolute().then(|| env.cwd.clone()));
    let progress = GateProgress::new();
    // A panic, and a step blocked where no deadline reaches (a hung file
    // system), still end in a verdict by what the gate knew by then.
    {
        let progress = progress.clone();
        let session = session.clone();
        std::panic::set_hook(Box::new(move |info| {
            let what = info.to_string().replace(['\n', '\r'], " ");
            give_up(
                &progress,
                session.as_deref(),
                format!("spec failed: {what}"),
            );
        }));
    }
    {
        let progress = progress.clone();
        let session = session.clone();
        let backstop = deadline + Duration::from_millis(400);
        let _ = std::thread::Builder::new().spawn(move || {
            std::thread::sleep(backstop.saturating_duration_since(Instant::now()));
            give_up(
                &progress,
                session.as_deref(),
                format!("no answer within {} s", GATE_DEADLINE.as_secs()),
            );
        });
    }
    let input = match (stdin, worktree, path) {
        (true, _, _) => specengine_cli::read_hook_input(std::io::stdin(), deadline),
        (false, Some(dir), Some(path)) => GateInput::Path { dir, path },
        // clap's group and `requires` leave no other case.
        (false, _, _) => {
            return gate_usage_line("give --stdin, or --worktree DIR with --path FILE");
        }
    };
    let request = GateRequest {
        input,
        session,
        git: process_git(&env),
        now: now(),
        deadline,
        progress,
    };
    let outcome = specengine_cli::gate(&env, &request);
    {
        let mut stderr = std::io::stderr().lock();
        for line in outcome.stderr_lines() {
            let _ = writeln!(stderr, "{line}");
        }
    }
    if !stdin {
        let text = if json {
            outcome.to_json()
        } else {
            outcome.stdout_text()
        };
        let mut stdout = std::io::stdout().lock();
        let _ = stdout.write_all(text.as_bytes());
        let _ = stdout.flush();
    }
    ExitCode::from(outcome.exit_code())
}

/// Ends `spec gate` past its deadline or on a panic, by the stage the
/// verdict reached: open passes, `observe` passes with a note, closed is no
/// verdict; nothing read yet: the session's project decides
/// ([`specengine_cli::gate_session`]).
fn give_up(progress: &GateProgress, session: Option<&std::path::Path>, cause: String) -> ! {
    let mut stderr = std::io::stderr().lock();
    let code = match progress.stage() {
        GateStage::Open => 0,
        GateStage::Observe => {
            let _ = writeln!(
                stderr,
                "note: specengine gate (observe): no verdict ({cause}); the write goes on, \
                 nothing recorded"
            );
            0
        }
        GateStage::Closed => {
            let _ = writeln!(
                stderr,
                "specengine gate: no verdict ({cause}): the write is refused."
            );
            Exit::CannotRun.code()
        }
        GateStage::Unknown => {
            let outcome = specengine_cli::gate_session(session, cause, progress);
            for line in outcome.stderr_lines() {
                let _ = writeln!(stderr, "{line}");
            }
            outcome.exit_code()
        }
    };
    let _ = stderr.flush();
    std::process::exit(i32::from(code));
}

/// `spec serve` (task spec `spec-serve`): the library's decision, then
/// either the running daemon's lines (stdout, exit 0) or the exec of the
/// daemon, after which its output and exits are the daemon's. Unix only;
/// `--json` refused first (the daemon prints lines). No runtime, thread or
/// socket here.
fn run_serve(globals: &Globals, json: bool, dirs: Vec<PathBuf>, port: u16) -> ExitCode {
    if !cfg!(unix) {
        return serve_error(&CliError::cannot(
            "spec serve: not supported on this platform",
        ));
    }
    if json {
        return serve_error(&CliError::cannot(
            "spec serve: --json is not supported: the daemon prints lines",
        ));
    }
    let request = ServeRequest {
        dirs,
        port,
        spec_path: std::env::current_exe().ok(),
        path: std::env::var_os("PATH"),
    };
    match Env::from_process().and_then(|env| specengine_cli::serve(&env, globals, &request)) {
        Ok(Serve::Running(lines)) => {
            let _ = std::io::stdout().lock().write_all(lines.as_bytes());
            ExitCode::SUCCESS
        }
        Ok(Serve::Start { program, args }) => exec(&program, &args),
        Err(error) => serve_error(&error),
    }
}

fn serve_error(error: &CliError) -> ExitCode {
    let _ = writeln!(std::io::stderr().lock(), "{error}");
    ExitCode::from(error.exit.code())
}

/// Replaces this process with `program`; returns only when that fails
/// (exit 2 naming the path and the OS error).
#[cfg(unix)]
fn exec(program: &std::path::Path, args: &[OsString]) -> ExitCode {
    use std::os::unix::process::CommandExt as _;
    let error = std::process::Command::new(program).args(args).exec();
    serve_error(&CliError::cannot(format!(
        "spec serve: cannot run {}: {error}",
        program.display()
    )))
}

#[cfg(not(unix))]
fn exec(_program: &std::path::Path, _args: &[OsString]) -> ExitCode {
    serve_error(&CliError::cannot(
        "spec serve: not supported on this platform",
    ))
}

/// The arguments name the `gate` subcommand: its name is the first
/// argument past the global options.
fn is_gate(args: impl Iterator<Item = OsString>) -> bool {
    let mut args = args.skip(1);
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--root" | "--config") => {
                args.next();
            }
            Some(flag)
                if flag == "--json"
                    || flag.starts_with("--root=")
                    || flag.starts_with("--config=") => {}
            Some(name) => return name == "gate",
            None => return false,
        }
    }
    false
}

/// A usage error of `spec gate`: no verdict, exit 2, one line; an unknown
/// argument hints at a `spec` older or newer than the plugin.
fn gate_usage(error: &clap::Error) -> ExitCode {
    let rendered = error.render().to_string();
    // The message up to its first blank line (clap's usage block follows).
    let first = rendered
        .lines()
        .map(str::trim)
        .take_while(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
        .trim_start_matches("error: ")
        .to_owned();
    let skew = matches!(
        error.kind(),
        ErrorKind::UnknownArgument | ErrorKind::InvalidSubcommand | ErrorKind::InvalidValue
    );
    let _ = writeln!(
        std::io::stderr().lock(),
        "specengine gate: no verdict (usage: {first}): the write is refused.{}",
        if skew {
            " Reinstall spec and the plugin from one commit."
        } else {
            ""
        }
    );
    ExitCode::from(Exit::CannotRun.code())
}

fn gate_usage_line(message: &str) -> ExitCode {
    let _ = writeln!(
        std::io::stderr().lock(),
        "specengine gate: no verdict (usage: {message}): the write is refused."
    );
    ExitCode::from(Exit::CannotRun.code())
}

/// `spec task …`; the owner's three run only on a terminal.
fn run_task(env: &Env, globals: &Globals, command: TaskCommand) -> Result<Outcome, CliError> {
    let task = |outcome| Ok(Outcome::Task(Box::new(outcome)));
    match command {
        TaskCommand::New {
            nodes,
            title,
            goal,
            author_role,
            author_model,
            run,
        } => task(specengine_cli::task_new(
            env,
            globals,
            &TaskNewRequest {
                nodes,
                title,
                goal,
                author_role,
                author_model,
                run,
                now: now(),
                git: process_git(env),
            },
        )?),
        TaskCommand::Show { id, next } => {
            Ok(Outcome::TaskShow(Box::new(specengine_cli::task_show(
                env,
                globals,
                &TaskShowRequest {
                    id,
                    next,
                    git: process_git(env),
                },
            )?)))
        }
        TaskCommand::List { statuses } => Ok(Outcome::TaskList(specengine_cli::task_list(
            env,
            globals,
            &TaskListRequest {
                // clap took only listed states.
                statuses: statuses
                    .iter()
                    .filter_map(|status| TaskStatus::parse(status))
                    .collect(),
                git: process_git(env),
            },
        )?)),
        TaskCommand::Approve { id } => {
            require_terminal("task approve")?;
            let mut consent = ask;
            task(specengine_cli::task_approve(
                env,
                globals,
                &decision(env, id, None),
                &mut consent,
            )?)
        }
        TaskCommand::Changes { id, note } => {
            require_terminal("task changes")?;
            let mut consent = ask;
            task(specengine_cli::task_changes(
                env,
                globals,
                &decision(env, id, Some(note)),
                &mut consent,
            )?)
        }
        TaskCommand::Cancel { id } => {
            require_terminal("task cancel")?;
            let mut consent = ask;
            task(specengine_cli::task_cancel(
                env,
                globals,
                &decision(env, id, None),
                &mut consent,
            )?)
        }
        TaskCommand::Plan {
            id,
            plan_file,
            criteria,
            affected,
        } => {
            let plan = if plan_file.as_os_str() == "-" {
                ProposedText::Given(read_stdin(TEXT_MAX_BYTES, "the plan")?)
            } else {
                ProposedText::File(plan_file)
            };
            task(specengine_cli::task_plan(
                env,
                globals,
                &TaskPlanRequest {
                    id,
                    plan,
                    criteria,
                    affected,
                    now: now(),
                    git: process_git(env),
                },
            )?)
        }
        TaskCommand::Claim { id, role, worktree } => task(specengine_cli::task_claim(
            env,
            globals,
            &TaskClaimRequest {
                id,
                role,
                worktree,
                now: now(),
                git: process_git(env),
            },
        )?),
        TaskCommand::Report {
            id,
            outcome,
            summary,
            changed,
        } => task(specengine_cli::task_report(
            env,
            globals,
            &TaskReportRequest {
                id,
                outcome,
                summary,
                changed_files: changed,
                now: now(),
                git: process_git(env),
            },
        )?),
        TaskCommand::Complete { id } => task(specengine_cli::task_complete(
            env,
            globals,
            &TaskCompleteRequest {
                id,
                now: now(),
                git: process_git(env),
            },
        )?),
    }
}

/// An owner's task request.
fn decision(env: &Env, id: String, note: Option<String>) -> TaskDecisionRequest {
    TaskDecisionRequest {
        id,
        note,
        now: now(),
        git: process_git(env),
    }
}

/// The process's directory and variables, for git (the library drops the
/// local `GIT_*` ones before running it in a proposal's worktree).
fn process_git(env: &Env) -> GitEnv {
    specengine_cli::process_git(env)
}

/// The clock: now, UTC, `YYYY-MM-DDTHH:MM:SSZ`.
fn now() -> String {
    specengine_cli::utc_now()
}

/// `--text-file -`, `--input -`: stdin, at most one byte more than the
/// library takes (`max`).
fn read_stdin(max: usize, what: &str) -> Result<Vec<u8>, CliError> {
    let mut bytes = Vec::new();
    std::io::stdin()
        .lock()
        .take(max as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            CliError::cannot(format!("spec: cannot read {what} from stdin: {error}"))
        })?;
    Ok(bytes)
}

/// `spec approve`, `spec reject`, `spec import-state`, `spec round answer`,
/// the owner's `spec task approve|changes|cancel` and `spec verify --tests`
/// decide on a terminal only: a stdin that is no terminal (a pipe, an agent's shell) exits 2
/// before anything is read or logged.
fn require_terminal(command: &str) -> Result<(), CliError> {
    if std::io::stdin().is_terminal() {
        return Ok(());
    }
    Err(CliError::cannot(format!(
        "spec: `spec {command}` asks the owner for consent on a terminal, and stdin is not one \
         (a pipe, a script or an agent's shell): run it in a terminal; nothing changed"
    )))
}

/// The consent prompt: `question` on stderr, one answer line from the
/// terminal; only `y` or `yes` consents.
fn ask(question: &str) -> bool {
    let mut stderr = std::io::stderr().lock();
    let _ = write!(stderr, "{question} ");
    let _ = stderr.flush();
    drop(stderr);
    let mut answer = String::new();
    match std::io::stdin().lock().read_line(&mut answer) {
        Ok(_) => matches!(answer.trim(), "y" | "yes"),
        Err(_) => false,
    }
}

/// `spec lock`'s consent: the rows on stdout (text only; `--json` prints
/// one document at the end), then the question on stderr and one answer
/// line from the terminal; only `y` or `yes` consents.
fn show_and_ask(prompt: &LockPrompt<'_>, json: bool) -> bool {
    if !json {
        let mut stdout = std::io::stdout().lock();
        let _ = stdout.write_all(prompt.rows.as_bytes());
        let _ = stdout.flush();
    }
    ask(prompt.question)
}

/// `spec round answer`'s terminal: the report on stdout before the first
/// question (text only; `--json` prints one document after the walk),
/// each question on stderr with one answer line read from the terminal
/// (only `y` or `yes` consents; the end of input ends the walk), the
/// walk's lines on stderr.
struct Console {
    json: bool,
}

impl RoundTerminal for Console {
    fn report(&mut self, text: &str) {
        if self.json {
            return;
        }
        let mut stdout = std::io::stdout().lock();
        let _ = stdout.write_all(text.as_bytes());
        let _ = stdout.flush();
    }

    fn ask(&mut self, question: &str) -> Option<bool> {
        let mut stderr = std::io::stderr().lock();
        let _ = write!(stderr, "{question} ");
        let _ = stderr.flush();
        drop(stderr);
        let mut answer = String::new();
        match std::io::stdin().lock().read_line(&mut answer) {
            Ok(0) | Err(_) => {
                let _ = writeln!(std::io::stderr().lock());
                None
            }
            Ok(_) => Some(matches!(answer.trim(), "y" | "yes")),
        }
    }

    fn tell(&mut self, line: &str) {
        let _ = writeln!(std::io::stderr().lock(), "{line}");
    }
}

/// Help and version go to stdout, exit 0; any other parse error is a usage
/// error on stderr, `spec: …`, exit 2.
fn usage_error(error: &clap::Error) -> ExitCode {
    if !error.use_stderr() {
        let _ = error.print();
        return ExitCode::SUCCESS;
    }
    let rendered = error.render().to_string();
    let message = rendered.strip_prefix("error: ").unwrap_or(&rendered);
    let _ = writeln!(std::io::stderr().lock(), "spec: {}", message.trim_end());
    ExitCode::from(Exit::CannotRun.code())
}

/// `export index --stdout` with `--json`, in any order and whatever else is
/// passed: one usage error, the same text, its usage block `export index`'s.
fn stdout_json_conflict() -> clap::Error {
    const MESSAGE: &str = "the argument '--stdout' cannot be used with '--json'";
    let mut command = Cli::command();
    command.build();
    let index = command
        .find_subcommand_mut("export")
        .and_then(|export| export.find_subcommand_mut("index"));
    match index {
        Some(index) => index.error(ErrorKind::ArgumentConflict, MESSAGE),
        None => Cli::command().error(ErrorKind::ArgumentConflict, MESSAGE),
    }
}

/// clap's own conflict of `--stdout` and `--json` (either named first).
fn is_stdout_json_conflict(error: &clap::Error) -> bool {
    if error.kind() != ErrorKind::ArgumentConflict {
        return false;
    }
    let names = |kind| match error.get(kind) {
        Some(ContextValue::String(name)) => vec![name.as_str()],
        Some(ContextValue::Strings(names)) => names.iter().map(String::as_str).collect(),
        _ => Vec::new(),
    };
    let mut named = names(ContextKind::InvalidArg);
    named.extend(names(ContextKind::PriorArg));
    let has = |flag: &str| {
        named
            .iter()
            .any(|name| name.split_whitespace().next() == Some(flag))
    };
    has("--stdout") && has("--json")
}
