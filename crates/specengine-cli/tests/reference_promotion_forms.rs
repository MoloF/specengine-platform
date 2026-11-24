//! docs/features/reference-promotion.md (RM-0138, ADR-0089) "Acceptance
//! criteria" PR14, PR15 and PR7's terminal half: the forms a promotion
//! meets at propose and apply, over scratch repositories of
//! `fixtures/promote-mini` through the `spec` binary (approve on a
//! pseudo-terminal), as `tests/reference_promotion.rs`.
//!
//! - PR14: a body line edited after propose: `rebases`, the commit keeps it
//!   and adds the entry; the key rewritten `[a.png]`: exit 1 `is not a block
//!   list`; `references: []` becomes a block of one; a CRLF front-matter
//!   gets a CRLF line; a path already kept: `already listed`; a second live
//!   promotion of one reference to one document: exit 1 naming the first; a
//!   commit failing in a hook: the file, its entry and new directories
//!   removed, the document restored, exit 1; a hand commit with the trailer
//!   completes it. M: the propose-time text with the entry written over the
//!   edit.
//! - PR15: a link: exit 1; a JPEG stored as `harbour.png`: exit 1 naming
//!   `.jpg, .jpeg`, `--name harbour.jpg` stores; `a|b.png`, `.hidden.png`,
//!   `x.png.` refused; a section target noted; "what matters" of 513
//!   characters, given or the reference's own: exit 1; `[ids]` claiming `RF`
//!   (or `PR`): exit 2. M: the extension rule dropped.
//! - PR7, the terminal: under `mode = "both"` covering every path, a pack's
//!   `reference` proposal is held `proposal-kind`, `a reference waits for a
//!   person`, while an update of the same task applies; no `Delegated-by:`
//!   commit. M: `reference` added to the covered kinds.
//!
//! Every expected value is worked out from the spec and
//! docs/canon/work-references-promotion.md. Non-Latin characters in Rust
//! sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;
#[path = "promote_common/mod.rs"]
mod promote_common;
#[path = "work_refs_common/mod.rs"]
mod refs_common;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;

use common::with_ids_line;
use promote_common::lfs::size_text;
use promote_common::{HARBOUR_MD, Lfs, MIRA_HOME, MIRA_MD, SLUG, Scene, b3, before_fence, id_of};
use refs_common::{blob_fixture, png};
use serde_json::{Value, json};
use specengine_core::auto_toml::AutoRule;
use specengine_store::{AutoEvent, AutoStarted, SqliteQueue};

const MIRA_PNG: &str = "docs/spec/cast/art/mira.png";

/// C-0001 on CHR-MIRA with a fresh `mira.png` (RF-0001): the scene and the
/// bytes.
fn mira_scene(label: &str) -> (Scene, Vec<u8>) {
    let scene = Scene::new(label);
    scene.card("Draw Mira", &["CHR-MIRA"]);
    let mira = png(40, 30, 0x2c, None);
    let file = scene.scratch_file("mira.png", &mira);
    assert_eq!(id_of(&scene.file_ref("C-0001", &file, &[])), "RF-0001");
    (scene, mira)
}

/// Every file and directory of the worktree, the `HEAD` and the status:
/// what an apply refused must leave.
fn worktree(scene: &Scene) -> String {
    let kept = scene.kept();
    kept.lines()
        .filter(|line| {
            line.starts_with("HEAD ") || line.starts_with("status ") || line.starts_with("files ")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// ------------------------------------------------------------------ PR14

/// PR14: a body line edited after propose: the apply `rebases`, its commit
/// keeps the edit and adds the entry. M: the propose-time text with the
/// entry written over the edit.
#[test]
fn pr14_an_edit_after_propose_rebases_and_stays() {
    let (scene, mira) = mira_scene("pr-pr14-rebase");
    let id = scene.proposed(Lfs::Off, "RF-0001", "CHR-MIRA", &[]);
    let edited = scene.text(MIRA_MD).replace("by its sail.", "by its mast.");
    assert_ne!(edited, scene.text(MIRA_MD), "premise: the line is there");
    scene.commit_file(MIRA_MD, &edited, "Mast, not sail.");
    let output = scene.applied(Lfs::Off, &id);
    assert!(
        output.contains(&format!(
            "(rebases; adds {MIRA_PNG}, RF-0001, image/png {}, plain)? [y/N]",
            size_text(mira.len() as u64)
        )),
        "{output}"
    );
    assert_eq!(
        scene.text(MIRA_MD),
        before_fence(&edited, "references:\n  - \"art/mira.png\"\n"),
        "M: the edit kept, the entry added"
    );
    assert_eq!(
        scene.git(Lfs::Off, &["show", &format!("HEAD:{MIRA_MD}")]),
        scene.text(MIRA_MD),
        "committed"
    );
}

/// PR14: the key rewritten as a flow list after propose: exit 1 `is not a
/// block list`, nothing written.
#[test]
fn pr14_a_key_no_longer_a_block_list_is_refused_at_apply() {
    let (scene, _) = mira_scene("pr-pr14-flow");
    let id = scene.proposed(Lfs::Off, "RF-0001", "CHR-MIRA", &[]);
    let document = scene.text(MIRA_MD);
    scene.commit_file(
        MIRA_MD,
        before_fence(&document, "references: [a.png]\n"),
        "A flow list.",
    );
    let before = worktree(&scene);
    let done = scene.approve(Lfs::Off, &id);
    assert_eq!(done.code, 1, "{}", done.output);
    assert!(
        done.output.contains(&format!(
            "`{MIRA_MD}`'s references: is not a block list: write it as `references:` and `- ` lines"
        )),
        "{}",
        done.output
    );
    assert_eq!(worktree(&scene), before, "nothing written");
}

/// PR14: `references: []` becomes a block of one; a CRLF front-matter gets
/// a CRLF line.
#[test]
fn pr14_an_empty_list_and_crlf() {
    let (scene, _) = mira_scene("pr-pr14-forms");
    let document = scene.text(MIRA_MD);
    scene.commit_file(
        MIRA_MD,
        before_fence(&document, "references: []\n"),
        "An empty list.",
    );
    let id = scene.proposed(Lfs::Off, "RF-0001", "CHR-MIRA", &[]);
    scene.applied(Lfs::Off, &id);
    assert_eq!(
        scene.text(MIRA_MD),
        before_fence(&document, "references:\n  - \"art/mira.png\"\n")
    );

    let (scene, _) = mira_scene("pr-pr14-crlf");
    let document = scene.text(MIRA_MD);
    let close = document.find("\n---\n").expect("a closing fence") + 5;
    let crlf = format!(
        "{}{}",
        document[..close].replace('\n', "\r\n"),
        &document[close..]
    );
    scene.commit_file(MIRA_MD, &crlf, "CRLF front-matter.");
    let id = scene.proposed(Lfs::Off, "RF-0001", "CHR-MIRA", &[]);
    scene.applied(Lfs::Off, &id);
    let fence = crlf.find("\r\n---\r\n").expect("the CRLF fence") + 2;
    assert_eq!(
        scene.text(MIRA_MD),
        format!(
            "{}references:\r\n  - \"art/mira.png\"\r\n{}",
            &crlf[..fence],
            &crlf[fence..]
        )
    );
}

/// PR14: a path the key already keeps: `already listed`, nothing stored; a
/// second live promotion of one reference to one document: exit 1 naming
/// the first.
#[test]
fn pr14_a_kept_path_and_a_live_twin_are_refused() {
    let (scene, _) = mira_scene("pr-pr14-twins");
    let document = scene.text(MIRA_MD);
    scene.commit_file(
        MIRA_MD,
        before_fence(
            &document,
            "references:\n  - \"art/mira.png | an old sketch\"\n",
        ),
        "Listed by hand.",
    );
    let before = scene.kept();
    let done = scene.propose(Lfs::Off, "RF-0001", "CHR-MIRA", &[]);
    assert_eq!(done.code, 1, "{}", done.show());
    assert!(
        done.stderr
            .contains("is already listed in `CHR-MIRA`'s references"),
        "{}",
        done.show()
    );
    assert_eq!(scene.kept(), before, "nothing stored");

    let (scene, _) = mira_scene("pr-pr14-live");
    let first = scene.proposed(Lfs::Off, "RF-0001", "CHR-MIRA", &[]);
    let before = scene.kept();
    let done = scene.propose(Lfs::Off, "RF-0001", "CHR-MIRA", &["--name", "mira-2.png"]);
    assert_eq!(done.code, 1, "{}", done.show());
    assert!(
        done.stderr.contains(&format!(
            "RF-0001 is already proposed for `CHR-MIRA` by {first} (open)"
        )),
        "{}",
        done.show()
    );
    assert_eq!(scene.kept(), before, "nothing stored");
}

/// PR14: a commit a hook refuses: the file, its entry and the new directory
/// removed, the document restored, exit 1; then a hand commit carrying the
/// trailer completes the proposal.
#[test]
fn pr14_a_failed_commit_is_undone_and_a_hand_commit_completes() {
    let (scene, mira) = mira_scene("pr-pr14-hook");
    let id = scene.proposed(Lfs::Off, "RF-0001", "CHR-MIRA", &[]);
    let hooks = scene.pair.main.join(".git/hooks");
    fs::create_dir_all(&hooks).expect("hooks");
    let hook = hooks.join("pre-commit");
    fs::write(&hook, "#!/bin/sh\necho 'the hook says no' >&2\nexit 1\n").expect("the hook");
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).expect("chmod");
    let document = scene.text(MIRA_MD);
    let head = scene.head();

    let done = scene.approve(Lfs::Off, &id);
    assert_eq!(done.code, 1, "{}", done.output);
    assert!(done.output.contains("the hook says no"), "{}", done.output);
    assert_eq!(scene.text(MIRA_MD), document, "the document restored");
    assert!(scene.file(MIRA_PNG).is_none(), "the file removed");
    assert!(
        !scene.pair.main.join(MIRA_HOME).exists(),
        "the new directory removed"
    );
    assert_eq!(scene.head(), head);
    assert_eq!(scene.status(), "");
    assert_eq!(scene.temps(), Vec::<String>::new());
    assert_eq!(scene.column(&id, "status"), "open", "reopened");

    // By hand: the file and the entry, one commit with the trailer.
    fs::remove_file(&hook).expect("the hook removed");
    fs::create_dir_all(scene.pair.main.join(MIRA_HOME)).expect("the home");
    fs::write(scene.pair.main.join(MIRA_PNG), &mira).expect("the file");
    fs::write(
        scene.pair.main.join(MIRA_MD),
        before_fence(&document, "references:\n  - \"art/mira.png\"\n"),
    )
    .expect("the entry");
    scene.git(Lfs::Off, &["add", MIRA_PNG, MIRA_MD]);
    scene.git(
        Lfs::Off,
        &[
            "commit",
            "-q",
            "-m",
            &format!("Promote Mira's drawing by hand\n\nProposal: {id}\n"),
        ],
    );
    let hand = scene.head();
    let done = scene.approve(Lfs::Off, &id);
    assert_eq!(done.code, 0, "{}", done.output);
    assert!(
        done.output
            .contains(&format!("complete {id} by its commit {hand}"))
            || done
                .output
                .contains(&format!("complete {id} by its commit {}", &hand[..12])),
        "{}",
        done.output
    );
    assert_eq!(scene.column(&id, "status"), "applied");
    assert_eq!(scene.column(&id, "applied_commit"), hand);
    assert_eq!(scene.head(), hand, "no second commit");
    assert_eq!(b3(&scene.file(MIRA_PNG).expect("the file")), b3(&mira));
}

// ------------------------------------------------------------------ PR15

/// PR15: a link is never promoted.
#[test]
fn pr15_a_link_is_refused() {
    let (scene, _) = mira_scene("pr-pr15-link");
    let link = scene.doc(&[
        "ref",
        "add",
        "--on",
        "C-0001",
        "--url",
        "https://example.org/mira",
    ]);
    let rf = id_of(&link);
    let before = scene.kept();
    let done = scene.propose(Lfs::Off, &rf, "CHR-MIRA", &[]);
    assert_eq!(done.code, 1, "{}", done.show());
    assert!(
        done.stderr
            .contains(&format!("{rf} is a link: only a file is promoted into git")),
        "{}",
        done.show()
    );
    assert_eq!(scene.kept(), before);
}

/// PR15: a JPEG stored as `harbour.png` is refused naming its extensions;
/// `--name harbour.jpg` stores; a pipe, a leading dot and a trailing dot or
/// space refused. M: the extension rule dropped.
#[test]
fn pr15_the_name_keeps_its_types_extension() {
    let scene = Scene::new("pr-pr15-name");
    scene.card("Light the harbour", &["LOC-HARBOUR"]);
    let jpeg = fs::read(blob_fixture("dusk.jpg")).expect("the JPEG");
    let file = scene.scratch_file("harbour.png", &jpeg);
    let rf = id_of(&scene.file_ref("C-0001", &file, &[]));
    let before = scene.kept();
    let done = scene.propose(Lfs::On, &rf, "LOC-HARBOUR", &[]);
    assert_eq!(done.code, 1, "{}", done.show());
    assert!(
        done.stderr.contains(
            "name `harbour.png` does not end in an extension of image/jpeg (.jpg, .jpeg): give one with --name"
        ),
        "M: the extension rule dropped: {}",
        done.show()
    );
    assert_eq!(scene.kept(), before);
    for name in ["a|b.png", ".hidden.png", "x.png.", "x.png ", "harbour.md"] {
        let done = scene.propose(Lfs::On, &rf, "LOC-HARBOUR", &["--name", name]);
        assert_eq!(done.code, 1, "{name:?}: {}", done.show());
        assert_eq!(scene.kept(), before, "{name:?}: nothing stored");
    }
    // Case-insensitive, either extension of the type.
    let id = scene.proposed(Lfs::On, &rf, "LOC-HARBOUR", &["--name", "harbour.JPEG"]);
    assert_eq!(scene.column(&id, "new_text"), "media/harbour.JPEG");
    let scene = Scene::new("pr-pr15-name-2");
    scene.card("Light the harbour", &["LOC-HARBOUR"]);
    let file = scene.scratch_file("harbour.png", &jpeg);
    let rf = id_of(&scene.file_ref("C-0001", &file, &[]));
    let id = scene.proposed(Lfs::On, &rf, "LOC-HARBOUR", &["--name", "harbour.jpg"]);
    assert_eq!(scene.column(&id, "new_text"), "media/harbour.jpg");
}

/// PR15, D3: a section target is its document's, noted.
#[test]
fn pr15_a_section_target_goes_beside_its_document() {
    let scene = Scene::new("pr-pr15-section");
    scene.card("The tides' rule", &["RULE-HARBOUR-TIDES"]);
    let rf = id_of(&scene.file_ref("C-0001", &blob_fixture("harbour.png"), &[]));
    let done = scene.propose(Lfs::On, &rf, "RULE-HARBOUR-TIDES", &[]);
    assert_eq!(done.code, 0, "{}", done.show());
    assert!(
        done.stderr.contains(
            "`RULE-HARBOUR-TIDES` is a section of `LOC-HARBOUR`: the file goes beside the document"
        ),
        "{}",
        done.show()
    );
    let id = id_of(&done.json());
    assert_eq!(scene.column(&id, "target_id"), "LOC-HARBOUR");
    assert_eq!(scene.column(&id, "target_path"), HARBOUR_MD);
    assert_eq!(scene.column(&id, "new_text"), "media/harbour.png");
}

/// PR15: "what matters" of 513 characters, given or the reference's own: exit
/// 1 with the count; 512 pass; line breaks become one space.
#[test]
fn pr15_what_matters_is_one_line_of_512() {
    let scene = Scene::new("pr-pr15-matters");
    scene.card("The bell", &["LOC-HARBOUR"]);
    let rf = id_of(&scene.file_ref("C-0001", &blob_fixture("harbour.png"), &[]));
    let before = scene.kept();
    let long = "m".repeat(513);
    let done = scene.propose(Lfs::On, &rf, "LOC-HARBOUR", &["--matters", &long]);
    assert_eq!(done.code, 1, "{}", done.show());
    assert!(
        done.stderr.contains(&format!(
            "{rf}'s what matters is 513 characters: an entry holds at most 512; give one with --matters"
        )),
        "{}",
        done.show()
    );
    assert_eq!(scene.kept(), before);

    // The reference's own text of 600 characters, no --matters.
    let wordy = "w".repeat(600);
    let other = png(24, 24, 0x41, None);
    let file = scene.scratch_file("gulls.png", &other);
    let rf2 = id_of(&scene.file_ref("C-0001", &file, &["--matters", &wordy]));
    let before = scene.kept();
    let done = scene.propose(Lfs::On, &rf2, "LOC-HARBOUR", &[]);
    assert_eq!(done.code, 1, "{}", done.show());
    assert!(
        done.stderr.contains(&format!(
            "{rf2}'s what matters is 600 characters: an entry holds at most 512; give one with --matters"
        )),
        "{}",
        done.show()
    );
    assert_eq!(scene.kept(), before);

    // 512 characters with line breaks in runs: one space each.
    let given = format!(
        "{}\n\n{}\r\n{}",
        "a".repeat(200),
        "b".repeat(200),
        "c".repeat(110)
    );
    let id = scene.proposed(Lfs::On, &rf2, "LOC-HARBOUR", &["--matters", &given]);
    assert_eq!(
        scene.column(&id, "new_text"),
        format!(
            "media/gulls.png | {} {} {}",
            "a".repeat(200),
            "b".repeat(200),
            "c".repeat(110)
        )
    );
}

/// PR15: `[ids]` claiming `RF` makes `spec propose reference` exit 2, as
/// every reference command; claiming `PR` too.
#[test]
fn pr15_ids_claiming_rf_or_pr_cannot_run() {
    let scene = Scene::new("pr-pr15-ids");
    scene.card("Light the harbour", &["LOC-HARBOUR"]);
    let rf = id_of(&scene.file_ref("C-0001", &blob_fixture("harbour.png"), &[]));
    let config = scene.text("specengine.toml");
    for (line, said) in [
        (
            "RF = { kind = \"reef\", width = 4 }\n",
            "[ids] claims RF, which names references here: rename that prefix",
        ),
        ("PR = { kind = \"pier\", width = 4 }\n", "PR"),
    ] {
        fs::write(
            scene.pair.main.join("specengine.toml"),
            with_ids_line(&config, line),
        )
        .expect("the config");
        let before = scene.kept();
        let done = scene.propose(Lfs::On, &rf, "LOC-HARBOUR", &[]);
        assert_eq!(done.code, 2, "{line}: {}", done.show());
        assert!(done.stderr.contains(said), "{said}: {}", done.show());
        assert_eq!(done.stdout, "");
        assert_eq!(scene.kept(), before);
    }
}

// ------------------------------------------------------------------- PR7

/// The fixture's config with `[team]` cut to `ann` alone, a `docs` zone and
/// `[auto] mode = "both"` covering it: the sole owner, every path covered.
fn auto_config(config: &str) -> String {
    let team = config.find("[team.lev]").expect("lev's entry");
    let mut text = config[..team].to_owned();
    text = text.replace(
        "[zones]\n",
        "[zones]\ndocs       = [\"docs/**\"]\neverything = [\"**\"]\n",
    );
    text.push_str("[auto]\nmode  = \"both\"\nzones = [\"docs\", \"everything\"]\n");
    text
}

/// PR7, the terminal: a pack's task on LOC-HARBOUR; an update of it bound
/// to the task applies by rule, its promotion bound to the same task is held
/// `proposal-kind`. M: `reference` added to the covered kinds.
#[test]
fn pr7_auto_holds_a_promotion_for_a_person() {
    let scene = Scene::new("pr-pr7");
    let config = auto_config(&scene.text("specengine.toml"));
    scene.commit_file("specengine.toml", &config, "auto: both");
    let task = id_of(&scene.doc(&[
        "task",
        "new",
        "--title",
        "Light the harbour",
        "--nodes",
        "LOC-HARBOUR",
    ]));
    // The pack, recorded as `spec auto start` records it.
    let head_config = scene.git(Lfs::Off, &["show", "HEAD:specengine.toml"]);
    let rule = AutoRule::from_toml(head_config.as_bytes(), "specengine.toml")
        .rule
        .expect("a rule at HEAD");
    scene.git(Lfs::Off, &["branch", &rule.branch, "HEAD"]);
    let repository = fs::canonicalize(scene.pair.main.join(".git"))
        .expect("the git dir")
        .to_str()
        .expect("UTF-8")
        .to_owned();
    let started = AutoStarted {
        repository,
        tasks: vec![task.clone()],
        lines: Vec::new(),
        until: None,
        branch: rule.branch.clone(),
        branch_at: scene.head(),
        rule: rule.hash.clone(),
        frozen: rule.canonical_form(),
        by: "Scratch Committer <committer@example.invalid>".to_owned(),
    };
    SqliteQueue::open(scene.pair.db(), SLUG)
        .expect("the queue")
        .record_auto(&AutoEvent::Started(started), "2026-11-23T08:00:00Z")
        .expect("the pack recorded");

    scene.card("Light the harbour", &["LOC-HARBOUR"]);
    let rf = id_of(&scene.file_ref("C-0001", &blob_fixture("harbour.png"), &[]));
    let promotion = scene.proposed(Lfs::On, &rf, "LOC-HARBOUR", &["--task", &task]);
    let (base, text) = scene.pair.span(&scene.pair.main, "LOC-HARBOUR");
    let edited = text.replacen("before dawn.", "before first light.", 1);
    assert_ne!(edited, text, "premise");
    let update = scene.piped(
        Lfs::On,
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
            "--task",
            &task,
        ],
        edited.as_bytes(),
    );
    update.code(0);
    let update = id_of(&update.json());

    let show = scene.json_run(Lfs::On, &["auto", "show"]);
    show.code(0);
    let show = show.json();
    let verdict = |id: &str| -> (String, String, String) {
        let row: &Value = show["proposals"]
            .as_array()
            .expect("proposals")
            .iter()
            .find(|row| row["id"] == json!(id))
            .unwrap_or_else(|| panic!("no row {id}: {show}"));
        (
            row["verdict"].as_str().unwrap_or_default().to_owned(),
            row["reason"].as_str().unwrap_or_default().to_owned(),
            row["detail"].as_str().unwrap_or_default().to_owned(),
        )
    };
    assert_eq!(
        verdict(&update),
        ("apply".to_owned(), String::new(), String::new()),
        "premise: the rule covers the document: {show}"
    );
    assert_eq!(
        verdict(&promotion),
        (
            "hold".to_owned(),
            "proposal-kind".to_owned(),
            "a reference waits for a person".to_owned()
        ),
        "M: `reference` among the covered kinds"
    );
    let text = scene.spec(Lfs::On, &["auto", "show"]);
    text.code(0);
    assert!(
        text.stdout.lines().any(|line| line
            == format!(
                "  {promotion} | reference | {task} | hold | proposal-kind: a reference waits for a person"
            )),
        "{}",
        text.stdout
    );
    // No commit of the rule's, nothing written.
    let log = scene.git(Lfs::Off, &["log", "--all", "--format=%B"]);
    assert!(!log.contains("Delegated-by:"), "{log}");
    assert_eq!(scene.status(), "");
    assert!(scene.file("docs/spec/world/media/harbour.png").is_none());
}
