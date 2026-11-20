//! docs/features/department-practices.md (RM-0128) "Acceptance criteria",
//! the scene every practices test reads: a git copy of
//! `fixtures/studio-lamp` (its `gamedev` flows list the disciplines `art`,
//! `development` and `sound`; the fixture itself is only read) with the
//! spec's changes:
//!
//! - `PRA = { kind = "practice", shape = "name" }` appended to `[ids]`;
//! - the practices under the default walk, `docs/spec/practices/<trade>/`
//!   (the orchestrator's ruling of 2026-11-20: `docs/practices/` is no
//!   default root; the spec's `docs/practices/<x>.md` read so):
//!   - `art/models.md` (`PRA-ART-MODELS`, `disciplines: [art]`, rules
//!     `PRA-ART-REALISM`, `PRA-ART-NORMALS`, `PRA-ART-LOD`), the spec's
//!     `art.md`;
//!   - `development/code.md` (`PRA-DEV-CODE`, `[development]`, two rules),
//!     the spec's `dev.md`;
//!   - `shared/naming.md` (`PRA-SHARED`, `[art, development]`, one rule,
//!     `PRA-SHARED-NAMING`), the spec's `shared.md`;
//!   - `puppetry/ghost.md` (`PRA-GHOST`, `[puppetry]`, one rule);
//!   - `art/2025-photoreal.md`, a superseded decision (Tier 3) with
//!     `disciplines: [art]` and one rule: it sorts before `art/models.md`,
//!     so a Tier 3 practice let in would lead layer 10;
//! - the queue, through the library at fixed clocks ([`Scene::with_queue`]):
//!   tasks `T-0001` (on `SCN-ARRIVAL`) and `T-0002` (on `SCN-STORM`), cards
//!   `C-0001` (`art`, target `SCN-ARRIVAL`, its goal citing `PRA-ART-LOD`
//!   and `PRA-NOPE-X`, reviewer `Ana Ruiz`), `C-0002` (`development`) and
//!   `C-0003` (`sound`); `C-0001` and `C-0002` list `T-0001`, none lists
//!   `T-0002`.
//!
//! Every commit is the sandbox's fixed author and committer at its fixed
//! date, so two scenes hold one history.
//!
//! `specengine-mcp`'s and `specengine-http`'s `tests/practices.rs` include
//! this file by path beside `common/git.rs` as their `git` module, so it
//! names nothing of the CLI's test `common` and runs no binary.
//!
//! Every name is invented. Non-Latin characters in Rust sources are Unicode
//! escapes (ADR-0024).

#![allow(dead_code)]

use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use specengine_cli::{
    CardEditRequest, CardNewRequest, Env, Exit, GitEnv, Globals, ProposeRequest, ProposedText,
    ShowRequest, TaskNewRequest, card_edit, card_new, propose, show, task_new,
};

use crate::git::Sandbox;

/// studio-lamp's `[project] slug`.
pub const SLUG: &str = "studio-lamp";

/// Where the scene's practices live: under the default walk.
pub const PRACTICES_DIR: &str = "docs/spec/practices";
pub const ART: &str = "docs/spec/practices/art/models.md";
pub const DEV: &str = "docs/spec/practices/development/code.md";
pub const SHARED: &str = "docs/spec/practices/shared/naming.md";
pub const GHOST: &str = "docs/spec/practices/puppetry/ghost.md";
/// The superseded decision.
pub const OLD: &str = "docs/spec/practices/art/2025-photoreal.md";
/// Every practice file of the scene, by path.
pub const PRACTICE_FILES: [&str; 5] = [OLD, ART, DEV, GHOST, SHARED];

pub const ART_ID: &str = "PRA-ART-MODELS";
pub const ART_TITLE: &str = "3D models: agreed rules";
pub const REALISM: &str = "PRA-ART-REALISM";
pub const NORMALS: &str = "PRA-ART-NORMALS";
pub const LOD: &str = "PRA-ART-LOD";
pub const LOD_TITLE: &str = "Level of detail";
pub const DEV_ID: &str = "PRA-DEV-CODE";
pub const DEV_TITLE: &str = "Code: agreed rules";
pub const DEV_CITE: &str = "PRA-DEV-CITE";
pub const DEV_STATE: &str = "PRA-DEV-STATE";
pub const SHARED_ID: &str = "PRA-SHARED";
pub const SHARED_TITLE: &str = "Shared naming rules";
pub const NAMING: &str = "PRA-SHARED-NAMING";
pub const GHOST_ID: &str = "PRA-GHOST";
pub const GHOST_RULE: &str = "PRA-GHOST-STRINGS";
pub const OLD_ID: &str = "PRA-ART-OLD";
pub const OLD_RULE: &str = "PRA-ART-PHOTO";
/// An ID of the declared prefix that names nothing.
pub const NOPE: &str = "PRA-NOPE-X";

/// The `[ids]` line the scene adds.
pub const PRA_LINE: &str = "PRA = { kind = \"practice\", shape = \"name\" }\n";

/// `art.md`'s `disciplines:` line, as written.
pub const ART_DISCIPLINES: &str = "disciplines: [art]\n";

pub const ART_TEXT: &str = "---
id: PRA-ART-MODELS
class: canon
tier: 2
scope: [practices]
owner: art-lead
reviewed: 2026-11-20
disciplines: [art]
---

# 3D models: agreed rules

The art department's agreed rules for every model of the lighthouse.

## Realism {#PRA-ART-REALISM}

Stylised, not photoreal: a shape reads at a glance from the keeper's distance.

## Normal maps {#PRA-ART-NORMALS}

Every hero prop carries a baked normal map; small props share one atlas.

## Level of detail {#PRA-ART-LOD}

Three levels for anything the camera passes; the last under 500 triangles.
";

pub const DEV_TEXT: &str = "---
id: PRA-DEV-CODE
class: canon
tier: 2
owner: dev-lead
reviewed: 2026-11-20
disciplines: [development]
---

# Code: agreed rules

The programmers' agreed rules.

## A change cites its rule {#PRA-DEV-CITE}

A commit that follows a rule names its ID in its message.

## No hidden global state {#PRA-DEV-STATE}

Each system reads what it needs from its arguments.
";

pub const SHARED_TEXT: &str = "---
id: PRA-SHARED
class: canon
tier: 2
owner: owner
reviewed: 2026-11-20
disciplines: [art, development]
---

# Shared naming rules

What the artists and the programmers both keep.

## Asset names {#PRA-SHARED-NAMING}

An asset's file name is its scene's ID, then its own word.
";

pub const GHOST_TEXT: &str = "---
id: PRA-GHOST
class: canon
tier: 2
owner: owner
reviewed: 2026-11-20
disciplines: [puppetry]
---

# Puppet rules

No flow of this project lists the discipline.

## Strings {#PRA-GHOST-STRINGS}

Every puppet hangs from three strings.
";

pub const OLD_TEXT: &str = "---
id: PRA-ART-OLD
class: decision
status: superseded-by PRA-ART-MODELS
date: 2025-06-01
disciplines: [art]
---

# Photoreal models

Superseded by the stylised rules.

## Photoreal {#PRA-ART-PHOTO}

Every model aims at a photograph.
";

/// The clock of every card and task write.
pub const CLOCK: &str = "2026-11-20T09:00:00Z";
/// The clock of every raise.
pub const RAISED: &str = "2026-11-20T08:00:00Z";

/// C-0001's lines.
pub const C1_TITLE: &str = "Model the arrival's rowing boat";
pub const C1_GOAL: &str =
    "Model the boat that runs aground to PRA-ART-LOD; the old PRA-NOPE-X sketch is gone.";
pub const C1_CRITERION: &str = "The boat reads at the keeper's distance.";
pub const REVIEWER: &str = "Ana Ruiz";
/// C-0002's and C-0003's titles.
pub const C2_TITLE: &str = "Script the boat's landing";
pub const C3_TITLE: &str = "Record the surf";

/// The rationale of the proposal of DP13.
pub const RATIONALE: &str = "Keeps PRA-ART-NORMALS for the hull; PRA-NOPE-X no longer applies.";

/// `fixtures/<name>` of this repository.
pub fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name)
}

/// Every file under `dir`, root-relative with `/`, and its bytes.
fn files_under(root: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>)>) {
    for entry in fs::read_dir(dir).expect("a readable fixture").flatten() {
        let path = entry.path();
        if path.is_dir() {
            files_under(root, &path, out);
        } else {
            let relative = path
                .strip_prefix(root)
                .expect("under the root")
                .to_str()
                .expect("a UTF-8 path")
                .replace('\\', "/");
            out.push((relative, fs::read(&path).expect("a readable file")));
        }
    }
}

/// `text` with its one `from` replaced by `to`.
pub fn once(text: &str, from: &str, to: &str, what: &str) -> String {
    assert_eq!(text.matches(from).count(), 1, "{what}: {from:?}");
    text.replacen(from, to, 1)
}

/// `config` with `line` (LF-ended) as the last key line of its `[ids]`
/// table, as `common::with_ids_line` places it.
pub fn with_ids_line(config: &str, line: &str) -> String {
    let lines: Vec<&str> = config.split_inclusive('\n').collect();
    let header = lines
        .iter()
        .position(|text| text.trim() == "[ids]")
        .expect("the config has an `[ids]` table");
    let last_key = lines[header + 1..]
        .iter()
        .take_while(|text| !text.trim_start().starts_with('['))
        .enumerate()
        .filter(|(_, text)| {
            let text = text.trim();
            !text.is_empty() && !text.starts_with('#')
        })
        .map(|(offset, _)| header + 1 + offset)
        .last()
        .unwrap_or(header);
    let mut out = String::new();
    for (index, text) in lines.iter().enumerate() {
        out.push_str(text);
        if index == last_key {
            if !text.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(line);
        }
    }
    out
}

/// The scene's config: studio-lamp's with [`PRA_LINE`] in `[ids]`.
pub fn scene_config() -> String {
    let fixture = fs::read_to_string(fixture("studio-lamp").join("specengine.toml"))
        .expect("studio-lamp's config");
    with_ids_line(&fixture, PRA_LINE)
}

/// The scene's files and bytes, sorted by path: studio-lamp with the
/// spec's changes.
pub fn scene_files() -> Vec<(String, Vec<u8>)> {
    let source = fixture("studio-lamp");
    let mut files = Vec::new();
    files_under(&source, &source, &mut files);
    for (path, bytes) in &mut files {
        if path == "specengine.toml" {
            *bytes = scene_config().into_bytes();
        }
    }
    for (path, text) in [
        (ART, ART_TEXT),
        (DEV, DEV_TEXT),
        (SHARED, SHARED_TEXT),
        (GHOST, GHOST_TEXT),
        (OLD, OLD_TEXT),
    ] {
        files.push((path.to_owned(), text.as_bytes().to_vec()));
    }
    files.sort();
    files
}

/// The 1-based line of `text` that is exactly `line`.
pub fn line_of(text: &str, line: &str) -> u64 {
    let at = text
        .lines()
        .position(|candidate| candidate == line)
        .unwrap_or_else(|| panic!("no line {line:?}"));
    u64::try_from(at + 1).expect("a line number")
}

/// About `chars` characters of plain prose marked with `mark`, sentence by
/// sentence, so no two rules share a sentence.
pub fn prose(mark: &str, chars: usize) -> String {
    let mut out = String::new();
    let mut n = 0;
    while out.len() < chars {
        n += 1;
        out.push_str(&format!(
            "The {mark} keeper trims wick number {n} before the tide turns, and the lamp \
             burns steady through watch {n}.\n"
        ));
    }
    out
}

/// A practice document `id` of `disciplines` whose rules are `rules`
/// (`(id, title, body)`), titled `title`.
pub fn practice_text(
    id: &str,
    title: &str,
    disciplines: &[&str],
    rules: &[(String, String, String)],
) -> String {
    let mut text = format!(
        "---\nid: {id}\nclass: canon\ntier: 2\nowner: owner\nreviewed: 2026-11-20\n\
         disciplines: [{}]\n---\n\n# {title}\n\nA practice of the scene.\n",
        disciplines.join(", ")
    );
    for (rule, heading, body) in rules {
        text.push_str(&format!("\n## {heading} {{#{rule}}}\n\n{body}"));
        if !body.ends_with('\n') {
            text.push('\n');
        }
    }
    text
}

static NEXT: AtomicUsize = AtomicUsize::new(0);

/// The scene's repository and its `HOME`, removed on drop.
pub struct Scene {
    dir: PathBuf,
    pub git: Sandbox,
    pub root: PathBuf,
    pub home: PathBuf,
}

impl Drop for Scene {
    fn drop(&mut self) {
        restore_permissions(&self.dir);
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn restore_permissions(dir: &Path) {
    let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o755));
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            restore_permissions(&path);
        } else {
            let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o644));
        }
    }
}

impl Scene {
    /// The scene's files, committed, its queue made.
    pub fn new(label: &str) -> Self {
        let scene = Self::files(label, false);
        scene.with_queue();
        scene
    }

    /// The scene's files, written in path order or `reversed`, committed;
    /// no queue.
    pub fn files(label: &str, reversed: bool) -> Self {
        Self::files_with(label, reversed, |_, _, _| {})
    }

    /// [`Scene::files`] with `after(root, home, n)` called after the `n`th
    /// file is written (before `git init`).
    pub fn files_with(
        label: &str,
        reversed: bool,
        mut after: impl FnMut(&Path, &Path, usize),
    ) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "specengine-dp-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("a scratch directory");
        let dir = fs::canonicalize(&dir).expect("canonical");
        let git = Sandbox::new(&dir);
        let root = dir.join("repo");
        fs::create_dir_all(&root).expect("the root");
        let home = dir.join("h");
        fs::create_dir_all(&home).expect("a home");
        let mut files = scene_files();
        if reversed {
            files.reverse();
        }
        for (n, (path, bytes)) in files.iter().enumerate() {
            let target = root.join(path);
            fs::create_dir_all(target.parent().expect("a parent")).expect("a directory");
            fs::write(&target, bytes).expect("a file");
            after(&root, &home, n + 1);
        }
        git.init(&root);
        git.add_all(&root);
        git.commit(&root, "The scene");
        Self {
            dir,
            git,
            root,
            home,
        }
    }

    /// The scratch directory every repository of the test lives in.
    pub fn scratch(&self) -> &Path {
        &self.dir
    }

    /// A fresh directory in the scratch.
    pub fn dir(&self, name: &str) -> PathBuf {
        let path = self.dir.join(name);
        fs::create_dir_all(&path).expect("a directory");
        fs::canonicalize(&path).expect("canonical")
    }

    pub fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    pub fn put(&self, relative: &str, text: &str) {
        let path = self.root.join(relative);
        fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
        fs::write(path, text).expect("a file");
    }

    pub fn read(&self, relative: &str) -> String {
        fs::read_to_string(self.root.join(relative)).expect("a readable file")
    }

    /// `relative` with its one `from` replaced by `to`.
    pub fn edit(&self, relative: &str, from: &str, to: &str) {
        let text = self.read(relative);
        self.put(relative, &once(&text, from, to, relative));
    }

    pub fn append(&self, relative: &str, text: &str) {
        let mut whole = self.read(relative);
        whole.push_str(text);
        self.put(relative, &whole);
    }

    pub fn remove(&self, relative: &str) {
        fs::remove_file(self.root.join(relative)).expect("a file removed");
    }

    /// `git add -A` and a commit of `message`.
    pub fn commit(&self, message: &str) {
        self.git.add_all(&self.root);
        self.git.commit(&self.root, message);
    }

    /// The library's environment: the root, the scene's `HOME`.
    pub fn env(&self) -> Env {
        Env {
            cwd: self.root.clone(),
            home: Some(self.home.clone().into_os_string()),
            xdg_data_home: None,
        }
    }

    /// The caller's git: the sandbox's variables.
    pub fn git_env(&self) -> GitEnv {
        GitEnv::new(self.root.clone(), self.git.vars())
    }

    /// The variables a `spec` process runs with: the sandbox's, `HOME` the
    /// scene's (or `home`).
    pub fn vars_with(&self, home: &Path) -> Vec<(String, OsString)> {
        let mut vars: Vec<(String, OsString)> = self
            .git
            .vars()
            .into_iter()
            .map(|(name, value)| (name.to_string_lossy().into_owned(), value))
            .filter(|(name, _)| name != "HOME")
            .collect();
        vars.push(("HOME".to_owned(), home.as_os_str().to_owned()));
        vars
    }

    pub fn vars(&self) -> Vec<(String, OsString)> {
        self.vars_with(&self.home)
    }

    /// A `draft` task on `nodes` titled `title`, at [`CLOCK`]: its ID.
    pub fn task(&self, nodes: &[&str], title: &str) -> String {
        let outcome = task_new(
            &self.env(),
            &Globals::default(),
            &TaskNewRequest {
                nodes: nodes.iter().map(|node| (*node).to_owned()).collect(),
                title: Some(title.to_owned()),
                goal: Some("Make it so.".to_owned()),
                author_role: None,
                author_model: None,
                run: None,
                now: CLOCK.to_owned(),
                git: self.git_env(),
            },
        )
        .unwrap_or_else(|error| panic!("task new {nodes:?}: {error}"));
        assert_eq!(outcome.exit(), Exit::Answered, "task new: {outcome:?}");
        outcome.id.expect("a task's ID")
    }

    /// A card of `discipline` at [`CLOCK`]: its ID.
    pub fn card(
        &self,
        title: &str,
        discipline: &str,
        targets: &[&str],
        goal: Option<&str>,
        criteria: &[&str],
        reviewer: Option<&str>,
    ) -> String {
        let outcome = card_new(
            &self.env(),
            &Globals::default(),
            &CardNewRequest {
                title: title.to_owned(),
                goal: goal.map(str::to_owned),
                discipline: Some(discipline.to_owned()),
                targets: targets.iter().map(|id| (*id).to_owned()).collect(),
                criteria: criteria.iter().map(|c| (*c).to_owned()).collect(),
                assignee: None,
                reviewer: reviewer.map(str::to_owned),
                mandate: None,
                estimate: None,
                due: None,
                parent: None,
                corrects: None,
                actor: None,
                now: CLOCK.to_owned(),
            },
        )
        .unwrap_or_else(|error| panic!("card new {title}: {error}"));
        assert_eq!(
            outcome.exit(),
            Exit::Answered,
            "card new {title}: {outcome:?}"
        );
        outcome.id.expect("a card's ID")
    }

    /// `card edit <card>` with exactly `edit` applied to an empty request
    /// at [`CLOCK`].
    pub fn edit_card(&self, card: &str, edit: impl FnOnce(&mut CardEditRequest)) {
        let mut request = CardEditRequest {
            id: card.to_owned(),
            title: None,
            goal: None,
            targets: Vec::new(),
            criteria: Vec::new(),
            estimate: None,
            due: None,
            parent: None,
            corrects: None,
            evidence: Vec::new(),
            tasks: Vec::new(),
            mandate: None,
            reason: None,
            revision: None,
            actor: None,
            now: CLOCK.to_owned(),
            tasks_here: true,
        };
        edit(&mut request);
        let outcome = card_edit(&self.env(), &Globals::default(), &request)
            .unwrap_or_else(|error| panic!("card edit {card}: {error}"));
        assert_eq!(
            outcome.exit(),
            Exit::Answered,
            "card edit {card}: {outcome:?}"
        );
    }

    /// `card edit <card> --task T…` at [`CLOCK`].
    pub fn link(&self, card: &str, tasks: &[&str]) {
        self.edit_card(card, |request| {
            request.tasks = tasks.iter().map(|task| (*task).to_owned()).collect();
        });
    }

    /// The scene's queue (see the module documentation).
    pub fn with_queue(&self) {
        assert_eq!(self.task(&["SCN-ARRIVAL"], "Land the boat"), "T-0001");
        assert_eq!(self.task(&["SCN-STORM"], "Douse the lantern"), "T-0002");
        let c1 = self.card(
            C1_TITLE,
            "art",
            &["SCN-ARRIVAL"],
            Some(C1_GOAL),
            &[C1_CRITERION],
            Some(REVIEWER),
        );
        assert_eq!(c1, "C-0001");
        let c2 = self.card(
            C2_TITLE,
            "development",
            &["SCN-ARRIVAL"],
            Some("The boat lands on the shingle when the tide allows."),
            &[],
            Some(REVIEWER),
        );
        assert_eq!(c2, "C-0002");
        let c3 = self.card(C3_TITLE, "sound", &["SCN-STORM"], None, &[], None);
        assert_eq!(c3, "C-0003");
        self.link("C-0001", &["T-0001"]);
        self.link("C-0002", &["T-0001"]);
    }

    /// `(span_hash, text)` of `reference` as `spec show` gives them.
    pub fn span(&self, reference: &str) -> (String, String) {
        let outcome = show(
            &self.env(),
            &Globals::default(),
            &ShowRequest {
                reference: reference.to_owned(),
                links: false,
                archive: false,
                bindings: false,
                history: false,
                git: self.git_env(),
            },
        )
        .unwrap_or_else(|error| panic!("show {reference}: {error}"));
        assert_eq!(outcome.nodes.len(), 1, "show {reference}: {outcome:?}");
        let node = outcome.nodes.into_iter().next().expect("one node");
        (node.span_hash, node.text)
    }

    /// An agent's `update` of `target` (its span with `from` replaced by
    /// `to`) raised with `rationale` at [`RAISED`]: its ID.
    pub fn propose_edit(&self, target: &str, from: &str, to: &str, rationale: &str) -> String {
        let (base, text) = self.span(target);
        let outcome = propose(
            &self.env(),
            &Globals::default(),
            &ProposeRequest {
                target: target.to_owned(),
                base,
                text: ProposedText::Given(once(&text, from, to, target).into_bytes()),
                rationale: rationale.to_owned(),
                author_role: Some("spec-writer".to_owned()),
                author_model: Some("claude-opus-5-5".to_owned()),
                run: None,
                login: None,
                now: RAISED.to_owned(),
                git: self.git_env(),
            },
        )
        .unwrap_or_else(|error| panic!("propose {target}: {error}"));
        assert_eq!(
            outcome.exit(),
            Exit::Answered,
            "propose {target}: {outcome:?}"
        );
        outcome.document.id.clone().expect("an ID")
    }

    /// DP13's proposal: an update of `SCN-ARRIVAL` whose rationale cites
    /// `PRA-ART-NORMALS` and `PRA-NOPE-X`.
    pub fn propose_cited(&self) -> String {
        self.propose_edit(
            "SCN-ARRIVAL",
            "runs aground on the shingle",
            "runs hard aground on the shingle",
            RATIONALE,
        )
    }
}
