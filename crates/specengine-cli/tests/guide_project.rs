//! docs/features/specengine-guide.md (RM-0129) "Acceptance criteria", the
//! guide never inside a project: the `spec` binary over scratch copies of
//! `fixtures/spec-a` (a game), each with its own `HOME`; the fixture and
//! the repository are only read.
//!
//! - GU4 `gu4_*`: on spec-a, `spec check`, `spec map`, `spec library`,
//!   `spec tree` and `spec search guide` (text and JSON, exits and stderr)
//!   equal the base commit's ([`GOLDEN`], recorded from 8015bdf8's `spec`
//!   by `record_the_reads_before_the_guide`, `#[ignore]`, by hand); a
//!   document citing `guide:records#GD-RECORDS-ONE-FILE` in a code span and
//!   as `[[guide:records#GD-RECORDS-ONE-FILE]]` gets no finding its twin
//!   without the citations lacks, and makes no edge (a link to it is
//!   `skipped`, never followed), in spec-a and in a twin declaring its own
//!   `GD` prefix (where the same ID written bare dangles); `spec
//!   init` and `spec init --pack` (both presets) still write one file.
//!   M (named): guide pages added to the project's walk (the golden's
//!   `tree`, `search`, `library` and `check` rows turn red).
//! - GU11 `gu11_*`: `spec bundle` and `spec show` on spec-a equal the base
//!   commit's ([`GOLDEN`]); no `bundle`, `show`, `card show --context` or
//!   `task show` output holds guide text (a rule ID, a page's title or
//!   summary). The card's context is CC1's `spec bundle` document (MCP
//!   `card_context.rs`), and `get_card {context}` and `get_task` are the
//!   CLI's reads by MCP parity, so the CLI's reads stand for the tools.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;

#[path = "guide_source/mod.rs"]
mod guide_source;

use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};

use common::git::Sandbox;
use common::{Run, SPEC, Scratch, program_with, snapshot, spec, spec_with, write};
use guide_source::{guide_text_in, source_pages};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use specengine_store::b3_hash;

/// The golden of the reads before the slice, beside this file.
const GOLDEN: &str = "guide_before.json";

/// The binary the golden was recorded from.
const RECORDED_WITH: &str = "spec at 8015bdf8 (release 0.48.1, before any src change of \
                             specengine-guide; target.noindex/debug/spec built at that commit)";

/// GU4's reads, then GU11's: each plain and with `--json`, `--root <copy>`
/// last.
const READS: [&[&str]; 14] = [
    &["check"],
    &["map"],
    &["library"],
    &["tree"],
    &["search", "guide"],
    &["search", "proposal", "door"],
    &["search", "stamina"],
    &["bundle", "MEC-STAMINA"],
    &[
        "bundle",
        "MEC-STAMINA",
        "MEC-SPRINT",
        "R-12",
        "--budget",
        "3000",
    ],
    &["bundle", "DOM-GAME", "--budget", "100000"],
    &["show", "MEC-STAMINA"],
    &["show", "MEC-STAMINA", "--links"],
    &["show", "DOM-GAME"],
    &["show", "docs/features/stamina-tuning.md"],
];

// ------------------------------------------------------------------ the golden

/// One read, as the golden holds it.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
struct Row {
    args: Vec<String>,
    /// The plain read's exit, then the JSON read's.
    code: String,
    /// b3 of the text stdout, of the JSON stdout and of the text stderr,
    /// each with the scratch's paths as placeholders.
    text: String,
    json: String,
    stderr: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct Golden {
    recorded_with: String,
    procedure: String,
    rows: Vec<Row>,
}

fn golden_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join(GOLDEN)
}

/// A copy of spec-a, a `HOME` and an empty working directory.
struct Copy {
    _scratch: Scratch,
    root: PathBuf,
    home: PathBuf,
    cwd: PathBuf,
}

impl Copy {
    fn new(label: &str) -> Self {
        let scratch = Scratch::new(label);
        let root = scratch.copy("spec-a", "spec-a");
        let home = scratch.home("h");
        let cwd = scratch.dir("cwd");
        Self {
            _scratch: scratch,
            root,
            home,
            cwd,
        }
    }

    /// `program args --root <copy>` in the working directory.
    fn run(&self, program: &OsStr, args: &[&str]) -> Run {
        let root = self.root.to_str().expect("a UTF-8 root");
        let mut all: Vec<&str> = args.to_vec();
        all.extend(["--root", root]);
        program_with(program, &self.cwd, &all, &[("HOME", self.home.as_os_str())])
    }

    fn normalised(&self, text: &str) -> String {
        text.replace(self.root.to_str().expect("UTF-8"), "<root>")
            .replace(self.home.to_str().expect("UTF-8"), "<home>")
            .replace(self.cwd.to_str().expect("UTF-8"), "<cwd>")
    }
}

/// Every read of [`READS`] by `program` on one fresh copy: its rows, and
/// the outputs themselves (plain stdout, JSON stdout) for the guide-text
/// scan.
fn play(program: &OsStr, label: &str) -> (Vec<Row>, Vec<String>) {
    let copy = Copy::new(label);
    let mut rows = Vec::new();
    let mut outputs = Vec::new();
    for args in READS {
        let plain = copy.run(program, args);
        let mut asked = args.to_vec();
        asked.push("--json");
        let json = copy.run(program, &asked);
        let hash = |text: &str| b3_hash(copy.normalised(text).as_bytes());
        rows.push(Row {
            args: args.iter().map(|arg| (*arg).to_owned()).collect(),
            code: format!("{} {}", plain.code, json.code),
            text: hash(&plain.stdout),
            json: hash(&json.stdout),
            stderr: hash(&plain.stderr),
        });
        outputs.push(plain.stdout);
        outputs.push(json.stdout);
    }
    (rows, outputs)
}

fn golden() -> Golden {
    let path = golden_path();
    serde_json::from_slice(&fs::read(&path).unwrap_or_else(|error| {
        panic!(
            "{}: {error}; record it once from the base binary: \
                 SPECENGINE_BEFORE_SPEC=<8015bdf8's spec> cargo nextest run -p specengine-cli \
                 --test guide_project record_the_reads_before --run-ignored only",
            path.display()
        )
    }))
    .expect("the golden is JSON")
}

/// GU4 and GU11: every read of the golden, played now, is the base
/// commit's byte for byte (exits, text, JSON, stderr). M (named): the guide
/// pages added to the project's walk.
#[test]
fn gu4_gu11_spec_a_reads_are_the_bytes_before() {
    let golden = golden();
    assert_eq!(golden.recorded_with, RECORDED_WITH);
    let (now, _) = play(OsStr::new(SPEC), "gu4-now");
    let want: Vec<&Vec<String>> = golden.rows.iter().map(|row| &row.args).collect();
    let got: Vec<&Vec<String>> = now.iter().map(|row| &row.args).collect();
    assert_eq!(got, want, "the golden holds every read, in order");
    let differ: Vec<String> = golden
        .rows
        .iter()
        .zip(&now)
        .filter(|(before, after)| before != after)
        .map(|(before, after)| format!("{before:?}\n  now {after:?}"))
        .collect();
    assert!(
        differ.is_empty(),
        "{} of {} reads differ from the base commit's:\n{}",
        differ.len(),
        now.len(),
        differ.join("\n")
    );
}

/// Writes [`GOLDEN`] from the binary `SPECENGINE_BEFORE_SPEC` names
/// (8015bdf8's `spec`). Run once, by hand: `SPECENGINE_BEFORE_SPEC=<binary>
/// cargo nextest run -p specengine-cli --test guide_project
/// record_the_reads_before --run-ignored only`. Every read is played twice
/// and must answer alike. It refuses when the golden exists.
#[test]
#[ignore = "manual: records the guide's golden from 8015bdf8's binary, once"]
fn record_the_reads_before_the_guide() {
    let program = std::env::var_os("SPECENGINE_BEFORE_SPEC").expect("SPECENGINE_BEFORE_SPEC");
    let path = golden_path();
    assert!(
        !path.exists(),
        "{GOLDEN} exists: it is recorded once, never again"
    );
    let (rows, _) = play(&program, "gu4-record-1");
    let (again, _) = play(&program, "gu4-record-2");
    assert_eq!(rows, again, "the binary before answers alike twice");
    let golden = Golden {
        recorded_with: RECORDED_WITH.to_owned(),
        procedure: "one fresh copy of fixtures/spec-a, one fresh HOME, an empty working \
                    directory; each read plain and with --json, `--root <copy>` last; b3 of \
                    the text stdout, of the JSON stdout, of the text stderr; scratch paths as \
                    <root>, <home>, <cwd>"
            .to_owned(),
        rows,
    };
    let mut text = format!(
        "{{\"recorded_with\":{},\"procedure\":{},\"rows\":[\n",
        serde_json::to_string(&golden.recorded_with).expect("JSON"),
        serde_json::to_string(&golden.procedure).expect("JSON")
    );
    let rows: Vec<String> = golden
        .rows
        .iter()
        .map(|row| serde_json::to_string(row).expect("JSON"))
        .collect();
    text.push_str(&rows.join(",\n"));
    text.push_str("\n]}\n");
    fs::write(&path, text).expect("write the golden");
}

/// GU11 (and GU4's reads): no read of the golden holds guide text: no rule
/// ID, no page title, no summary.
#[test]
fn gu11_no_read_holds_guide_text() {
    let pages = source_pages();
    let (_, outputs) = play(OsStr::new(SPEC), "gu11-text");
    for (n, output) in outputs.iter().enumerate() {
        let found = guide_text_in(output, &pages);
        assert!(
            found.is_empty(),
            "{:?} (json: {}): guide text {found:?}",
            READS[n / 2],
            n % 2 == 1
        );
    }
}

// ------------------------------------------------------------------ cards, tasks

/// A git copy of spec-a, committed once, and `spec` with the sandbox's
/// variables.
struct Repo {
    _scratch: Scratch,
    root: PathBuf,
    vars: Vec<(OsString, OsString)>,
}

impl Repo {
    fn new(label: &str) -> Self {
        let scratch = Scratch::new(label);
        let root = scratch.copy("spec-a", "spec-a");
        let git = Sandbox::new(scratch.path());
        git.init(&root);
        git.add_all(&root);
        git.commit(&root, "fixture");
        Self {
            _scratch: scratch,
            root,
            vars: git.vars(),
        }
    }

    fn spec(&self, args: &[&str]) -> Run {
        let env: Vec<(&str, &OsStr)> = self
            .vars
            .iter()
            .map(|(name, value)| (name.to_str().expect("a UTF-8 name"), value.as_os_str()))
            .collect();
        spec_with(&self.root, args, &env)
    }
}

/// GU11: a card on spec-a's `MEC-STAMINA` and a task on it: `card show
/// --context` and `task show`, text and JSON, hold no guide text (the
/// guide is never loaded into a context unasked).
#[test]
fn gu11_a_cards_context_and_a_task_hold_no_guide_text() {
    let pages = source_pages();
    let repo = Repo::new("gu11-card");
    let card = repo.spec(&[
        "--json",
        "card",
        "new",
        "--title",
        "Stamina pacing",
        "--target",
        "MEC-STAMINA",
    ]);
    card.code(0);
    let card_id = card.json()["id"].as_str().expect("a card ID").to_owned();
    let task = repo.spec(&[
        "--json",
        "task",
        "new",
        "--nodes",
        "MEC-STAMINA",
        "--title",
        "Stamina pacing",
    ]);
    task.code(0);
    let task_id = task.json()["id"].as_str().expect("a task ID").to_owned();
    for args in [
        vec!["card", "show", card_id.as_str(), "--context"],
        vec!["card", "show", card_id.as_str(), "--context", "--json"],
        vec!["task", "show", task_id.as_str()],
        vec!["task", "show", task_id.as_str(), "--json"],
    ] {
        let run = repo.spec(&args);
        run.code(0);
        let found = guide_text_in(&run.stdout, &pages);
        assert!(found.is_empty(), "{args:?}: guide text {found:?}");
    }
}

// ------------------------------------------------------------------ citations

/// A spec document of spec-a whose one prose line is `line`.
fn citing_document(line: &str) -> String {
    format!(
        "---\nclass: spec\nstatus: draft\nscope: [docs/spec]\n---\n\n# Lamp notes\n\n{line}\n\n\
         More notes.\n"
    )
}

/// `spec check --json` on `root` under `home`: exit 0 or 1, its findings.
fn findings(home: &Path, root: &Path) -> (i32, Vec<Value>, String) {
    let run = spec(home, root, &["check", "--json"]);
    assert!(run.code == 0 || run.code == 1, "{}", run.show());
    let findings = run.json()["findings"]
        .as_array()
        .cloned()
        .expect("findings");
    let text = spec(home, root, &["check"]);
    (run.code, findings, text.stdout)
}

/// GU4: a document citing `guide:records#GD-RECORDS-ONE-FILE` in a code
/// span, bare, and as `[[guide:records#GD-RECORDS-ONE-FILE]]` gets the
/// findings of its twin whose line cites nothing (no new finding, the same
/// exit and text), and `show --links` follows no edge for it (plain text, or
/// a link `skipped` as a `project:` reference is). M: the form read as an
/// ID or a path.
#[test]
fn gu4_guide_citations_raise_no_finding() {
    let scratch = Scratch::new("gu4-cite");
    let plain = scratch.copy("spec-a", "plain");
    let citing = scratch.copy("spec-a", "citing");
    let path = "docs/spec/lamp-notes.md";
    write(
        &plain,
        path,
        citing_document("Where a document goes: see the rule on one file a place."),
    );
    write(
        &citing,
        path,
        citing_document(
            "Where a document goes: `guide:records#GD-RECORDS-ONE-FILE`, \
             guide:records#GD-RECORDS-ONE-FILE and [[guide:records#GD-RECORDS-ONE-FILE]].",
        ),
    );
    let (plain_code, plain_findings, plain_text) = findings(&scratch.home("h1"), &plain);
    let (citing_code, citing_findings, citing_text) = findings(&scratch.home("h2"), &citing);
    assert_eq!(citing_code, plain_code, "the same exit");
    assert_eq!(
        citing_findings, plain_findings,
        "no finding the citations add (M: read as an ID or a path)"
    );
    assert_eq!(citing_text, plain_text, "the same text report");
    for finding in &citing_findings {
        let subject = finding["subject"].as_str().unwrap_or_default();
        assert!(
            !subject.contains("guide:") && !subject.contains("GD-"),
            "a finding on the citation: {finding}"
        );
    }
    assert_citations_skipped(&citation_links(&scratch.home("h3"), &citing, path));
}

/// The outgoing links of `path`'s nodes under `root` whose JSON names the
/// guide's citation (`guide:` or a `GD-` ID).
fn citation_links(home: &Path, root: &Path, path: &str) -> Vec<Value> {
    let shown = spec(home, root, &["show", path, "--links", "--json"]);
    shown.code(0);
    let document = shown.json();
    document["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .flat_map(|node| node["links"]["outgoing"].as_array().into_iter().flatten())
        .filter(|link| link.to_string().contains("guide:") || link.to_string().contains("GD-"))
        .cloned()
        .collect()
}

/// A guide citation makes no edge: it is plain text, or a link `skipped`
/// as a `project:` reference is, never followed (`resolved`), never
/// `dangling` or `unchecked`.
#[track_caller]
fn assert_citations_skipped(links: &[Value]) {
    for link in links {
        assert_eq!(link["state"], "skipped", "a citation followed: {link}");
    }
}

/// The spec-a config with a prefix of its own named `GD`, as a consumer
/// project may declare (its `[ids]` are its own; ADR-0008).
fn with_own_gd(root: &Path) {
    let config = common::read_text(root, "specengine.toml");
    write(
        root,
        "specengine.toml",
        common::with_ids_line(
            &config,
            "GD   = { kind = \"lamp-guard\", shape = \"name\" }\n",
        ),
    );
}

/// A spec document defining the project's own `GD-LAMP-GUARD`, its prose
/// line `line`.
fn own_gd_document(line: &str) -> String {
    format!(
        "---\nclass: spec\nstatus: draft\nscope: [docs/spec]\n---\n\n# Lamp notes\n\n{line}\n\n\
         ## Lamp guard {{#GD-LAMP-GUARD}}\n\nThe lamp stays lit.\n"
    )
}

/// GU4 with the project's own `GD` prefix (docs/features/specengine-guide.md
/// "Rules and edge cases": a `[[guide:…]]` is skipped as a `project:`
/// reference is): the three citation forms of `guide:records#GD-RECORDS-ONE-FILE`
/// raise no finding the twin without them lacks and make no edge, while
/// the same ID written bare in a third twin dangles (the prefix is live, so
/// the silence is the skip's). M: the skip removed (the citation read as the
/// project's `GD-RECORDS-ONE-FILE`, a dangling mention).
#[test]
fn gu4_a_project_with_its_own_gd_prefix_skips_the_guides_citations() {
    let scratch = Scratch::new("gu4-own-gd");
    let path = "docs/spec/lamp-notes.md";
    let twin = |dir: &str, line: &str| {
        let root = scratch.copy("spec-a", dir);
        with_own_gd(&root);
        write(&root, path, own_gd_document(line));
        root
    };
    let plain = twin(
        "plain",
        "Where a document goes: see the rule on one file a place.",
    );
    let citing = twin(
        "citing",
        "Where a document goes: `guide:records#GD-RECORDS-ONE-FILE`, \
         guide:records#GD-RECORDS-ONE-FILE and [[guide:records#GD-RECORDS-ONE-FILE]].",
    );
    let bare = twin(
        "bare",
        "Where a document goes: GD-RECORDS-ONE-FILE and [[GD-RECORDS-ONE-FILE]].",
    );
    let (plain_code, plain_findings, plain_text) = findings(&scratch.home("h1"), &plain);
    let (citing_code, citing_findings, citing_text) = findings(&scratch.home("h2"), &citing);
    let (_, bare_findings, _) = findings(&scratch.home("h3"), &bare);
    let names_the_id = |finding: &Value| finding.to_string().contains("GD-RECORDS-ONE-FILE");
    assert!(
        bare_findings.iter().any(names_the_id),
        "premise: the project's own `GD` is live, the bare ID dangles: {bare_findings:#?}"
    );
    assert!(
        plain_findings.iter().all(|finding| !names_the_id(finding)),
        "premise: the plain twin names no such ID: {plain_findings:#?}"
    );
    assert_eq!(citing_code, plain_code, "the same exit");
    assert_eq!(
        citing_findings, plain_findings,
        "no finding the citations add (M: the skip removed)"
    );
    assert_eq!(citing_text, plain_text, "the same text report");
    let links = citation_links(&scratch.home("h4"), &citing, path);
    assert!(
        links
            .iter()
            .all(|link| !link.to_string().contains("GD-LAMP-GUARD")),
        "the project's own node is no citation: {links:?}"
    );
    assert_citations_skipped(&links);
}

/// GU4: `spec init` and `spec init --pack` with each preset write exactly
/// one file, `specengine.toml`, holding no guide text, and no data
/// directory. M: `spec init` copying the guide.
#[test]
fn gu4_init_still_writes_one_file() {
    let pages = source_pages();
    let presets = guide_source::repository_root()
        .join("scripts")
        .join("presets");
    let scratch = Scratch::new("gu4-init");
    let mut packs: Vec<Option<PathBuf>> = vec![None];
    for name in ["gamedev.toml", "software.toml"] {
        packs.push(Some(presets.join(name)));
    }
    for (n, pack) in packs.iter().enumerate() {
        let home = scratch.home(&format!("h{n}"));
        let dir = scratch.dir(&format!("lamp-{n}"));
        let mut args = vec!["init"];
        let given;
        if let Some(pack) = pack {
            given = pack.to_str().expect("a UTF-8 path").to_owned();
            args.extend(["--pack", given.as_str()]);
        }
        let run = spec(&home, &dir, &args);
        run.code(0);
        let entries: Vec<String> = snapshot(&dir).into_keys().collect();
        assert_eq!(entries, ["specengine.toml"], "{args:?}: one file");
        assert!(snapshot(&home).is_empty(), "{args:?}: no data directory");
        let text = fs::read_to_string(dir.join("specengine.toml")).expect("the config");
        let found = guide_text_in(&text, &pages);
        assert!(found.is_empty(), "{args:?}: guide text {found:?}");
    }
}
