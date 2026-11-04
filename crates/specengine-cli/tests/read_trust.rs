//! docs/features/read-trust.md (RM-0054), the CLI half through the library
//! calls the binary makes, inside and outside `specengine_cli::trusted`:
//! - AC-4 "Two modes, one answer" (T1, T2): every row of
//!   `store_reads_before.json` replays equal inside `trusted` of an
//!   `IndexTrust` whose first read walked, plain and `--json`, stderr
//!   included, with no walk after the first; `search`, `tree` and `flow` on
//!   each configured fixture answer as the walking read; on a scratch tree
//!   with a written `[paths]` root missing and a mode-000 directory, a
//!   trusted `search` and a trusted `show` print the walk's two warnings.
//! - AC-5 "Trust skips the walk": a file deleted after a walking read is
//!   still found by a trusted `search`, `walks()` unchanged, and not by a
//!   read with no scope; a trust of another root keeps no books.
//! - AC-6 (g): a hand edit is not seen by a trusted `search` until
//!   `with_every`'s interval passes, then found, `walks()` up by one; a
//!   `show` of a node in that file sees an edit at once (T5) and marks the
//!   trust dirty, so the next read walks.
//! - Beyond the criteria, R13 and R14: a trusted `tree` re-parses a file
//!   whose bytes moved (Q7) and marks the trust dirty; `spec index` walks
//!   inside a scope.
//! - AC-7 "One state per read": `tests/freshness.rs`' racing `tree`, and
//!   `tests/store_reads.rs` AC-7's flipper over `show --links` and `bundle`
//!   (here with the mtime restored to the nanosecond on every swap), give
//!   no mixed answer inside `trusted`.
//!
//! A read's answer is what `spec` prints for it (`src/main.rs`): on `Ok`,
//! `stderr_lines()` on stderr and `render_text` or `render_json` on stdout,
//! exit `outcome.exit()`; on `Err`, the error's line on stderr, exit its
//! own. Every test works on scratch copies under `std::env::temp_dir()`,
//! each with its own `HOME`; the fixtures are only read. Non-Latin
//! characters are Unicode escapes.

#![cfg(unix)]

mod common;

use std::collections::BTreeSet;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use common::{Scratch, fixture};
use serde::Deserialize;
use specengine_cli::{
    BundleRequest, CliError, Env, FlowRequest, Globals, IndexRequest, IndexTrust, Outcome,
    RECONCILE_EVERY, SearchRequest, ShowRequest, TreeRequest, render_json, render_text, trusted,
};
use specengine_core::ProjectConfig;
use specengine_store::{GitEnv, WorkingTree, b3_hash, check_input};

/// `store-reads`' golden (`tests/store_reads.rs`).
const GOLDEN: &str = "store_reads_before.json";
/// The golden's rows over its 19 fixtures ("Acceptance criteria" AC-4).
const GOLDEN_ROWS: usize = 2322;
/// A stream this long or shorter, and UTF-8, is kept as its bytes.
const KEEP_BYTES: usize = 160;
/// The one configured fixture that opens no index (no `[project] slug`).
const NO_INDEX: &str = "corpus-mini";

// ------------------------------------------------------------------ a place

/// A scratch copy of one tree, its own `HOME` and an empty working
/// directory, laid out as `store_reads.rs` plays a row: `<scratch>/copy`,
/// `<scratch>/homes/h`, `<scratch>/cwd`.
struct Place {
    scratch: Scratch,
    root: PathBuf,
    home: PathBuf,
    cwd: PathBuf,
    env: Env,
    globals: Globals,
    git: GitEnv,
}

impl Place {
    /// A copy of `fixtures/<name>`.
    fn copy(name: &str) -> Self {
        let scratch = Scratch::new(&format!("read-trust-{name}"));
        let root = scratch.copy(name, "copy");
        Self::over(scratch, root)
    }

    /// A tree written by `write` under `<scratch>/copy`.
    fn written(name: &str, write: impl FnOnce(&Path)) -> Self {
        let scratch = Scratch::new(&format!("read-trust-{name}"));
        let root = scratch.dir("copy");
        write(&root);
        Self::over(scratch, root)
    }

    fn over(scratch: Scratch, root: PathBuf) -> Self {
        let home = scratch.home("h");
        let cwd = scratch.dir("cwd");
        Self {
            env: Env {
                cwd: cwd.clone(),
                home: Some(home.as_os_str().to_owned()),
                xdg_data_home: None,
            },
            globals: Globals {
                root: Some(root.clone()),
                config: None,
            },
            // The binary runs with `HOME` alone (`store_reads.rs`' `run`).
            git: GitEnv::new(cwd.clone(), [("HOME", home.as_os_str().to_owned())]),
            scratch,
            root,
            home,
            cwd,
        }
    }

    /// `spec index`, walking.
    fn index(&self) -> Result<(), CliError> {
        specengine_cli::index(&self.env, &self.globals, &IndexRequest { full: false }).map(|_| ())
    }

    fn show(&self, reference: &str, links: bool, archive: bool) -> Result<Outcome, CliError> {
        specengine_cli::show(
            &self.env,
            &self.globals,
            &ShowRequest {
                reference: reference.to_owned(),
                links,
                archive,
                bindings: false,
                git: self.git.clone(),
            },
        )
        .map(Outcome::Show)
    }

    fn bundle(&self, reference: &str, budget: Option<i64>) -> Result<Outcome, CliError> {
        specengine_cli::bundle(
            &self.env,
            &self.globals,
            &BundleRequest {
                references: vec![reference.to_owned()],
                budget,
                git: self.git.clone(),
            },
        )
        .map(Outcome::Bundle)
    }

    fn search(&self, term: &str) -> Result<Outcome, CliError> {
        specengine_cli::search(
            &self.env,
            &self.globals,
            &SearchRequest {
                terms: vec![term.to_owned()],
                kinds: Vec::new(),
                limit: None,
                archive: false,
            },
        )
        .map(Outcome::Search)
    }

    fn tree(&self, archive: bool) -> Result<Outcome, CliError> {
        self.tree_from(None, archive)
    }

    /// `tree [ROOT]`.
    fn tree_from(&self, start: Option<&str>, archive: bool) -> Result<Outcome, CliError> {
        specengine_cli::tree(
            &self.env,
            &self.globals,
            &TreeRequest {
                root: start.map(str::to_owned),
                depth: None,
                kinds: Vec::new(),
                archive,
            },
        )
        .map(Outcome::Tree)
    }

    fn flow(&self, trace: bool) -> Result<Outcome, CliError> {
        specengine_cli::flow(
            &self.env,
            &self.globals,
            &FlowRequest {
                root: None,
                depth: None,
                trace,
            },
        )
        .map(|flow| Outcome::Flow(Box::new(flow)))
    }

    /// One of the golden's reads: `show REF [--links [--archive]]` or
    /// `bundle REF [--budget N]`.
    fn golden_read(&self, args: &[String]) -> Result<Outcome, CliError> {
        let flag = |name: &str| args.iter().any(|arg| arg == name);
        match args[0].as_str() {
            "show" => self.show(&args[1], flag("--links"), flag("--archive")),
            "bundle" => {
                let budget = args.iter().position(|arg| arg == "--budget").map(|at| {
                    args[at + 1]
                        .parse::<i64>()
                        .unwrap_or_else(|error| panic!("{args:?}: the budget: {error}"))
                });
                self.bundle(&args[1], budget)
            }
            other => panic!("not a golden read: {other} ({args:?})"),
        }
    }

    /// The scratch's paths, longest first, with what the golden writes.
    fn placeholders(&self) -> Vec<(Vec<u8>, &'static str)> {
        let text = |path: &Path| path.to_str().expect("UTF-8 path").as_bytes().to_vec();
        let mut out = vec![
            (text(&self.root), "<root>"),
            (text(&self.home), "<home>"),
            (text(&self.cwd), "<cwd>"),
            (text(self.scratch.path()), "<scratch>"),
        ];
        out.sort_by_key(|(from, _)| std::cmp::Reverse(from.len()));
        out
    }
}

/// What `spec` prints for a read.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Printed {
    code: i32,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

impl Printed {
    fn of(result: Result<Outcome, CliError>, json: bool) -> Self {
        match result {
            Ok(outcome) => {
                let mut stderr = String::new();
                for line in outcome.stderr_lines() {
                    stderr.push_str(&line);
                    stderr.push('\n');
                }
                let stdout = if json {
                    render_json(&outcome)
                } else {
                    render_text(&outcome)
                };
                Self {
                    code: i32::from(outcome.exit().code()),
                    stdout: stdout.into_bytes(),
                    stderr: stderr.into_bytes(),
                }
            }
            Err(error) => Self {
                code: i32::from(error.exit.code()),
                stdout: Vec::new(),
                stderr: format!("{error}\n").into_bytes(),
            },
        }
    }

    fn text(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into_owned()
    }

    fn show(&self) -> String {
        format!(
            "exit {}\n--- stdout\n{}--- stderr\n{}",
            self.code,
            String::from_utf8_lossy(&self.stdout),
            String::from_utf8_lossy(&self.stderr)
        )
    }
}

/// A read printed plain and as JSON: two calls, as the golden's two runs.
fn both(read: &dyn Fn() -> Result<Outcome, CliError>) -> [Printed; 2] {
    [Printed::of(read(), false), Printed::of(read(), true)]
}

/// `read` inside `trusted(&[trust])`, printed plain and as JSON.
fn both_trusted(trust: &IndexTrust, read: &dyn Fn() -> Result<Outcome, CliError>) -> [Printed; 2] {
    [
        trusted(&[trust], || Printed::of(read(), false)),
        trusted(&[trust], || Printed::of(read(), true)),
    ]
}

// ------------------------------------------------------------------ the golden

#[derive(Deserialize)]
struct Golden {
    rows: Vec<Row>,
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
struct Row {
    fixture: String,
    args: Vec<String>,
    code: String,
    text: Stream,
    json: Stream,
    stderr: Stream,
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(untagged)]
enum Stream {
    Bytes(String),
    Digest { b3: String, len: usize },
}

impl Stream {
    fn of(bytes: &[u8]) -> Self {
        match std::str::from_utf8(bytes) {
            Ok(text) if bytes.len() <= KEEP_BYTES => Self::Bytes(text.to_owned()),
            _ => Self::Digest {
                b3: b3_hash(bytes),
                len: bytes.len(),
            },
        }
    }

    fn describe(&self) -> String {
        match self {
            Self::Bytes(text) => format!("{text:?}"),
            Self::Digest { b3, len } => format!("{len} bytes, {b3}"),
        }
    }
}

fn golden() -> Golden {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join(GOLDEN);
    serde_json::from_slice(&fs::read(&path).unwrap_or_else(|error| panic!("{GOLDEN}: {error}")))
        .unwrap_or_else(|error| panic!("{GOLDEN} is no golden: {error}"))
}

/// `bytes` with every `from` written as `to`.
fn replace_all(bytes: &[u8], from: &[u8], to: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        if !from.is_empty() && bytes[at..].starts_with(from) {
            out.extend_from_slice(to);
            at += from.len();
        } else {
            out.push(bytes[at]);
            at += 1;
        }
    }
    out
}

fn normalised(bytes: &[u8], places: &[(Vec<u8>, &'static str)]) -> Vec<u8> {
    let mut out = bytes.to_vec();
    for (from, to) in places {
        out = replace_all(&out, from, to.as_bytes());
    }
    out
}

/// A row as the golden keeps it, from its two printed runs.
fn row_of(place: &Place, name: &str, args: &[String], [plain, json]: &[Printed; 2]) -> Row {
    let places = place.placeholders();
    let mut stderr = plain.stderr.clone();
    stderr.push(0);
    stderr.extend_from_slice(&json.stderr);
    Row {
        fixture: name.to_owned(),
        args: args.to_vec(),
        code: format!("{} {}", plain.code, json.code),
        text: Stream::of(&normalised(&plain.stdout, &places)),
        json: Stream::of(&normalised(&json.stdout, &places)),
        stderr: Stream::of(&normalised(&stderr, &places)),
    }
}

fn row_difference(want: &Row, got: &Row) -> Option<String> {
    if got == want {
        return None;
    }
    let mut parts = Vec::new();
    if got.code != want.code {
        parts.push(format!(
            "exits {} trusted, {} in the golden",
            got.code, want.code
        ));
    }
    for (what, now, before) in [
        ("the text", &got.text, &want.text),
        ("the JSON", &got.json, &want.json),
        ("stderr", &got.stderr, &want.stderr),
    ] {
        if now != before {
            parts.push(format!(
                "{what}: trusted {}, golden {}",
                now.describe(),
                before.describe()
            ));
        }
    }
    Some(format!(
        "{} {:?}: {}",
        want.fixture,
        want.args,
        parts.join("; ")
    ))
}

/// Up to four distinct words of four or more ASCII letters from the
/// fixture's node titles, in path order, and one word no fixture holds.
fn search_terms(root: &Path) -> Vec<String> {
    let text = fs::read_to_string(root.join("specengine.toml")).expect("specengine.toml");
    let mut terms: Vec<String> = Vec::new();
    if let Ok(config) = ProjectConfig::from_toml(&text) {
        let tree = WorkingTree::new(root, &config.paths).expect("the working tree");
        let input = check_input(&tree, &config.scheme);
        let mut seen: BTreeSet<String> = BTreeSet::new();
        'files: for file in &input.files {
            for node in file.parsed.iter().flat_map(|parsed| &parsed.nodes) {
                for word in node.title.iter().flat_map(|title| title.split_whitespace()) {
                    if word.len() >= 4
                        && word.bytes().all(|byte| byte.is_ascii_alphabetic())
                        && seen.insert(word.to_ascii_lowercase())
                    {
                        terms.push(word.to_owned());
                        if terms.len() == 4 {
                            break 'files;
                        }
                    }
                }
            }
        }
    }
    terms.push("qqzzvvxx".to_owned());
    terms
}

/// One golden row played as the golden was: a fresh copy, `spec index`,
/// a new trust whose first read (this one) walks, then the row inside
/// `trusted`; and the same row by walking reads, for the message.
fn fresh_trusted_row(name: &str, args: &[String]) -> (Row, Row) {
    let place = Place::copy(name);
    let _ = place.index();
    let trust = IndexTrust::new(place.root.clone());
    let _ = trusted(&[&trust], || place.golden_read(args));
    let trusted_row = row_of(
        &place,
        name,
        args,
        &both_trusted(&trust, &|| place.golden_read(args)),
    );
    let walking_row = row_of(&place, name, args, &both(&|| place.golden_read(args)));
    (trusted_row, walking_row)
}

/// A read the whole-read comparison runs twice, walking and trusted.
type BoxedRead<'a> = Box<dyn Fn() -> Result<Outcome, CliError> + 'a>;

/// AC-4's first two bullets for one fixture.
fn trusted_replay(name: &str) {
    let golden = golden();
    assert_eq!(
        golden.rows.len(),
        GOLDEN_ROWS,
        "premise: the golden's rows over the 19 configured fixtures"
    );
    let rows: Vec<&Row> = golden
        .rows
        .iter()
        .filter(|row| row.fixture == name)
        .collect();
    assert!(!rows.is_empty(), "{name}: premise: the golden has its rows");

    let place = Place::copy(name);
    let indexed = place.index();
    assert!(
        indexed.is_ok() || name == NO_INDEX,
        "{name}: spec index: {:?}",
        indexed.err()
    );
    let trust = IndexTrust::new(place.root.clone());
    assert_eq!(trust.walks(), 0, "{name}: a new trust has not walked");
    // The first read in scope walks.
    let first = &rows[0].args;
    let _ = trusted(&[&trust], || place.golden_read(first));
    let walked = trust.walks();
    if name != NO_INDEX {
        assert_eq!(walked, 1, "{name}: the trust's first read walks");
    }

    // One copy serves every row; a row that answers otherwise is played
    // again as the golden was, over a fresh copy, index and trust, and
    // fails only if it still answers otherwise.
    let mut failed: Vec<String> = Vec::new();
    let mut replayed_fresh = 0usize;
    for want in &rows {
        let printed = both_trusted(&trust, &|| place.golden_read(&want.args));
        let got = row_of(&place, name, &want.args, &printed);
        if row_difference(want, &got).is_none() {
            continue;
        }
        replayed_fresh += 1;
        let (fresh, walking) = fresh_trusted_row(name, &want.args);
        if let Some(difference) = row_difference(want, &fresh) {
            failed.push(format!(
                "{difference} (the walking read {} the golden)",
                if walking == **want {
                    "equals"
                } else {
                    "differs from"
                }
            ));
        }
    }
    assert!(
        failed.is_empty(),
        "{} of {} rows answer otherwise inside `trusted`:\n{}",
        failed.len(),
        rows.len(),
        failed.join("\n")
    );
    if replayed_fresh > 0 {
        eprintln!("{name}: {replayed_fresh} rows answered as the golden only over a fresh copy");
    }
    assert_eq!(
        trust.walks(),
        walked,
        "{name}: no read after the first walks at rest"
    );

    // `search`, `tree` and `flow`: the trusted answer is the walking one.
    let at = &place;
    let mut reads: Vec<(String, BoxedRead<'_>)> = Vec::new();
    for term in search_terms(&place.root) {
        let label = format!("search {term}");
        reads.push((label, Box::new(move || at.search(&term))));
    }
    reads.push(("tree".to_owned(), Box::new(move || at.tree(false))));
    reads.push(("tree --archive".to_owned(), Box::new(move || at.tree(true))));
    reads.push(("flow".to_owned(), Box::new(move || at.flow(false))));
    reads.push(("flow --trace".to_owned(), Box::new(move || at.flow(true))));
    let mut differs: Vec<String> = Vec::new();
    for (label, read) in &reads {
        let walking = both(read.as_ref());
        let trusted_answer = both_trusted(&trust, read.as_ref());
        for (mode, walk, trust_answer) in [
            ("plain", &walking[0], &trusted_answer[0]),
            ("--json", &walking[1], &trusted_answer[1]),
        ] {
            if walk != trust_answer {
                differs.push(format!(
                    "{name} {label} {mode}:\nwalking\n{}\ntrusted\n{}",
                    walk.show(),
                    trust_answer.show()
                ));
            }
        }
    }
    assert!(
        differs.is_empty(),
        "{} answers differ inside `trusted`:\n{}",
        differs.len(),
        differs.join("\n")
    );
    assert_eq!(
        trust.walks(),
        walked,
        "{name}: the trusted search, tree and flow did not walk"
    );
}

macro_rules! trusted_replays {
    ($($test:ident => $name:literal),* $(,)?) => {
        $(
            /// AC-4, the golden's rows and the whole reads, for one fixture.
            #[test]
            fn $test() {
                trusted_replay($name);
            }
        )*
    };
}

trusted_replays! {
    ac4_trusted_replays_anchor_states => "anchor-states",
    ac4_trusted_replays_card_press => "card-press",
    ac4_trusted_replays_code_a => "code-a",
    ac4_trusted_replays_code_bevy => "code-bevy",
    ac4_trusted_replays_code_index => "code-index",
    ac4_trusted_replays_corpus_mini => "corpus-mini",
    ac4_trusted_replays_flow_gaps => "flow-gaps",
    ac4_trusted_replays_flow_hire => "flow-hire",
    ac4_trusted_replays_flow_relay => "flow-relay",
    ac4_trusted_replays_impact => "impact",
    ac4_trusted_replays_parent_ref => "parent-ref",
    ac4_trusted_replays_roadmap_a => "roadmap-a",
    ac4_trusted_replays_roadmap_b => "roadmap-b",
    ac4_trusted_replays_spec_a => "spec-a",
    ac4_trusted_replays_spec_b => "spec-b",
    ac4_trusted_replays_spec_c => "spec-c",
    ac4_trusted_replays_trace_almanac => "trace-almanac",
    ac4_trusted_replays_verify_ecs => "verify-ecs",
    ac4_trusted_replays_verify_tests => "verify-tests",
}

/// The golden's fixtures are the macro's: every row is replayed.
#[test]
fn ac4_every_golden_fixture_is_replayed() {
    let source = include_str!("read_trust.rs");
    let fixtures: BTreeSet<String> = golden()
        .rows
        .iter()
        .map(|row| row.fixture.clone())
        .collect();
    assert_eq!(fixtures.len(), 19, "{fixtures:?}");
    for name in &fixtures {
        assert!(
            fixture(name).join("specengine.toml").is_file(),
            "{name}: a configured fixture"
        );
        assert!(
            source.contains(&format!("=> \"{name}\",")),
            "{name} has no trusted replay"
        );
    }
}

// ------------------------------------------------------------------ a written tree

/// The scratch project's config: one prefix and the `[paths]` roots given.
fn notes_config(roots: &str) -> String {
    format!(
        "[project]\nslug = \"trust-notes\"\n\n[ids]\nR = {{ kind = \"requirement\", width = 2 }}\n\n\
         [paths]\nroots = [{roots}]\n"
    )
}

/// Record `number` with one distinctive word.
fn note(number: usize, word: &str) -> String {
    format!(
        "---\nid: R-{number:02}\nkind: requirement\nstatus: draft\n---\n\n\
         # Note {number:02}\n\nThe note holds {word} and nothing else.\n"
    )
}

/// The distinctive words of the three notes.
const WORDS: [&str; 3] = ["javoku", "qezyra", "vixomu"];

/// Three notes under `notes/`, with `roots` as the config's roots.
fn notes(name: &str, roots: &str) -> Place {
    Place::written(name, |root| {
        common::write(root, "specengine.toml", notes_config(roots));
        for (at, word) in WORDS.iter().enumerate() {
            common::write(
                root,
                &format!("notes/r-{:02}.md", at + 1),
                note(at + 1, word),
            );
        }
    })
}

/// The search found the node `id`.
fn finds(printed: &Printed, id: &str) -> bool {
    printed.code == 0 && printed.text().contains(id)
}

// ------------------------------------------------------------------ AC-4, the warnings

#[test]
fn ac4_a_trusted_read_replays_the_walks_two_warnings() {
    let place = notes("warnings", "\"notes\", \"absent\"");
    common::write(&place.root, "notes/locked/inner.md", note(9, "kelomu"));
    let locked = place.root.join("notes/locked");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).expect("mode 000");
    if fs::read_dir(&locked).is_ok() {
        eprintln!("mode-000 directories are listable here (root?): case skipped");
        return;
    }
    place.index().expect("spec index");
    let walking = [
        Printed::of(place.search(WORDS[0]), false),
        Printed::of(place.show("R-01", false, false), false),
    ];
    for answer in &walking {
        let stderr = String::from_utf8_lossy(&answer.stderr).into_owned();
        let lines: Vec<&str> = stderr.lines().collect();
        assert!(
            lines.len() >= 2,
            "premise: the walk warns of the missing root and the unreadable directory\n{}",
            answer.show()
        );
        assert!(
            lines.iter().any(|line| line.contains("absent")),
            "premise: a warning names the missing root\n{}",
            answer.show()
        );
        assert!(
            lines.iter().any(|line| line.contains("notes/locked")),
            "premise: a warning names the unreadable directory\n{}",
            answer.show()
        );
    }

    let trust = IndexTrust::new(place.root.clone());
    let _ = trusted(&[&trust], || place.search(WORDS[1]));
    assert_eq!(trust.walks(), 1, "the trust's first read walks");
    let trusted_answers = [
        trusted(&[&trust], || Printed::of(place.search(WORDS[0]), false)),
        trusted(&[&trust], || {
            Printed::of(place.show("R-01", false, false), false)
        }),
    ];
    assert_eq!(trust.walks(), 1, "the two reads are trusted, not walked");
    for (walk, trust_answer) in walking.iter().zip(&trusted_answers) {
        assert_eq!(
            trust_answer,
            walk,
            "a trusted read prints the walk's warnings in order, and the walk's answer\n\
             walking\n{}\ntrusted\n{}",
            walk.show(),
            trust_answer.show()
        );
    }
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).expect("restore");
}

// ------------------------------------------------------------------ AC-5

#[test]
fn ac5_a_trusted_read_does_not_walk() {
    assert_eq!(RECONCILE_EVERY, Duration::from_secs(300));
    let place = notes("skips", "\"notes\"");
    place.index().expect("spec index");
    let trust = IndexTrust::new(place.root.clone());
    let first = trusted(&[&trust], || Printed::of(place.search(WORDS[0]), false));
    assert!(finds(&first, "R-01"), "{}", first.show());
    assert_eq!(trust.walks(), 1, "the trust's first read walks");

    fs::remove_file(place.root.join("notes/r-01.md")).expect("delete on disk");
    let trusted_answer = trusted(&[&trust], || Printed::of(place.search(WORDS[0]), false));
    assert!(
        finds(&trusted_answer, "R-01"),
        "a trusted search answers from the index, which still holds the deleted file's node\n{}",
        trusted_answer.show()
    );
    assert_eq!(trust.walks(), 1, "the trusted search did not walk");

    // A trust of another root matches no read: the read walks, and that
    // trust keeps no books (R16, R15).
    let elsewhere = place.scratch.dir("elsewhere");
    let other = IndexTrust::new(elsewhere);
    let unmatched = trusted(&[&other], || Printed::of(place.search(WORDS[0]), false));
    assert!(
        !finds(&unmatched, "R-01"),
        "a read whose root no trust matches walks\n{}",
        unmatched.show()
    );
    assert_eq!(other.walks(), 0, "an unmatched trust counts no walk");

    let walking = Printed::of(place.search(WORDS[0]), false);
    assert!(
        !finds(&walking, "R-01"),
        "a read with no scope walks and does not find the deleted node\n{}",
        walking.show()
    );
    assert_eq!(trust.walks(), 1, "a read with no scope keeps no books");
}

// ------------------------------------------------------------------ AC-6 (g)

#[test]
fn ac6g_a_hand_edit_shows_after_the_reconcile_and_at_once_in_a_printed_file() {
    let every = Duration::from_secs(1);
    let place = notes("reconcile", "\"notes\"");
    place.index().expect("spec index");
    let trust = IndexTrust::with_every(place.root.clone(), every);
    let walked_at = Instant::now();
    let first = trusted(&[&trust], || Printed::of(place.search(WORDS[0]), false));
    assert!(finds(&first, "R-01"), "{}", first.show());
    assert_eq!(trust.walks(), 1, "the trust's first read walks");
    assert!(!trust.is_dirty(), "a successful walk clears the mark");

    let edited = "notes/r-02.md";
    common::replace(&place.root, edited, WORDS[1], "lomidrax");
    let before = trusted(&[&trust], || Printed::of(place.search("lomidrax"), false));
    assert!(
        walked_at.elapsed() < every,
        "premise: the trusted search ran within the interval"
    );
    assert!(
        !finds(&before, "R-02"),
        "a hand edit is not seen before the reconcile\n{}",
        before.show()
    );
    assert_eq!(trust.walks(), 1, "no walk within the interval");

    std::thread::sleep(every + Duration::from_millis(100));
    let after = trusted(&[&trust], || Printed::of(place.search("lomidrax"), false));
    assert!(
        finds(&after, "R-02"),
        "the reconcile walks and finds the edit\n{}",
        after.show()
    );
    assert_eq!(trust.walks(), 2, "the reconcile is one walk");

    // A file a read prints is checked as it is now (T5), and the mismatch
    // marks the trust dirty.
    common::replace(&place.root, edited, "lomidrax", "pevantu");
    let shown = trusted(&[&trust], || {
        Printed::of(place.show("R-02", false, false), false)
    });
    assert_eq!(shown.code, 0, "{}", shown.show());
    assert!(
        shown.text().contains("pevantu") && !shown.text().contains("lomidrax"),
        "show prints the file as it is now\n{}",
        shown.show()
    );
    assert!(trust.is_dirty(), "a T5 mismatch marks the trust dirty");
    let walks = trust.walks();
    let next = trusted(&[&trust], || Printed::of(place.search(WORDS[0]), false));
    assert!(finds(&next, "R-01"), "{}", next.show());
    assert_eq!(
        trust.walks(),
        walks + 1,
        "a dirty trust walks on its next read"
    );
    assert!(!trust.is_dirty(), "the walk clears the mark");
}

// ------------------------------------------------------------------ R13, R14

/// R13 (Q7): a trusted whole read with bytes (`tree`) re-parses a file
/// whose bytes no longer match its row, prints it as it is now and marks
/// the trust dirty; R14: `spec index` walks in any scope.
#[test]
fn r13_r14_a_trusted_tree_sees_a_changed_file_and_spec_index_walks() {
    let place = notes("whole", "\"notes\"");
    place.index().expect("spec index");
    let trust = IndexTrust::new(place.root.clone());
    let _ = trusted(&[&trust], || place.search(WORDS[0]));
    assert_eq!(trust.walks(), 1, "the trust's first read walks");

    common::replace(&place.root, "notes/r-02.md", "# Note 02", "# Note two");
    let tree = trusted(&[&trust], || {
        Printed::of(place.tree_from(Some("R-02"), false), false)
    });
    assert_eq!(tree.code, 0, "{}", tree.show());
    assert!(
        tree.text().contains("Note two") && !tree.text().contains("Note 02"),
        "a whole read with bytes prints the file as it is now (Q7)\n{}",
        tree.show()
    );
    assert!(trust.is_dirty(), "the mismatch marks the trust dirty (R13)");

    let _ = trusted(&[&trust], || place.search(WORDS[0]));
    let walks = trust.walks();
    assert!(!trust.is_dirty(), "premise: the dirty read walked");
    common::replace(&place.root, "notes/r-03.md", WORDS[2], "zenthari");
    trusted(&[&trust], || place.index()).expect("spec index in scope");
    assert_eq!(
        trust.walks(),
        walks + 1,
        "`spec index` walks in a scope (R14)"
    );
    let found = trusted(&[&trust], || Printed::of(place.search("zenthari"), false));
    assert!(finds(&found, "R-03"), "{}", found.show());
    assert_eq!(trust.walks(), walks + 1, "the next read trusts");
}

// ------------------------------------------------------------------ AC-7

/// The subset file the races flip: `MEC-SPRINT`'s document in `spec-a`.
const SPRINT: &str = "docs/spec/movement/sprint.md";
/// The race's limits: at most this many runs, within this long.
const RACE_RUNS: usize = 150;
const RACE_TIME: Duration = Duration::from_secs(8);

/// `original` with another `parent:`, another end of one mention and its
/// sections four lines lower at the same byte offsets: the same size
/// (`store_reads.rs`' `sprint_moved`).
fn sprint_moved(original: &str) -> String {
    for from in [
        "parent: DOM-MOVEMENT\n",
        "\n# Sprint\n",
        "bounded by A-101.",
    ] {
        assert_eq!(
            original.matches(from).count(),
            1,
            "premise: {from:?} once in {SPRINT}"
        );
    }
    let moved = original
        .replacen("parent: DOM-MOVEMENT\n", "parent: DOM-GAME\n", 1)
        .replacen("\n# Sprint\n", "\n\n\n\n\n# Sprint\n", 1)
        .replacen("bounded by A-101.", "bounded by A-102.", 1);
    assert_eq!(moved.len(), original.len(), "premise: the same size");
    moved
}

/// A thread swapping `path` between `original` and `moved` (an atomic
/// rename every 200 us), each version's mtime set to `mtime` to the
/// nanosecond before its rename, until `stop`; it returns its flips.
fn flipper(
    path: PathBuf,
    original: String,
    moved: String,
    mtime: std::time::SystemTime,
    stop: Arc<AtomicBool>,
) -> std::thread::JoinHandle<usize> {
    let temp = path.with_extension("swap");
    std::thread::spawn(move || {
        let mut flips = 0usize;
        while !stop.load(Ordering::Relaxed) {
            let text = if flips.is_multiple_of(2) {
                &moved
            } else {
                &original
            };
            fs::write(&temp, text).expect("the next version");
            fs::File::options()
                .write(true)
                .open(&temp)
                .and_then(|file| file.set_times(fs::FileTimes::new().set_modified(mtime)))
                .expect("the mtime restored");
            fs::rename(&temp, &path).expect("the swap");
            flips += 1;
            std::thread::sleep(Duration::from_micros(200));
        }
        flips
    })
}

/// Each read's at-rest answer for the original and the moved version, by
/// walking reads with each version in place.
fn at_rest(
    path: &Path,
    original: &str,
    moved: &str,
    reads: &[&dyn Fn() -> Printed],
) -> Vec<[Printed; 2]> {
    let mut out = Vec::new();
    for read in reads {
        fs::write(path, original).expect("the original");
        let first = read();
        fs::write(path, moved).expect("the moved version");
        let second = read();
        for answer in [&first, &second] {
            assert_eq!(
                answer.code,
                0,
                "premise: answers at rest\n{}",
                answer.show()
            );
        }
        assert_ne!(
            first.stdout, second.stdout,
            "premise: each version answers differently"
        );
        out.push([first, second]);
    }
    fs::write(path, original).expect("the original");
    out
}

/// Runs `reads` in turn inside `trusted(&[trust])` under the flipper; each
/// answer must be one version's at-rest answer, and both must be seen.
fn race(place: &Place, trust: &IndexTrust, reads: &[&dyn Fn() -> Printed]) {
    let path = place.root.join(SPRINT);
    let original = fs::read_to_string(&path).expect("sprint.md");
    let moved = sprint_moved(&original);
    let mtime = fs::metadata(&path)
        .and_then(|meta| meta.modified())
        .expect("the mtime");
    let rest = at_rest(&path, &original, &moved, reads);
    let _ = trusted(&[trust], || reads[0]());
    assert!(trust.walks() >= 1, "the trust's first read walks");

    let stop = Arc::new(AtomicBool::new(false));
    let flipping = flipper(
        path.clone(),
        original.clone(),
        moved,
        mtime,
        Arc::clone(&stop),
    );
    let started = Instant::now();
    let mut seen = [0usize; 2];
    let mut mixed: Vec<String> = Vec::new();
    let mut runs = 0usize;
    while runs < RACE_RUNS && started.elapsed() < RACE_TIME {
        let which = runs % reads.len();
        let got = trusted(&[trust], || reads[which]());
        runs += 1;
        match rest[which].iter().position(|answer| *answer == got) {
            Some(version) => seen[version] += 1,
            None => mixed.push(got.show()),
        }
    }
    stop.store(true, Ordering::Relaxed);
    let flips = flipping.join().expect("the flipper");
    fs::write(&path, &original).expect("the original");
    assert!(
        mixed.is_empty(),
        "{} of {runs} trusted answers are neither version's ({flips} flips); first:\n{}",
        mixed.len(),
        mixed[0]
    );
    assert!(
        seen[0] > 0 && seen[1] > 0,
        "the race was not exercised: {seen:?} (original, moved) of {runs} runs"
    );
    assert!(runs >= 40, "only {runs} runs in {RACE_TIME:?}");
}

/// `tests/freshness.rs`' `a_same_size_edit_racing_the_read_is_never_mixed`
/// inside `trusted`: every `tree` prints exactly one version's tree.
#[test]
fn ac7_a_same_size_edit_racing_a_trusted_tree_is_never_mixed() {
    let place = Place::copy("spec-a");
    place.index().expect("spec index");
    let trust = IndexTrust::new(place.root.clone());
    let tree = || Printed::of(place.tree(false), false);
    race(&place, &trust, &[&tree]);
}

/// `tests/store_reads.rs` AC-7's flipper inside `trusted`, the mtime
/// restored to the nanosecond: `show --links` and `bundle` of the flipped
/// file's document are never mixed. M: bytes printed without their BLAKE3
/// compared.
#[test]
fn ac7_show_links_and_bundle_under_a_flipper_are_never_mixed_when_trusted() {
    let place = Place::copy("spec-a");
    place.index().expect("spec index");
    let trust = IndexTrust::new(place.root.clone());
    let show = || Printed::of(place.show("MEC-SPRINT", true, false), false);
    let bundle = || Printed::of(place.bundle("MEC-SPRINT", None), false);
    race(&place, &trust, &[&show, &bundle]);
}
