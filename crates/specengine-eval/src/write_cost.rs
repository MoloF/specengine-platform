//! Measurement `write-cost` (`docs/features/write-cost.md`, "Data"): a
//! write's introduced findings over the walked rows against the whole disk
//! check, on a scratch copy of the corpus's walked files.
//!
//! For each sampled document (`--sample`, by `--seed`) and each edit of the
//! catalogue that applies to it (an ID taken, dropped or given twice, a
//! section, a mention, a file link, a status, a `depends_on`, broken
//! front-matter, a renamed title, a created file in its directory), after
//! one walk: the store's whole disk check and its check over the rows, the
//! latter with no memo and with one (a miss, then a hit). Equal findings,
//! order included, are `equal`; each other case goes to
//! `--out/write-cost/<label>/differ.json` with its path. stdout carries
//! counts and times only: the cases' total milliseconds per path and one
//! unchanged walk's.

use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Serialize;
use specengine_store::{
    CheckMemo, CheckSetup, IndexWriter as _, NamedBytes, ProposalFinding, SetupFiles, Source as _,
    SpecIndex as _, SqliteIndex, WorkingTree, check_memo, default_baseline, introduced_findings,
    introduced_from_rows, load_check, today_utc,
};

use crate::check::refusal;
use crate::harness::Corpus;

/// Fixture directory (relative to `fixtures/`) used without `--pilot`.
pub const FIXTURE: &str = "spec-b";

/// The measurement's name.
pub const MEASUREMENT: &str = "write-cost";

/// The config file at the corpus root.
const CONFIG: &str = "specengine.toml";

/// What the run needs, read before anything is written.
pub struct Setup {
    config: Vec<u8>,
    baseline: Option<Vec<u8>>,
    sample: usize,
    seed: u64,
}

/// Reads the config and the baseline; a missing or invalid one refuses.
pub fn prepare(root: &Path, sample: usize, seed: u64) -> Result<Setup, String> {
    let config = NamedBytes::read(CONFIG, &root.join(CONFIG));
    let baseline = default_baseline(root);
    load_check(&config, baseline.as_ref()).map_err(|report| refusal(&report))?;
    let config = config.bytes.map_err(|error| format!("{CONFIG}: {error}"))?;
    let baseline = match baseline {
        Some(file) => Some(
            file.bytes
                .map_err(|error| format!("the baseline: {error}"))?,
        ),
        None => None,
    };
    Ok(Setup {
        config,
        baseline,
        sample,
        seed,
    })
}

/// The `result` object.
#[derive(Serialize)]
pub struct WriteCostResult {
    pub documents: usize,
    pub cases: usize,
    pub equal: usize,
    pub differ: usize,
    /// Every case's whole disk check, in total.
    pub full_ms: u64,
    /// Every case's check over the rows, no memo, in total.
    pub rows_ms: u64,
    /// Every case's memo hit, in total.
    pub hit_ms: u64,
    /// One walk of the unchanged copy.
    pub walk_ms: u64,
}

/// One case that differed, for the detail file.
#[derive(Serialize)]
struct Differ<'a> {
    path: &'a str,
    edit: usize,
    whole: &'a [ProposalFinding],
    rows: &'a [ProposalFinding],
    memo: &'a [ProposalFinding],
}

pub fn run(corpus: &Corpus, setup: Setup) -> Result<WriteCostResult, String> {
    let dir = corpus.out.join(MEASUREMENT).join(&corpus.label);
    let tree_dir = dir.join("tree");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&tree_dir)
        .map_err(|error| format!("cannot create {}: {error}", tree_dir.display()))?;
    let config = NamedBytes {
        name: CONFIG.to_owned(),
        bytes: Ok(setup.config.clone()),
    };
    let baseline = setup.baseline.clone().map(|bytes| NamedBytes {
        name: specengine_store::BASELINE_FILE.to_owned(),
        bytes: Ok(bytes),
    });
    let check = load_check(&config, baseline.as_ref()).map_err(|report| refusal(&report))?;
    let source =
        WorkingTree::new(&corpus.root, &check.project.paths).map_err(|error| error.to_string())?;
    let listing = source.list().map_err(|error| error.to_string())?;
    // The scratch copy: the walked files, the config and the baseline.
    let write = |path: &str, bytes: &[u8]| -> Result<(), String> {
        let target = tree_dir.join(path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
        }
        fs::write(&target, bytes)
            .map_err(|error| format!("cannot write {}: {error}", target.display()))
    };
    write(CONFIG, &setup.config)?;
    if let Some(bytes) = &setup.baseline {
        write(specengine_store::BASELINE_FILE, bytes)?;
    }
    for path in &listing.paths {
        if let Ok(bytes) = source.read(path) {
            write(path, &bytes)?;
        }
    }
    let root = fs::canonicalize(&tree_dir).map_err(|error| error.to_string())?;
    let tree = WorkingTree::new(&root, &check.project.paths).map_err(|error| error.to_string())?;
    let slug = check.project.slug().unwrap_or(MEASUREMENT).to_owned();
    let mut index =
        SqliteIndex::open(dir.join("index.db"), &slug, &root).map_err(|error| error.to_string())?;
    let scheme = &check.project.scheme;
    index
        .update(&tree, scheme)
        .map_err(|error| error.to_string())?;
    let started = Instant::now();
    index
        .update(&tree, scheme)
        .map_err(|error| error.to_string())?;
    let walk = started.elapsed();
    let input = index.indexed_input().map_err(|error| error.to_string())?;
    let ids: Vec<String> = input
        .files
        .iter()
        .filter_map(|file| file.parsed.as_ref()?.document()?.id.clone())
        .collect();
    let paths: Vec<String> = input.files.iter().map(|file| file.path.clone()).collect();
    let sampled = sample(&paths, setup.sample, setup.seed);
    let today = today_utc();
    let files = SetupFiles {
        config: &config,
        baseline: baseline.as_ref(),
    };
    let memo = Arc::new(CheckMemo::new());
    let mut totals = [Duration::ZERO; 3];
    let mut cases = 0;
    let mut equal = 0;
    let mut differs: Vec<(String, usize, [Vec<ProposalFinding>; 3])> = Vec::new();
    let run_case = |target: &str, bytes: Vec<u8>, check: &CheckSetup| {
        let started = Instant::now();
        let whole = introduced_findings(&tree, check, &today, target, bytes.clone());
        let full = started.elapsed();
        let started = Instant::now();
        let rows = introduced_from_rows(
            Some(&index),
            &tree,
            check,
            files,
            &today,
            target,
            bytes.clone(),
        );
        let over_rows = started.elapsed();
        let (memo_found, hit) = check_memo::scoped(&memo, "-", || {
            let call = || {
                introduced_from_rows(
                    Some(&index),
                    &tree,
                    check,
                    files,
                    &today,
                    target,
                    bytes.clone(),
                )
            };
            call();
            let started = Instant::now();
            let found = call();
            (found, started.elapsed())
        });
        (
            whole,
            rows.findings,
            memo_found.findings,
            [full, over_rows, hit],
        )
    };
    // The memo's first call is the cold path: warm it once.
    if let Some(first) = paths.first()
        && let Ok(bytes) = tree.read(first)
    {
        check_memo::scoped(&memo, "-", || {
            introduced_from_rows(Some(&index), &tree, &check, files, &today, first, bytes)
        });
    }
    for path in &sampled {
        let Ok(bytes) = tree.read(path) else {
            continue;
        };
        let Ok(text) = String::from_utf8(bytes) else {
            continue;
        };
        let own = input
            .files
            .iter()
            .find(|file| &file.path == path)
            .and_then(|file| file.parsed.as_ref()?.document()?.id.clone());
        let other = paths
            .iter()
            .find(|other| *other != path)
            .and_then(|other| other.rsplit('/').next())
            .unwrap_or_default()
            .to_owned();
        let mut targets: Vec<(String, String)> = edits(&text, own.as_deref(), &ids, &other)
            .into_iter()
            .map(|edited| (path.clone(), edited))
            .collect();
        if let Some(id) = ids.first() {
            let created = match path.rsplit_once('/') {
                Some((dir, _)) => format!("{dir}/zz-write-cost.md"),
                None => "zz-write-cost.md".to_owned(),
            };
            targets.push((
                created,
                format!("---\nid: {id}\nclass: decision\n---\n\n# Made\n\nCites {id}.\n"),
            ));
        }
        for (edit, (target, edited)) in targets.into_iter().enumerate() {
            let (whole, rows, memo_found, times) = run_case(&target, edited.into_bytes(), &check);
            cases += 1;
            for (total, time) in totals.iter_mut().zip(times) {
                *total += time;
            }
            if whole == rows && whole == memo_found {
                equal += 1;
            } else {
                differs.push((target, edit, [whole, rows, memo_found]));
            }
        }
    }
    let detail: Vec<Differ<'_>> = differs
        .iter()
        .map(|(path, edit, [whole, rows, memo])| Differ {
            path,
            edit: *edit,
            whole,
            rows,
            memo,
        })
        .collect();
    let json = serde_json::to_string_pretty(&detail).map_err(|error| error.to_string())?;
    let detail_path = dir.join("differ.json");
    fs::write(&detail_path, json)
        .map_err(|error| format!("cannot write {}: {error}", detail_path.display()))?;
    let ms = |duration: Duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX);
    eprintln!(
        "write-cost: {cases} cases over {} documents, {} differ; detail in {}",
        sampled.len(),
        differs.len(),
        dir.display()
    );
    Ok(WriteCostResult {
        documents: sampled.len(),
        cases,
        equal,
        differ: differs.len(),
        full_ms: ms(totals[0]),
        rows_ms: ms(totals[1]),
        hit_ms: ms(totals[2]),
        walk_ms: ms(walk),
    })
}

/// `count` paths of `paths` picked by `seed` (a fixed linear congruential
/// order), in path order; all when there are no more.
fn sample(paths: &[String], count: usize, seed: u64) -> Vec<String> {
    if paths.len() <= count {
        return paths.to_vec();
    }
    let mut state = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
    let mut keyed: Vec<(u64, &String)> = paths
        .iter()
        .map(|path| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (state, path)
        })
        .collect();
    keyed.sort();
    let mut picked: Vec<String> = keyed
        .into_iter()
        .take(count)
        .map(|(_, path)| path.clone())
        .collect();
    picked.sort();
    picked
}

/// The catalogue's edits of `text`, each a new text that differs from it:
/// `own` its document's ID, `ids` the corpus's, `other` another file's name.
fn edits(text: &str, own: Option<&str>, ids: &[String], other: &str) -> Vec<String> {
    let foreign = ids.iter().find(|id| Some(id.as_str()) != own).cloned();
    let mut out = vec![format!(
        "{text}\nSee [there]({other}#nowhere) and [it]({other}).\n"
    )];
    if let Some(id) = &foreign {
        out.push(format!("{text}\nAs {id} says.\n"));
        out.push(format!("{text}\n## Again {{#{id}}}\n\nOnce more.\n"));
        if let Some(own) = own {
            out.push(text.replacen(&format!("id: {own}"), &format!("id: {id}"), 1));
        }
        out.push(text.replacen("links:\n", &format!("links:\n  depends_on: [{id}]\n"), 1));
        if let Some(status) = text.lines().find(|line| line.starts_with("status:")) {
            out.push(text.replacen(status, &format!("status: superseded-by {id}"), 1));
        }
    }
    if let Some(own) = own {
        out.push(text.replacen(&format!("id: {own}\n"), "", 1));
        out.push(text.replacen(&format!("id: {own}"), &format!("id: {own}X"), 1));
    }
    out.push(text.replacen("---\n", "---\nbroken: [\n", 1));
    out.push(text.replacen("title:", "title: Renamed", 1));
    out.retain(|edited| edited != text);
    out
}
