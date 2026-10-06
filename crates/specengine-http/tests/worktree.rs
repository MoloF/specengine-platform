//! AC-05 of docs/features/daemon-read.md: the worktree. The root is on
//! branch `x` with an uncommitted node edit; a proposal is raised in a
//! second worktree on `y`, whose committed text differs. `inbox` lists
//! it; `proposals/:id` has `y`'s `base_text`, its preview, that
//! `worktree`, `branch` `y`; `nodes` the root's edited text (the files on
//! disk, never `HEAD`); `/api/projects` the root and `x`. M: preview
//! against the root; `HEAD` read.

mod common;

use common::{Scratch, Server, propose_update, replace, spec_json};
use serde_json::json;

#[test]
fn ac05_reads_follow_the_root_on_disk_and_a_proposal_its_own_worktree() {
    let scratch = Scratch::new("worktree");
    let root = scratch.repo("spec-a", "root", "x");
    let git = scratch.git();
    let y = scratch.join("wt-y");
    git.run(
        &root,
        &["worktree", "add", "-q", "-b", "y", y.to_str().unwrap()],
    );
    let y = std::fs::canonicalize(&y).unwrap();
    // y: a committed edit of EDGE-STAM-ZERO.
    replace(
        &y,
        "docs/spec/movement/stamina.md",
        "`Exhausted` is applied immediately.",
        "`Exhausted` is applied after 0.1 s.",
    );
    git.run(&y, &["commit", "-q", "-am", "y: exhausted after 0.1 s"]);
    // The root: an uncommitted edit of RULE-STAM-REGEN.
    replace(
        &root,
        "docs/spec/movement/stamina.md",
        "Base rate 10 units/s",
        "Base rate 12 units/s",
    );
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");

    let id = propose_update(
        &home,
        &cwd,
        &y,
        "EDGE-STAM-ZERO",
        "## Depletion {#EDGE-STAM-ZERO}\n- Stamina reaches 0 \u{2192} `Exhausted` is applied after 0.2 s.\n",
    );
    let in_y = spec_json(&home, &cwd, &y, &["review", &id]).json();
    let y_text = in_y["base_text"].as_str().expect("base_text").to_owned();
    assert!(
        y_text.contains("after 0.1 s"),
        "the base is y's text: {y_text}"
    );

    let server = Server::serve(&home, &cwd, &[&root]);

    // /api/projects: the root, on x.
    let projects = server.get("/api/projects").status(200).json();
    assert_eq!(projects[0]["root"], json!(root.to_str().unwrap()));
    assert_eq!(projects[0]["branch"], json!("x"));

    // inbox: the root's repository, every worktree.
    let inbox = server
        .get("/api/projects/lantern-keep/inbox")
        .status(200)
        .json();
    let entry = inbox["proposals"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["id"] == json!(id))
        .unwrap_or_else(|| panic!("the inbox lists {id}: {inbox}"))
        .clone();
    assert_eq!(entry["branch"], json!("y"), "{entry}");
    let cli_inbox = spec_json(&home, &cwd, &root, &["inbox"]);
    assert_eq!(
        server.get("/api/projects/lantern-keep/inbox").text(),
        cli_inbox.document()
    );

    // proposals/:id: y's base text, preview, worktree and branch.
    let reply = server.get(&format!("/api/projects/lantern-keep/proposals/{id}"));
    reply.status(200);
    let review = reply.json();
    assert_eq!(review["base_text"], json!(y_text), "{review}");
    assert_eq!(review["worktree"], json!(y.to_str().unwrap()), "{review}");
    assert_eq!(review["branch"], json!("y"), "{review}");
    assert_eq!(
        review["preview"], in_y["preview"],
        "the preview in y's worktree, as `spec review` there gives it"
    );
    assert_eq!(
        review["preview"],
        json!("applies"),
        "it applies in y: {review}"
    );
    assert!(review["conflict"].is_null(), "no conflict in y: {review}");
    assert_eq!(
        reply.text(),
        spec_json(&home, &cwd, &root, &["review", &id]).document(),
        "the review from the root, byte for byte"
    );

    // nodes: the root's text on disk, uncommitted edit included.
    let node = server
        .get("/api/projects/lantern-keep/nodes/RULE-STAM-REGEN")
        .status(200)
        .json();
    let text = node["nodes"][0]["text"].as_str().expect("text");
    assert!(
        text.contains("Base rate 12 units/s"),
        "the root's edit: {text}"
    );
    let edge = server
        .get("/api/projects/lantern-keep/nodes/EDGE-STAM-ZERO")
        .status(200)
        .json();
    assert!(
        edge["nodes"][0]["text"]
            .as_str()
            .unwrap()
            .contains("applied immediately"),
        "the root's own text, not y's: {edge}"
    );
    // search and tree read the same files.
    let hits = server
        .get("/api/projects/lantern-keep/search?query=units")
        .status(200)
        .json();
    let regen = hits["hits"]
        .as_array()
        .unwrap()
        .iter()
        .find(|hit| hit["id"] == json!("RULE-STAM-REGEN"))
        .expect("RULE-STAM-REGEN is a hit")
        .clone();
    let shown: String = regen["snippet"]["segments"]
        .as_array()
        .unwrap()
        .iter()
        .map(|segment| segment["text"].as_str().unwrap())
        .collect();
    assert!(
        shown.contains("12"),
        "the snippet of the edited text: {regen}"
    );
}
