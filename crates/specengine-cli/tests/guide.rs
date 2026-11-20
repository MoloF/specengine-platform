//! docs/features/specengine-guide.md (RM-0129) "Acceptance criteria", the
//! CLI's guide itself: the `spec` binary of this build run in scratch
//! directories under scratch `HOME`s (or none), compared with the pages of
//! `docs/guide/` as `guide_source` reads them by the spec's "Data"; the
//! repository is only read.
//!
//! - GU1 `gu1_*`: `spec guide` lists the ten topics in `GUIDE_ORDER` with
//!   titles, summaries and rule counts, exit 0; every topic prints its page
//!   without its front-matter byte for byte, `proposals` checked against
//!   the file's own bytes too; every `TOPIC#ID` prints that rule's section;
//!   an unknown topic or rule exits 1 with "Data"'s line; a REF with a space
//!   or a control character, two REFs or an unknown flag exit 2; the
//!   40 000-character cap never binds. M (named): reading `docs/guide` from
//!   disk at run time: `gu1_the_binary_reads_no_guide_file_at_run_time`
//!   runs the binary under macOS's `sandbox-exec` with every read under
//!   `docs/` denied (not a rename of the source directory: under nextest a
//!   rename races every other test that reads `docs/guide`).
//! - GU2 `gu2_*`: with `HOME` unset, a file, missing or empty, and no
//!   `specengine.toml` above, `spec guide` answers; the scratch tree, the
//!   `HOME` and the data directory are unchanged; inside a broken project
//!   and inside a real one alike, no index is built. M (named): calling
//!   `discover` (exit 2).
//! - GU3 `gu3_*`: the embed, read through `spec guide --json` and each
//!   topic's JSON, equals `docs/guide/*.md` in order, topics, titles,
//!   summaries, `summarises`, `reviewed`, rule IDs, titles and lines, and
//!   text; the comparison names a page edited in a scratch copy without a
//!   rebuild (M, named: a stale embed); `build.rs`, compiled alone and run
//!   on scratch trees, refuses a missing directory, a page with no
//!   front-matter, a topic absent from `GUIDE_ORDER` and the reverse, and a
//!   repeated rule ID, each naming the file.
//! - GU15 `gu15_*`: `spec guide --json` and every topic's text are
//!   byte-equal across two directories, two `HOME`s and with or without a
//!   project.
//!
//! GU4 and GU11 are `guide_project.rs`, GU8 `guide_freshness.rs`.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;

#[path = "guide_source/mod.rs"]
mod guide_source;

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use common::{Run, SPEC, Scratch, copy_dir, data_dir, program_with, snapshot, spec_with, write};
use guide_source::{
    GUIDE_ORDER, OUTPUT_CAP_CHARS, PAGE_CAP, Page, TITLES, forms, index_json, index_json_text,
    index_text, page_json, page_json_text, pages, rule_json, rule_json_text, source_dir,
    source_pages, unknown_rule, unknown_topic,
};
use serde_json::Value;

/// `spec args` in `cwd` with `HOME=home`, or no `HOME` at all.
fn guide_in(cwd: &Path, home: Option<&Path>, args: &[&str]) -> Run {
    match home {
        Some(home) => spec_with(cwd, args, &[("HOME", home.as_os_str())]),
        None => spec_with(cwd, args, &[]),
    }
}

/// `spec guide [reference] [--json]` in `cwd` under `home`.
fn guide(cwd: &Path, home: Option<&Path>, reference: Option<&str>, json: bool) -> Run {
    let mut args = vec!["guide"];
    args.extend(reference);
    if json {
        args.push("--json");
    }
    guide_in(cwd, home, &args)
}

/// Exit 0, no stderr, stdout `want`.
#[track_caller]
fn assert_answers(run: &Run, want: &str, context: &str) {
    run.code(0);
    assert_eq!(run.stderr, "", "{context}: {}", run.show());
    assert!(
        run.stdout == want,
        "{context}: stdout differs from the source\n--- got\n{}\n--- want\n{}",
        run.stdout,
        want
    );
}

/// Exit 0, no stderr, one JSON document equal to `want`.
#[track_caller]
fn assert_document(run: &Run, want: &Value, context: &str) {
    run.code(0);
    assert_eq!(run.stderr, "", "{context}: {}", run.show());
    assert_eq!(&run.json(), want, "{context}");
}

/// No `specengine.toml` in `dir` or any directory above it.
fn assert_no_project_above(dir: &Path) {
    for ancestor in dir.ancestors() {
        assert!(
            !ancestor.join("specengine.toml").exists(),
            "premise: {} holds a specengine.toml",
            ancestor.display()
        );
    }
}

// ------------------------------------------------------------------ GU1

/// GU1: `spec guide` in an empty directory under a scratch `HOME`: the ten
/// topics in `GUIDE_ORDER`, each with its title, summary and rule count,
/// text and JSON; `--json` before the command gives the same bytes.
#[test]
fn gu1_the_index_lists_the_ten_topics_in_order() {
    let pages = source_pages();
    let scratch = Scratch::new("gu1-index");
    let home = scratch.home("h");
    let cwd = scratch.dir("empty");
    assert_no_project_above(&cwd);
    let text = guide(&cwd, Some(&home), None, false);
    assert_answers(&text, &index_text(&pages), "spec guide");
    assert_eq!(text.stdout.lines().count(), 10, "{}", text.stdout);
    let json = guide(&cwd, Some(&home), None, true);
    assert_document(&json, &index_json(&pages), "spec guide --json");
    assert_eq!(
        json.stdout,
        index_json_text(&pages),
        "the keys in \"Data\"'s order"
    );
    let document = json.json();
    let topics: Vec<&str> = document["topics"]
        .as_array()
        .expect("topics")
        .iter()
        .map(|entry| entry["topic"].as_str().expect("a topic"))
        .collect();
    assert_eq!(topics, GUIDE_ORDER, "GUIDE_ORDER");
    let titles: Vec<&str> = document["topics"]
        .as_array()
        .expect("topics")
        .iter()
        .map(|entry| entry["title"].as_str().expect("a title"))
        .collect();
    assert_eq!(titles, TITLES, "\"Data\"'s titles");
    for entry in document["topics"].as_array().expect("topics") {
        assert!(
            entry["summary"]
                .as_str()
                .is_some_and(|text| !text.is_empty()),
            "a summary: {entry}"
        );
        assert!(
            entry["rules"].as_u64().is_some_and(|n| n >= 1),
            "a rule count of at least one: {entry}"
        );
    }
    let before = guide_in(&cwd, Some(&home), &["--json", "guide"]);
    assert_eq!(
        before.stdout, json.stdout,
        "a global --json before the command"
    );
}

/// GU1: every topic prints its page without its front-matter, byte for
/// byte, and its JSON; `proposals` against `docs/guide/proposals.md`'s own
/// bytes after its closing `---` line.
#[test]
fn gu1_each_topic_prints_its_page_without_front_matter() {
    let pages = source_pages();
    let scratch = Scratch::new("gu1-pages");
    let home = scratch.home("h");
    let cwd = scratch.dir("empty");
    for page in &pages {
        let text = guide(&cwd, Some(&home), Some(&page.topic), false);
        assert_answers(&text, &page.text, &format!("spec guide {}", page.topic));
        let json = guide(&cwd, Some(&home), Some(&page.topic), true);
        assert_document(
            &json,
            &page_json(page),
            &format!("spec guide {} --json", page.topic),
        );
        assert!(
            json.stdout == page_json_text(page),
            "{}: the keys in \"Data\"'s order: {}",
            page.topic,
            json.stdout.chars().take(300).collect::<String>()
        );
    }
    let bytes = fs::read_to_string(source_dir().join("proposals.md")).expect("proposals.md");
    let body = bytes
        .strip_prefix("---\n")
        .and_then(|rest| rest.split_once("\n---\n"))
        .map(|(_, body)| body)
        .expect("proposals.md has front-matter");
    let printed = guide(&cwd, Some(&home), Some("proposals"), false);
    assert_eq!(printed.stdout, body, "the file without its front-matter");
    assert!(
        !printed.stdout.contains("summarises:") && !printed.stdout.contains("class: canon"),
        "no front-matter key printed"
    );
}

/// GU1: every `TOPIC#ID` prints that rule's section, nested sections
/// included, and its JSON; `proposals#GD-PROPOSALS-ONE-DOOR` named.
#[test]
fn gu1_each_rule_prints_its_section() {
    let pages = source_pages();
    let scratch = Scratch::new("gu1-rules");
    let home = scratch.home("h");
    let cwd = scratch.dir("empty");
    let mut seen = 0;
    for page in &pages {
        for rule in &page.rules {
            let reference = format!("{}#{}", page.topic, rule.id);
            let text = guide(&cwd, Some(&home), Some(&reference), false);
            assert_answers(&text, &rule.text, &format!("spec guide {reference}"));
            let json = guide(&cwd, Some(&home), Some(&reference), true);
            assert_document(
                &json,
                &rule_json(page, rule),
                &format!("spec guide {reference} --json"),
            );
            assert!(
                json.stdout == rule_json_text(page, rule),
                "{reference}: the keys in \"Data\"'s order: {}",
                json.stdout
            );
            seen += 1;
        }
    }
    assert!(seen >= GUIDE_ORDER.len(), "every page holds a rule");
    let proposals = pages
        .iter()
        .find(|page| page.topic == "proposals")
        .expect("proposals");
    let door = proposals
        .rules
        .iter()
        .find(|rule| rule.id == "GD-PROPOSALS-ONE-DOOR")
        .expect("GU1's rule exists");
    let run = guide(
        &cwd,
        Some(&home),
        Some("proposals#GD-PROPOSALS-ONE-DOOR"),
        false,
    );
    assert!(
        run.stdout.starts_with(
            "### Never write a spec file by hand from an agent {#GD-PROPOSALS-ONE-DOOR}\n"
        ),
        "{}",
        run.show()
    );
    assert_eq!(run.stdout, door.text);
    assert!(
        !run.stdout.contains("{#GD-PROPOSALS-NO-AGENT-CHOICE}"),
        "the next rule is not part of the section: {}",
        run.stdout
    );
}

/// GU1: an unknown topic, an unknown rule and another topic's rule exit 1
/// with "Data"'s lines on stderr and nothing on stdout; with `--json` the
/// exit is 1 as well.
#[test]
fn gu1_an_unknown_topic_or_rule_exits_1_with_its_line() {
    let pages = source_pages();
    let proposals = pages
        .iter()
        .find(|page| page.topic == "proposals")
        .expect("proposals");
    let scratch = Scratch::new("gu1-unknown");
    let home = scratch.home("h");
    let cwd = scratch.dir("empty");
    let cases = [
        ("nope", unknown_topic("nope")),
        ("nope#GD-PROPOSALS-ONE-DOOR", unknown_topic("nope")),
        (
            "proposals#GD-PROPOSALS-NOPE",
            unknown_rule(proposals, "GD-PROPOSALS-NOPE"),
        ),
        (
            "proposals#GD-RECORDS-ONE-FILE",
            unknown_rule(proposals, "GD-RECORDS-ONE-FILE"),
        ),
    ];
    for (reference, line) in &cases {
        let run = guide(&cwd, Some(&home), Some(reference), false);
        run.code(1);
        assert_eq!(run.stdout, "", "spec guide {reference}: {}", run.show());
        assert_eq!(
            run.stderr,
            format!("{line}\n"),
            "spec guide {reference}: {}",
            run.show()
        );
        let json = guide(&cwd, Some(&home), Some(reference), true);
        json.code(1);
    }
    assert!(
        unknown_topic("nope").ends_with(
            "topics: start, records, features, proposals, cards, practices, auto, markers, hub, \
             documents"
        ),
        "the line lists GUIDE_ORDER"
    );
    assert_eq!(
        snapshot(scratch.path())
            .keys()
            .filter(|path| !path.starts_with("homes") && path.as_str() != "empty")
            .count(),
        0,
        "nothing written beside the working directory"
    );
}

/// GU1: exit 2 for usage: a REF with a space or a control character, two
/// REFs, an unknown flag; nothing on stdout.
#[test]
fn gu1_a_bad_ref_or_usage_exits_2() {
    let scratch = Scratch::new("gu1-usage");
    let home = scratch.home("h");
    let cwd = scratch.dir("empty");
    let before = snapshot(scratch.path());
    let space = "proposals #GD-PROPOSALS-ONE-DOOR";
    let tab = "proposals\t";
    let control = format!("proposals{}", char::from(1u8));
    let bell = format!("proposals#GD-PROPOSALS-ONE-DOOR{}", char::from(7u8));
    for args in [
        vec!["guide", space],
        vec!["guide", "pro posals"],
        vec!["guide", tab],
        vec!["guide", control.as_str()],
        vec!["guide", bell.as_str()],
        vec!["guide", "proposals", "records"],
        vec!["guide", "--nope"],
    ] {
        let run = guide_in(&cwd, Some(&home), &args);
        run.code(2);
        assert_eq!(run.stdout, "", "{args:?}: {}", run.show());
        assert!(!run.stderr.is_empty(), "{args:?}: a reason on stderr");
    }
    assert_eq!(snapshot(scratch.path()), before, "usage writes nothing");
}

/// GU1, "The 40 000-character cap never binds": every answer is shorter
/// than the cap and printed whole, every page within the Tier 2 cap.
#[test]
fn gu1_the_output_cap_never_binds() {
    let pages = source_pages();
    let scratch = Scratch::new("gu1-cap");
    let home = scratch.home("h");
    let cwd = scratch.dir("empty");
    for page in &pages {
        assert!(
            page.source.len() <= PAGE_CAP,
            "{}: {} B over the Tier 2 cap",
            page.topic,
            page.source.len()
        );
    }
    for form in forms(&pages) {
        assert!(
            form.text.chars().count() < OUTPUT_CAP_CHARS,
            "{:?}: the cap would bind",
            form.reference
        );
        let run = guide(&cwd, Some(&home), form.reference.as_deref(), false);
        assert_eq!(run.stdout, form.text, "{:?} printed whole", form.reference);
        let json = guide(&cwd, Some(&home), form.reference.as_deref(), true);
        assert!(
            json.stdout.chars().count() > 0 && json.json() == form.json,
            "{:?}: the JSON whole",
            form.reference
        );
    }
}

/// The `sandbox-exec` profile that denies every read under `dir`.
#[cfg(target_os = "macos")]
fn deny_reads_under(dir: &Path) -> String {
    format!(
        "(version 1)(allow default)(deny file-read* (subpath \"{}\"))",
        dir.display()
    )
}

/// GU1, M (named) "reading `docs/guide` from disk at run time": the binary
/// under `sandbox-exec` with every read under this repository's `docs/`
/// denied answers the index, a page and a rule with the source's bytes. A
/// sandboxed `cat` of a page fails first, so the profile is proven to
/// bite. M: `spec guide` reading `docs/guide/<topic>.md` at run time.
#[cfg(target_os = "macos")]
#[test]
fn gu1_the_binary_reads_no_guide_file_at_run_time() {
    let pages = source_pages();
    let docs = guide_source::repository_root().join("docs");
    let profile = deny_reads_under(&docs);
    let scratch = Scratch::new("gu1-sandbox");
    let home = scratch.home("h");
    let cwd = scratch.dir("empty");
    let sandbox = OsStr::new("/usr/bin/sandbox-exec");
    let page = source_dir().join("proposals.md");
    let page_text = page.to_str().expect("a UTF-8 path");
    let control = program_with(
        sandbox,
        &cwd,
        &["-p", &profile, "/bin/cat", page_text],
        &[("HOME", home.as_os_str())],
    );
    assert!(
        !control.stderr.contains("sandbox_apply"),
        "sandbox-exec cannot apply a profile here, so this check cannot run: {}",
        control.show()
    );
    assert_ne!(
        control.code,
        0,
        "the profile denies reading {page_text}: {}",
        control.show()
    );
    assert_eq!(control.stdout, "", "nothing of the page read");
    let rule = pages
        .iter()
        .find(|page| page.topic == "proposals")
        .and_then(|page| page.rules.first())
        .expect("a rule");
    let reference = format!("proposals#{}", rule.id);
    for (args, want) in [
        (vec!["guide"], index_text(&pages)),
        (
            vec!["guide", "proposals"],
            pages
                .iter()
                .find(|page| page.topic == "proposals")
                .expect("proposals")
                .text
                .clone(),
        ),
        (vec!["guide", reference.as_str()], rule.text.clone()),
    ] {
        let mut sandboxed = vec!["-p", profile.as_str(), SPEC];
        sandboxed.extend(args.iter().copied());
        let run = program_with(sandbox, &cwd, &sandboxed, &[("HOME", home.as_os_str())]);
        assert_answers(
            &run,
            &want,
            &format!("{args:?} with docs/ unreadable (M: read at run time)"),
        );
    }
}

// ------------------------------------------------------------------ GU2

/// The four reads GU2 asks of every environment, with their stdout.
fn gu2_reads(pages: &[Page]) -> Vec<(Vec<&'static str>, String)> {
    let proposals = pages
        .iter()
        .find(|page| page.topic == "proposals")
        .expect("proposals");
    let door = proposals
        .rules
        .iter()
        .find(|rule| rule.id == "GD-PROPOSALS-ONE-DOOR")
        .expect("the rule");
    vec![
        (vec!["guide"], index_text(pages)),
        (vec!["guide", "--json"], index_json_text(pages)),
        (vec!["guide", "proposals"], proposals.text.clone()),
        (
            vec!["guide", "proposals#GD-PROPOSALS-ONE-DOOR"],
            door.text.clone(),
        ),
    ]
}

/// GU2: `HOME` unset, a regular file, a missing path and an empty
/// directory, `XDG_DATA_HOME` missing too, no `specengine.toml` above:
/// every read answers; the scratch tree (the working directory, each
/// `HOME`, the data directory) is byte-equal after. M (named): calling
/// `discover` (exit 2).
#[test]
fn gu2_no_home_no_project_writes_nothing() {
    let pages = source_pages();
    let scratch = Scratch::new("gu2-nothing");
    let cwd = scratch.dir("empty");
    assert_no_project_above(&cwd);
    let file_home = scratch.join("home-file");
    fs::write(&file_home, b"not a directory\n").expect("a file HOME");
    let missing_home = scratch.join("missing-home");
    let empty_home = scratch.home("h");
    let missing_xdg = scratch.join("missing-xdg");
    let before = snapshot(scratch.path());
    let environments: Vec<(&str, Vec<(&str, &OsStr)>)> = vec![
        ("no HOME", vec![]),
        ("HOME a file", vec![("HOME", file_home.as_os_str())]),
        ("HOME missing", vec![("HOME", missing_home.as_os_str())]),
        ("HOME empty", vec![("HOME", empty_home.as_os_str())]),
        (
            "HOME empty, XDG_DATA_HOME missing",
            vec![
                ("HOME", empty_home.as_os_str()),
                ("XDG_DATA_HOME", missing_xdg.as_os_str()),
            ],
        ),
    ];
    for (label, env) in &environments {
        for (args, want) in gu2_reads(&pages) {
            let run = spec_with(&cwd, &args, env);
            assert_answers(&run, &want, &format!("{label}: {args:?} (M: discover)"));
        }
    }
    assert_eq!(
        snapshot(scratch.path()),
        before,
        "the working directory, every HOME and the data directory unchanged"
    );
    assert!(!data_dir(&empty_home).exists(), "no data directory");
    assert!(!missing_home.exists() && !missing_xdg.exists());
}

/// GU2, "No discovery, no index": inside a directory whose
/// `specengine.toml` is not TOML, and inside a copy of `fixtures/spec-a`,
/// `spec guide` answers the same bytes, builds no index (the data
/// directory stays absent) and leaves the tree byte-equal.
#[test]
fn gu2_a_project_around_changes_nothing() {
    let pages = source_pages();
    let scratch = Scratch::new("gu2-around");
    let broken = scratch.dir("broken");
    write(&broken, "specengine.toml", "[project\nslug = = \"x\"\n");
    write(
        &broken,
        "docs/spec/bad.md",
        "---\nclass: [unclosed\n---\n\n# Bad\n",
    );
    let project = scratch.copy("spec-a", "spec-a");
    let nested = project.join("docs").join("spec");
    for cwd in [&broken, &project, &nested] {
        let home = scratch.home(&format!(
            "h-{}",
            cwd.file_name().and_then(OsStr::to_str).unwrap_or("x")
        ));
        let before = snapshot(scratch.path());
        for (args, want) in gu2_reads(&pages) {
            let run = spec_with(cwd, &args, &[("HOME", home.as_os_str())]);
            assert_answers(&run, &want, &format!("{}: {args:?}", cwd.display()));
        }
        assert_eq!(
            snapshot(scratch.path()),
            before,
            "{}: nothing written",
            cwd.display()
        );
        assert!(
            !data_dir(&home).exists(),
            "{}: no index built (M: discover and refresh)",
            cwd.display()
        );
    }
}

// ------------------------------------------------------------------ GU3

/// How the binary's embed differs from `pages`, read through `spec guide
/// --json` and each topic's `--json`: one line per differing field, each
/// naming its topic.
fn embed_differences(pages: &[Page]) -> Vec<String> {
    let scratch = Scratch::new("gu3-embed");
    let home = scratch.home("h");
    let cwd = scratch.dir("empty");
    let mut out = Vec::new();
    let index = guide(&cwd, Some(&home), None, true);
    index.code(0);
    let listed = index.json()["topics"].clone();
    let listed = listed.as_array().cloned().unwrap_or_default();
    let topics: Vec<String> = listed
        .iter()
        .map(|entry| entry["topic"].as_str().unwrap_or("?").to_owned())
        .collect();
    let want: Vec<String> = pages.iter().map(|page| page.topic.clone()).collect();
    if topics != want {
        out.push(format!("order: {topics:?}, the source {want:?}"));
    }
    let wanted_index = index_json(pages);
    for (n, page) in pages.iter().enumerate() {
        let entry = listed.get(n).cloned().unwrap_or(Value::Null);
        for key in ["title", "summary", "rules"] {
            if entry[key] != wanted_index["topics"][n][key] {
                out.push(format!(
                    "{}: index {key} {}, the source {}",
                    page.topic, entry[key], wanted_index["topics"][n][key]
                ));
            }
        }
        let run = guide(&cwd, Some(&home), Some(&page.topic), true);
        if run.code != 0 {
            out.push(format!("{}: exit {}: {}", page.topic, run.code, run.stderr));
            continue;
        }
        let got = run.json();
        let wanted = page_json(page);
        for key in [
            "topic",
            "title",
            "summary",
            "summarises",
            "reviewed",
            "rules",
            "text",
        ] {
            if got[key] != wanted[key] {
                out.push(format!("{}: {key} differs from the source", page.topic));
            }
        }
        let keys: Vec<&String> = got
            .as_object()
            .map(|map| map.keys().collect())
            .unwrap_or_default();
        if keys.len() != 7 {
            out.push(format!("{}: keys {keys:?}", page.topic));
        }
    }
    out
}

/// GU3, the library's `GUIDE` itself against `docs/guide/*.md`: its order
/// is `GUIDE_ORDER` (the library's and "Data"'s), and each page's topic,
/// title, summary, `summarises`, `reviewed`, rule IDs, titles, lines and
/// sections, and text are the source's. M: a stale embed.
#[test]
fn gu3_guide_is_the_source_field_by_field() {
    let pages = source_pages();
    assert_eq!(specengine_cli::GUIDE_ORDER, GUIDE_ORDER, "GUIDE_ORDER");
    let embedded: Vec<&str> = specengine_cli::GUIDE
        .iter()
        .map(|page| page.topic)
        .collect();
    assert_eq!(embedded, GUIDE_ORDER, "GUIDE in GUIDE_ORDER");
    for (page, source) in specengine_cli::GUIDE.iter().zip(&pages) {
        let topic = source.topic.as_str();
        assert_eq!(page.title, source.title, "{topic}: title");
        assert_eq!(page.summary, source.summary, "{topic}: summary");
        assert_eq!(page.summarises, source.summarises, "{topic}: summarises");
        assert_eq!(page.reviewed, source.reviewed, "{topic}: reviewed");
        assert!(page.text == source.text, "{topic}: text");
        let rules: Vec<(&str, &str, usize)> = page
            .rules
            .iter()
            .map(|rule| (rule.id, rule.title, rule.line))
            .collect();
        let want: Vec<(&str, &str, usize)> = source
            .rules
            .iter()
            .map(|rule| (rule.id.as_str(), rule.title.as_str(), rule.line))
            .collect();
        assert_eq!(rules, want, "{topic}: rules");
        for (rule, wanted) in page.rules.iter().zip(&source.rules) {
            assert!(
                page.section(rule) == wanted.text,
                "{topic}#{}: section\n--- embedded\n{}\n--- source\n{}",
                rule.id,
                page.section(rule),
                wanted.text
            );
        }
        assert_eq!(
            specengine_cli::guide_page(topic).map(|found| found.topic),
            Some(page.topic)
        );
    }
    assert!(specengine_cli::guide_page("nope").is_none());
    // A title and a summary are as written, Markdown kept: `auto`'s title
    // holds its code span (as "Data"'s table writes it), its summary the
    // backticked `specengine.toml` of its first sentence. M: backticks
    // dropped.
    let auto = specengine_cli::guide_page("auto").expect("the auto topic");
    assert_eq!(auto.title, "The `[auto]` mode");
    assert!(
        auto.summary.contains("`specengine.toml`") && auto.summary.ends_with('.'),
        "{}",
        auto.summary
    );
    let index = guide(
        &Scratch::new("gu3-as-written").dir("empty"),
        None,
        None,
        false,
    );
    assert!(
        index.stdout.contains("auto | The `[auto]` mode | Auto mode is a committed rule in \
                               `specengine.toml`"),
        "the index line as written: {}",
        index.stdout
    );
}

/// GU3: the embed equals `docs/guide/*.md`: order, topics, titles,
/// summaries, `summarises`, `reviewed`, rule IDs, titles and lines, text.
#[test]
fn gu3_the_embed_equals_the_source() {
    let pages = source_pages();
    let differences = embed_differences(&pages);
    assert!(
        differences.is_empty(),
        "the embed is not docs/guide/*.md (rebuild?):\n{}",
        differences.join("\n")
    );
    // The source itself is what the spec names: ten pages, each read.
    let on_disk: Vec<String> = fs::read_dir(source_dir())
        .expect("docs/guide/")
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            name.strip_suffix(".md").map(str::to_owned)
        })
        .collect();
    let mut sorted_disk = on_disk.clone();
    sorted_disk.sort();
    let mut sorted_order: Vec<String> = GUIDE_ORDER.iter().map(|t| (*t).to_owned()).collect();
    sorted_order.sort();
    assert_eq!(
        sorted_disk, sorted_order,
        "docs/guide/ holds GUIDE_ORDER's pages alone"
    );
}

/// GU3, M (named) "a stale embed": a page edited in the test's copy of
/// `docs/guide` without a rebuild (its text, a rule's title, its
/// `reviewed`) is named by the comparison; the untouched copy is not.
#[test]
fn gu3_a_stale_embed_is_caught() {
    let scratch = Scratch::new("gu3-stale");
    let copy = scratch.join("guide");
    copy_dir(&source_dir(), &copy);
    assert!(
        embed_differences(&pages(&copy)).is_empty(),
        "premise: the copy is the embed"
    );
    let edit = |topic: &str, from: &str, to: &str| {
        let path = copy.join(format!("{topic}.md"));
        let text = fs::read_to_string(&path).expect("a page");
        assert!(text.contains(from), "{topic}: {from:?}");
        fs::write(&path, text.replacen(from, to, 1)).expect("an edit");
    };
    let proposals = fs::read_to_string(copy.join("proposals.md")).expect("proposals");
    fs::write(
        copy.join("proposals.md"),
        format!("{proposals}\nOne more line.\n"),
    )
    .expect("an appended line");
    edit(
        "records",
        "{#GD-RECORDS-ONE-FILE}",
        "{#GD-RECORDS-ONE-PLACE}",
    );
    edit("hub", "reviewed: ", "reviewed: 1999-01-01\nx-was: ");
    let differences = embed_differences(&pages(&copy));
    for topic in ["proposals", "records", "hub"] {
        assert!(
            differences
                .iter()
                .any(|line| line.starts_with(&format!("{topic}:"))),
            "{topic} named: {differences:#?}"
        );
    }
    assert!(
        differences
            .iter()
            .all(|line| ["proposals:", "records:", "hub:"]
                .iter()
                .any(|topic| line.starts_with(topic))),
        "only the edited pages: {differences:#?}"
    );
}

// ------------------------------------------------------------- GU3 build.rs

/// The crate's build script, as its `Cargo.toml` names it (`build =`, by
/// default `build.rs` at the crate's root), relative to the crate.
fn build_script() -> String {
    let manifest = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
        .expect("the crate's Cargo.toml");
    manifest
        .lines()
        .find_map(|line| {
            line.trim()
                .strip_prefix("build")
                .map(str::trim_start)
                .and_then(|rest| rest.strip_prefix('='))
                .map(|value| value.trim().trim_matches('"').to_owned())
        })
        .unwrap_or_else(|| "build.rs".to_owned())
}

/// The crate's `src/` and its build script, copied under `root` as the
/// crate's own place, so a relative `include!` and `../../docs/guide`
/// resolve inside the scratch tree.
fn plant_crate(root: &Path) -> PathBuf {
    let crate_dir = root.join("crates").join("specengine-cli");
    let source = guide_source::repository_root()
        .join("crates")
        .join("specengine-cli");
    fs::create_dir_all(&crate_dir).expect("the crate's directory");
    copy_dir(&source.join("src"), &crate_dir.join("src"));
    let script = build_script();
    let target = crate_dir.join(&script);
    fs::create_dir_all(target.parent().expect("a parent")).expect("the script's directory");
    common::copy_file(source.join(&script), &target);
    crate_dir
}

/// What one run of the build script gave.
struct Built {
    ok: bool,
    output: String,
}

/// `build.rs`, compiled alone by `rustc` with the scratch crate as its
/// `CARGO_MANIFEST_DIR` (at compile time and at run time), then run with an
/// `OUT_DIR` of its own. A failure is a non-zero exit or a `cargo:error` /
/// `cargo::error` line.
fn run_build_script(root: &Path) -> Built {
    let crate_dir = plant_crate(root);
    let out_dir = root.join("out");
    fs::create_dir_all(&out_dir).expect("OUT_DIR");
    let exe = root.join("build-script");
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| OsString::from("rustc"));
    let env: BTreeMap<&str, &OsStr> = BTreeMap::from([
        ("CARGO_MANIFEST_DIR", crate_dir.as_os_str()),
        ("OUT_DIR", out_dir.as_os_str()),
        ("CARGO_PKG_NAME", OsStr::new("specengine-cli")),
        ("CARGO_PKG_VERSION", OsStr::new("0.1.0")),
    ]);
    let compiled = Command::new(&rustc)
        .current_dir(&crate_dir)
        .envs(env.iter().map(|(key, value)| (*key, *value)))
        .args(["--edition", "2024", "--crate-type", "bin", "--crate-name"])
        .arg("build_script_build")
        .arg("-o")
        .arg(&exe)
        .arg(build_script())
        .stdin(Stdio::null())
        .output()
        .expect("rustc runs");
    assert!(
        compiled.status.success(),
        "build.rs compiles alone (it has no build-dependency):\n{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let ran = Command::new(&exe)
        .current_dir(&crate_dir)
        .env_clear()
        .envs(env.iter().map(|(key, value)| (*key, *value)))
        .stdin(Stdio::null())
        .output()
        .expect("the build script runs");
    let stdout = String::from_utf8_lossy(&ran.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&ran.stderr).into_owned();
    let error_line = stdout
        .lines()
        .any(|line| line.starts_with("cargo:error=") || line.starts_with("cargo::error="));
    Built {
        ok: ran.status.success() && !error_line,
        output: format!("{stdout}{stderr}"),
    }
}

/// One refused case of `build.rs`: its name, how it breaks the copy of
/// `docs/guide`, and the files the build error may name.
type BrokenGuide = (&'static str, Box<dyn FnOnce(&Path)>, &'static [&'static str]);

/// A scratch tree holding a copy of `docs/guide`, edited by `change`.
fn tree_with(scratch: &Scratch, name: &str, change: impl FnOnce(&Path)) -> PathBuf {
    let root = scratch.dir(name);
    let guide = root.join("docs").join("guide");
    copy_dir(&source_dir(), &guide);
    change(&guide);
    root
}

/// GU3: `build.rs` accepts `docs/guide` as it is and refuses, naming the
/// file or the directory: a missing directory; a page with no front-matter;
/// a topic absent from `GUIDE_ORDER`; a `GUIDE_ORDER` topic with no page;
/// a rule ID repeated across pages. M: any of the five accepted.
#[test]
fn gu3_build_rs_refuses_a_broken_guide_naming_the_file() {
    let scratch = Scratch::new("gu3-build");
    let fine = tree_with(&scratch, "fine", |_| {});
    let built = run_build_script(&fine);
    assert!(built.ok, "the guide as it is builds:\n{}", built.output);

    let missing = scratch.dir("missing");
    let built = run_build_script(&missing);
    assert!(!built.ok, "M: a missing docs/guide accepted");
    assert!(
        built.output.contains("docs/guide"),
        "the missing directory named:\n{}",
        built.output
    );

    let cases: Vec<BrokenGuide> = vec![
        (
            "no-front-matter",
            Box::new(|guide: &Path| {
                let path = guide.join("cards.md");
                let text = fs::read_to_string(&path).expect("cards.md");
                let body = text
                    .strip_prefix("---\n")
                    .and_then(|rest| rest.split_once("\n---\n"))
                    .map(|(_, body)| body.to_owned())
                    .expect("front-matter");
                fs::write(&path, body).expect("no front-matter");
            }),
            &["cards.md"],
        ),
        (
            "extra-topic",
            Box::new(|guide: &Path| {
                common::copy_file(guide.join("hub.md"), guide.join("lighthouse.md"));
                let path = guide.join("lighthouse.md");
                let text = fs::read_to_string(&path).expect("lighthouse.md");
                fs::write(&path, text.replace("{#GD-HUB-", "{#GD-LIGHTHOUSE-")).expect("renamed");
            }),
            &["lighthouse.md"],
        ),
        (
            "missing-topic",
            Box::new(|guide: &Path| fs::remove_file(guide.join("markers.md")).expect("removed")),
            &["markers"],
        ),
        (
            "repeated-rule-in-a-page",
            Box::new(|guide: &Path| {
                let path = guide.join("records.md");
                let text = fs::read_to_string(&path).expect("records.md");
                fs::write(
                    &path,
                    format!("{text}\n### Once more {{#GD-RECORDS-ONE-FILE}}\n\nTwice.\n"),
                )
                .expect("a repeated rule");
            }),
            &["records.md"],
        ),
        (
            "repeated-rule-across-pages",
            Box::new(|guide: &Path| {
                let path = guide.join("records.md");
                let text = fs::read_to_string(&path).expect("records.md");
                fs::write(
                    &path,
                    format!(
                        "{text}\n### Never write by hand {{#GD-PROPOSALS-ONE-DOOR}}\n\nTwice.\n"
                    ),
                )
                .expect("a repeated rule");
            }),
            // Either of the two pages holding it.
            &["records.md", "proposals.md"],
        ),
    ];
    for (name, change, named) in cases {
        let root = tree_with(&scratch, name, change);
        let built = run_build_script(&root);
        assert!(!built.ok, "{name}: M: accepted:\n{}", built.output);
        assert!(
            named.iter().any(|file| built.output.contains(file)),
            "{name}: the build error names {named:?}:\n{}",
            built.output
        );
    }
}

// ------------------------------------------------------------------ GU15

/// Every read's stdout at `cwd` under `home`, by REF (`-` the index) and
/// form.
fn all_reads(cwd: &Path, home: &Path, pages: &[Page]) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for json in [false, true] {
        let run = guide(cwd, Some(home), None, json);
        run.code(0);
        out.insert(format!("- {json}"), run.stdout);
        for page in pages {
            let run = guide(cwd, Some(home), Some(&page.topic), json);
            run.code(0);
            out.insert(format!("{} {json}", page.topic), run.stdout);
        }
    }
    out
}

/// GU15: `spec guide --json` and every topic's text and JSON are
/// byte-equal across two directories, two `HOME`s and with or without a
/// project (a copy of `fixtures/spec-a`, at its root and with `--root`).
#[test]
fn gu15_one_build_one_guide() {
    let pages = source_pages();
    let one = Scratch::new("gu15-one");
    let two = Scratch::new("gu15-two");
    let first = all_reads(&one.dir("a"), &one.home("h1"), &pages);
    let second = all_reads(&two.dir("b/c/deep"), &two.home("h2"), &pages);
    assert_eq!(first, second, "two directories, two HOMEs");
    let project = two.copy("spec-a", "spec-a");
    let inside = all_reads(&project, &two.home("h3"), &pages);
    assert_eq!(first, inside, "inside a project");
    let root = project.to_str().expect("a UTF-8 root");
    let rooted = guide_in(
        &one.dir("a"),
        Some(&one.home("h4")),
        &["--root", root, "guide", "--json"],
    );
    rooted.code(0);
    assert_eq!(rooted.stdout, first["- true"], "with --root a project");
    assert_eq!(first["- false"], index_text(&pages));
    for page in &pages {
        assert_eq!(first[&format!("{} false", page.topic)], page.text);
    }
}
