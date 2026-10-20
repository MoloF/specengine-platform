//! docs/features/roadmap-view.md RD-2 (the comparison) and RD-4 (links),
//! core part, over a crafted corpus built in memory: a plan of seven chores
//! under `w/` (one outside it, one done, one in the pool, one unplaced, a
//! chain, a bad estimate), the index root `a/index.md`, its archive shard
//! `a/b/arch.md` and the roadmap view `a/v/plan.md` registered on the index
//! entry.
//!
//! - The view is compared as every index output is (§11.5): a hand edit,
//!   an item edited or `[roadmap]` changed without the view regenerated →
//!   one `index-drift` on the view, on the line of the first differing
//!   byte, the root and the shard clean; the view not walked →
//!   `index-missing` on its path; a walk gap → nothing compared; the view's
//!   bytes not supplied → the run cannot check.
//! - The view is never listed in the index, whatever its front-matter
//!   (seeded `class: canon`, an unclosed front-matter); without the key the
//!   root and the shard are the same bytes and there is no third output.
//! - Determinism: reversed and rotated input → identical bytes.
//! - RD-4: every item link, read from the view's directory, names the
//!   item's file, the view at the root, in the items' directory, one and
//!   three levels down.
//!
//! Named mutations: M2 (the view skipped by the comparison) turns
//! `hand_edited_view_is_index_drift` red; M3 (a missing view not reported)
//! `missing_view_is_index_missing`; M5 (links written root-relative)
//! `view_links_resolve_from_its_directory`.

mod common;

use common::check::{Config, show};
use specengine_core::check::{
    CheckFile, CheckInput, IndexOutput, Report, Verdict, render_index_set,
};

const ROOT: &str = "a/index.md";
const ARCH: &str = "a/b/arch.md";
const VIEW: &str = "a/v/plan.md";

/// The index entry with an archive shard and the view.
const TOML: &str = "\
[paths]
index = \"a/index.md\"

[ids]
TSK = { kind = \"chore\", width = 3 }
ADR = { kind = \"decision\", width = 4 }

[roadmap]
prefix     = \"TSK\"
dir        = \"w\"
releases   = [\"first\", \"second\"]
mvp_cut    = \"first\"
priorities = [\"high\", \"low\"]
unit       = \"hours\"

[[generators]]
command = \"make index\"
writes  = [\"a/index.md\", \"a/b/arch.md\", \"a/v/plan.md\"]
index   = true
gate    = \"make gate\"
shards  = [{ path = \"a/b/arch.md\", tier3 = true }]
roadmap = \"a/v/plan.md\"
";

/// [`TOML`] without the view: the key and its `writes` entry dropped.
fn toml_without_view() -> String {
    let text = TOML
        .replace("roadmap = \"a/v/plan.md\"\n", "")
        .replace(", \"a/v/plan.md\"]", "]");
    assert!(!text.contains(VIEW), "{text}");
    text
}

/// A plan item with the given front-matter lines after `scope`.
fn item(id: &str, status: &str, title: &str, extra: &str) -> String {
    format!("---\nclass: spec\nid: {id}\nstatus: {status}\nscope: [w]\n{extra}---\n\n# {title}\n")
}

/// Every kind the plan tells apart, beside a canon and a decision.
fn corpus() -> Vec<(String, String)> {
    let mut files = vec![
        (
            "top.md".to_owned(),
            "---\nclass: canon\ntier: 0\nscope: [all]\n---\n\n# Top rules\n".to_owned(),
        ),
        (
            "a/d/ADR-0001.md".to_owned(),
            "---\nid: ADR-0001\nclass: decision\ntitle: One\nstatus: accepted\nscope: [x]\n---\n\n# One\n"
                .to_owned(),
        ),
    ];
    let items = [
        (
            "w/TSK-001.md",
            item("TSK-001", "shipped", "Lay the floor", "release: first\n"),
        ),
        (
            "w/TSK-002.md",
            item(
                "TSK-002",
                "draft",
                "Paint the walls",
                "release: first\npriority: high\nestimate: 2-3\n",
            ),
        ),
        (
            "w/TSK-003.md",
            item(
                "TSK-003",
                "in-progress",
                "Hang the doors",
                "release: first\npriority: low\nestimate: 1\nlinks:\n  depends_on: [TSK-002]\n",
            ),
        ),
        (
            "w/TSK-004.md",
            item(
                "TSK-004",
                "draft",
                "Wire the lights",
                "release: second\npriority: high\nestimate: lots\n",
            ),
        ),
        (
            "w/TSK-005.md",
            item(
                "TSK-005",
                "draft",
                "Plant a hedge",
                "priority: low\nestimate: 4\n",
            ),
        ),
        (
            "w/TSK-006.md",
            item("TSK-006", "draft", "Build a shed", "release: later\n"),
        ),
        (
            "q/TSK-007.md",
            item(
                "TSK-007",
                "draft",
                "Fix the gate",
                "release: second\npriority: low\nestimate: 2\n",
            ),
        ),
    ];
    files.extend(
        items
            .into_iter()
            .map(|(path, text)| (path.to_owned(), text)),
    );
    files
}

fn pairs(files: &[(String, String)]) -> Vec<(&str, &str)> {
    files
        .iter()
        .map(|(p, t)| (p.as_str(), t.as_str()))
        .collect()
}

fn render_set(config: &Config, input: &CheckInput) -> Vec<IndexOutput> {
    let generator = config.check.index_generator().expect("the index entry");
    render_index_set(input, ROOT, generator)
}

fn output<'o>(outputs: &'o [IndexOutput], path: &str) -> &'o str {
    &outputs
        .iter()
        .find(|o| o.path == path)
        .unwrap_or_else(|| panic!("no output {path}"))
        .bytes
}

/// The corpus with every output holding its render.
fn rendered(config: &Config) -> (Vec<(String, String)>, Vec<IndexOutput>) {
    let mut files = corpus();
    let outputs = render_set(config, &config.input(&pairs(&files)));
    for o in &outputs {
        files.push((o.path.clone(), o.bytes.clone()));
    }
    (files, outputs)
}

fn check(config: &Config, files: &[(String, String)]) -> Report {
    config.check(&pairs(files))
}

fn index_findings(report: &Report) -> Vec<(String, String, usize)> {
    report
        .findings
        .iter()
        .filter(|f| f.code.starts_with("index-"))
        .map(|f| (f.code.clone(), f.path.clone(), f.line))
        .collect()
}

fn edit(files: &mut [(String, String)], path: &str, from: &str, to: &str) {
    let file = files.iter_mut().find(|(p, _)| p == path).unwrap();
    assert_eq!(file.1.matches(from).count(), 1, "{from:?} once in {path}");
    file.1 = file.1.replacen(from, to, 1);
}

fn line_of(text: &str, needle: &str) -> usize {
    text.lines()
        .position(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("{needle:?} not in:\n{text}"))
        + 1
}

/// The 1-based line of the first byte where `walked` and `rendered` differ.
fn first_difference_line(walked: &str, rendered: &str) -> usize {
    let (a, b) = (walked.as_bytes(), rendered.as_bytes());
    let offset = a
        .iter()
        .zip(b)
        .position(|(x, y)| x != y)
        .unwrap_or_else(|| a.len().min(b.len()));
    assert!(walked != rendered, "no difference");
    1 + a[..offset].iter().filter(|&&byte| byte == b'\n').count()
}

// ------------------------------------------------------------------ the view

/// The view is the last output, after the root and the shard, under the
/// index header naming the registered command and gate; on a clean walk no
/// finding at all is on its path (its front-matter meets the `generated`
/// contract, its `generator:` is registered and writes it).
#[test]
fn the_view_is_the_last_output_and_checks_clean() {
    let config = Config::from_toml(TOML);
    let (files, outputs) = rendered(&config);
    let paths: Vec<&str> = outputs.iter().map(|o| o.path.as_str()).collect();
    assert_eq!(paths, [ROOT, ARCH, VIEW]);
    let view = output(&outputs, VIEW);
    assert!(
        view.starts_with("---\nclass: generated\ngenerator: make index\nsource: "),
        "{view}"
    );
    assert!(
        view.contains(
            "<!-- Built by `make index`. Manual edits are overwritten on rebuild, and `make gate` rejects them. -->\n"
        ),
        "{view}"
    );
    assert!(view.ends_with('\n') && !view.ends_with("\n\n"), "{view:?}");
    assert!(!view.contains('\r'), "LF only");
    // Open items listed (links from `a/v/`), the done one only counted.
    assert!(view.contains("[TSK-002](../../w/TSK-002.md)"), "{view}");
    assert!(view.contains("[TSK-007](../../q/TSK-007.md)"), "{view}");
    assert!(
        !view.contains("TSK-001"),
        "a done item is not listed:\n{view}"
    );

    let report = check(&config, &files);
    assert!(index_findings(&report).is_empty(), "{}", show(&report));
    let on_view: Vec<String> = report
        .findings
        .iter()
        .filter(|f| f.path == VIEW)
        .map(|f| format!("{} {}", f.code, f.message))
        .collect();
    assert!(on_view.is_empty(), "{on_view:?}");
    // The rendered set is stable once its files are walked.
    assert_eq!(
        render_set(&config, &config.input(&pairs(&files))),
        outputs,
        "the outputs are not listed in themselves, the view not in the plan"
    );
}

/// M2.
#[test]
fn hand_edited_view_is_index_drift() {
    let config = Config::from_toml(TOML);
    let (files, outputs) = rendered(&config);
    let view = output(&outputs, VIEW).to_owned();
    let only_view = |report: &Report, line: usize, context: &str| {
        assert_eq!(
            index_findings(report),
            [("index-drift".to_owned(), VIEW.to_owned(), line)],
            "{context}\n{}",
            show(report)
        );
        let finding = report
            .findings
            .iter()
            .find(|f| f.code == "index-drift")
            .unwrap();
        assert!(
            finding.message.contains("`make index`"),
            "{context}: {}",
            finding.message
        );
    };

    // A title edited on line k of the view.
    let k = line_of(&view, "[TSK-002]");
    let mut edited = files.clone();
    edit(&mut edited, VIEW, "Paint the walls", "Paint the Walls");
    only_view(&check(&config, &edited), k, "a hand edit");

    // A line appended: drift where the render ends.
    let mut edited = files.clone();
    edited
        .iter_mut()
        .find(|(p, _)| p == VIEW)
        .unwrap()
        .1
        .push_str("- extra\n");
    only_view(
        &check(&config, &edited),
        view.lines().count() + 1,
        "a line appended",
    );

    // An item edited, the view not regenerated: a priority the index never
    // shows, so the root and the shard stay clean.
    let mut edited = files.clone();
    edit(
        &mut edited,
        "w/TSK-003.md",
        "priority: low",
        "priority: high",
    );
    let fresh = render_set(&config, &config.input(&pairs(&edited)));
    let k = first_difference_line(&view, output(&fresh, VIEW));
    only_view(&check(&config, &edited), k, "an item edited");

    // `[roadmap]` changed, the view not regenerated.
    let changed =
        Config::from_toml(&TOML.replace("unit       = \"hours\"", "unit       = \"points\""));
    let fresh = render_set(&changed, &changed.input(&pairs(&files)));
    assert_eq!(output(&fresh, ROOT), output(&outputs, ROOT));
    let k = first_difference_line(&view, output(&fresh, VIEW));
    only_view(&check(&changed, &files), k, "[roadmap] changed");

    // The view and the root edited: each reported, neither stops the other.
    let mut edited = files.clone();
    edit(&mut edited, VIEW, "Paint the walls", "Paint the Walls");
    edit(&mut edited, ROOT, "Top rules", "Top Rules");
    let mut got: Vec<String> = index_findings(&check(&config, &edited))
        .into_iter()
        .map(|(code, path, _)| format!("{code} {path}"))
        .collect();
    got.sort();
    assert_eq!(
        got,
        [format!("index-drift {ROOT}"), format!("index-drift {VIEW}")]
    );
}

/// M3.
#[test]
fn missing_view_is_index_missing() {
    let config = Config::from_toml(TOML);
    let (files, _) = rendered(&config);
    let without: Vec<(String, String)> = files.iter().filter(|(p, _)| p != VIEW).cloned().collect();
    let report = check(&config, &without);
    assert_eq!(
        index_findings(&report),
        [("index-missing".to_owned(), VIEW.to_owned(), 1)],
        "{}",
        show(&report)
    );
    let finding = report
        .findings
        .iter()
        .find(|f| f.code == "index-missing")
        .unwrap();
    assert!(
        finding.message.contains("`make index`") && finding.message.contains("roadmap view"),
        "names the writer and the output: {}",
        finding.message
    );
    assert!(report.findings.iter().all(|f| f.code != "index-drift"));
}

#[test]
fn a_walk_gap_compares_no_view() {
    let config = Config::from_toml(TOML);
    let (files, _) = rendered(&config);
    let mut edited = files.clone();
    edit(&mut edited, VIEW, "Paint the walls", "Paint the Walls");
    let complete = check(&config, &edited);
    assert_eq!(index_findings(&complete).len(), 1, "{}", show(&complete));
    for gap in [Some(VIEW), None] {
        let kept: Vec<(String, String)> = edited
            .iter()
            .filter(|(p, _)| Some(p.as_str()) != gap)
            .cloned()
            .collect();
        let mut input = config.input(&pairs(&kept));
        input.files.push(CheckFile {
            path: "q/unread.md".to_owned(),
            size: 10,
            parsed: None,
            read_error: Some("Permission denied (os error 13)".to_owned()),
            bytes: Vec::new(),
        });
        let report = config.run(&input);
        assert_eq!(report.verdict, Verdict::CannotCheck, "{gap:?}");
        assert!(
            index_findings(&report).is_empty(),
            "{gap:?}: {}",
            show(&report)
        );
    }
}

#[test]
fn view_bytes_not_supplied_cannot_be_compared() {
    let config = Config::from_toml(TOML);
    let (files, _) = rendered(&config);
    let mut input = config.input(&pairs(&files));
    let view = input.files.iter_mut().find(|f| f.path == VIEW).unwrap();
    assert!(view.size > 0);
    view.bytes = Vec::new();
    let report = config.run(&input);
    assert_eq!(report.verdict, Verdict::CannotCheck, "{}", show(&report));
    let causes: Vec<&str> = report
        .cannot_check
        .iter()
        .filter(|cause| cause.path == VIEW)
        .map(|cause| cause.message.as_str())
        .collect();
    assert_eq!(causes.len(), 1, "{:?}", report.cannot_check);
    assert!(causes[0].contains("`make index`"), "{}", causes[0]);
    assert!(index_findings(&report).is_empty(), "{}", show(&report));
}

/// The view is never a line of the index, whatever its walked front-matter;
/// the index outputs are the same bytes with and without the view.
#[test]
fn the_view_is_never_listed_in_the_index() {
    let config = Config::from_toml(TOML);
    let plain = Config::from_toml(&toml_without_view());
    let seeds = [
        "---\nclass: canon\ntier: 2\nscope: [x]\n---\n\n# Not the view yet\n",
        "---\nclass: spec\nstatus: draft\n\n# Never closed\n",
        "no front-matter at all\n",
        "---\nclass: spec\nstatus: shipped\nscope: [x]\n---\n\n# Shipped, so archive\n",
    ];
    for seed in seeds {
        let mut files = corpus();
        files.push((VIEW.to_owned(), seed.to_owned()));
        let input = config.input(&pairs(&files));
        let outputs = render_set(&config, &input);
        for o in &outputs {
            assert!(
                !o.bytes.contains("](../v/plan.md)")
                    && !o.bytes.contains("](v/plan.md)")
                    && !o.bytes.contains(&format!("[{VIEW}]")),
                "{seed:?}: {} lists the view:\n{}",
                o.path,
                o.bytes
            );
        }
        // The view's seed does not enter the plan either.
        let clean = corpus();
        let reference = render_set(&config, &config.input(&pairs(&clean)));
        assert_eq!(
            output(&outputs, VIEW),
            output(&reference, VIEW),
            "{seed:?}: the walked view changes its own render"
        );
        // Without the key: no view output; the root and the shard as with it
        // (the seeded file is then an ordinary document, so compare on the
        // corpus without it).
        let without = render_set(&plain, &plain.input(&pairs(&clean)));
        assert_eq!(
            without.iter().map(|o| o.path.as_str()).collect::<Vec<_>>(),
            [ROOT, ARCH]
        );
        assert_eq!(
            without[..],
            reference[..2],
            "the index unchanged by the view"
        );
    }
}

/// Determinism: reversed and rotated input → identical bytes for every
/// output, the view included.
#[test]
fn reversed_input_renders_identical_view_bytes() {
    let config = Config::from_toml(TOML);
    let files = corpus();
    let mut input = config.input(&pairs(&files));
    let forward = render_set(&config, &input);
    input.files.reverse();
    assert_eq!(render_set(&config, &input), forward);
    input.files.rotate_left(3);
    assert_eq!(render_set(&config, &input), forward);
    // A second render of one input: the same bytes.
    assert_eq!(render_set(&config, &input), forward);
}

// ------------------------------------------------------------------ RD-4

/// `link` from the directory of `from`, root-relative.
fn resolve(from: &str, link: &str) -> String {
    let mut parts: Vec<&str> = from.split('/').collect();
    parts.pop();
    for part in link.split('/') {
        match part {
            ".." => {
                assert!(parts.pop().is_some(), "{link} from {from} leaves the root");
            }
            "." | "" => {}
            part => parts.push(part),
        }
    }
    parts.join("/")
}

/// Every `[label](link)` of `text`, in order.
fn links(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('[') {
        rest = &rest[open + 1..];
        let Some((label, after)) = rest.split_once("](") else {
            break;
        };
        if label.contains(['[', '\n']) {
            continue;
        }
        let (link, after) = after.split_once(')').expect("a closed link");
        out.push((label.to_owned(), link.to_owned()));
        rest = after;
    }
    out
}

/// M5: every item link of the view, read from the view's directory, names
/// the item's file, wherever the view sits: at the root, inside the items'
/// directory, one and three levels down beside them.
#[test]
fn view_links_resolve_from_its_directory() {
    let files = corpus();
    let path_of = |id: &str| -> String {
        files
            .iter()
            .find(|(_, text)| text.contains(&format!("\nid: {id}\n")))
            .map(|(path, _)| path.clone())
            .unwrap_or_else(|| panic!("no item {id}"))
    };
    let mut away = 0;
    for view in ["plan.md", "w/view.md", VIEW, "x/y/z/plan.md"] {
        let config = Config::from_toml(&TOML.replace(VIEW, view));
        let outputs = render_set(&config, &config.input(&pairs(&files)));
        let text = output(&outputs, view);
        let found = links(text);
        let ids: Vec<&str> = found.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "TSK-002", "TSK-003", "TSK-004", "TSK-007", "TSK-005", "TSK-006"
            ],
            "{view}: every open item linked once, in plan order:\n{text}"
        );
        for (id, link) in &found {
            assert_eq!(
                resolve(view, link),
                path_of(id),
                "{view}: [{id}]({link})\n{text}"
            );
            if link != &path_of(id) {
                away += 1;
            }
        }
    }
    assert!(away > 0, "some view sits away from the root");
}
