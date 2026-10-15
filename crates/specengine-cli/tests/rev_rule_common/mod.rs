//! Helpers of the docs/features/rev-rule.md tests ("Acceptance criteria",
//! RD-1..RD-5 and TE-1..TE-9): scratch repositories of real fixtures made
//! at test time through `lock_common` and `verify_common` (every git
//! process sandboxed, fixed dates, a scratch `HOME`), the `spec` binary run
//! with the sandbox's variables (stdin null), and the texts the spec's
//! "Data" gives ("The rule `rev-not-raised`", "spec bump") built here,
//! never from the command's own output.
//!
//! - `fixtures/code-a` with its golden lock (`fixtures/spec-lock/code-a.lock`)
//!   committed (`verify_common::golden`): `GOAL-01` (a document, revision 1
//!   unwritten, two entries), `LAW-BOUND` (a section at `rev=2`, four),
//!   `PIECE-COUNTER` (a document at `rev: 3`, four) and the feature-scoped
//!   `cap-check/PASS-01` (one) are locked; `LAW-STEP`, `LAW-ODD` and
//!   `PIECE-LIMITS`' nodes are not.
//! - `fixtures/verify-ecs` (an ECS-style game crate) with its four nodes
//!   accepted (`verify_common::ecs`).
//!
//! Only the rev-rule test binaries include this module (`mod
//! rev_rule_common;`, after `mod common;` and `mod lock_common;`).

#![allow(dead_code)]

use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;

use serde_json::Value;
use specengine_cli::{Globals, ShowRequest, show};

use crate::common::{Run, SPEC, spec_with};
use crate::lock_common::{LockRepo, blocks, value};

/// The rule's one code (`CHECK_CODES` 36).
pub const CODE: &str = "rev-not-raised";

/// code-a's documents.
pub const COUNTER_MD: &str = "docs/spec/counter.md";
pub const GOAL_MD: &str = "docs/records/GOAL-01.md";
pub const CAP_MD: &str = "docs/features/cap-check.md";
pub const LIMITS_MD: &str = "docs/spec/limits.md";
pub const LOCK: &str = "spec.lock";

/// code-a's locked nodes, as their lock keys name them.
pub const GOAL: &str = "GOAL-01";
pub const BOUND: &str = "LAW-BOUND";
pub const PIECE: &str = "PIECE-COUNTER";
pub const PASS: &str = "cap-check/PASS-01";
/// code-a's unlocked nodes.
pub const STEP: &str = "LAW-STEP";
pub const ODD: &str = "LAW-ODD";

/// One-line meaning edits of code-a's nodes (each a change of the node's
/// own text, its line count kept).
pub const BOUND_EDIT: (&str, &str) = (
    "The count never exceeds the cap.",
    "The count never goes past the cap.",
);
pub const GOAL_EDIT: (&str, &str) = (
    "A caller may step as often as it likes;",
    "A caller may step as often as it wants;",
);
pub const PIECE_EDIT: (&str, &str) = ("grows one step at a time", "grows a single step at a time");
pub const PASS_EDIT: (&str, &str) = ("it reads 10.", "it still reads 10.");
pub const STEP_EDIT: (&str, &str) = ("Each step adds one.", "Each step adds exactly one.");
pub const ODD_EDIT: (&str, &str) = (
    "A revision written but unreadable:",
    "A revision written yet unreadable:",
);

/// The lines the edited nodes are reported at: a section's heading, a
/// document's `id:` line ("The rule `rev-not-raised`").
pub const BOUND_LINE: u64 = 13;
pub const STEP_LINE: u64 = 17;
pub const ODD_LINE: u64 = 22;
pub const PIECE_LINE: u64 = 3;
pub const GOAL_LINE: u64 = 3;
pub const PASS_LINE: u64 = 13;

/// verify-ecs's spec document and its edits.
pub const STAMINA_MD: &str = "docs/spec/stamina.md";
pub const REGEN_EDIT: (&str, &str) = (
    "Stamina regains a fixed amount per second while the runner rests.",
    "Stamina regains a fixed amount each second while the runner rests.",
);
pub const STAMINA_EDIT: (&str, &str) = (
    "A runner spends stamina to sprint and regains it at rest.",
    "A runner spends stamina to sprint and regains it while resting.",
);
/// verify-ecs's locked document and one of its locked sections.
pub const STAMINA_DOC: &str = "PERK-STAMINA";
pub const REGEN: &str = "PERK-REGEN";

/// The largest revision (`REV_MAX`, nine digits).
pub const MAX_REV: u32 = 999_999_999;

/// A `[classes]` table closing `decision` to the keys code-a's `GOAL-01`
/// and verify-ecs's documents write, `rev` not among them
/// (`RaiseRefusal::ClosedClass` at apply).
pub const CLOSED_DECISION: &str = "\n[classes]\ndecision = { required = [\"class\", \"id\"], \
                                   optional = [\"owner\", \"reviewed\"], closed = true }\n";

/// The written form of a raise to `to`: `rev: N` for a document, `rev=N`
/// for a section (as the raised line writes it).
pub fn written(document: bool, to: u32) -> String {
    if document {
        format!("rev: {to}")
    } else {
        format!("rev={to}")
    }
}

/// The finding's message for `id` at `HEAD`'s revision `rev`, raised to
/// `to` ("The rule `rev-not-raised`").
pub fn message(id: &str, document: bool, rev: u32, to: u32) -> String {
    format!(
        "`{id}` changed at rev {rev}, which spec.lock vouches for: raise it ({}), or have the \
         owner approve it through the queue with --editorial",
        written(document, to)
    )
}

/// The finding's text line when it blocks.
pub fn error_line(path: &str, line: u64, id: &str, document: bool, rev: u32, to: u32) -> String {
    format!(
        "error  {path}:{line}: {CODE}: {}",
        message(id, document, rev, to)
    )
}

// ------------------------------------------------------------ processes

/// The variables of a `spec` process of `repo`: the sandbox's, `HOME` its
/// own, then `extra` (replacing a sandbox variable of the same name).
pub fn spec_vars(repo: &LockRepo, extra: &[(&str, &OsStr)]) -> Vec<(OsString, OsString)> {
    let mut vars: Vec<(OsString, OsString)> = repo
        .git
        .vars()
        .into_iter()
        .filter(|(name, _)| {
            name != "HOME" && !extra.iter().any(|(given, _)| name.as_os_str() == *given)
        })
        .collect();
    vars.push(("HOME".into(), repo.home.clone().into_os_string()));
    for (name, value) in extra {
        vars.push(((*name).into(), (*value).to_owned()));
    }
    vars
}

/// `spec args` in `cwd` with [`spec_vars`], stdin null.
pub fn spec_at(repo: &LockRepo, cwd: &Path, args: &[&str], extra: &[(&str, &OsStr)]) -> Run {
    let vars = spec_vars(repo, extra);
    let env: Vec<(&str, &OsStr)> = vars
        .iter()
        .map(|(name, value)| (name.to_str().expect("UTF-8 name"), value.as_os_str()))
        .collect();
    spec_with(cwd, args, &env)
}

/// `git add -A` at the root.
pub fn stage_all(repo: &LockRepo) {
    repo.git.add_all(&repo.root);
}

/// One `spec check <flags>` run, text and `--json`.
pub struct Check {
    pub text: Run,
    pub json_run: Run,
    pub json: Value,
}

impl Check {
    /// The exit code, the same for both forms.
    pub fn code(&self) -> i32 {
        self.text.code
    }

    pub fn verdict(&self) -> &str {
        self.json["verdict"].as_str().expect("a verdict")
    }

    pub fn show(&self) -> String {
        format!("{}\n{}", self.text.show(), self.json_run.show())
    }

    /// The rule's findings, in the report's order.
    pub fn rev(&self) -> Vec<&Value> {
        self.json["findings"]
            .as_array()
            .expect("findings")
            .iter()
            .filter(|finding| finding["code"] == CODE)
            .collect()
    }

    /// The rule's subjects, in the report's order.
    pub fn subjects(&self) -> Vec<String> {
        self.rev()
            .into_iter()
            .map(|finding| finding["subject"].as_str().expect("a subject").to_owned())
            .collect()
    }

    /// The one finding of the rule naming `id`.
    pub fn finding(&self, id: &str) -> &Value {
        let found: Vec<&Value> = self
            .rev()
            .into_iter()
            .filter(|finding| finding["subject"] == id)
            .collect();
        assert_eq!(found.len(), 1, "{id}:\n{}", self.show());
        found[0]
    }

    /// Every finding of another code, as printed in the JSON.
    pub fn others(&self) -> Vec<&Value> {
        self.json["findings"]
            .as_array()
            .expect("findings")
            .iter()
            .filter(|finding| finding["code"] != CODE)
            .collect()
    }

    /// The causes, each `path: message`.
    pub fn causes(&self) -> Vec<String> {
        self.json["cannot_check"]
            .as_array()
            .expect("cannot_check")
            .iter()
            .map(|cause| {
                format!(
                    "{}: {}",
                    cause["path"].as_str().unwrap_or_default(),
                    cause["message"].as_str().unwrap_or_default()
                )
            })
            .collect()
    }

    /// The text's lines.
    pub fn lines(&self) -> Vec<&str> {
        self.text.stdout.lines().collect()
    }
}

/// `spec check <flags>` and `spec --json check <flags>` from `cwd` with
/// `extra` variables: both exit alike, the JSON one document.
pub fn check_at(repo: &LockRepo, cwd: &Path, flags: &[&str], extra: &[(&str, &OsStr)]) -> Check {
    let mut args = vec!["check"];
    args.extend_from_slice(flags);
    let text = spec_at(repo, cwd, &args, extra);
    let mut json_args = vec!["--json"];
    json_args.extend_from_slice(&args);
    let json_run = spec_at(repo, cwd, &json_args, extra);
    assert_eq!(
        text.code,
        json_run.code,
        "text and JSON exit alike:\n{}\n{}",
        text.show(),
        json_run.show()
    );
    let json = json_run.json();
    Check {
        text,
        json_run,
        json,
    }
}

/// `spec check --staged` from the root.
pub fn staged(repo: &LockRepo) -> Check {
    check_at(repo, &repo.root, &["--staged"], &[])
}

/// `spec check --changed` from the root.
pub fn changed(repo: &LockRepo) -> Check {
    check_at(repo, &repo.root, &["--changed"], &[])
}

/// A plain `spec check` from the root.
pub fn plain(repo: &LockRepo) -> Check {
    check_at(repo, &repo.root, &[], &[])
}

/// The rule's finding for `id` as "The rule `rev-not-raised`" gives it:
/// its code, severity, path, line, subject, message and `fix` (the raised
/// line, its ending aside), always introduced; when it blocks, its text
/// line too.
#[allow(clippy::too_many_arguments)]
pub fn assert_finding(
    check: &Check,
    path: &str,
    line: u64,
    id: &str,
    document: bool,
    rev: u32,
    to: u32,
    fix: &str,
) {
    let finding = check.finding(id);
    let context = check.show();
    assert_eq!(finding["code"], CODE, "{context}");
    assert_eq!(finding["severity"], "error", "{context}");
    assert_eq!(finding["path"], path, "{context}");
    assert_eq!(finding["line"], line, "{context}");
    assert_eq!(finding["subject"], id, "{context}");
    assert_eq!(
        finding["message"].as_str(),
        Some(message(id, document, rev, to).as_str()),
        "{context}"
    );
    assert_eq!(
        finding["fix"]["text"]
            .as_str()
            .map(|text| text.trim_end_matches(['\n', '\r'])),
        Some(fix),
        "{context}"
    );
    assert_eq!(finding["introduced"], true, "always introduced: {context}");
    if check.code() == 1 {
        let wanted = error_line(path, line, id, document, rev, to);
        assert!(
            check.lines().contains(&wanted.as_str()),
            "{wanted:?} not printed:\n{context}"
        );
    }
}

// ------------------------------------------------------------- the lock

/// The `spec_hash` the lock text holds on `node`'s first block.
pub fn lock_hash(text: &str, node: &str) -> String {
    blocks(text)
        .into_iter()
        .find(|block| value(block, "node") == Some(node))
        .and_then(|block| value(&block, "spec_hash").map(str::to_owned))
        .unwrap_or_else(|| panic!("no block of {node}"))
}

/// `text` with the `key = …` line of `node`'s blocks (only its `only`-th,
/// 0-based in file order, when given) set to `key = literal`.
pub fn with_field(text: &str, node: &str, only: Option<usize>, key: &str, literal: &str) -> String {
    let head = format!("[[binding]]\nnode = \"{node}\"\n");
    let line_start = format!("\n{key} = ");
    let mut out = String::new();
    let mut rest = text;
    let mut seen = 0;
    let mut set = 0;
    while let Some(at) = rest.find(&head) {
        out.push_str(&rest[..at]);
        let block = &rest[at..];
        let line = block
            .find(&line_start)
            .unwrap_or_else(|| panic!("no `{key}` in a block of {node}"))
            + 1;
        let end = line + block[line..].find('\n').expect("its line end");
        if only.is_none_or(|wanted| wanted == seen) {
            out.push_str(&block[..line]);
            out.push_str(&format!("{key} = {literal}"));
            set += 1;
        } else {
            out.push_str(&block[..end]);
        }
        rest = &block[end..];
        seen += 1;
    }
    out.push_str(rest);
    assert!(set > 0, "no block of {node} (only {only:?}) in the lock");
    out
}

/// `text` with every `spec_hash` of `node` (or its `only`-th block) set to
/// `hash`.
pub fn with_spec_hash(text: &str, node: &str, only: Option<usize>, hash: &str) -> String {
    with_field(text, node, only, "spec_hash", &format!("\"{hash}\""))
}

/// `text` with its top-level `key = …` line set to `key = literal`.
pub fn with_top(text: &str, key: &str, literal: &str) -> String {
    let prefix = format!("{key} = ");
    let mut found = false;
    let mut out: Vec<String> = Vec::new();
    for line in text.split('\n') {
        if !found && line.starts_with(&prefix) {
            out.push(format!("{prefix}{literal}"));
            found = true;
        } else {
            out.push(line.to_owned());
        }
    }
    assert!(found, "no top-level `{key}`");
    out.join("\n")
}

/// The disk lock with `node`'s vouching entries (or its `only`-th) given
/// the node's `norm_hash` on disk: an editorial refresh by hand. The new
/// hash.
pub fn refresh(repo: &LockRepo, node: &str, only: Option<usize>) -> String {
    let new = repo.norm_hash(node);
    let text = repo.lock_text();
    assert_ne!(lock_hash(&text, node), new, "{node}: its text is unchanged");
    repo.write(LOCK, with_spec_hash(&text, node, only, &new));
    new
}

/// A `b3:` digest no text has.
pub fn fake_hash() -> String {
    format!("b3:{}", "0".repeat(64))
}

/// The disk lock with every entry of each of `nodes` given the node's
/// `norm_hash` on disk, whatever it held: the lock vouching for the text
/// as it is (the setups below change a document's front-matter first).
pub fn pin_lock(repo: &LockRepo, nodes: &[&str]) {
    let mut text = repo.lock_text();
    for node in nodes {
        text = with_spec_hash(&text, node, None, &repo.norm_hash(node));
    }
    repo.write(LOCK, text);
}

/// The number of `node`'s entries in a lock text.
pub fn entries_of(text: &str, node: &str) -> usize {
    blocks(text)
        .iter()
        .filter(|block| value(block, "node") == Some(node))
        .count()
}

// ------------------------------------------- no raise (iteration 2, W14)

/// code-a (`verify_common::golden`) with `GOAL-01` made a document of the
/// closed class `decision` (`CLOSED_DECISION`), its two entries vouching
/// for it, committed: its raise is refused at apply, so the rule advises
/// none.
pub fn close_goal(repo: &LockRepo) {
    repo.replace(GOAL_MD, "class: canon\n", "class: decision\n");
    repo.append("specengine.toml", CLOSED_DECISION);
    pin_lock(repo, &[GOAL]);
    repo.commit_all("GOAL-01 a closed decision");
}

/// verify-ecs (`verify_common::ecs`) with `stamina.md` made a document of
/// the closed class `decision`: its `rev: 2` dropped (the contract admits
/// none), `PERK-STAMINA`'s entries re-pinned at revision 1 to its text,
/// committed. `PERK-STAMINA` at rev 1 (its `id:` line 3);
/// its section `PERK-REGEN` at `rev=1`, its heading now line 12.
pub fn close_stamina(repo: &LockRepo) {
    repo.replace(STAMINA_MD, "class: canon\n", "class: decision\n");
    repo.replace(STAMINA_MD, "\nrev: 2\n", "\n");
    repo.append("specengine.toml", CLOSED_DECISION);
    repo.write(
        LOCK,
        with_field(&repo.lock_text(), STAMINA_DOC, None, "rev", "1"),
    );
    pin_lock(repo, &[STAMINA_DOC]);
    repo.commit_all("stamina a closed decision");
}

/// code-a with two documents whose raise no line can write, each under
/// its vouching entries, committed: `PIECE-COUNTER`'s `rev: +3` (read as
/// 3, not written as plain digits) and `GOAL-01`'s front-matter written
/// flow-style (no `id:` line to insert `rev:` after).
pub fn unwritable_raises(repo: &LockRepo) {
    repo.replace(COUNTER_MD, "\nrev: 3\n", "\nrev: +3\n");
    repo.replace(
        GOAL_MD,
        "class: canon\nid: GOAL-01\nowner: owner\nreviewed: 2026-10-10\n",
        "{class: canon, id: GOAL-01, owner: owner, reviewed: 2026-10-10}\n",
    );
    pin_lock(repo, &[PIECE, GOAL]);
    repo.commit_all("revisions no raise can write");
}

/// code-a with `LAW-BOUND` at `REV_MAX` (`rev=999999999`), its four
/// entries at that revision vouching for it, committed.
pub fn bound_at_max(repo: &LockRepo) {
    repo.replace(
        COUNTER_MD,
        "{#LAW-BOUND rev=2}",
        &format!("{{#LAW-BOUND rev={MAX_REV}}}"),
    );
    repo.write(
        LOCK,
        with_field(&repo.lock_text(), BOUND, None, "rev", &MAX_REV.to_string()),
    );
    pin_lock(repo, &[BOUND]);
    repo.commit_all("LAW-BOUND at the largest revision");
}

/// The rule's finding for `id` (`HEAD`'s revision `rev`) where no raise
/// can be advised ("Raising": a closed class admitting no `rev`, a raised
/// line that cannot be written): code, severity, path, `line` when given,
/// subject, always introduced; no `fix` key; a message naming the node and
/// `HEAD`'s revision and only the `--editorial` way out, saying neither
/// "raise it" nor "the largest revision", no raised value, no `[classes]`
/// way; when the check blocks, its text line. The message.
pub fn assert_no_raise(check: &Check, path: &str, line: Option<u64>, id: &str, rev: u32) -> String {
    let finding = check.finding(id);
    let context = check.show();
    assert_eq!(finding["code"], CODE, "{context}");
    assert_eq!(finding["severity"], "error", "{context}");
    assert_eq!(finding["path"], path, "{context}");
    if let Some(line) = line {
        assert_eq!(finding["line"], line, "{context}");
    }
    assert_eq!(finding["subject"], id, "{context}");
    assert_eq!(finding["introduced"], true, "always introduced: {context}");
    assert!(
        finding.get("fix").is_none(),
        "{id}: no raise can be written, so no fix: {finding}"
    );
    let message = finding["message"]
        .as_str()
        .unwrap_or_else(|| panic!("a message: {finding}"));
    assert!(
        message.starts_with(&format!("`{id}` changed at rev {rev}, ")),
        "{message}"
    );
    assert!(message.ends_with("with --editorial"), "{message}");
    for said in ["raise it", "largest revision", "rev=", "rev: ", "[classes]"] {
        assert!(!message.contains(said), "{said:?} in {message:?}");
    }
    if check.code() == 1 {
        let at = finding["line"].as_u64().expect("a line");
        let wanted = format!("error  {path}:{at}: {CODE}: {message}");
        assert!(
            check.lines().contains(&wanted.as_str()),
            "{wanted:?} not printed:\n{context}"
        );
    }
    message.to_owned()
}

/// A no-raise message with its node and `HEAD`'s revision set aside:
/// equal for every such finding.
pub fn no_raise_shape(message: &str, id: &str, rev: u32) -> String {
    message
        .replacen(&format!("`{id}`"), "`ID`", 1)
        .replacen(&format!("rev {rev},"), "rev N,", 1)
}

// ------------------------------------------------------------- the hook

/// `[check] mode = "<mode>"` appended to the root's config (written, not
/// committed).
pub fn check_mode(repo: &LockRepo, mode: &str) {
    repo.append(
        "specengine.toml",
        &format!("\n[check]\nmode = \"{mode}\"\n"),
    );
}

/// A `pre-commit` hook running the built `spec check --staged --root .`
/// (its output kept: git shows it), then appending `ran` to the log it
/// returns when it passed.
pub fn hook_check(repo: &LockRepo) -> PathBuf {
    let log = repo.scratch.join("hook.log");
    repo.hook(
        "pre-commit",
        &format!(
            "\"{SPEC}\" check --staged --root . || exit 1\necho ran >> '{}'",
            log.display()
        ),
    );
    log
}

/// The number of passing hook runs.
pub fn hook_runs(log: &Path) -> usize {
    fs::read_to_string(log).map_or(0, |text| text.lines().count())
}

/// `git commit -q -m message` at the root, hooks run, whatever its
/// outcome.
pub fn commit_hooked(repo: &LockRepo, message: &str) -> Output {
    repo.git
        .git_output(&repo.root, &["commit", "-q", "-m", message], &[])
}

/// A git run's stdout and stderr, as text.
pub fn shown(output: &Output) -> String {
    format!(
        "exit {:?}\n--- stdout\n{}--- stderr\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// `(span_hash, text, norm_hash)` of `reference` as `spec show` gives
/// them, from the root.
pub fn node_of(repo: &LockRepo, reference: &str) -> (String, String, String) {
    let outcome = show(
        &repo.env(&repo.root),
        &Globals::default(),
        &ShowRequest {
            reference: reference.to_owned(),
            links: false,
            archive: false,
        },
    )
    .unwrap_or_else(|error| panic!("show {reference}: {error}"));
    let node = outcome.nodes.into_iter().next().expect("one node");
    (
        node.span_hash,
        node.text,
        node.norm_hash.expect("a norm hash"),
    )
}

// ---------------------------------------------------------------- bump

/// One `spec bump <flags>` run, text and `--json`.
pub struct Bump {
    pub text: Run,
    pub json_run: Run,
    pub json: Value,
}

impl Bump {
    pub fn code(&self) -> i32 {
        self.text.code
    }

    pub fn show(&self) -> String {
        format!("{}\n{}", self.text.show(), self.json_run.show())
    }

    /// The JSON rows.
    pub fn rows(&self) -> Vec<&Value> {
        self.json["rows"].as_array().expect("rows").iter().collect()
    }

    /// The one JSON row of `id`.
    pub fn row(&self, id: &str) -> &Value {
        let found: Vec<&Value> = self
            .rows()
            .into_iter()
            .filter(|row| row["id"] == id)
            .collect();
        assert_eq!(found.len(), 1, "{id}:\n{}", self.show());
        found[0]
    }

    /// `(id, need)` of every JSON row, in order.
    pub fn needs(&self) -> Vec<(String, String)> {
        self.rows()
            .into_iter()
            .map(|row| {
                (
                    row["id"].as_str().expect("an id").to_owned(),
                    row["need"].as_str().expect("a need").to_owned(),
                )
            })
            .collect()
    }

    /// The text's row lines (`required | …`, `suggested | …`), in order.
    pub fn row_lines(&self) -> Vec<&str> {
        self.text
            .stdout
            .lines()
            .filter(|line| line.starts_with("required | ") || line.starts_with("suggested | "))
            .collect()
    }

    /// Every printed line, stdout then stderr.
    pub fn all_lines(&self) -> Vec<&str> {
        self.text
            .stdout
            .lines()
            .chain(self.text.stderr.lines())
            .collect()
    }
}

/// `spec bump <flags>` and `spec bump <flags> --json` from `cwd`: both
/// exit alike; the JSON one document when it ran (exit 0).
pub fn bump_at(repo: &LockRepo, cwd: &Path, flags: &[&str], extra: &[(&str, &OsStr)]) -> Bump {
    let mut args = vec!["bump"];
    args.extend_from_slice(flags);
    let text = spec_at(repo, cwd, &args, extra);
    args.push("--json");
    let json_run = spec_at(repo, cwd, &args, extra);
    assert_eq!(
        text.code,
        json_run.code,
        "text and JSON exit alike:\n{}\n{}",
        text.show(),
        json_run.show()
    );
    let json = if json_run.code == 0 {
        json_run.json()
    } else {
        Value::Null
    };
    Bump {
        text,
        json_run,
        json,
    }
}

/// `spec bump <flags>` from the root.
pub fn bump(repo: &LockRepo, flags: &[&str]) -> Bump {
    bump_at(repo, &repo.root, flags, &[])
}

/// A text row of `spec bump` ("spec bump"): `1 lock entry` in the
/// singular, `N lock entries` else.
#[allow(clippy::too_many_arguments)]
pub fn bump_line(
    need: &str,
    id: &str,
    path: &str,
    line: u64,
    rev: u32,
    to: u32,
    entries: usize,
    write: &str,
) -> String {
    let entries = entries_text(entries);
    format!("{need} | {id} | {path}:{line} | rev {rev} -> {to} | {entries} | {write}")
}

/// A text row with no raise ("spec bump": `rev N` alone, `-` where no
/// line can be written).
pub fn bump_line_alone(
    need: &str,
    id: &str,
    path: &str,
    line: u64,
    rev: u32,
    entries: usize,
) -> String {
    let entries = entries_text(entries);
    format!("{need} | {id} | {path}:{line} | rev {rev} | {entries} | -")
}

/// `1 lock entry` in the singular, `N lock entries` else.
fn entries_text(entries: usize) -> String {
    match entries {
        1 => "1 lock entry".to_owned(),
        count => format!("{count} lock entries"),
    }
}
