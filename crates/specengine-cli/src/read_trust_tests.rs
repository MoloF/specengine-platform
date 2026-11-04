//! The read-trust task spec (step 12e of the scale reads), the CLI unit
//! half, behind the `#[cfg(test)]` counters of `crate::subset::seams` and
//! the crate-private `crate::freshness::post_commit_update`:
//! - AC-8 (a): a warm `search` and a warm `show` read no file in their
//!   freshness step (`ReadCounts.store_reads` empty, `Listed::stat`
//!   called), over a copy last changed 2.1 s before its `spec index`; one
//!   edited file is the next step's one read.
//! - AC-6 (f) and R10, R15: `post_commit_update` given a source whose root
//!   is not the index's (an injected store error) returns today's warning,
//!   after the commit or the completion; inside a scope whose trust
//!   matches it marks that trust dirty and the next read walks; outside a
//!   scope it marks nothing.
//! - D8: a walk dropped without its end (a panic in the update) doubts.
//!
//! Every test works on a scratch copy of a fixture under
//! `std::env::temp_dir()`, with a scratch `HOME`, removed on drop; the
//! repository and the fixtures are only read. This file is a CLI source,
//! so the genre rule (`tests/genre.rs`) holds for it: no project word is
//! written here, and every name a case reads comes from the copy.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use specengine_core::ProjectConfig;
use specengine_store::{GitEnv, WorkingTree, check_input};

use crate::freshness::{After, IndexTrust, post_commit_update, trusted};
use crate::subset::seams;
use crate::{
    Env, Globals, IndexRequest, Message, Outcome, SearchRequest, ShowRequest, render_text,
};

/// The racy window's wait (the store's `RACY_NS` is 2 s).
const PAST_RACY: Duration = Duration::from_millis(2100);

static NEXT: AtomicUsize = AtomicUsize::new(0);

/// A scratch directory, canonical, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "specengine-cli-read-trust-{name}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("a scratch directory");
        Self(fs::canonicalize(&dir).expect("canonical"))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("a copy directory");
    for entry in fs::read_dir(from).expect("a fixture directory").flatten() {
        let target = to.join(entry.file_name());
        if entry.file_type().expect("a file type").is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            let bytes = fs::read(entry.path()).expect("a fixture file");
            fs::write(&target, bytes).expect("a fixture file copied");
        }
    }
}

/// A scratch copy of one fixture with its own `HOME`.
struct Place {
    _scratch: Scratch,
    root: PathBuf,
    other: PathBuf,
    env: Env,
    globals: Globals,
    git: GitEnv,
}

impl Place {
    fn new(fixture: &str) -> Self {
        let scratch = Scratch::new(fixture);
        let root = scratch.0.join("copy");
        copy_dir(&fixtures_dir().join(fixture), &root);
        let root = fs::canonicalize(&root).expect("the copy");
        let other = scratch.0.join("other");
        fs::create_dir_all(&other).expect("another directory");
        let home = scratch.0.join("home");
        fs::create_dir_all(&home).expect("a home");
        Self {
            env: Env {
                cwd: root.clone(),
                home: Some(home.as_os_str().to_owned()),
                xdg_data_home: None,
            },
            globals: Globals {
                root: Some(root.clone()),
                config: None,
            },
            git: GitEnv::new(root.clone(), [("HOME", home.as_os_str().to_owned())]),
            other: fs::canonicalize(&other).expect("canonical"),
            root,
            _scratch: scratch,
        }
    }

    fn config(&self) -> ProjectConfig {
        let text = fs::read_to_string(self.root.join("specengine.toml")).expect("the config");
        ProjectConfig::from_toml(&text).expect("the config")
    }

    /// Every walked file and the first document ID, in path order.
    fn files_and_first_id(&self) -> (Vec<String>, String) {
        let config = self.config();
        let tree = WorkingTree::new(&self.root, &config.paths).expect("the working tree");
        let input = check_input(&tree, &config.scheme);
        let paths: Vec<String> = input.files.iter().map(|file| file.path.clone()).collect();
        let id = input
            .files
            .iter()
            .filter_map(|file| file.parsed.as_ref())
            .find_map(|parsed| parsed.document().and_then(|node| node.id.clone()))
            .expect("premise: a document with an ID");
        (paths, id)
    }

    fn index(&self) {
        crate::index(&self.env, &self.globals, &IndexRequest { full: false }).expect("spec index");
    }

    fn search(&self, term: &str) -> String {
        let outcome = crate::search(
            &self.env,
            &self.globals,
            &SearchRequest {
                terms: vec![term.to_owned()],
                kinds: Vec::new(),
                limit: None,
                archive: false,
            },
        )
        .expect("search");
        render_text(&Outcome::Search(outcome))
    }

    fn show(&self, reference: &str) -> String {
        let outcome = crate::show(
            &self.env,
            &self.globals,
            &ShowRequest {
                reference: reference.to_owned(),
                links: false,
                archive: false,
                bindings: false,
                git: self.git.clone(),
            },
        )
        .expect("show");
        render_text(&Outcome::Show(outcome))
    }
}

/// The counters cleared, the hook and the forced walk off.
fn clean_seams() {
    seams::set_read_hook(None);
    seams::set_force_walk(false);
    let _ = seams::take_read_counts();
}

// ------------------------------------------------------------------ AC-8 (a)

#[test]
fn ac8a_a_warm_search_and_show_read_no_file_in_their_freshness_step() {
    let place = Place::new("spec-a");
    let (files, id) = place.files_and_first_id();
    std::thread::sleep(PAST_RACY);
    place.index();
    clean_seams();

    let _ = place.search("the");
    let counts = seams::take_read_counts();
    assert_eq!(
        counts.store_reads,
        Vec::<String>::new(),
        "a warm search's freshness step reads no file"
    );
    assert!(
        counts.stats >= files.len(),
        "premise: the step asks `Listed` for every file's key: {} of {} files",
        counts.stats,
        files.len()
    );

    let shown = place.show(&id);
    assert!(shown.contains(&id), "premise: show prints {id}\n{shown}");
    let counts = seams::take_read_counts();
    assert_eq!(
        counts.store_reads,
        Vec::<String>::new(),
        "a warm show's freshness step reads no file"
    );
    assert!(counts.stats > 0, "premise: the step asks `Listed` for keys");

    // The counter sees a read: one edited file is the next step's one read.
    let edited = &files[files.len() / 2];
    let path = place.root.join(edited);
    let mut bytes = fs::read(&path).expect("a walked file");
    bytes.extend_from_slice(b"\nOne more line.\n");
    fs::write(&path, bytes).expect("an edit");
    let _ = place.search("the");
    let counts = seams::take_read_counts();
    assert_eq!(
        counts.store_reads,
        std::slice::from_ref(edited),
        "the edited file alone is read"
    );
    clean_seams();
}

// ------------------------------------------------------------------ AC-6 (f)

/// `post_commit_update` over a source rooted elsewhere: the store refuses
/// it (a root mismatch), so the update fails as an injected error.
fn failed_update(place: &Place, after: After) -> Option<Message> {
    let project = crate::discover(&place.env, &place.globals).expect("the project");
    let mut open = crate::open_index(&place.env, &project).expect("the index");
    let elsewhere =
        WorkingTree::new(&place.other, &project.config.paths).expect("another working tree");
    let (files, _) = place.files_and_first_id();
    post_commit_update(
        &mut open.index,
        &elsewhere,
        &project.config.scheme,
        &[files[0].as_str()],
        after,
    )
}

/// The warning's text, split around the store's error: today's sentence.
fn assert_todays_warning(message: Option<Message>, after: &str) {
    let Some(Message::Warning(text)) = message else {
        panic!("a failed post-commit update gives a warning: {message:?}");
    };
    let head = format!("the index was not updated after the {after}: ");
    assert!(
        text.starts_with(&head) && text.ends_with("; the next command updates it"),
        "today's warning: {text}"
    );
    assert!(
        text.len() > head.len() + "; the next command updates it".len(),
        "the warning quotes the store's error: {text}"
    );
}

#[test]
fn ac6f_a_failed_post_commit_update_warns_and_doubts_its_root() {
    let place = Place::new("spec-a");
    place.index();
    let trust = IndexTrust::new(place.root.clone());

    // Outside a scope: the warning, and no mark anywhere.
    assert_todays_warning(failed_update(&place, After::Commit), "commit");
    assert!(!trust.is_dirty(), "a doubt outside a scope marks nothing");

    trusted(&[&trust], || {
        let _ = place.search("the");
    });
    assert_eq!(trust.walks(), 1, "the trust's first read walks");
    trusted(&[&trust], || {
        let _ = place.search("the");
    });
    assert_eq!(trust.walks(), 1, "premise: the next read is trusted");

    // Inside the scope: the same warning, and the root is doubted.
    let message = trusted(&[&trust], || failed_update(&place, After::Commit));
    assert_todays_warning(message, "commit");
    assert!(
        trust.is_dirty(),
        "a failed post-commit update doubts its root"
    );
    trusted(&[&trust], || {
        let _ = place.search("the");
    });
    assert_eq!(trust.walks(), 2, "the next read walks");
    assert!(!trust.is_dirty(), "the walk clears the mark");

    // After a completion, its own sentence.
    let message = trusted(&[&trust], || failed_update(&place, After::Completion));
    assert_todays_warning(message, "completion");
    assert!(
        trust.is_dirty(),
        "a failed update after a completion doubts too"
    );

    // A trust of another root is not doubted.
    let other = IndexTrust::new(place.other.clone());
    let message = trusted(&[&other], || failed_update(&place, After::Commit));
    assert_todays_warning(message, "commit");
    assert!(!other.is_dirty(), "only the update's own root is doubted");
}

// ------------------------------------------------------------------ D8, the walk's books

/// A walk begun in a scope and dropped without its end (a panic in the
/// update) marks the trust dirty, so the next read walks again; one that
/// ends well clears the mark and counts once.
#[test]
fn d8_a_walk_dropped_without_its_end_doubts_its_trust() {
    let place = Place::new("spec-a");
    let trust = IndexTrust::new(place.root.clone());
    trusted(&[&trust], || {
        let walk = crate::freshness::Walk::begin(&place.root).expect("the trust matches");
        walk.end(true);
    });
    assert_eq!(trust.walks(), 1, "a walk is counted as it begins");
    assert!(!trust.is_dirty(), "a walk that ends well leaves no mark");
    trusted(&[&trust], || {
        let walk = crate::freshness::Walk::begin(&place.root).expect("the trust matches");
        drop(walk);
    });
    assert_eq!(trust.walks(), 2);
    assert!(trust.is_dirty(), "a walk dropped without its end doubts");
    assert!(
        crate::freshness::Walk::begin(&place.root).is_none(),
        "outside a scope no walk keeps books"
    );
}
