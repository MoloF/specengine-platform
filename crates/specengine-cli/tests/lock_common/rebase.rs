//! The `spec lock rebase` helpers of docs/features/lock-rebase.md
//! ("Acceptance criteria", RD-1..RD-7, TE-1..TE-8): the rebase through the
//! library (`lock_rebase`) with a consent closure that records what the
//! owner was shown (no clock: a rebase keeps `verified_at` and `commit`),
//! or the `spec` binary on a pseudo-terminal ([`LockRepo::spec_pty`]);
//! the rebased golden (`fixtures/spec-lock/code-a.rebased.lock`) derived
//! from the two goldens by key, never hand-written; a lock's blocks edited
//! by key; objects written into a scratch repository's database by
//! `git hash-object -w` (a blob no commit holds: a rebase reads each
//! entry's `file_blob` by OID alone).
//!
//! The expected texts are the spec's "Data" ("`spec lock rebase`",
//! "Output", "The commit", "The base"), never the command's own output.

use std::fs;
use std::path::{Path, PathBuf};

use specengine_cli::{
    CliError, Exit, Globals, LockPrompt, LockRebaseRequest, Message, RebaseOutcome, lock_rebase,
};
use specengine_store::GitEnv;

use super::{CODE_RECIPE, DECIDER, LockRepo, Prompted, at, blocks, golden_format_1_text, value};
use crate::common::fixture;

/// The rebased golden's file name under `fixtures/spec-lock/`.
pub const REBASED_GOLDEN: &str = "code-a.rebased.lock";

/// The variable that makes `rebased_golden_is_accepts_with_format_1_commits`
/// write the rebased golden (after `code-a.lock`, as 5b-1a's
/// `SPECENGINE_WRITE_LOCK_GOLDEN`).
pub const WRITE_REBASED_GOLDEN: &str = "SPECENGINE_WRITE_REBASED_GOLDEN";

/// The commit's subject ("The commit", Q35).
pub const SUBJECT: &str = "spec: lock rebase";

/// Step 7's refusal when the lock, `HEAD` or the branch changed during
/// the question.
pub const CHANGED_WHILE_REBASING: &str = "spec.lock changed while being rebased; nothing written";

/// Step 3: no lock to rebase.
pub const NO_LOCK: &str = "no spec.lock: nothing to rebase";

/// Step 4: a lock already of this build's format and recipes.
pub const NOTHING_TO_CHANGE: &str = "nothing to change";

/// Step 4: a lock that is not UTF-8 (`read_lock`'s text).
pub const NOT_UTF8: &str = "spec.lock: not UTF-8: SpecEngine writes it";

/// The format-1 golden's two entries a rebase drops `bad_levels` (Q15,
/// S9): `counter.rs:77`'s `[path, path]`, `bounds.rs:14`'s `[Sig]`.
pub const GOLDEN_DROPS: [(&str, &str, &str); 2] = [
    ("GOAL-01", "verifies", "crates/tally::counter::inner::limit"),
    (
        "cap-check/PASS-01",
        "verifies",
        "crates/tally::test:bounds::steps_by_one",
    ),
];

/// The rebased golden's path.
pub fn rebased_golden_file() -> PathBuf {
    fixture("spec-lock").join(REBASED_GOLDEN)
}

/// The pinned rebased golden's text.
pub fn rebased_golden_text() -> String {
    fs::read_to_string(rebased_golden_file()).unwrap_or_else(|error| {
        panic!(
            "fixtures/spec-lock/{REBASED_GOLDEN}: {error}; write it with \
             {WRITE_REBASED_GOLDEN}=1 cargo nextest run -p specengine-cli --test lock_rebase \
             rebased_golden_is_accepts_with_format_1_commits"
        )
    })
}

/// The format-2 golden's text (`fixtures/spec-lock/code-a.lock`).
pub fn golden_text() -> String {
    fs::read_to_string(fixture("spec-lock").join("code-a.lock"))
        .expect("fixtures/spec-lock/code-a.lock")
}

/// The whole message of a rebase commit ("The commit"): the subject, an
/// empty line, `Lock-mode: rebase`, `Decided-by:`; no `Lock:`,
/// `Proposal:` or caller trailer.
pub fn rebase_message() -> String {
    format!("{SUBJECT}\n\nLock-mode: rebase\nDecided-by: {DECIDER}\n")
}

/// The note of a lock that cannot be cross-checked (Q16, R11), its text
/// after `note: `.
pub fn not_this_builds_note(old_code_recipe: &str) -> String {
    format!("rebase: \"{old_code_recipe}\" is not this build's: located without a hash cross-check")
}

/// "The base" (Q34): the note of a `HEAD` lock judged with the current
/// one, its text after `note: `.
pub fn head_note(format: i64, code_recipe: &str) -> String {
    format!("HEAD's spec.lock is format {format} (\"{code_recipe}\"): judged with the current lock")
}

/// Step 2's refusal of a merge in progress in `worktree` (`Repo::open`).
pub fn merge_text(worktree: &Path) -> String {
    format!(
        "a merge is in progress in {}: finish or abort it first; nothing written",
        worktree.display()
    )
}

/// The question ("Output"): both counts always.
pub fn question(
    from_format: i64,
    from_code_recipe: &str,
    rebased: usize,
    dropped: usize,
    head: &str,
    worktree: &Path,
) -> String {
    format!(
        "rebase spec.lock (format {from_format} -> 2, \"{from_code_recipe}\" -> \
         \"{CODE_RECIPE}\": {rebased} rebased, {dropped} dropped){}",
        at(head, "main", worktree)
    )
}

/// One row, LF-ended: `<change> | <node> | <relation> | <file> | <kind>
/// <symbol>[ <item_kind>][ | <reason>]` (`what` the `<kind> <symbol>[
/// <item_kind>]` part).
pub fn row(
    change: &str,
    node: &str,
    relation: &str,
    file: &str,
    what: &str,
    reason: Option<&str>,
) -> String {
    match reason {
        Some(reason) => format!("{change} | {node} | {relation} | {file} | {what} | {reason}\n"),
        None => format!("{change} | {node} | {relation} | {file} | {what}\n"),
    }
}

/// The format-1 golden's rows rebased at this build, in key order: 9
/// `rebased`, 2 `dropped bad_levels`.
pub fn golden_rows() -> String {
    const COUNTER: &str = "crates/tally/src/counter.rs";
    const BOUNDS: &str = "crates/tally/tests/bounds.rs";
    const LIB: &str = "crates/tally/src/lib.rs";
    const LIMITS: &str = "data/limits.ron";
    [
        row(
            "rebased",
            "GOAL-01",
            "implements",
            COUNTER,
            "item crates/tally::counter::Tally::value function_item",
            None,
        ),
        row(
            "dropped",
            "GOAL-01",
            "verifies",
            COUNTER,
            "item crates/tally::counter::inner::limit function_item",
            Some("bad_levels"),
        ),
        row(
            "rebased",
            "LAW-BOUND",
            "implements",
            COUNTER,
            "item crates/tally::counter::<Tally as fmt::Display>::fmt function_item",
            None,
        ),
        row(
            "rebased",
            "LAW-BOUND",
            "verifies",
            BOUNDS,
            "item crates/tally::test:bounds::stops_at_cap function_item",
            None,
        ),
        row(
            "rebased",
            "LAW-BOUND",
            "configures",
            COUNTER,
            "item crates/tally::counter::CAP const_item",
            None,
        ),
        row(
            "rebased",
            "LAW-BOUND",
            "configures",
            LIMITS,
            "ron root.cap",
            None,
        ),
        row(
            "rebased",
            "PIECE-COUNTER",
            "implements",
            COUNTER,
            "item crates/tally::counter::Tally struct_item",
            None,
        ),
        row(
            "rebased",
            "PIECE-COUNTER",
            "implements",
            LIB,
            "module crates/tally",
            None,
        ),
        row(
            "rebased",
            "PIECE-COUNTER",
            "verifies",
            BOUNDS,
            "module crates/tally::test:bounds",
            None,
        ),
        row(
            "rebased",
            "PIECE-COUNTER",
            "configures",
            LIMITS,
            "ron root",
            None,
        ),
        row(
            "dropped",
            "cap-check/PASS-01",
            "verifies",
            BOUNDS,
            "item crates/tally::test:bounds::steps_by_one function_item",
            Some("bad_levels"),
        ),
    ]
    .concat()
}

// ------------------------------------------------------------- blocks

/// An entry's key ("Entry"): `node`, `relation`, `file`, `kind`,
/// `symbol`, `item_kind`.
pub type Key = (String, String, String, String, String, Option<String>);

/// The key of a block of [`blocks`].
pub fn key_of(block: &[(String, String)]) -> Key {
    let field = |name: &str| {
        value(block, name)
            .unwrap_or_else(|| panic!("no `{name}` in {block:?}"))
            .to_owned()
    };
    (
        field("node"),
        field("relation"),
        field("file"),
        field("kind"),
        field("symbol"),
        value(block, "item_kind").map(str::to_owned),
    )
}

/// A lock's text cut into its preamble (the header and top keys, no final
/// LF) and its blocks (each a block's lines, LF-ended, no `[[binding]]`
/// line).
pub fn chunks(text: &str) -> (String, Vec<String>) {
    let mut parts = text.split("\n[[binding]]\n");
    let preamble = parts.next().unwrap_or_default().to_owned();
    let blocks = parts
        .map(|chunk| format!("{}\n", chunk.trim_end_matches('\n')))
        .collect();
    (preamble, blocks)
}

/// The lock text of `preamble` and `blocks` ([`chunks`]' inverse).
pub fn joined(preamble: &str, blocks: &[String]) -> String {
    let mut out = preamble.to_owned();
    for block in blocks {
        out.push_str("\n[[binding]]\n");
        out.push_str(block);
    }
    out
}

/// `text` with its entries in reverse order (lock-levels.md TE-2's twin).
pub fn reversed(text: &str) -> String {
    let (preamble, mut blocks) = chunks(text);
    blocks.reverse();
    joined(&preamble, &blocks)
}

/// Whether a block's lines name `node`, `relation` and `symbol`.
fn names(block: &str, node: &str, relation: &str, symbol: &str) -> bool {
    let lines: Vec<&str> = block.lines().collect();
    lines.contains(&format!("node = \"{node}\"").as_str())
        && lines.contains(&format!("relation = \"{relation}\"").as_str())
        && lines.contains(&format!("symbol = \"{symbol}\"").as_str())
}

/// `text` with the one block naming `node`, `relation`, `symbol` edited
/// by `edit` (its lines in, its lines out).
pub fn edit_block(
    text: &str,
    (node, relation, symbol): (&str, &str, &str),
    edit: impl FnOnce(&str) -> String,
) -> String {
    let (preamble, mut blocks) = chunks(text);
    let found: Vec<usize> = blocks
        .iter()
        .enumerate()
        .filter(|(_, block)| names(block, node, relation, symbol))
        .map(|(index, _)| index)
        .collect();
    assert_eq!(
        found.len(),
        1,
        "one block {node} {relation} {symbol} in:\n{text}"
    );
    blocks[found[0]] = edit(&blocks[found[0]]);
    joined(&preamble, &blocks)
}

/// `text` with the one block naming the key's `node`, `relation`,
/// `symbol` dropped.
pub fn without_block(text: &str, (node, relation, symbol): (&str, &str, &str)) -> String {
    let (preamble, blocks) = chunks(text);
    let kept: Vec<String> = blocks
        .iter()
        .filter(|block| !names(block, node, relation, symbol))
        .cloned()
        .collect();
    assert_eq!(kept.len() + 1, blocks.len(), "one block {node} {symbol}");
    joined(&preamble, &kept)
}

/// The block's `key = value` line replaced by `key = <written>`
/// (`written` as the lock writes it: a string quoted, an integer bare).
pub fn set_line(block: &str, key: &str, written: &str) -> String {
    let prefix = format!("{key} = ");
    let mut seen = 0;
    let lines: Vec<String> = block
        .lines()
        .map(|line| {
            if line.starts_with(&prefix) {
                seen += 1;
                format!("{prefix}{written}")
            } else {
                line.to_owned()
            }
        })
        .collect();
    assert_eq!(seen, 1, "one `{key}` line in:\n{block}");
    format!("{}\n", lines.join("\n"))
}

/// [`set_line`] of a string value.
pub fn set_string(block: &str, key: &str, value: &str) -> String {
    set_line(block, key, &format!("\"{value}\""))
}

// ------------------------------------------------------ the rebased golden

/// The rebased golden ("Fixtures", R12, Q30): `code-a.lock` (`format_2`)
/// with each entry's `commit` line taken from the same key in
/// `code-a.format-1.lock` (`format_1`); every other byte `format_2`'s.
pub fn derive_rebased_golden(format_2: &str, format_1: &str) -> String {
    let commits: std::collections::BTreeMap<Key, String> = blocks(format_1)
        .iter()
        .map(|block| {
            (
                key_of(block),
                value(block, "commit").expect("a commit").to_owned(),
            )
        })
        .collect();
    let (preamble, chunked) = chunks(format_2);
    let rebased: Vec<String> = chunked
        .iter()
        .map(|chunk| {
            let parsed = blocks(&format!("[[binding]]\n{chunk}"));
            let key = key_of(&parsed[0]);
            let commit = commits
                .get(&key)
                .unwrap_or_else(|| panic!("{key:?}: no format-1 entry"));
            set_string(chunk, "commit", commit)
        })
        .collect();
    joined(&preamble, &rebased)
}

// ---------------------------------------------------------- the attempt

/// One library rebase: its result and every prompt shown.
#[derive(Debug)]
pub struct RebaseAttempt {
    pub outcome: Result<RebaseOutcome, CliError>,
    pub prompts: Vec<Prompted>,
}

impl RebaseAttempt {
    /// Exit 0 with a commit, after exactly one question.
    pub fn committed(&self, context: &str) -> &RebaseOutcome {
        let outcome = self
            .outcome
            .as_ref()
            .unwrap_or_else(|error| panic!("{context}: exit {:?}: {error}", error.exit));
        assert_eq!(outcome.exit(), Exit::Answered, "{context}: {outcome:?}");
        assert!(outcome.refusal.is_none(), "{context}: {outcome:?}");
        assert!(
            outcome.commit.is_some(),
            "{context}: no commit: {outcome:?}"
        );
        assert!(!outcome.unchanged, "{context}: {outcome:?}");
        assert_eq!(self.prompts.len(), 1, "{context}: {:?}", self.prompts);
        outcome
    }

    /// Exit 0, `nothing to change`, no question.
    pub fn unchanged(&self, context: &str) -> &RebaseOutcome {
        let outcome = self
            .outcome
            .as_ref()
            .unwrap_or_else(|error| panic!("{context}: exit {:?}: {error}", error.exit));
        assert_eq!(outcome.exit(), Exit::Answered, "{context}: {outcome:?}");
        assert!(
            outcome.unchanged,
            "{context}: not `nothing to change`: {outcome:?}"
        );
        assert!(outcome.commit.is_none(), "{context}: {outcome:?}");
        assert!(
            self.prompts.is_empty(),
            "{context}: asked {:?}",
            self.prompts
        );
        outcome
    }

    /// Exit 1: the refusal.
    pub fn refused(&self, context: &str) -> String {
        let outcome = self
            .outcome
            .as_ref()
            .unwrap_or_else(|error| panic!("{context}: exit {:?}: {error}", error.exit));
        assert_eq!(outcome.exit(), Exit::NotFound, "{context}: {outcome:?}");
        assert_eq!(outcome.exit().code(), 1, "{context}");
        outcome.refusal.clone().expect("a refusal")
    }

    /// Exit 2: the message.
    pub fn cannot(&self, context: &str) -> String {
        match &self.outcome {
            Ok(outcome) => panic!("{context}: expected exit 2, got {outcome:?}"),
            Err(error) => {
                assert_eq!(error.exit, Exit::CannotRun, "{context}: {error}");
                assert_eq!(error.exit.code(), 2, "{context}");
                error.message.clone()
            }
        }
    }

    /// The rows the owner saw (the one prompt's), else the rows of the
    /// outcome still to print.
    pub fn rows(&self) -> String {
        if let Some(prompt) = self.prompts.first() {
            return prompt.rows.clone();
        }
        self.outcome
            .as_ref()
            .ok()
            .and_then(|outcome| outcome.report.clone())
            .unwrap_or_default()
    }

    /// The one question.
    pub fn question(&self) -> &str {
        &self.prompts.first().expect("a question was asked").question
    }

    /// The `note:` texts of the outcome (exit 0 and 1), in order.
    pub fn notes(&self) -> Vec<String> {
        self.outcome
            .as_ref()
            .map(|outcome| {
                outcome
                    .messages
                    .iter()
                    .filter_map(|message| match message {
                        Message::Note(text) => Some(text.clone()),
                        Message::Warning(_) => None,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl LockRepo {
    /// `fixtures/code-a` committed as `the fixture`, `text` committed on
    /// top as `spec.lock` (`the lock`).
    pub fn with_lock(label: &str, text: &str) -> Self {
        let repo = Self::code_a(label);
        repo.write("spec.lock", text);
        repo.commit_all("the lock");
        repo
    }

    /// [`LockRepo::with_lock`] of the format-1 golden: what a project not
    /// yet rebased holds.
    pub fn format_1_golden(label: &str) -> Self {
        Self::with_lock(label, &golden_format_1_text())
    }

    /// `bytes` written into the object database as a blob no commit holds
    /// (`git hash-object -w --stdin`): its OID.
    pub fn hash_object(&self, bytes: &[u8]) -> String {
        let out = self
            .git
            .git_stdin(&self.root, &["hash-object", "-w", "--stdin"], bytes);
        String::from_utf8(out)
            .expect("an OID")
            .trim_end()
            .to_owned()
    }

    /// Library `lock_rebase` from `cwd` with `git`, `consent` answering
    /// the one question.
    pub fn rebase_with(
        &self,
        cwd: &Path,
        git: GitEnv,
        consent: &mut dyn FnMut(&LockPrompt<'_>) -> bool,
    ) -> Result<RebaseOutcome, CliError> {
        lock_rebase(
            &self.env(cwd),
            &Globals::default(),
            &LockRebaseRequest { git },
            consent,
        )
    }

    /// [`LockRepo::rebase_with`] from `cwd`, `before` run when the question
    /// is shown (a change made while the owner reads it), the answer
    /// `answer`, every prompt recorded.
    pub fn rebase_during(
        &self,
        cwd: &Path,
        answer: bool,
        before: &mut dyn FnMut(),
    ) -> RebaseAttempt {
        let mut prompts = Vec::new();
        let mut consent = |prompt: &LockPrompt<'_>| {
            prompts.push(Prompted {
                rows: prompt.rows.to_owned(),
                question: prompt.question.to_owned(),
            });
            before();
            answer
        };
        let outcome = self.rebase_with(cwd, self.git_env(cwd), &mut consent);
        RebaseAttempt { outcome, prompts }
    }

    /// A rebase from `cwd`, the answer `answer`.
    pub fn rebase_at(&self, cwd: &Path, answer: bool) -> RebaseAttempt {
        self.rebase_during(cwd, answer, &mut || {})
    }

    /// A rebase from the root, the answer `answer`.
    pub fn rebase(&self, answer: bool) -> RebaseAttempt {
        self.rebase_at(&self.root, answer)
    }

    /// A rebase from the root that must commit: the outcome.
    pub fn rebase_ok(&self) -> RebaseOutcome {
        self.rebase(true).committed("rebase").clone()
    }
}
