//! docs/features/reference-promotion.md (RM-0138, ADR-0089): the code
//! review's findings of iteration 1, each a case the criteria imply, through
//! the `spec` binary over scratch repositories of `fixtures/promote-mini`
//! (approve on a pseudo-terminal), as `tests/reference_promotion.rs`.
//!
//! - Finding 1 (a blocker): entries ending in multibyte characters
//!   (Cyrillic `\u{430}\u{440}\u{442}/...`, `art/\u{e9}\u{e9}`) never panic `spec check`, `spec index` or an
//!   update's propose; the index keeps them. M: the byte slice in
//!   `is_markdown`.
//! - Finding 2: an LFS home whose path already holds the reference's raw
//!   bytes (untracked, no `filter.lfs` configured): the apply refuses or
//!   commits the pointer, never the raw blob; a pointer placed by hand in a
//!   plain home is refused; step 10 judges the blob a commit added (a
//!   `pre-commit` hook rewriting the file: the apply does not end
//!   `applied`).
//! - Finding 3: "in place" is `HEAD`'s, not git's index: an equal file
//!   staged but not committed is added by the apply; after a commit a hook
//!   refused, no intent-to-add entry stays and the next apply adds the file.
//! - Finding 5: a sibling temp file a killed copy left in the home is gone
//!   after the next apply, the worktree clean.
//! - Finding 7: `slug/ID` resolves to its exact feature document, never to
//!   one whose slug merely begins with it.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;
#[path = "promote_common/mod.rs"]
mod promote_common;
#[path = "work_refs_common/mod.rs"]
mod refs_common;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;

use promote_common::lfs::{self, Pointer};
use promote_common::{
    HARBOUR_HOME, HARBOUR_MD, Lfs, MIRA_HOME, MIRA_MD, Scene, b3, before_fence, id_of,
};
use refs_common::{blob_fixture, png};
use serde_json::json;

const HARBOUR_PNG: &str = "docs/spec/world/media/harbour.png";
const MIRA_PNG: &str = "docs/spec/cast/art/mira.png";

/// C-0001 on LOC-HARBOUR with harbour.png (RF-0001), C-0002 on CHR-MIRA
/// with a fresh `mira.png` (RF-0002): the scene and the two files' bytes.
fn two_refs(label: &str) -> (Scene, Vec<u8>, Vec<u8>) {
    let scene = Scene::new(label);
    scene.card("Light the harbour", &["LOC-HARBOUR"]);
    scene.card("Draw Mira", &["CHR-MIRA"]);
    let harbour = fs::read(blob_fixture("harbour.png")).expect("the fixture");
    assert_eq!(
        id_of(&scene.file_ref("C-0001", &blob_fixture("harbour.png"), &[])),
        "RF-0001"
    );
    let mira = png(64, 48, 0x5a, None);
    let file = scene.scratch_file("mira.png", &mira);
    assert_eq!(id_of(&scene.file_ref("C-0002", &file, &[])), "RF-0002");
    (scene, harbour, mira)
}

/// `git hash-object --no-filters` of `bytes`: the raw blob's ID.
fn raw_blob_id(scene: &Scene, bytes: &[u8]) -> String {
    let file = scene.scratch_file("raw.bin", bytes);
    scene
        .git(
            Lfs::Off,
            &["hash-object", "--no-filters", file.to_str().expect("UTF-8")],
        )
        .trim()
        .to_owned()
}

/// Whether any commit's tree holds the object `id`.
fn reachable(scene: &Scene, id: &str) -> bool {
    scene
        .git(Lfs::Off, &["rev-list", "--objects", "--all"])
        .lines()
        .any(|line| line.split_whitespace().next() == Some(id))
}

/// A `pre-commit` hook of the main repository running `body`.
fn hook(scene: &Scene, body: &str) {
    let hooks = scene.pair.main.join(".git/hooks");
    fs::create_dir_all(&hooks).expect("hooks");
    let path = hooks.join("pre-commit");
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("the hook");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("chmod");
}

// ------------------------------------------------------------ finding 1

/// Finding 1: multibyte entries: `spec check` answers (exit 0 or 1, no
/// panic), `spec index` keeps both, an update's propose stores. M: the byte
/// slice in `is_markdown`.
#[test]
fn review_multibyte_entries_never_panic() {
    let scene = Scene::new("pr-review-utf8");
    let cyrillic = "\u{430}\u{440}\u{442}/\u{43c}\u{438}\u{440}\u{430}";
    let latin = "art/\u{e9}\u{e9}";
    let document = scene.text(HARBOUR_MD);
    scene.commit_file(
        HARBOUR_MD,
        before_fence(
            &document,
            &format!("references:\n  - \"{cyrillic}\"\n  - \"{latin} | the light\"\n"),
        ),
        "Multibyte entries.",
    );
    let checked = scene.json_run(Lfs::Off, &["check"]);
    assert!(
        checked.code == 0 || checked.code == 1,
        "M: a panic (101): {}",
        checked.show()
    );
    let indexed = scene.spec(Lfs::Off, &["index"]);
    assert_eq!(indexed.code, 0, "{}", indexed.show());
    let rows = scene.pair.sql(
        "SELECT p.path FROM promoted p JOIN files f ON f.file_id = p.file_id \
         WHERE f.path = 'docs/spec/world/harbour.md' ORDER BY p.ord;",
    );
    assert_eq!(
        rows,
        format!("docs/spec/world/{cyrillic}\ndocs/spec/world/{latin}\n"),
        "both kept, root-relative"
    );
    let (base, text) = scene.pair.span(&scene.pair.main, "LOC-HARBOUR");
    let edited = text.replacen("before dawn.", "before first light.", 1);
    let proposed = scene.piped(
        Lfs::Off,
        &[
            "--json",
            "propose",
            "update",
            "LOC-HARBOUR",
            "--base",
            &base,
            "--text-file",
            "-",
            "--rationale",
            "Wording.",
        ],
        edited.as_bytes(),
    );
    assert_eq!(proposed.code, 0, "M: a panic: {}", proposed.show());
}

// ------------------------------------------------------------ finding 2

/// Finding 2: the LFS home's path already holds the reference's raw bytes,
/// untracked (no `filter.lfs` configured, so git would store them raw),
/// placed before the propose or after it: the apply refuses or commits the
/// pointer, never the raw blob.
#[test]
fn review_raw_bytes_in_an_lfs_home_are_never_committed() {
    for before_propose in [true, false] {
        let (scene, harbour, _) = two_refs(if before_propose {
            "pr-review-raw-early"
        } else {
            "pr-review-raw-late"
        });
        let place = |scene: &Scene| {
            fs::create_dir_all(scene.pair.main.join(HARBOUR_HOME)).expect("the home");
            fs::write(scene.pair.main.join(HARBOUR_PNG), &harbour).expect("the raw bytes");
        };
        if before_propose {
            place(&scene);
        }
        let proposed = scene.propose(Lfs::On, "RF-0001", "LOC-HARBOUR", &[]);
        if !before_propose {
            assert_eq!(proposed.code, 0, "{}", proposed.show());
            place(&scene);
        }
        let raw = raw_blob_id(&scene, &harbour);
        if proposed.code == 0 {
            let id = id_of(&proposed.json());
            let done = scene.approve(Lfs::On, &id);
            assert!(
                done.code == 0 || done.code == 1,
                "before_propose {before_propose}: {}",
                done.output
            );
            let at_head = scene.git_output(
                Lfs::Off,
                &["cat-file", "blob", &format!("HEAD:{HARBOUR_PNG}")],
            );
            if at_head.status.success() {
                assert!(
                    Pointer::parse(&at_head.stdout).is_some(),
                    "before_propose {before_propose}: HEAD holds no pointer but {} B",
                    at_head.stdout.len()
                );
            }
        }
        assert!(
            !reachable(&scene, &raw),
            "before_propose {before_propose}: the raw blob reached a commit"
        );
    }
}

/// Finding 2: a pointer placed by hand in a plain home (the pointer of the
/// reference's own bytes) is never taken for the file: the propose or the
/// apply refuses, and no commit adds it.
#[test]
fn review_a_pointer_in_a_plain_home_is_refused() {
    let (scene, _, mira) = two_refs("pr-review-pointer");
    let sha = lfs::sha256_bytes(scene.pair.scratch.path(), &mira);
    let pointer = Pointer {
        oid: sha,
        size: mira.len() as u64,
    }
    .bytes();
    fs::create_dir_all(scene.pair.main.join(MIRA_HOME)).expect("the home");
    fs::write(scene.pair.main.join(MIRA_PNG), &pointer).expect("the pointer");
    let head = scene.head();
    let proposed = scene.propose(Lfs::On, "RF-0002", "CHR-MIRA", &[]);
    if proposed.code == 0 {
        let id = id_of(&proposed.json());
        let done = scene.approve(Lfs::On, &id);
        assert_eq!(
            done.code, 1,
            "a pointer is no file of a plain home: {}",
            done.output
        );
    } else {
        assert_eq!(proposed.code, 1, "{}", proposed.show());
    }
    assert_eq!(scene.head(), head, "no commit");
    assert_eq!(scene.file(MIRA_PNG), Some(pointer), "the hand's file kept");
}

/// Finding 2, step 10: a `pre-commit` hook that rewrites the file and
/// stages it: the commit holds another blob than the one computed; the
/// apply does not end `applied`, and says so naming the commit.
#[test]
fn review_step_10_judges_the_added_blob() {
    let (scene, _, _) = two_refs("pr-review-step10");
    let id = scene.proposed(Lfs::Off, "RF-0002", "CHR-MIRA", &[]);
    hook(
        &scene,
        &format!(
            "if [ -f {MIRA_PNG} ]; then printf 'tampered' > {MIRA_PNG}; git add {MIRA_PNG}; fi\nexit 0"
        ),
    );
    let head = scene.head();
    let done = scene.approve(Lfs::Off, &id);
    let committed = scene.head();
    assert_ne!(
        committed, head,
        "premise: git made the commit, the hook's blob in it: {}",
        done.output
    );
    let blob = scene.git_bytes(&["cat-file", "blob", &format!("HEAD:{MIRA_PNG}")]);
    assert_eq!(blob, b"tampered", "premise: the hook's blob was committed");
    assert_eq!(done.code, 1, "step 10 refuses: {}", done.output);
    assert_ne!(scene.column(&id, "status"), "applied", "{}", done.output);
    assert!(
        done.output.contains(&committed[..7]),
        "the commit named: {}",
        done.output
    );
}

// ------------------------------------------------------------ finding 3

/// Finding 3: an equal file staged (`git add`) but not committed is not in
/// place: `HEAD` lacks it, so the apply adds it, one commit of the file and
/// the document.
#[test]
fn review_a_staged_file_is_not_in_place() {
    let (scene, _, mira) = two_refs("pr-review-staged");
    let id = scene.proposed(Lfs::Off, "RF-0002", "CHR-MIRA", &[]);
    fs::create_dir_all(scene.pair.main.join(MIRA_HOME)).expect("the home");
    fs::write(scene.pair.main.join(MIRA_PNG), &mira).expect("the file");
    scene.git(Lfs::Off, &["add", MIRA_PNG]);
    let output = scene.applied(Lfs::Off, &id);
    assert!(!output.contains("is in place"), "{output}");
    assert!(output.contains(&format!("adds {MIRA_PNG}")), "{output}");
    assert_eq!(
        scene.changes(&scene.head()),
        vec![format!("A {MIRA_PNG}"), format!("M {MIRA_MD}")]
    );
    assert_eq!(b3(&scene.file(MIRA_PNG).expect("the file")), b3(&mira));
    assert_eq!(scene.status(), "");
}

/// Finding 3: after a commit a hook refused, the index keeps no entry of the
/// file (no intent-to-add), and the next apply adds it.
#[test]
fn review_a_failed_commit_leaves_no_index_entry() {
    let (scene, _, mira) = two_refs("pr-review-ita");
    let id = scene.proposed(Lfs::Off, "RF-0002", "CHR-MIRA", &[]);
    hook(&scene, "echo 'the hook says no' >&2\nexit 1");
    let done = scene.approve(Lfs::Off, &id);
    assert_eq!(done.code, 1, "{}", done.output);
    assert_eq!(
        scene.git(Lfs::Off, &["ls-files", "--stage", "--", MIRA_PNG]),
        "",
        "no index entry of the file"
    );
    assert_eq!(scene.status(), "");
    fs::remove_file(scene.pair.main.join(".git/hooks/pre-commit")).expect("the hook removed");
    let output = scene.applied(Lfs::Off, &id);
    assert!(!output.contains("is in place"), "{output}");
    assert_eq!(
        scene.changes(&scene.head()),
        vec![format!("A {MIRA_PNG}"), format!("M {MIRA_MD}")]
    );
    assert_eq!(
        scene.git_bytes(&["cat-file", "blob", &format!("HEAD:{MIRA_PNG}")]),
        mira
    );
    assert_eq!(scene.status(), "");
}

// ------------------------------------------------------------ finding 5

/// Finding 5: a dot-named sibling temp a killed copy left in the home
/// (`.<name>.specengine-<pid>-<n>.tmp`) is gone after the next apply; the
/// worktree is clean.
#[test]
fn review_a_killed_copys_temp_is_swept() {
    let (scene, _, mira) = two_refs("pr-review-temp");
    let id = scene.proposed(Lfs::Off, "RF-0002", "CHR-MIRA", &[]);
    fs::create_dir_all(scene.pair.main.join(MIRA_HOME)).expect("the home");
    let stray = format!("{MIRA_HOME}/.mira.png.specengine-99999-0.tmp");
    fs::write(scene.pair.main.join(&stray), &mira[..mira.len() / 2]).expect("a half copy");
    let output = scene.applied(Lfs::Off, &id);
    assert!(output.contains(&format!("adds {MIRA_PNG}")), "{output}");
    assert_eq!(
        scene.temps(),
        Vec::<String>::new(),
        "no temp in the worktree"
    );
    assert_eq!(scene.status(), "");
    assert_eq!(b3(&scene.file(MIRA_PNG).expect("the file")), b3(&mira));
}

// ------------------------------------------------------------ finding 7

/// Finding 7: `harbour/AC-01` resolves to the feature `harbour.md`, not to
/// `harbour-night.md`, whose slug begins with `harbour` and sorts first.
#[test]
fn review_slug_id_names_its_exact_feature() {
    let scene = Scene::new("pr-review-slug");
    let config = scene.text("specengine.toml");
    let config = config.replacen(
        "RULE = { kind = \"rule\",      shape = \"name\" }\n",
        "RULE = { kind = \"rule\",      shape = \"name\" }\nAC   = { kind = \"criterion\", width = 2, scope = \"feature\" }\n",
        1,
    );
    let config = config.replacen(
        "[zones]",
        "[[references.homes]]\nkinds = [\"criterion\"]\ndir   = \"media\"\n\n[zones]",
        1,
    );
    assert!(config.contains("scope = \"feature\""), "premise: {config}");
    scene.commit_file("specengine.toml", &config, "A feature kind with a home.");
    for (slug, title) in [
        ("harbour-night", "The harbour at night"),
        ("harbour", "The harbour"),
    ] {
        scene.commit_file(
            &format!("docs/features/{slug}.md"),
            format!(
                "---\nclass: spec\nkind: criterion\nstatus: draft\n---\n\n# {title}\n\n\
                 ## Acceptance criteria {{#acceptance}}\n\n\
                 ### The lamp is lit {{#AC-01}}\n\nThe lamp burns from dusk to dawn.\n"
            ),
            &format!("The feature {slug}."),
        );
    }
    scene.card("The lamp", &["harbour/AC-01"]);
    let rf = id_of(&scene.file_ref("C-0001", &blob_fixture("harbour.png"), &[]));
    let id = scene.proposed(Lfs::On, &rf, "harbour/AC-01", &[]);
    assert_eq!(scene.column(&id, "target_path"), "docs/features/harbour.md");
    assert_eq!(scene.column(&id, "new_text"), "media/harbour.png");
    let review = scene.doc(&["review", &id]);
    assert!(
        review["notes"]
            .as_array()
            .expect("notes")
            .iter()
            .any(|note| note
                .as_str()
                .is_some_and(|note| note.contains("docs/features/media/harbour.png"))),
        "{}",
        review["notes"]
    );
    let _ = json!(null);
}

/// A pid no process holds: the highest of 2..=99_999 `kill -0` finds
/// dead (none running there now).
fn dead_pid() -> u32 {
    (2..=99_999u32)
        .rev()
        .find(|pid| {
            !std::process::Command::new("/bin/kill")
                .args(["-0", &pid.to_string()])
                .stderr(std::process::Stdio::null())
                .status()
                .expect("kill -0 runs")
                .success()
        })
        .expect("a free pid")
}

/// The re-review's iteration 3: step 8 sweeps a killed run's temp files out
/// of SpecEngine's places — beside the file (`.<name>.specengine-<pid>-
/// <n>.tmp`), in `<common>/specengine-tmp/` and in `<lfs storage>/tmp/
/// specengine-<pid>-<n>` — when their pid runs no more; a live run's stay
/// (younger than an hour). A dead pid's three go at the next apply (plain,
/// then LFS), a live pid's (a sleeping child of this test) stay.
#[test]
fn review_a_dead_runs_temps_go_and_a_live_runs_stay() {
    let (scene, harbour, mira) = two_refs("pr-review-dead-temps");
    let common = scene.pair.main.join(".git");
    let mut live = std::process::Command::new("/usr/bin/perl")
        .args(["-e", "alarm 120; sleep 120"])
        .spawn()
        .expect("a live process");
    let (dead, alive) = (dead_pid(), live.id());
    let places = |pid: u32| {
        [
            scene
                .pair
                .main
                .join(format!("{MIRA_HOME}/.mira.png.specengine-{pid}-0.tmp")),
            common.join(format!("specengine-tmp/.mira.png.specengine-{pid}-0.tmp")),
            common.join(format!("lfs/tmp/specengine-{pid}-0")),
        ]
    };
    for path in places(dead).iter().chain(places(alive).iter()) {
        fs::create_dir_all(path.parent().expect("a directory")).expect("the directory");
        fs::write(path, &mira[..mira.len() / 2]).expect("a run's temp");
    }
    let plain = scene.proposed(Lfs::Off, "RF-0002", "CHR-MIRA", &[]);
    scene.applied(Lfs::Off, &plain);
    assert_eq!(b3(&scene.file(MIRA_PNG).expect("the file")), b3(&mira));
    let lfs_id = scene.proposed(Lfs::On, "RF-0001", "LOC-HARBOUR", &[]);
    scene.applied(Lfs::On, &lfs_id);
    let sha = lfs::sha256_bytes(scene.pair.scratch.path(), &harbour);
    assert_eq!(
        fs::read(lfs::lfs_object(&common, &sha)).ok(),
        Some(harbour),
        "the object written"
    );
    let left = |pid: u32| -> Vec<bool> { places(pid).iter().map(|path| path.exists()).collect() };
    let dead_left = left(dead);
    let live_left = left(alive);
    let _ = live.kill();
    let _ = live.wait();
    assert_eq!(
        dead_left,
        [false, false, false],
        "a dead pid's temps swept (pid {dead})"
    );
    assert_eq!(
        live_left,
        [true, true, true],
        "a live pid's temps kept (pid {alive})"
    );
}

// --------------------------------------------- the release gate's lines

/// The release gate: the `Listed` refusal names the entry already kept, at
/// propose and at apply, never the document. M: the document's path in
/// the line.
#[test]
fn gate_the_listed_line_names_the_entry() {
    let hand = "references:\n  - \"art/mira.png | an old sketch\"\n";
    let want = "`art/mira.png` is already listed in `CHR-MIRA`'s references";
    let wrong = format!("`{MIRA_MD}` is already listed");

    // At propose: the key keeps the path already.
    let (scene, _, _) = two_refs("pr-gate-listed-propose");
    scene.commit_file(
        MIRA_MD,
        before_fence(&scene.text(MIRA_MD), hand),
        "Listed by hand.",
    );
    let before = scene.kept();
    let done = scene.propose(Lfs::Off, "RF-0002", "CHR-MIRA", &[]);
    assert_eq!(done.code, 1, "{}", done.show());
    assert!(done.stderr.contains(want), "{}", done.show());
    assert!(!done.stderr.contains(&wrong), "{}", done.show());
    assert_eq!(scene.kept(), before, "nothing stored");

    // At apply: the entry written by hand after the propose.
    let (scene, _, _) = two_refs("pr-gate-listed-apply");
    let id = scene.proposed(Lfs::Off, "RF-0002", "CHR-MIRA", &[]);
    scene.commit_file(
        MIRA_MD,
        before_fence(&scene.text(MIRA_MD), hand),
        "Listed by hand.",
    );
    let head = scene.head();
    let document = scene.text(MIRA_MD);
    let done = scene.approve(Lfs::Off, &id);
    assert_eq!(done.code, 1, "{}", done.output);
    assert!(done.output.contains(want), "{}", done.output);
    assert!(!done.output.contains(&wrong), "{}", done.output);
    assert!(scene.file(MIRA_PNG).is_none(), "no file");
    assert_eq!(scene.text(MIRA_MD), document, "the document as it was");
    assert_eq!(scene.head(), head, "HEAD unmoved");
}

/// The release gate: `HEAD` holds the promoted path, and the store cannot
/// give the reference's file: gone (the reference's own check) or there but
/// unreadable (the occupancy check, which cannot judge the bytes in place):
/// each refuses with `<RF>'s file is not on this host: nothing to promote`,
/// nothing stored. M: the bytes judged other without the stored file.
#[test]
fn gate_a_lost_stored_file_under_a_held_path_is_not_on_this_host() {
    let line = "RF-0002's file is not on this host: nothing to promote";
    for unreadable in [false, true] {
        let label = if unreadable {
            "pr-gate-unhosted-unreadable"
        } else {
            "pr-gate-unhosted-gone"
        };
        let (scene, _, mira) = two_refs(label);
        scene.commit_file(MIRA_PNG, &mira, "Mira by hand.");
        let stored = scene.store().join(b3(&mira));
        assert!(stored.is_file(), "premise: {}", stored.display());
        if unreadable {
            fs::set_permissions(&stored, fs::Permissions::from_mode(0o000)).expect("chmod 000");
        } else {
            fs::remove_file(&stored).expect("remove the stored file");
        }
        let before = scene.kept();
        let done = scene.propose(Lfs::Off, "RF-0002", "CHR-MIRA", &[]);
        if unreadable {
            fs::set_permissions(&stored, fs::Permissions::from_mode(0o644)).expect("chmod back");
        }
        assert_eq!(done.code, 1, "{unreadable}: {}", done.show());
        assert!(done.stderr.contains(line), "{unreadable}: {}", done.show());
        assert!(
            !done.stderr.contains("holds other bytes"),
            "{unreadable}: {}",
            done.show()
        );
        assert_eq!(scene.kept(), before, "{unreadable}: nothing stored");
    }
}
