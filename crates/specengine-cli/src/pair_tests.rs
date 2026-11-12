//! The collision-pairs task spec, PP6, the CLI unit half: a review resolves
//! targets only for a pair, counted behind the `#[cfg(test)]` counter of
//! `crate::subset::seams` (`pair_targets`).
//!
//! - `spec review` of a two-target `contradicts` discrepancy resolves its
//!   two targets (2), the brief and the full review alike, and serves a
//!   pair; a question on two targets, an update, a one-target and a
//!   three-target `contradicts` and a two-target `missing` resolve none (0)
//!   and serve `pair` null. M: targets resolved for every item.
//!
//! Every case works on a scratch repository written here under
//! `std::env::temp_dir()`, with a scratch `HOME`, removed on drop; its git
//! setup runs with no global or system config and a fixed identity. This
//! file is a CLI source, so the genre rule (`tests/genre.rs`) holds for it:
//! the corpus's words (a lamp's wick) are invented here and its root is
//! `spec`.

use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::json;
use specengine_core::intake::DiscrepancyInput;
use specengine_store::GitEnv;

use crate::subset::seams;
use crate::{
    DiscrepancyRequest, Env, Globals, ProposeRequest, ProposedText, QuestionRequest, ReviewRequest,
    ShowRequest, propose, propose_discrepancy, propose_question, review, review_brief, show,
};

static NEXT: AtomicUsize = AtomicUsize::new(0);

const NOW: &str = "2026-03-01T09:00:00Z";

const CONFIG: &str = r#"[project]
slug = "lamp-ledger"

[paths]
roots = ["spec"]
spec = "spec"

[ids]
WCK = { kind = "wick", shape = "name" }
"#;

/// The four documents: `(path, ID, title)`.
const WICKS: [(&str, &str, &str); 4] = [
    ("spec/wicks/wck-oil.md", "WCK-OIL", "Oil feed"),
    ("spec/wicks/wck-flame.md", "WCK-FLAME", "Flame height"),
    ("spec/wicks/wck-glass.md", "WCK-GLASS", "Chimney glass"),
    ("spec/wicks/wck-soot.md", "WCK-SOOT", "Soot trim"),
];

/// The scratch repository, its `HOME` and the reads' environment.
struct Place {
    dir: PathBuf,
    root: PathBuf,
    home: PathBuf,
    env: Env,
    globals: Globals,
}

impl Drop for Place {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

impl Place {
    fn new(label: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "specengine-pair-tests-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&dir);
        let root = dir.join("repo");
        let home = dir.join("home");
        fs::create_dir_all(&root).expect("the root");
        fs::create_dir_all(&home).expect("a home");
        let place = Self {
            env: Env {
                cwd: root.clone(),
                home: Some(home.clone().into_os_string()),
                xdg_data_home: None,
            },
            globals: Globals::default(),
            dir,
            root,
            home,
        };
        place.git(&["init", "-q", "--template=", "-b", "trunk"]);
        place.put("specengine.toml", CONFIG);
        for (path, id, title) in WICKS {
            place.put(
                path,
                &format!(
                    "---\nid: {id}\nclass: canon\ntitle: {title}\n---\n\n# {title}\n\nThe \
                     {title} of the lamp, as kept.\n"
                ),
            );
        }
        place.git(&["add", "-A"]);
        place.git(&["commit", "-q", "--no-verify", "-m", "the wicks"]);
        place
    }

    fn put(&self, relative: &str, text: &str) {
        let path = self.root.join(relative);
        fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
        fs::write(path, text).expect("a file");
    }

    /// The process environment git and the reads take: no global or system
    /// config, a fixed identity, this `HOME`.
    fn vars(&self) -> Vec<(OsString, OsString)> {
        let mut vars: Vec<(OsString, OsString)> = [
            ("GIT_CONFIG_NOSYSTEM", "1"),
            ("GIT_CONFIG_GLOBAL", "/dev/null"),
            ("GIT_AUTHOR_NAME", "Wick Keeper"),
            ("GIT_AUTHOR_EMAIL", "wick@example.invalid"),
            ("GIT_COMMITTER_NAME", "Wick Keeper"),
            ("GIT_COMMITTER_EMAIL", "wick@example.invalid"),
        ]
        .iter()
        .map(|(name, value)| ((*name).into(), (*value).into()))
        .collect();
        vars.push(("HOME".into(), self.home.as_os_str().to_owned()));
        if let Some(path) = std::env::var_os("PATH") {
            vars.push(("PATH".into(), path));
        }
        vars
    }

    fn git(&self, args: &[&str]) {
        let status = Command::new("git")
            .env_clear()
            .envs(self.vars())
            .current_dir(&self.root)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .status()
            .expect("git runs");
        assert!(status.success(), "git {args:?}");
    }

    fn git_env(&self) -> GitEnv {
        GitEnv::new(&self.root, self.vars())
    }

    /// A discrepancy of `gap` on `targets`, its evidence citing the first
    /// two wicks' files and a code file; stored, its hits named.
    fn discrepancy(&self, targets: &[&str], gap: &str, summary: &str) -> String {
        let mut document = json!({
            "node_ids": targets, "gap_type": gap, "severity": "normal", "summary": summary,
            "evidence": [
                {"file": WICKS[0].0, "observed": "the oil runs dry", "documented": "it never does"},
                {"file": WICKS[1].0, "observed": "the flame climbs", "documented": "it holds"},
                {"file": "src/lamp.rs", "observed": "no trim", "documented": "a trim"}],
            "options": [{"label": "trim", "effect": "a lower flame", "price": "one rule"},
                {"label": "feed", "effect": "more oil", "price": "another rule"}],
            "recommendation": 0
        });
        for _ in 0..2 {
            let input: DiscrepancyInput =
                serde_json::from_value(document.clone()).expect("the input");
            let outcome = propose_discrepancy(
                &self.env,
                &self.globals,
                &DiscrepancyRequest {
                    input,
                    card: None,
                    author_role: Some("developer".to_owned()),
                    author_model: None,
                    run: None,
                    login: None,
                    now: NOW.to_owned(),
                    git: self.git_env(),
                },
            )
            .expect("propose discrepancy");
            if outcome.document.created
                && let Some(id) = outcome.document.id.clone()
            {
                return id;
            }
            let hits: Vec<String> = outcome
                .document
                .hits
                .iter()
                .filter_map(|hit| hit.id.clone())
                .collect();
            assert!(
                !hits.is_empty(),
                "not stored, no hit: {:?}",
                outcome.document
            );
            document["distinct_from"] = json!(hits);
        }
        panic!("{summary}: not stored with its hits named");
    }

    /// A question on `targets`, stored, its hits named.
    fn question(&self, targets: &[&str]) -> String {
        let mut distinct = Vec::new();
        for _ in 0..2 {
            let outcome = propose_question(
                &self.env,
                &self.globals,
                &QuestionRequest {
                    node_ids: targets.iter().map(|id| (*id).to_owned()).collect(),
                    text: "Does the flame need a trim before the oil runs low?".to_owned(),
                    working_answer: "yes".to_owned(),
                    price_of_other: "a rule rewritten".to_owned(),
                    gist: None,
                    matters: None,
                    options: None,
                    recommendation: None,
                    severity: None,
                    distinct_from: distinct.clone(),
                    card: None,
                    author_role: Some("developer".to_owned()),
                    author_model: None,
                    run: None,
                    login: None,
                    now: NOW.to_owned(),
                    git: self.git_env(),
                },
            )
            .expect("propose question");
            if outcome.document.created
                && let Some(id) = outcome.document.id.clone()
            {
                return id;
            }
            distinct = outcome
                .document
                .hits
                .iter()
                .filter_map(|hit| hit.id.clone())
                .collect();
        }
        panic!("the question: not stored with its hits named");
    }

    /// An update of `id`'s span, its body reworded.
    fn update(&self, id: &str) -> String {
        let shown = show(
            &self.env,
            &self.globals,
            &ShowRequest {
                reference: id.to_owned(),
                links: false,
                archive: false,
                bindings: false,
                history: false,
                git: self.git_env(),
            },
        )
        .expect("show");
        let node = shown.nodes.into_iter().next().expect("one node");
        let text = node.text.replacen("as kept", "as trimmed", 1);
        assert_ne!(text, node.text, "the body reworded");
        let outcome = propose(
            &self.env,
            &self.globals,
            &ProposeRequest {
                target: id.to_owned(),
                base: node.span_hash,
                text: ProposedText::Given(text.into_bytes()),
                rationale: "Reword the wick.".to_owned(),
                author_role: Some("spec-writer".to_owned()),
                author_model: None,
                run: None,
                login: None,
                now: NOW.to_owned(),
                git: self.git_env(),
            },
        )
        .expect("propose");
        outcome.document.id.expect("an update stored")
    }

    /// `spec review ID` (`brief` for `--brief`): the targets resolved and
    /// whether a pair was served.
    fn counted(&self, id: &str, brief: bool) -> (usize, bool) {
        let request = ReviewRequest {
            id: id.to_owned(),
            git: self.git_env(),
        };
        let _ = seams::take_read_counts();
        let outcome = if brief {
            review_brief(&self.env, &self.globals, &request)
        } else {
            review(&self.env, &self.globals, &request)
        }
        .expect("review");
        let counts = seams::take_read_counts();
        (counts.pair_targets, outcome.document.pair.is_some())
    }
}

/// PP6: a pair's review resolves its two targets; every other item's none.
/// M: targets resolved for every item.
#[test]
fn pp6_only_a_pair_resolves_its_targets() {
    let place = Place::new("pp6");
    let pair = place.discrepancy(
        &["WCK-OIL", "WCK-FLAME"],
        "contradicts",
        "The oil feed and the flame height disagree",
    );
    let one = place.discrepancy(
        &["WCK-GLASS"],
        "contradicts",
        "The chimney glass contradicts the lamp",
    );
    let three = place.discrepancy(
        &["WCK-OIL", "WCK-FLAME", "WCK-GLASS"],
        "contradicts",
        "Three wick pages disagree",
    );
    let missing = place.discrepancy(
        &["WCK-GLASS", "WCK-SOOT"],
        "missing",
        "Neither page says when soot is trimmed",
    );
    let question = place.question(&["WCK-SOOT", "WCK-OIL"]);
    let update = place.update("WCK-SOOT");
    assert_eq!(place.counted(&pair, false), (2, true), "the pair");
    assert_eq!(place.counted(&pair, true), (2, true), "the pair, brief");
    for (label, id) in [
        ("a question", &question),
        ("an update", &update),
        ("one target", &one),
        ("three targets", &three),
        ("missing", &missing),
    ] {
        assert_eq!(place.counted(id, false), (0, false), "{label}");
        assert_eq!(place.counted(id, true), (0, false), "{label}, brief");
    }
}
