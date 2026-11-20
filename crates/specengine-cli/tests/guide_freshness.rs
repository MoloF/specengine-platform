//! docs/features/specengine-guide.md (RM-0129) "Acceptance criteria" GU8,
//! the CLI's half: front-matter `summarises` and the warning
//! `summary-stale`, through the `spec` binary over a scratch project made
//! here (a pottery studio's kiln notes: canon pages, one page summarising
//! them, a spec document), `[check] mode = "enforce"`, its own `HOME`.
//!
//! - `summarises` is typed (no `unknown-key`; a bare string or a list
//!   holding a number is `frontmatter-type`, subject `summarises`, at its
//!   line) and never counted: the page's `norm_hash` and `rev`, and its
//!   section's, are equal with and without the key.
//! - A summarised canon page reviewed one day after the page gives one
//!   `summary-stale` warning at the key's line, subject `summarises`, "Data"'s
//!   second message; on the same day or earlier, none (M, named: `>=` for
//!   `>`); an entry naming no walked canon document (a missing file, a
//!   spec-class document, a path with no `.md`, a missing anchor) gives the
//!   first message; a present anchor is judged by its page's date; one
//!   finding per entry; an unreadable date gives no second message.
//! - `spec check`'s exit is unchanged in `enforce`: 0 with the warnings as
//!   without them (M, named: raising the finding to an error).
//!
//! `TYPED_KEYS` 34 and `CHECK_CODES` 41 are core `guide_keys.rs`; `INDEX_FORMAT`
//! 21 (since docs/features/release-tail.md) the store's `format.rs` and this
//! crate's `bundle_config.rs`.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;

use std::path::{Path, PathBuf};

use common::{Run, Scratch, read_text, spec, write};
use serde_json::Value;

/// The page that summarises, root-relative.
const PAGE: &str = "docs/guide/firing.md";
/// The page's own `reviewed:`.
const PAGE_DATE: &str = "2026-11-20";
const DAY_AFTER: &str = "2026-11-21";
const DAY_BEFORE: &str = "2026-11-19";

const CONFIG: &str = "[project]\nname = \"kiln-notes\"\nslug = \"kiln-notes\"\n\n\
                      [paths]\nroots = [\"docs\"]\n\n[ids]\n\
                      FIRE = { kind = \"firing\", shape = \"name\" }\n\
                      GLZ = { kind = \"glaze\", shape = \"name\" }\n\n[check]\nmode = \"enforce\"\n";

/// A canon page reviewed on `date`, with an anchored part.
fn canon(title: &str, date: &str) -> String {
    format!(
        "---\nclass: canon\nowner: potter\nreviewed: {date}\n---\n\n# {title}\n\nHow it works now.\n\n\
         <a id=\"the-rule\"></a>\n## The rule\n\nCone six, slow cool.\n"
    )
}

/// The summarising page with `summarises` written as `line` (empty: no
/// key) and its own `reviewed:` `date`.
fn summarising(line: &str, date: &str) -> String {
    let key = if line.is_empty() {
        String::new()
    } else {
        format!("{line}\n")
    };
    format!(
        "---\nid: FIRE-COOLING\nclass: canon\nowner: potter\nreviewed: {date}\n{key}---\n\n\
         # Cooling a firing\n\nHow a kiln cools after a glaze firing.\n\n\
         ## Ramp down {{#FIRE-RAMP}}\n\nOne hundred degrees an hour to five hundred.\n"
    )
}

/// The line of `summarises:` in [`summarising`]'s page (the key follows
/// `---`, `id`, `class`, `owner`, `reviewed`).
const KEY_LINE: u64 = 6;

/// A scratch kiln-notes project.
struct Kiln {
    _scratch: Scratch,
    root: PathBuf,
    home: PathBuf,
}

impl Kiln {
    /// The project with `glazes.md` reviewed `glazes`, `kilns.md` on the
    /// page's day, and the page's `summarises` line `line`.
    fn new(label: &str, glazes: &str, line: &str) -> Self {
        let scratch = Scratch::new(label);
        let root = scratch.dir("kiln-notes");
        write(&root, "specengine.toml", CONFIG);
        write(&root, "docs/canon/glazes.md", canon("Glazes", glazes));
        write(&root, "docs/canon/kilns.md", canon("Kilns", PAGE_DATE));
        write(
            &root,
            "docs/spec/plan.md",
            "---\nclass: spec\nstatus: draft\nscope: [docs/spec]\n---\n\n# Next firing plan\n\n\
             Fire on Friday.\n",
        );
        write(&root, PAGE, summarising(line, PAGE_DATE));
        let home = scratch.home("h");
        Self {
            _scratch: scratch,
            root,
            home,
        }
    }

    fn spec(&self, args: &[&str]) -> Run {
        spec(&self.home, &self.root, args)
    }

    /// `spec check --json`: its exit and document.
    fn check(&self) -> (i32, Value) {
        let run = self.spec(&["check", "--json"]);
        assert!(run.code == 0 || run.code == 1, "{}", run.show());
        (run.code, run.json())
    }

    fn write(&self, relative: &str, text: &str) {
        write(&self.root, relative, text);
    }

    fn root(&self) -> &Path {
        &self.root
    }
}

/// The findings of `code` in `document`.
fn of(document: &Value, code: &str) -> Vec<Value> {
    document["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .filter(|finding| finding["code"] == code)
        .cloned()
        .collect()
}

/// The messages of the `summary-stale` findings.
fn stale_messages(document: &Value) -> Vec<String> {
    of(document, "summary-stale")
        .iter()
        .map(|finding| finding["message"].as_str().expect("a message").to_owned())
        .collect()
}

fn later_message(entry: &str, date: &str) -> String {
    format!("summarises `{entry}`: reviewed {date}, after this page's {PAGE_DATE}")
}

fn missing_message(entry: &str) -> String {
    format!("summarises `{entry}`: names no walked canon document")
}

/// The premise every case stands on: with no `summarises`, the project
/// checks clean in `enforce`.
#[test]
fn gu8_premise_the_kiln_notes_check_clean() {
    let kiln = Kiln::new("gu8-premise", PAGE_DATE, "");
    let (code, document) = kiln.check();
    assert_eq!(code, 0, "{document}");
    assert_eq!(
        document["findings"].as_array().map(Vec::len),
        Some(0),
        "{document}"
    );
}

/// GU8: the summarised page reviewed a day after the page: one
/// `summary-stale`, a warning, at the key's line, subject `summarises`,
/// "Data"'s second message; exit 0 in `enforce`, the text report naming
/// it. M (named): raising it to an error (exit 1, `error`).
#[test]
fn gu8_a_day_later_gives_one_summary_stale_warning() {
    let kiln = Kiln::new(
        "gu8-later",
        DAY_AFTER,
        "summarises: [docs/canon/glazes.md, docs/canon/kilns.md]",
    );
    let (code, document) = kiln.check();
    let stale = of(&document, "summary-stale");
    assert_eq!(stale.len(), 1, "{document}");
    let finding = &stale[0];
    assert_eq!(finding["severity"], "warning", "M: an error: {finding}");
    assert_eq!(finding["path"], PAGE);
    assert_eq!(finding["line"], KEY_LINE, "the key's line: {finding}");
    assert_eq!(finding["subject"], "summarises");
    assert_eq!(
        finding["message"],
        later_message("docs/canon/glazes.md", DAY_AFTER)
    );
    assert_eq!(code, 0, "the exit is unchanged in enforce: {document}");
    assert_eq!(
        document["findings"].as_array().map(Vec::len),
        Some(1),
        "nothing else: {document}"
    );
    let text = kiln.spec(&["check"]);
    text.code(0);
    assert!(
        text.stdout.contains("0 errors, 1 warnings") && text.stdout.contains("\u{2014} clean"),
        "the text summary: one warning, clean: {}",
        text.show()
    );
    let listed = kiln.spec(&["check", "--debt"]);
    listed.code(0);
    assert!(
        listed.stdout.contains("summary-stale"),
        "the full list names it: {}",
        listed.show()
    );
}

/// GU8: the same day, and the day before: no `summary-stale` (strictly
/// later only). M (named): `>=` for `>`.
#[test]
fn gu8_the_same_day_or_earlier_gives_none() {
    for (label, date) in [("gu8-same", PAGE_DATE), ("gu8-before", DAY_BEFORE)] {
        let kiln = Kiln::new(
            label,
            date,
            "summarises: [docs/canon/glazes.md, docs/canon/glazes.md#the-rule]",
        );
        let (code, document) = kiln.check();
        assert_eq!(
            stale_messages(&document),
            Vec::<String>::new(),
            "{label}: M: >= for >"
        );
        assert_eq!(code, 0, "{document}");
    }
}

/// GU8: an entry naming no walked canon document gives the first message:
/// a missing file, a spec-class document, a path without `.md`, a missing
/// anchor; a present anchor is judged by its page's date; one finding per
/// entry, each at the key's line.
#[test]
fn gu8_each_entry_is_judged_once() {
    let entries = [
        "docs/canon/nope.md",
        "docs/spec/plan.md",
        "docs/canon/glazes",
        "docs/canon/glazes.md#nope",
        "docs/canon/glazes.md#the-rule",
        "docs/canon/kilns.md",
        "docs/canon/kilns.md#the-rule",
    ];
    let line = format!("summarises: [{}]", entries.join(", "));
    let kiln = Kiln::new("gu8-entries", DAY_AFTER, &line);
    let (code, document) = kiln.check();
    let mut got = stale_messages(&document);
    got.sort();
    let mut want = vec![
        missing_message("docs/canon/nope.md"),
        missing_message("docs/spec/plan.md"),
        missing_message("docs/canon/glazes"),
        missing_message("docs/canon/glazes.md#nope"),
        later_message("docs/canon/glazes.md#the-rule", DAY_AFTER),
    ];
    want.sort();
    assert_eq!(got, want, "{document}");
    for finding in of(&document, "summary-stale") {
        assert_eq!(finding["line"], KEY_LINE, "{finding}");
        assert_eq!(finding["subject"], "summarises", "{finding}");
        assert_eq!(finding["severity"], "warning", "{finding}");
    }
    assert_eq!(code, 0, "warnings alone: {document}");
}

/// GU8: a block list reads as the flow list does.
#[test]
fn gu8_a_block_list_is_read_alike() {
    let kiln = Kiln::new(
        "gu8-block",
        DAY_AFTER,
        "summarises:\n  - docs/canon/glazes.md\n  - docs/canon/kilns.md",
    );
    let (_, document) = kiln.check();
    assert_eq!(
        stale_messages(&document),
        [later_message("docs/canon/glazes.md", DAY_AFTER)],
        "{document}"
    );
}

/// GU8: "only when both dates read as `YYYY-MM-DD`": a summarised page
/// whose `reviewed:` is no date, or a page whose own is none, gives no
/// second message.
#[test]
fn gu8_an_unreadable_date_gives_no_second_message() {
    let kiln = Kiln::new("gu8-undated", "soon", "summarises: [docs/canon/glazes.md]");
    let (_, document) = kiln.check();
    assert!(
        stale_messages(&document)
            .iter()
            .all(|message| !message.contains("after this page's")),
        "{document}"
    );
    let own = Kiln::new("gu8-own-undated", DAY_AFTER, "");
    own.write(
        PAGE,
        &summarising("summarises: [docs/canon/glazes.md]", PAGE_DATE)
            .replace(&format!("reviewed: {PAGE_DATE}"), "reviewed: later"),
    );
    let (_, document) = own.check();
    assert!(
        stale_messages(&document)
            .iter()
            .all(|message| !message.contains("after this page's")),
        "{document}"
    );
}

/// GU8, "a mistyped value gives `frontmatter-type`": a bare string and a
/// list holding a number, each `frontmatter-type` subject `summarises` at
/// the key's line, and no `summary-stale` from a mistyped value; a
/// well-typed key is no `unknown-key`.
#[test]
fn gu8_summarises_is_typed() {
    let typed = Kiln::new("gu8-typed", PAGE_DATE, "summarises: [docs/canon/glazes.md]");
    let (_, document) = typed.check();
    assert!(
        of(&document, "unknown-key")
            .iter()
            .all(|finding| finding["subject"] != "summarises"),
        "M: untyped: {document}"
    );
    for wrong in [
        "summarises: docs/canon/glazes.md",
        "summarises: [docs/canon/glazes.md, 3]",
        "summarises: {a: docs/canon/glazes.md}",
    ] {
        let kiln = Kiln::new("gu8-mistyped", DAY_AFTER, wrong);
        let (_, document) = kiln.check();
        let mistyped: Vec<Value> = of(&document, "frontmatter-type")
            .into_iter()
            .filter(|finding| finding["subject"] == "summarises")
            .collect();
        assert_eq!(mistyped.len(), 1, "{wrong}: {document}");
        assert_eq!(mistyped[0]["path"], PAGE, "{wrong}");
        assert_eq!(mistyped[0]["line"], KEY_LINE, "{wrong}");
    }
}

/// GU8, "never counted": the page's and its section's `norm_hash` and
/// `rev` are equal with and without `summarises`, and with another entry.
/// M: `summarises` counted in `norm_input`.
#[test]
fn gu8_summarises_is_never_counted() {
    let lines = [
        "",
        "summarises: [docs/canon/glazes.md]",
        "summarises: [docs/canon/glazes.md, docs/canon/kilns.md#the-rule]",
    ];
    let mut seen: Vec<Vec<(Value, Value)>> = Vec::new();
    for (n, line) in lines.iter().enumerate() {
        let kiln = Kiln::new(&format!("gu8-count-{n}"), PAGE_DATE, line);
        assert!(
            read_text(kiln.root(), PAGE).contains(line),
            "premise: the key written"
        );
        let mut nodes = Vec::new();
        for id in ["FIRE-COOLING", "FIRE-RAMP"] {
            let run = kiln.spec(&["show", id, "--json"]);
            run.code(0);
            let node = run.json()["nodes"][0].clone();
            assert!(node["norm_hash"].is_string(), "premise: hashed: {node}");
            nodes.push((node["norm_hash"].clone(), node["rev"].clone()));
        }
        seen.push(nodes);
    }
    assert_eq!(seen[0], seen[1], "M: summarises counted");
    assert_eq!(seen[0], seen[2], "M: summarises counted");
}

/// GU8, a write's half (`write-cost`'s affected set): in a git copy of the
/// kiln notes whose page summarises `glazes.md` reviewed the same day, a
/// dry-run update of `glazes.md` moving its `reviewed:` a day later lists,
/// among the findings it would introduce, `summary-stale` on the
/// summarising page, at the key's line. M: `summarised_reads` dropped (the
/// summarising page left out of the change's affected set).
#[test]
fn gu8_a_proposal_moving_a_summarised_review_names_the_stale_summary() {
    let kiln = Kiln::new(
        "gu8-propose",
        PAGE_DATE,
        "summarises: [docs/canon/glazes.md]",
    );
    let git = common::git::Sandbox::new(kiln._scratch.path());
    git.init(&kiln.root);
    git.add_all(&kiln.root);
    git.commit(&kiln.root, "kiln notes");
    let vars: Vec<(String, std::ffi::OsString)> = git
        .vars()
        .into_iter()
        .filter(|(name, _)| name != "HOME")
        .map(|(name, value)| (name.to_string_lossy().into_owned(), value))
        .chain([("HOME".to_owned(), kiln.home.clone().into_os_string())])
        .collect();
    let env: Vec<(&str, &std::ffi::OsStr)> = vars
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_os_str()))
        .collect();
    let run = |args: &[&str]| common::spec_with(&kiln.root, args, &env);
    let (code, document) = kiln.check();
    assert_eq!(
        (code, stale_messages(&document)),
        (0, Vec::new()),
        "premise"
    );
    let shown = run(&["show", "docs/canon/glazes.md", "--json"]);
    shown.code(0);
    let node = shown.json()["nodes"][0].clone();
    let base = node["span_hash"].as_str().expect("a span hash").to_owned();
    let text = node["text"].as_str().expect("a text").to_owned();
    let from = format!("reviewed: {PAGE_DATE}");
    assert!(
        text.contains(&from),
        "premise: the span holds `{from}`: {text:?}"
    );
    let draft = kiln._scratch.join("glazes-draft.md");
    std::fs::write(
        &draft,
        text.replacen(&from, &format!("reviewed: {DAY_AFTER}"), 1),
    )
    .expect("the draft");
    let proposed = run(&[
        "propose",
        "update",
        "docs/canon/glazes.md",
        "--base",
        &base,
        "--text-file",
        draft.to_str().expect("a UTF-8 path"),
        "--rationale",
        "Glazes reviewed again",
        "--dry-run",
        "--json",
    ]);
    proposed.code(0);
    let document = proposed.json();
    let stale: Vec<&Value> = document["diagnostics"]
        .as_array()
        .unwrap_or_else(|| panic!("diagnostics: {document}"))
        .iter()
        .filter(|finding| finding["code"] == "summary-stale")
        .collect();
    assert_eq!(stale.len(), 1, "M: summarised_reads dropped: {document}");
    assert_eq!(stale[0]["path"], PAGE, "{document}");
    assert_eq!(stale[0]["line"], KEY_LINE, "{document}");
    assert_eq!(stale[0]["subject"], "summarises");
    assert_eq!(
        stale[0]["message"],
        later_message("docs/canon/glazes.md", DAY_AFTER)
    );
}
