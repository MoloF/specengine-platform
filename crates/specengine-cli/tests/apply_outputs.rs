//! docs/features/apply-outputs.md (RM-0072, Phase 7 step 10a), the terminal's
//! half: an apply's one commit carries the outputs of the `index = true`
//! `[[generators]]` entry whose bytes it changes, rendered from the committed
//! tree (`HEAD` plus the apply's paths), never the disk walk; a gap commits the
//! target alone with a warning. O1, O3-O8, O11-O14; O2 and O8's hub half are
//! `specengine-http`'s `tests/hub_confirm.rs`, O9, O10 and O15 its
//! `tests/hub_ids.rs`.
//!
//! - O1: a file-form create commits `A <path>` and `M docs/index.md`, a note
//!   naming the output, stdout as before, and leaves no index finding, on two
//!   projects of different nature (ADR-0008). M: skip the render.
//! - O3: an untracked document and an unstaged `title:` edit stay out of the
//!   committed index, which a staged hook passes; a hook exiting 1 restores the
//!   target and the index. M: render from the disk walk; restore only the
//!   target.
//! - O4: no `index = true` entry: the commit as today, no note, the registered
//!   command never run. M: render whenever `[[generators]]` exists; run it.
//! - O5: an update changing no index line commits its path alone and writes no
//!   output. M: stage every output whatever its bytes.
//! - O6: a create's or an update's commit with its output, `approved` left,
//!   completes; one more path reads `changes`. M: keep "only the target's path
//!   changed"; accept any extra path.
//! - O7: a decision record commits `A <record>` beside `M docs/index.md` and an
//!   interrupted one completes. M: keep "name-status exactly `A <path>`".
//! - O8: a root absent from `HEAD`'s tree, a symlinked output: the target
//!   alone, `applied`, the warning naming the cause and `spec export index`. M:
//!   render the partial walk; refuse the apply.
//! - O11 (iteration 2): an ignored untracked output, a case variant on a
//!   case-insensitive file system, an output whose staging fails: the target
//!   alone with the warning, no index entry left. M: refuse at step 9.
//! - O12: `HEAD`'s config naming the update's own target, or `spec.lock`, as a
//!   shard: the target alone, the warning naming the path, both files as the
//!   apply alone would leave them. M: write it.
//! - O13: a hand commit whose own config adds `specengine.toml` or
//!   an ordinary new shard, and one carrying `spec.lock` that its
//!   parent's config names: none completes; without the extra path it does.
//!   M: read the commit's config; accept the four paths as outputs.
//! - O14: a `pre-commit` hook rewriting the staged index: step 10's warning,
//!   still `applied`. M: check names only.
//! - O16 (iteration 3): `HEAD`'s config naming another spec document as a
//!   shard, committed or only on disk: the target alone, the warning `(a spec
//!   document)`, the document's bytes kept; a hand commit editing it under
//!   that parent config completes nothing. M: exclude only the four paths and
//!   the walk scope; judge `HEAD`'s tree alone.
//!
//! Beyond the criteria, from "Rules and edge cases": `HEAD`'s config that does
//! not read commits the target alone with its warning; an index stale at `HEAD`
//! is brought up to date by the next apply's commit; an output `HEAD` lacks is
//! written when its disk bytes read as a generated document, and narrows when
//! they do not read at all; step 9's rollback keeps an entry a person staged
//! meanwhile, after a failed commit and after a failed staging. Not staged:
//! `disk_name` finding no listing entry by name or identity ("cannot tell"),
//! which needs a file system that answers a lookup with no entry of its own.
//!
//! Every repository is a scratch copy of `fixtures/apply-outputs/forge` (an
//! invented smithing game) or `fixtures/apply-outputs/press` (an invented
//! newsletter), its outputs rendered by `spec export index` before the first
//! commit, a linked worktree on `t1` (`common::proposal::Pair`'s shape). The
//! binary raises each create (`spec propose create --text-file -`); approve
//! runs through the library with a consent callback, and its stdout and stderr
//! are what `spec` prints of the outcome.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;

use std::fs;
use std::os::unix::fs::{MetadataExt as _, symlink};
use std::path::Path;

use common::git::Sandbox;
use common::proposal::{DECIDER, LATER, Pair, printed};
use common::{Run, Scratch, read, read_text, replace, repository_root, snapshot, spec, write};
use serde_json::Value;
use specengine_cli::{ApproveFlags, Exit, Outcome, ProposalOutcome};
use specengine_store::ProposalStatus;

/// The rationale of every create.
const RATIONALE: &str = "The smith tires.";

/// The command the fixtures register, named in every message.
const COMMAND: &str = "spec export index";

const INDEX: &str = "docs/index.md";
const ARCHIVE: &str = "docs/index-archive.md";

/// "Data"'s new file in the game.
const STAMINA: &str = "docs/features/stamina.md";
const STAMINA_TEXT: &str = "---\nclass: spec\nstatus: draft\nscope: [forge]\n---\n\n\
     # Smith stamina {#MEC-STAMINA}\n\nEach strike costs the smith one stamina; at zero the \
     hammer stops until the smith rests.\n";

/// The newsletter's new file.
const TIDES: &str = "docs/spec/tides.md";
const TIDES_TEXT: &str = "---\nclass: spec\nstatus: draft\nscope: [weekly]\n---\n\n\
     # The tide table {#SHT-TIDES}\n\nThe back page prints the week's high and low tides.\n";

/// One fixture project and the file a create adds there.
struct Project {
    dir: &'static str,
    slug: &'static str,
    new_path: &'static str,
    new_text: &'static str,
}

const FORGE: Project = Project {
    dir: "forge",
    slug: "ember-forge",
    new_path: STAMINA,
    new_text: STAMINA_TEXT,
};

const PRESS: Project = Project {
    dir: "press",
    slug: "harbour-weekly",
    new_path: TIDES,
    new_text: TIDES_TEXT,
};

// ---------------------------------------------------------------- helpers

/// A scratch repository of `fixtures/apply-outputs/<dir>` whose config is
/// `config` of the fixture's, its outputs rendered by `spec export index` when
/// it registers an `index = true` entry, one commit on `main`, a linked
/// worktree on `t1`.
fn project(label: &str, of: &Project, config: impl FnOnce(String) -> String) -> Pair {
    let scratch = Scratch::new(label);
    let git = Sandbox::new(scratch.path());
    let main = scratch.copy(&format!("apply-outputs/{}", of.dir), "main");
    let text = config(read_text(&main, "specengine.toml"));
    write(&main, "specengine.toml", &text);
    let home = scratch.home("h");
    if text.contains("index   = true") {
        spec(&home, &main, &["export", "index"]).code(0);
    }
    git.init(&main);
    git.add_all(&main);
    git.commit(&main, "the fixture");
    let linked = scratch.join("t1");
    git.git(
        &main,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "t1",
            linked.to_str().expect("a UTF-8 scratch path"),
        ],
    );
    let linked = fs::canonicalize(linked).expect("the linked worktree");
    Pair {
        scratch,
        git,
        main,
        linked,
        home,
        slug: of.slug.to_owned(),
    }
}

/// The game with its config as the fixture holds it.
fn forge(label: &str) -> Pair {
    project(label, &FORGE, |config| config)
}

/// `config` without its `[[generators]]` table and with `entry` in its place.
fn generators_replaced(config: &str, entry: &str) -> String {
    let at = config
        .find("\n[[generators]]\n")
        .expect("the fixture registers its index");
    format!("{}\n{entry}", &config[..at])
}

/// `spec propose create <path> --text-file - --rationale R` in the linked
/// worktree, as an agent raises it: the ID it prints.
fn raise_create(pair: &Pair, path: &str, text: &str) -> String {
    let run = pair.spec_piped(
        &pair.linked,
        &[
            "propose",
            "create",
            path,
            "--text-file",
            "-",
            "--rationale",
            RATIONALE,
        ],
        text.as_bytes(),
    );
    run.code(0);
    let id = run.stdout.lines().next().unwrap_or_default().to_owned();
    assert!(id.starts_with("PR-"), "not stored: {}", run.show());
    id
}

/// `spec propose update <target>` of its span in the linked worktree with
/// `from` become `to`: the ID.
fn raise_update(pair: &Pair, target: &str, from: &str, to: &str) -> String {
    pair.propose_edit(&pair.linked, target, from, to)
}

/// Library approve from the main worktree, consent yes: the outcome (exit 0
/// or 1) and the questions asked.
fn approve(pair: &Pair, id: &str) -> (ProposalOutcome, Vec<String>) {
    let (outcome, questions) = pair.approve_answer(&pair.main, id, true, pair.git_env(&pair.main));
    let outcome = outcome.unwrap_or_else(|error| panic!("approve {id}: {error}"));
    (outcome, questions)
}

/// An approve that must apply.
fn applied(pair: &Pair, id: &str) -> (ProposalOutcome, Vec<String>) {
    let (outcome, questions) = approve(pair, id);
    assert_eq!(outcome.exit(), Exit::Answered, "approve {id}: {outcome:?}");
    (outcome, questions)
}

/// What `spec` prints of `outcome` on stderr.
fn stderr_of(outcome: &ProposalOutcome) -> Vec<String> {
    Outcome::Proposal(Box::new(outcome.clone())).stderr_lines()
}

/// The lines of `lines` about the index's outputs in the commit.
fn carries(lines: &[String]) -> Vec<String> {
    lines
        .iter()
        .filter(|line| line.contains("the commit carries"))
        .cloned()
        .collect()
}

/// "Data"'s note naming the outputs a commit carries.
fn outputs_note(paths: &[&str]) -> String {
    format!(
        "note: the commit carries the index's outputs too: {}",
        paths.join(", ")
    )
}

/// "Data"'s warning of a commit that carries `target` alone.
fn alone_warning(target: &str, cause: &str) -> String {
    format!(
        "warning: the commit carries `{target}` alone: the index's outputs were not rendered: \
         {cause}; `{COMMAND}` writes them"
    )
}

/// `git diff-tree --name-status` of `commit`, in git's path order.
fn name_status(pair: &Pair, commit: &str) -> Vec<String> {
    pair.git_text(
        &pair.main,
        &["diff-tree", "-r", "--no-commit-id", "--name-status", commit],
    )
    .lines()
    .map(str::to_owned)
    .collect()
}

/// `lines` (`<status>\t<path>`) in git's path order.
fn by_path(mut lines: Vec<String>) -> Vec<String> {
    lines.sort_by(|a, b| a.split('\t').nth(1).cmp(&b.split('\t').nth(1)));
    lines
}

/// The blob of `path` at `commit`.
fn blob(pair: &Pair, commit: &str, path: &str) -> Vec<u8> {
    pair.git.git(
        &pair.main,
        &["cat-file", "blob", &format!("{commit}:{path}")],
    )
}

/// `t1`'s tip.
fn tip(pair: &Pair) -> String {
    pair.rev(&pair.linked, "t1")
}

/// `commit`'s parents.
fn parents(pair: &Pair, commit: &str) -> Vec<String> {
    pair.git_text(&pair.main, &["rev-list", "--parents", "-n", "1", commit])
        .split_whitespace()
        .skip(1)
        .map(str::to_owned)
        .collect()
}

/// `git status --porcelain=v1 --untracked-files=all` of the linked worktree.
fn porcelain(pair: &Pair) -> String {
    pair.porcelain(&pair.linked)
}

/// The `index-*` findings of `spec --json check` in `dir`: `(code, path)`.
fn index_findings(pair: &Pair, dir: &Path) -> Vec<(String, String)> {
    let run = spec(&pair.home, dir, &["--json", "check"]);
    let json: Value = serde_json::from_str(&run.stdout)
        .unwrap_or_else(|error| panic!("a JSON report: {error}\n{}", run.show()));
    json["findings"]
        .as_array()
        .unwrap_or_else(|| panic!("findings: {}", run.show()))
        .iter()
        .filter(|f| f["code"].as_str().is_some_and(|c| c.starts_with("index-")))
        .map(|f| {
            (
                f["code"].as_str().unwrap_or_default().to_owned(),
                f["path"].as_str().unwrap_or_default().to_owned(),
            )
        })
        .collect()
}

/// `spec export index` in `dir` reports every output `unchanged`: what is
/// on disk is the render.
#[track_caller]
fn assert_unchanged(pair: &Pair, dir: &Path) {
    let run = spec(&pair.home, dir, &["export", "index"]);
    run.code(0);
    let lines: Vec<&str> = run.stdout.lines().collect();
    assert_eq!(
        lines.len(),
        2,
        "the root and its archive shard: {}",
        run.show()
    );
    assert!(
        lines.iter().all(|line| line.starts_with("unchanged ")),
        "{}",
        run.show()
    );
}

/// A clean clone of `t1` at `<scratch>/<dir>`.
fn clean_clone(pair: &Pair, dir: &str) -> std::path::PathBuf {
    let clone = pair.scratch.join(dir);
    pair.git.git(
        pair.scratch.path(),
        &[
            "clone",
            "-q",
            "-b",
            "t1",
            pair.main.to_str().expect("UTF-8"),
            clone.to_str().expect("UTF-8"),
        ],
    );
    clone
}

/// `body` as the repository's `pre-commit` hook (the common git directory's,
/// so the linked worktree runs it too).
fn hook(pair: &Pair, body: &str) {
    let hooks = pair.main.join(".git/hooks");
    fs::create_dir_all(&hooks).expect("hooks");
    let path = hooks.join("pre-commit");
    fs::write(&path, body).expect("hook");
    let mut mode = fs::metadata(&path).expect("hook").permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut mode, 0o755);
    fs::set_permissions(&path, mode).expect("executable hook");
}

/// `id` approved and nothing applied, as a run killed between steps 7 and 9
/// leaves it.
fn set_approved(pair: &Pair, id: &str) {
    pair.sql(&format!(
        "UPDATE proposals SET status = 'approved', decided_by = '{DECIDER}', decided_at = \
         '{LATER}' WHERE id = '{id}';"
    ));
    assert_eq!(pair.proposal(id).status, ProposalStatus::Approved);
}

/// `id`'s apply set back to `approved` past its commit, as a run killed
/// between steps 9 and 10 leaves it: no `applied_commit`, no `applied` event.
fn rewound_to_approved(pair: &Pair, id: &str) {
    pair.sql(&format!(
        "UPDATE proposals SET status = 'approved', applied_commit = NULL WHERE id = '{id}'; \
         DELETE FROM events WHERE type = 'proposal.applied' AND \
         json_extract(payload, '$.id') = '{id}';"
    ));
    let stored = pair.proposal(id);
    assert_eq!(stored.status, ProposalStatus::Approved);
    assert!(stored.applied_commit.is_none());
}

/// `paths` added and committed by hand on `t1` with the trailer `Proposal: id`,
/// no hook: the commit.
fn commit_by_hand(pair: &Pair, id: &str, paths: &[&str]) -> String {
    let mut add = vec!["add", "--"];
    add.extend_from_slice(paths);
    pair.git.git(&pair.linked, &add);
    pair.git.git(
        &pair.linked,
        &[
            "commit",
            "-q",
            "-m",
            &format!("spec: apply {id}"),
            "-m",
            &format!("Proposal: {id}"),
        ],
    );
    tip(pair)
}

/// The note of a completion by `commit` on `t1`.
fn completed_note(id: &str, commit: &str) -> String {
    format!("note: `{id}` completed by its commit {commit} on `t1`; no new commit")
}

/// Everything an approve answered on either stream, and its refusal.
fn said(outcome: &ProposalOutcome) -> String {
    let mut text = stderr_of(outcome).join("\n");
    text.push('\n');
    text.push_str(&outcome.document.notes.join("\n"));
    if let Some(refusal) = &outcome.refusal {
        text.push('\n');
        text.push_str(refusal);
    }
    text
}

/// `(inode, mtime)` of `path` under `root`.
fn stamp(root: &Path, path: &str) -> (u64, i64, i64) {
    let meta = fs::symlink_metadata(root.join(path)).expect("metadata");
    (meta.ino(), meta.mtime(), meta.mtime_nsec())
}

/// The run of `spec` in `dir`, which must answer.
fn ok(pair: &Pair, dir: &Path, args: &[&str]) -> Run {
    let run = spec(&pair.home, dir, args);
    run.code(0);
    run
}

// ------------------------------------------------------------------- O1

/// O1, on the game and on the newsletter: `spec propose create <path>
/// --text-file - --rationale R` and `spec approve PR-0001` (`y`): exit 0, the
/// consent question as ever, one commit of one parent whose name-status is
/// `A <path>` beside `M docs/index.md` (the archive shard's bytes unchanged,
/// so left out), stderr's note naming `docs/index.md`, stdout `applied ...`
/// byte for byte and the JSON holding no note; afterwards `spec check` has no
/// `index-drift` or `index-missing` and `spec export index` prints `unchanged`
/// for every output. M: skip the render.
#[test]
fn o1_a_create_commits_its_file_and_the_index_it_changes() {
    for of in [&FORGE, &PRESS] {
        let pair = project(&format!("ao-o1-{}", of.dir), of, |config| config);
        let base = tip(&pair);
        let archive = read(&pair.linked, ARCHIVE);
        let id = raise_create(&pair, of.new_path, of.new_text);
        assert_eq!(id, "PR-0001", "{}", of.dir);

        let (outcome, questions) = applied(&pair, &id);
        assert_eq!(
            questions,
            [format!(
                "apply PR-0001 to {} on t1 in {} (new file)? [y/N]",
                of.new_path,
                pair.linked.display()
            )],
            "{}: the consent question is unchanged",
            of.dir
        );
        let head = tip(&pair);
        assert_eq!(
            parents(&pair, &head),
            std::slice::from_ref(&base),
            "{}: one commit",
            of.dir
        );
        assert_eq!(
            name_status(&pair, &head),
            by_path(vec![format!("A\t{}", of.new_path), format!("M\t{INDEX}")]),
            "{}",
            of.dir
        );
        assert_eq!(
            blob(&pair, &head, of.new_path),
            of.new_text.as_bytes(),
            "{}",
            of.dir
        );
        let link = of.new_path.strip_prefix("docs/").expect("under docs/");
        let committed = String::from_utf8(blob(&pair, &head, INDEX)).expect("UTF-8 index");
        assert!(
            committed.contains(&format!("]({link})")),
            "{}: the new file's line: {committed}",
            of.dir
        );
        assert_eq!(read(&pair.linked, ARCHIVE), archive, "{}", of.dir);

        let lines = stderr_of(&outcome);
        assert_eq!(
            carries(&lines),
            [outputs_note(&[INDEX])],
            "{}: {lines:?}",
            of.dir
        );
        let (stdout, json) = printed(&outcome);
        assert_eq!(
            stdout,
            format!("applied PR-0001 as {head} on t1\n"),
            "{}",
            of.dir
        );
        assert!(
            !json.contains("the commit carries"),
            "{}: --json unchanged: {json}",
            of.dir
        );
        let stored = pair.proposal(&id);
        assert_eq!(stored.status, ProposalStatus::Applied, "{}", of.dir);
        assert_eq!(stored.applied_commit.as_deref(), Some(head.as_str()));

        assert_eq!(porcelain(&pair), "", "{}: the worktree clean", of.dir);
        assert_eq!(index_findings(&pair, &pair.linked), [], "{}", of.dir);
        assert_unchanged(&pair, &pair.linked);
    }
}

// ------------------------------------------------------------------- O3

/// The game with an untracked document and an unstaged `title:` edit in the
/// linked worktree, and `hook_body` as its `pre-commit` hook: the pair, the
/// draft's bytes and the edited file's.
fn dirty_forge(label: &str, hook_body: &str) -> (Pair, Vec<u8>, Vec<u8>) {
    let pair = forge(label);
    let draft = "---\nclass: spec\nstatus: draft\nscope: [forge]\n---\n\n\
                 # Draft tongs {#MEC-TONGS}\n\nTongs hold a blank off the anvil.\n";
    write(&pair.linked, "docs/features/draft.md", draft);
    replace(
        &pair.linked,
        "docs/canon/forge.md",
        "title: The forge\n",
        "title: The forge at night\n",
    );
    hook(&pair, hook_body);
    let edited = read(&pair.linked, "docs/canon/forge.md");
    (pair, draft.as_bytes().to_vec(), edited)
}

/// O3: the create of O1 beside an untracked draft document and an unstaged
/// edit of the canon page's `title:`, the `pre-commit` hook
/// running the built `spec check --staged --root .`: exit 0; the committed
/// `docs/index.md` is what `spec export index --stdout` renders in a clean
/// clone of the commit, byte for byte, and holds neither the draft's line nor
/// the new title; the draft and the edit are untouched, `git status` as
/// before. M: render from the disk walk.
#[test]
fn o3_the_index_is_rendered_from_the_committed_tree_and_a_staged_hook_passes_it() {
    let spec_bin = common::SPEC;
    let (pair, draft, edited) = dirty_forge(
        "ao-o3-pass",
        &format!("#!/bin/sh\nexec '{spec_bin}' check --staged --root .\n"),
    );
    let status = porcelain(&pair);
    assert_eq!(
        status, " M docs/canon/forge.md\n?? docs/features/draft.md\n",
        "the setup"
    );
    let id = raise_create(&pair, STAMINA, STAMINA_TEXT);
    let (outcome, _) = applied(&pair, &id);
    let head = tip(&pair);
    assert_eq!(
        name_status(&pair, &head),
        [format!("A\t{STAMINA}"), format!("M\t{INDEX}")]
    );
    assert_eq!(carries(&stderr_of(&outcome)), [outputs_note(&[INDEX])]);

    let index = String::from_utf8(blob(&pair, &head, INDEX)).expect("UTF-8");
    let archive = String::from_utf8(blob(&pair, &head, ARCHIVE)).expect("UTF-8");
    assert!(
        !index.contains("features/draft.md"),
        "the draft's line: {index}"
    );
    assert!(
        !index.contains("The forge at night"),
        "the new title: {index}"
    );
    assert!(index.contains("The forge"), "the committed title: {index}");
    assert!(index.contains("](features/stamina.md)"), "{index}");

    let clone = clean_clone(&pair, "clean");
    let shown = ok(&pair, &clone, &["export", "index", "--stdout"]);
    assert_eq!(
        shown.stdout,
        format!("==> {INDEX} <==\n{index}==> {ARCHIVE} <==\n{archive}"),
        "the committed outputs are the clean clone's render"
    );
    assert_unchanged(&pair, &clone);
    assert_eq!(index_findings(&pair, &clone), []);

    assert_eq!(read(&pair.linked, "docs/features/draft.md"), draft);
    assert_eq!(read(&pair.linked, "docs/canon/forge.md"), edited);
    assert_eq!(porcelain(&pair), status, "the worktree as before");
}

/// O3, the hook exiting 1: the target and `docs/index.md` back to their prior
/// bytes (the new file and its index entry gone), the archive shard as it was,
/// the proposal `open`, exit 1, `t1` unmoved, git's index and the worktree's
/// other changes as before. M: restore only the target.
#[test]
fn o3_a_failed_commit_restores_the_target_and_every_output_written() {
    let (pair, draft, edited) = dirty_forge("ao-o3-fail", "#!/bin/sh\nexit 1\n");
    let id = raise_create(&pair, STAMINA, STAMINA_TEXT);
    let head = tip(&pair);
    let index = read_text(&pair.linked, INDEX);
    let archive = read_text(&pair.linked, ARCHIVE);
    let status = porcelain(&pair);
    let staged = pair.git_text(&pair.linked, &["ls-files", "-s"]);

    let (outcome, _) = approve(&pair, &id);
    assert_eq!(outcome.exit(), Exit::NotFound, "exit 1: {outcome:?}");
    assert_eq!(tip(&pair), head, "no commit");
    assert!(!pair.linked.join(STAMINA).exists(), "the new file removed");
    assert_eq!(
        read_text(&pair.linked, INDEX),
        index,
        "the index's prior bytes"
    );
    assert_eq!(read_text(&pair.linked, ARCHIVE), archive);
    assert_eq!(
        pair.git_text(&pair.linked, &["ls-files", "-s"]),
        staged,
        "git's index as before"
    );
    assert_eq!(porcelain(&pair), status);
    assert_eq!(read(&pair.linked, "docs/features/draft.md"), draft);
    assert_eq!(read(&pair.linked, "docs/canon/forge.md"), edited);
    assert_eq!(pair.proposal(&id).status, ProposalStatus::Open);
    let leftovers: Vec<String> = snapshot(&pair.linked.join("docs"))
        .into_keys()
        .filter(|path| path.contains(".specengine-"))
        .collect();
    assert!(
        leftovers.is_empty(),
        "no scratch sibling left: {leftovers:?}"
    );
    let reason = outcome.refusal.clone().expect("a refusal");
    // The coordinator's wording of 2026-11-03: the restore names the outputs.
    assert!(
        reason.contains(&format!(
            "the commit of `{STAMINA}` and `{INDEX}` failed, the file removed, `{INDEX}` \
             restored: "
        )),
        "{reason}"
    );
}

// ------------------------------------------------------------------- O4

/// O4: no `index = true` entry, one registered `command = "touch marker.txt"`:
/// O1's apply commits `A` of the new file alone, as today, no note on
/// either stream, no index written, and no `marker.txt` anywhere. M: render
/// whenever `[[generators]]` exists; run the command.
#[test]
fn o4_with_no_index_entry_the_commit_is_todays_and_no_command_runs() {
    let pair = project("ao-o4", &FORGE, |config| {
        generators_replaced(
            &config,
            "[[generators]]\ncommand = \"touch marker.txt\"\nwrites  = [\"docs/marker.md\"]\n",
        )
    });
    assert!(!pair.main.join(INDEX).exists(), "nothing rendered at setup");
    let id = raise_create(&pair, STAMINA, STAMINA_TEXT);
    let (outcome, _) = applied(&pair, &id);
    let head = tip(&pair);
    assert_eq!(name_status(&pair, &head), [format!("A\t{STAMINA}")]);
    let lines = stderr_of(&outcome);
    assert_eq!(carries(&lines), Vec::<String>::new(), "no note: {lines:?}");
    let (stdout, json) = printed(&outcome);
    assert_eq!(stdout, format!("applied PR-0001 as {head} on t1\n"));
    assert!(!json.contains("the commit carries"), "{json}");
    for dir in [&pair.main, &pair.linked] {
        assert!(!dir.join(INDEX).exists(), "{}", dir.display());
        assert!(!dir.join("docs/marker.md").exists(), "{}", dir.display());
    }
    let markers: Vec<String> = snapshot(pair.scratch.path())
        .into_keys()
        .filter(|path| path.rsplit('/').next() == Some("marker.txt"))
        .collect();
    assert!(markers.is_empty(), "the command ran: {markers:?}");
    for dir in [
        repository_root(),
        std::env::current_dir().expect("the test's directory"),
    ] {
        assert!(!dir.join("marker.txt").exists(), "{}", dir.display());
    }
}

// ------------------------------------------------------------------- O5

/// O5: an update of a section's body in the game: the commit changes only its
/// path, no note, and neither output is written (inode, mtime and bytes kept).
/// M: stage every output whatever its bytes.
#[test]
fn o5_an_update_changing_no_index_line_commits_its_path_alone() {
    let pair = forge("ao-o5");
    let id = raise_update(
        &pair,
        "RULE-QUENCH-MEDIUM",
        "may crack the blade",
        "may crack a thin blade",
    );
    let outputs =
        [INDEX, ARCHIVE].map(|path| (path, stamp(&pair.linked, path), read(&pair.linked, path)));
    let (outcome, _) = applied(&pair, &id);
    let head = tip(&pair);
    assert_eq!(
        name_status(&pair, &head),
        ["M\tdocs/features/quench.md".to_owned()]
    );
    let lines = stderr_of(&outcome);
    assert_eq!(carries(&lines), Vec::<String>::new(), "no note: {lines:?}");
    assert!(
        !lines.iter().any(|line| line.starts_with("warning:")),
        "{lines:?}"
    );
    for (path, stamped, bytes) in outputs {
        assert_eq!(stamp(&pair.linked, path), stamped, "{path} not written");
        assert_eq!(read(&pair.linked, path), bytes, "{path}");
    }
    assert_eq!(porcelain(&pair), "");
    assert_unchanged(&pair, &pair.linked);
}

// ------------------------------------------------------------------- O6

/// O6, a create: its commit made by hand (the file and the index `spec export
/// index` renders, trailer `Proposal: PR-0001`) and the proposal left
/// `approved`: `spec approve PR-0001` asks nothing, notes `` `PR-0001`
/// completed by its commit <sha> on `t1`; no new commit ``, `t1` does not
/// move, `applied` with that commit. M: keep "only the target's path
/// changed".
#[test]
fn o6_a_creates_commit_with_its_output_completes_it() {
    let pair = forge("ao-o6-create");
    let id = raise_create(&pair, STAMINA, STAMINA_TEXT);
    set_approved(&pair, &id);
    write(&pair.linked, STAMINA, STAMINA_TEXT);
    ok(&pair, &pair.linked, &["export", "index"]);
    assert_eq!(porcelain(&pair), format!(" M {INDEX}\n?? {STAMINA}\n"));
    let commit = commit_by_hand(&pair, &id, &[STAMINA, INDEX]);
    assert_eq!(
        name_status(&pair, &commit),
        [format!("A\t{STAMINA}"), format!("M\t{INDEX}")]
    );

    let (outcome, questions) = applied(&pair, &id);
    assert!(questions.is_empty(), "no prompt: {questions:?}");
    let lines = stderr_of(&outcome);
    assert!(lines.contains(&completed_note(&id, &commit)), "{lines:?}");
    assert_eq!(tip(&pair), commit, "no new commit");
    let stored = pair.proposal(&id);
    assert_eq!(stored.status, ProposalStatus::Applied);
    assert_eq!(stored.applied_commit.as_deref(), Some(commit.as_str()));
}

/// O6, an update whose commit changes an index line (the H1 a title is read
/// from) and so carries `docs/index.md`: made by hand, `approved` left, it
/// completes alike. M: keep "only the target's path changed".
#[test]
fn o6_an_updates_commit_with_its_output_completes_it() {
    let pair = forge("ao-o6-update");
    let from = "# Quench {#MEC-QUENCH}\n";
    let to = "# Quench and temper {#MEC-QUENCH}\n";
    let id = raise_update(&pair, "MEC-QUENCH", from, to);
    set_approved(&pair, &id);
    replace(&pair.linked, "docs/features/quench.md", from, to);
    ok(&pair, &pair.linked, &["export", "index"]);
    assert_eq!(
        porcelain(&pair),
        format!(" M docs/features/quench.md\n M {INDEX}\n"),
        "the title is an index line"
    );
    let commit = commit_by_hand(&pair, &id, &["docs/features/quench.md", INDEX]);

    let (outcome, questions) = applied(&pair, &id);
    assert!(questions.is_empty(), "no prompt: {questions:?}");
    let lines = stderr_of(&outcome);
    assert!(lines.contains(&completed_note(&id, &commit)), "{lines:?}");
    assert_eq!(tip(&pair), commit, "no new commit");
    assert_eq!(pair.proposal(&id).status, ProposalStatus::Applied);
}

/// O6: a commit adding one more path beside the target and the index reads
/// `changes ...` naming it and completes nothing: still `approved`, `t1`
/// unmoved. M: accept any extra path.
#[test]
fn o6_a_commit_with_one_more_path_completes_nothing() {
    let pair = forge("ao-o6-extra");
    let id = raise_create(&pair, STAMINA, STAMINA_TEXT);
    set_approved(&pair, &id);
    write(&pair.linked, STAMINA, STAMINA_TEXT);
    ok(&pair, &pair.linked, &["export", "index"]);
    write(
        &pair.linked,
        "design/sketch.txt",
        "A sketch of the anvil.\n",
    );
    let commit = commit_by_hand(&pair, &id, &[STAMINA, INDEX, "design/sketch.txt"]);

    let (outcome, _) = approve(&pair, &id);
    assert_ne!(outcome.exit(), Exit::Answered, "{outcome:?}");
    let text = said(&outcome);
    assert!(
        text.contains("changes") && text.contains("design/sketch.txt"),
        "why it does not complete: {text}"
    );
    assert!(text.contains(&commit), "the commit named: {text}");
    assert_eq!(tip(&pair), commit);
    let stored = pair.proposal(&id);
    assert_eq!(stored.status, ProposalStatus::Approved);
    assert!(stored.applied_commit.is_none());
}

// ------------------------------------------------------------------- O7

/// A question on `target` raised in the linked worktree: its ID.
fn question(pair: &Pair, target: &str, text: &str) -> String {
    pair.raise_question(
        &pair.linked,
        &[target],
        text,
        "Keep it as written.",
        "A new rule case.",
    )
}

/// O7: a question approved in the game (the working answer): one commit,
/// `A <record>` beside `M docs/index.md`, the record's line in the committed
/// index, no index finding after. Then another one's apply stopped between its
/// steps 9 and 10: its `spec approve` completes from that commit, no prompt,
/// no new commit. M: keep "name-status exactly `A <path>`".
#[test]
fn o7_a_decision_record_commits_beside_the_index_and_completes_from_it() {
    let pair = forge("ao-o7");
    let first = question(&pair, "MEC-FORGE", "Does a dark blank take a strike?");
    let base = tip(&pair);
    let (outcome, _) = pair.decide_ok(&pair.main, &first, &ApproveFlags::default(), None);
    let record = outcome
        .document
        .record_path
        .clone()
        .expect("the record's path");
    assert!(record.starts_with("docs/records/DEC/"), "{record}");
    let head = tip(&pair);
    assert_eq!(parents(&pair, &head), [base]);
    assert_eq!(
        name_status(&pair, &head),
        [format!("M\t{INDEX}"), format!("A\t{record}")]
    );
    let index = String::from_utf8(blob(&pair, &head, INDEX)).expect("UTF-8");
    let link = record.strip_prefix("docs/").expect("under docs/");
    assert!(index.contains(&format!("]({link})")), "{index}");
    assert_eq!(carries(&stderr_of(&outcome)), [outputs_note(&[INDEX])]);
    assert_eq!(index_findings(&pair, &pair.linked), []);

    let second = question(&pair, "RULE-FORGE-DECAY", "Does a quench stop the decay?");
    let (outcome, _) = pair.decide_ok(&pair.main, &second, &ApproveFlags::default(), None);
    let commit = tip(&pair);
    let record = outcome.document.record_path.clone().expect("its record");
    assert_eq!(
        name_status(&pair, &commit),
        [format!("M\t{INDEX}"), format!("A\t{record}")]
    );
    rewound_to_approved(&pair, &second);
    let (outcome, questions) = pair.decide_at(
        &pair.main,
        &second,
        &ApproveFlags::default(),
        None,
        true,
        LATER,
    );
    let outcome = outcome.unwrap_or_else(|error| panic!("approve {second}: {error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert!(questions.is_empty(), "no prompt: {questions:?}");
    let lines = stderr_of(&outcome);
    assert!(
        lines.contains(&completed_note(&second, &commit)),
        "{lines:?}"
    );
    assert_eq!(tip(&pair), commit, "no new commit");
    let stored = pair.proposal(&second);
    assert_eq!(stored.status, ProposalStatus::Applied);
    assert_eq!(stored.applied_commit.as_deref(), Some(commit.as_str()));
}

// ------------------------------------------------------------------- O8

/// O8, a gap: `HEAD`'s config lists the root `notes`, present on disk and
/// absent from `HEAD`'s tree: O1's apply commits the target alone, exit 0,
/// `applied`, no output written, stderr's warning naming the `MissingRoot`
/// gap as `spec export index` words it and the command. M: render the partial
/// walk; refuse the apply.
#[test]
fn o8_a_root_missing_from_heads_tree_commits_the_target_alone_with_a_warning() {
    let pair = forge("ao-o8-root");
    replace(
        &pair.linked,
        "specengine.toml",
        "roots = [\"README.md\", \"docs\", \"design\"]",
        "roots = [\"README.md\", \"docs\", \"design\", \"notes\"]",
    );
    pair.commit_all(&pair.linked, "a notes root");
    let plan = "---\nclass: spec\nstatus: draft\nscope: [forge]\n---\n\n# A plan\n\nTo sort.\n";
    write(&pair.linked, "notes/plan.md", plan);
    let index = read(&pair.linked, INDEX);
    let id = raise_create(&pair, STAMINA, STAMINA_TEXT);
    let (outcome, _) = applied(&pair, &id);
    let head = tip(&pair);
    assert_eq!(name_status(&pair, &head), [format!("A\t{STAMINA}")]);
    assert_eq!(pair.proposal(&id).status, ProposalStatus::Applied);
    let lines = stderr_of(&outcome);
    assert_eq!(
        carries(&lines),
        [alone_warning(
            STAMINA,
            "the `[paths] roots` entry `notes` names no directory and no `.md` file"
        )],
        "{lines:?}"
    );
    assert_eq!(read(&pair.linked, INDEX), index, "no output written");
    assert_eq!(read(&pair.linked, "notes/plan.md"), plan.as_bytes());
    assert_eq!(porcelain(&pair), "?? notes/plan.md\n");
}

/// O8, an output failing inspection: `docs/index.md` a symlink in the
/// worktree: the target alone, exit 0, `applied`, the warning naming the
/// symlink in `spec export index`'s words; the symlink and its target as they
/// were. M: render the partial walk; refuse the apply.
#[test]
fn o8_a_symlinked_output_commits_the_target_alone_with_a_warning() {
    let pair = forge("ao-o8-symlink");
    let elsewhere = pair.scratch.dir("elsewhere");
    let target = elsewhere.join("index.md");
    let bytes = read(&pair.linked, INDEX);
    fs::write(&target, &bytes).expect("the symlink's target");
    fs::remove_file(pair.linked.join(INDEX)).expect("the index");
    symlink(&target, pair.linked.join(INDEX)).expect("a symlinked index");
    let id = raise_create(&pair, STAMINA, STAMINA_TEXT);
    let (outcome, _) = applied(&pair, &id);
    let head = tip(&pair);
    assert_eq!(name_status(&pair, &head), [format!("A\t{STAMINA}")]);
    assert_eq!(pair.proposal(&id).status, ProposalStatus::Applied);
    let lines = stderr_of(&outcome);
    assert_eq!(
        carries(&lines),
        [alone_warning(
            STAMINA,
            "`docs/index.md` is a symlink: the index is written only through real directories, \
             to a regular file"
        )],
        "{lines:?}"
    );
    let meta = fs::symlink_metadata(pair.linked.join(INDEX)).expect("the link");
    assert!(meta.file_type().is_symlink(), "still a symlink");
    assert_eq!(
        fs::read(&target).expect("target"),
        bytes,
        "its target untouched"
    );
}

// --------------------------------------------------- beyond the criteria

/// "Rules and edge cases": `HEAD`'s config does not read (an unknown key in its
/// generator entry) while the worktree's does: the apply goes on, the commit is
/// the target alone, and the warning names `HEAD's specengine.toml is not a
/// valid config (line N): ...` and the disk config's command. M: the disk
/// config rendered.
#[test]
fn a_head_config_that_does_not_read_commits_the_target_alone_with_a_warning() {
    let pair = forge("ao-head-config");
    let good = read_text(&pair.linked, "specengine.toml");
    // HEAD's entry names another command: the warning names the disk's
    // (the coordinator's decision of 2026-11-03).
    let bad = good
        .replacen(
            "command = \"spec export index\"",
            "command = \"make index\"",
            1,
        )
        .replacen("index   = true\n", "index   = true\ncolour  = \"red\"\n", 1);
    assert_ne!(bad, good);
    write(&pair.linked, "specengine.toml", &bad);
    pair.commit_all(&pair.linked, "a config that does not read");
    write(&pair.linked, "specengine.toml", &good);
    let id = raise_create(&pair, STAMINA, STAMINA_TEXT);
    let (outcome, _) = applied(&pair, &id);
    let head = tip(&pair);
    assert_eq!(name_status(&pair, &head), [format!("A\t{STAMINA}")]);
    let lines = carries(&stderr_of(&outcome));
    assert_eq!(lines.len(), 1, "{lines:?}");
    let prefix = format!(
        "warning: the commit carries `{STAMINA}` alone: the index's outputs were not rendered: \
         HEAD's specengine.toml is not a valid config (line "
    );
    assert!(
        lines[0].starts_with(&prefix) && lines[0].ends_with(&format!("; `{COMMAND}` writes them")),
        "{lines:?}"
    );
}

/// "Rules and edge cases": an index stale at `HEAD` (a document committed
/// without its line) is brought up to date by the next apply's commit, even
/// one whose own change adds no line: outputs are judged against `HEAD`'s
/// blobs. M: outputs judged against the render before the change.
#[test]
fn a_stale_index_at_head_is_brought_up_to_date_by_the_next_apply() {
    let pair = forge("ao-stale");
    write(
        &pair.linked,
        "docs/features/anvil.md",
        "---\nclass: spec\nstatus: draft\nscope: [forge]\n---\n\n# Anvil {#MEC-ANVIL}\n\n\
         The anvil holds the blank.\n",
    );
    pair.commit_all(&pair.linked, "a document without its index line");
    let id = raise_update(
        &pair,
        "RULE-QUENCH-MEDIUM",
        "may crack the blade",
        "may crack a thin blade",
    );
    let (outcome, _) = applied(&pair, &id);
    let head = tip(&pair);
    assert_eq!(
        name_status(&pair, &head),
        [
            "M\tdocs/features/quench.md".to_owned(),
            format!("M\t{INDEX}")
        ]
    );
    let index = String::from_utf8(blob(&pair, &head, INDEX)).expect("UTF-8");
    assert!(index.contains("](features/anvil.md)"), "{index}");
    assert_eq!(carries(&stderr_of(&outcome)), [outputs_note(&[INDEX])]);
    assert_eq!(index_findings(&pair, &pair.linked), []);
}

// ---------------------------------------------- iteration 2: O11 to O14

/// The cause list's ignored output (docs/features/apply-outputs.md "Data").
fn ignored_cause(path: &str) -> String {
    format!("`{path}` is ignored by git")
}

/// The cause list's case variant: `configured` is `on_disk` there.
fn case_variant_cause(configured: &str, on_disk: &str) -> String {
    format!("`{configured}` is `{on_disk}` on disk (a case variant)")
}

/// The cause list's path that can never be an output, and why.
fn never_cause(path: &str, why: &str) -> String {
    format!("`{path}` can never be an output ({why})")
}

/// Step 10's warning of an output committed with other bytes than rendered.
fn mismatch_warning(path: &str) -> String {
    format!(
        "warning: the committed `{path}` is not the bytes this apply rendered; `{COMMAND}` writes \
         them"
    )
}

/// `docs/index.md` dropped from git's index and from `HEAD` by a commit, its
/// file kept on disk, untracked; listed in `.gitignore` when `ignored`.
fn untracked_index(pair: &Pair, ignored: bool) {
    pair.git
        .git(&pair.linked, &["rm", "-q", "--cached", "--", INDEX]);
    if ignored {
        write(&pair.linked, ".gitignore", format!("{INDEX}\n"));
        pair.git.git(&pair.linked, &["add", "--", ".gitignore"]);
    }
    pair.git
        .git(&pair.linked, &["commit", "-q", "-m", "the index untracked"]);
    assert!(pair.linked.join(INDEX).is_file(), "kept on disk");
    assert_eq!(pair.git_text(&pair.linked, &["ls-files", "--", INDEX]), "");
}

/// `config` with one more shard of the index entry: `path`, claiming
/// `design/**`, in its `writes` too.
fn with_shard(config: &str, path: &str) -> String {
    let writes = "writes  = [\"docs/index.md\", \"docs/index-archive.md\"]";
    let shards = "shards  = [{ path = \"docs/index-archive.md\", tier3 = true }]";
    assert!(
        config.contains(writes) && config.contains(shards),
        "{config}"
    );
    config
        .replacen(
            writes,
            &format!("writes  = [\"docs/index.md\", \"docs/index-archive.md\", \"{path}\"]"),
            1,
        )
        .replacen(
            shards,
            &format!(
                "shards  = [{{ path = \"docs/index-archive.md\", tier3 = true }}, {{ path = \
                 \"{path}\", claims = [\"design/**\"] }}]"
            ),
            1,
        )
}

/// `config` committed on `t1` as `HEAD`'s, the worktree's own config then
/// put back on disk (unstaged): `HEAD`'s config is the only one that says it.
fn head_config_only(pair: &Pair, config: &str, message: &str) {
    let disk = read_text(&pair.linked, "specengine.toml");
    write(&pair.linked, "specengine.toml", config);
    pair.commit_all(&pair.linked, message);
    write(&pair.linked, "specengine.toml", &disk);
}

/// A `spec.lock` holding no binding: the header of
/// `fixtures/spec-lock/code-a.lock`, up to its first blank line.
fn empty_lock() -> String {
    let lock = read_text(&common::fixture("spec-lock"), "code-a.lock");
    let header: String = lock
        .split_inclusive('\n')
        .take_while(|line| !line.trim().is_empty())
        .collect();
    assert!(header.contains("format = "), "{header}");
    header
}

/// Whether `dir`'s file system folds case: a name written in lower case is
/// found in upper case.
fn folds_case(dir: &Path) -> bool {
    let probe = dir.join("case-probe");
    fs::write(&probe, b"probe").expect("the probe");
    let folds = dir.join("CASE-PROBE").exists();
    fs::remove_file(&probe).expect("the probe removed");
    folds
}

/// A `git` first on a scratch `PATH` that refuses `git add --intent-to-add`
/// of `docs/index.md` (exit 128, a line on stderr) and runs the real one for
/// everything else: the git environment of an approve from the main worktree
/// that uses it.
fn refusing_intent_to_add(pair: &Pair) -> specengine_store::GitEnv {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = pair.scratch.dir("refusing-git");
    let real = pair.git.git_program().to_str().expect("a UTF-8 git");
    let shim = dir.join("git");
    fs::write(
        &shim,
        format!(
            "#!/bin/sh\nintent=0; output=0\nfor arg in \"$@\"; do\n  case \"$arg\" in\n    \
             --intent-to-add|-N) intent=1;;\n    *docs/index.md) output=1;;\n  esac\ndone\n\
             if [ $intent = 1 ] && [ $output = 1 ]; then\n  \
             echo 'fatal: the test refuses to stage docs/index.md' >&2\n  exit 128\nfi\n\
             exec '{real}' \"$@\"\n"
        ),
    )
    .expect("the shim");
    fs::set_permissions(&shim, fs::Permissions::from_mode(0o755)).expect("executable");
    let mut vars = pair.git.vars();
    vars.retain(|(name, _)| name != "PATH");
    vars.push(("PATH".into(), pair.git.path_with(&dir)));
    specengine_store::GitEnv::new(&pair.main, vars)
}

/// O11: `docs/index.md` untracked and listed in `.gitignore`: O1's apply
/// commits `A` of the new file alone, exit 0, `applied`, the warning
/// `` `docs/index.md` is ignored by git ``, the file on disk untouched and not
/// in `git ls-files`. M: refuse at step 9.
#[test]
fn o11_an_ignored_output_commits_the_target_alone_with_a_warning() {
    let pair = forge("ao-o11-ignored");
    untracked_index(&pair, true);
    let index = read_text(&pair.linked, INDEX);
    let id = raise_create(&pair, STAMINA, STAMINA_TEXT);
    let (outcome, _) = applied(&pair, &id);
    let head = tip(&pair);
    assert_eq!(name_status(&pair, &head), [format!("A\t{STAMINA}")]);
    assert_eq!(pair.proposal(&id).status, ProposalStatus::Applied);
    let lines = stderr_of(&outcome);
    assert_eq!(
        carries(&lines),
        [alone_warning(STAMINA, &ignored_cause(INDEX))],
        "{lines:?}"
    );
    assert_eq!(read_text(&pair.linked, INDEX), index, "untouched");
    assert_eq!(pair.git_text(&pair.linked, &["ls-files", "--", INDEX]), "");
    assert_eq!(porcelain(&pair), "", "ignored, so no status line");
}

/// O11, a case variant (skipped where the file system is case-sensitive):
/// `HEAD`'s config names the index with a capital `I` while `docs/index.md` is
/// on disk: the target alone, the warning naming the configured path as a case
/// variant of `docs/index.md`, whose bytes are unchanged. M: refuse at step 9.
#[test]
fn o11_a_case_variant_output_commits_the_target_alone_with_a_warning() {
    let pair = forge("ao-o11-case");
    if !folds_case(&pair.linked) {
        eprintln!("skipped: {} is case-sensitive", pair.linked.display());
        return;
    }
    let config = read_text(&pair.linked, "specengine.toml");
    let variant = config
        .replacen("index = \"docs/index.md\"", "index = \"docs/Index.md\"", 1)
        .replacen(
            "writes  = [\"docs/index.md\",",
            "writes  = [\"docs/Index.md\",",
            1,
        );
    assert_eq!(variant.matches("docs/Index.md").count(), 2, "{variant}");
    write(&pair.linked, "specengine.toml", &variant);
    pair.commit_all(&pair.linked, "the index named in another case");
    let index = read(&pair.linked, INDEX);
    let id = raise_create(&pair, STAMINA, STAMINA_TEXT);
    let (outcome, _) = applied(&pair, &id);
    let head = tip(&pair);
    assert_eq!(name_status(&pair, &head), [format!("A\t{STAMINA}")]);
    assert_eq!(pair.proposal(&id).status, ProposalStatus::Applied);
    let lines = stderr_of(&outcome);
    assert_eq!(
        carries(&lines),
        [alone_warning(
            STAMINA,
            &case_variant_cause("docs/Index.md", INDEX)
        )],
        "{lines:?}"
    );
    assert_eq!(read(&pair.linked, INDEX), index, "its bytes unchanged");
    let names: Vec<String> = fs::read_dir(pair.linked.join("docs"))
        .expect("docs")
        .map(|entry| {
            entry
                .expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|name| name.eq_ignore_ascii_case("index.md"))
        .collect();
    assert_eq!(names, ["index.md"], "the entry's own name kept");
}

/// O11, an output whose staging fails: `docs/index.md` untracked (not
/// ignored), and a `git` that refuses its `git add --intent-to-add`: the
/// target alone, exit 0, `applied`, the warning `` staging `docs/index.md`
/// failed: <git's error> ``, no index entry left for it, its bytes the prior
/// ones. M: refuse at step 9.
#[test]
fn o11_an_output_whose_staging_fails_commits_the_target_alone_with_a_warning() {
    let pair = forge("ao-o11-staging");
    untracked_index(&pair, false);
    let index = read_text(&pair.linked, INDEX);
    let archive = read_text(&pair.linked, ARCHIVE);
    let id = raise_create(&pair, STAMINA, STAMINA_TEXT);
    let git = refusing_intent_to_add(&pair);
    let (outcome, _) = pair.approve_answer(&pair.main, &id, true, git);
    let outcome = outcome.unwrap_or_else(|error| panic!("approve {id}: {error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    let head = tip(&pair);
    assert_eq!(name_status(&pair, &head), [format!("A\t{STAMINA}")]);
    assert_eq!(pair.proposal(&id).status, ProposalStatus::Applied);
    let lines = carries(&stderr_of(&outcome));
    let prefix = format!(
        "warning: the commit carries `{STAMINA}` alone: the index's outputs were not rendered: \
         staging `{INDEX}` failed: "
    );
    assert!(
        lines.len() == 1
            && lines[0].starts_with(&prefix)
            && lines[0].contains("the test refuses to stage docs/index.md")
            && lines[0].ends_with(&format!("; `{COMMAND}` writes them")),
        "{lines:?}"
    );
    assert_eq!(
        pair.git_text(&pair.linked, &["ls-files", "--", INDEX]),
        "",
        "no intent-to-add entry left"
    );
    assert_eq!(read_text(&pair.linked, INDEX), index, "its prior bytes");
    assert_eq!(read_text(&pair.linked, ARCHIVE), archive);
    assert_eq!(porcelain(&pair), format!("?? {INDEX}\n"));
}

/// O12: `HEAD`'s config names the update's own target as a shard (the disk
/// config does not): the apply commits the target alone, the warning names
/// it, and the target holds the proposal's text, committed and on disk. M:
/// write it.
#[test]
fn o12_a_config_naming_the_target_as_a_shard_commits_the_target_alone() {
    let pair = forge("ao-o12-target");
    let quench = "docs/features/quench.md";
    let config = read_text(&pair.linked, "specengine.toml");
    head_config_only(&pair, &with_shard(&config, quench), "the target a shard");
    let id = raise_update(
        &pair,
        "RULE-QUENCH-MEDIUM",
        "may crack the blade",
        "may crack a thin blade",
    );
    let (outcome, _) = applied(&pair, &id);
    let head = tip(&pair);
    assert_eq!(name_status(&pair, &head), [format!("M\t{quench}")]);
    assert_eq!(pair.proposal(&id).status, ProposalStatus::Applied);
    let lines = stderr_of(&outcome);
    assert_eq!(
        carries(&lines),
        [alone_warning(
            quench,
            &never_cause(quench, "the apply's target")
        )],
        "{lines:?}"
    );
    let committed = String::from_utf8(blob(&pair, &head, quench)).expect("UTF-8");
    assert!(
        committed.contains("may crack a thin blade")
            && committed.contains("# Quench {#MEC-QUENCH}"),
        "the proposal's text, not a shard: {committed}"
    );
    assert_eq!(read_text(&pair.linked, quench), committed, "on disk alike");
}

/// O12, `spec.lock` named as a shard by `HEAD`'s config: the update commits its
/// target alone, the warning names `spec.lock`, and `spec.lock` keeps its
/// bytes, committed and on disk. M: write it.
#[test]
fn o12_a_config_naming_spec_lock_as_a_shard_leaves_it_alone() {
    let pair = forge("ao-o12-lock");
    let lock = empty_lock();
    write(&pair.linked, "spec.lock", &lock);
    let config = read_text(&pair.linked, "specengine.toml");
    head_config_only(
        &pair,
        &with_shard(&config, "spec.lock"),
        "spec.lock a shard",
    );
    let id = raise_update(
        &pair,
        "RULE-QUENCH-MEDIUM",
        "may crack the blade",
        "may crack a thin blade",
    );
    let (outcome, _) = applied(&pair, &id);
    let head = tip(&pair);
    assert_eq!(
        name_status(&pair, &head),
        ["M\tdocs/features/quench.md".to_owned()]
    );
    let lines = stderr_of(&outcome);
    assert_eq!(
        carries(&lines),
        [alone_warning(
            "docs/features/quench.md",
            &never_cause("spec.lock", "spec.lock")
        )],
        "{lines:?}"
    );
    assert_eq!(read_text(&pair.linked, "spec.lock"), lock, "on disk");
    assert_eq!(
        blob(&pair, &head, "spec.lock"),
        lock.as_bytes(),
        "committed"
    );
}

/// O13's proposal: O1's create raised and left `approved`, its file written
/// and `docs/index.md` rendered as an apply would (the parent's config): the
/// pair and the ID.
fn hand_made_create(label: &str) -> (Pair, String) {
    let pair = forge(label);
    let id = raise_create(&pair, STAMINA, STAMINA_TEXT);
    set_approved(&pair, &id);
    write(&pair.linked, STAMINA, STAMINA_TEXT);
    ok(&pair, &pair.linked, &["export", "index"]);
    (pair, id)
}

/// O13's verdict: approve completes nothing, says `changes ...` naming
/// `extra`, the commit, and leaves the proposal `approved` on `commit`.
#[track_caller]
fn completes_nothing(pair: &Pair, id: &str, commit: &str, extra: &str) {
    let (outcome, _) = approve(pair, id);
    assert_ne!(outcome.exit(), Exit::Answered, "{outcome:?}");
    let text = said(&outcome);
    assert!(
        text.contains("changes") && text.contains(extra) && text.contains(commit),
        "why it does not complete, naming {extra}: {text}"
    );
    assert_eq!(tip(pair), commit, "no new commit");
    let stored = pair.proposal(id);
    assert_eq!(stored.status, ProposalStatus::Approved);
    assert!(stored.applied_commit.is_none());
}

/// O13: a hand commit with `Proposal: PR-0001`, one parent, changing the target,
/// `docs/index.md` and `specengine.toml`, whose own config adds
/// `specengine.toml` as a shard while the parent's does not: approve completes
/// nothing, `changes ...` naming `specengine.toml`, `approved`. M: read the
/// commit's config; accept the four paths as outputs.
#[test]
fn o13_a_commit_whose_own_config_names_its_config_completes_nothing() {
    let (pair, id) = hand_made_create("ao-o13-config");
    let config = read_text(&pair.linked, "specengine.toml");
    write(
        &pair.linked,
        "specengine.toml",
        with_shard(&config, "specengine.toml"),
    );
    let commit = commit_by_hand(&pair, &id, &[STAMINA, INDEX, "specengine.toml"]);
    assert_eq!(
        name_status(&pair, &commit),
        [
            format!("A\t{STAMINA}"),
            format!("M\t{INDEX}"),
            "M\tspecengine.toml".to_owned()
        ]
    );
    completes_nothing(&pair, &id, &commit, "specengine.toml");
}

/// O13: a hand commit whose own config adds an ordinary shard, `extra.md` under
/// `docs/`, which it adds too: completes nothing, `changes ...` naming it. M: read
/// the commit's config.
#[test]
fn o13_a_commit_whose_own_config_adds_a_shard_completes_nothing() {
    let (pair, id) = hand_made_create("ao-o13-shard");
    let config = read_text(&pair.linked, "specengine.toml");
    write(
        &pair.linked,
        "specengine.toml",
        with_shard(&config, "docs/extra.md"),
    );
    write(&pair.linked, "docs/extra.md", "# An extra shard\n");
    let commit = commit_by_hand(
        &pair,
        &id,
        &[STAMINA, INDEX, "specengine.toml", "docs/extra.md"],
    );
    completes_nothing(&pair, &id, &commit, "docs/extra.md");
}

/// O13 beside the criterion ("Completion": never accepted as outputs, the four
/// paths): the parent's config names `spec.lock` as a shard and the hand
/// commit changes it beside the target and `docs/index.md`: completes nothing,
/// `changes ...` naming `spec.lock`. M: accept the four paths as outputs.
#[test]
fn o13_a_commit_carrying_spec_lock_that_its_parent_names_completes_nothing() {
    let pair = forge("ao-o13-lock");
    let lock = empty_lock();
    write(&pair.linked, "spec.lock", &lock);
    let config = read_text(&pair.linked, "specengine.toml");
    write(
        &pair.linked,
        "specengine.toml",
        with_shard(&config, "spec.lock"),
    );
    pair.commit_all(&pair.linked, "spec.lock a shard");
    let id = raise_create(&pair, STAMINA, STAMINA_TEXT);
    set_approved(&pair, &id);
    write(&pair.linked, STAMINA, STAMINA_TEXT);
    let index = read_text(&pair.linked, INDEX);
    write(&pair.linked, INDEX, format!("{index}\n"));
    write(&pair.linked, "spec.lock", format!("{lock}# touched\n"));
    let commit = commit_by_hand(&pair, &id, &[STAMINA, INDEX, "spec.lock"]);
    completes_nothing(&pair, &id, &commit, "spec.lock");
}

/// O13 beside the criterion, iteration 3 ("Completion": `spec.lock` is never
/// accepted by its name, whatever its bytes): the parent's config names
/// `spec.lock` as a shard, the parent lacks it, and the hand commit adds it
/// holding a generated document beside the target and `docs/index.md`:
/// completes nothing, `changes ...` naming `spec.lock`. Since iteration 3 the
/// parent-class judge refuses a `spec.lock` the parent holds by itself; only
/// one the parent lacks is the name rule's alone. M: accept the four paths as
/// outputs; read the commit's config and accept them.
#[test]
fn o13_a_commit_adding_a_spec_lock_its_parent_names_but_lacks_completes_nothing() {
    let pair = forge("ao-o13-new-lock");
    let config = read_text(&pair.linked, "specengine.toml");
    write(
        &pair.linked,
        "specengine.toml",
        with_shard(&config, "spec.lock"),
    );
    pair.commit_all(&pair.linked, "spec.lock a shard, no spec.lock");
    let id = raise_create(&pair, STAMINA, STAMINA_TEXT);
    set_approved(&pair, &id);
    write(&pair.linked, STAMINA, STAMINA_TEXT);
    let index = read_text(&pair.linked, INDEX);
    write(&pair.linked, INDEX, format!("{index}\n"));
    write(
        &pair.linked,
        "spec.lock",
        format!("---\nclass: generated\ngenerator: {COMMAND}\n---\n\n# A shard\n"),
    );
    let commit = commit_by_hand(&pair, &id, &[STAMINA, INDEX, "spec.lock"]);
    assert_eq!(
        name_status(&pair, &commit),
        [
            format!("A\t{STAMINA}"),
            format!("M\t{INDEX}"),
            "A\tspec.lock".to_owned()
        ]
    );
    completes_nothing(&pair, &id, &commit, "spec.lock");
}

/// O13: the same hand commit with neither extra path, the target and
/// `docs/index.md` alone, completes: no prompt, the note, `applied`. M: accept
/// no output beside the target.
#[test]
fn o13_the_same_commit_with_neither_extra_path_completes() {
    let (pair, id) = hand_made_create("ao-o13-plain");
    let commit = commit_by_hand(&pair, &id, &[STAMINA, INDEX]);
    let (outcome, questions) = applied(&pair, &id);
    assert!(questions.is_empty(), "no prompt: {questions:?}");
    assert!(
        stderr_of(&outcome).contains(&completed_note(&id, &commit)),
        "{outcome:?}"
    );
    assert_eq!(tip(&pair), commit);
    assert_eq!(pair.proposal(&id).status, ProposalStatus::Applied);
}

/// O14: a `pre-commit` hook appends a line to `docs/index.md` and stages it:
/// O1's apply ends exit 0, `applied`, the commit carrying the hook's bytes, and
/// step 10's warning names `docs/index.md` and `spec export index`. M: check
/// names only.
#[test]
fn o14_a_hook_rewriting_an_output_is_step_10s_warning_and_the_run_stays_applied() {
    let pair = forge("ao-o14");
    hook(
        &pair,
        "#!/bin/sh\nprintf -- '- a line the hook adds\\n' >> docs/index.md && git add -- \
         docs/index.md\n",
    );
    let id = raise_create(&pair, STAMINA, STAMINA_TEXT);
    let (outcome, _) = applied(&pair, &id);
    let head = tip(&pair);
    assert_eq!(
        name_status(&pair, &head),
        [format!("A\t{STAMINA}"), format!("M\t{INDEX}")]
    );
    let committed = String::from_utf8(blob(&pair, &head, INDEX)).expect("UTF-8");
    assert!(
        committed.ends_with("- a line the hook adds\n"),
        "the hook's bytes committed: {committed}"
    );
    let stored = pair.proposal(&id);
    assert_eq!(stored.status, ProposalStatus::Applied);
    assert_eq!(stored.applied_commit.as_deref(), Some(head.as_str()));
    let lines = stderr_of(&outcome);
    assert!(lines.contains(&mismatch_warning(INDEX)), "{lines:?}");
}

// --------------------------------------------------- iteration 3: O16

/// Another walked document of the game, `class: spec`, with an ID of its own.
const OTHER: &str = "docs/features/other.md";
const OTHER_TEXT: &str = "---\nclass: spec\nstatus: draft\nscope: [forge]\n---\n\n\
     # Tongs {#MEC-TONGS}\n\nTongs hold a hot blank.\n";

/// An untracked draft of the game, `class: spec`, never committed.
const DRAFT_OTHER: &str = "docs/features/draft-other.md";
const DRAFT_OTHER_TEXT: &str = "---\nclass: spec\nstatus: draft\nscope: [forge]\n---\n\n\
     # Anvil horn {#MEC-HORN}\n\nThe horn bends a blank.\n";

/// The empty blob's ID: what an intent-to-add entry holds.
const EMPTY_BLOB: &str = "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391";

/// A `git` first on a scratch `PATH`: `middle` (shell, `REAL` the real git's
/// path) runs first, then the real git for whatever it lets through. The git
/// environment of an approve from the main worktree that uses it.
fn git_shim(pair: &Pair, name: &str, middle: &str) -> specengine_store::GitEnv {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = pair.scratch.dir(name);
    let real = pair.git.git_program().to_str().expect("a UTF-8 git");
    let shim = dir.join("git");
    let middle = middle.replace("REAL", &format!("'{real}'"));
    fs::write(
        &shim,
        format!("#!/bin/sh\n{middle}\nexec '{real}' \"$@\"\n"),
    )
    .expect("the shim");
    fs::set_permissions(&shim, fs::Permissions::from_mode(0o755)).expect("executable");
    let mut vars = pair.git.vars();
    vars.retain(|(name, _)| name != "PATH");
    vars.push(("PATH".into(), pair.git.path_with(&dir)));
    specengine_store::GitEnv::new(&pair.main, vars)
}

/// `git ls-files -s` of `path` in the linked worktree: `(mode, oid)`, or
/// `None` without an entry.
fn index_entry(pair: &Pair, path: &str) -> Option<(String, String)> {
    let line = pair.git_text(&pair.linked, &["ls-files", "-s", "--", path]);
    let mut fields = line.split_whitespace();
    let mode = fields.next()?.to_owned();
    let oid = fields.next()?.to_owned();
    Some((mode, oid))
}

/// O16: `HEAD`'s config names another walked document, `class: spec`, as a
/// shard: O1's apply commits the target alone, exit 0, `applied`, the warning
/// naming it, `can never be an output (a spec document)`, and the document
/// keeps its bytes on disk and stays out of the commit. M: exclude
/// only the four paths and the walk scope.
#[test]
fn o16_a_spec_document_named_as_a_shard_is_never_an_output() {
    let pair = forge("ao-o16-spec");
    write(&pair.linked, OTHER, OTHER_TEXT);
    let config = read_text(&pair.linked, "specengine.toml");
    write(&pair.linked, "specengine.toml", with_shard(&config, OTHER));
    pair.commit_all(&pair.linked, "another document a shard");
    let id = raise_create(&pair, STAMINA, STAMINA_TEXT);
    let (outcome, _) = applied(&pair, &id);
    let head = tip(&pair);
    assert_eq!(name_status(&pair, &head), [format!("A\t{STAMINA}")]);
    assert_eq!(pair.proposal(&id).status, ProposalStatus::Applied);
    let lines = stderr_of(&outcome);
    assert_eq!(
        carries(&lines),
        [alone_warning(
            STAMINA,
            &never_cause(OTHER, "a spec document")
        )],
        "{lines:?}"
    );
    assert_eq!(read_text(&pair.linked, OTHER), OTHER_TEXT, "its bytes kept");
    assert_eq!(
        blob(&pair, &head, OTHER),
        OTHER_TEXT.as_bytes(),
        "as committed"
    );
    assert_eq!(porcelain(&pair), "");
}

/// O16: an untracked, non-ignored `class: spec` document `HEAD` lacks, named as
/// a shard by `HEAD`'s config: the same warning naming it, and it keeps its
/// bytes, still untracked. M: judge `HEAD`'s tree alone.
#[test]
fn o16_an_untracked_spec_document_named_as_a_shard_keeps_its_bytes() {
    let pair = forge("ao-o16-draft");
    let config = read_text(&pair.linked, "specengine.toml");
    write(
        &pair.linked,
        "specengine.toml",
        with_shard(&config, DRAFT_OTHER),
    );
    pair.commit_all(&pair.linked, "a draft a shard");
    write(&pair.linked, DRAFT_OTHER, DRAFT_OTHER_TEXT);
    let id = raise_create(&pair, STAMINA, STAMINA_TEXT);
    let (outcome, _) = applied(&pair, &id);
    let head = tip(&pair);
    assert_eq!(name_status(&pair, &head), [format!("A\t{STAMINA}")]);
    assert_eq!(pair.proposal(&id).status, ProposalStatus::Applied);
    let lines = stderr_of(&outcome);
    assert_eq!(
        carries(&lines),
        [alone_warning(
            STAMINA,
            &never_cause(DRAFT_OTHER, "a spec document")
        )],
        "{lines:?}"
    );
    assert_eq!(read_text(&pair.linked, DRAFT_OTHER), DRAFT_OTHER_TEXT);
    assert_eq!(porcelain(&pair), format!("?? {DRAFT_OTHER}\n"), "untracked");
}

/// O16: a hand commit with `Proposal: PR-0001`, one parent, changing the
/// target, `docs/index.md` and the spec document the parent's config names as a
/// shard: approve completes nothing, `changes ...` naming it, `approved`. M:
/// exclude only the four paths and the walk scope.
#[test]
fn o16_a_commit_editing_a_spec_document_its_parent_names_completes_nothing() {
    let pair = forge("ao-o16-hand");
    write(&pair.linked, OTHER, OTHER_TEXT);
    let config = read_text(&pair.linked, "specengine.toml");
    write(&pair.linked, "specengine.toml", with_shard(&config, OTHER));
    pair.commit_all(&pair.linked, "another document a shard");
    let id = raise_create(&pair, STAMINA, STAMINA_TEXT);
    set_approved(&pair, &id);
    write(&pair.linked, STAMINA, STAMINA_TEXT);
    let index = read_text(&pair.linked, INDEX);
    write(&pair.linked, INDEX, format!("{index}\n"));
    write(&pair.linked, OTHER, format!("{OTHER_TEXT}\nA line more.\n"));
    let commit = commit_by_hand(&pair, &id, &[STAMINA, INDEX, OTHER]);
    assert_eq!(
        name_status(&pair, &commit),
        by_path(vec![
            format!("A\t{STAMINA}"),
            format!("M\t{OTHER}"),
            format!("M\t{INDEX}")
        ])
    );
    completes_nothing(&pair, &id, &commit, OTHER);
}

/// "Never an output", the bytes that do not read: an output `HEAD` lacks whose
/// file on disk is not UTF-8: the target alone, the warning `` `<p>` can never
/// be an output (not a generated document on disk) ``, the bytes kept. M: judge
/// `HEAD`'s tree alone.
#[test]
fn an_output_on_disk_that_does_not_read_is_never_overwritten() {
    let pair = forge("ao-o16-unread");
    let config = read_text(&pair.linked, "specengine.toml");
    write(
        &pair.linked,
        "specengine.toml",
        with_shard(&config, DRAFT_OTHER),
    );
    pair.commit_all(&pair.linked, "a draft a shard");
    let bytes: &[u8] = b"\xff\xfe not UTF-8\n";
    write(&pair.linked, DRAFT_OTHER, bytes);
    let id = raise_create(&pair, STAMINA, STAMINA_TEXT);
    let (outcome, _) = applied(&pair, &id);
    let head = tip(&pair);
    assert_eq!(name_status(&pair, &head), [format!("A\t{STAMINA}")]);
    let lines = stderr_of(&outcome);
    assert_eq!(
        carries(&lines),
        [alone_warning(
            STAMINA,
            &never_cause(DRAFT_OTHER, "not a generated document on disk")
        )],
        "{lines:?}"
    );
    assert_eq!(read(&pair.linked, DRAFT_OTHER), bytes, "its bytes kept");
}

/// "Never an output", its other side: an output `HEAD` lacks whose file on disk
/// reads as a generated document (`docs/index.md` untracked, not ignored) is
/// written and committed, `A docs/index.md` beside the target, with the note.
/// M: never write an output `HEAD` lacks.
#[test]
fn an_untracked_generated_output_is_written_and_committed() {
    let pair = forge("ao-o16-generated");
    untracked_index(&pair, false);
    let id = raise_create(&pair, STAMINA, STAMINA_TEXT);
    let (outcome, _) = applied(&pair, &id);
    let head = tip(&pair);
    assert_eq!(
        name_status(&pair, &head),
        [format!("A\t{STAMINA}"), format!("A\t{INDEX}")]
    );
    assert_eq!(carries(&stderr_of(&outcome)), [outputs_note(&[INDEX])]);
    assert_eq!(read(&pair.linked, INDEX), blob(&pair, &head, INDEX));
    assert_eq!(porcelain(&pair), "");
}

/// Step 9 (iteration 3): an output this run staged intent-to-add, then staged
/// for real by a person before a commit that fails (a `git` whose `commit`
/// first runs `git add -- docs/index.md`, then exits 1): exit 1, `open`, and
/// the person's entry stays, its blob no intent-to-add's. M: drop this run's
/// entries whatever they hold.
#[test]
fn a_failed_commit_keeps_an_entry_a_person_staged_meanwhile() {
    let pair = forge("ao-rollback-commit");
    untracked_index(&pair, false);
    let id = raise_create(&pair, STAMINA, STAMINA_TEXT);
    let head = tip(&pair);
    let git = git_shim(
        &pair,
        "staging-person",
        "if [ \"$1\" = -C ] && [ \"$3\" = commit ]; then\n  REAL -C \"$2\" add -- docs/index.md \
         || exit 97\n  echo 'error: the test refuses the commit' >&2\n  exit 1\nfi",
    );
    let (outcome, _) = pair.approve_answer(&pair.main, &id, true, git);
    let outcome = outcome.unwrap_or_else(|error| panic!("approve {id}: {error}"));
    assert_eq!(outcome.exit(), Exit::NotFound, "exit 1: {outcome:?}");
    assert_eq!(tip(&pair), head, "no commit");
    assert_eq!(pair.proposal(&id).status, ProposalStatus::Open);
    assert!(!pair.linked.join(STAMINA).exists(), "the new file removed");
    let entry = index_entry(&pair, INDEX).expect("the person's entry kept");
    assert_ne!(
        entry.1, EMPTY_BLOB,
        "a real entry, not an intent-to-add: {entry:?}"
    );
}

/// Step 9 (iteration 3): two outputs `HEAD` lacks; the second's staging fails
/// after a person staged the first for real (a `git` that runs `git add --
/// docs/index.md` before refusing `--intent-to-add` of the archive shard): the
/// target alone with the staging warning, and the person's entry stays. M: drop
/// this run's entries whatever they hold.
#[test]
fn a_failed_staging_keeps_an_entry_a_person_staged_meanwhile() {
    let pair = forge("ao-rollback-staging");
    pair.git.git(
        &pair.linked,
        &["rm", "-q", "--cached", "--", INDEX, ARCHIVE],
    );
    pair.git.git(
        &pair.linked,
        &["commit", "-q", "-m", "the outputs untracked"],
    );
    let id = raise_create(&pair, STAMINA, STAMINA_TEXT);
    let git = git_shim(
        &pair,
        "staging-person-too",
        "intent=0; archive=0\nfor arg in \"$@\"; do\n  case \"$arg\" in\n    \
         --intent-to-add|-N) intent=1;;\n    *docs/index-archive.md) archive=1;;\n  esac\ndone\n\
         if [ $intent = 1 ] && [ $archive = 1 ]; then\n  REAL -C \"$2\" add -- docs/index.md \
         || exit 97\n  echo 'fatal: the test refuses to stage docs/index-archive.md' >&2\n  \
         exit 128\nfi",
    );
    let (outcome, _) = pair.approve_answer(&pair.main, &id, true, git);
    let outcome = outcome.unwrap_or_else(|error| panic!("approve {id}: {error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    let head = tip(&pair);
    assert_eq!(name_status(&pair, &head), [format!("A\t{STAMINA}")]);
    let lines = carries(&stderr_of(&outcome));
    assert!(
        lines.len() == 1 && lines[0].contains(&format!("staging `{ARCHIVE}` failed: ")),
        "{lines:?}"
    );
    let entry = index_entry(&pair, INDEX).expect("the person's entry kept");
    assert_ne!(
        entry.1, EMPTY_BLOB,
        "a real entry, not an intent-to-add: {entry:?}"
    );
    assert_eq!(
        index_entry(&pair, ARCHIVE),
        None,
        "no entry left for the archive"
    );
}
