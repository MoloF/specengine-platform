//! Plumbing of docs/features/department-practices.md (RM-0128) DP16 and
//! DP17, shared by `practices_scale.rs` and `practices_soak.rs`, on top of
//! `library_common` (the release binaries, the hub, the requests, the slow
//! lines, RSS and Theil-Sen):
//!
//! - **The tree**: `specengine-eval scale --corpus-only --nodes N --seed 1`
//!   as `library_common::Tree` builds it, with no seeded history (DP16
//!   names none), plus `PRA = { kind = "practice", shape = "name" }` in
//!   `[ids]` and 20 practices × 30 rules of one discipline (`art`) under
//!   `areas/practices/`, committed;
//! - **a card** of that discipline with 3 targets (three part documents)
//!   and a goal citing three rules: made by the release `spec` in a `HOME`,
//!   or on a hub by `create_card` over `/mcp/<slug>`;
//! - **a hang's sample**: `sample <pid> 5` into
//!   `$TMPDIR/specengine-practices-soak-samples/`, kept after the run.
//!
//! Every name is invented.

#![allow(dead_code)]

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use serde_json::{Value, json};

use crate::library_common::{Hub, Tree, mcp_call, release_bin};

/// The practices of DP16's tree, the rules each, their discipline.
pub const PRACTICES: usize = 20;
pub const RULES: usize = 30;
pub const DISCIPLINE: &str = "art";
/// A discipline no practice names.
pub const NOTHING: &str = "nothing-here";
/// The build DP16 and DP17 read (`--bins`: a `--bin` filter built no
/// daemon and gave stale numbers).
pub const BUILD: &str =
    "cargo build --release -p specengine-cli -p specengine-http -p specengine-mcp --bins";

/// One rule's prose, about 200 characters, its own words.
fn rule_text(practice: usize, rule: usize) -> String {
    format!(
        "Every model of batch {practice} keeps rule {rule}: the silhouette reads at a glance, \
         the budget of piece {rule} holds, and the reviewer of batch {practice} checks it before \
         the model ships.\n"
    )
}

/// DP16's tree at `nodes`.
pub fn practice_tree(label: &str, nodes: usize) -> Tree {
    let tree = Tree::new(&format!("practices-{label}"), nodes, 0);
    let config_path = tree.root.join("specengine.toml");
    let config = fs::read_to_string(&config_path).expect("the config");
    let anchor = "[ids]\n";
    assert_eq!(config.matches(anchor).count(), 1, "{config}");
    fs::write(
        &config_path,
        config.replacen(
            anchor,
            "[ids]\nPRA = { kind = \"practice\", shape = \"name\" }\n",
            1,
        ),
    )
    .expect("the config");
    let dir = tree.root.join("areas/practices");
    fs::create_dir_all(&dir).expect("the practices' directory");
    for practice in 1..=PRACTICES {
        let mut text = format!(
            "---\nid: PRA-ART-{practice:02}\nclass: canon\ntier: 2\nowner: art-lead\n\
             reviewed: 2026-11-20\ndisciplines: [{DISCIPLINE}]\n---\n\n# Art practice \
             {practice:02}\n\nThe agreed rules of batch {practice:02}.\n"
        );
        for rule in 1..=RULES {
            text.push_str(&format!(
                "\n## Rule {practice:02}-{rule:02} {{#PRA-ART-{practice:02}-{rule:02}}}\n\n{}",
                rule_text(practice, rule)
            ));
        }
        fs::write(dir.join(format!("pra-art-{practice:02}.md")), text).expect("a practice");
    }
    tree.git.add_all(&tree.root);
    tree.git.commit(&tree.root, "Twenty practices of art");
    tree
}

/// The three targets of DP16's card.
pub fn targets(tree: &Tree) -> Vec<String> {
    (0..3).map(|at| tree.part_id(at)).collect()
}

/// The card's goal: three rules cited.
pub const GOAL: &str = "Model the hall to PRA-ART-01-01, PRA-ART-07-12 and PRA-ART-20-30.";

/// The release `spec args` in the tree under `home`: its output and ms.
pub fn release_spec(tree: &Tree, home: &Path, args: &[&str]) -> (std::process::Output, u64) {
    let vars: Vec<(OsString, OsString)> = tree.spec_vars(home);
    let started = std::time::Instant::now();
    let output = Command::new(release_bin("spec"))
        .env_clear()
        .envs(vars.iter().map(|(name, value)| (name, value)))
        .current_dir(&tree.root)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .expect("spec runs");
    let ms = u64::try_from(started.elapsed().as_millis()).expect("ms");
    (output, ms)
}

/// A card of `discipline` made by the release `spec` under `home`: its ID.
pub fn cli_card(tree: &Tree, home: &Path, discipline: &str) -> String {
    let mut args = vec![
        "--json",
        "card",
        "new",
        "--title",
        "Model the hall",
        "--goal",
        GOAL,
        "--discipline",
        discipline,
    ];
    let targets = targets(tree);
    for target in &targets {
        args.extend(["--target", target.as_str()]);
    }
    let (output, _) = release_spec(tree, home, &args);
    assert!(
        output.status.success(),
        "card new: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document: Value = serde_json::from_slice(&output.stdout).expect("JSON");
    document["id"].as_str().expect("an ID").to_owned()
}

/// A card of [`DISCIPLINE`] made on `hub` by `login` with `create_card`:
/// its ID.
pub fn hub_card(hub: &Hub, tree: &Tree, login: &str) -> String {
    let (answer, response) = mcp_call(
        hub.port,
        &tree.slug,
        login,
        1,
        "create_card",
        json!({
            "title": "Model the hall",
            "goal": GOAL,
            "discipline": DISCIPLINE,
            "target": targets(tree),
        }),
        Duration::from_secs(120),
    )
    .expect("create_card answers");
    assert_eq!(answer.status, 200, "{}", answer.text());
    let response = response.unwrap_or_else(|| panic!("no response: {}", answer.text()));
    let result = &response["result"];
    assert_ne!(result["isError"], json!(true), "{response}");
    result["structuredContent"]["id"]
        .as_str()
        .unwrap_or_else(|| panic!("a card's ID: {response}"))
        .to_owned()
}

/// `sample <pid> 5` into `$TMPDIR/specengine-practices-soak-samples/`, kept
/// after the run (macOS); elsewhere, nothing.
pub fn sample(hub: &Hub, label: &str) -> Option<PathBuf> {
    if !cfg!(target_os = "macos") || !Path::new("/usr/bin/sample").exists() {
        return None;
    }
    let dir = std::env::temp_dir().join("specengine-practices-soak-samples");
    fs::create_dir_all(&dir).ok()?;
    let at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    let file = dir.join(format!("{label}-{}-{at}.txt", hub.pid));
    let status = Command::new("/usr/bin/sample")
        .arg(hub.pid.to_string())
        .arg("5")
        .arg("-file")
        .arg(&file)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .ok()?;
    status.success().then_some(file)
}
