//! docs/features/scale-eval.md "Acceptance criteria" SE-1 to SE-9 through
//! the real `specengine-eval scale` binary. Every run writes under its own
//! scratch directory in `std::env::temp_dir()`, removed on drop, with `HOME`
//! and `XDG_DATA_HOME` empty scratch directories of its own and git's trace
//! variables pointed at scratch files no git child may write.
//!
//! - SE-1 `se1_*`: two `--corpus-only --nodes 1000,2000 --seed 7` runs give
//!   byte-identical trees, digests and `size.json`s; `--seed 8` another
//!   digest; the READMEs are byte-equal between `n1000` and `n2000`.
//!   `se1_digest_is_the_documented_hash` recomputes `digest` as "Detail"
//!   defines it; `se1_the_generator_reads_no_clock_pid_or_environment`
//!   reads `src/scale/generate.rs` for S2's sources of variation.
//! - SE-2 `se2_*`: SE-1's `n1000` tree read back against "The generated
//!   tree" (the config, counts, layout, IDs, caps, hubs, rare words).
//! - SE-3 `se3_*`: `specengine_cli::check` on that tree: 0 and 0.
//! - SE-4 `se4_*`: every token in the closed vocabulary; the pilot corpus
//!   names (`anonymity.rs`'s variables), passed through to the child.
//! - SE-5, SE-7, SE-8: one debug run at 1k,
//!   `se5_se7_se8_one_debug_run_at_1k`, each criterion checked apart and
//!   the failures reported together. SE-7's static half,
//!   `se7_the_scale_sources_take_neither_the_process_env_nor_process_git`,
//!   reads `src/scale/`: no git child is spawned on this corpus (it has no
//!   `[code]`), so `process_git` leaves no trace a run can show.
//! - SE-6 `se6_*`: the refusals, the accepted bounds and `--timeout 0`.
//! - SE-9 `scale_baseline`: `#[ignore]`, the release baseline.
//!
//! No test asserts a time (S9). Nothing here names a pilot.

mod import_support;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::process::Output;

use import_support::{Scratch, eval, read_json, repository_root, snapshot, stderr};
use serde_json::Value;
use specengine_cli::{CheckRequest, Env, Globals};

/// `--label`'s default.
const LABEL: &str = "synthetic";

/// `specengine.toml`, byte for byte ("Data", "The generated tree").
const CONFIG: &str = "[project]\n\
slug = \"synthetic\"\n\
\n\
[paths]\n\
roots = [\"README.md\", \"areas\"]\n\
tier0 = \"README.md\"\n\
tier1_name = \"README.md\"\n\
\n\
[ids]\n\
P = { kind = \"node\", width = 6 }\n";

/// Tier 1 areas, hubs, the in-degree cap, the sections one rare word
/// spans, W targets, timed targets and queries ("Layout" to "Samples").
const AREAS: usize = 16;
const HUBS: usize = 4;
const IN_DEGREE_CAP: usize = 16;
const RARE_SECTIONS: usize = 8;
const RARE_SYLLABLES: usize = 5;
const W_TARGETS: usize = 20;
const QUERIES: usize = 3;
/// A group directory's cap.
const GROUP_FILES: usize = 100;
/// The nodes beside the parts: Tier 0 and the 16 Tier 1 READMEs.
const README_NODES: u64 = 17;

/// `--budget`'s default and `--nodes`'s.
const DEFAULT_BUDGET: u64 = 10_000;
const DEFAULT_SIZES: [u64; 3] = [1_000, 10_000, 100_000];

/// The listed words of "The closed vocabulary".
const KEYWORDS: &[&str] = &[
    "project",
    "slug",
    "synthetic",
    "paths",
    "roots",
    "README",
    "md",
    "areas",
    "tier0",
    "tier1_name",
    "ids",
    "P",
    "kind",
    "node",
    "width",
    "class",
    "canon",
    "spec",
    "tier",
    "owner",
    "reviewed",
    "scope",
    "id",
    "status",
    "draft",
    "shipped",
    "links",
    "depends_on",
];

/// The four reads and the steps `other_us` subtracts from each: `get_node`,
/// `get_node_links` and `get_context_bundle` read a subset of the index
/// since store-reads (its "Eval": open and update alone); `search` keeps
/// its full-text query. `update_us` covers `walk_us` and `hash_us`.
const READ_STEPS: [(&str, &[&str]); 4] = [
    ("get_node", &["open_us", "update_us"]),
    ("get_node_links", &["open_us", "update_us"]),
    ("search", &["open_us", "update_us", "fts_us"]),
    ("get_context_bundle", &["open_us", "update_us"]),
];

/// The leaves of `result` beside `sizes` ("Data", "stdout").
const TOP_LEAVES: [&str; 4] = ["budget/source", "budget/tokens", "corpus_only", "seed"];

/// The leaves of one measured size ("Data", "stdout"), leaf by leaf.
const SIZE_LEAVES: &[&str] = &[
    "nodes",
    "corpus/files",
    "corpus/nodes",
    "corpus/bytes",
    "corpus/links",
    "corpus/tier3_files",
    "corpus/degree_max",
    "corpus/hubs",
    "corpus/hub_degree/min",
    "corpus/hub_degree/max",
    "index/files",
    "index/nodes",
    "index/build_us",
    "check/errors",
    "check/warnings",
    "check/us",
    "reads/get_node/median_us",
    "reads/get_node/max_us",
    "reads/get_node_links/median_us",
    "reads/get_node_links/max_us",
    "reads/search/median_us",
    "reads/search/max_us",
    "reads/search/hits_min",
    "reads/search/hits_max",
    "reads/get_context_bundle/median_us",
    "reads/get_context_bundle/max_us",
    "growth/get_node",
    "growth/get_node_links",
    "growth/search",
    "growth/get_context_bundle",
    "steps/open_us",
    "steps/walk_us",
    "steps/hash_us",
    "steps/update_us",
    "steps/update_parsed",
    "steps/decode_us",
    "steps/reparse_us",
    "steps/resolver_us",
    "steps/graph_us",
    "steps/fts_us",
    "other_us/get_node",
    "other_us/get_node_links",
    "other_us/search",
    "other_us/get_context_bundle",
    "w_after/median",
    "w_after/p90",
    "w_after/max",
    "w_after/tier0",
    "w_after/tier1_max",
    "hub/degree",
    "hub/links_shown",
    "hub/get_node_links_us",
    "hub/get_context_bundle_us",
    "hub/w_after",
    "nondeterministic",
];

/// The leaves of a `--corpus-only` size: `nodes` and `corpus` only.
const CORPUS_ONLY_LEAVES: &[&str] = &[
    "nodes",
    "corpus/files",
    "corpus/nodes",
    "corpus/bytes",
    "corpus/links",
    "corpus/tier3_files",
    "corpus/degree_max",
    "corpus/hubs",
    "corpus/hub_degree/min",
    "corpus/hub_degree/max",
];

// ------------------------------------------------------------------ plumbing

/// One `scale` run: its output, its `--out` and the scratch environment the
/// child saw.
struct ScaleRun {
    output: Output,
    out: PathBuf,
    home: PathBuf,
    xdg: PathBuf,
    /// `GIT_TRACE` and `GIT_TRACE2_EVENT`: written only by a git child that
    /// inherits the process's environment.
    traces: [PathBuf; 2],
}

/// `scale --out <scratch>/<name> <args>`.
fn scale(scratch: &Scratch, name: &str, args: &[&str]) -> ScaleRun {
    scale_into(scratch, name, &scratch.join(name), args, &[])
}

/// `scale --out <out> <args>`, `HOME` and `XDG_DATA_HOME` empty scratch
/// directories `<scratch>/<name>.home` and `.xdg`, `vars` added.
fn scale_into(
    scratch: &Scratch,
    name: &str,
    out: &Path,
    args: &[&str],
    vars: &[(&str, &std::ffi::OsStr)],
) -> ScaleRun {
    let home = scratch.join(&format!("{name}.home"));
    let xdg = scratch.join(&format!("{name}.xdg"));
    for dir in [&home, &xdg] {
        fs::create_dir_all(dir).expect("scratch environment directory");
    }
    let traces = [
        scratch.join(&format!("{name}.git-trace")),
        scratch.join(&format!("{name}.git-trace2")),
    ];
    let mut command = eval();
    command
        .env("HOME", &home)
        .env("XDG_DATA_HOME", &xdg)
        .env("GIT_TRACE", &traces[0])
        .env("GIT_TRACE2_EVENT", &traces[1]);
    for (variable, value) in vars {
        command.env(variable, value);
    }
    let output = command
        .arg("scale")
        .arg("--out")
        .arg(out)
        .args(args)
        .output()
        .expect("specengine-eval runs");
    ScaleRun {
        output,
        out: out.to_path_buf(),
        home,
        xdg,
        traces,
    }
}

impl ScaleRun {
    fn code(&self) -> Option<i32> {
        self.output.status.code()
    }

    fn stdout(&self) -> String {
        String::from_utf8_lossy(&self.output.stdout).into_owned()
    }

    fn stderr(&self) -> String {
        stderr(&self.output)
    }

    fn envelope(&self) -> Value {
        import_support::envelope(&self.output)
    }

    /// `<out>/scale/<label>/`.
    fn detail(&self) -> PathBuf {
        self.out.join("scale").join(LABEL)
    }

    fn size_dir(&self, nodes: u64) -> PathBuf {
        self.detail().join(format!("n{nodes}"))
    }

    fn tree(&self, nodes: u64) -> PathBuf {
        self.size_dir(nodes).join("tree")
    }

    fn size_json(&self, nodes: u64) -> Value {
        read_json(&self.size_dir(nodes).join("size.json"))
    }

    fn digest(&self, nodes: u64) -> String {
        let size = self.size_json(nodes);
        let digest = size["digest"]
            .as_str()
            .unwrap_or_else(|| panic!("n{nodes}/size.json has no string `digest`: {size}"))
            .to_owned();
        assert!(
            digest.starts_with("b3:"),
            "n{nodes}: `digest` is `b3:` and hex: {digest}"
        );
        digest
    }
}

/// SE-1's run: `scale --corpus-only --nodes 1000,2000 --seed <seed>`.
fn corpus_run(scratch: &Scratch, name: &str, seed: &str) -> ScaleRun {
    let run = scale(
        scratch,
        name,
        &["--corpus-only", "--nodes", "1000,2000", "--seed", seed],
    );
    assert_eq!(
        run.code(),
        Some(0),
        "`scale --corpus-only --nodes 1000,2000 --seed {seed}` exits 0; stderr:\n{}",
        run.stderr()
    );
    run
}

/// A criterion's name and its checks.
type Criterion<'a> = (&'static str, Box<dyn FnOnce() + 'a>);

/// A criterion of `name` running `check`.
fn criterion<'a>(name: &'static str, check: impl FnOnce() + 'a) -> Criterion<'a> {
    (name, Box::new(check))
}

/// Runs each criterion's checks apart and fails once, naming every
/// criterion that failed: one run feeds several criteria (SE-5, SE-7,
/// SE-8), and the first failure must not hide the others.
fn each_criterion(checks: Vec<Criterion<'_>>) {
    let mut failed = Vec::new();
    for (criterion, check) in checks {
        if let Err(panic) = catch_unwind(AssertUnwindSafe(check)) {
            let message = panic
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| panic.downcast_ref::<&str>().map(|text| (*text).to_owned()))
                .unwrap_or_else(|| "a panic without a message".to_owned());
            failed.push(format!("{criterion}: {message}"));
        }
    }
    assert!(
        failed.is_empty(),
        "{} criteria failed:\n\n{}",
        failed.len(),
        failed.join("\n\n")
    );
}

/// The entries under `dir`, symbolic links not followed: each relative
/// path with its kind, and a link's target.
fn listing(dir: &Path) -> Vec<(String, String)> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, String)>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries {
            let path = entry.expect("entry").path();
            let relative = path.strip_prefix(root).unwrap().display().to_string();
            let meta = fs::symlink_metadata(&path).expect("lstat");
            if meta.file_type().is_symlink() {
                let target = fs::read_link(&path).expect("readlink");
                out.push((relative, format!("link {}", target.display())));
            } else if meta.is_dir() {
                out.push((relative, "dir".to_owned()));
                walk(root, &path, out);
            } else {
                out.push((relative, format!("file {}", meta.len())));
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out.sort();
    out
}

fn is_empty_dir(dir: &Path) -> bool {
    fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("{}: {error}", dir.display()))
        .next()
        .is_none()
}

// ---------------------------------------------------------------- vocabulary

/// "The closed vocabulary": a maximal run of `[A-Za-z0-9_-]`, its edge `-`
/// stripped.
fn tokens(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
        .map(|token| token.trim_matches('-'))
        .filter(|token| !token.is_empty())
}

/// The syllables of a syllable word (case-insensitive
/// `^(?:[jkqvxz][aeiouy])+$`), or `None`.
fn syllables(token: &str) -> Option<usize> {
    let (pairs, rest) = token.as_bytes().as_chunks::<2>();
    if token.is_empty() || !rest.is_empty() {
        return None;
    }
    pairs
        .iter()
        .all(|pair| {
            b"jkqvxz".contains(&pair[0].to_ascii_lowercase())
                && b"aeiouy".contains(&pair[1].to_ascii_lowercase())
        })
        .then_some(token.len() / 2)
}

fn digits(text: &str, width: usize) -> bool {
    text.len() == width && text.bytes().all(|b| b.is_ascii_digit())
}

/// `^P-\d{6}$`.
fn is_id(token: &str) -> bool {
    token.strip_prefix("P-").is_some_and(|rest| digits(rest, 6))
}

fn in_vocabulary(token: &str) -> bool {
    syllables(token).is_some()
        || is_id(token)
        || token.strip_prefix('a').is_some_and(|rest| digits(rest, 2))
        || token.strip_prefix('g').is_some_and(|rest| digits(rest, 4))
        || token == "2026-01-01"
        || token.bytes().all(|b| b.is_ascii_digit())
        || KEYWORDS.contains(&token)
}

/// The numbers of the IDs `text` names.
fn ids_in(text: &str) -> Vec<u32> {
    tokens(text)
        .filter(|token| is_id(token))
        .map(|token| token[2..].parse().expect("six digits"))
        .collect()
}

/// A `P-` and a digit anywhere: an ID on a stream.
fn holds_id(text: &str) -> bool {
    text.as_bytes()
        .windows(3)
        .any(|w| w[0] == b'P' && w[1] == b'-' && w[2].is_ascii_digit())
}

/// The corpus directory names of `SPECENGINE_PILOT_A` and `_B`, as
/// `anonymity.rs` takes them.
fn pilot_names() -> Vec<(&'static str, String)> {
    let mut names = Vec::new();
    for variable in ["SPECENGINE_PILOT_A", "SPECENGINE_PILOT_B"] {
        let Some(value) = std::env::var_os(variable).filter(|v| !v.is_empty()) else {
            continue;
        };
        let path = fs::canonicalize(&value).unwrap_or_else(|_| PathBuf::from(&value));
        if let Some(name) = path.file_name() {
            names.push((variable, name.to_string_lossy().into_owned()));
        }
    }
    names
}

// ------------------------------------------------------------------ the tree

/// One `{#P-...}` section of a part file.
struct Section {
    id: u32,
    heading: String,
    text: String,
}

/// One part file, read back.
struct Part {
    rel: String,
    area: String,
    id: u32,
    class: String,
    status: String,
    shipped: Option<String>,
    scope: String,
    depends_on: Vec<u32>,
    title: String,
    text: String,
    sections: Vec<Section>,
}

impl Part {
    /// Every text that may name an ID: the title, the paragraph, each
    /// section's heading (its anchor cut) and paragraph.
    fn texts(&self) -> impl Iterator<Item = &str> {
        [self.title.as_str(), self.text.as_str()].into_iter().chain(
            self.sections
                .iter()
                .flat_map(|section| [section.heading.as_str(), section.text.as_str()]),
        )
    }

    fn live(&self) -> bool {
        self.status == "draft"
    }
}

/// A generated tree, every file's bytes by its `/`-separated relative path,
/// the part files in ID order.
struct Tree {
    files: BTreeMap<String, Vec<u8>>,
    parts: Vec<Part>,
}

fn id_of(rel: &str, line: usize, text: &str) -> u32 {
    assert!(is_id(text), "{rel}:{line}: `{text}` is not a `P-` ID");
    text[2..].parse().expect("six digits")
}

fn flow_list(rel: &str, line: usize, value: &str) -> Vec<u32> {
    let inner = value
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
        .unwrap_or_else(|| panic!("{rel}:{line}: `depends_on` is not a `[...]` list: {value}"));
    inner
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(|item| id_of(rel, line, item))
        .collect()
}

fn parse_part(rel: &str, area: &str, text: &str) -> Part {
    let mut lines = text.lines().enumerate().map(|(at, line)| (at + 1, line));
    assert_eq!(
        lines.next().map(|(_, line)| line),
        Some("---"),
        "{rel}:1: a part file opens with front matter"
    );
    let mut fields = BTreeMap::new();
    let mut depends_on = Vec::new();
    let mut in_depends = false;
    let mut closed = false;
    for (at, line) in lines.by_ref() {
        if line == "---" {
            closed = true;
            break;
        }
        let trimmed = line.trim_start();
        if in_depends && let Some(item) = trimmed.strip_prefix("- ") {
            depends_on.push(id_of(rel, at, item.trim()));
            continue;
        }
        in_depends = false;
        let (key, value) = trimmed
            .split_once(':')
            .unwrap_or_else(|| panic!("{rel}:{at}: not `key: value`: {line}"));
        let value = value.trim();
        if key == "depends_on" {
            if value.is_empty() {
                in_depends = true;
            } else {
                depends_on.extend(flow_list(rel, at, value));
            }
        } else {
            assert!(
                !line.starts_with(' '),
                "{rel}:{at}: a nested key other than `depends_on`: {line}"
            );
            fields.insert(key.to_owned(), value.to_owned());
        }
    }
    assert!(closed, "{rel}: the front matter is not closed");
    let mut title = None;
    let mut paragraph = String::new();
    let mut sections: Vec<Section> = Vec::new();
    for (at, line) in lines {
        if let Some(heading) = line.strip_prefix("## ") {
            let (words, anchor) = heading.rsplit_once(" {#").unwrap_or_else(|| {
                panic!("{rel}:{at}: a section heading without `{{#ID}}`: {line}")
            });
            let anchor = anchor
                .strip_suffix('}')
                .unwrap_or_else(|| panic!("{rel}:{at}: an unclosed anchor: {line}"));
            sections.push(Section {
                id: id_of(rel, at, anchor),
                heading: words.to_owned(),
                text: String::new(),
            });
        } else if let Some(heading) = line.strip_prefix("# ") {
            assert!(
                title.is_none() && sections.is_empty(),
                "{rel}:{at}: one H1, before every section"
            );
            title = Some(heading.to_owned());
        } else if !line.trim().is_empty() {
            let into = match sections.last_mut() {
                Some(section) => &mut section.text,
                None => &mut paragraph,
            };
            into.push_str(line);
            into.push('\n');
        }
    }
    let field = |key: &str| {
        fields
            .get(key)
            .cloned()
            .unwrap_or_else(|| panic!("{rel}: no `{key}` in the front matter"))
    };
    Part {
        rel: rel.to_owned(),
        area: area.to_owned(),
        id: id_of(rel, 1, &field("id")),
        class: field("class"),
        status: field("status"),
        shipped: fields.get("shipped").cloned(),
        scope: field("scope"),
        depends_on,
        title: title.unwrap_or_else(|| panic!("{rel}: no H1 title")),
        text: paragraph,
        sections,
    }
}

impl Tree {
    fn read(root: &Path) -> Self {
        assert!(root.is_dir(), "{} is not a directory", root.display());
        let files: BTreeMap<String, Vec<u8>> = snapshot(root)
            .into_iter()
            .map(|(path, bytes)| (path.to_string_lossy().into_owned(), bytes))
            .collect();
        let mut parts = Vec::new();
        for (rel, bytes) in &files {
            let segments: Vec<&str> = rel.split('/').collect();
            if let ["areas", area, _group, _name] = segments.as_slice() {
                let text =
                    std::str::from_utf8(bytes).unwrap_or_else(|_| panic!("{rel}: not UTF-8"));
                parts.push(parse_part(rel, area, text));
            }
        }
        parts.sort_by_key(|part| part.id);
        Self { files, parts }
    }

    fn text(&self, rel: &str) -> &str {
        let bytes = self
            .files
            .get(rel)
            .unwrap_or_else(|| panic!("the tree has no {rel}"));
        std::str::from_utf8(bytes).unwrap_or_else(|_| panic!("{rel}: not UTF-8"))
    }

    fn bytes(&self, rel: &str) -> u64 {
        self.files
            .get(rel)
            .unwrap_or_else(|| panic!("the tree has no {rel}"))
            .len() as u64
    }

    fn area_readme(&self, area: &str) -> u64 {
        self.bytes(&format!("areas/{area}/README.md"))
    }

    /// The documents of the first four part files.
    fn hubs(&self) -> Vec<u32> {
        self.parts.iter().take(HUBS).map(|part| part.id).collect()
    }

    /// The part file holding node `id`, a document or a section.
    fn holder(&self, id: u32) -> Option<&Part> {
        self.parts
            .iter()
            .find(|part| part.id == id || part.sections.iter().any(|section| section.id == id))
    }

    /// Incoming links per named ID: `depends_on` items and inline
    /// mentions, each occurrence once.
    fn incoming(&self) -> BTreeMap<u32, usize> {
        let mut counts = BTreeMap::new();
        for part in &self.parts {
            for &target in &part.depends_on {
                *counts.entry(target).or_default() += 1;
            }
            for text in part.texts() {
                for target in ids_in(text) {
                    *counts.entry(target).or_default() += 1;
                }
            }
        }
        counts
    }

    /// The rare words (exactly five syllables) of section `section`,
    /// lower-cased.
    fn rare_words(section: &Section) -> BTreeSet<String> {
        tokens(&section.heading)
            .chain(tokens(&section.text))
            .filter(|token| syllables(token) == Some(RARE_SYLLABLES))
            .map(str::to_ascii_lowercase)
            .collect()
    }
}

fn id_name(id: u32) -> String {
    format!("P-{id:06}")
}

/// IDs from a `size.json` list: strings, or objects with an `id` or
/// `target`.
fn id_strings(value: &Value, what: &str) -> Vec<String> {
    value
        .as_array()
        .unwrap_or_else(|| panic!("size.json `{what}` is not a list: {value}"))
        .iter()
        .map(|item| {
            item.as_str()
                .or_else(|| item.get("id").and_then(Value::as_str))
                .or_else(|| item.get("target").and_then(Value::as_str))
                .unwrap_or_else(|| panic!("size.json `{what}` holds no ID: {item}"))
                .to_owned()
        })
        .collect()
}

fn id_number(text: &str) -> u32 {
    assert!(is_id(text), "`{text}` is not a `P-` ID");
    text[2..].parse().expect("six digits")
}

/// `w.rs`'s nearest rank: the 0-based index of the `p`-th percentile of
/// `n` values.
fn nearest_rank(n: usize, p: usize) -> usize {
    (p * n).div_ceil(100).max(1) - 1
}

// ------------------------------------------------------------- shape (SE-2)

/// "The generated tree" of `nodes` part nodes, read back.
fn assert_shape(tree: &Tree, nodes: usize) {
    assert_eq!(
        tree.text("specengine.toml"),
        CONFIG,
        "specengine.toml is \"Data\"'s, byte for byte"
    );

    // Exactly the four kinds of file.
    let readmes: BTreeSet<String> = std::iter::once("README.md".to_owned())
        .chain((0..AREAS).map(|area| format!("areas/a{area:02}/README.md")))
        .collect();
    let parts: BTreeSet<&str> = tree.parts.iter().map(|part| part.rel.as_str()).collect();
    for rel in tree.files.keys() {
        assert!(
            rel == "specengine.toml" || readmes.contains(rel) || parts.contains(rel.as_str()),
            "{rel}: not one of the generated tree's files"
        );
    }
    for readme in &readmes {
        assert!(tree.files.contains_key(readme), "{readme} is missing");
    }

    // Counts: exactly N part nodes, N + 17 in all.
    let part_nodes: usize = tree.parts.iter().map(|part| 1 + part.sections.len()).sum();
    assert_eq!(
        part_nodes, nodes,
        "part nodes (documents and `{{#P-...}}` sections)"
    );
    let documents = tree.files.keys().filter(|rel| rel.ends_with(".md")).count();
    let anchors: usize = tree
        .files
        .values()
        .map(|bytes| bytes.windows(4).filter(|w| w.starts_with(b"{#P-")).count())
        .sum();
    assert_eq!(
        (documents + anchors) as u64,
        nodes as u64 + README_NODES,
        "nodes in all: every document and every `{{#P-...}}` section"
    );

    // Layout and IDs in file order.
    let mut next = 1;
    let last = tree.parts.len() - 1;
    for (k, part) in tree.parts.iter().enumerate() {
        let expected = format!(
            "areas/a{:02}/g{:04}/{}.md",
            k % AREAS,
            (k / AREAS) / GROUP_FILES,
            id_name(part.id)
        );
        assert_eq!(
            part.rel, expected,
            "part file {k}: area k mod 16, group floor(k / 16) / 100, named by its document"
        );
        assert_eq!(
            part.id, next,
            "{}: IDs run from P-000001 in file order",
            part.rel
        );
        next += 1;
        for section in &part.sections {
            assert_eq!(
                section.id, next,
                "{}: a section's ID follows its document's and the section before",
                part.rel
            );
            next += 1;
        }
        assert_eq!(part.class, "spec", "{}: `class: spec`", part.rel);
        assert_eq!(
            part.scope,
            format!("[{}]", part.area),
            "{}: `scope` is its area",
            part.rel
        );
        let sections = part.sections.len();
        if k < last {
            assert!(
                (2..=6).contains(&sections),
                "{}: {sections} sections, not 2..=6",
                part.rel
            );
        } else {
            assert!(
                sections <= 6,
                "{}: the last file, {sections} sections",
                part.rel
            );
        }
    }
    let mean = nodes as f64 / tree.parts.len() as f64;
    assert!(
        (4.5..=5.5).contains(&mean),
        "{mean:.2} nodes per part file, not 4.5 to 5.5"
    );

    // No group directory above 100 files.
    let mut groups: BTreeMap<&str, usize> = BTreeMap::new();
    for part in &tree.parts {
        let (group, _) = part.rel.rsplit_once('/').expect("a group directory");
        *groups.entry(group).or_default() += 1;
    }
    for (group, files) in &groups {
        assert!(*files <= GROUP_FILES, "{group} holds {files} files");
    }

    // Status: hubs draft; shipped with its date; about one in five.
    let hubs = tree.hubs();
    assert_eq!(hubs.len(), HUBS, "four hubs");
    for part in &tree.parts {
        match part.status.as_str() {
            "draft" => assert!(
                part.shipped.is_none(),
                "{}: a draft with `shipped`",
                part.rel
            ),
            "shipped" => {
                assert_eq!(
                    part.shipped.as_deref(),
                    Some("2026-01-01"),
                    "{}: `shipped: 2026-01-01`",
                    part.rel
                );
                assert!(
                    !hubs.contains(&part.id),
                    "{}: a hub is never shipped",
                    part.rel
                );
            }
            other => panic!("{}: status `{other}`", part.rel),
        }
    }
    let shipped = tree.parts.iter().filter(|part| !part.live()).count();
    let share = shipped as f64 / tree.parts.len() as f64;
    assert!(
        (0.1..=0.3).contains(&share),
        "{shipped} of {} part documents shipped: not about one in five",
        tree.parts.len()
    );

    // Links: `depends_on` back to earlier non-hub documents; section
    // mentions of non-hub nodes other than themselves; the caps.
    let document_ids: BTreeSet<u32> = tree.parts.iter().map(|part| part.id).collect();
    for part in &tree.parts {
        assert!(
            part.depends_on.len() <= 2,
            "{}: {} `depends_on` items",
            part.rel,
            part.depends_on.len()
        );
        for &target in &part.depends_on {
            assert!(
                target < part.id,
                "{}: `depends_on` names {}, not a smaller ID",
                part.rel,
                id_name(target)
            );
            assert!(
                document_ids.contains(&target) && !hubs.contains(&target),
                "{}: `depends_on` names {}, not a non-hub part document",
                part.rel,
                id_name(target)
            );
        }
        for section in &part.sections {
            let named = ids_in(&section.heading)
                .into_iter()
                .chain(ids_in(&section.text))
                .collect::<Vec<_>>();
            assert!(
                named.len() <= 2,
                "{}: {} holds {} mentions",
                part.rel,
                id_name(section.id),
                named.len()
            );
            for target in named {
                assert!(
                    target != section.id && !hubs.contains(&target),
                    "{}: {} mentions {}: itself or a hub",
                    part.rel,
                    id_name(section.id),
                    id_name(target)
                );
            }
        }
    }
    let incoming = tree.incoming();
    for (&target, &count) in &incoming {
        assert!(
            (1..=nodes as u32).contains(&target),
            "{} is named but never issued",
            id_name(target)
        );
        if !hubs.contains(&target) {
            assert!(
                count <= IN_DEGREE_CAP,
                "{} has {count} incoming links, above 16",
                id_name(target)
            );
        }
    }
    for hub in &hubs {
        assert!(
            incoming.get(hub).copied().unwrap_or(0) >= 1,
            "hub {} has no incoming link",
            id_name(*hub)
        );
    }

    // Words: titles 2 to 4 words, the first capitalised; common words 1
    // to 3 syllables; a rare word only in sections, one per section, the
    // same for each run of 8 sections in ID order.
    for part in &tree.parts {
        let headings = std::iter::once(part.title.as_str())
            .chain(part.sections.iter().map(|s| s.heading.as_str()));
        for heading in headings {
            let words: Vec<&str> = tokens(heading).collect();
            assert!(
                (2..=4).contains(&words.len()) && words.iter().all(|w| syllables(w).is_some()),
                "{}: title `{heading}` is not 2 to 4 syllable words",
                part.rel
            );
            assert!(
                words[0].starts_with(|c: char| c.is_ascii_uppercase()),
                "{}: title `{heading}`: the first word is not capitalised",
                part.rel
            );
        }
        for text in part.texts() {
            for token in tokens(text) {
                if let Some(count) = syllables(token) {
                    assert!(
                        count <= 3 || count == RARE_SYLLABLES,
                        "{}: `{token}` has {count} syllables",
                        part.rel
                    );
                }
            }
        }
        for token in tokens(&part.title).chain(tokens(&part.text)) {
            assert!(
                syllables(token) != Some(RARE_SYLLABLES),
                "{}: the document holds the rare word `{token}`",
                part.rel
            );
        }
    }
    let mut held: BTreeMap<String, usize> = BTreeMap::new();
    let mut blocks: BTreeMap<usize, String> = BTreeMap::new();
    let all_sections = tree.parts.iter().flat_map(|part| {
        part.sections
            .iter()
            .map(move |section| (part.rel.as_str(), section))
    });
    for (j, (rel, section)) in all_sections.enumerate() {
        let rare = Tree::rare_words(section);
        assert_eq!(
            rare.len(),
            1,
            "{rel}: {} holds {} rare words, not one: {rare:?}",
            id_name(section.id),
            rare.len()
        );
        let word = rare.into_iter().next().unwrap();
        let block = blocks
            .entry(j / RARE_SECTIONS)
            .or_insert_with(|| word.clone());
        assert_eq!(
            *block,
            word,
            "{rel}: section {j} ({}) holds `{word}`, its block of 8 `{block}`",
            id_name(section.id)
        );
        *held.entry(word).or_default() += 1;
    }
    for (word, sections) in &held {
        assert!(
            *sections <= RARE_SECTIONS,
            "rare word `{word}` is in {sections} sections"
        );
    }
    assert_eq!(
        held.len(),
        blocks.len(),
        "each block of 8 sections has its own rare word"
    );

    // READMEs: their front matter, Tier 0's 16 items, no ID.
    let tier0 = tree.text("README.md");
    for line in [
        "class: canon",
        "tier: 0",
        "owner: owner",
        "reviewed: 2026-01-01",
    ] {
        assert!(tier0.lines().any(|l| l == line), "README.md lacks `{line}`");
    }
    let items: Vec<&str> = tier0.lines().filter(|l| l.starts_with("- [")).collect();
    assert_eq!(items.len(), AREAS, "README.md holds 16 items");
    for (area, item) in items.iter().enumerate() {
        assert!(
            item.ends_with(&format!("](areas/a{area:02}/README.md)")),
            "README.md item {area}: `{item}`"
        );
    }
    for area in 0..AREAS {
        let rel = format!("areas/a{area:02}/README.md");
        let text = tree.text(&rel);
        for line in [
            "class: canon".to_owned(),
            "tier: 1".to_owned(),
            "owner: owner".to_owned(),
            "reviewed: 2026-01-01".to_owned(),
            format!("scope: [a{area:02}]"),
        ] {
            assert!(text.lines().any(|l| l == line), "{rel} lacks `{line}`");
        }
    }
    for rel in &readmes {
        assert!(ids_in(tree.text(rel)).is_empty(), "{rel} holds an ID");
    }
}

// --------------------------------------------------- a measured run (SE-5)

/// The leaves of `value`, `/`-joined; a scalar, a `null`, an array and an
/// empty object are leaves.
fn leaves(value: &Value, prefix: &str, out: &mut Vec<(String, Value)>) {
    match value {
        Value::Object(map) if !map.is_empty() => {
            for (key, inner) in map {
                let path = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}/{key}")
                };
                leaves(inner, &path, out);
            }
        }
        other => out.push((prefix.to_owned(), other.clone())),
    }
}

fn leaf_names(value: &Value) -> BTreeSet<String> {
    let mut found = Vec::new();
    leaves(value, "", &mut found);
    found.into_iter().map(|(name, _)| name).collect()
}

/// A whole number at `path` (`/`-joined) of `entry`.
fn whole(entry: &Value, path: &str) -> u64 {
    entry
        .pointer(&format!("/{path}"))
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("`{path}` is not a whole number: {entry}"))
}

/// SE-5's checks over a measured run of `sizes` at the default seed and
/// budget, and over every size's `size.json` and tree.
fn assert_measured(run: &ScaleRun, sizes: &[u64]) {
    let envelope = run.envelope();
    let keys: BTreeSet<&str> = envelope
        .as_object()
        .expect("the envelope is an object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        BTreeSet::from(["label", "measurement", "result", "versions", "wall_ms"]),
        "the envelope's keys (main.rs's)"
    );
    assert_eq!(envelope["measurement"], "scale");
    assert_eq!(envelope["label"], LABEL);
    let result = &envelope["result"];
    assert!(result.is_object(), "`result` is a measurement: {result}");

    // Every key of "Data"; numbers and `null` but for two leaves.
    let mut top = result.clone();
    let entries = top
        .as_object_mut()
        .unwrap()
        .remove("sizes")
        .expect("result.sizes");
    let names = leaf_names(&top);
    let expected: BTreeSet<String> = TOP_LEAVES.iter().map(|s| (*s).to_owned()).collect();
    assert_eq!(names, expected, "result's leaves beside `sizes`");
    assert_eq!(result["seed"], 1, "the default seed");
    assert_eq!(result["budget"]["tokens"], DEFAULT_BUDGET);
    assert_eq!(result["budget"]["source"], "default");
    assert_eq!(result["corpus_only"], false);
    let entries = entries.as_array().expect("sizes is a list").clone();
    assert_eq!(entries.len(), sizes.len(), "one entry per size");

    let mut queries = Vec::new();
    for (at, (entry, &nodes)) in entries.iter().zip(sizes).enumerate() {
        let mut found = Vec::new();
        leaves(entry, "", &mut found);
        let names: BTreeSet<&str> = found.iter().map(|(name, _)| name.as_str()).collect();
        let expected: BTreeSet<&str> = SIZE_LEAVES.iter().copied().collect();
        assert_eq!(names, expected, "n{nodes}: the size's leaves");
        for (name, value) in &found {
            assert!(
                value.is_number() || value.is_null(),
                "n{nodes}: `{name}` is {value}, not a number or null"
            );
        }
        let previous = at.checked_sub(1).map(|before| &entries[before]);
        queries.extend(assert_size(run, entry, nodes, previous));
    }

    // Neither stream names an ID, a path or a query.
    for (stream, text) in [("stdout", run.stdout()), ("stderr", run.stderr())] {
        assert!(!holds_id(&text), "{stream} holds a `P-` ID:\n{text}");
        assert!(!text.contains('/'), "{stream} holds a `/`:\n{text}");
        let lower = text.to_ascii_lowercase();
        for query in &queries {
            assert!(
                !lower.contains(&query.to_ascii_lowercase()),
                "{stream} holds the query `{query}`:\n{text}"
            );
        }
    }
}

/// One measured size against its `size.json` and its tree; its queries.
fn assert_size(run: &ScaleRun, entry: &Value, nodes: u64, previous: Option<&Value>) -> Vec<String> {
    let at = format!("n{nodes}");
    assert_eq!(whole(entry, "nodes"), nodes, "{at}: `nodes`");
    assert_eq!(
        whole(entry, "corpus/nodes"),
        nodes + README_NODES,
        "{at}: `corpus.nodes` is N + 17"
    );
    assert_eq!(
        whole(entry, "index/nodes"),
        whole(entry, "corpus/nodes"),
        "{at}: `index.nodes` equals `corpus.nodes`"
    );
    assert_eq!(
        (whole(entry, "check/errors"), whole(entry, "check/warnings")),
        (0, 0),
        "{at}: the check finds nothing"
    );
    assert_eq!(
        whole(entry, "steps/update_parsed"),
        0,
        "{at}: the warm `update` parses nothing"
    );
    assert_eq!(
        whole(entry, "nondeterministic"),
        0,
        "{at}: `nondeterministic`"
    );
    let (hits_min, hits_max) = (
        whole(entry, "reads/search/hits_min"),
        whole(entry, "reads/search/hits_max"),
    );
    assert!(
        hits_min >= 1 && hits_max <= 16 && hits_min <= hits_max,
        "{at}: hits {hits_min}..{hits_max}, not within 1..=16"
    );
    assert_eq!(whole(entry, "corpus/hubs"), HUBS as u64, "{at}: four hubs");
    assert!(
        whole(entry, "corpus/degree_max") <= IN_DEGREE_CAP as u64,
        "{at}: `degree_max` above 16"
    );

    // Growth: `null` at the first size; else the ratio of medians.
    for (read, _) in READ_STEPS {
        let growth = &entry["growth"][read];
        match previous {
            None => assert!(
                growth.is_null(),
                "{at}: `growth.{read}` at the first size: {growth}"
            ),
            Some(before) => {
                let before = whole(before, &format!("reads/{read}/median_us"));
                let now = whole(entry, &format!("reads/{read}/median_us"));
                if before == 0 {
                    assert!(growth.is_null(), "{at}: `growth.{read}` over a 0: {growth}");
                } else {
                    let ratio = now as f64 / before as f64;
                    let printed = growth
                        .as_f64()
                        .unwrap_or_else(|| panic!("{at}: `growth.{read}` is {growth}"));
                    assert!(
                        (printed - ratio).abs() <= 0.0051,
                        "{at}: `growth.{read}` {printed}, {now} / {before} is {ratio:.4}"
                    );
                }
            }
        }
    }
    assert_split(entry, &at);

    // size.json: 20 targets, 3 queries, 4 hubs, and W_after from the files.
    let size = run.size_json(nodes);
    let targets = id_strings(&size["targets"], "targets");
    let hubs = id_strings(&size["hubs"], "hubs");
    let queries: Vec<String> = size["queries"]
        .as_array()
        .unwrap_or_else(|| panic!("{at}: size.json `queries` is not a list: {size}"))
        .iter()
        .map(|query| {
            query
                .as_str()
                .or_else(|| query.get("query").and_then(Value::as_str))
                .or_else(|| query.get("word").and_then(Value::as_str))
                .unwrap_or_else(|| panic!("{at}: a query that is no word: {query}"))
                .to_owned()
        })
        .collect();
    assert_eq!(targets.len(), W_TARGETS, "{at}: 20 targets");
    assert_eq!(queries.len(), QUERIES, "{at}: 3 queries");
    assert_eq!(hubs.len(), HUBS, "{at}: 4 hubs");

    let tree = Tree::read(&run.tree(nodes));
    let tree_hubs: Vec<String> = tree.hubs().into_iter().map(id_name).collect();
    assert_eq!(
        hubs, tree_hubs,
        "{at}: the hubs are the first four part documents"
    );
    assert_eq!(hubs[0], "P-000001", "{at}: the hub measured apart");
    let distinct: BTreeSet<&String> = targets.iter().collect();
    assert_eq!(
        distinct.len(),
        W_TARGETS,
        "{at}: targets drawn without replacement"
    );
    for target in &targets {
        let number = id_number(target);
        let holder = tree
            .holder(number)
            .unwrap_or_else(|| panic!("{at}: target {target} is in no part file"));
        assert!(holder.live(), "{at}: target {target} is not live");
        assert!(!hubs.contains(target), "{at}: target {target} is a hub");
    }
    let distinct: BTreeSet<&String> = queries.iter().collect();
    assert_eq!(distinct.len(), QUERIES, "{at}: three distinct queries");
    for query in &queries {
        assert_eq!(
            syllables(query),
            Some(RARE_SYLLABLES),
            "{at}: query `{query}` is not a rare word"
        );
        let held = tree.parts.iter().filter(|part| part.live()).any(|part| {
            part.sections
                .iter()
                .any(|section| Tree::rare_words(section).contains(&query.to_ascii_lowercase()))
        });
        assert!(held, "{at}: query `{query}` is in no live section");
    }

    let tier0 = tree.bytes("README.md");
    let rows = &size["w_after_bytes"];
    let per_target: Vec<(&String, &Value)> = match rows {
        Value::Object(map) => targets
            .iter()
            .map(|target| {
                let row = map
                    .get(target)
                    .unwrap_or_else(|| panic!("{at}: w_after_bytes has no {target}"));
                (target, row)
            })
            .collect(),
        Value::Array(items) => {
            assert_eq!(
                items.len(),
                W_TARGETS,
                "{at}: w_after_bytes, one per target"
            );
            targets.iter().zip(items).collect()
        }
        other => panic!("{at}: w_after_bytes is {other}"),
    };
    let mut totals = Vec::new();
    let mut tier1_max = 0;
    for (target, row) in per_target {
        if let Some(named) = row
            .get("id")
            .or_else(|| row.get("target"))
            .and_then(Value::as_str)
        {
            assert_eq!(named, target, "{at}: w_after_bytes in target order");
        }
        let area = &tree.holder(id_number(target)).unwrap().area;
        let tier1 = tree.area_readme(area);
        let (row_tier1, bundle, total) = (
            whole(row, "tier1"),
            whole(row, "bundle"),
            whole(row, "total"),
        );
        assert_eq!(
            row_tier1, tier1,
            "{at}: {target}: `tier1` is areas/{area}/README.md's bytes"
        );
        assert_eq!(
            total,
            tier0 + tier1 + bundle,
            "{at}: {target}: `total` is README.md ({tier0}) + areas/{area}/README.md ({tier1}) + `bundle` ({bundle})"
        );
        tier1_max = tier1_max.max(tier1);
        totals.push(total);
    }
    totals.sort_unstable();
    let rank = |p| totals[nearest_rank(totals.len(), p)];
    assert_eq!(
        (
            whole(entry, "w_after/median"),
            whole(entry, "w_after/p90"),
            whole(entry, "w_after/max"),
        ),
        (rank(50), rank(90), rank(100)),
        "{at}: W_after by nearest rank over the targets' totals"
    );
    assert_eq!(
        whole(entry, "w_after/tier0"),
        tier0,
        "{at}: `w_after.tier0`"
    );
    let readme_max = (0..AREAS)
        .map(|area| tree.area_readme(&format!("a{area:02}")))
        .max()
        .unwrap();
    let printed_tier1 = whole(entry, "w_after/tier1_max");
    assert!(
        (tier1_max..=readme_max).contains(&printed_tier1),
        "{at}: `w_after.tier1_max` {printed_tier1}, not within {tier1_max}..={readme_max}"
    );

    // `calls`: per read and target, three timings and four answers, the
    // pairs whose answers differ counted by `nondeterministic`.
    let calls = size
        .get("calls")
        .unwrap_or_else(|| panic!("{at}: size.json has no `calls`"));
    {
        let reads = calls
            .as_object()
            .unwrap_or_else(|| panic!("{at}: size.json `calls` is {calls}"));
        for (read, _) in READ_STEPS {
            assert!(reads.contains_key(read), "{at}: `calls` has no `{read}`");
        }
        let mut differing = 0;
        for (read, by_target) in reads {
            let by_target = by_target
                .as_object()
                .unwrap_or_else(|| panic!("{at}: `calls.{read}` is {by_target}"));
            for (target, call) in by_target {
                let us = call["us"].as_array().map_or(0, Vec::len);
                let answers: Vec<&str> = call["answers"]
                    .as_array()
                    .unwrap_or_else(|| panic!("{at}: `calls.{read}.{target}` has no answers"))
                    .iter()
                    .map(|answer| answer.as_str().unwrap_or(""))
                    .collect();
                assert_eq!(us, 3, "{at}: `calls.{read}.{target}`: three timings");
                assert_eq!(
                    answers.len(),
                    4,
                    "{at}: `calls.{read}.{target}`: four answers"
                );
                assert!(
                    answers.iter().all(|answer| answer.starts_with("b3:")),
                    "{at}: `calls.{read}.{target}`: answers are BLAKE3: {answers:?}"
                );
                differing += usize::from(answers.iter().any(|answer| *answer != answers[0]));
            }
        }
        assert_eq!(
            whole(entry, "nondeterministic"),
            differing as u64,
            "{at}: `nondeterministic` counts the pairs whose answers differ"
        );
    }

    // The corpus figures the tree fixes.
    let shipped = tree.parts.iter().filter(|part| !part.live()).count() as u64;
    assert_eq!(
        whole(entry, "corpus/tier3_files"),
        shipped,
        "{at}: `tier3_files`"
    );
    let incoming = tree.incoming();
    let hub_degrees: Vec<u64> = tree
        .hubs()
        .iter()
        .map(|hub| incoming.get(hub).copied().unwrap_or(0) as u64)
        .collect();
    assert_eq!(
        (
            whole(entry, "corpus/hub_degree/min"),
            whole(entry, "corpus/hub_degree/max"),
        ),
        (
            *hub_degrees.iter().min().unwrap(),
            *hub_degrees.iter().max().unwrap(),
        ),
        "{at}: `hub_degree` over the four hubs' incoming links"
    );
    assert_eq!(
        whole(entry, "hub/degree"),
        hub_degrees[0],
        "{at}: `hub.degree` is P-000001's incoming links"
    );
    assert!(
        whole(entry, "hub/w_after") >= tier0 + tree.area_readme("a00"),
        "{at}: the hub's W_after holds README.md and areas/a00/README.md"
    );
    queries
}

/// SE-8: each `other_us.<read>` is its median less its steps, floored at 0.
fn assert_split(entry: &Value, at: &str) {
    let mut positive = 0;
    for (read, steps) in READ_STEPS {
        let median = whole(entry, &format!("reads/{read}/median_us"));
        let sum: u64 = steps
            .iter()
            .map(|step| whole(entry, &format!("steps/{step}")))
            .sum();
        let expected = median.saturating_sub(sum);
        positive += usize::from(expected > 0);
        assert_eq!(
            whole(entry, &format!("other_us/{read}")),
            expected,
            "{at}: `other_us.{read}` is max(0, {median} - ({}) = {sum})",
            steps.join(" + ")
        );
    }
    if positive == 0 {
        eprintln!("{at}: every `other_us` is 0: the split's sums are not told apart this run");
    }
}

// ---------------------------------------------------------------------- SE-1

/// SE-1: one seed and size give one tree; another seed another digest; the
/// READMEs are the same at every size.
#[test]
fn se1_one_seed_and_size_give_one_tree() {
    let scratch = Scratch::new("scale-se1");
    let first = corpus_run(&scratch, "first", "7");
    let second = corpus_run(&scratch, "second", "7");
    for nodes in [1_000, 2_000] {
        let a = snapshot(&first.tree(nodes));
        let b = snapshot(&second.tree(nodes));
        assert!(!a.is_empty(), "n{nodes}/tree/ is written");
        let only: Vec<String> = a
            .keys()
            .filter(|path| !b.contains_key(*path))
            .chain(b.keys().filter(|path| !a.contains_key(*path)))
            .take(10)
            .map(|path| path.display().to_string())
            .collect();
        assert!(
            only.is_empty(),
            "n{nodes}/tree/: paths in one run only (first 10): {only:?}"
        );
        for (path, bytes) in &a {
            assert!(
                b[path] == *bytes,
                "n{nodes}/tree/{}: the bytes differ between two runs",
                path.display()
            );
        }
        assert_eq!(
            first.digest(nodes),
            second.digest(nodes),
            "n{nodes}: the same digest"
        );
        let size_a = fs::read(first.size_dir(nodes).join("size.json")).expect("size.json");
        let size_b = fs::read(second.size_dir(nodes).join("size.json")).expect("size.json");
        assert!(
            size_a == size_b,
            "n{nodes}/size.json differs between two runs"
        );
    }
    assert_eq!(
        first.envelope()["result"],
        second.envelope()["result"],
        "the same `result`"
    );

    let other = corpus_run(&scratch, "other", "8");
    assert_ne!(
        other.digest(1_000),
        first.digest(1_000),
        "`--seed 8` gives another digest"
    );

    let small = snapshot(&first.tree(1_000));
    let large = snapshot(&first.tree(2_000));
    let readmes: Vec<&PathBuf> = small
        .keys()
        .filter(|path| path.file_name().is_some_and(|name| name == "README.md"))
        .collect();
    assert_eq!(
        readmes.len(),
        1 + AREAS,
        "README.md and 16 areas/*/README.md"
    );
    for path in readmes {
        assert!(
            large.get(path) == Some(&small[path]),
            "{} differs between n1000 and n2000",
            path.display()
        );
    }
}

/// "Detail": `digest` is `b3:` over each file's path, NUL, bytes, NUL, in
/// path byte order.
#[test]
fn se1_digest_is_the_documented_hash() {
    let scratch = Scratch::new("scale-se1-digest");
    let run = corpus_run(&scratch, "out", "7");
    for nodes in [1_000, 2_000] {
        let mut files: Vec<(Vec<u8>, Vec<u8>)> = snapshot(&run.tree(nodes))
            .into_iter()
            .map(|(path, bytes)| (path.to_string_lossy().into_owned().into_bytes(), bytes))
            .collect();
        files.sort();
        let mut fed = Vec::new();
        for (path, bytes) in files {
            fed.extend(path);
            fed.push(0);
            fed.extend(bytes);
            fed.push(0);
        }
        assert_eq!(
            run.digest(nodes),
            specengine_store::b3_hash(&fed),
            "n{nodes}: `digest` over path, NUL, bytes, NUL in path byte order"
        );
    }
}

/// S2: the generator has no clock, no process id and no environment
/// (comments aside).
#[test]
fn se1_the_generator_reads_no_clock_pid_or_environment() {
    let path = repository_root().join("crates/specengine-eval/src/scale/generate.rs");
    let source = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{}: {error} (\"Files in scope\")", path.display()));
    let mut hits = Vec::new();
    for (at, line) in source.lines().enumerate() {
        let code = line.split("//").next().unwrap_or("");
        for pattern in [
            "SystemTime",
            "Instant",
            "std::env",
            "env::var",
            "process::id",
        ] {
            if code.contains(pattern) {
                hits.push(format!(
                    "generate.rs:{}: {pattern}: {}",
                    at + 1,
                    line.trim()
                ));
            }
        }
    }
    assert!(hits.is_empty(), "the generator reads:\n{}", hits.join("\n"));
}

/// "Data": a `--corpus-only` run writes the trees and `size.json`, no
/// `home/`, and prints `nodes` and `corpus` only per size.
#[test]
fn se1_corpus_only_writes_the_trees_and_prints_nodes_and_corpus() {
    let scratch = Scratch::new("scale-corpus-only");
    let run = corpus_run(&scratch, "out", "7");
    let envelope = run.envelope();
    assert_eq!(envelope["measurement"], "scale");
    assert_eq!(envelope["label"], LABEL);
    let result = &envelope["result"];
    let mut top = result.clone();
    let sizes = top
        .as_object_mut()
        .expect("`result` is an object")
        .remove("sizes")
        .expect("result.sizes");
    let expected: BTreeSet<String> = TOP_LEAVES.iter().map(|s| (*s).to_owned()).collect();
    assert_eq!(leaf_names(&top), expected, "result's leaves beside `sizes`");
    assert_eq!(result["seed"], 7);
    assert_eq!(result["corpus_only"], true);
    let sizes = sizes.as_array().expect("sizes is a list").clone();
    assert_eq!(sizes.len(), 2);
    let expected: BTreeSet<String> = CORPUS_ONLY_LEAVES.iter().map(|s| (*s).to_owned()).collect();
    for (entry, nodes) in sizes.iter().zip([1_000u64, 2_000]) {
        assert_eq!(
            leaf_names(entry),
            expected,
            "n{nodes}: `nodes` and `corpus` only"
        );
        assert_eq!(whole(entry, "nodes"), nodes);
        assert_eq!(whole(entry, "corpus/nodes"), nodes + README_NODES);
        assert!(run.tree(nodes).is_dir(), "n{nodes}/tree/ written");
        assert!(
            run.size_dir(nodes).join("size.json").is_file(),
            "n{nodes}/size.json written"
        );
        assert!(
            !run.size_dir(nodes).join("home").exists(),
            "n{nodes}: `home` is absent with --corpus-only"
        );
    }
    let stdout = run.stdout();
    assert!(
        !holds_id(&stdout) && !stdout.contains('/'),
        "stdout: {stdout}"
    );
}

// ---------------------------------------------------------------------- SE-2

/// SE-2: SE-1's `n1000` tree has "Data"'s shape.
#[test]
fn se2_the_n1000_tree_has_the_documented_shape() {
    let scratch = Scratch::new("scale-se2");
    let run = corpus_run(&scratch, "out", "7");
    assert_shape(&Tree::read(&run.tree(1_000)), 1_000);
}

// ---------------------------------------------------------------------- SE-3

/// SE-3: the `n1000` tree checks clean through `specengine_cli::check`.
#[test]
fn se3_the_n1000_tree_checks_clean() {
    let scratch = Scratch::new("scale-se3");
    let run = corpus_run(&scratch, "out", "7");
    let tree = run.tree(1_000);
    let home = scratch.join("check-home");
    fs::create_dir_all(&home).expect("scratch HOME");
    let env = Env {
        cwd: tree.clone(),
        home: Some(home.into_os_string()),
        xdg_data_home: None,
    };
    let globals = Globals {
        root: Some(tree),
        config: None,
    };
    let outcome = specengine_cli::check(&env, &globals, &CheckRequest::default())
        .unwrap_or_else(|error| panic!("check: {error}"));
    let report = &outcome.report;
    let listed = report
        .findings
        .iter()
        .take(20)
        .map(|f| {
            format!(
                "{}:{}: {} `{}`: {}",
                f.path, f.line, f.code, f.subject, f.message
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        report.cannot_check.is_empty(),
        "cannot check: {:?}",
        report.cannot_check
    );
    assert_eq!(
        (report.counts.errors, report.counts.warnings),
        (0, 0),
        "the n1000 tree's findings (first 20):\n{listed}"
    );
}

// ---------------------------------------------------------------------- SE-4

/// SE-4, first half: every token of the `n1000` tree is in the closed
/// vocabulary, and the tree is ASCII (a token is ASCII by definition, so a
/// non-ASCII word would slip past the token scan).
#[test]
fn se4_every_token_is_in_the_closed_vocabulary() {
    let scratch = Scratch::new("scale-se4-words");
    let run = corpus_run(&scratch, "out", "7");
    let tree = Tree::read(&run.tree(1_000));
    let mut misses = Vec::new();
    for (rel, bytes) in &tree.files {
        assert!(bytes.is_ascii(), "{rel}: a non-ASCII byte");
        let text = std::str::from_utf8(bytes).expect("ASCII");
        for (at, line) in text.lines().enumerate() {
            for token in tokens(line) {
                if !in_vocabulary(token) {
                    misses.push(format!("{rel}:{}: `{token}`", at + 1));
                }
            }
        }
    }
    let shown: Vec<&String> = misses.iter().take(30).collect();
    assert!(
        misses.is_empty(),
        "{} tokens outside the closed vocabulary, first 30:\n{shown:#?}",
        misses.len()
    );
}

/// SE-4, second half: with `SPECENGINE_PILOT_A` or `_B` set (and passed to
/// the child), neither corpus directory's name occurs in any written file,
/// case-insensitively. Unset, it says so and passes.
#[test]
fn se4_no_pilot_corpus_name_occurs_in_any_written_file() {
    let names = pilot_names();
    if names.is_empty() {
        eprintln!("no SPECENGINE_PILOT_A/_B set: the pilot-name half of SE-4 skipped");
        return;
    }
    let scratch = Scratch::new("scale-se4-pilots");
    let values: Vec<(&str, std::ffi::OsString)> = ["SPECENGINE_PILOT_A", "SPECENGINE_PILOT_B"]
        .into_iter()
        .filter_map(|variable| std::env::var_os(variable).map(|value| (variable, value)))
        .collect();
    let vars: Vec<(&str, &std::ffi::OsStr)> = values
        .iter()
        .map(|(variable, value)| (*variable, value.as_os_str()))
        .collect();
    let out = scratch.join("out");
    let run = scale_into(
        &scratch,
        "out",
        &out,
        &["--corpus-only", "--nodes", "1000,2000", "--seed", "7"],
        &vars,
    );
    assert_eq!(run.code(), Some(0), "stderr:\n{}", run.stderr());
    let written = snapshot(&run.out);
    assert!(!written.is_empty(), "the run wrote its trees");
    for (variable, name) in &names {
        let needle = name.to_ascii_lowercase();
        let hits: Vec<String> = written
            .iter()
            .filter(|(_, bytes)| {
                String::from_utf8_lossy(bytes)
                    .to_ascii_lowercase()
                    .contains(&needle)
            })
            .map(|(path, _)| path.display().to_string())
            .collect();
        assert!(
            hits.is_empty(),
            "the corpus name of {variable} occurs in:\n{}",
            hits.join("\n")
        );
        let stdout = run.stdout().to_ascii_lowercase();
        assert!(
            !stdout.contains(&needle),
            "the corpus name of {variable} is on stdout"
        );
    }
}

// ---------------------------------------------------------- SE-5, SE-7, SE-8

/// SE-5, SE-7 and SE-8 on one debug run at 1k (`scale --nodes 1000`),
/// `HOME` an empty scratch directory.
#[test]
fn se5_se7_se8_one_debug_run_at_1k() {
    let scratch = Scratch::new("scale-se5");
    let run = scale(&scratch, "out", &["--nodes", "1000"]);
    assert_eq!(
        run.code(),
        Some(0),
        "`scale --nodes 1000` exits 0; stderr:\n{}",
        run.stderr()
    );
    each_criterion(vec![
        criterion("SE-5", || assert_measured(&run, &[1_000])),
        criterion("SE-7", || {
            assert!(
                is_empty_dir(&run.home),
                "HOME is not left empty: {:?}",
                listing(&run.home)
            );
            assert!(
                is_empty_dir(&run.xdg),
                "XDG_DATA_HOME is not left empty: {:?}",
                listing(&run.xdg)
            );
            for trace in &run.traces {
                assert!(
                    !trace.exists(),
                    "a git child saw the process's environment: {} written",
                    trace.display()
                );
            }
            let data = if cfg!(target_os = "macos") {
                "Library/Application Support/specengine"
            } else {
                ".local/share/specengine"
            };
            let db = run
                .size_dir(1_000)
                .join("home")
                .join(data)
                .join("synthetic.db");
            assert!(
                db.is_file(),
                "the database is not at {}; n1000's `home` holds {:?}",
                db.display(),
                listing(&run.size_dir(1_000).join("home"))
            );
        }),
        criterion("SE-8", || {
            let envelope = run.envelope();
            assert_split(&envelope["result"]["sizes"][0], "n1000");
        }),
    ]);
}

/// SE-7, static half: the `scale` sources take neither `Env::from_process`
/// nor `process_git` (nor `GitEnv::from_process`), comments aside.
#[test]
fn se7_the_scale_sources_take_neither_the_process_env_nor_process_git() {
    let dir = repository_root().join("crates/specengine-eval/src/scale");
    let mut sources = Vec::new();
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(dir).unwrap_or_else(|error| panic!("{}: {error}", dir.display()))
        {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                out.push(path);
            }
        }
    }
    walk(&dir, &mut sources);
    sources.sort();
    for name in ["mod.rs", "generate.rs"] {
        assert!(
            sources.iter().any(|path| path.ends_with(name)),
            "src/scale/{name} (\"Files in scope\") is missing"
        );
    }
    let mut hits = Vec::new();
    for path in &sources {
        let source = fs::read_to_string(path).expect("readable source");
        for (at, line) in source.lines().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            for pattern in ["from_process", "process_git"] {
                if code.contains(pattern) {
                    hits.push(format!(
                        "{}:{}: {}",
                        path.strip_prefix(&dir).unwrap().display(),
                        at + 1,
                        line.trim()
                    ));
                }
            }
        }
    }
    assert!(
        hits.is_empty(),
        "the process's environment taken:\n{}",
        hits.join("\n")
    );
}

// ---------------------------------------------------------------------- SE-6

/// `scale <args>` exits 2 and writes no `--out`.
fn assert_refused(scratch: &Scratch, name: &str, args: &[&str]) {
    let run = scale(scratch, name, args);
    assert_eq!(
        run.code(),
        Some(2),
        "`scale {}` exits 2; stderr:\n{}",
        args.join(" "),
        run.stderr()
    );
    assert!(
        !run.out.exists(),
        "`scale {}` wrote {:?}",
        args.join(" "),
        listing(&run.out)
    );
}

/// SE-6: each flag outside its range is refused and nothing is written.
/// `--corpus-only --timeout 1` bounds a run a broken guard would let go.
#[test]
fn se6_flags_out_of_range_are_refused_and_nothing_is_written() {
    let scratch = Scratch::new("scale-se6-flags");
    let cases: [(&str, &[&str]); 10] = [
        ("nodes-499", &["--nodes", "499"]),
        ("nodes-200001", &["--nodes", "200001"]),
        ("nodes-unsorted", &["--nodes", "2000,1000"]),
        ("nodes-repeated", &["--nodes", "1000,1000"]),
        ("nodes-empty", &["--nodes", ""]),
        ("budget-0", &["--budget", "0"]),
        ("budget-above", &["--budget", "4294967296"]),
        ("label-dotdot", &["--label", ".."]),
        ("label-nested", &["--label", "a/b"]),
        ("pilot", &["--pilot", "x"]),
    ];
    for (name, args) in cases {
        let mut full = args.to_vec();
        full.extend(["--corpus-only", "--timeout", "1"]);
        assert_refused(&scratch, name, &full);
    }
}

/// The bounds themselves run: 500 nodes, seed 0, the largest budget, a
/// label of its own.
#[test]
fn se6_the_bounds_themselves_are_accepted() {
    let scratch = Scratch::new("scale-se6-bounds");
    let run = scale(
        &scratch,
        "out",
        &[
            "--corpus-only",
            "--nodes",
            "500",
            "--seed",
            "0",
            "--budget",
            "4294967295",
            "--label",
            "edge",
        ],
    );
    let envelope = run.envelope();
    assert_eq!(envelope["label"], "edge");
    assert_eq!(envelope["result"]["seed"], 0);
    assert_eq!(envelope["result"]["budget"]["tokens"], 4_294_967_295u64);
    assert_eq!(envelope["result"]["budget"]["source"], "flag");
    assert_eq!(
        whole(&envelope["result"]["sizes"][0], "corpus/nodes"),
        500 + README_NODES
    );
    assert!(run.out.join("scale/edge/n500/tree/README.md").is_file());
}

/// SE-6: `--out` inside the repository is refused before anything is
/// written; the directory is removed whatever the run did.
#[test]
fn se6_an_out_inside_the_repository_is_refused_before_any_write() {
    struct Removed(PathBuf);
    impl Drop for Removed {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let scratch = Scratch::new("scale-se6-repository");
    let out = repository_root()
        .join("target.noindex")
        .join("scale-refused");
    let _removed = Removed(out.clone());
    let _ = fs::remove_dir_all(&out);
    let args = ["--nodes", "1000", "--corpus-only", "--timeout", "1"];
    let run = scale_into(&scratch, "repository", &out, &args, &[]);
    assert_eq!(
        run.code(),
        Some(2),
        "`--out <repository>/target.noindex/scale-refused` exits 2; stderr:\n{}",
        run.stderr()
    );
    assert!(
        !out.exists(),
        "the refused run wrote under the repository: {:?}",
        listing(&out)
    );
}

/// SE-6: a symbolic link at `<out>/scale` or at `<out>/scale/<label>` is
/// refused, neither followed nor removed.
#[cfg(unix)]
#[test]
fn se6_a_symbolic_link_at_out_scale_or_its_label_is_refused() {
    use std::os::unix::fs::symlink;
    let scratch = Scratch::new("scale-se6-links");
    for (name, at) in [("at-scale", "scale"), ("at-label", "scale/synthetic")] {
        let out = scratch.join(name);
        let elsewhere = scratch.join(&format!("{name}.elsewhere"));
        fs::create_dir_all(&elsewhere).unwrap();
        fs::write(elsewhere.join("kept"), b"kept").unwrap();
        let link = out.join(at);
        fs::create_dir_all(link.parent().unwrap()).unwrap();
        symlink(&elsewhere, &link).unwrap();
        let before = listing(&out);
        let args = ["--nodes", "1000", "--corpus-only", "--timeout", "1"];
        let run = scale_into(&scratch, name, &out, &args, &[]);
        assert_eq!(
            run.code(),
            Some(2),
            "a link at <out>/{at} exits 2; stderr:\n{}",
            run.stderr()
        );
        assert_eq!(
            listing(&out),
            before,
            "<out> as it was, the link at {at} kept"
        );
        assert_eq!(
            listing(&elsewhere),
            vec![("kept".to_owned(), "file 4".to_owned())],
            "the link's target untouched"
        );
    }
}

/// SE-6: `--timeout 0` exits 0 with `result` `"timeout"`.
#[test]
fn se6_timeout_zero_exits_0_with_result_timeout() {
    let scratch = Scratch::new("scale-se6-timeout");
    let run = scale(&scratch, "out", &["--nodes", "1000", "--timeout", "0"]);
    let envelope = run.envelope();
    assert_eq!(envelope["measurement"], "scale");
    assert_eq!(envelope["result"], "timeout", "{envelope}");
}

// ---------------------------------------------------------------------- SE-9

/// SE-9, manual and in release:
/// `cargo nextest run -p specengine-eval --release --test scale_cli
/// --run-ignored ignored-only scale_baseline --no-capture`. The default
/// sizes under `--timeout 3600`, not `"timeout"`; SE-5's checks and SE-8's
/// split at each size, and each tree's shape (SE-2's checks, where the
/// group and in-degree caps bind). It prints the envelope and asserts no
/// time. Its corpora lie under `std::env::temp_dir()` (`TMPDIR`) and are
/// removed; with `SPECENGINE_SCALE_KEEP` set to a directory, the envelope
/// (`envelope.json`) and each size's `size.json` (`n<N>.size.json`) are
/// copied there first, whatever the run's outcome.
#[test]
#[ignore = "manual, release: the scale baseline (docs/features/scale-eval.md SE-9)"]
fn scale_baseline() {
    let scratch = Scratch::new("scale-baseline");
    let run = scale(&scratch, "out", &["--timeout", "3600"]);
    println!("{}", run.stdout());
    eprintln!("{}", run.stderr());
    if let Some(keep) = std::env::var_os("SPECENGINE_SCALE_KEEP").filter(|v| !v.is_empty()) {
        let keep = PathBuf::from(keep);
        fs::create_dir_all(&keep).expect("the keep directory");
        fs::write(keep.join("envelope.json"), &run.output.stdout).expect("envelope.json kept");
        for nodes in DEFAULT_SIZES {
            let size = run.size_dir(nodes).join("size.json");
            if size.is_file() {
                fs::write(
                    keep.join(format!("n{nodes}.size.json")),
                    fs::read(&size).expect("size.json"),
                )
                .expect("size.json kept");
            }
        }
    }
    assert_eq!(run.code(), Some(0), "the baseline exits 0");
    let envelope = run.envelope();
    assert_ne!(
        envelope["result"], "timeout",
        "the baseline ran out of --timeout 3600"
    );
    assert_measured(&run, &DEFAULT_SIZES);
    for nodes in DEFAULT_SIZES {
        assert_shape(&Tree::read(&run.tree(nodes)), nodes as usize);
    }
}
