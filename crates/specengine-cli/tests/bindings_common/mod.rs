//! Shared by `bundle_bindings.rs` and `package.rs`
//! (docs/features/bundle-bindings.md): the runs of `spec` over a scratch
//! git repository, the oracle (`spec verify --node`, as `show_bindings.rs`
//! reads it), the bundle's and the brief's parts, the corpus edits, the
//! package's row checks. Each test binary using it declares `common`,
//! `code_index_common`, `lock_common` and `verify_common` at its root.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::Read as _;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::code_index_common;
use crate::common::bundle::split_text;
use crate::common::{self, Run, spec_with};
use crate::lock_common::LockRepo;
use crate::verify_common::{BOUNDS, COUNTER, COUNTER_MD, LIMITS, set_mode};
use serde_json::{Value, json};
use specengine_core::ProjectConfig;

/// The slugs of the fixtures: the index and queue databases' names.
pub const CODE_A: &str = "code-a";
pub const ANCHORS: &str = "anchor-states";
pub const KILN: &str = "kiln-works";
pub const GAME: &str = "verify-ecs";

/// `OUTPUT_CAP_CHARS`: no bundle body passes it.
pub const CAP: usize = 40_000;

/// `--budget u32::MAX`: every candidate and row that the 40 000-character
/// ceiling allows.
pub const ALL: &str = "4294967295";

/// `PACKAGE_BINDINGS_MAX` (docs/features/bundle-bindings.md
/// "Task package").
pub const PACKAGE_BINDINGS_MAX: usize = 64;

/// The look-alike of `GOAL-01` (a Cyrillic O).
pub const LOOK_ALIKE: &str = "G\u{41E}AL-01";

/// The look-alike of `LAW-BOUND` (a Cyrillic A).
pub const LAW_LOOK_ALIKE: &str = "L\u{410}W-BOUND";

/// Every state of `spec-verify.md` "States": the words `sync` takes.
pub const STATES: [&str; 9] = [
    "bad_marker",
    "broken_binding",
    "cannot_verify",
    "code_ahead",
    "conflict",
    "ok",
    "predated",
    "spec_ahead",
    "unlocked",
];

/// Every anchor state (`spec-verify-anchors.md` "Anchors per row").
pub const ANCHOR_STATES: [&str; 4] = ["ambiguous", "exact", "lost", "moved"];

/// The 14 keys of a binding row, in order (docs/canon/code-reads.md
/// "The binding row").
pub const ROW_KEYS: [&str; 14] = [
    "node",
    "relation",
    "file",
    "kind",
    "symbol",
    "item_kind",
    "line",
    "end_line",
    "signature",
    "layer",
    "sync",
    "category",
    "detail",
    "anchor",
];

/// The keys of a tail entry (`spec-cli-bundle.md` "Output").
pub const TAIL_KEYS: [&str; 6] = ["name", "title", "path", "line", "tokens_est", "layer"];

/// The two code layers: JSON key, heading, whether a row of it is a
/// `verifies` row.
pub const CODE_LAYERS: [(&str, &str, bool); 2] =
    [("bindings", "Bindings", false), ("tests", "Tests", true)];

/// The text between two rows of a layer: one empty line, as between items
/// (`spec-cli-bundle.md` "Fitting").
pub const ROW_GAP: &str = "\n";

// ------------------------------------------------------------------- runs

/// One `spec <args>` from one cwd: text, then `--json`.
pub struct Read {
    pub what: String,
    pub text: Run,
    pub json_run: Run,
}

impl Read {
    pub fn code(&self) -> i32 {
        self.text.code
    }

    pub fn json(&self) -> Value {
        common::one_json_document(&self.json_run.stdout)
            .unwrap_or_else(|problem| panic!("{}: {problem}\n{}", self.what, self.show()))
    }

    pub fn show(&self) -> String {
        let cut = |text: &str| -> String { text.chars().take(8_000).collect() };
        format!(
            "{}: exit {}\n--- stdout\n{}\n--- stdout --json\n{}\n--- stderr\n{}",
            self.what,
            self.code(),
            cut(&self.text.stdout),
            cut(&self.json_run.stdout),
            self.text.stderr
        )
    }

    /// Exit 0 with an answer on stdout, text and JSON.
    pub fn answered(&self) -> &Self {
        assert_eq!(self.code(), 0, "{}", self.show());
        assert!(
            !self.text.stdout.is_empty() && !self.json_run.stdout.is_empty(),
            "{}: an answer\n{}",
            self.what,
            self.show()
        );
        self
    }

    /// Every printed byte: the exit, stdout (text, JSON), stderr.
    pub fn bytes(&self) -> (i32, &str, &str, &str) {
        (
            self.code(),
            &self.text.stdout,
            &self.json_run.stdout,
            &self.text.stderr,
        )
    }

    /// The `note: bindings: ` lines of stderr.
    pub fn binding_notes(&self) -> Vec<String> {
        self.text
            .stderr
            .lines()
            .filter(|line| line.starts_with("note: bindings: "))
            .map(str::to_owned)
            .collect()
    }

    /// A bundle's body (the text before its two closing lines).
    pub fn body(&self) -> String {
        split_text(&self.text.stdout).body
    }

    /// A bundle layer's placed entries.
    pub fn layer(&self, key: &str) -> Vec<Value> {
        list(&self.json()["layers"][key])
    }

    /// A bundle's tail entries.
    pub fn tail(&self) -> Vec<Value> {
        list(&self.json()["tail"])
    }
}

/// Every variable a `spec` process of `repo` gets: the sandbox's (its
/// `PATH` with git), `repo`'s `HOME`.
pub fn vars_of(repo: &LockRepo) -> Vec<(OsString, OsString)> {
    let mut vars: Vec<(OsString, OsString)> = repo
        .git
        .vars()
        .into_iter()
        .filter(|(name, _)| name != "HOME")
        .collect();
    vars.push(("HOME".into(), repo.home.clone().into_os_string()));
    vars
}

/// `vars` with `name` set to `value`.
pub fn with_var(
    vars: &[(OsString, OsString)],
    name: &str,
    value: &OsStr,
) -> Vec<(OsString, OsString)> {
    let mut vars: Vec<(OsString, OsString)> = vars
        .iter()
        .filter(|(known, _)| known != name)
        .cloned()
        .collect();
    vars.push((name.into(), value.to_owned()));
    vars
}

/// `program args` in `cwd` with exactly `vars`, stdin null, killed past
/// two minutes (`common::spec_with`'s, any binary).
pub fn run_program(
    program: &Path,
    cwd: &Path,
    args: &[&str],
    vars: &[(OsString, OsString)],
) -> Run {
    if program == Path::new(common::SPEC) {
        let env: Vec<(&str, &OsStr)> = vars
            .iter()
            .map(|(name, value)| (name.to_str().expect("a UTF-8 name"), value.as_os_str()))
            .collect();
        return spec_with(cwd, args, &env);
    }
    let mut command = Command::new(program);
    command
        .env_clear()
        .current_dir(cwd)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (name, value) in vars {
        command.env(name, value);
    }
    let mut child = command.spawn().expect("spawn the binary");
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
        if let Some(status) = child.try_wait().expect("wait") {
            break status;
        }
        if started.elapsed() > common::RUN_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            panic!("{args:?} ran longer than {:?}; killed", common::RUN_TIMEOUT);
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    Run {
        args: args.iter().map(|arg| (*arg).to_owned()).collect(),
        code: status.code().expect("exited, not killed by a signal"),
        stdout: String::from_utf8(out.join().expect("stdout reader")).expect("UTF-8 stdout"),
        stderr: String::from_utf8(err.join().expect("stderr reader")).expect("UTF-8 stderr"),
    }
}

/// `spec args` in `cwd` with exactly `vars`.
pub fn run_with(cwd: &Path, args: &[&str], vars: &[(OsString, OsString)]) -> Run {
    run_program(Path::new(common::SPEC), cwd, args, vars)
}

/// `program args` and `program args --json` from `cwd`: both exit alike and
/// print the same stderr.
pub fn read_program(
    program: &Path,
    cwd: &Path,
    args: &[&str],
    vars: &[(OsString, OsString)],
) -> Read {
    let what = format!("spec {}", args.join(" "));
    let text = run_program(program, cwd, args, vars);
    let mut with_json = args.to_vec();
    with_json.push("--json");
    let json_run = run_program(program, cwd, &with_json, vars);
    assert_eq!(
        text.code,
        json_run.code,
        "{what}: text and JSON exit alike\n{}\n{}",
        text.show(),
        json_run.show()
    );
    assert_eq!(
        text.stderr, json_run.stderr,
        "{what}: text and JSON print the same stderr"
    );
    Read {
        what,
        text,
        json_run,
    }
}

/// `spec args` text and `--json` from `cwd` with `vars`.
pub fn read_with(cwd: &Path, args: &[&str], vars: &[(OsString, OsString)]) -> Read {
    read_program(Path::new(common::SPEC), cwd, args, vars)
}

/// `spec bundle <refs> --budget <budget>` from the root.
pub fn bundle_of(repo: &LockRepo, refs: &[&str], budget: &str) -> Read {
    bundle_in(&repo.root, refs, budget, &vars_of(repo))
}

/// `spec bundle <refs> --budget <budget>` from `cwd` with `vars`.
pub fn bundle_in(cwd: &Path, refs: &[&str], budget: &str, vars: &[(OsString, OsString)]) -> Read {
    let mut args = vec!["bundle"];
    args.extend_from_slice(refs);
    args.extend_from_slice(&["--budget", budget]);
    read_with(cwd, &args, vars)
}

/// `spec task show <id>` (the brief, then the package) from `cwd`.
pub fn task_show_in(cwd: &Path, id: &str, vars: &[(OsString, OsString)]) -> Read {
    read_with(cwd, &["task", "show", id], vars)
}

pub fn text(value: &Value) -> &str {
    value.as_str().unwrap_or_default()
}

pub fn list(value: &Value) -> Vec<Value> {
    value
        .as_array()
        .unwrap_or_else(|| panic!("a list, not {value}"))
        .clone()
}

pub fn owned(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

/// The distinct strings of `field` over `rows` (`null` left out).
pub fn values_of(rows: &[Value], field: impl Fn(&Value) -> &Value) -> BTreeSet<String> {
    rows.iter()
        .map(field)
        .filter(|value| !value.is_null())
        .map(|value| text(value).to_owned())
        .collect()
}

/// The nodes a list of rows names.
pub fn nodes_of(rows: &[Value]) -> BTreeSet<String> {
    rows.iter()
        .map(|row| text(&row["node"]).to_owned())
        .collect()
}

/// A row's six key fields.
pub fn key_of(row: &Value) -> String {
    json!({
        "node": row["node"],
        "relation": row["relation"],
        "file": row["file"],
        "kind": row["kind"],
        "symbol": row["symbol"],
        "item_kind": row["item_kind"],
    })
    .to_string()
}

/// The keys of `rows`, sorted.
pub fn keys_of(rows: &[Value]) -> Vec<String> {
    let mut keys: Vec<String> = rows.iter().map(key_of).collect();
    keys.sort();
    keys
}

/// The keys of the JSON object opening at byte `open` of `raw`, in the
/// order printed (depth 1 of that object only).
pub fn raw_order(raw: &str, open: usize) -> Vec<String> {
    let bytes = raw.as_bytes();
    assert_eq!(bytes[open], b'{', "an object opens at {open}");
    let (mut depth, mut in_string, mut escaped, mut start) = (0usize, false, false, 0);
    let mut keys = Vec::new();
    for (at, &byte) in bytes.iter().enumerate().skip(open) {
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
                if depth == 1 && bytes.get(at + 1) == Some(&b':') {
                    keys.push(raw[start..at].to_owned());
                }
            }
            continue;
        }
        match byte {
            b'"' => {
                in_string = true;
                start = at + 1;
            }
            b'{' | b'[' => depth += 1,
            b'}' | b']' => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            _ => {}
        }
    }
    keys
}

/// The keys of the first object of the list printed as `"<key>":[{` in
/// `raw`, if any.
pub fn first_object_keys(raw: &str, key: &str) -> Option<Vec<String>> {
    let needle = format!("\"{key}\":[{{");
    raw.find(&needle)
        .map(|at| raw_order(raw, at + needle.len() - 1))
}

// ---------------------------------------------------------------- oracle

/// `verify --node`'s answer for one REF: its `--json` document and, per
/// row in the same order, the text's `<binding>` field.
pub struct Judged {
    pub document: Value,
    pub bindings: Vec<String>,
}

/// The `<binding>` field of each row line of a `verify` report (`<class> |
/// <state> | <node> | <relation> | <file> | <binding>`).
pub fn verify_text_bindings(text: &str) -> Vec<String> {
    let classes = [
        "-",
        "fix",
        "check",
        "cannot",
        "pre-existing fix",
        "pre-existing check",
        "pre-existing cannot",
    ];
    text.lines()
        .filter(|line| !line.starts_with("  "))
        .filter_map(|line| {
            let fields: Vec<&str> = line.splitn(6, " | ").collect();
            (fields.len() == 6 && classes.contains(&fields[0])).then(|| fields[5].to_owned())
        })
        .collect()
}

/// `spec verify --node REF` from `cwd` with `vars`, text and `--json`.
pub fn judged_with(cwd: &Path, reference: &str, vars: &[(OsString, OsString)]) -> Judged {
    let text = run_with(cwd, &["verify", "--node", reference], vars);
    let json_run = run_with(cwd, &["verify", "--node", reference, "--json"], vars);
    let document = common::one_json_document(&json_run.stdout).unwrap_or_else(|problem| {
        panic!(
            "spec verify --node {reference}: {problem}\n{}",
            json_run.show()
        )
    });
    let bindings = verify_text_bindings(&text.stdout);
    assert_eq!(
        bindings.len(),
        list(&document["rows"]).len(),
        "premise: verify --node {reference}'s text prints its JSON rows\n{}",
        text.show()
    );
    Judged { document, bindings }
}

/// `spec verify --node REF` from the root, judging (no run cause, uncut).
pub fn judged(repo: &LockRepo, reference: &str) -> Judged {
    let judged = judged_with(&repo.root, reference, &vars_of(repo));
    assert_eq!(
        judged.document["cannot"],
        json!([]),
        "premise: verify --node {reference} judges (no run cause): {}",
        judged.document
    );
    assert_eq!(
        judged.document["omitted"],
        json!({"rows": 0}),
        "premise: verify --node {reference} uncut"
    );
    judged
}

/// `spec verify --node ID --json`'s rows from the root (judging).
pub fn verify_rows(repo: &LockRepo, id: &str) -> Vec<Value> {
    list(&judged(repo, id).document["rows"])
}

/// One verify row with its text: the oracle of one binding row.
#[derive(Debug, Clone)]
pub struct Expected {
    pub row: Value,
    pub binding: String,
}

/// The verify rows of each set node and plain verify's row order, asked
/// once per state.
pub struct Oracle {
    pub by_ref: BTreeMap<String, Vec<Expected>>,
    /// Each row's place in plain `spec verify --json`'s rows, by the whole
    /// row (keyless rows share a key).
    pub order: BTreeMap<String, usize>,
}

impl Oracle {
    pub fn new(repo: &LockRepo) -> Self {
        let run = repo.spec_piped(&repo.root, &["verify", "--json"]);
        let plain = common::one_json_document(&run.stdout)
            .unwrap_or_else(|problem| panic!("spec verify: {problem}\n{}", run.show()));
        let order = list(&plain["rows"])
            .iter()
            .enumerate()
            .map(|(at, row)| (row.to_string(), at))
            .collect();
        Self {
            by_ref: BTreeMap::new(),
            order,
        }
    }

    /// `verify --node REF`'s rows with their texts.
    pub fn rows(&mut self, repo: &LockRepo, reference: &str) -> Vec<Expected> {
        self.by_ref
            .entry(reference.to_owned())
            .or_insert_with(|| {
                let judged = judged(repo, reference);
                list(&judged.document["rows"])
                    .into_iter()
                    .zip(judged.bindings)
                    .map(|(row, binding)| Expected { row, binding })
                    .collect()
            })
            .clone()
    }

    /// The rows of every REF of `set`, each row once, in plain verify's
    /// row order.
    pub fn set_rows(&mut self, repo: &LockRepo, set: &[String]) -> Vec<Expected> {
        let mut by_row: BTreeMap<String, Expected> = BTreeMap::new();
        for reference in set {
            for expected in self.rows(repo, reference) {
                by_row.entry(expected.row.to_string()).or_insert(expected);
            }
        }
        let mut rows: Vec<Expected> = by_row.into_values().collect();
        rows.sort_by_key(|expected| {
            *self
                .order
                .get(&expected.row.to_string())
                .unwrap_or_else(|| panic!("premise: plain verify has {}", expected.row))
        });
        rows
    }
}

/// The REF `verify --node` takes for `id` of a node in `path`: a
/// feature-scoped ID as `<feature>/<ID>`, the feature its document's file
/// stem; any other ID as is.
pub fn reference_of(root: &Path, path: &str, id: &str) -> String {
    let config = ProjectConfig::from_toml(
        &fs::read_to_string(root.join("specengine.toml")).expect("the config"),
    )
    .expect("the config parses");
    let prefix = id.split('-').next().unwrap_or(id);
    let scoped = config
        .scheme
        .prefix(prefix)
        .is_some_and(|spec| spec.scope == specengine_model::IdScope::Feature);
    if scoped && !id.contains('/') {
        let stem = Path::new(path)
            .file_stem()
            .and_then(OsStr::to_str)
            .expect("a feature document's stem");
        format!("{stem}/{id}")
    } else {
        id.to_owned()
    }
}

/// The parser's set of a shown node: the ID nodes of its file whose span
/// starts within the node's lines.
pub fn parsed_set(root: &Path, node: &Value) -> Vec<String> {
    let path = text(&node["path"]);
    let config = ProjectConfig::from_toml(
        &fs::read_to_string(root.join("specengine.toml")).expect("the config"),
    )
    .expect("the config parses");
    let bytes = fs::read(root.join(path)).expect("the holder's file");
    let parsed = specengine_core::parse(path, &bytes, &config.scheme);
    let first = node["line"].as_u64().expect("line");
    let last = node["end_line"].as_u64().expect("end_line");
    let line_of = |offset: usize| -> u64 {
        1 + bytes[..offset]
            .iter()
            .filter(|byte| **byte == b'\n')
            .count() as u64
    };
    parsed
        .nodes
        .iter()
        .filter(|parsed| {
            let start = line_of(parsed.span.range().start);
            parsed.id.is_some() && start >= first && start <= last
        })
        .filter_map(|parsed| parsed.id.clone())
        .collect()
}

/// The set of nodes of `refs` as `spec show REF --json` lists them from
/// `cwd`: each holder's ID, then its `sections` (checked against the
/// parser), each as the REF `verify --node` takes.
pub fn holder_set(cwd: &Path, refs: &[&str], vars: &[(OsString, OsString)]) -> Vec<String> {
    let mut set: Vec<String> = Vec::new();
    for reference in refs {
        let run = run_with(cwd, &["show", reference, "--json"], vars);
        assert_eq!(run.code, 0, "premise: show {reference}\n{}", run.show());
        for node in list(&run.json()["nodes"]) {
            let path = text(&node["path"]).to_owned();
            let mut ids: Vec<String> = node["id"].as_str().map(str::to_owned).into_iter().collect();
            ids.extend(list(&node["sections"]).iter().map(|id| text(id).to_owned()));
            assert_eq!(
                ids,
                parsed_set(cwd, &node),
                "premise: {reference}'s set is its ID and its sections"
            );
            for id in ids {
                let reference = reference_of(cwd, &path, &id);
                if !set.contains(&reference) {
                    set.push(reference);
                }
            }
        }
    }
    set
}

/// A verify row as the binding row it must be: its key, `sync` its
/// `state`, `category`, `detail`, `anchor`.
pub fn judged_view(row: &Value) -> Value {
    json!({
        "node": row["node"],
        "relation": row["relation"],
        "file": row["file"],
        "kind": row["kind"],
        "symbol": row["symbol"],
        "item_kind": row["item_kind"],
        "sync": row["state"],
        "category": row["category"],
        "detail": row["detail"],
        "anchor": row["anchor"],
    })
}

/// A placed row's judged part (what [`judged_view`] makes of verify's).
pub fn shown_view(row: &Value) -> Value {
    json!({
        "node": row["node"],
        "relation": row["relation"],
        "file": row["file"],
        "kind": row["kind"],
        "symbol": row["symbol"],
        "item_kind": row["item_kind"],
        "sync": row["sync"],
        "category": row["category"],
        "detail": row["detail"],
        "anchor": row["anchor"],
    })
}

/// Every stored symbol of `slug`'s code index under `home`, by (path,
/// qpath): `(item_kind, start_line, end_line, signature, layer)`.
pub fn stored_symbols(home: &Path, slug: &str) -> BTreeMap<(String, String), Vec<[Value; 5]>> {
    let db = code_index_common::db(home, slug);
    let mut out: BTreeMap<(String, String), Vec<[Value; 5]>> = BTreeMap::new();
    for row in code_index_common::symbols(&db) {
        out.entry((
            text(&row["path"]).to_owned(),
            text(&row["qpath"]).to_owned(),
        ))
        .or_default()
        .push([
            row["item_kind"].clone(),
            row["start_line"].clone(),
            row["end_line"].clone(),
            row["signature"].clone(),
            row["layer"].clone(),
        ]);
    }
    out
}

/// A row's four located fields as placed.
pub fn shown_located(row: &Value) -> Vec<Value> {
    vec![
        row["line"].clone(),
        row["end_line"].clone(),
        row["signature"].clone(),
        row["layer"].clone(),
    ]
}

/// The four located fields a row must carry: an `item` key's stored
/// symbol's, all `null` with none; a `ron` key its first marker's line
/// (`markers` as verify gives them), the rest `null`; any other key all
/// `null`.
pub fn located_view(
    row: &Value,
    markers: &Value,
    stored: &BTreeMap<(String, String), Vec<[Value; 5]>>,
) -> Vec<Value> {
    match text(&row["kind"]) {
        "item" => {
            let symbols: Vec<&[Value; 5]> = stored
                .get(&(
                    text(&row["file"]).to_owned(),
                    text(&row["symbol"]).to_owned(),
                ))
                .map(|found| {
                    found
                        .iter()
                        .filter(|symbol| symbol[0] == row["item_kind"])
                        .collect()
                })
                .unwrap_or_default();
            match symbols.as_slice() {
                [] => vec![Value::Null; 4],
                [one] => one[1..].to_vec(),
                several => {
                    let mine = shown_located(row);
                    assert!(
                        several.iter().any(|symbol| symbol[1..] == mine[..]),
                        "{row}: one of the stored symbols {several:?}"
                    );
                    mine
                }
            }
        }
        "ron" => vec![
            markers
                .as_array()
                .and_then(|markers| markers.first())
                .map_or(Value::Null, |marker| marker["line"].clone()),
            Value::Null,
            Value::Null,
            Value::Null,
        ],
        _ => vec![Value::Null; 4],
    }
}

/// verify's `<binding>` text of a keyed row: `<kind> <symbol>[ <item_kind>]`.
pub fn binding_of(row: &Value) -> String {
    let mut binding = format!("{} {}", text(&row["kind"]), text(&row["symbol"]));
    if let Some(item_kind) = row["item_kind"].as_str() {
        binding.push(' ');
        binding.push_str(item_kind);
    }
    binding
}

/// show's row line without its indent (docs/canon/code-reads.md
/// "spec show --bindings"): `<node> | <relation> | <place> | <binding> | layer
/// <layer> | sync <sync>[ <category>][ <detail>]`, `-` for `null`.
pub fn row_line(row: &Value, binding: &str) -> String {
    let file = text(&row["file"]);
    let place = match (row["line"].as_u64(), row["end_line"].as_u64()) {
        (Some(line), Some(end)) => format!("{file}:{line}-{end}"),
        (Some(line), None) => format!("{file}:{line}"),
        _ => file.to_owned(),
    };
    let layer = row["layer"].as_str().unwrap_or("-");
    let mut sync = row["sync"].as_str().unwrap_or("-").to_owned();
    for part in ["category", "detail"] {
        if let Some(word) = row[part].as_str() {
            sync.push(' ');
            sync.push_str(word);
        }
    }
    format!(
        "{} | {} | {place} | {binding} | layer {layer} | sync {sync}",
        text(&row["node"]),
        text(&row["relation"])
    )
}

/// A row's form in a bundle (docs/features/bundle-bindings.md
/// "Layers 5 and 9", **Form**): the row line, then `  sig <signature>` when not
/// `null`.
pub fn row_form(row: &Value, binding: &str) -> String {
    let mut form = format!("{}\n", row_line(row, binding));
    if let Some(signature) = row["signature"].as_str() {
        form.push_str(&format!("  sig {signature}\n"));
    }
    form
}

/// The forms `rows` must print as in a code layer's `printed` section,
/// in order: a keyed row's binding [`binding_of`]; a keyless one's as
/// verify words it (the marker's words), read from the row line printed
/// at its place and checked not empty.
pub fn section_forms(printed: &str, rows: &[Value]) -> Vec<String> {
    let groups: Vec<&str> = printed.split("\n\n").collect();
    assert_eq!(
        groups.len(),
        rows.len(),
        "a group of lines per row: {printed:?}"
    );
    rows.iter()
        .zip(groups)
        .map(|(row, group)| {
            if row["kind"].is_string() {
                return row_form(row, &binding_of(row));
            }
            let head = format!(
                "{} | {} | {} | ",
                text(&row["node"]),
                text(&row["relation"]),
                text(&row["file"])
            );
            let line = group.lines().next().unwrap_or_default();
            let binding = line
                .strip_prefix(&head)
                .and_then(|rest| rest.split(" | layer ").next())
                .unwrap_or_default();
            assert!(
                !binding.trim().is_empty(),
                "a keyless row's binding words: {line:?} for {row}"
            );
            row_form(row, binding)
        })
        .collect()
}

pub fn sorted(rows: &[Value]) -> Vec<String> {
    let mut texts: Vec<String> = rows.iter().map(Value::to_string).collect();
    texts.sort();
    texts
}

/// What `got` lacks of `want` and holds beyond it.
pub fn rows_diff(want: &[Value], got: &[Value]) -> String {
    let want = sorted(want);
    let got = sorted(got);
    let missing: Vec<&String> = want.iter().filter(|row| !got.contains(row)).collect();
    let extra: Vec<&String> = got.iter().filter(|row| !want.contains(row)).collect();
    format!("missing: {missing:#?}\nextra: {extra:#?}")
}

// ------------------------------------------------------- the body's parts

/// A body's `## ` sections in order: (heading, the lines under it, the
/// empty line before the next heading left out). The title is not one.
pub fn sections(body: &str) -> Vec<(String, String)> {
    body.split("\n## ")
        .skip(1)
        .map(|part| {
            let (heading, rest) = part.split_once('\n').unwrap_or((part, ""));
            (heading.to_owned(), rest.to_owned())
        })
        .collect()
}

/// The headings of a body, in order.
pub fn headings(body: &str) -> Vec<String> {
    sections(body)
        .into_iter()
        .map(|(heading, _)| heading)
        .collect()
}

/// The text under `heading`, if the body prints it.
pub fn section(body: &str, heading: &str) -> Option<String> {
    sections(body)
        .into_iter()
        .find(|(name, _)| name == heading)
        .map(|(_, text)| text)
}

/// The body with its Bindings and Tests sections taken out: what a bundle
/// of the same state without `[code]` prints, when nothing was left out.
pub fn without_code_layers(body: &str) -> String {
    body.split("\n## ")
        .enumerate()
        .filter(|(at, part)| {
            *at == 0 || !(part.starts_with("Bindings\n") || part.starts_with("Tests\n"))
        })
        .map(|(_, part)| part)
        .collect::<Vec<_>>()
        .join("\n## ")
}

/// A bundle's JSON with the parts the code layers move set aside:
/// `layers.bindings`, `layers.tests`, the totals and hash, the body, the
/// `bindings: ` notes.
pub fn spec_part_json(document: &Value) -> Value {
    let mut document = document.clone();
    for key in ["tokens", "chars", "bytes", "bundle_hash", "body"] {
        document[key] = Value::Null;
    }
    document["layers"]["bindings"] = json!([]);
    document["layers"]["tests"] = json!([]);
    if let Some(notes) = document["notes"].as_array_mut() {
        notes.retain(|note| !text(note).starts_with("bindings: "));
    }
    document
}

// ---------------------------------------------------------- without code

/// `config` without its `[code]` table (from `[code]` to the next table).
pub fn without_code_table(config: &str) -> String {
    let mut out = String::new();
    let mut skipping = false;
    for line in config.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if trimmed.starts_with('[') {
            skipping = trimmed.starts_with("[code]");
        }
        if !skipping {
            out.push_str(line);
        }
    }
    out
}

/// `run` with the root's config without `[code]`, restored after.
pub fn without_code<T>(root: &Path, run: impl FnOnce() -> T) -> T {
    let path = root.join("specengine.toml");
    let config = fs::read_to_string(&path).expect("the config");
    assert!(config.contains("[code]"), "premise: a [code] table");
    fs::write(&path, without_code_table(&config)).expect("the config without [code]");
    let out = run();
    fs::write(&path, config).expect("the config restored");
    out
}

// ----------------------------------------------------------------- edits

/// `show_bindings.rs`' `drift()` over the golden: `CAP` and the RON cap
/// edited, LAW-BOUND's text edited, PIECE-COUNTER's `//!` marker in the
/// test file removed, a non-UTF-8 file, plain `enforce-introduced`.
pub fn drift(repo: &LockRepo) {
    repo.replace(COUNTER, "CAP: u32 = 10;", "CAP: u32 = 12;");
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

/// The two files of a cross-file pair (`symbol-names.md` "Ambiguity").
pub const PAIR_FILES: [&str; 2] = ["crates/tally/src/pair.rs", "crates/tally/src/pair/mod.rs"];

/// `show_bindings.rs`' `halves()`: GOAL-01's marker written as a
/// look-alike (its entry lock-only, `marker_gone`), a look-alike of
/// LAW-BOUND on a new item, LAW-BOUND's Display impl removed (a lock-only
/// `symbol_gone` row), an alias marker, a typo, a cross-file twin.
pub fn halves(repo: &LockRepo) {
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
    repo.write(PAIR_FILES[0], "// @implements LAW-STEP\npub fn twin() {}\n");
    repo.write(PAIR_FILES[1], "pub fn twin() {}\n");
}

/// Markers on PIECE-LIMITS' nested sections and on the document, in a new
/// file, and a comment that only cites LAW-BOUND (a mention).
pub const NESTED: &str = "crates/tally/src/nested.rs";

pub fn nested(repo: &LockRepo) {
    repo.write(
        NESTED,
        "// @implements PIECE-LIMITS\npub fn limits() -> u32 {\n    10\n}\n\n// @implements LAW-SOURCE\npub fn source() -> u32 {\n    10\n}\n\n// @implements LAW-DEFAULT\npub fn fallback() -> u32 {\n    10\n}\n\n// LAW-BOUND is cited here only: a mention, never a row.\npub fn cited() -> u32 {\n    10\n}\n",
    );
}

/// The feature document's criterion text before [`criterion`].
pub const PASS_TEXT: &str = "Step the count eleven times from zero: it reads 10.";

/// `cap-check/PASS-01` linked to GOAL-01 (its text mentions it): a layer-4
/// candidate of GOAL-01's bundle (a feature-scoped section linked in by any
/// type), whose `@verifies` test in `bounds.rs` reaches layer 9. `padding`
/// lines make it large.
pub fn criterion(repo: &LockRepo, padding: usize) {
    let mut text = format!("{PASS_TEXT} It keeps GOAL-01.");
    for at in 0..padding {
        text.push_str(&format!(
            "\nPadding line {at} of the criterion keeps its text long enough to weigh."
        ));
    }
    repo.replace("docs/features/cap-check.md", PASS_TEXT, &text);
}

/// A record whose `links: {verifies: [GOAL-01]}` makes it a layer-4
/// candidate of GOAL-01's bundle, bound by a `@verifies` test of its own.
pub const PREMISE_MD: &str = "docs/records/PREM-002.md";

pub fn verifying_record(repo: &LockRepo) {
    repo.write(
        PREMISE_MD,
        "---\nclass: canon\nid: PREM-002\nowner: owner\nreviewed: 2026-10-10\nlinks:\n  verifies: [GOAL-01]\n---\n\n# The cap is checked by a test\n\nA test steps past the cap.\n",
    );
    repo.append(
        BOUNDS,
        "\n// @verifies PREM-002\n#[test]\nfn premise_holds() {}\n",
    );
}

/// Restores a path's mode when dropped.
pub struct Mode {
    pub path: PathBuf,
    pub mode: u32,
}

impl Mode {
    pub fn set(path: PathBuf, mode: u32) -> Self {
        let old = fs::metadata(&path).expect("the path").permissions().mode() & 0o7777;
        fs::set_permissions(&path, fs::Permissions::from_mode(mode)).expect("chmod");
        Self { path, mode: old }
    }
}

impl Drop for Mode {
    fn drop(&mut self) {
        let _ = fs::set_permissions(&self.path, fs::Permissions::from_mode(self.mode));
    }
}

// ------------------------------------------------------------- the brief

/// The brief's blocks (split at empty lines).
pub fn brief_blocks(brief: &str) -> Vec<Vec<String>> {
    brief
        .split("\n\n")
        .map(|block| block.lines().map(str::to_owned).collect())
        .collect()
}

/// The brief's section `name` (the block whose first line starts with
/// `<name>:`).
pub fn brief_block(brief: &str, name: &str) -> Option<Vec<String>> {
    brief_blocks(brief).into_iter().find(|block| {
        block
            .first()
            .is_some_and(|line| line.starts_with(&format!("{name}:")))
    })
}

/// The names of the brief's sections, in order (the header block aside).
pub fn brief_names(brief: &str) -> Vec<String> {
    brief_blocks(brief)
        .iter()
        .skip(1)
        .filter_map(|block| block.first())
        .filter_map(|line| line.split(':').next().map(str::to_owned))
        .collect()
}

/// A row's binding in the brief (docs/features/bundle-bindings.md
/// "Implementation", divergences: from the row alone): `<kind> <symbol>[
/// <item_kind>]`; a keyless row's `<sync> <category>` when judged,
/// `<sync>` alone without a category, else `-`.
pub fn brief_binding(row: &Value) -> String {
    if row["kind"].is_string() && row["symbol"].is_string() {
        return binding_of(row);
    }
    match (row["sync"].as_str(), row["category"].as_str()) {
        (Some(sync), Some(category)) => format!("{sync} {category}"),
        (Some(sync), None) => sync.to_owned(),
        _ => "-".to_owned(),
    }
}

/// An anchor place's text: `<file> | <kind> <symbol>[ <item_kind>]`.
fn place_text(place: &Value) -> String {
    format!("{} | {}", text(&place["file"]), binding_of(place))
}

/// The brief's Bindings section for `rows`
/// (docs/features/bundle-bindings.md "Task package", **Brief**): `Bindings:
/// <n>`, then each row as `spec show --bindings` prints it: its line two
/// spaces in, then four in its `sig`, `moved from` and `candidate` lines.
pub fn brief_bindings(rows: &[Value]) -> Vec<String> {
    let mut lines = vec![format!("Bindings: {}", rows.len())];
    for row in rows {
        lines.push(format!("  {}", row_line(row, &brief_binding(row))));
        if let Some(signature) = row["signature"].as_str() {
            lines.push(format!("    sig {signature}"));
        }
        if row["anchor"]["from"].is_object() {
            lines.push(format!(
                "    moved from {}",
                place_text(&row["anchor"]["from"])
            ));
        }
        for candidate in row["anchor"]["candidates"].as_array().into_iter().flatten() {
            lines.push(format!("    candidate {}", place_text(candidate)));
        }
    }
    lines
}

/// The brief with its Bindings section taken out and the bundle's hash
/// blanked: what the same state without `[code]` prints.
pub fn brief_spec_part(brief: &str) -> String {
    brief
        .split("\n\n")
        .filter(|block| !block.starts_with("Bindings:"))
        .map(|block| {
            if block.starts_with("Bundle:") {
                block
                    .split(" | ")
                    .map(|field| {
                        if field.starts_with("b3:") {
                            "b3:-"
                        } else {
                            field
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" | ")
            } else {
                block.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// A package with what code moves set aside: `bindings`, the bundle's
/// hash, the `bindings: ` notes.
pub fn package_spec_part(package: &Value) -> Value {
    let mut package = package.clone();
    package["bindings"] = json!([]);
    package["bundle"]["bundle_hash"] = Value::Null;
    if let Some(notes) = package["notes"].as_array_mut() {
        notes.retain(|note| !text(note).starts_with("bindings: "));
    }
    package
}

// ------------------------------------------------------------- the package

/// The 26 keys of a package, in order (`task-package.md` "Package").
pub const PACKAGE_KEY_ORDER: [&str; 26] = [
    "schema_version",
    "id",
    "project",
    "status",
    "title",
    "goal",
    "profile",
    "stale",
    "targets",
    "criteria",
    "affected_nodes",
    "plan",
    "assumptions",
    "open_proposals",
    "owner_notes",
    "bindings",
    "spec_snapshot",
    "snapshot_diff",
    "claim",
    "runs",
    "bundle",
    "roadmap",
    "author",
    "created_at",
    "updated_at",
    "notes",
];

/// The package's rows against the oracle: `want`'s first rows, key for key
/// and in verify's order, located as the code index stores them, each
/// with the 14 keys in order; the brief's Bindings section the rows'
/// lines, after Runs and before Bundle.
pub fn assert_package_rows(repo: &LockRepo, read: &Read, want: &[Expected], context: &str) {
    let package = read.json();
    assert_eq!(package["schema_version"], json!(1), "{context}: version 1");
    assert_eq!(
        raw_order(&read.json_run.stdout, 0),
        PACKAGE_KEY_ORDER,
        "{context}: the 26 keys in order"
    );
    let rows = list(&package["bindings"]);
    let shown: Vec<Value> = rows.iter().map(shown_view).collect();
    let judged: Vec<Value> = want
        .iter()
        .map(|expected| judged_view(&expected.row))
        .collect();
    assert_eq!(
        shown,
        judged,
        "{context}: the package's rows as verify --node judges them, in its order\n{}",
        rows_diff(&judged, &shown)
    );
    let stored = stored_symbols(&repo.home, &slug_of(&repo.root));
    for (row, expected) in rows.iter().zip(want) {
        assert_eq!(
            shown_located(row),
            located_view(row, &expected.row["markers"], &stored),
            "{context}: {row} located"
        );
        let keys: Vec<&str> = row
            .as_object()
            .expect("a row")
            .keys()
            .map(String::as_str)
            .collect();
        let mut sorted_keys = keys.clone();
        sorted_keys.sort_unstable();
        let mut wanted = ROW_KEYS.to_vec();
        wanted.sort_unstable();
        assert_eq!(sorted_keys, wanted, "{context}: a package row's 14 keys");
    }
    if let Some(keys) = first_object_keys(&read.json_run.stdout, "bindings") {
        assert_eq!(keys, ROW_KEYS, "{context}: the row's keys in order");
    }
    let brief = &read.text.stdout;
    let names = brief_names(brief);
    if rows.is_empty() {
        assert!(
            brief_block(brief, "Bindings").is_none(),
            "{context}: no Bindings section for no row\n{}",
            read.show()
        );
    } else {
        // docs/canon/task-package.md "Brief": Bindings 10th of 11, after
        // Runs, before Bundle (last but one).
        let at = names
            .iter()
            .position(|name| name == "Runs")
            .expect("a Runs section");
        assert_eq!(
            names.get(at + 1..at + 3),
            Some(&["Bindings".to_owned(), "Bundle".to_owned()][..]),
            "{context}: Bindings after Runs, before Bundle: {names:?}"
        );
        assert_eq!(
            brief_block(brief, "Bindings"),
            Some(brief_bindings(&rows)),
            "{context}: the brief's Bindings section\n{}",
            read.show()
        );
    }
}

/// `[project] slug` of the config under `root`.
pub fn slug_of(root: &Path) -> String {
    let config = fs::read_to_string(root.join("specengine.toml")).expect("the config");
    config
        .lines()
        .find_map(|line| line.trim().strip_prefix("slug"))
        .and_then(|rest| rest.split('"').nth(1))
        .expect("a slug")
        .to_owned()
}

/// The package's `bindings: ` notes.
pub fn package_binding_notes(package: &Value) -> Vec<String> {
    list(&package["notes"])
        .iter()
        .map(|note| text(note).to_owned())
        .filter(|note| note.starts_with("bindings: "))
        .collect()
}

/// `count` functions bound to GOAL-01 in one new file.
pub fn many_bindings(repo: &LockRepo, count: usize) {
    let mut many = String::new();
    for at in 0..count {
        many.push_str(&format!(
            "// @implements GOAL-01\npub fn many_{at:03}() -> u32 {{\n    {at}\n}}\n\n"
        ));
    }
    repo.write("crates/tally/src/many.rs", many);
}

/// Where two reads first differ: per stream (exit, text, JSON, stderr) the
/// first differing line of each, or the JSON's first differing key.
pub fn first_difference(got: &Read, want: &Read) -> String {
    let mut out = Vec::new();
    if got.code() != want.code() {
        out.push(format!("exit {} against {}", got.code(), want.code()));
    }
    for (name, a, b) in [
        ("stdout", &got.text.stdout, &want.text.stdout),
        ("stdout --json", &got.json_run.stdout, &want.json_run.stdout),
        ("stderr", &got.text.stderr, &want.text.stderr),
    ] {
        if a == b {
            continue;
        }
        let split = |text: &str| -> Vec<String> {
            text.replace("\",\"", "\",\n\"")
                .lines()
                .map(str::to_owned)
                .collect()
        };
        let (a, b) = (split(a), split(b));
        let at = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
        out.push(format!(
            "{name}: line {at}: got {:?}, want {:?}",
            a.get(at)
                .map(|line| line.chars().take(400).collect::<String>()),
            b.get(at)
                .map(|line| line.chars().take(400).collect::<String>())
        ));
    }
    out.join("\n")
}
