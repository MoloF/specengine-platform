//! docs/features/health-drift.md (9b-1), the CLI half: `spec health`'s row
//! 6, `drift`, is plain `spec verify`'s judge run in-process over the code
//! index the call refreshed (ADR-0047, ADR-0051): plain's rows but `ok`,
//! verbatim (17 keys, its key order), its counts, its `tested` entries but
//! `verified`, its `cannot:` lines as `causes`; never its mode, verdict,
//! hints or exit. "Acceptance criteria" RD-1..RD-8 (the CLI's), TE-1..TE-3
//! and TE-6. `health_rows` holds the 9a pins extended to `drift` (RD-2's
//! scan, RD-4's probe, RD-8's byte identity), `code_refs_walk.rs` TE-4,
//! `health_code_pilot.rs` TE-7.
//!
//! **The oracle is plain verify.** On one state and in one environment (the
//! repository's sandboxed git, its `HOME`, stdin null), `spec verify`,
//! `spec verify --json`, `spec health` and `spec health --json` run as the
//! binary; row 6 is derived from plain's document and text by "Data" and
//! "Text and stderr" ([`Judged::assert_parity`]), never from Health's own
//! output; exits are never compared (plain exits 2 on Cannot rows, Health
//! 0). Rows are compared in [`J`], which keeps every key in the order
//! written: never through core's 15-key `specengine_core::drift::DriftRow`.
//!
//! Corpora (TE-1, those of `verify_node.rs`'s
//! `verify_node_equals_plain_on_every_corpus`, no new fixture): `code-a`
//! with the golden lock and its edits (a node renamed away too:
//! `broken_binding node_gone`), `anchor-states` with its 18 scenarios,
//! `verify-tests` with `fixtures/spec-lock/verify-tests.tested.lock`,
//! `verify-ecs` with evidence blocks of every state, and a walk gap. The
//! example tree of "Data" ([`example`]): the golden with `CAP: u32 = 10;`
//! -> `12;` in `crates/tally/src/counter.rs`, uncommitted.
//!
//! Named mutations ("Acceptance criteria"): M1 [`drift_lists_no_ok_row`];
//! M2 [`health_drift_equals_plain_on_every_corpus`]; M3
//! `health_rows::health_reads_the_code_index_not_the_tree`; M4
//! [`a_run_cause_is_drift_cannot_never_zero`]; M5
//! [`drift_never_exits_1_or_carries_a_verdict`]; M6
//! [`drift_tested_lists_only_unverified`].
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use specengine_cli::{Globals, HealthRequest, Outcome, health, render_json};

use super::{J, NO_CODE};
use crate::anchor_common;
use crate::common::{Run, fixture, spec_with};
use crate::lock_common::LockRepo;
use crate::tested_common;
use crate::verify_common::{
    BOUNDS, COUNTER, COUNTER_MD, LIB, LIMITS, MODES, MOVEMENT, ROW_KEYS, STAMINA, ecs, golden,
    golden_format_1, set_mode,
};

// ------------------------------------------------------------ the words

/// "Data": the document's keys, `drift` the 7th.
const HEALTH_KEYS: [&str; 7] = [
    "documents",
    "amendments",
    "notes",
    "code",
    "assumes",
    "unbound",
    "drift",
];

/// "Data": `HealthDrift`'s keys, in order.
pub(super) const DRIFT_KEYS: [&str; 6] = ["state", "causes", "counts", "rows", "tested", "notes"];

/// "Data": verify's `counts`, 7 keys.
const COUNT_KEYS: [&str; 7] = [
    "rows",
    "ok",
    "fix",
    "check",
    "cannot",
    "pre_existing",
    "skipped",
];

/// `TestedRow`'s keys (8b-1).
const TESTED_KEYS: [&str; 7] = [
    "node",
    "state",
    "category",
    "rev",
    "node_rev",
    "commit",
    "verified_at",
];

/// Run cause (4) without a worktree (plain's line).
const NO_WORKTREE: &str = "no git repository was found for the root, or git could not read it";

/// Run cause (4) with no `git` on `PATH` (RD-8's cause).
const NO_GIT: &str = "`git` could not be run: no `git` program was found on PATH";

/// Run cause (5).
const UNMERGED: &str = "spec.lock is unmerged: run spec lock regen";

/// The states a non-`ok` row takes (`spec-verify.md` "States").
const NON_OK_STATES: [&str; 8] = [
    "bad_marker",
    "broken_binding",
    "cannot_verify",
    "code_ahead",
    "conflict",
    "predated",
    "spec_ahead",
    "unlocked",
];

/// Every anchor state (`spec-verify-anchors.md` "Anchors per row").
const ANCHOR_STATES: [&str; 4] = ["ambiguous", "exact", "lost", "moved"];

/// The categories the corpora give a non-`ok` row: `verify_node.rs`'s,
/// plus the dangling marker and the gone node M2 would drop.
const CATEGORIES: [&str; 18] = [
    "item_error",
    "ambiguous_symbol",
    "ambiguous_id",
    "rev_unknown",
    "unreadable",
    "hidden_by_error",
    "homoglyph",
    "bad_rev",
    "unanchored",
    "bad_levels",
    "file_gone",
    "symbol_gone",
    "marker_gone",
    "marker_behind",
    "code_changed",
    "levels_changed",
    "dangling",
    "node_gone",
];

/// The details the corpora give a non-`ok` row.
const DETAILS: [&str; 3] = ["duplicate", "not_utf8", "outside_code"];

/// The look-alike of `GOAL-01` (a Cyrillic O).
const LOOK_ALIKE: &str = "G\u{41E}AL-01";

/// The look-alike of `LAW-BOUND` (a Cyrillic A).
const LAW_LOOK_ALIKE: &str = "L\u{410}W-BOUND";

/// The example tree's edit ("Data").
const CAP_10: &str = "CAP: u32 = 10;";
const CAP_12: &str = "CAP: u32 = 12;";

// ------------------------------------------------------------ runs

type Vars = Vec<(OsString, OsString)>;

/// What `repo.spec_piped` gives a `spec` process: the sandbox's variables,
/// `HOME` the repository's.
fn vars_of(repo: &LockRepo) -> Vars {
    let mut vars: Vars = repo
        .git
        .vars()
        .into_iter()
        .filter(|(name, _)| name != "HOME")
        .collect();
    vars.push(("HOME".into(), repo.home.clone().into_os_string()));
    vars
}

/// `vars` with `name` set to `value`.
fn with_var(mut vars: Vars, name: &str, value: &OsStr) -> Vars {
    vars.retain(|(known, _)| known != name);
    vars.push((name.into(), value.to_os_string()));
    vars
}

/// `vars` without `name`.
fn without_var(mut vars: Vars, name: &str) -> Vars {
    vars.retain(|(known, _)| known != name);
    vars
}

/// `spec args` in `cwd` with exactly `vars`, stdin null.
fn spec_in(cwd: &Path, args: &[&str], vars: &[(OsString, OsString)]) -> Run {
    let env: Vec<(&str, &OsStr)> = vars
        .iter()
        .map(|(name, value)| (name.to_str().expect("a UTF-8 name"), value.as_os_str()))
        .collect();
    spec_with(cwd, args, &env)
}

/// An empty directory: a `PATH` without `git`.
fn no_git_dir(repo: &LockRepo) -> PathBuf {
    repo.scratch.dir("no-git-bin")
}

/// A directory whose `git` refuses everything (exit 128): a git failure.
fn failing_git_dir(repo: &LockRepo) -> PathBuf {
    let bin = repo.scratch.dir("failing-git-bin");
    let git = bin.join("git");
    fs::write(
        &git,
        "#!/bin/sh\necho 'fatal: refused by the test' >&2\nexit 128\n",
    )
    .expect("the failing git");
    fs::set_permissions(&git, fs::Permissions::from_mode(0o755)).expect("executable");
    bin
}

/// A directory whose `git` refuses `hash-object` alone and runs the
/// sandbox's git otherwise: the evidence cannot be judged (8b-1).
fn hash_refusing_git_dir(repo: &LockRepo) -> PathBuf {
    let bin = repo.scratch.dir("hash-refusing-git-bin");
    let git = bin.join("git");
    let real = repo.git.git_program().display().to_string();
    fs::write(
        &git,
        format!(
            "#!/bin/sh\nfor arg in \"$@\"; do\n  if [ \"$arg\" = hash-object ]; then\n    echo 'fatal: hash-object refused by the test' >&2\n    exit 128\n  fi\ndone\nexec '{real}' \"$@\"\n"
        ),
    )
    .expect("the wrapper");
    fs::set_permissions(&git, fs::Permissions::from_mode(0o755)).expect("executable");
    bin
}

/// Restores a path's mode when dropped (a failing test leaves no
/// unreadable path in its scratch).
struct ModeGuard {
    path: PathBuf,
    mode: u32,
}

impl ModeGuard {
    fn set(path: PathBuf, mode: u32) -> Self {
        let old = fs::metadata(&path).expect("the path").permissions().mode() & 0o7777;
        fs::set_permissions(&path, fs::Permissions::from_mode(mode)).expect("chmod");
        Self { path, mode: old }
    }
}

impl Drop for ModeGuard {
    fn drop(&mut self) {
        let _ = fs::set_permissions(&self.path, fs::Permissions::from_mode(self.mode));
    }
}

// ------------------------------------------------------------ ordered JSON

fn s(text: &str) -> J {
    J::Str(text.to_owned())
}

/// `value` as text: a string's own text.
fn word(value: &J) -> &str {
    match value {
        J::Str(text) => text,
        other => panic!("not a string: {other:?}"),
    }
}

/// Every key written anywhere under `value`.
fn all_keys(value: &J, out: &mut BTreeSet<String>) {
    match value {
        J::Obj(entries) => {
            for (key, inner) in entries {
                out.insert(key.clone());
                all_keys(inner, out);
            }
        }
        J::Arr(items) => {
            for item in items {
                all_keys(item, out);
            }
        }
        _ => {}
    }
}

/// The first place `want` and `got` differ, shown short.
fn difference(want: &[J], got: &[J]) -> String {
    if want.len() != got.len() {
        return format!("{} rows wanted, {} got", want.len(), got.len());
    }
    for (at, (a, b)) in want.iter().zip(got).enumerate() {
        if a != b {
            return format!("row {at}:\n want {a:?}\n  got {b:?}");
        }
    }
    "equal".to_owned()
}

// ------------------------------------------------------------ text

/// Plain's printed row blocks and `tested |` lines, in its order: its
/// stdout but the `cannot:` lines, the hints and the summary.
fn plain_printed(plain: &Run) -> Vec<String> {
    let mut lines: Vec<&str> = plain.stdout.lines().collect();
    lines.pop();
    lines
        .into_iter()
        .filter(|line| !line.starts_with("cannot: ") && !line.starts_with("hint: "))
        .map(str::to_owned)
        .collect()
}

/// Where a line of Health's text sits ("Text and stderr": the row lines of
/// rows 4, 5, 6, their summaries in that order, `code:`, the last line);
/// row 3's lines first. A sub-line (two spaces in) is its row's.
fn ranks(stdout: &str) -> Vec<(u8, String)> {
    const RANKS: [(&str, u8); 8] = [
        ("assumes | ", 1),
        ("unbound | ", 2),
        ("drift | ", 3),
        ("assumes: ", 4),
        ("unbound: ", 5),
        ("drift: ", 6),
        ("code: ", 7),
        ("spec health: ", 8),
    ];
    let mut out: Vec<(u8, String)> = Vec::new();
    for line in stdout.lines() {
        let rank = if line.starts_with("  ") {
            out.last().map_or(0, |(rank, _)| *rank)
        } else {
            RANKS
                .iter()
                .find(|(prefix, _)| line.starts_with(prefix))
                .map_or(0, |(_, rank)| *rank)
        };
        out.push((rank, line.to_owned()));
    }
    out
}

/// Row 6's printed lines ("Text and stderr"): each `drift | ` line without
/// its prefix, each sub-line under one as written.
fn drift_printed(stdout: &str) -> Vec<String> {
    ranks(stdout)
        .into_iter()
        .filter(|(rank, _)| *rank == 3)
        .map(|(_, line)| {
            line.strip_prefix("drift | ")
                .map_or_else(|| line.clone(), str::to_owned)
        })
        .collect()
}

/// Row 6's summary lines (`drift: …`).
fn drift_summaries(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .filter(|line| line.starts_with("drift: "))
        .map(str::to_owned)
        .collect()
}

/// stderr's `note: drift: …` lines.
fn drift_notes(stderr: &str) -> Vec<String> {
    stderr
        .lines()
        .filter(|line| line.starts_with("note: drift: "))
        .map(str::to_owned)
        .collect()
}

/// The summary plain's implies ("Text and stderr"): its counts and moved
/// `ok` rows, its skipped markers; no verdict, mode, `by blob`, `by ast`.
fn summary_of(plain: &Run) -> String {
    let last = plain.stdout.lines().last().expect("plain's summary");
    let (_, rest) = last
        .split_once("]: ")
        .unwrap_or_else(|| panic!("`verify: <verdict> [<mode>]: …`: {last}"));
    let parts: Vec<&str> = rest.split("; ").collect();
    assert_eq!(parts.len(), 3, "counts; cascade; skipped: {last}");
    format!("drift: {}; {}", parts[0], parts[2])
}

/// Neither Health run carries a verdict, a mode or a hint (RD-3, ADR-0042):
/// the document's keys are Health's, row 6's are "Data"'s and none of
/// verify's top-level keys is under it; no `hint:` line, no mode note, no
/// `blocked` anywhere; the summary without verdict word, mode or cascade.
fn assert_no_verdict(json: &Run, text: &Run, context: &str) {
    let document = J::parse(json.stdout.trim_end());
    assert_eq!(document.keys(), HEALTH_KEYS, "{context}");
    let drift = document.get("drift");
    assert_eq!(drift.keys(), DRIFT_KEYS, "{context}");
    let mut keys = BTreeSet::new();
    all_keys(&document, &mut keys);
    for key in [
        "mode", "verdict", "base", "cascade", "delta", "omitted", "hint", "hints", "blocked",
    ] {
        assert!(!keys.contains(key), "{context}: no `{key}` key: {keys:?}");
    }
    for key in ["mode", "verdict", "base", "cascade", "delta", "omitted"] {
        assert!(
            !drift.keys().contains(&key),
            "{context}: verify's `{key}` left out"
        );
    }
    for run in [json, text] {
        for stream in [&run.stdout, &run.stderr] {
            assert!(!stream.contains("blocked"), "{context}: {}", run.show());
            assert!(
                !stream.lines().any(|line| line.starts_with("hint:")),
                "{context}: no hint: {}",
                run.show()
            );
            assert!(
                !stream.contains("judged as `enforce`"),
                "{context}: no mode note: {}",
                run.show()
            );
        }
    }
    for line in drift_summaries(&text.stdout) {
        for word in [
            "clean",
            "observed",
            "cannot-verify",
            "[observe]",
            "[enforce]",
            "[enforce-introduced]",
            "by blob",
            "by ast",
            "pre-existing",
        ] {
            assert!(!line.contains(word), "{context}: {line}");
        }
    }
}

// ------------------------------------------------------------ the judged

/// One state judged by both commands in one environment.
struct Judged {
    what: String,
    plain_text: Run,
    plain_json: Run,
    text: Run,
    json: Run,
}

impl Judged {
    fn plain(&self) -> J {
        J::parse(self.plain_json.stdout.trim_end())
    }

    fn document(&self) -> J {
        J::parse(self.json.stdout.trim_end())
    }

    fn value(&self) -> Value {
        serde_json::from_str(self.json.stdout.trim_end())
            .unwrap_or_else(|error| panic!("{}: {error}\n{}", self.what, self.json.show()))
    }

    fn plain_value(&self) -> Value {
        serde_json::from_str(self.plain_json.stdout.trim_end())
            .unwrap_or_else(|error| panic!("{}: {error}\n{}", self.what, self.plain_json.show()))
    }

    fn drift(&self) -> Value {
        self.value()["drift"].clone()
    }

    fn show(&self) -> String {
        let cut = |text: &str| -> String { text.chars().take(6_000).collect() };
        format!(
            "{}\n--- health\n{}\n--- health --json\n{}\n--- verify\n{}\n--- verify --json\n{}",
            self.what,
            cut(&self.text.show()),
            cut(&self.json.show()),
            cut(&self.plain_text.show()),
            cut(&self.plain_json.show()),
        )
    }

    /// "Data", "Text and stderr" and RD-1 from plain's document and text:
    /// measured, row 6 holds plain's counts, its rows whose class is not
    /// null (verbatim, 17 keys in order), its `tested` but `verified`, its
    /// `tested:` notes alone, the summary plain's less verdict, mode and
    /// cascade, each printed line `drift | ` + plain's (sub-lines as
    /// plain's), `note: drift: <note>` on stderr; with a run cause, row 6
    /// `cannot`, `causes` plain's `cannot` lines in order, `counts`,
    /// `rows`, `tested` null, `notes` `[]`, `drift: cannot: <cause>` lines
    /// and `note: drift: <cause>` on stderr. Both modes exit 0; the order
    /// of the text's parts; no verdict.
    fn assert_parity(&self) {
        let what = self.what.as_str();
        self.json.code(0);
        self.text.code(0);
        assert!(
            !self.plain_json.stdout.is_empty() && !self.plain_text.stdout.is_empty(),
            "{what}: premise: plain answers with a report\n{}",
            self.show()
        );
        let plain = self.plain();
        let document = self.document();
        assert_eq!(document.keys(), HEALTH_KEYS, "{what}");
        let drift = document.get("drift");
        assert_eq!(drift.keys(), DRIFT_KEYS, "{what}");
        let cannot = plain.get("cannot").items();
        let stdout = &self.text.stdout;
        if cannot.is_empty() {
            assert_eq!(drift.get("state"), &s("measured"), "{}", self.show());
            assert_eq!(drift.get("causes"), &J::Arr(Vec::new()), "{what}");
            assert_eq!(drift.get("counts").keys(), COUNT_KEYS, "{what}");
            assert_eq!(drift.get("counts"), plain.get("counts"), "{what}: counts");
            let want: Vec<J> = plain
                .get("rows")
                .items()
                .iter()
                .filter(|row| row.get("class") != &J::Null)
                .cloned()
                .collect();
            for row in &want {
                assert_eq!(row.keys(), ROW_KEYS, "{what}: premise: plain's 17 keys");
            }
            let got = drift.get("rows").items();
            assert_eq!(
                got,
                want.as_slice(),
                "{what}: plain's rows but `ok`, verbatim: {}",
                difference(&want, got)
            );
            let tested = match plain.get("tested") {
                J::Null => J::Null,
                J::Arr(entries) => J::Arr(
                    entries
                        .iter()
                        .filter(|entry| entry.get("state") != &s("verified"))
                        .cloned()
                        .collect(),
                ),
                other => panic!("{what}: plain's `tested`: {other:?}"),
            };
            assert_eq!(drift.get("tested"), &tested, "{what}: tested");
            if let J::Arr(entries) = drift.get("tested") {
                for entry in entries {
                    assert_eq!(entry.keys(), TESTED_KEYS, "{what}");
                }
            }
            let notes: Vec<J> = plain
                .get("notes")
                .items()
                .iter()
                .filter(|note| word(note).starts_with("tested: "))
                .cloned()
                .collect();
            assert_eq!(
                drift.get("notes"),
                &J::Arr(notes.clone()),
                "{what}: the row's own notes alone"
            );
            assert_eq!(
                drift_printed(stdout),
                plain_printed(&self.plain_text),
                "{what}: `drift | ` + plain's printed lines\n{}",
                self.show()
            );
            assert_eq!(
                drift_summaries(stdout),
                [summary_of(&self.plain_text)],
                "{what}"
            );
            let note_lines: Vec<String> = notes
                .iter()
                .map(|note| format!("note: drift: {}", word(note)))
                .collect();
            for run in [&self.json, &self.text] {
                assert_eq!(
                    drift_notes(&run.stderr),
                    note_lines,
                    "{what}: {}",
                    run.show()
                );
            }
        } else {
            assert_eq!(drift.get("state"), &s("cannot"), "{}", self.show());
            assert_eq!(
                drift.get("causes"),
                plain.get("cannot"),
                "{what}: plain's `cannot` lines, in order"
            );
            for key in ["counts", "rows", "tested"] {
                assert_eq!(drift.get(key), &J::Null, "{what}: `{key}` null, never 0");
            }
            assert_eq!(drift.get("notes"), &J::Arr(Vec::new()), "{what}");
            assert!(drift_printed(stdout).is_empty(), "{what}: {stdout}");
            let causes: Vec<&str> = cannot.iter().map(word).collect();
            assert_eq!(
                drift_summaries(stdout),
                causes
                    .iter()
                    .map(|cause| format!("drift: cannot: {cause}"))
                    .collect::<Vec<_>>(),
                "{what}"
            );
            for run in [&self.json, &self.text] {
                assert_eq!(
                    drift_notes(&run.stderr),
                    causes
                        .iter()
                        .map(|cause| format!("note: drift: {cause}"))
                        .collect::<Vec<_>>(),
                    "{what}: {}",
                    run.show()
                );
            }
        }
        let order: Vec<u8> = ranks(stdout).iter().map(|(rank, _)| *rank).collect();
        assert!(
            order.windows(2).all(|pair| pair[0] <= pair[1]),
            "{what}: rows 4, 5, 6, their summaries, `code:`, the last line:\n{stdout}"
        );
        assert!(
            stdout
                .lines()
                .last()
                .is_some_and(|line| line.starts_with("spec health: ")),
            "{what}: row 3's last line last"
        );
        assert_no_verdict(&self.json, &self.text, what);
    }
}

/// Health (JSON, text, JSON again), then plain verify (JSON, text) in
/// `repo` with `vars`; Health byte-identical on the repeat and, when
/// `untouched`, nothing under the worktree changed (TE-3).
fn judged_with(
    repo: &LockRepo,
    what: &str,
    vars: &[(OsString, OsString)],
    untouched: bool,
) -> Judged {
    let before = untouched.then(|| repo.state());
    let json = spec_in(&repo.root, &["health", "--json"], vars);
    let text = spec_in(&repo.root, &["health"], vars);
    let again = spec_in(&repo.root, &["health", "--json"], vars);
    if let Some(before) = before {
        assert!(
            repo.state() == before,
            "{what}: Health wrote under the worktree"
        );
    }
    assert_eq!(
        (again.code, &again.stdout, &again.stderr),
        (json.code, &json.stdout, &json.stderr),
        "{what}: a repeat, byte for byte"
    );
    let plain_json = spec_in(&repo.root, &["verify", "--json"], vars);
    let plain_text = spec_in(&repo.root, &["verify"], vars);
    Judged {
        what: what.to_owned(),
        plain_text,
        plain_json,
        text,
        json,
    }
}

/// [`judged_with`] in the repository's own environment, parity asserted.
fn judge(repo: &LockRepo, what: &str) -> Judged {
    let judged = judged_with(repo, what, &vars_of(repo), true);
    judged.assert_parity();
    judged
}

// ------------------------------------------------------------ the trees

/// "Data"'s example tree: the golden, `CAP` edited, uncommitted.
fn example(label: &str) -> LockRepo {
    let repo = golden(label);
    repo.replace(COUNTER, CAP_10, CAP_12);
    repo
}

/// `verify_node.rs`'s `drift()` over the golden: `CAP` and the RON cap
/// edited, LAW-BOUND's text edited, PIECE-COUNTER's `//!` marker in the
/// test file removed, a non-UTF-8 file (a note), plain
/// `enforce-introduced` (the mode note).
fn edits(repo: &LockRepo) {
    repo.replace(COUNTER, CAP_10, CAP_12);
    repo.replace(LIMITS, "cap: 10,", "cap: 12,");
    repo.replace(
        COUNTER_MD,
        "The count never exceeds the cap.",
        "The count never exceeds the cap, ever.",
    );
    repo.replace(BOUNDS, "//! @verifies PIECE-COUNTER\n", "");
    repo.write("crates/tally/src/odd.rs", b"// \xff\n".as_slice());
    set_mode(&repo.root, "enforce-introduced");
}

/// `verify_node.rs`'s `halves()`: GOAL-01's marker written as a
/// look-alike, a look-alike of LAW-BOUND on a new item, LAW-BOUND's
/// Display impl removed, an alias marker, a typo (`LAW-BUOND`: dangling),
/// a cross-file `duplicate` twin.
fn halves(repo: &LockRepo) {
    repo.replace(
        COUNTER,
        "@implements AIM-01@1",
        &format!("@implements {LOOK_ALIKE}@1"),
    );
    repo.replace(
        COUNTER,
        "impl fmt::Display for Tally {\n    // @implements PIECE-COUNTER#LAW-BOUND@2\n    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {\n        write!(f, \"{}\", self.count)\n    }\n}\n",
        "",
    );
    repo.replace(
        COUNTER,
        "pub fn total(",
        "// @implements LAW-BUOND\npub fn total(",
    );
    repo.append(
        COUNTER,
        &format!(
            "\n// @implements AIM-01\npub fn spare() -> u32 {{\n    1\n}}\n\n// @implements {LAW_LOOK_ALIKE}@2\npub fn ceiling() -> u32 {{\n    CAP\n}}\n"
        ),
    );
    repo.write(
        "crates/tally/src/pair.rs",
        "// @implements LAW-STEP\npub fn twin() {}\n",
    );
    repo.write("crates/tally/src/pair/mod.rs", "pub fn twin() {}\n");
}

/// `fixtures/code-a/expected.json`'s "generated" files written under the
/// root: a second holder of `GOAL-03`, its ambiguous marker, a look-alike
/// of PIECE-COUNTER, a RON file past the depth cap.
fn generated(repo: &LockRepo) {
    let expected: Value = serde_json::from_str(
        &fs::read_to_string(fixture("code-a").join("expected.json")).expect("code-a's expected"),
    )
    .expect("code-a's expected.json is JSON");
    for file in expected["generated"]["files"].as_array().expect("files") {
        let mut body = String::new();
        for part in file["parts"].as_array().expect("parts") {
            let times = usize::try_from(part["times"].as_u64().expect("times")).expect("usize");
            body.push_str(&part["text"].as_str().expect("text").repeat(times));
        }
        repo.write(file["path"].as_str().expect("path"), body);
    }
}

/// `from` -> `to` in every UTF-8 file under the root but `.git` and
/// `spec.lock`: a node renamed away, its lock entries left behind.
fn rename_everywhere(repo: &LockRepo, from: &str, to: &str) {
    for (path, bytes) in crate::common::snapshot(&repo.root) {
        let Some(bytes) = bytes else { continue };
        if path == ".git" || path.starts_with(".git/") || path == "spec.lock" {
            continue;
        }
        let Ok(text) = String::from_utf8(bytes) else {
            continue;
        };
        if text.contains(from) {
            repo.write(&path, text.replace(from, to));
        }
    }
}

/// Every row 6 a corpus showed, for TE-1's coverage.
#[derive(Default)]
struct Kept {
    rows: Vec<Value>,
    tested: Vec<Value>,
    skipped: u64,
    causes: Vec<String>,
}

impl Kept {
    fn add(&mut self, judged: &Judged) {
        let drift = judged.drift();
        if let Some(rows) = drift["rows"].as_array() {
            self.rows.extend(rows.iter().cloned());
        }
        if let Some(tested) = drift["tested"].as_array() {
            self.tested.extend(tested.iter().cloned());
        }
        self.skipped = self
            .skipped
            .max(drift["counts"]["skipped"].as_u64().unwrap_or(0));
        if let Some(causes) = drift["causes"].as_array() {
            self.causes
                .extend(causes.iter().filter_map(Value::as_str).map(str::to_owned));
        }
    }

    fn extend(&mut self, other: Kept) {
        self.rows.extend(other.rows);
        self.tested.extend(other.tested);
        self.skipped = self.skipped.max(other.skipped);
        self.causes.extend(other.causes);
    }
}

/// TE-1 (a): the golden as committed; then a second holder, `edits`,
/// `halves` (every category `verify_node.rs` meets, the mode note and a
/// non-UTF-8 note plain lists and row 6 does not); then a bound file
/// deleted, one not UTF-8, one excluded, a level list narrowed; then
/// LAW-BOUND renamed away (`broken_binding node_gone`).
fn corpus_golden() -> Kept {
    let repo = golden("hd-te1-golden");
    let mut kept = Kept::default();
    kept.add(&judge(&repo, "golden: as committed"));

    generated(&repo);
    repo.commit_all("a second holder");
    edits(&repo);
    halves(&repo);
    let drifted = judge(&repo, "golden: drifted");
    let plain_notes = drifted.plain_value()["notes"].clone();
    assert!(
        plain_notes
            .as_array()
            .is_some_and(|notes| notes.iter().any(|note| note
                .as_str()
                .is_some_and(|note| note.contains("enforce-introduced")))),
        "premise: plain notes its mode: {plain_notes}"
    );
    assert_eq!(drifted.drift()["notes"], json!([]), "no mode note in row 6");
    kept.add(&drifted);

    fs::remove_file(repo.root.join(BOUNDS)).expect("the test file deleted");
    let mut bytes = fs::read(repo.root.join(LIB)).expect("lib.rs");
    bytes.extend_from_slice(b"// \xff\n");
    repo.write(LIB, bytes);
    repo.replace(
        "specengine.toml",
        "exclude = [\"crates/*/tests/fixtures/**\"]",
        "exclude = [\"crates/*/tests/fixtures/**\", \"data/limits.ron\"]",
    );
    repo.replace(
        COUNTER,
        "@implements PIECE-COUNTER@3 [sig, body]",
        "@implements PIECE-COUNTER@3 [body]",
    );
    kept.add(&judge(&repo, "golden: files gone"));

    rename_everywhere(&repo, "LAW-BOUND", "LAW-LIMIT");
    kept.add(&judge(&repo, "golden: LAW-BOUND renamed away"));
    kept
}

/// TE-1 (b): `fixtures/anchor-states`, its baseline, then its 18
/// scenarios together (moved, lost, ambiguous rows; `ok` rows moved).
fn corpus_anchor_states() -> Kept {
    let repo = anchor_common::base("hd-te1-anchors");
    let mut kept = Kept::default();
    kept.add(&judge(&repo, "anchor-states: baseline"));
    for name in anchor_common::SCENARIOS {
        anchor_common::change(&repo, name);
    }
    let changed = judge(&repo, "anchor-states: every scenario");
    assert!(
        changed.plain_value()["rows"]
            .as_array()
            .expect("rows")
            .iter()
            .any(|row| row["class"].is_null() && row["anchor"]["state"] == "moved"),
        "premise: an `ok` row moved (the summary's `(m moved)`)"
    );
    kept.add(&changed);
    kept
}

/// TE-1 (c): `fixtures/verify-tests` without a lock (every row
/// `unlocked`: the flood), then `verify-tests.tested.lock` committed (its
/// four blocks), then KILN-RAMP's code and KILN-HOLD's text edited
/// (`stale`), CONE-01's bound test broken (`unverifiable`), then
/// KILN-HOLD renamed away (`node_gone`).
fn corpus_kiln() -> Kept {
    let repo = LockRepo::of("hd-te1-kiln", "verify-tests", "repo", &[]);
    let mut kept = Kept::default();
    let flood = judge(&repo, "verify-tests: no lock");
    // "Rules and edge cases": no `spec.lock`, row 6 measured, every
    // binding `unlocked` (the flood).
    let drift = flood.drift();
    assert_eq!(drift["state"], "measured", "{drift}");
    let rows = drift["rows"].as_array().expect("rows");
    assert!(
        !rows.is_empty() && rows.iter().all(|row| row["lock"].is_null()),
        "{drift}"
    );
    assert!(rows.iter().any(|row| row["state"] == "unlocked"), "{drift}");
    kept.add(&flood);

    repo.write("spec.lock", tested_common::golden_text());
    repo.commit_all("the evidence");
    kept.add(&judge(&repo, "verify-tests: the golden evidence"));

    repo.replace(
        "crates/kiln/src/ramp.rs",
        "(now + rate).min(peak)",
        "(now + rate + 1).min(peak)",
    );
    repo.replace(
        "docs/spec/kiln.md",
        "keeps its temperature until the hold time ends.",
        "keeps its temperature until the hold time has ended.",
    );
    let ramp = repo.read("crates/kiln/src/ramp.rs");
    let broken = ramp.replacen(
        "assert!(next_temperature(0, 5000, 1300) <= 1300);",
        "assert!(next_temperature(0, 5000, 1300) <= );",
        1,
    );
    assert_ne!(broken, ramp, "premise: CONE-01's test broken");
    repo.write("crates/kiln/src/ramp.rs", broken);
    kept.add(&judge(&repo, "verify-tests: stale evidence"));

    repo.replace("docs/spec/kiln.md", "{#KILN-HOLD}", "{#KILN-DWELL}");
    repo.commit_all("the hold is a dwell now");
    kept.add(&judge(&repo, "verify-tests: a block's node gone"));
    kept
}

/// TE-1 (d): `verify-ecs` with evidence blocks of every state
/// (`tested_common::ecs_with_blocks`), then a body edited, a RON value
/// edited and a marker raised past its node.
fn corpus_ecs() -> Kept {
    let (repo, _, _) = tested_common::ecs_with_blocks("hd-te1-ecs");
    let mut kept = Kept::default();
    kept.add(&judge(&repo, "verify-ecs: evidence"));
    repo.replace(
        STAMINA,
        "(stamina.current - cost * dt).max(0.0)",
        "(stamina.current - cost * dt * 2.0).max(0.0)",
    );
    repo.replace(MOVEMENT, "regen_per_second: 2.5,", "regen_per_second: 3.0,");
    repo.replace(
        STAMINA,
        "// @implements PERK-REGEN@1\n",
        "// @implements PERK-REGEN@2\n",
    );
    kept.add(&judge(&repo, "verify-ecs: drifted"));
    kept
}

/// TE-1 (e): a walk gap is a run cause plain lists and row 6 shares with
/// rows 4-5 ("Degrade matrix"): a code file no node binds unreadable
/// (cold, then warm), a directory that cannot be listed; readable again,
/// measured.
fn corpus_walk_gap() -> Kept {
    let repo = ecs("hd-te1-gap");
    let spare = "crates/game/src/spare.rs";
    repo.write(spare, "pub fn spare() {}\n");
    repo.write("crates/game/src/deep/inner.rs", "pub fn inner() {}\n");
    repo.commit_all("unbound code");
    let mut kept = Kept::default();
    let vars = vars_of(&repo);
    // Row 5 holds no rule here: `not_configured` before any code cause
    // (`health-code.md` "Failures and exits"); row 4 shares row 6's.
    let shared = |judged: &Judged| {
        let document = judged.value();
        assert_eq!(
            document["assumes"]["causes"], document["drift"]["causes"],
            "{}: rows 4 and 6 share the cause",
            judged.what
        );
        assert_eq!(document["unbound"]["state"], "not_configured");
    };
    {
        let _mode = ModeGuard::set(repo.root.join(spare), 0o000);
        for warmth in ["cold", "warm"] {
            let judged = judged_with(
                &repo,
                &format!("an unreadable file, {warmth}"),
                &vars,
                false,
            );
            judged.assert_parity();
            assert_eq!(judged.drift()["state"], "cannot");
            shared(&judged);
            kept.add(&judged);
        }
    }
    {
        let _mode = ModeGuard::set(repo.root.join("crates/game/src/deep"), 0o000);
        let judged = judged_with(&repo, "a directory not listed", &vars, false);
        judged.assert_parity();
        assert_eq!(judged.drift()["state"], "cannot");
        shared(&judged);
        kept.add(&judged);
    }
    let again = judge(&repo, "readable again");
    assert_eq!(again.drift()["state"], "measured");
    kept.add(&again);
    kept
}

/// The distinct strings of `field` over `values` (`null` left out).
fn values_of(values: &[Value], field: impl Fn(&Value) -> &Value) -> BTreeSet<String> {
    values
        .iter()
        .map(field)
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}

fn owned(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

/// One corpus of TE-1.
type Corpus = fn() -> Kept;

// ------------------------------------------------------------ RD-1, TE-1, M2

/// RD-1, TE-1 (M2): over the five corpora (each on a thread of its own),
/// in every state, row 6 is plain verify's on the same tree
/// ([`Judged::assert_parity`]): its non-`ok` rows verbatim, counts,
/// non-`verified` evidence, run causes; across them every non-`ok` state,
/// every anchor state, every category and detail is met, a bare `""` and
/// dangling rows, `broken_binding node_gone`, `skipped` above 0, evidence
/// `stale`, `unverifiable` and `node_gone`, a walk gap's cause. M2: row 6
/// judged as a node set over every node (`judge_set` unchanged).
#[test]
fn health_drift_equals_plain_on_every_corpus() {
    let corpora: [(&str, Corpus); 5] = [
        ("golden", corpus_golden),
        ("anchor-states", corpus_anchor_states),
        ("verify-tests", corpus_kiln),
        ("verify-ecs", corpus_ecs),
        ("a walk gap", corpus_walk_gap),
    ];
    let kept: Kept = std::thread::scope(|scope| {
        let handles: Vec<_> = corpora
            .iter()
            .map(|(name, corpus)| (*name, scope.spawn(corpus)))
            .collect();
        let mut kept = Kept::default();
        let mut failed = Vec::new();
        for (name, handle) in handles {
            match handle.join() {
                Ok(more) => kept.extend(more),
                Err(_) => failed.push(name),
            }
        }
        assert!(failed.is_empty(), "corpora failed (above): {failed:?}");
        kept
    });
    assert!(
        kept.rows.iter().all(|row| !row["class"].is_null()),
        "never an `ok` row"
    );
    assert_eq!(
        values_of(&kept.rows, |row| &row["state"]),
        owned(&NON_OK_STATES),
        "every non-`ok` state"
    );
    assert_eq!(
        values_of(&kept.rows, |row| &row["class"]),
        owned(&["cannot", "check", "fix"]),
        "every class"
    );
    assert_eq!(
        values_of(&kept.rows, |row| &row["anchor"]["state"]),
        owned(&ANCHOR_STATES),
        "every anchor state"
    );
    let categories = values_of(&kept.rows, |row| &row["category"]);
    for category in CATEGORIES {
        assert!(
            categories.contains(category),
            "{category} met: {categories:?}"
        );
    }
    let details = values_of(&kept.rows, |row| &row["detail"]);
    for detail in DETAILS {
        assert!(details.contains(detail), "{detail} met: {details:?}");
    }
    assert!(
        kept.rows.iter().any(|row| row["node"] == ""),
        "a bare marker's row (node \"\")"
    );
    assert!(
        kept.rows
            .iter()
            .any(|row| row["state"] == "broken_binding" && row["category"] == "node_gone"),
        "a gone node's entries"
    );
    assert!(kept.skipped > 0, "markers skipped and counted");
    assert_eq!(
        values_of(&kept.tested, |entry| &entry["state"]),
        owned(&["node_gone", "stale", "unverifiable"]),
        "evidence of every state but `verified`"
    );
    assert!(
        kept.causes
            .iter()
            .any(|cause| cause.starts_with("the code walk is incomplete: ")),
        "a walk gap's cause: {:?}",
        kept.causes
    );
}

// ------------------------------------------------------------ RD-1, M1

/// RD-1 (M1): no `ok` row in row 6: on the golden (8 `ok` rows of 39 on
/// the example tree) `rows` holds `counts.rows - counts.ok` rows, none
/// with class `null`, the text no `- |` class and one `drift | ` row line
/// per row; on anchor-states' scenarios the moved `ok` rows are counted in
/// the summary's `(<m> moved)`, never listed. M1: an `ok` row listed.
#[test]
fn drift_lists_no_ok_row() {
    let repo = example("hd-no-ok");
    let judged = judge(&repo, "the example tree");
    let drift = judged.drift();
    let counts = &drift["counts"];
    let ok = counts["ok"].as_u64().expect("ok");
    assert!(ok > 0, "premise: `ok` rows: {counts}");
    let rows = drift["rows"].as_array().expect("rows");
    assert_eq!(
        rows.len() as u64,
        counts["rows"].as_u64().expect("rows") - ok,
        "{counts}"
    );
    assert!(rows.iter().all(|row| !row["class"].is_null()));
    let lines: Vec<&str> = judged
        .text
        .stdout
        .lines()
        .filter(|line| line.starts_with("drift | ") && !line.starts_with("drift | tested | "))
        .collect();
    assert_eq!(lines.len(), rows.len(), "{}", judged.text.stdout);
    assert!(
        lines.iter().all(
            |line| ["drift | fix | ", "drift | check | ", "drift | cannot | "]
                .iter()
                .any(|class| line.starts_with(class))
        ),
        "{lines:#?}"
    );

    let anchors = anchor_common::base("hd-no-ok-moved");
    for name in anchor_common::SCENARIOS {
        anchor_common::change(&anchors, name);
    }
    let judged = judge(&anchors, "anchor-states: every scenario");
    let plain = judged.plain_value();
    let moved = plain["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .filter(|row| row["class"].is_null() && row["anchor"]["state"] == "moved")
        .count();
    assert!(moved > 0, "premise: moved `ok` rows");
    let summary = drift_summaries(&judged.text.stdout);
    assert!(
        summary.len() == 1 && summary[0].contains(&format!(" ok ({moved} moved); ")),
        "{summary:?}"
    );
    let shown = judged.drift()["rows"].as_array().expect("rows").len();
    let non_ok = plain["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .filter(|row| !row["class"].is_null())
        .count();
    assert_eq!(shown, non_ok);
}

// ------------------------------------------------------------ RD-1, M6

/// RD-1 (M6): `tested` lists only the blocks not `verified`: verify-ecs'
/// four blocks (PERK-STAMINA `verified`, PERK-REGEN `stale spec_changed`,
/// PERK-DRAIN `stale code_changed`, GONE-01 `node_gone`) give three
/// entries, verbatim and by node bytes, and three `drift | tested | …`
/// lines; a lock whose blocks are all `verified` gives `[]`; blocks that
/// cannot be judged (git refusing `hash-object`) give `tested` `null`, one
/// note `tested: <cause>` in `drift.notes` and on stderr as `note: drift:
/// tested: <cause>`, rows and counts as plain's. M6: a `verified` entry
/// listed.
#[test]
fn drift_tested_lists_only_unverified() {
    let (repo, _, _) = tested_common::ecs_with_blocks("hd-tested");
    let judged = judge(&repo, "verify-ecs' four blocks");
    let plain = judged.plain_value();
    let states: Vec<(String, String)> = plain["tested"]
        .as_array()
        .expect("plain's tested")
        .iter()
        .map(|entry| {
            (
                entry["node"].as_str().expect("node").to_owned(),
                entry["state"].as_str().expect("state").to_owned(),
            )
        })
        .collect();
    assert!(
        states.contains(&("PERK-STAMINA".to_owned(), "verified".to_owned())),
        "premise: a verified block: {states:?}"
    );
    let tested = judged.drift()["tested"].clone();
    let nodes: Vec<&str> = tested
        .as_array()
        .expect("tested")
        .iter()
        .map(|entry| entry["node"].as_str().expect("node"))
        .collect();
    assert_eq!(nodes, ["GONE-01", "PERK-DRAIN", "PERK-REGEN"], "{tested}");
    let lines: Vec<&str> = judged
        .text
        .stdout
        .lines()
        .filter(|line| line.starts_with("drift | tested | "))
        .collect();
    assert_eq!(
        lines,
        [
            "drift | tested | node_gone | GONE-01",
            "drift | tested | stale code_changed | PERK-DRAIN",
            "drift | tested | stale spec_changed | PERK-REGEN",
        ],
        "{}",
        judged.text.stdout
    );

    // Every block verified: `[]`, no line.
    let kiln = LockRepo::of("hd-tested-kiln", "verify-tests", "repo", &[]);
    kiln.write("spec.lock", tested_common::golden_text());
    kiln.commit_all("the evidence");
    let judged = judge(&kiln, "the golden evidence");
    let plain = judged.plain_value();
    assert!(
        plain["tested"]
            .as_array()
            .is_some_and(|entries| !entries.is_empty()
                && entries.iter().all(|entry| entry["state"] == "verified")),
        "premise: every block verified: {}",
        plain["tested"]
    );
    assert_eq!(judged.drift()["tested"], json!([]));
    assert!(!judged.text.stdout.contains("drift | tested | "));

    // Blocks that cannot be judged: `null` and the row's note.
    let lone = LockRepo::of("hd-tested-unjudged", "verify-tests", "repo", &[]);
    let text = tested_common::lock_with(
        &[],
        &[tested_common::made_up_block(
            "KILN-RAMP",
            1,
            '7',
            &lone.head(),
        )],
    );
    lone.write("spec.lock", &text);
    lone.commit_all("evidence alone");
    let bin = hash_refusing_git_dir(&lone);
    let mut path = OsString::from(bin.as_os_str());
    path.push(":");
    path.push(
        vars_of(&lone)
            .iter()
            .find(|(name, _)| name == "PATH")
            .map(|(_, value)| value.clone())
            .unwrap_or_default(),
    );
    let vars = with_var(vars_of(&lone), "PATH", &path);
    let judged = judged_with(&lone, "evidence that cannot be judged", &vars, true);
    judged.assert_parity();
    let drift = judged.drift();
    assert_eq!(drift["state"], "measured", "{drift}");
    assert_eq!(drift["tested"], Value::Null, "{drift}");
    let notes = drift["notes"].as_array().expect("notes");
    assert!(
        notes.len() == 1
            && notes[0]
                .as_str()
                .is_some_and(|note| note.starts_with("tested: ")),
        "{drift}"
    );
    assert!(
        judged
            .json
            .stderr
            .lines()
            .any(|line| line == format!("note: drift: {}", notes[0].as_str().unwrap_or(""))),
        "{}",
        judged.json.show()
    );
    let document = judged.value();
    assert!(
        !document["notes"]
            .as_array()
            .expect("row 3's notes")
            .iter()
            .any(|note| note.as_str().is_some_and(|note| note.contains("tested"))),
        "never in the top-level notes: {}",
        document["notes"]
    );
}

// ------------------------------------------------------------ RD-6

/// RD-6: on the example tree Health's `drift | ` lines are plain's row
/// lines prefixed, in its order, the summary `drift: 19 fix, 6 check, 6
/// cannot, 8 ok; 2 skipped` ("Text and stderr"), after rows 4-5's
/// summaries and before `code:`; the LAW-BOUND `CAP` row as "Data" shows
/// it (`lock`, `levels`, `anchor` filled), its line as "Text and stderr"
/// shows it. Sub-lines stay unprefixed: anchor-states' moved and
/// candidate sub-lines under their `drift | ` line, two spaces in;
/// evidence lines `drift | tested | …` (verify-ecs). M: a sub-line
/// prefixed, a summary with verify's verdict or cascade.
#[test]
fn drift_text_is_verify_lines_prefixed() {
    let repo = example("hd-text");
    let judged = judge(&repo, "the example tree");
    let lines: Vec<&str> = judged.text.stdout.lines().collect();
    let summary = "drift: 19 fix, 6 check, 6 cannot, 8 ok; 2 skipped";
    let at = lines
        .iter()
        .position(|line| *line == summary)
        .unwrap_or_else(|| panic!("{summary}\n{}", judged.text.stdout));
    assert!(
        lines[at - 1].starts_with("unbound: "),
        "{}",
        judged.text.stdout
    );
    assert!(
        lines[at + 1].starts_with("code: "),
        "{}",
        judged.text.stdout
    );
    assert!(
        lines.contains(
            &"drift | fix | bad_marker dangling |  | implements | crates/tally/src/lib.rs | unanchored not_an_item"
        ),
        "{}",
        judged.text.stdout
    );
    assert!(
        lines.contains(
            &"drift | check | code_ahead code_changed body | LAW-BOUND | configures | crates/tally/src/counter.rs | item crates/tally::counter::CAP const_item"
        ),
        "{}",
        judged.text.stdout
    );
    let drift = judged.drift();
    assert_eq!(
        drift["counts"],
        json!({"rows": 39, "ok": 8, "fix": 19, "check": 6, "cannot": 6, "pre_existing": 0, "skipped": 2})
    );
    let cap: Vec<&Value> = drift["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .filter(|row| row["symbol"] == "crates/tally::counter::CAP" && row["node"] == "LAW-BOUND")
        .collect();
    assert_eq!(cap.len(), 1, "{drift}");
    let row = cap[0];
    assert_eq!(row["state"], "code_ahead");
    assert_eq!(row["category"], "code_changed");
    assert_eq!(row["detail"], "body");
    assert_eq!(row["class"], "check");
    assert_eq!(row["markers"], json!([{"line": 8, "rev": 2}]));
    assert_eq!(row["node_rev"], 2);
    assert_eq!(row["lock"]["rev"], 2);
    assert_eq!(row["lock"]["verified_at"], "2026-10-08");
    assert_eq!(row["compared"], "ast");
    assert_eq!(
        row["levels"],
        json!({"lock": ["sig", "body"], "markers": ["sig", "body"], "changed": ["body"]})
    );
    assert_eq!(
        row["anchor"],
        json!({"state": "exact", "from": null, "candidates": null})
    );
    let first = &drift["rows"][0];
    assert_eq!(
        first["node"], "",
        "the bare row first, by key order: {first}"
    );
    assert_eq!(first["state"], "bad_marker");
    assert_eq!(first["category"], "dangling");
    assert!(
        judged
            .text
            .stderr
            .lines()
            .all(|line| !line.starts_with("note: drift: "))
    );

    let anchors = anchor_common::base("hd-text-anchors");
    for name in anchor_common::SCENARIOS {
        anchor_common::change(&anchors, name);
    }
    let judged = judge(&anchors, "anchor-states: every scenario");
    let printed = ranks(&judged.text.stdout);
    let mut sub = 0;
    for (index, (rank, line)) in printed.iter().enumerate() {
        if *rank == 3 && line.starts_with("  ") {
            sub += 1;
            assert!(
                line.starts_with("  moved from ") || line.starts_with("  candidate "),
                "{line}"
            );
            let above = &printed[index - 1].1;
            assert!(
                above.starts_with("drift | ") || above.starts_with("  "),
                "a sub-line under its row line: {above}\n{line}"
            );
        }
    }
    assert!(
        printed
            .iter()
            .any(|(_, line)| line.starts_with("  moved from "))
            && printed
                .iter()
                .any(|(_, line)| line.starts_with("  candidate ")),
        "moved and candidate sub-lines met: {}",
        judged.text.stdout
    );
    assert!(sub > 0);
    assert!(
        !judged.text.stdout.contains("drift |   "),
        "sub-lines unprefixed: {}",
        judged.text.stdout
    );

    let (ecs_repo, _, _) = tested_common::ecs_with_blocks("hd-text-ecs");
    let judged = judge(&ecs_repo, "verify-ecs' blocks");
    assert!(
        judged
            .text
            .stdout
            .lines()
            .any(|line| line.starts_with("drift | tested | stale ")),
        "{}",
        judged.text.stdout
    );
}

// ------------------------------------------------------------ RD-3, TE-2, M4

/// How a scenario of the degrade matrix leaves row 6.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Expect {
    /// A step only row 6 takes: `cannot`, plain's `cannot` lines; rows
    /// 4-5 unaffected.
    Alone,
    /// A shared step (rows 4-6): `cannot`, rows 4-5's one cause.
    Shared,
    /// No `[code]`: `off`, the sentence.
    Off,
}

/// One scenario: a tree, the variables its runs get, whether the modes
/// apply (a bad `[verify]` has none), the expectation.
struct Scenario {
    what: &'static str,
    repo: LockRepo,
    vars: Vars,
    modes: bool,
    expect: Expect,
    /// Paths made unreadable for the scenario's life.
    _guards: Vec<ModeGuard>,
}

/// The golden with `tail` appended to its config (uncommitted).
fn golden_with(label: &str, tail: &str) -> LockRepo {
    let repo = golden(label);
    repo.append("specengine.toml", tail);
    repo
}

/// The golden in a merge whose `spec.lock` conflicts (both branches
/// changed one entry's `verified_at`).
fn unmerged(label: &str) -> LockRepo {
    let repo = golden(label);
    let lock = repo.lock_text();
    let at = lock.find("verified_at = \"").expect("an entry");
    let end = at + lock[at..].find('\n').expect("a line end");
    let line = lock[at..end].to_owned();
    let dated = |date: &str| lock.replacen(&line, &format!("verified_at = \"{date}\""), 1);
    repo.git(&["checkout", "-q", "-b", "side"]);
    repo.write("spec.lock", dated("2026-11-01"));
    repo.commit_all("side lock");
    repo.git(&["checkout", "-q", "main"]);
    repo.write("spec.lock", dated("2026-11-02"));
    repo.commit_all("main lock");
    let merge = repo
        .git
        .git_output(&repo.root, &["merge", "-q", "--no-edit", "side"], &[]);
    assert!(!merge.status.success(), "the scenario: spec.lock conflicts");
    repo
}

/// The degrade matrix's rows ("Degrade matrix"), each on its own git
/// repository of the golden (spec-c a plain copy).
fn scenarios() -> Vec<Scenario> {
    let mut out = Vec::new();
    let plain = |what: &'static str, repo: LockRepo, expect: Expect| {
        let vars = vars_of(&repo);
        Scenario {
            what,
            repo,
            vars,
            modes: true,
            expect,
            _guards: Vec::new(),
        }
    };

    // Row 6 alone.
    let repo = golden_with("hd-dm-verify", "\n[verify]\nmode = \"strict\"\n");
    let mut scenario = plain("a bad [verify] value", repo, Expect::Alone);
    scenario.modes = false;
    out.push(scenario);
    let repo = golden_with("hd-dm-verify-key", "\n[verify]\nflavour = 1\n");
    let mut scenario = plain("an unknown [verify] key", repo, Expect::Alone);
    scenario.modes = false;
    out.push(scenario);

    let repo = LockRepo::uncommitted("hd-dm-worktree");
    repo.write(
        "spec.lock",
        fs::read(fixture("spec-lock").join("code-a.lock")).expect("the golden lock"),
    );
    out.push(plain("no worktree", repo, Expect::Alone));

    let repo = golden("hd-dm-no-git");
    let empty = no_git_dir(&repo);
    let mut scenario = plain("no git on PATH", repo, Expect::Alone);
    scenario.vars = with_var(scenario.vars, "PATH", empty.as_os_str());
    out.push(scenario);

    let repo = golden("hd-dm-git-fails");
    let failing = failing_git_dir(&repo);
    let mut scenario = plain("a git failure", repo, Expect::Alone);
    scenario.vars = with_var(scenario.vars, "PATH", failing.as_os_str());
    out.push(scenario);

    out.push(plain(
        "an unmerged lock",
        unmerged("hd-dm-unmerged"),
        Expect::Alone,
    ));

    let repo = golden("hd-dm-invalid");
    repo.write(
        "spec.lock",
        "# spec.lock\nformat = 2\n[[binding]\nnode = \"LAW-BOUND\"\n",
    );
    out.push(plain("an invalid lock", repo, Expect::Alone));

    let repo = golden("hd-dm-not-utf8");
    repo.write("spec.lock", b"format = 2\n# \xff\n".as_slice());
    out.push(plain("a lock not UTF-8", repo, Expect::Alone));

    out.push(plain(
        "a format-1 lock",
        golden_format_1("hd-dm-format-1"),
        Expect::Alone,
    ));

    let repo = golden("hd-dm-recipe");
    repo.replace(
        "spec.lock",
        "tree-sitter-rust/0.24.2",
        "tree-sitter-rust/0.24.1",
    );
    out.push(plain("a lock of another recipe", repo, Expect::Alone));

    // Shared with rows 4-5.
    let repo = golden("hd-dm-code");
    repo.replace(
        "specengine.toml",
        "roots   = [\"crates\", \"data\", \"tools/loose.rs\"]",
        "roots   = \"crates\"",
    );
    out.push(plain("a bad [code]", repo, Expect::Shared));

    let repo = golden_with("hd-dm-framework", "frameworks = [\"nosuch\"]\n");
    out.push(plain("an unknown framework", repo, Expect::Shared));

    let repo = golden("hd-dm-gap");
    let guard = ModeGuard::set(repo.root.join("crates/tally/src/lib.rs"), 0o000);
    let mut scenario = plain("a code walk gap", repo, Expect::Shared);
    scenario._guards.push(guard);
    out.push(scenario);

    let repo = golden("hd-dm-home");
    let mut scenario = plain("HOME unset", repo, Expect::Shared);
    scenario.vars = without_var(scenario.vars, "HOME");
    out.push(scenario);

    // Off.
    let repo = golden("hd-dm-off");
    let config = repo.read("specengine.toml");
    let (head, _) = config.split_once("\n[code]\n").expect("a [code] table");
    repo.write("specengine.toml", format!("{head}\n"));
    out.push(plain("no [code]", repo, Expect::Off));

    let repo = LockRepo::uncommitted("hd-dm-spec-c");
    let c = repo.scratch.copy("spec-c", "spec-c");
    let mut scenario = plain("fixtures/spec-c", repo, Expect::Off);
    scenario.repo.root = c;
    out.push(scenario);
    out
}

/// RD-3, TE-2 (M4): the degrade matrix × `observe`,
/// `enforce-introduced`, `enforce` (a bad `[verify]` alone): exit 0 in
/// both modes; a step only row 6 takes (a bad `[verify]` value or key, no
/// worktree, no git, a git failure, an unmerged, invalid, not-UTF-8,
/// format-1 or other-recipe lock) leaves row 6 `cannot` with plain's
/// `cannot` lines in its order (the first as "Data" names it), rows 4-5
/// measured; a shared step (a bad `[code]`, an unknown framework, a code
/// walk gap, `HOME` unset) rows 4 and 6 `cannot` with the same one cause
/// (row 5, no rule here, `not_configured` first); no
/// `[code]` (code-a without it, `fixtures/spec-c`) `off` with the
/// sentence, no note. Never 0, never partial: `counts`, `rows`, `tested`
/// null, `notes` `[]`; `drift: <state>: <cause>` per cause, `note: drift:
/// <cause>` on stderr for `cannot` alone. M4: a run cause rendered as 0 or
/// `[]`.
#[test]
fn a_run_cause_is_drift_cannot_never_zero() {
    let all = scenarios();
    for scenario in &all {
        let root = &scenario.repo.root;
        let config = fs::read_to_string(root.join("specengine.toml")).expect("config");
        let modes: Vec<Option<&str>> = if scenario.modes {
            MODES.iter().map(|mode| Some(*mode)).collect()
        } else {
            vec![None]
        };
        for mode in modes {
            fs::write(root.join("specengine.toml"), &config).expect("config");
            if let Some(mode) = mode {
                set_mode(root, mode);
            }
            let context = format!("{} [{}]", scenario.what, mode.unwrap_or("-"));
            let json = spec_in(root, &["health", "--json"], &scenario.vars);
            let text = spec_in(root, &["health"], &scenario.vars);
            json.code(0);
            text.code(0);
            let document: Value = serde_json::from_str(json.stdout.trim_end())
                .unwrap_or_else(|error| panic!("{context}: {error}\n{}", json.show()));
            let drift = &document["drift"];
            let causes: Vec<String> = drift["causes"]
                .as_array()
                .unwrap_or_else(|| panic!("{context}: causes: {drift}"))
                .iter()
                .map(|cause| cause.as_str().expect("a cause").to_owned())
                .collect();
            assert!(!causes.is_empty(), "{context}: a cause: {drift}");
            let state = match scenario.expect {
                Expect::Off => "off",
                Expect::Alone | Expect::Shared => "cannot",
            };
            assert_eq!(
                drift,
                &json!({"state": state, "causes": causes, "counts": null, "rows": null,
                    "tested": null, "notes": []}),
                "{context}: never 0, never partial\n{}",
                json.show()
            );
            match scenario.expect {
                Expect::Alone => {
                    let plain = spec_in(root, &["verify", "--json"], &scenario.vars);
                    let plain: Value = serde_json::from_str(plain.stdout.trim_end())
                        .unwrap_or_else(|error| panic!("{context}: {error}\n{}", plain.show()));
                    assert_eq!(
                        drift["causes"], plain["cannot"],
                        "{context}: plain's `cannot` lines"
                    );
                    assert_eq!(
                        document["assumes"]["state"], "measured",
                        "{context}: rows 4-5 unaffected"
                    );
                    assert_eq!(document["unbound"]["state"], "not_configured", "{context}");
                }
                Expect::Shared => {
                    // Row 5 holds no rule: `not_configured` comes first.
                    assert_eq!(causes.len(), 1, "{context}: the one shared cause");
                    assert_eq!(document["assumes"]["state"], "cannot", "{context}");
                    assert_eq!(
                        document["assumes"]["causes"], drift["causes"],
                        "{context}: the same cause"
                    );
                    assert_eq!(document["unbound"]["state"], "not_configured", "{context}");
                }
                Expect::Off => {
                    assert_eq!(causes, [NO_CODE], "{context}");
                    for row in ["assumes", "unbound"] {
                        assert_eq!(document[row]["state"], "off", "{context}: {row}");
                    }
                }
            }
            let word = if scenario.expect == Expect::Off {
                "off"
            } else {
                "cannot"
            };
            assert_eq!(
                drift_summaries(&text.stdout),
                causes
                    .iter()
                    .map(|cause| format!("drift: {word}: {cause}"))
                    .collect::<Vec<_>>(),
                "{context}\n{}",
                text.show()
            );
            assert!(
                !text.stdout.lines().any(|line| line.starts_with("drift | ")),
                "{context}: no row line\n{}",
                text.stdout
            );
            let notes: Vec<String> = if scenario.expect == Expect::Off {
                Vec::new()
            } else {
                causes
                    .iter()
                    .map(|cause| format!("note: drift: {cause}"))
                    .collect()
            };
            for run in [&json, &text] {
                assert_eq!(
                    drift_notes(&run.stderr),
                    notes,
                    "{context}: stderr\n{}",
                    run.show()
                );
            }
            assert_no_verdict(&json, &text, &context);
        }
        fs::write(root.join("specengine.toml"), &config).expect("config");
    }
    // The causes "Data" and RD-8 name, met as written.
    let first = |what: &str| -> String {
        let scenario = all
            .iter()
            .find(|scenario| scenario.what == what)
            .expect("a scenario");
        let run = spec_in(&scenario.repo.root, &["health", "--json"], &scenario.vars);
        let document: Value = serde_json::from_str(run.stdout.trim_end()).expect("JSON");
        document["drift"]["causes"][0]
            .as_str()
            .expect("a cause")
            .to_owned()
    };
    assert_eq!(first("no worktree"), NO_WORKTREE);
    assert_eq!(first("no git on PATH"), NO_GIT);
    assert_eq!(first("an unmerged lock"), UNMERGED);
    assert!(first("an invalid lock").starts_with("spec.lock:"));
    assert_eq!(
        first("a lock not UTF-8"),
        "spec.lock: not UTF-8: SpecEngine writes it"
    );
    assert!(first("a bad [verify] value").starts_with("specengine.toml:"));
    assert!(first("a code walk gap").starts_with("the code walk is incomplete: "));
    assert!(first("HOME unset").starts_with("HOME is not set"));
}

/// "Text and stderr": a row-6 cause holding a control character (a
/// `[verify] mode` value with BEL, the TOML escape `\u0007`): JSON
/// `causes` raw, as plain's `cannot`; the stdout line `drift: cannot: …`
/// through `escape_controls`; the stderr note escaped as verify escapes
/// its own stderr (the developer's deviation 5: the spec names no
/// stderr escaping; rows 4-5's notes stay raw, 9a's G4). M: a raw BEL on
/// stdout.
#[test]
fn a_row_6_cause_is_escaped_in_text_raw_in_json() {
    let repo = golden_with("hd-escape", "\n[verify]\nmode = \"str\\u0007ict\"\n");
    let vars = vars_of(&repo);
    let json = spec_in(&repo.root, &["health", "--json"], &vars);
    let text = spec_in(&repo.root, &["health"], &vars);
    json.code(0);
    text.code(0);
    let plain = spec_in(&repo.root, &["verify", "--json"], &vars);
    let plain: Value = serde_json::from_str(plain.stdout.trim_end()).expect("plain's JSON");
    let document: Value = serde_json::from_str(json.stdout.trim_end()).expect("JSON");
    assert_eq!(document["drift"]["causes"], plain["cannot"], "raw, plain's");
    let cause = document["drift"]["causes"][0]
        .as_str()
        .expect("a cause")
        .to_owned();
    assert!(
        cause.contains('\u{7}'),
        "premise: BEL in the cause: {cause:?}"
    );
    let escaped = cause.replace('\u{7}', "\\u{7}");
    assert_eq!(
        drift_summaries(&text.stdout),
        [format!("drift: cannot: {escaped}")],
        "{}",
        text.stdout
    );
    assert!(!text.stdout.contains('\u{7}'), "{}", text.stdout);
    for run in [&json, &text] {
        assert_eq!(
            drift_notes(&run.stderr),
            [format!("note: drift: {escaped}")],
            "{}",
            run.show()
        );
    }
}

/// "Degrade matrix", the row "`code_refs`, an unknown stored word, the
/// queue, `[health]`": rows 4-5's own causes leave row 6 as plain verify
/// judges the tree: on the example tree, a malformed `[[health.bound]]`
/// (row 5 `cannot`), then an unknown stored binding word in the code
/// index (rows 4-5 `cannot`, `place`'s error): row 6 measured, plain's
/// rows. M: row 6 failed by a rows 4-5 step.
#[test]
fn rows_4_5_causes_leave_row_6_measured() {
    let repo = example("hd-row-4-5-causes");
    repo.append("specengine.toml", "\n[[health.bound]]\nkinds = 3\n");
    let judged = judge(&repo, "a malformed rule");
    let document = judged.value();
    assert_eq!(
        document["unbound"]["state"], "cannot",
        "{}",
        document["unbound"]
    );
    assert_eq!(judged.drift()["state"], "measured");

    let config = repo.read("specengine.toml");
    let (head, _) = config.split_once("\n[[health.bound]]\n").expect("the rule");
    repo.write("specengine.toml", format!("{head}\n"));
    repo.spec_piped(&repo.root, &["index"]).code(0);
    let db = crate::common::data_dir(&repo.home).join("code-a.db");
    let sqlite = std::process::Command::new(crate::common::proposal::sqlite3())
        .arg(&db)
        .arg("UPDATE code_markers SET binding = 'bogus';")
        .output()
        .expect("the system's sqlite3");
    assert!(
        sqlite.status.success(),
        "{}",
        String::from_utf8_lossy(&sqlite.stderr)
    );
    let judged = judged_with(&repo, "an unknown stored word", &vars_of(&repo), true);
    let document = judged.value();
    assert_eq!(
        document["assumes"]["state"], "cannot",
        "premise: rows 4-5 fail on the word: {}",
        document["assumes"]
    );
    judged.assert_parity();
    assert_eq!(judged.drift()["state"], "measured", "{}", judged.drift());
}

// ------------------------------------------------------------ RD-3, M5

/// RD-3 (M5): Health never exits 1 and never carries a verdict, a mode or
/// a hint: verify-ecs with a bound body edited and a marker raised past
/// its node (a Fix: plain exits 0 under `observe`, 1 under `enforce` and
/// `enforce-introduced`, judged as `enforce` with its note) and the
/// example tree (plain exits 2: Cannot rows) under each mode, JSON and
/// text: exit 0, row 6 measured with its rows, no `mode` or `verdict`
/// key, no `hint:` line, no mode note, no `blocked`, the summary without
/// verdict, mode or cascade. M5: exit 1, or a verdict, mode or hint under
/// `enforce`.
#[test]
fn drift_never_exits_1_or_carries_a_verdict() {
    let game = ecs("hd-exit-ecs");
    game.replace(
        STAMINA,
        "(stamina.current - cost * dt).max(0.0)",
        "(stamina.current - cost * dt * 2.0).max(0.0)",
    );
    game.replace(
        STAMINA,
        "// @implements PERK-REGEN@1\n",
        "// @implements PERK-REGEN@2\n",
    );
    let tally = example("hd-exit-tally");
    for (repo, label) in [
        (&game, "verify-ecs, a body edited"),
        (&tally, "the example tree"),
    ] {
        let config = repo.read("specengine.toml");
        let mut exits = Vec::new();
        for mode in MODES {
            repo.write("specengine.toml", &config);
            set_mode(&repo.root, mode);
            let context = format!("{label} [{mode}]");
            let judged = judge(repo, &context);
            exits.push(judged.plain_json.code);
            assert_eq!(judged.drift()["state"], "measured", "{context}");
            assert!(
                judged.drift()["rows"]
                    .as_array()
                    .is_some_and(|rows| rows.iter().any(|row| row["class"] == "fix")),
                "{context}: a Fix listed"
            );
            assert!(
                !judged.json.stdout.contains("\"mode\"")
                    && !judged.json.stdout.contains("\"verdict\""),
                "{context}"
            );
            assert!(
                !judged.text.stdout.contains(&format!("[{mode}]")),
                "{context}"
            );
        }
        repo.write("specengine.toml", &config);
        if label.starts_with("verify-ecs") {
            assert_eq!(exits, [0, 1, 1], "premise: plain's exits by mode");
        } else {
            assert_eq!(exits, [2, 2, 2], "premise: plain's exits by mode");
        }
    }
}

// ------------------------------------------------------------ RD-2

/// RD-2 (the pattern of `health_refreshes_the_code_index_as_refs_does`):
/// after `spec index`, the example tree's `CAP` edit: Health shows the
/// `code_ahead` row and leaves `spec index --json` `code.parsed` at 0; a
/// repeat re-derives nothing and prints the same bytes. M: row 6 reads
/// files the refresh did not store, or a second refresh.
#[test]
fn health_drift_leaves_the_code_index_fresh() {
    let repo = golden("hd-fresh");
    repo.spec_piped(&repo.root, &["index"]).code(0);
    repo.replace(COUNTER, CAP_10, CAP_12);
    let first = judge(&repo, "the example tree after an index");
    assert!(
        first.drift()["rows"]
            .as_array()
            .expect("rows")
            .iter()
            .any(|row| row["state"] == "code_ahead"),
        "{}",
        first.drift()
    );
    let parsed = || -> Value {
        let run = repo.spec_piped(&repo.root, &["--json", "index"]);
        run.code(0);
        run.json()["code"]["parsed"].clone()
    };
    assert_eq!(parsed(), 0, "Health refreshed the edited file once");
    let again = repo.spec_piped(&repo.root, &["health", "--json"]);
    assert_eq!(again.stdout, first.json.stdout, "a repeat");
    assert_eq!(parsed(), 0, "a repeat re-derives nothing");
}

// ------------------------------------------------------------ RD-5, RD-8

/// RD-5, RD-8: rows 3-5's bytes do not depend on row 6: on the example
/// tree with no `git` on `PATH`, row 6 is `cannot` with RD-8's cause and
/// rows 3-5 keep the document's bytes (its first six keys) and the text's
/// lines; from a subdirectory, through `--root` from elsewhere and in a
/// byte copy of the repository (`.git` too) at another path, every byte of
/// both modes is the same. M: row 6 judged from the cwd, or rows 3-5
/// reading row 6.
#[test]
fn rows_3_to_5_keep_their_bytes_and_row_6_is_rooted() {
    let repo = example("hd-rooted");
    let vars = vars_of(&repo);
    let empty = no_git_dir(&repo);
    let no_git = with_var(vars.clone(), "PATH", empty.as_os_str());
    let both = |cwd: &Path, root: Option<&Path>, vars: &[(OsString, OsString)]| -> Vec<String> {
        let root = root.map(|root| root.to_str().expect("UTF-8").to_owned());
        let mut out = Vec::new();
        for json in [true, false] {
            let mut args: Vec<&str> = Vec::new();
            if let Some(root) = &root {
                args.extend(["--root", root.as_str()]);
            }
            args.push("health");
            if json {
                args.push("--json");
            }
            let run = spec_in(cwd, &args, vars);
            run.code(0);
            out.push(run.stdout);
            out.push(run.stderr);
        }
        out
    };
    let with = both(&repo.root, None, &vars);
    let without = both(&repo.root, None, &no_git);
    let measured = J::parse(with[0].trim_end());
    let cannot = J::parse(without[0].trim_end());
    assert_eq!(
        cannot.get("drift").get("causes"),
        &J::Arr(vec![s(NO_GIT)]),
        "RD-8's cause"
    );
    assert_eq!(measured.get("drift").get("state"), &s("measured"));
    for key in &HEALTH_KEYS[..6] {
        assert_eq!(measured.get(key), cannot.get(key), "row 3-5's `{key}`");
    }
    let rows_3_5 = |text: &str| -> Vec<String> {
        text.lines()
            .filter(|line| !line.starts_with("drift"))
            .map(str::to_owned)
            .collect()
    };
    assert_eq!(
        rows_3_5(&with[2]),
        rows_3_5(&without[2]),
        "the text's rows 3-5"
    );
    // Row 6's notes aside, rows 4-5's stderr is the same.
    let stderr_3_5 = |text: &str| -> String {
        text.lines()
            .filter(|line| !line.starts_with("note: drift: "))
            .map(|line| format!("{line}\n"))
            .collect()
    };
    assert_eq!(stderr_3_5(&with[3]), stderr_3_5(&without[3]));

    let sub = repo.root.join("crates");
    assert_eq!(both(&sub, None, &vars), with, "from a subdirectory");
    let elsewhere = repo.scratch.dir("elsewhere");
    assert_eq!(
        both(&elsewhere, Some(&repo.root), &vars),
        with,
        "--root from elsewhere"
    );
    let copy = crate::verify_common::clone_dir(&repo, "far/away/copy");
    assert_eq!(
        both(&copy, None, &vars),
        with,
        "a byte copy at another path"
    );
}

// ------------------------------------------------------------ RD-8 (the library)

/// "Description and interactions": `health(env, globals, &HealthRequest {
/// git })` reads git through the caller's `GitEnv`, never the process's:
/// the repository's sandbox gives the binary's document byte for byte
/// (`render_json`); the same with a `PATH` holding no `git` gives row 6
/// `cannot` with RD-8's cause and rows 3-5 unchanged, while this test
/// process's own `PATH` holds one. M: the library calls `process_git`.
#[test]
fn health_takes_the_callers_git_env() {
    let repo = example("hd-git-env");
    let binary = repo.spec_piped(&repo.root, &["health", "--json"]);
    binary.code(0);
    let call = |git| -> String {
        let outcome = health(
            &repo.env(&repo.root),
            &Globals::default(),
            &HealthRequest { git },
        )
        .expect("the library answers");
        render_json(&Outcome::Health(outcome))
    };
    assert_eq!(
        call(repo.git_env(&repo.root)),
        binary.stdout,
        "the caller's git: the binary's document"
    );
    let empty = no_git_dir(&repo);
    let without = call(repo.git_env(&repo.root).with_var("PATH", empty.as_os_str()));
    let measured = J::parse(binary.stdout.trim_end());
    let cannot = J::parse(without.trim_end());
    assert_eq!(cannot.get("drift").get("state"), &s("cannot"), "{without}");
    assert_eq!(
        cannot.get("drift").get("causes"),
        &J::Arr(vec![s(NO_GIT)]),
        "{without}"
    );
    for key in &HEALTH_KEYS[..6] {
        assert_eq!(measured.get(key), cannot.get(key), "`{key}`");
    }
}

// ------------------------------------------------------------ TE-6

/// The median of five timings of `call`.
fn median(mut call: impl FnMut() -> Duration) -> Duration {
    let mut times: Vec<Duration> = (0..5).map(|_| call()).collect();
    times.sort();
    times[2]
}

/// `files` unrelated `.rs` files in a crate of their own under `crates/`.
fn filler(repo: &LockRepo, files: usize) {
    repo.write(
        "crates/filler/Cargo.toml",
        "[package]\nname = \"filler\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    );
    let mut lib = String::new();
    for at in 0..files {
        lib.push_str(&format!("pub mod part_{at:05};\n"));
        repo.write(
            &format!("crates/filler/src/part_{at:05}.rs"),
            format!(
                "/// Part {at}.\npub fn value_{at:05}(input: u32) -> u32 {{\n    input.wrapping_mul({at}).wrapping_add(1)\n}}\n"
            ),
        );
    }
    repo.write("crates/filler/src/lib.rs", lib);
}

/// TE-6, manual (`#[ignore]`), release, a record ("Implementation"): the
/// example tree plus 1 000, then 10 000 unrelated `.rs` files, committed,
/// the indexes warm; the medians of five warm runs of plain `spec verify`,
/// `spec health --json` with the lock, without it (every binding
/// `unlocked`: the flood), and with no `git` on `PATH` (row 6 `cannot`:
/// rows 3-5 alone); row 6's share is the difference. A6: a warm Health
/// read ≤ 2 s, row 6's share ≤ plain verify's.
///
/// `cargo nextest run -p specengine-cli --release --test health
/// health_drift_cost --run-ignored ignored-only --no-capture`
#[test]
#[ignore = "manual (TE-6): release, figures recorded in the spec"]
fn health_drift_cost() {
    for files in [1_000usize, 10_000] {
        let repo = golden(&format!("hd-cost-{files}"));
        filler(&repo, files);
        repo.commit_all("unrelated files");
        repo.replace(COUNTER, CAP_10, CAP_12);
        repo.spec_piped(&repo.root, &["index"]).code(0);
        let vars = vars_of(&repo);
        let empty = no_git_dir(&repo);
        let no_git = with_var(vars.clone(), "PATH", empty.as_os_str());
        let time = |args: &[&str], vars: &[(OsString, OsString)]| {
            median(|| {
                let started = Instant::now();
                let run = spec_in(&repo.root, args, vars);
                let took = started.elapsed();
                assert!(!run.stdout.is_empty(), "{}", run.show());
                took
            })
        };
        let whole = time(&["verify"], &vars);
        let locked = time(&["health", "--json"], &vars);
        let rows_3_5 = time(&["health", "--json"], &no_git);
        let lock = repo.lock().expect("the lock");
        fs::remove_file(repo.root.join("spec.lock")).expect("the lock removed");
        let unlocked = time(&["health", "--json"], &vars);
        repo.write("spec.lock", lock);
        let share = locked.saturating_sub(rows_3_5);
        eprintln!(
            "TE-6: example tree + {files} unrelated .rs files, warm indexes, median of 5: plain verify {} ms, health with the lock {} ms, without it {} ms, with no git on PATH {} ms; row 6's share {} ms",
            whole.as_millis(),
            locked.as_millis(),
            unlocked.as_millis(),
            rows_3_5.as_millis(),
            share.as_millis()
        );
        assert!(
            locked <= Duration::from_secs(2) && unlocked <= Duration::from_secs(2),
            "A6: a warm Health read within 2 s"
        );
        assert!(
            share <= whole,
            "A6: row 6's share {share:?} at most plain verify's {whole:?}"
        );
    }
}
