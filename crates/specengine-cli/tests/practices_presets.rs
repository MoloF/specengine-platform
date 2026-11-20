//! docs/features/department-practices.md (RM-0128) "Acceptance criteria",
//! the examples and `[auto]`:
//!
//! - DP18: `scripts/presets/practices/gamedev/` holds the four English
//!   examples, each `class: canon`, its `disciplines` one `gamedev.toml`
//!   flow discipline, each rule a `{#PRA-…}` section, `art-3d-models.md`
//!   covering realism, normal maps, level of detail, polygon and texture
//!   budgets and optimisation, each opening with its header (copy or
//!   propose; declare `PRA`; examples to replace); `gamedev.toml` holds the
//!   three commented lines, its zone covering `docs/spec/practices/`
//!   (where practices live, under the default walk: the orchestrator's
//!   ruling of 2026-11-20); copied there into a scratch copy of studio-lamp
//!   with those lines enabled, `spec check` gives no error and no
//!   `practice-discipline`, every rule
//!   shows; `spec init --pack` still writes one file; this repository's
//!   config excludes them and its `spec` does not read them. The guards
//!   reading them (LG-8: a Cyrillic line turns it red) are
//!   `specengine-eval`'s `anonymity.rs` and `workspace_names.rs`.
//! - DP19: under the preset's `[auto]` line enabled (with the `mode` and
//!   `zones` a rule needs, `docs` covering `docs/**`), `spec auto show`
//!   leaves a covered `update` of the scene's art practice bound to a live
//!   pack's task to a person: `hold`, `excluded-zone`, naming the file and
//!   `practices`. M: the zone removed lets it be delegated (the twin
//!   without `exclude_zones` reads `apply`).
//!
//! The fixtures and this repository are only read; every write is in a
//! scratch directory with a `HOME` of its own.

#![cfg(unix)]

mod common;
mod practices_scene;

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

#[allow(unused_imports)]
use common::git;
use common::{Run, repository_root, snapshot, spec_with};
use practices_scene::{ART, Scene, with_ids_line};
use serde_json::Value;
use specengine_core::auto_toml::AutoRule;
use specengine_store::{AutoEvent, AutoStarted, SqliteQueue};

/// The four examples and the discipline each names.
const EXAMPLES: [(&str, &str); 4] = [
    ("art-3d-models.md", "art"),
    ("development.md", "development"),
    ("sound.md", "sound"),
    ("production.md", "production"),
];
const EXAMPLES_DIR: &str = "scripts/presets/practices/gamedev";

fn example(name: &str) -> String {
    let path = repository_root().join(EXAMPLES_DIR).join(name);
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// The front-matter of `text` (between the two `---` lines).
fn front_matter(text: &str) -> &str {
    let rest = text
        .strip_prefix("---\n")
        .unwrap_or_else(|| panic!("front-matter first: {text}"));
    let end = rest.find("\n---\n").expect("a closing ---");
    &rest[..end]
}

/// The `{#ID}` of each heading of `text`, and the headings that carry none.
fn rules(text: &str) -> (Vec<String>, Vec<String>) {
    let mut ids = Vec::new();
    let mut bare = Vec::new();
    let mut fenced = false;
    for line in text.lines() {
        if line.starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if fenced || !line.starts_with("##") {
            continue;
        }
        match line
            .rfind("{#")
            .and_then(|at| line[at + 2..].strip_suffix('}'))
        {
            Some(id) => ids.push(id.to_owned()),
            None => bare.push(line.to_owned()),
        }
    }
    (ids, bare)
}

/// Every `disciplines` entry of `gamedev.toml`'s flows.
fn flow_disciplines() -> BTreeSet<String> {
    let text = fs::read_to_string(repository_root().join("scripts/presets/gamedev.toml"))
        .expect("the preset");
    let table: toml::Table = text.parse().expect("the preset is TOML");
    table["card"]["flow"]
        .as_array()
        .expect("flows")
        .iter()
        .flat_map(|flow| {
            flow["disciplines"]
                .as_array()
                .expect("disciplines")
                .iter()
                .map(|value| value.as_str().expect("a name").to_owned())
                .collect::<Vec<_>>()
        })
        .collect()
}

/// The preset's three commented lines enabled: `(ids key, zones key, auto
/// key)`, each the text after `# [<table>]`.
fn preset_lines() -> (String, String, String) {
    let text = fs::read_to_string(repository_root().join("scripts/presets/gamedev.toml"))
        .expect("the preset");
    let find = |table: &str| -> String {
        let found: Vec<String> = text
            .lines()
            .filter_map(|line| line.strip_prefix(&format!("# {table}")))
            .filter(|rest| rest.starts_with(' '))
            .map(|rest| rest.trim().to_owned())
            .collect();
        assert_eq!(found.len(), 1, "one `# {table} …` line: {found:?}");
        found[0].clone()
    };
    let ids = find("[ids]");
    let zones = find("[zones]");
    let auto = find("[auto]");
    assert!(ids.starts_with("PRA = "), "{ids}");
    assert!(
        ids.contains("kind = \"practice\"") && ids.contains("shape = \"name\""),
        "{ids}"
    );
    assert!(zones.starts_with("practices = "), "{zones}");
    let glob = zones
        .split('"')
        .nth(1)
        .unwrap_or_else(|| panic!("a glob: {zones}"));
    let prefix = glob
        .strip_suffix("/**")
        .unwrap_or_else(|| panic!("a directory's glob: {glob}"));
    assert!(
        ART.starts_with(&format!("{prefix}/")),
        "the zone `{glob}` covers where practices live (`{ART}`)"
    );
    assert!(auto.starts_with("exclude_zones = "), "{auto}");
    assert!(auto.contains("\"practices\""), "{auto}");
    (ids, zones, auto)
}

/// Where an example is copied in a project: under the default walk, in its
/// discipline's directory.
fn copied(name: &str, discipline: &str) -> String {
    format!("{}/{discipline}/{name}", practices_scene::PRACTICES_DIR)
}

fn spec_at(scene: &Scene, cwd: &Path, home: &Path, args: &[&str]) -> Run {
    let vars = scene.vars_with(home);
    let env: Vec<(&str, &OsStr)> = vars
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_os_str()))
        .collect();
    spec_with(cwd, args, &env)
}

fn spec(scene: &Scene, args: &[&str]) -> Run {
    spec_at(scene, &scene.root, &scene.home, args)
}

// ----------------------------------------------------------------- DP18

/// DP18: the four files, English, `class: canon`, one flow discipline
/// each, every rule a `PRA` section (at least two per file), the header,
/// and art's five topics.
#[test]
fn dp18_the_four_examples() {
    let flows = flow_disciplines();
    for (name, discipline) in EXAMPLES {
        let text = example(name);
        assert!(
            !text.chars().any(|c| ('\u{0400}'..='\u{04FF}').contains(&c)),
            "{name}: English only (no Cyrillic)"
        );
        let front = front_matter(&text);
        assert!(
            front.lines().any(|line| line == "class: canon"),
            "{name}: {front}"
        );
        assert!(
            front
                .lines()
                .any(|line| line == format!("disciplines: [{discipline}]")),
            "{name}: disciplines [{discipline}]: {front}"
        );
        assert!(
            flows.contains(discipline),
            "{discipline} is a gamedev flow's"
        );
        let (ids, bare) = rules(&text);
        assert!(ids.len() >= 2, "{name}: rules {ids:?}");
        assert_eq!(
            bare,
            Vec::<String>::new(),
            "{name}: every rule an ID section"
        );
        for id in &ids {
            assert!(id.starts_with("PRA-"), "{name}: {id}");
            assert!(
                id.bytes()
                    .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'-'),
                "{name}: {id} is Latin, the name shape"
            );
        }
        let head = &text[..text.find("\n## ").unwrap_or(text.len())].to_lowercase();
        for word in ["copy", "propose", "pra", "[ids]", "example"] {
            assert!(
                head.contains(word),
                "{name}: the header says `{word}`:\n{head}"
            );
        }
    }
    let art = example("art-3d-models.md");
    let headings: String = art
        .lines()
        .filter(|line| line.starts_with("##"))
        .collect::<Vec<_>>()
        .join("\n")
        .to_lowercase();
    for topic in [
        "realism",
        "normal map",
        "level of detail",
        "polygon",
        "texture",
        "optimi",
    ] {
        assert!(headings.contains(topic), "art covers {topic}: {headings}");
    }
    // The IDs of the four are distinct.
    let mut all = BTreeSet::new();
    for (name, _) in EXAMPLES {
        for id in rules(&example(name)).0 {
            assert!(all.insert(id.clone()), "{id} twice");
        }
    }
    preset_lines();
}

/// A scratch copy of studio-lamp with the preset's lines enabled, the four
/// examples under `docs/spec/practices/<discipline>/`, committed (the
/// scene's own practices removed).
fn lamp_with_examples(label: &str) -> Scene {
    let scene = Scene::files(label, false);
    let (ids, zones, auto) = preset_lines();
    let fixture =
        fs::read_to_string(practices_scene::fixture("studio-lamp").join("specengine.toml"))
            .expect("studio-lamp's config");
    let config = format!(
        "{}\n[zones]\n{zones}\n\n[auto]\n{auto}\n",
        with_ids_line(&fixture, &format!("{ids}\n"))
    );
    scene.put("specengine.toml", &config);
    for file in practices_scene::PRACTICE_FILES {
        scene.remove(file);
    }
    for (name, discipline) in EXAMPLES {
        scene.put(&copied(name, discipline), &example(name));
    }
    scene.commit("The examples");
    scene
}

/// DP18: in a copy of studio-lamp, the examples check with no error and no
/// `practice-discipline`, and every rule is a walked node.
#[test]
fn dp18_the_examples_check_clean_in_a_game_project() {
    let scene = lamp_with_examples("dp18-lamp");
    let run = spec(&scene, &["check", "--json"]);
    assert!(run.code == 0 || run.code == 1, "{}", run.show());
    let report = run.json();
    let findings = report["findings"].as_array().expect("findings");
    let errors: Vec<&Value> = findings
        .iter()
        .filter(|finding| finding["severity"] == "error")
        .collect();
    assert_eq!(errors, Vec::<&Value>::new(), "{}", run.show());
    let warned: Vec<&Value> = findings
        .iter()
        .filter(|finding| finding["code"] == "practice-discipline")
        .collect();
    assert_eq!(warned, Vec::<&Value>::new(), "{}", run.show());
    for (name, discipline) in EXAMPLES {
        for id in rules(&example(name)).0 {
            let shown = spec(&scene, &["--json", "show", &id]);
            shown.code(0);
            assert_eq!(
                shown.json()["nodes"][0]["path"],
                copied(name, discipline),
                "{id}"
            );
        }
    }
    // Each discipline's card receives its example.
    let card = scene.card("Model the lamp", "art", &[], None, &[], None);
    let shown = spec(&scene, &["--json", "card", "show", &card, "--practices"]);
    shown.code(0);
    assert_eq!(
        shown.json()["practices"][0]["path"],
        copied("art-3d-models.md", "art")
    );
}

/// DP18: `spec init --pack` of the preset still writes its one file.
#[test]
fn dp18_init_pack_writes_one_file() {
    let scene = Scene::files("dp18-init", false);
    let dir = scene.dir("lamp-test");
    let home = scene.dir("init-home");
    let pack = repository_root().join("scripts/presets/gamedev.toml");
    let run = spec_at(
        &scene,
        &dir,
        &home,
        &["init", "--pack", pack.to_str().expect("UTF-8")],
    );
    run.code(0);
    let entries: Vec<String> = snapshot(&dir).into_keys().collect();
    assert_eq!(entries, ["specengine.toml"], "init writes only its file");
}

/// DP18: this repository's `[paths] exclude` holds the examples' glob, and
/// its `spec` does not read them (a path REF to one names nothing).
#[test]
fn dp18_this_repository_does_not_walk_the_examples() {
    let root = repository_root();
    let config = fs::read_to_string(root.join("specengine.toml")).expect("the config");
    let table: toml::Table = config.parse().expect("TOML");
    let exclude: Vec<&str> = table["paths"]["exclude"]
        .as_array()
        .expect("exclude")
        .iter()
        .filter_map(|value| value.as_str())
        .collect();
    assert!(
        exclude.contains(&"scripts/presets/practices/**"),
        "{exclude:?}"
    );
    let scratch = common::Scratch::new("dp18-repo");
    let home = scratch.home("h");
    let path = format!("{EXAMPLES_DIR}/art-3d-models.md");
    let run = spec_with(
        &root,
        &["--json", "show", &path],
        &[("HOME", home.as_os_str())],
    );
    assert_eq!(
        run.code,
        1,
        "not a document of this repository: {}",
        run.show()
    );
}

// ----------------------------------------------------------------- DP19

/// Now, UTC, `offset` seconds away (as `auto_common::utc`).
fn utc(offset: i64) -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("after the epoch")
        .as_secs() as i64
        + offset;
    let (days, rest) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

/// The scene under a committed `[auto]`: the preset's lines enabled
/// (`excluded`: with `exclude_zones`, else the twin without it), `docs`
/// covering `docs/**`, `mode = "local"`.
fn auto_scene(label: &str, excluded: bool) -> Scene {
    let scene = Scene::files(label, false);
    let (_, zones, auto) = preset_lines();
    let auto_key = if excluded {
        format!("{auto}\n")
    } else {
        String::new()
    };
    scene.append(
        "specengine.toml",
        &format!(
            "\n[zones]\n{zones}\ndocs = [\"docs/**\"]\n\n[auto]\nmode = \"local\"\nzones = \
             [\"docs\"]\n{auto_key}"
        ),
    );
    scene.commit("auto: the preset's line");
    scene
}

/// A pack of `tasks` recorded through the store as `spec auto start`
/// records one: `HEAD`'s rule frozen, the branch `auto` made at `HEAD`, a
/// minute ago.
fn record_pack(scene: &Scene, tasks: &[&str]) {
    spec(scene, &["index"]).code(0);
    let config = scene
        .git
        .git_text(&scene.root, &["show", "HEAD:specengine.toml"]);
    let rule = AutoRule::from_toml(config.as_bytes(), "specengine.toml")
        .rule
        .expect("a rule at HEAD");
    let head = scene.git.git_text(&scene.root, &["rev-parse", "HEAD"]);
    scene
        .git
        .git(&scene.root, &["branch", &rule.branch, "HEAD"]);
    let repository = fs::canonicalize(scene.root.join(".git"))
        .expect("the git dir")
        .to_str()
        .expect("UTF-8")
        .to_owned();
    let started = AutoStarted {
        repository,
        tasks: tasks.iter().map(|task| (*task).to_owned()).collect(),
        lines: Vec::new(),
        until: None,
        branch: rule.branch.clone(),
        branch_at: head,
        rule: rule.hash.clone(),
        frozen: rule.canonical_form(),
        by: "Scratch Committer <committer@example.invalid>".to_owned(),
    };
    let db: PathBuf = common::data_dir(&scene.home).join(format!("{}.db", practices_scene::SLUG));
    SqliteQueue::open(db, practices_scene::SLUG)
        .expect("the queue opens")
        .record_auto(&AutoEvent::Started(started), &utc(-60))
        .expect("the pack is recorded");
}

/// `(verdict, reason, detail)` of `spec auto show --json`'s row `id`.
fn verdict(scene: &Scene, id: &str) -> (String, String, String) {
    let run = spec(scene, &["--json", "auto", "show"]);
    run.code(0);
    let show = run.json();
    let row = show["proposals"]
        .as_array()
        .expect("proposals")
        .iter()
        .find(|row| row["id"] == id)
        .unwrap_or_else(|| panic!("no row {id}: {show}"))
        .clone();
    (
        row["verdict"].as_str().unwrap_or_default().to_owned(),
        row["reason"].as_str().unwrap_or_default().to_owned(),
        row["detail"].as_str().unwrap_or_default().to_owned(),
    )
}

/// A task on `PRA-ART-REALISM`, packed, and an `update` of it bound to the
/// task (written to a file, `--text-file`): the proposal's ID.
fn packed_update(scene: &Scene) -> String {
    let made = spec(
        scene,
        &[
            "--json",
            "task",
            "new",
            "--title",
            "Restyle the boat",
            "--nodes",
            "PRA-ART-REALISM",
        ],
    );
    made.code(0);
    let task = made.json()["id"].as_str().expect("a task").to_owned();
    record_pack(scene, &[&task]);
    let (base, text) = scene.span("PRA-ART-REALISM");
    let file = scene.scratch().join("realism.md");
    fs::write(
        &file,
        practices_scene::once(&text, "Stylised", "Stylised and warm", "realism"),
    )
    .expect("the text file");
    let raised = spec(
        scene,
        &[
            "--json",
            "propose",
            "update",
            "PRA-ART-REALISM",
            "--base",
            &base,
            "--text-file",
            file.to_str().expect("UTF-8"),
            "--rationale",
            "Warmer models.",
            "--task",
            &task,
        ],
    );
    raised.code(0);
    raised.json()["id"].as_str().expect("an ID").to_owned()
}

/// DP19: the preset's `exclude_zones` holds a covered update of art.md for
/// a person; the twin without it delegates the same update. M: the zone
/// removed.
#[test]
fn dp19_auto_keeps_off_practices() {
    let held = auto_scene("dp19-held", true);
    let id = packed_update(&held);
    assert_eq!(
        verdict(&held, &id),
        (
            "hold".to_owned(),
            "excluded-zone".to_owned(),
            format!("`{ART}` is in the excluded zone `practices`"),
        )
    );
    let open = auto_scene("dp19-open", false);
    let id = packed_update(&open);
    assert_eq!(
        verdict(&open, &id).0,
        "apply",
        "M: the zone removed: the same update is delegated"
    );
}
