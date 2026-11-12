//! The direction-catalog task spec, DC-8, the CLI unit half: one read per
//! call, counted behind the `#[cfg(test)]` counters of
//! `crate::subset::seams` (`catalog_traces`, `catalog_items`,
//! `catalog_walks`).
//!
//! - One `spec catalog` over 40 rows in 12 files: one trace reading, one
//!   open proposal list, one git walk for the 12 files. M: a walk per file.
//! - The daemon's `LastChangeCache`: a second read at the same `HEAD` walks
//!   for nothing; a new commit walks again, for every file; a read with one
//!   more kind walks for the new kind's files alone; every answer equals the
//!   uncached read's at that state. M: the cache keyed without `HEAD`.
//!
//! Every case works on a scratch repository written here under
//! `std::env::temp_dir()`, with a scratch `HOME`, removed on drop; its git
//! setup runs with no global or system config and a fixed identity, the
//! read's own git the process's. This file is a CLI source, so the genre
//! rule (`tests/genre.rs`) holds for it: the corpus's words are invented
//! here and its root is `spec`.

use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::subset::seams;
use crate::{
    CardNewRequest, CatalogOutcome, CatalogRequest, Env, Exit, Globals, LastChangeCache, Outcome,
    View, card_new, catalog, catalog_with_view, render_json,
};

static NEXT: AtomicUsize = AtomicUsize::new(0);

const CONFIG: &str = r#"[project]
slug = "harbor-rows"

[paths]
roots = ["spec"]
spec = "spec"

[ids]
BTH = { kind = "berth", shape = "name" }
PIR = { kind = "pier", shape = "name" }

[[card.flow]]
name = "rig"
disciplines = ["rig"]
columns = ["todo", "done"]
accept = { from = "todo", to = "done" }
"#;

/// The scratch repository, its `HOME` and the read's environment.
struct Place {
    dir: PathBuf,
    root: PathBuf,
    home: PathBuf,
    env: Env,
    globals: Globals,
    commits: usize,
}

impl Drop for Place {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

/// The berth file `n`'s path, root-relative.
fn berth_path(n: usize) -> String {
    format!("spec/berths/bth-{n:02}.md")
}

/// Berth file `n`: a document and `sections` sections.
fn berth(n: usize, sections: usize, word: &str) -> String {
    let mut text = format!(
        "---\nid: BTH-B{n:02}\nclass: canon\ntitle: Berth {n:02}\n---\n\n# Berth {n:02}\n\nA \
         {word} berth.\n"
    );
    for section in 0..sections {
        text.push_str(&format!(
            "\n## Bollard {section} {{#BTH-B{n:02}-K{section}}}\n\nA bollard.\n"
        ));
    }
    text
}

impl Place {
    /// The config and 12 berth files (four with three sections, eight with
    /// two: 40 nodes), each committed by itself; one card on the first
    /// berth, so the queue exists; one pier file, committed.
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "specengine-catalog-unit-{name}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("a scratch directory");
        let dir = fs::canonicalize(&dir).expect("canonical");
        let root = dir.join("repo");
        let home = dir.join("home");
        fs::create_dir_all(&root).expect("the root");
        fs::create_dir_all(&home).expect("a home");
        let mut place = Self {
            env: Env {
                cwd: root.clone(),
                home: Some(home.clone().into_os_string()),
                xdg_data_home: None,
            },
            globals: Globals::default(),
            dir,
            root,
            home,
            commits: 0,
        };
        place.git(&["init", "-q", "--template=", "-b", "trunk"]);
        place.put("specengine.toml", CONFIG);
        place.commit("the config");
        for n in 0..12 {
            let sections = if n < 4 { 3 } else { 2 };
            place.put(&berth_path(n), &berth(n, sections, "deep"));
            place.commit(&format!("berth {n:02}"));
        }
        place.put(
            "spec/piers/pir-north.md",
            "---\nid: PIR-NORTH\nclass: canon\ntitle: The north pier\n---\n\n# The north \
             pier\n\nStone.\n",
        );
        place.commit("the pier");
        let made = card_new(
            &place.env,
            &place.globals,
            &CardNewRequest {
                title: "Paint the bollards".to_owned(),
                discipline: Some("rig".to_owned()),
                targets: vec!["BTH-B00".to_owned()],
                now: "2026-02-01T10:00:00Z".to_owned(),
                ..CardNewRequest::default()
            },
        )
        .expect("card new");
        assert_eq!(made.exit(), Exit::Answered, "{made:?}");
        place
    }

    fn put(&self, relative: &str, text: &str) {
        let path = self.root.join(relative);
        fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
        fs::write(path, text).expect("a file");
    }

    /// The setup's git: no global or system config, a fixed identity.
    fn git(&self, args: &[&str]) {
        let mut vars: Vec<(OsString, OsString)> = [
            ("GIT_CONFIG_NOSYSTEM", "1"),
            ("GIT_CONFIG_GLOBAL", "/dev/null"),
            ("GIT_AUTHOR_NAME", "Rope Maker"),
            ("GIT_AUTHOR_EMAIL", "rope@example.invalid"),
            ("GIT_COMMITTER_NAME", "Rope Maker"),
            ("GIT_COMMITTER_EMAIL", "rope@example.invalid"),
        ]
        .iter()
        .map(|(name, value)| ((*name).into(), (*value).into()))
        .collect();
        vars.push(("HOME".into(), self.home.as_os_str().to_owned()));
        if let Some(path) = std::env::var_os("PATH") {
            vars.push(("PATH".into(), path));
        }
        let status = Command::new("git")
            .env_clear()
            .envs(vars)
            .current_dir(&self.root)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .status()
            .expect("git runs");
        assert!(status.success(), "git {args:?}");
    }

    /// `git add -A` and a commit, one second after the last.
    fn commit(&mut self, message: &str) {
        self.commits += 1;
        let date = format!("{} +0000", 1_780_000_000 + self.commits);
        self.git(&["add", "-A"]);
        let mut args = vec!["commit", "-q", "--no-verify"];
        let author = format!("--date={date}");
        args.push(&author);
        args.extend(["-m", message]);
        // The committer's date too: one stamp per commit.
        let status = Command::new("git")
            .env_clear()
            .envs([
                ("GIT_CONFIG_NOSYSTEM", OsString::from("1")),
                ("GIT_CONFIG_GLOBAL", OsString::from("/dev/null")),
                ("GIT_AUTHOR_NAME", OsString::from("Rope Maker")),
                ("GIT_AUTHOR_EMAIL", OsString::from("rope@example.invalid")),
                ("GIT_COMMITTER_NAME", OsString::from("Rope Maker")),
                (
                    "GIT_COMMITTER_EMAIL",
                    OsString::from("rope@example.invalid"),
                ),
                ("GIT_COMMITTER_DATE", OsString::from(&date)),
                ("HOME", self.home.as_os_str().to_owned()),
                ("PATH", std::env::var_os("PATH").unwrap_or_default()),
            ])
            .current_dir(&self.root)
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .status()
            .expect("git runs");
        assert!(status.success(), "git commit {message}");
    }

    fn request(kinds: &[&str]) -> CatalogRequest {
        CatalogRequest {
            kinds: kinds.iter().map(|kind| (*kind).to_owned()).collect(),
            archive: false,
        }
    }

    /// One read of `kinds` with `cache`: the outcome and the counts it took.
    fn counted(
        &self,
        kinds: &[&str],
        cache: Option<&LastChangeCache>,
    ) -> (CatalogOutcome, seams::ReadCounts) {
        let _ = seams::take_read_counts();
        let outcome = catalog_with_view(
            &self.env,
            &self.globals,
            &Self::request(kinds),
            View::Browser,
            cache,
        )
        .expect("catalog");
        (outcome, seams::take_read_counts())
    }

    /// The uncached read's JSON at this state.
    fn uncached(&self, kinds: &[&str]) -> String {
        let outcome = catalog_with_view(
            &self.env,
            &self.globals,
            &Self::request(kinds),
            View::Browser,
            None,
        )
        .expect("catalog");
        render_json(&Outcome::Catalog(Box::new(outcome)))
    }
}

/// The JSON of `outcome`.
fn json(outcome: CatalogOutcome) -> String {
    render_json(&Outcome::Catalog(Box::new(outcome)))
}

/// Every berth file, root-relative, sorted.
fn berth_files() -> Vec<String> {
    (0..12).map(berth_path).collect()
}

/// The walked paths of `counts`, each walk's sorted (the root is the
/// worktree's top: top-relative is root-relative).
fn walks(counts: &seams::ReadCounts) -> Vec<Vec<String>> {
    counts
        .catalog_walks
        .iter()
        .map(|walk| {
            let mut walk = walk.clone();
            walk.sort();
            walk
        })
        .collect()
}

/// DC-8: one capped read over 40 rows in 12 files: one trace reading, one
/// proposal list, one walk for the 12 files. M: a walk per file.
#[test]
fn dc8_one_call_reads_each_source_once() {
    let place = Place::new("once");
    let _ = seams::take_read_counts();
    let outcome = catalog(&place.env, &place.globals, &Place::request(&["berth"])).expect("read");
    let counts = seams::take_read_counts();
    assert_eq!(outcome.rows.len(), 40);
    assert!(
        outcome.rows.iter().all(|row| row.last_change.is_some()),
        "{outcome:?}"
    );
    assert_eq!(
        (counts.catalog_traces, counts.catalog_items),
        (1, 1),
        "{counts:?}"
    );
    assert_eq!(walks(&counts), [berth_files()]);
}

/// DC-8: the cache: a second read at one `HEAD` walks for nothing; a new
/// commit walks for every file again; one more kind walks for its file
/// alone; each answer the uncached read's. M: the cache keyed without
/// `HEAD`.
#[test]
fn dc8_the_cache_walks_once_per_head() {
    let mut place = Place::new("cache");
    let cache = LastChangeCache::default();
    let (first, counts) = place.counted(&["berth"], Some(&cache));
    assert_eq!(walks(&counts), [berth_files()], "first");
    assert_eq!(json(first), place.uncached(&["berth"]));

    let (second, counts) = place.counted(&["berth"], Some(&cache));
    assert_eq!(
        walks(&counts),
        Vec::<Vec<String>>::new(),
        "the same HEAD: no walk"
    );
    assert_eq!((counts.catalog_traces, counts.catalog_items), (1, 1));
    assert_eq!(json(second), place.uncached(&["berth"]));

    place.put(&berth_path(5), &berth(5, 2, "shallow"));
    place.commit("a shallower berth");
    let (third, counts) = place.counted(&["berth"], Some(&cache));
    assert_eq!(walks(&counts), [berth_files()], "a new HEAD walks again");
    let third = json(third);
    assert_eq!(third, place.uncached(&["berth"]));

    let (both, counts) = place.counted(&["berth", "pier"], Some(&cache));
    assert_eq!(
        walks(&counts),
        [vec!["spec/piers/pir-north.md".to_owned()]],
        "one more kind: its file alone"
    );
    assert_eq!(json(both), place.uncached(&["berth", "pier"]));
}
