//! docs/features/write-cost.md (RM-0125) (b), a write's check over the walked
//! rows (ADR-0083), against the whole disk check (0.47.0's
//! `introduced_findings`), on scratch copies of the fixture corpora:
//!
//! - WC19 `wc19_*`: after hand edits the walk saw (three documents changed on
//!   disk, then `update`), every edit of WC1's catalogue
//!   (`crates/specengine-core/tests/common/write_cost.rs`) of the corpus's
//!   first documents, and its new files, gives the disk check's findings,
//!   byte for byte as JSON, order included, with a memo and without, never
//!   doubted. M: affected files taken from rows instead of disk.
//! - WC21 `wc21_*`: an affected file whose bytes differ from its row (a hand
//!   edit no walk saw, the stat a lie) is caught by the hash net: the
//!   findings are the disk check's, the call says doubted, and the memo's D
//!   is emptied. M: no hash check.
//! - WC22 `wc22_*`: a same-size edit under a stat key inside the racy window
//!   (ctime now) is read again by the next walk, and a write's check after it
//!   is the disk check's. M: trusting a racy key.
//! - WC23 `wc23_*`, the hole pinned (ADR-0083's cost, not a failure): a
//!   same-size edit under an unmoved stat key past the racy window, outside
//!   the set the rows give, is unseen: the rows' findings miss what the disk
//!   check finds, and a repeated call is a memo hit.
//! - WC24 `wc24_*`: the ID preview by query (`SqliteIndex::
//!   highest_record_number`) equals core's `highest_record_number` over the
//!   decoded corpus, on each side, a kept (alias) ID included.

#![cfg(unix)]

mod common;

#[path = "../../specengine-core/tests/common/write_cost.rs"]
mod catalogue;

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use catalogue::{
    Doc, Draw, Edit, GENERATED_CONFIG, chain_closers, creates, cycle_cases, cycle_corpus, edits_of,
    generated, sample, unmet,
};
use common::{Scratch, copy_dir, repository_root};
use specengine_core::Paths;
use specengine_core::record::{IdSide, highest_record_number};
use specengine_model::Shape;
use specengine_store::check_memo::scoped;
use specengine_store::{
    CheckMemo, IndexWriter as _, Listing, NamedBytes, Probed, SetupFiles, Source, SpecIndex as _,
    SqliteIndex, StatKey, WorkingTree, default_baseline, introduced_findings, introduced_from_rows,
    load_check,
};

const TODAY: &str = "2026-11-15";

/// A scratch copy of one corpus, its index walked once.
struct World {
    _scratch: Scratch,
    root: PathBuf,
    config: NamedBytes,
    baseline: Option<NamedBytes>,
    setup: specengine_store::CheckSetup,
    index: SqliteIndex,
}

impl World {
    fn new(corpus: &Path, label: &str) -> Option<Self> {
        let scratch = Scratch::new(label);
        let root = scratch.join("root");
        copy_dir(corpus, &root);
        let root = fs::canonicalize(&root).expect("canonical root");
        let config = NamedBytes::read("specengine.toml", &root.join("specengine.toml"));
        let baseline = default_baseline(&root);
        let setup = load_check(&config, baseline.as_ref()).ok()?;
        let slug = setup.project.slug().unwrap_or("corpus").to_owned();
        let db = scratch.db("index");
        let index = SqliteIndex::open(db, &slug, &root).expect("open");
        let mut world = Self {
            _scratch: scratch,
            root,
            config,
            baseline,
            setup,
            index,
        };
        world.walk();
        Some(world)
    }

    fn tree(&self) -> WorkingTree {
        WorkingTree::new(&self.root, &self.setup.project.paths).expect("a tree")
    }

    /// The index walked over the tree as it is now.
    fn walk(&mut self) {
        let tree = self.tree();
        self.index
            .update(&tree, &self.setup.project.scheme)
            .expect("update");
    }

    /// The walked documents, in path order.
    fn docs(&self) -> Vec<Doc> {
        let input = self.index.indexed_input().expect("decode");
        input
            .files
            .iter()
            .filter_map(|file| {
                let text = fs::read_to_string(self.root.join(&file.path)).ok()?;
                Some(Doc {
                    path: file.path.clone(),
                    text,
                })
            })
            .collect()
    }

    fn files(&self) -> SetupFiles<'_> {
        SetupFiles {
            config: &self.config,
            baseline: self.baseline.as_ref(),
        }
    }

    /// The disk check of `edit` over `source`.
    fn whole(&self, source: &dyn Source, edit: &Edit) -> String {
        json(&introduced_findings(
            source,
            &self.setup,
            TODAY,
            &edit.path,
            edit.text.clone().into_bytes(),
        ))
    }

    /// The rows' check of `edit` over `source`: its findings as JSON and
    /// whether it doubted.
    fn rows(&self, source: &dyn Source, edit: &Edit) -> (String, bool) {
        let found = introduced_from_rows(
            Some(&self.index),
            source,
            &self.setup,
            self.files(),
            TODAY,
            &edit.path,
            edit.text.clone().into_bytes(),
        );
        (json(&found.findings), found.doubted)
    }
}

fn json(findings: &[specengine_store::ProposalFinding]) -> String {
    serde_json::to_string(findings).expect("JSON")
}

/// Every fixture corpus's directory, in path order.
fn corpora() -> Vec<PathBuf> {
    let fixtures = repository_root().join("fixtures");
    let mut found = Vec::new();
    let mut dirs: Vec<_> = fs::read_dir(&fixtures)
        .expect("fixtures")
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    dirs.sort();
    for dir in dirs {
        if dir.join("specengine.toml").is_file() {
            found.push(dir.clone());
        }
        let mut inner: Vec<_> = fs::read_dir(&dir)
            .expect("a fixture")
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.is_dir() && path.join("specengine.toml").is_file())
            .collect();
        inner.sort();
        found.extend(inner);
    }
    found
}

fn label(corpus: &Path) -> String {
    corpus
        .strip_prefix(repository_root().join("fixtures"))
        .expect("a fixture")
        .display()
        .to_string()
        .replace('/', "-")
}

// ------------------------------------------------------------------ WC19

/// WC19: hand edits the walk saw, then WC1's catalogue: rows equal disk.
#[test]
fn wc19_after_a_walk_that_saw_every_edit_rows_equal_the_disk_check() {
    let mut cases = 0;
    let mut differ = Vec::new();
    for corpus in corpora() {
        let Some(mut world) = World::new(&corpus, &format!("wc19-{}", label(&corpus))) else {
            continue;
        };
        let docs = world.docs();
        if docs.len() < 2 {
            continue;
        }
        // Three hand edits on disk, each one of the catalogue's, then a walk.
        for at in [0, docs.len() / 2, docs.len() - 1] {
            if let Some(edit) = edits_of(&docs, at)
                .into_iter()
                .find(|edit| edit.path == docs[at].path)
            {
                fs::write(world.root.join(&edit.path), &edit.text).expect("a hand edit");
            }
        }
        world.walk();
        let docs = world.docs();
        let mut edits: Vec<Edit> = (0..docs.len().min(6))
            .flat_map(|at| edits_of(&docs, at))
            .collect();
        edits.extend(creates(&docs));
        let tree = world.tree();
        let memo = Arc::new(CheckMemo::new());
        for edit in &edits {
            let whole = world.whole(&tree, edit);
            let cold = world.rows(&tree, edit);
            let warm = scoped(&memo, "-", || {
                world.rows(&tree, edit);
                world.rows(&tree, edit)
            });
            cases += 1;
            for (side, (got, doubted)) in [("no memo", cold), ("memo", warm)] {
                if got != whole || doubted {
                    differ.push(format!(
                        "{} {} {} ({side}, doubted {doubted}):\n  rows  {got}\n  whole {whole}",
                        label(&corpus),
                        edit.family,
                        edit.path
                    ));
                }
            }
        }
    }
    assert!(cases > 1_000, "{cases} cases");
    assert!(
        differ.is_empty(),
        "{} of {cases} differ:\n{}",
        differ.len(),
        differ
            .iter()
            .take(6)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n\n")
    );
}

/// A three-file corpus under `dir`: `a`, `b` and `c`, giving `N-A`, `N-B`,
/// `N-C`; `a` and `b` point on (`key`: `depends_on` under `links:`, or
/// `parent:`), `a` -> `b` -> `c`, each pointer a few lines down.
fn three_in_a_row(dir: &Path, key: &str) {
    let config = "[project]\nname = \"cycles\"\nslug = \"cycles\"\n\n[ids]\n\
                  N = { kind = \"node\", shape = \"name\" }\n\n[paths]\nroots = [\"docs\"]\n";
    fs::create_dir_all(dir.join("docs")).expect("docs");
    fs::write(dir.join("specengine.toml"), config).expect("a config");
    for (name, next) in [("a", Some("N-B")), ("b", Some("N-C")), ("c", None)] {
        let id = format!("N-{}", name.to_uppercase());
        let pointer = match (next, key) {
            (Some(next), "depends_on") => format!("links:\n  depends_on: [{next}]\n"),
            (Some(next), _) => format!("parent: {next}\n"),
            (None, _) => String::new(),
        };
        fs::write(
            dir.join("docs").join(format!("{name}.md")),
            format!(
                "---\nid: {id}\nclass: spec\nstatus: draft\ntitle: Node {name}\nowner: owner\n\
                 {pointer}---\n\n# Node {name}\n\nA body line.\n"
            ),
        )
        .expect("a document");
    }
}

/// WC19 (the review's case): `a` -> `b` -> `c`, and the write makes `c` point
/// at `a`, closing a cycle whose first member by name, `a`, lies outside the
/// set (it reads `N-B`, not `N-C`). The rows' finding names the line `a`'s
/// pointer stands on, as the disk check does, byte for byte; for a
/// `depends_on` cycle and a `parent:` one, with a memo and without. Red while
/// rows are decoded without bytes (line 1).
#[test]
fn wc19_a_cycle_whose_first_member_is_outside_the_set_keeps_its_line() {
    for key in ["depends_on", "parent"] {
        let scratch = Scratch::new(&format!("wc19-cycle-{key}"));
        let source = scratch.join("source");
        three_in_a_row(&source, key);
        let world = World::new(&source, &format!("wc19-cycle-{key}-world")).expect("a world");
        let c = fs::read_to_string(world.root.join("docs/c.md")).expect("c.md");
        let closing = if key == "depends_on" {
            "links:\n  depends_on: [N-A]\n---\n"
        } else {
            "parent: N-A\n---\n"
        };
        let edit = Edit {
            family: if key == "depends_on" {
                "depends-closed"
            } else {
                "parent-closed"
            },
            path: "docs/c.md".to_owned(),
            text: c.replacen(
                "owner: owner\n---\n",
                &format!("owner: owner\n{closing}"),
                1,
            ),
        };
        assert_ne!(edit.text, c, "{key}: c points at a");
        let tree = world.tree();
        let whole = world.whole(&tree, &edit);
        let cycle = if key == "depends_on" {
            "depends-cycle"
        } else {
            "parent-cycle"
        };
        assert!(
            whole.contains(cycle),
            "{key}: the disk check finds the cycle: {whole}"
        );
        // The cycle is named on `a`, its first member, at its pointer's line,
        // not line 1.
        assert!(
            whole.contains("\"path\":\"docs/a.md\",\"line\":")
                && !whole.contains("\"path\":\"docs/a.md\",\"line\":1,"),
            "{key}: the cycle stands on a's pointer: {whole}"
        );
        let (cold, doubted) = world.rows(&tree, &edit);
        assert!(!doubted, "{key}");
        assert_eq!(cold, whole, "{key}: the rows' cycle, its line included");
        let memo = Arc::new(CheckMemo::new());
        let warm = scoped(&memo, "-", || {
            world.rows(&tree, &edit);
            world.rows(&tree, &edit)
        });
        assert_eq!(warm, (whole.clone(), false), "{key}: under a memo");
    }
}

/// WC19 over WC2's generated corpora (written to disk, walked): five drawn
/// documents each under every family, the new files, and the review's chain
/// closers; rows equal the disk check, line included.
#[test]
fn wc19_generated_corpora_rows_equal_the_disk_check() {
    let mut cases = 0;
    let mut differ = Vec::new();
    for seed in 1..=6_u64 {
        let count = 20 + Draw::new(seed * 7_919).below(181);
        let docs = generated(seed, count);
        let scratch = Scratch::new(&format!("wc19-gen-{seed}"));
        let source = scratch.join("source");
        fs::create_dir_all(&source).expect("source");
        fs::write(source.join("specengine.toml"), GENERATED_CONFIG).expect("a config");
        for doc in &docs {
            let path = source.join(&doc.path);
            fs::create_dir_all(path.parent().expect("a parent")).expect("dirs");
            fs::write(path, &doc.text).expect("a document");
        }
        let world = World::new(&source, &format!("wc19-gen-{seed}-world")).expect("a world");
        let mut edits: Vec<Edit> = sample(docs.len(), 5, seed)
            .into_iter()
            .flat_map(|at| edits_of(&docs, at))
            .collect();
        edits.extend(creates(&docs).into_iter().take(8));
        edits.extend(chain_closers(&docs));
        let tree = world.tree();
        for edit in &edits {
            let whole = world.whole(&tree, edit);
            let (got, doubted) = world.rows(&tree, edit);
            cases += 1;
            if got != whole || doubted {
                differ.push(format!(
                    "seed {seed} {} {} (doubted {doubted}):\n  rows  {got}\n  whole {whole}",
                    edit.family, edit.path
                ));
            }
        }
    }
    assert!(cases >= 300, "{cases} cases");
    assert!(
        differ.is_empty(),
        "{} of {cases} differ:\n{}",
        differ.len(),
        differ
            .iter()
            .take(6)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n\n")
    );
}

/// `(code, path, line, subject)` of each finding of a JSON list.
fn tuples(json: &str) -> Vec<(String, String, usize, String)> {
    let list: Vec<serde_json::Value> = serde_json::from_str(json).expect("JSON");
    list.iter()
        .map(|finding| {
            (
                finding["code"].as_str().unwrap_or("").to_owned(),
                finding["path"].as_str().unwrap_or("").to_owned(),
                usize::try_from(finding["line"].as_u64().unwrap_or(0)).expect("a line"),
                finding["subject"].as_str().unwrap_or("").to_owned(),
            )
        })
        .collect()
}

/// The analyst's cycle cases through the rows path (written to disk,
/// walked): the rows equal the disk check byte for byte, line included,
/// with and without a memo, and each case's expected finding stands on both.
/// M: the region without the both-sides closure; `parent-cycle` located by
/// name; "introduced" by node set; dangling keys left out; P's liveness
/// flip left out.
#[test]
fn wc19_cycle_cases_rows_equal_the_disk_check() {
    let scratch = Scratch::new("wc19-cycles");
    let source = scratch.join("source");
    fs::create_dir_all(&source).expect("source");
    fs::write(source.join("specengine.toml"), GENERATED_CONFIG).expect("a config");
    for doc in cycle_corpus() {
        let path = source.join(&doc.path);
        fs::create_dir_all(path.parent().expect("a parent")).expect("dirs");
        fs::write(path, &doc.text).expect("a document");
    }
    let world = World::new(&source, "wc19-cycles-world").expect("a world");
    let (_, edits, expects) = cycle_cases();
    let tree = world.tree();
    let memo = Arc::new(CheckMemo::new());
    let mut problems = Vec::new();
    for edit in &edits {
        let whole = world.whole(&tree, edit);
        let (cold, doubted) = world.rows(&tree, edit);
        if cold != whole || doubted {
            problems.push(format!(
                "{} (doubted {doubted}):\n  rows  {cold}\n  whole {whole}",
                edit.family
            ));
        }
        let warm = scoped(&memo, "-", || {
            world.rows(&tree, edit);
            world.rows(&tree, edit)
        });
        if warm != (whole.clone(), false) {
            problems.push(format!("{} under a memo: {warm:?}", edit.family));
        }
        for expect in expects.iter().filter(|expect| expect.family == edit.family) {
            for (side, found) in [("whole", &whole), ("rows", &cold)] {
                if let Some(problem) = unmet(expect, &tuples(found)) {
                    problems.push(format!("{side}: {problem}"));
                }
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

// ------------------------------------------------------------------ WC21

/// WC21: a hand edit no walk saw, on the file written (in every set): the
/// hash net doubts, the findings are the disk check's, and D is emptied.
#[test]
fn wc21_an_affected_file_unlike_its_row_doubts_and_gives_the_disk_check() {
    let mut checked = 0;
    for name in ["spec-a", "spec-b", "card-press", "code-a"] {
        let corpus = repository_root().join("fixtures").join(name);
        let world = World::new(&corpus, &format!("wc21-{name}")).expect("a world");
        let docs = world.docs();
        let target = docs
            .iter()
            .position(|doc| catalogue::own_id(&doc.text).is_some())
            .expect("a document with an ID");
        let edit = edits_of(&docs, target)
            .into_iter()
            .find(|edit| edit.family == "mention")
            .expect("a mention edit");
        // Warm a memo over the walked rows with another edit: this edit's D
        // key is then a miss, and its check goes over the rows.
        let tree = world.tree();
        let other = edits_of(&docs, target)
            .into_iter()
            .find(|edit| edit.family == "heading-renamed" || edit.family == "id-added")
            .expect("another edit");
        let memo = Arc::new(CheckMemo::new());
        scoped(&memo, "-", || {
            world.rows(&tree, &other);
            world.rows(&tree, &other);
        });
        let held = memo.entries();
        assert!(held > 0, "{name}: the memo holds C and D");
        // The file changes on disk; no walk: its row lies.
        let path = world.root.join(&docs[target].path);
        let mut text = fs::read_to_string(&path).expect("the file");
        text.push_str("\nA hand edit no walk saw, citing ZZ-405.\n");
        fs::write(&path, text).expect("a hand edit");
        let whole = world.whole(&tree, &edit);
        let (got, doubted) = world.rows(&tree, &edit);
        assert!(doubted, "{name}: the hash net doubts");
        assert_eq!(got, whole, "{name}: the disk check's findings");
        let (got, doubted) = scoped(&memo, "-", || world.rows(&tree, &edit));
        assert!(doubted, "{name}: under the memo too");
        assert_eq!(got, whole, "{name}");
        assert!(memo.entries() < held, "{name}: D emptied by the doubt");
        checked += 1;
    }
    assert_eq!(checked, 4);
}

// ------------------------------------------------------------- WC22, WC23

/// A source over `inner` whose `stat` gives `key` for `path` and the inner
/// key for every other.
struct Keyed<'a> {
    inner: &'a dyn Source,
    path: String,
    key: StatKey,
}

impl Source for Keyed<'_> {
    fn root(&self) -> &Path {
        self.inner.root()
    }

    fn paths(&self) -> &Paths {
        self.inner.paths()
    }

    fn list(&self) -> io::Result<Listing> {
        self.inner.list()
    }

    fn probe(&self, path: &str) -> bool {
        self.inner.probe(path)
    }

    fn read(&self, path: &str) -> io::Result<Vec<u8>> {
        self.inner.read(path)
    }

    fn is_dir(&self, path: &str) -> bool {
        self.inner.is_dir(path)
    }

    fn probe_paths(&self, paths: &[&str]) -> Vec<Probed> {
        self.inner.probe_paths(paths)
    }

    fn stat(&self, path: &str) -> Option<StatKey> {
        if path == self.path {
            Some(self.key)
        } else {
            self.inner.stat(path)
        }
    }
}

fn now_ns() -> i64 {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("after the epoch")
            .as_nanos(),
    )
    .expect("before 2262")
}

const HOUR_NS: i64 = 3_600_000_000_000;

/// spec-a's `game.md` body line `from` made `to`, the same length.
fn same_size(text: &str, from: &str, to: &str) -> String {
    assert_eq!(from.len(), to.len(), "{from:?} and {to:?}");
    assert!(text.contains(from), "{from:?}");
    text.replacen(from, to, 1)
}

/// WC22: one file's stat key fixed with its ctime now (inside the racy
/// window): a same-size edit is read again by the next walk, and a write
/// after it is the disk check's. M: a racy key trusted (the edit unseen).
#[test]
fn wc22_a_same_size_edit_inside_the_racy_window_is_read_again() {
    let corpus = repository_root().join("fixtures").join("spec-a");
    let mut world = World::new(&corpus, "wc22").expect("spec-a");
    let path = "docs/spec/movement/sprint.md";
    let tree = world.tree();
    let key = StatKey {
        size: fs::metadata(world.root.join(path))
            .expect("sprint.md")
            .len(),
        mtime_ns: now_ns() - HOUR_NS,
        ctime_ns: now_ns(),
        ino: 1,
        dev: 1,
    };
    let keyed = Keyed {
        inner: &tree,
        path: path.to_owned(),
        key,
    };
    world
        .index
        .update(&keyed, &world.setup.project.scheme)
        .expect("a walk under the fixed key");
    // The same size, the same key: a mention of the movement document's ID
    // turned into one of an ID nothing gives.
    let full = world.root.join(path);
    let text = fs::read_to_string(&full).expect("sprint.md");
    let edited = same_size(&text, "parent: DOM-MOVEMENT", "parent: DOM-MOVEMENX");
    fs::write(&full, &edited).expect("a same-size edit");
    world
        .index
        .update(&keyed, &world.setup.project.scheme)
        .expect("a walk under the same key");
    let docs = world.docs();
    let movement = docs
        .iter()
        .position(|doc| doc.path == "docs/spec/movement/README.md")
        .expect("README.md");
    for edit in edits_of(&docs, movement) {
        let whole = world.whole(&tree, &edit);
        let (got, doubted) = world.rows(&tree, &edit);
        assert!(!doubted, "{}: the walk read the edit", edit.family);
        assert_eq!(got, whole, "{} {}", edit.family, edit.path);
    }
}

/// WC23, the hole pinned: the same, the ctime an hour back too (past the
/// racy window): the walk trusts the key, the row keeps the old bytes, and a
/// write whose set the rows give misses what the disk check finds; a
/// repeated call is a memo hit. ADR-0083's cost, documented, not a failure.
#[test]
fn wc23_a_stat_unmoved_edit_past_the_racy_window_is_unseen() {
    let corpus = repository_root().join("fixtures").join("spec-a");
    let mut world = World::new(&corpus, "wc23").expect("spec-a");
    let path = "docs/spec/movement/sprint.md";
    let tree = world.tree();
    let key = StatKey {
        size: fs::metadata(world.root.join(path))
            .expect("sprint.md")
            .len(),
        mtime_ns: now_ns() - HOUR_NS,
        ctime_ns: now_ns() - HOUR_NS,
        ino: 1,
        dev: 1,
    };
    let keyed = Keyed {
        inner: &tree,
        path: path.to_owned(),
        key,
    };
    world
        .index
        .update(&keyed, &world.setup.project.scheme)
        .expect("a walk under the fixed key");
    std::thread::sleep(Duration::from_millis(50));
    // sprint.md's parent becomes DOM-GAME (the same size: YAML drops the
    // trailing spaces); the walk trusts the unmoved key and keeps the row
    // whose parent is DOM-MOVEMENT.
    let full = world.root.join(path);
    let text = fs::read_to_string(&full).expect("sprint.md");
    fs::write(
        &full,
        same_size(&text, "parent: DOM-MOVEMENT", "parent: DOM-GAME    "),
    )
    .expect("a same-size edit");
    let report = world
        .index
        .update(&keyed, &world.setup.project.scheme)
        .expect("a walk under the same key");
    assert_eq!(
        report.walked - report.hashed,
        1,
        "the walk trusted sprint.md's unmoved key: {report:?}"
    );
    // The write: game.md loses its ID. On disk sprint.md reads that key, so
    // the disk check finds its parent dangling; the rows say it reads
    // DOM-MOVEMENT, so the set leaves it out and nothing reads its bytes.
    let game = "docs/spec/game.md";
    let text = fs::read_to_string(world.root.join(game)).expect("game.md");
    let edit = Edit {
        family: "id-removed",
        path: game.to_owned(),
        text: text.replacen("id: DOM-GAME\n", "", 1),
    };
    assert_ne!(edit.text, text, "game.md gives DOM-GAME");
    let sprint = "\"path\":\"docs/spec/movement/sprint.md\"";
    let whole = world.whole(&keyed, &edit);
    assert!(
        whole.contains(sprint),
        "the disk check finds sprint.md's parent dangling: {whole}"
    );
    let (cold, doubted) = world.rows(&keyed, &edit);
    assert!(!doubted, "nothing the set reads differs from its row");
    assert!(!cold.contains(sprint), "the hole: the rows miss it: {cold}");
    // A memo warmed by another write: this one's check goes over the rows,
    // is kept, and the next call is a hit with the same stale findings.
    let other = Edit {
        family: "mention",
        path: game.to_owned(),
        text: format!("{text}\nAs ZZ-404 says.\n"),
    };
    let memo = Arc::new(CheckMemo::new());
    let (first, again, hits) = scoped(&memo, "-", || {
        world.rows(&keyed, &other);
        let first = world.rows(&keyed, &edit);
        let hits = memo.hits();
        let again = world.rows(&keyed, &edit);
        (first, again, memo.hits() - hits)
    });
    assert_eq!(
        first,
        (cold.clone(), false),
        "the rows' findings under the memo"
    );
    assert_eq!(again, first, "a repeated call keeps them");
    assert_eq!(hits, 1, "a memo hit");
}

// ------------------------------------------------------------------ WC24

/// WC24: `SqliteIndex::highest_record_number` (the ID preview by query)
/// equals core's over the decoded corpus, every number prefix of every
/// fixture corpus, both sides; hub-side numbers and a kept (alias) ID
/// written in, and the answers again equal. M: a wrong prefix or width; a
/// missed alias.
#[test]
fn wc24_the_preview_by_query_equals_the_decoded_corpus() {
    let mut compared = 0;
    for corpus in corpora() {
        let Some(mut world) = World::new(&corpus, &format!("wc24-{}", label(&corpus))) else {
            continue;
        };
        let specs: Vec<_> = world
            .setup
            .project
            .scheme
            .prefixes()
            .iter()
            .filter(|spec| spec.shape == Shape::Number)
            .cloned()
            .collect();
        if specs.is_empty() {
            continue;
        }
        let mut compare = |world: &World, when: &str| {
            let input = world.index.indexed_input().expect("decode");
            let parsed: Vec<_> = input
                .files
                .iter()
                .filter_map(|file| file.parsed.as_ref())
                .collect();
            for spec in &specs {
                for side in [IdSide::Terminal, IdSide::Hub] {
                    let decoded = highest_record_number(parsed.iter().copied(), spec, side);
                    let queried = world
                        .index
                        .highest_record_number(spec, side)
                        .expect("a query");
                    assert_eq!(
                        queried,
                        decoded,
                        "{} {when}: {} {side:?}",
                        label(&corpus),
                        spec.prefix
                    );
                    compared += 1;
                }
            }
        };
        compare(&world, "as it is");
        // A hub-side number and a kept ID (an alias) of the first prefix.
        let spec = &specs[0];
        let width = usize::try_from(spec.width.unwrap_or(1)).expect("a width");
        let hub = format!("{}-{:0width$}", spec.prefix, 100_007);
        let kept = format!("{}-{:0width$}", spec.prefix, 77);
        let Some(dir) = corpus_dir(&world) else {
            continue;
        };
        fs::write(
            world.root.join(&dir).join("zz-wc24-hub.md"),
            format!("---\nid: {hub}\nclass: decision\naliases: [{kept}]\n---\n\n# Hub\n\nA hub record.\n"),
        )
        .expect("a hub record");
        world.walk();
        compare(&world, "with a hub number and an alias");
    }
    assert!(compared > 20, "{compared} comparisons");
}

/// A directory of the corpus that holds a walked document.
fn corpus_dir(world: &World) -> Option<String> {
    let docs = world.docs();
    docs.first()
        .map(|doc| catalogue::dir_of(&doc.path).to_owned())
}
