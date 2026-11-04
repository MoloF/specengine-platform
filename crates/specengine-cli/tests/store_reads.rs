//! docs/features/store-reads.md (RM-0079), the CLI half through the
//! binary: SR-0's golden.
//!
//! `store_reads_before.json` holds what `spec show`, `spec show --links`,
//! `spec show --links --archive` and `spec bundle` printed with the `spec`
//! binary of this slice's parent commit (`2220d05`), before any `src/`
//! change of the slice (flow-linear's L6). It is recorded once, by
//! [`record_the_answers_before_store_reads`], and never again: a second
//! recording with a binary after the slice would prove nothing, so the
//! recorder refuses to overwrite the file.
//!
//! Each row is one read of one fixture. It is run in one fresh scratch
//! `HOME` over one fresh scratch copy of the fixture, from an empty working
//! directory: first `spec index --root <copy>`, so the read meets an index
//! at rest, then the read as given and the read with `--json`, `--root
//! <copy>` last. A row keeps:
//! - `code`: the two exits, `"<plain> <json>"`;
//! - `text`: the plain run's stdout;
//! - `json`: the `--json` run's stdout;
//! - `stderr`: the plain run's stderr, a NUL, the `--json` run's.
//!
//! A stream of at most [`KEEP_BYTES`] bytes of UTF-8 is kept as its bytes;
//! a longer one as its BLAKE3 (`b3`, as `specengine_store::b3_hash` writes
//! it) and its length. Before either, the scratch's own paths (the copy,
//! the `HOME`, the working directory, the scratch) are written as
//! `<root>`, `<home>`, `<cwd>` and `<scratch>`, so a replay elsewhere
//! compares the same bytes.
//!
//! The rows ("SR-0" "The golden"), for each of the 19 configured fixtures
//! (`fixtures/*/specengine.toml`):
//! - every node ID the parse of the indexed files holds, each once, then
//!   every indexed document path;
//! - five REF errors: an unknown ID (exit 1), a look-alike (exit 2), a
//!   `project:` REF (exit 2), a missing `.md` path (exit 1) and `ID@3`;
//! - for each: `show REF`, `show REF --links`, `show REF --links
//!   --archive` where the fixture has a Tier 3 file, `bundle REF` and
//!   `bundle REF --budget 4294967295`.
//!
//! The replays below run every row the same way with the binary under
//! test, one test per fixture so nextest spreads them.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024);
//! the golden's are JSON `\u` escapes.

#![cfg(unix)]

mod common;

use std::collections::BTreeSet;
use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use common::{SPEC, Scratch, fixture, repository_root};
use serde::{Deserialize, Serialize};
use specengine_core::ProjectConfig;
use specengine_core::check::is_tier3_file;
use specengine_store::{WorkingTree, b3_hash, check_input};

/// The golden's file name, under `tests/`.
const GOLDEN: &str = "store_reads_before.json";

/// What recorded the golden.
const RECORDED_WITH: &str =
    "spec at 2220d05 (store-reads' parent commit, before any src change of the slice)";

/// The budget that leaves nothing out.
const UNBOUNDED: &str = "4294967295";

/// A stream this long or shorter, and UTF-8, is kept as its bytes.
const KEEP_BYTES: usize = 160;

/// One `spec` run longer than this is killed and the test fails.
const RUN_LIMIT: Duration = Duration::from_secs(90);

/// The configured fixtures: every `fixtures/*/specengine.toml`.
const FIXTURES: [&str; 19] = [
    "anchor-states",
    "card-press",
    "code-a",
    "code-bevy",
    "code-index",
    "corpus-mini",
    "flow-gaps",
    "flow-hire",
    "flow-relay",
    "impact",
    "parent-ref",
    "roadmap-a",
    "roadmap-b",
    "spec-a",
    "spec-b",
    "spec-c",
    "trace-almanac",
    "verify-ecs",
    "verify-tests",
];

/// A path no fixture holds: exit 1, `is no indexed document`.
const MISSING_PATH: &str = "docs/store-reads-missing.md";

// ------------------------------------------------------------------ the rows

/// The golden file.
#[derive(Serialize, Deserialize, Debug)]
struct Golden {
    recorded_with: String,
    procedure: String,
    rows: Vec<Row>,
}

/// One read's two runs (see the module documentation).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
struct Row {
    fixture: String,
    args: Vec<String>,
    code: String,
    text: Stream,
    json: Stream,
    stderr: Stream,
}

/// A stream as its bytes, or as its BLAKE3 and length.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
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

/// What a fixture's index holds that the rows name.
struct Corpus {
    /// Every node ID, each once, in path then node order.
    ids: Vec<String>,
    /// Every indexed file, in path order.
    paths: Vec<String>,
    /// Some indexed file is Tier 3.
    tier3: bool,
}

/// The parse of every file the fixture's config walks: what `spec index`
/// stores (store `tests/projection.rs`). The fixture is only read.
fn corpus(name: &str) -> Corpus {
    let root = fixture(name);
    let text = fs::read_to_string(root.join("specengine.toml"))
        .unwrap_or_else(|error| panic!("{name}: specengine.toml: {error}"));
    let config = ProjectConfig::from_toml(&text)
        .unwrap_or_else(|error| panic!("{name}: the config: {error:?}"));
    let tree = WorkingTree::new(&root, &config.paths)
        .unwrap_or_else(|error| panic!("{name}: the working tree: {error:?}"));
    let input = check_input(&tree, &config.scheme);
    let mut ids: Vec<String> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut paths: Vec<String> = Vec::new();
    let mut tier3 = false;
    for file in &input.files {
        paths.push(file.path.clone());
        let Some(parsed) = &file.parsed else {
            continue;
        };
        tier3 |= is_tier3_file(parsed);
        for node in &parsed.nodes {
            if let Some(id) = &node.id
                && seen.insert(id.clone())
            {
                ids.push(id.clone());
            }
        }
    }
    paths.sort();
    assert!(
        !ids.is_empty() || name == NO_INDEX,
        "{name}: premise: the fixture holds an ID"
    );
    Corpus { ids, paths, tier3 }
}

/// The one configured fixture with no `[project] slug` and no walked file
/// (`corpus-mini`, `specengine-eval parse`'s): every read is its config
/// error, so its rows are the five REF errors only, their exits as they
/// come.
const NO_INDEX: &str = "corpus-mini";

/// The ID `NO_INDEX`'s REF errors are built on: its `[ids]` scheme's shape.
const NO_INDEX_ID: &str = "ZR-001";

/// Latin letters and their look-alikes: Cyrillic first, Greek where no
/// Cyrillic one exists.
const LOOK_ALIKES: [(char, char); 27] = [
    ('A', '\u{0410}'),
    ('B', '\u{0412}'),
    ('C', '\u{0421}'),
    ('E', '\u{0415}'),
    ('H', '\u{041d}'),
    ('I', '\u{0406}'),
    ('J', '\u{0408}'),
    ('K', '\u{041a}'),
    ('M', '\u{041c}'),
    ('O', '\u{041e}'),
    ('P', '\u{0420}'),
    ('S', '\u{0405}'),
    ('T', '\u{0422}'),
    ('X', '\u{0425}'),
    ('Y', '\u{04ae}'),
    ('a', '\u{0430}'),
    ('c', '\u{0441}'),
    ('e', '\u{0435}'),
    ('i', '\u{0456}'),
    ('j', '\u{0458}'),
    ('o', '\u{043e}'),
    ('p', '\u{0440}'),
    ('s', '\u{0455}'),
    ('x', '\u{0445}'),
    ('y', '\u{0443}'),
    ('N', '\u{039d}'),
    ('Z', '\u{0396}'),
];

/// The first of `ids` with a letter that has a look-alike, that letter
/// replaced by it: refused with exit 2, naming the Latin fix.
fn look_alike(ids: &[String]) -> String {
    for id in ids {
        if let Some(at) = id
            .char_indices()
            .find(|(_, letter)| LOOK_ALIKES.iter().any(|(latin, _)| latin == letter))
        {
            let (index, letter) = at;
            let (_, other) = LOOK_ALIKES
                .iter()
                .find(|(latin, _)| *latin == letter)
                .expect("a look-alike");
            let mut out = id[..index].to_owned();
            out.push(*other);
            out.push_str(&id[index + letter.len_utf8()..]);
            return out;
        }
    }
    panic!("premise: some ID of {ids:?} has a letter with a look-alike");
}

/// A REF error of the golden: its REF and the plain run's exit, when the
/// spec names one.
struct ErrorRef {
    reference: String,
    exit: Option<i32>,
}

/// The five REF errors of a fixture, built on its first ID (the
/// look-alike on the first with a letter that has one).
fn error_refs(corpus: &Corpus) -> Vec<ErrorRef> {
    let fallback = [NO_INDEX_ID.to_owned()];
    let ids = if corpus.ids.is_empty() {
        &fallback[..]
    } else {
        &corpus.ids[..]
    };
    let first = &ids[0];
    let known = !corpus.ids.is_empty();
    let exit = |code: i32| known.then_some(code);
    vec![
        ErrorRef {
            reference: format!("{first}9"),
            exit: exit(1),
        },
        ErrorRef {
            reference: look_alike(ids),
            exit: exit(2),
        },
        ErrorRef {
            reference: format!("other:{first}"),
            exit: exit(2),
        },
        ErrorRef {
            reference: MISSING_PATH.to_owned(),
            exit: exit(1),
        },
        ErrorRef {
            reference: format!("{first}@3"),
            exit: None,
        },
    ]
}

/// The reads of one REF.
fn reads(reference: &str, tier3: bool) -> Vec<Vec<String>> {
    let mut out: Vec<Vec<&str>> = vec![vec!["show", reference], vec!["show", reference, "--links"]];
    if tier3 {
        out.push(vec!["show", reference, "--links", "--archive"]);
    }
    out.push(vec!["bundle", reference]);
    out.push(vec!["bundle", reference, "--budget", UNBOUNDED]);
    out.into_iter()
        .map(|args| args.into_iter().map(str::to_owned).collect())
        .collect()
}

/// Every row's arguments for `name`, in the golden's order, each with the
/// plain run's exit the spec names.
fn cases(name: &str) -> Vec<(Vec<String>, Option<i32>)> {
    let corpus = corpus(name);
    let mut out = Vec::new();
    for reference in corpus.ids.iter().chain(&corpus.paths) {
        for args in reads(reference, corpus.tier3) {
            out.push((args, None));
        }
    }
    for error in error_refs(&corpus) {
        for args in reads(&error.reference, corpus.tier3) {
            out.push((args, error.exit));
        }
    }
    out
}

// ------------------------------------------------------------------ the runs

/// One finished process, as bytes.
struct Out {
    code: i32,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

/// `program args` in `cwd` with `HOME=home` and nothing else set, killed
/// after [`RUN_LIMIT`].
fn run(program: &Path, home: &Path, cwd: &Path, args: &[&str]) -> Out {
    let mut child = Command::new(program)
        .env_clear()
        .env("HOME", home.as_os_str())
        .current_dir(cwd)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| panic!("spawn {}: {error}", program.display()));
    let mut stdout = child.stdout.take().expect("stdout");
    let mut stderr = child.stderr.take().expect("stderr");
    let out = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stdout.read_to_end(&mut bytes);
        bytes
    });
    let err = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stderr.read_to_end(&mut bytes);
        bytes
    });
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().expect("wait on spec") {
            break status;
        }
        if started.elapsed() > RUN_LIMIT {
            let _ = child.kill();
            let _ = child.wait();
            panic!("spec {args:?} ran longer than {RUN_LIMIT:?}; killed");
        }
        std::thread::sleep(Duration::from_millis(2));
    };
    Out {
        code: status
            .code()
            .unwrap_or_else(|| panic!("spec {args:?} was killed by a signal")),
        stdout: out.join().expect("stdout reader"),
        stderr: err.join().expect("stderr reader"),
    }
}

/// `bytes` with every `from` written as `to`.
fn replace_all(bytes: &[u8], from: &[u8], to: &[u8]) -> Vec<u8> {
    if from.is_empty() {
        return bytes.to_vec();
    }
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at..].starts_with(from) {
            out.extend_from_slice(to);
            at += from.len();
        } else {
            out.push(bytes[at]);
            at += 1;
        }
    }
    out
}

/// The scratch's own paths of one row, longest first, each with what it
/// is written as.
fn placeholders(
    scratch: &Path,
    root: &Path,
    home: &Path,
    cwd: &Path,
) -> Vec<(Vec<u8>, &'static str)> {
    let text = |path: &Path| {
        path.to_str()
            .expect("UTF-8 scratch path")
            .as_bytes()
            .to_vec()
    };
    let mut out = vec![
        (text(root), "<root>"),
        (text(home), "<home>"),
        (text(cwd), "<cwd>"),
        (text(scratch), "<scratch>"),
    ];
    out.sort_by_key(|(from, _)| std::cmp::Reverse(from.len()));
    out
}

/// `bytes` with the scratch's paths written as placeholders.
fn normalised(bytes: &[u8], places: &[(Vec<u8>, &'static str)]) -> Vec<u8> {
    let mut out = bytes.to_vec();
    for (from, to) in places {
        out = replace_all(&out, from, to.as_bytes());
    }
    out
}

/// One row of `name`, played with `program` as the module documentation
/// says; the second value is whether a scratch path was replaced.
fn play(program: &Path, name: &str, args: &[String]) -> (Row, bool) {
    let scratch = Scratch::new("store-reads");
    let root = scratch.copy(name, "copy");
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    warm_up(program, name, &root, &home, &cwd);
    let at = Place {
        scratch: scratch.path(),
        root: &root,
        home: &home,
        cwd: &cwd,
    };
    answer(program, name, &at, args, &mut |_| {})
}

/// `spec index --root <root>`: the index the read meets is at rest.
fn warm_up(program: &Path, name: &str, root: &Path, home: &Path, cwd: &Path) {
    let root_text = root.to_str().expect("UTF-8 root");
    let warm = run(program, home, cwd, &["index", "--root", root_text]);
    assert!(
        warm.code == 0 || name == NO_INDEX,
        "{name}: spec index exits {}: {}",
        warm.code,
        String::from_utf8_lossy(&warm.stderr)
    );
}

/// Where one row's two runs happen.
struct Place<'a> {
    scratch: &'a Path,
    root: &'a Path,
    home: &'a Path,
    cwd: &'a Path,
}

/// Which of a row's two runs is about to start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Turn {
    Plain,
    Json,
}

/// One row's two runs over `at.root`, the plain one then the `--json` one,
/// `before` called just before each; the second value is whether a scratch
/// path was replaced.
fn answer(
    program: &Path,
    name: &str,
    at: &Place<'_>,
    args: &[String],
    before: &mut dyn FnMut(Turn),
) -> (Row, bool) {
    let root_text = at.root.to_str().expect("UTF-8 root").to_owned();
    let mut plain: Vec<&str> = args.iter().map(String::as_str).collect();
    let mut as_json = plain.clone();
    as_json.push("--json");
    plain.extend(["--root", root_text.as_str()]);
    as_json.extend(["--root", root_text.as_str()]);
    before(Turn::Plain);
    let text = run(program, at.home, at.cwd, &plain);
    before(Turn::Json);
    let json = run(program, at.home, at.cwd, &as_json);
    let places = placeholders(at.scratch, at.root, at.home, at.cwd);
    let mut stderr = text.stderr.clone();
    stderr.push(0);
    stderr.extend_from_slice(&json.stderr);
    let streams = [&text.stdout, &json.stdout, &stderr].map(|bytes| normalised(bytes, &places));
    let moved = streams[0] != text.stdout || streams[1] != json.stdout || streams[2] != stderr;
    let row = Row {
        fixture: name.to_owned(),
        args: args.to_vec(),
        code: format!("{} {}", text.code, json.code),
        text: Stream::of(&streams[0]),
        json: Stream::of(&streams[1]),
        stderr: Stream::of(&streams[2]),
    };
    (row, moved)
}

// ------------------------------------------------------------------ the golden

fn golden_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join(GOLDEN)
}

fn golden() -> Golden {
    let path = golden_path();
    serde_json::from_slice(&fs::read(&path).unwrap_or_else(|error| panic!("{GOLDEN}: {error}")))
        .unwrap_or_else(|error| panic!("{GOLDEN} is no golden: {error}"))
}

/// `text` with every non-ASCII character as a JSON `\u` escape (it only
/// occurs inside JSON strings).
fn ascii_json(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for letter in text.chars() {
        if letter.is_ascii() {
            out.push(letter);
        } else {
            let mut units = [0u16; 2];
            for unit in letter.encode_utf16(&mut units) {
                out.push_str(&format!("\\u{unit:04x}"));
            }
        }
    }
    out
}

/// The golden as written: one row per line, ASCII only.
fn golden_text(golden: &Golden) -> String {
    let mut out = String::from("{\n");
    out.push_str(&format!(
        "\"recorded_with\": {},\n",
        serde_json::to_string(&golden.recorded_with).expect("JSON")
    ));
    out.push_str(&format!(
        "\"procedure\": {},\n",
        serde_json::to_string(&golden.procedure).expect("JSON")
    ));
    out.push_str("\"rows\": [\n");
    let rows: Vec<String> = golden
        .rows
        .iter()
        .map(|row| serde_json::to_string(row).expect("JSON"))
        .collect();
    out.push_str(&rows.join(",\n"));
    out.push_str("\n]}\n");
    ascii_json(&out)
}

/// Every fixture's rows played with `program`, `workers` fixtures at a
/// time; the second value counts the rows with a scratch path replaced.
fn play_all(program: &Path, workers: usize) -> (Vec<Row>, usize) {
    let next = AtomicUsize::new(0);
    let done: Mutex<Vec<(usize, Vec<Row>, usize)>> = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                loop {
                    let at = next.fetch_add(1, Ordering::SeqCst);
                    let Some(name) = FIXTURES.get(at) else {
                        break;
                    };
                    let mut rows = Vec::new();
                    let mut moved = 0;
                    for (args, exit) in cases(name) {
                        let (row, replaced) = play(program, name, &args);
                        if let Some(exit) = exit {
                            let plain = row.code.split(' ').next().unwrap_or_default();
                            assert_eq!(
                                plain,
                                exit.to_string(),
                                "{name} {args:?}: premise: the REF error's exit"
                            );
                        }
                        moved += usize::from(replaced);
                        rows.push(row);
                    }
                    done.lock().expect("lock").push((at, rows, moved));
                }
            });
        }
    });
    let mut done = done.into_inner().expect("lock");
    done.sort_by_key(|(at, _, _)| *at);
    let moved = done.iter().map(|(_, _, moved)| moved).sum();
    (
        done.into_iter().flat_map(|(_, rows, _)| rows).collect(),
        moved,
    )
}

/// Writes SR-0's golden from the binary `SPECENGINE_BEFORE_SPEC` names
/// (2220d05's `spec`, built before any `src/` change of store-reads). Run
/// once, by hand: `SPECENGINE_BEFORE_SPEC=<binary> cargo nextest run -p
/// specengine-cli --test store_reads record_the_answers_before
/// --run-ignored ignored-only`. Every row is played twice and must answer
/// the same both times. Never re-recorded from a later binary: it refuses
/// when the golden exists.
#[test]
#[ignore = "manual: records SR-0's golden from 2220d05's binary, once"]
fn record_the_answers_before_store_reads() {
    let program = PathBuf::from(
        std::env::var_os("SPECENGINE_BEFORE_SPEC").expect("SPECENGINE_BEFORE_SPEC: the binary"),
    );
    let path = golden_path();
    assert!(
        !path.exists(),
        "{GOLDEN} exists: it is recorded once, from 2220d05, never again"
    );
    let started = Instant::now();
    let (rows, moved) = play_all(&program, 6);
    let (again, _) = play_all(&program, 6);
    let unstable: Vec<String> = rows
        .iter()
        .zip(&again)
        .filter(|(first, second)| first != second)
        .map(|(first, second)| format!("{first:?}\n  then {second:?}"))
        .collect();
    assert_eq!(rows.len(), again.len());
    assert!(
        unstable.is_empty(),
        "{} rows answer differently twice:\n{}",
        unstable.len(),
        unstable.join("\n")
    );
    let golden = Golden {
        recorded_with: RECORDED_WITH.to_owned(),
        procedure: "per row: a fresh HOME, a fresh copy of the fixture, an empty working \
                    directory; `spec index --root <copy>`, then the read, then the read with \
                    --json, `--root <copy>` last; streams of at most 160 bytes of UTF-8 kept, \
                    longer ones as b3 and len; scratch paths as <root>, <home>, <cwd>, <scratch>"
            .to_owned(),
        rows,
    };
    fs::write(&path, golden_text(&golden)).expect("the golden");
    eprintln!(
        "{GOLDEN}: {} rows, {moved} with a scratch path, in {:?}",
        golden.rows.len(),
        started.elapsed()
    );
}

// ------------------------------------------------------------------ SR-0

/// The fixture list is every configured fixture.
#[test]
fn sr0_the_fixtures_are_every_configured_fixture() {
    let mut found: Vec<String> = fs::read_dir(repository_root().join("fixtures"))
        .expect("fixtures/")
        .filter_map(Result::ok)
        .filter(|entry| entry.path().join("specengine.toml").is_file())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    found.sort();
    assert_eq!(found, FIXTURES.map(str::to_owned).to_vec());
}

/// SR-0 "The golden": it holds exactly the rows of the spec, in order
/// (every ID and path of each fixture, the five REF errors, each read), and
/// each REF error's plain exit is the one the spec names. On the file
/// alone.
#[test]
fn sr0_the_golden_holds_every_case_of_the_spec() {
    let golden = golden();
    assert_eq!(golden.recorded_with, RECORDED_WITH);
    let mut want: Vec<(String, Vec<String>, Option<i32>)> = Vec::new();
    for name in FIXTURES {
        for (args, exit) in cases(name) {
            want.push((name.to_owned(), args, exit));
        }
    }
    let got: Vec<(String, Vec<String>)> = golden
        .rows
        .iter()
        .map(|row| (row.fixture.clone(), row.args.clone()))
        .collect();
    let named: Vec<(String, Vec<String>)> = want
        .iter()
        .map(|(name, args, _)| (name.clone(), args.clone()))
        .collect();
    assert_eq!(got.len(), named.len(), "the golden's row count");
    for (at, (have, need)) in got.iter().zip(&named).enumerate() {
        assert_eq!(have, need, "row {at}");
    }
    for ((name, args, exit), row) in want.iter().zip(&golden.rows) {
        if let Some(exit) = exit {
            assert_eq!(
                row.code,
                format!("{exit} {exit}"),
                "{name} {args:?}: the REF error's exits"
            );
        }
    }
    let bundles = golden
        .rows
        .iter()
        .filter(|row| row.args[0] == "bundle")
        .count();
    let archive = golden
        .rows
        .iter()
        .filter(|row| row.args.iter().any(|arg| arg == "--archive"))
        .count();
    assert!(
        bundles > 0 && archive > 0,
        "premise: bundles and --archive rows"
    );
}

/// What `got` differs in from `want`, naming the row; `None` when equal.
fn row_difference(want: &Row, got: &Row) -> Option<String> {
    if got == want {
        return None;
    }
    let mut parts = Vec::new();
    if got.code != want.code {
        parts.push(format!("exits {} now, {} before", got.code, want.code));
    }
    for (what, now, before) in [
        ("the text", &got.text, &want.text),
        ("the JSON", &got.json, &want.json),
        ("stderr", &got.stderr, &want.stderr),
    ] {
        if now != before {
            parts.push(format!(
                "{what}: now {}, before {}",
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

/// Replays `name`'s rows with the binary under test; fails naming each row
/// whose exits or streams moved.
fn replay_fixture(name: &str) {
    let golden = golden();
    let rows: Vec<&Row> = golden
        .rows
        .iter()
        .filter(|row| row.fixture == name)
        .collect();
    assert!(!rows.is_empty(), "{name}: premise: the golden has its rows");
    let program = Path::new(SPEC);
    let mut failed: Vec<String> = Vec::new();
    for want in &rows {
        let (got, _) = play(program, name, &want.args);
        failed.extend(row_difference(want, &got));
    }
    assert!(
        failed.is_empty(),
        "{} of {} rows differ from 2220d05's answers:\n{}",
        failed.len(),
        rows.len(),
        failed.join("\n")
    );
}

macro_rules! replays {
    ($($test:ident => $name:literal),* $(,)?) => {
        $(
            /// SR-0 "Every row replays equal", for one fixture.
            #[test]
            fn $test() {
                replay_fixture($name);
            }
        )*
    };
}

replays! {
    sr0_replays_anchor_states => "anchor-states",
    sr0_replays_card_press => "card-press",
    sr0_replays_code_a => "code-a",
    sr0_replays_code_bevy => "code-bevy",
    sr0_replays_code_index => "code-index",
    sr0_replays_corpus_mini => "corpus-mini",
    sr0_replays_flow_gaps => "flow-gaps",
    sr0_replays_flow_hire => "flow-hire",
    sr0_replays_flow_relay => "flow-relay",
    sr0_replays_impact => "impact",
    sr0_replays_parent_ref => "parent-ref",
    sr0_replays_roadmap_a => "roadmap-a",
    sr0_replays_roadmap_b => "roadmap-b",
    sr0_replays_spec_a => "spec-a",
    sr0_replays_spec_b => "spec-b",
    sr0_replays_spec_c => "spec-c",
    sr0_replays_trace_almanac => "trace-almanac",
    sr0_replays_verify_ecs => "verify-ecs",
    sr0_replays_verify_tests => "verify-tests",
}

// ------------------------------------------------------------------ helpers of AC-7 and SR-2

/// A plain run's whole answer.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Answer {
    code: i32,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

impl Answer {
    fn show(&self) -> String {
        format!(
            "exit {}\n--- stdout\n{}--- stderr\n{}",
            self.code,
            String::from_utf8_lossy(&self.stdout),
            String::from_utf8_lossy(&self.stderr)
        )
    }
}

/// `spec <args> --root <root>` with the binary under test.
fn read_once(home: &Path, cwd: &Path, root: &Path, args: &[&str]) -> Answer {
    let mut all: Vec<&str> = args.to_vec();
    all.extend(["--root", root.to_str().expect("UTF-8 root")]);
    let out = run(Path::new(SPEC), home, cwd, &all);
    Answer {
        code: out.code,
        stdout: out.stdout,
        stderr: out.stderr,
    }
}

/// The index database of `root`'s project under `home`.
fn database(home: &Path, root: &Path) -> PathBuf {
    let text = fs::read_to_string(root.join("specengine.toml")).expect("specengine.toml");
    let config = ProjectConfig::from_toml(&text).expect("the config");
    let slug = config.slug().expect("a [project] slug");
    common::data_dir(home).join(format!("{slug}.db"))
}

/// `statements` run by the system's `sqlite3` on `db`; its stdout.
fn sql(db: &Path, statements: &str) -> String {
    let output = Command::new(common::proposal::sqlite3())
        .env_clear()
        .args(["-cmd", ".timeout 5000"])
        .arg(db)
        .arg(statements)
        .stdin(Stdio::null())
        .output()
        .expect("sqlite3 runs");
    assert!(
        output.status.success(),
        "sqlite3 {statements}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("UTF-8 sqlite3 output")
}

// ------------------------------------------------------------------ AC-7

/// The subset file AC-7 flips: `MEC-SPRINT`'s document in `spec-a`.
const SPRINT: &str = "docs/spec/movement/sprint.md";

/// The reads AC-7 alternates under the flipper.
const FLIPPED_READS: [&[&str]; 2] = [
    &["show", "MEC-SPRINT", "--links"],
    &["bundle", "MEC-SPRINT"],
];

/// `original` with another `parent:`, another end of one mention and its
/// sections four lines lower at the same byte offsets: the same size
/// (`freshness.rs`'s `sprint_moved`, plus a link).
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

/// AC-7 "One state per read", through the binary. While sprint.md flips
/// (an atomic rename every 200 us) between two same-size versions (another
/// `parent:`, another mention end, its sections four lines lower at the
/// same byte offsets), `show MEC-SPRINT --links` and `bundle MEC-SPRINT`
/// alternate for 150 runs within 40 s. Each answer (exit, stdout, stderr)
/// is exactly one version's at-rest answer, and both versions are seen.
/// The race can only be provoked, not placed, so a mutation's red is a
/// high probability over the runs. M: bytes printed without their BLAKE3
/// compared; the attempt's rows kept with the swapped bytes attached.
#[test]
fn ac7_show_links_and_bundle_under_a_flipper_are_never_mixed() {
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

    let scratch = Scratch::new("store-reads-flip");
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let root = scratch.copy("spec-a", "copy");
    let path = root.join(SPRINT);
    let original = fs::read_to_string(&path).expect("sprint.md");
    let moved = sprint_moved(&original);
    let mut at_rest: Vec<[Answer; 2]> = Vec::new();
    for read in FLIPPED_READS {
        fs::write(&path, &original).expect("the original");
        let first = read_once(&home, &cwd, &root, read);
        fs::write(&path, &moved).expect("the moved version");
        let second = read_once(&home, &cwd, &root, read);
        for answer in [&first, &second] {
            assert!(
                answer.code == 0 && answer.stderr.is_empty(),
                "premise: {read:?} answers at rest\n{}",
                answer.show()
            );
        }
        assert_ne!(
            first.stdout, second.stdout,
            "premise: {read:?} answers each version differently"
        );
        at_rest.push([first, second]);
    }
    fs::write(&path, &original).expect("the original");

    let stop = Arc::new(AtomicBool::new(false));
    let flipper = {
        let stop = Arc::clone(&stop);
        let (path, original, moved) = (path.clone(), original.clone(), moved.clone());
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
                fs::rename(&temp, &path).expect("the swap");
                flips += 1;
                std::thread::sleep(Duration::from_micros(200));
            }
            flips
        })
    };
    let started = Instant::now();
    let mut seen = [[0usize; 2]; 2];
    let mut mixed: Vec<String> = Vec::new();
    let mut runs = 0usize;
    while runs < 150 && started.elapsed() < Duration::from_secs(40) {
        let which = runs % 2;
        let got = read_once(&home, &cwd, &root, FLIPPED_READS[which]);
        runs += 1;
        match at_rest[which].iter().position(|answer| *answer == got) {
            Some(version) => seen[which][version] += 1,
            None => mixed.push(format!("{:?}: {}", FLIPPED_READS[which], got.show())),
        }
    }
    stop.store(true, Ordering::Relaxed);
    let flips = flipper.join().expect("the flipper");
    fs::write(&path, &original).expect("the original");
    assert!(
        mixed.is_empty(),
        "{} of {runs} answers are neither version's ({flips} flips); first:\n{}",
        mixed.len(),
        mixed[0]
    );
    let original_seen = seen[0][0] + seen[1][0];
    let moved_seen = seen[0][1] + seen[1][1];
    assert!(
        original_seen > 0 && moved_seen > 0,
        "the race was not exercised: {seen:?} (read, version) of {runs} runs"
    );
    assert!(runs >= 40, "only {runs} runs in 40 s");
}

// ------------------------------------------------------------------ SR-2

/// The REFs SR-2's binary cases read, over two conventions: documents, a
/// section, a record, a path.
const SR2_REFS: [(&str, &[&str]); 2] = [
    (
        "spec-a",
        &[
            "MEC-SPRINT",
            "MEC-STAMINA",
            "RULE-SPRINT-COST",
            "R-12",
            "docs/spec/movement/sprint.md",
        ],
    ),
    ("spec-b", &["REQ-001", "MOD-CLI", "REQ-002"]),
];

/// The golden's rows of `name` whose REF is one of `refs`: every read of
/// each, four at least.
fn golden_rows_of(golden: &Golden, name: &str, refs: &[&str]) -> Vec<Row> {
    let mut out = Vec::new();
    for reference in refs {
        let rows: Vec<Row> = golden
            .rows
            .iter()
            .filter(|row| row.fixture == name && row.args[1] == *reference)
            .cloned()
            .collect();
        assert!(
            rows.len() >= 4,
            "{name} {reference}: premise: the golden reads it ({} rows)",
            rows.len()
        );
        out.extend(rows);
    }
    out
}

/// Fails naming every row in `failed` among `count`.
fn assert_no_difference(failed: &[String], count: usize, case: &str) {
    assert!(
        failed.is_empty(),
        "{case}: {} of {count} rows differ from 2220d05's answers:\n{}",
        failed.len(),
        failed.join("\n")
    );
}

/// SR-2 "through the binary": an index whose format stamp is 15 and whose
/// edge tables were emptied before each run (after `spec index`) gives the
/// golden's answer, plain and `--json`: the freshness step's full pass
/// repairs both before the snapshot (R7), and the stamp reads 16 after. M:
/// the subset read before the freshness step (with a snapshot that does not
/// check the stamp: the emptied edges printed).
#[test]
fn sr2_a_stale_stamp_over_emptied_edge_tables_answers_the_golden() {
    const CORRUPT: &str = "UPDATE index_meta SET value = '15' WHERE key = 'format'; \
                           DELETE FROM edges; DELETE FROM edge_parents; \
                           DELETE FROM edge_deps; DELETE FROM edge_names;";
    let golden = golden();
    let program = Path::new(SPEC);
    let (mut failed, mut count) = (Vec::new(), 0);
    for (name, refs) in SR2_REFS {
        for want in golden_rows_of(&golden, name, refs) {
            let scratch = Scratch::new("store-reads-stamp");
            let root = scratch.copy(name, "copy");
            let home = scratch.home("h");
            let cwd = scratch.dir("cwd");
            warm_up(program, name, &root, &home, &cwd);
            let db = database(&home, &root);
            let at = Place {
                scratch: scratch.path(),
                root: &root,
                home: &home,
                cwd: &cwd,
            };
            let (got, _) = answer(program, name, &at, &want.args, &mut |_| {
                sql(&db, CORRUPT);
                assert_eq!(
                    sql(
                        &db,
                        "SELECT value FROM index_meta WHERE key = 'format'; \
                         SELECT count(*) FROM edges; SELECT count(*) FROM edge_parents;"
                    ),
                    "15\n0\n0\n",
                    "premise: the stamp is stale and the edge tables empty"
                );
            });
            assert_eq!(
                sql(&db, "SELECT value FROM index_meta WHERE key = 'format';"),
                format!("{}\n", specengine_store::INDEX_FORMAT),
                "{name} {:?}: the freshness step restamped the index",
                want.args
            );
            count += 1;
            failed.extend(row_difference(&want, &got));
        }
    }
    assert_no_difference(&failed, count, "a stale stamp over emptied edge tables");
}

/// SR-2 "through the binary": a worktree never indexed (no database under
/// `HOME` before either run) gives the golden's answer, plain and
/// `--json`.
#[test]
fn sr2_a_worktree_never_indexed_answers_the_golden() {
    let golden = golden();
    let program = Path::new(SPEC);
    let (mut failed, mut count) = (Vec::new(), 0);
    for (name, refs) in SR2_REFS {
        for want in golden_rows_of(&golden, name, refs) {
            let scratch = Scratch::new("store-reads-never");
            let root = scratch.copy(name, "copy");
            let home = scratch.home("h");
            let cwd = scratch.dir("cwd");
            let at = Place {
                scratch: scratch.path(),
                root: &root,
                home: &home,
                cwd: &cwd,
            };
            let data = common::data_dir(&home);
            let (got, _) = answer(program, name, &at, &want.args, &mut |_| {
                if data.exists() {
                    fs::remove_dir_all(&data).expect("the data directory removed");
                }
            });
            count += 1;
            failed.extend(row_difference(&want, &got));
        }
    }
    assert_no_difference(&failed, count, "a worktree never indexed");
}

/// SR-2 "through the binary": a `git worktree add` copy read first, in a
/// `HOME` with no index, gives the golden's answer; then the main worktree,
/// not yet indexed in that database, does too.
#[test]
fn sr2_a_linked_worktree_read_first_answers_the_golden() {
    let golden = golden();
    let program = Path::new(SPEC);
    let (mut failed, mut count) = (Vec::new(), 0);
    for (name, refs) in SR2_REFS {
        for want in golden_rows_of(&golden, name, refs) {
            let scratch = Scratch::new("store-reads-linked");
            let root = scratch.copy(name, "copy");
            let home = scratch.home("h");
            let cwd = scratch.dir("cwd");
            let git = common::git::Sandbox::new(scratch.path());
            git.init(&root);
            git.add_all(&root);
            git.commit(&root, "the fixture");
            let linked = scratch.path().join("linked");
            git.git(
                &root,
                &[
                    "worktree",
                    "add",
                    "-q",
                    "--detach",
                    linked.to_str().expect("UTF-8 path"),
                ],
            );
            let linked = fs::canonicalize(&linked).expect("the linked worktree");
            for worktree in [&linked, &root] {
                let at = Place {
                    scratch: scratch.path(),
                    root: worktree,
                    home: &home,
                    cwd: &cwd,
                };
                let (got, _) = answer(program, name, &at, &want.args, &mut |_| {});
                count += 1;
                failed.extend(row_difference(&want, &got).map(|difference| {
                    let which = if worktree == &linked {
                        "linked"
                    } else {
                        "main"
                    };
                    format!("{which}: {difference}")
                }));
            }
        }
    }
    assert_no_difference(&failed, count, "a linked worktree read first");
}

/// SR-2 M "the subset read run before the freshness step", and "What a
/// subset read cannot see" kept to its window: a file outside
/// `MEC-SPRINT`'s subset that starts citing it after `spec index`, before
/// the read, is listed by the next `show --links` and placed by the next
/// `bundle`, plain and `--json`, as a fresh `HOME` reads the edited tree.
#[test]
fn sr2_a_citation_added_before_the_read_outside_the_subset_is_read() {
    const CITING: &str = "docs/records/Q/Q-032.md";
    let scratch = Scratch::new("store-reads-cite");
    let root = scratch.copy("spec-a", "copy");
    let home = scratch.home("h");
    let fresh = scratch.home("fresh");
    let cwd = scratch.dir("cwd");
    warm_up(Path::new(SPEC), "spec-a", &root, &home, &cwd);
    let before = read_once(&home, &cwd, &root, &["show", "MEC-SPRINT", "--links"]);
    assert!(
        before.code == 0 && !String::from_utf8_lossy(&before.stdout).contains(CITING),
        "premise: {CITING} does not cite MEC-SPRINT\n{}",
        before.show()
    );
    let mut text = fs::read_to_string(root.join(CITING)).expect("the citing file");
    text.push_str("\nSee MEC-SPRINT.\n");
    fs::write(root.join(CITING), text).expect("the edit");
    let reads: [&[&str]; 4] = [
        &["show", "MEC-SPRINT", "--links"],
        &["show", "MEC-SPRINT", "--links", "--json"],
        &["bundle", "MEC-SPRINT"],
        &["bundle", "MEC-SPRINT", "--json"],
    ];
    for read in reads {
        let got = read_once(&home, &cwd, &root, read);
        let want = read_once(&fresh, &cwd, &root, read);
        assert!(
            got == want,
            "{read:?}: the read after the edit is not a fresh HOME's\n--- got\n{}\n--- fresh\n{}",
            got.show(),
            want.show()
        );
        assert!(
            got.code == 0 && String::from_utf8_lossy(&got.stdout).contains("Q-032"),
            "{read:?}: the new citer is read\n{}",
            got.show()
        );
    }
}
