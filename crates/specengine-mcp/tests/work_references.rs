//! docs/features/work-references.md (RM-0137, ADR-0088) "Acceptance
//! criteria", the MCP half: the default build's stdio server over a scratch
//! git repository of `fixtures/spec-a` (a game's brief), its references
//! written by this build's `spec` binary (the build directory's, next to the
//! server, refused when older than a source its dep-info names), and the
//! hub's constants. Every server has a cleared environment and a scratch
//! `HOME`. The images are built at the sizes the criteria name (valid PNGs).
//!
//! - RF11: `get_reference` on a 1200x800 image of 300 KiB answers its text,
//!   byte-equal to `spec ref get RF 2>&1`, then one image block whose
//!   decoded bytes are the file's and whose `mimeType` its media; on a
//!   4032x3024 image with a 1568x1176 copy, the copy's bytes; without one,
//!   the text alone with `image: none for agents: …`; a link, an audio file
//!   the text alone. M: the original's bytes sent over the limits.
//! - RF15, MCP: `get_card {context: true}` on a card with 5 agent-ready
//!   images: its text `spec card show C --marks --context 2>&1`, then the
//!   newest 3 images in the `context.images:` order; `get_task` the
//!   package's `references` as text and JSON alone (one content block).
//! - RF20, stdio: 38 tools with `list_references`, `get_reference`,
//!   `add_reference` and no reference delete; the three inputs as "MCP"
//!   lists them, no `as`, `login`, `actor`, `b3`, `file`; the reads'
//!   annotations and the intake writers'; `add_reference` stores a link
//!   authored `{type: agent, role, model, run}`, `actor` `agent`; an `as`
//!   refused; `list_references`' content byte-equal to `spec ref list …
//!   2>&1`, its `structuredContent` the `--json` document; `HUB_TOOLS` 24,
//!   `HUB_READ_TOOLS` 10 with the two reads; the hub's instructions name the
//!   three, ASCII within 2 048 bytes. M: `as` added to `add_reference`.
//!
//! The hub over `/mcp` is `specengine-http`'s `tests/work_references.rs`.

#![cfg(all(unix, not(feature = "probes")))]

mod common;

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;
#[path = "../../specengine-cli/tests/work_refs_common/mod.rs"]
mod refs_common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use common::read::{Era, Session};
use common::*;
use git::Sandbox;
use refs_common::{MATERIAL, base64_decode, blob_fixture, png};
use serde_json::{Value, json};
use specengine_mcp::{HUB_INSTRUCTIONS, HUB_READ_TOOLS, HUB_TOOLS};

// ------------------------------------------------------------- the scene

/// The build directory's `spec`, next to the server's binary, refused when
/// older than a file its dep-info (`spec.d`) names: a `#[cfg(test)]` module,
/// which `cargo build --bins` never compiles, never refuses it.
fn spec_bin() -> PathBuf {
    let dir = Path::new(BIN).parent().expect("the server's directory");
    let path = dir.join("spec");
    let built = fs::metadata(&path)
        .and_then(|meta| meta.modified())
        .unwrap_or_else(|error| {
            panic!(
                "{}: {error}; build it first (`cargo build --workspace --bins`)",
                path.display()
            )
        });
    let info = dir.join("spec.d");
    let deps = fs::read_to_string(&info).unwrap_or_else(|error| {
        panic!(
            "{}: {error}; build it first (`cargo build --workspace --bins`)",
            info.display()
        )
    });
    let sources = dep_info_sources(&deps, &path);
    assert!(
        sources
            .iter()
            .any(|source| source.ends_with("specengine-store/src/lib.rs")),
        "{}: no store source named: {sources:?}",
        info.display()
    );
    let newer: Vec<&PathBuf> = sources
        .iter()
        .filter(|source| {
            fs::metadata(source)
                .and_then(|meta| meta.modified())
                .is_ok_and(|at| at > built)
        })
        .collect();
    assert!(
        newer.is_empty(),
        "{} is older than {newer:?}: rebuild it (`cargo build --workspace --bins`)",
        path.display()
    );
    path
}

/// The files the rule for `target` in a dep-info file names.
fn dep_info_sources(deps: &str, target: &Path) -> Vec<PathBuf> {
    let head = format!("{}:", target.display());
    let line = deps
        .lines()
        .find(|line| line.starts_with(&head))
        .unwrap_or_else(|| panic!("no rule for {}", target.display()));
    let mut sources = Vec::new();
    let mut word = String::new();
    let mut chars = line[head.len()..].chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => word.extend(chars.next()),
            ' ' => {
                if !word.is_empty() {
                    sources.push(PathBuf::from(std::mem::take(&mut word)));
                }
            }
            _ => word.push(c),
        }
    }
    if !word.is_empty() {
        sources.push(PathBuf::from(word));
    }
    sources
}

struct Ran {
    code: i32,
    stdout: String,
    stderr: String,
}

/// A scratch git repository of spec-a, committed, its queue in `home`.
struct Repo {
    scratch: Scratch,
    root: PathBuf,
    home: PathBuf,
}

impl Repo {
    fn new(label: &str) -> Self {
        let scratch = Scratch::new(label);
        let git = Sandbox::new(scratch.path());
        let root = scratch.copy("spec-a", "repo");
        git.init(&root);
        git.add_all(&root);
        git.commit(&root, "the fixture");
        let home = scratch.home("h");
        Self {
            scratch,
            root,
            home,
        }
    }

    fn spec(&self, args: &[&str]) -> Ran {
        let mut command = isolated_command(spec_bin(), args, &self.root, Some(&self.home));
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let output = command.output().expect("spec ran");
        Ran {
            code: output.status.code().expect("an exit code"),
            stdout: String::from_utf8(output.stdout).expect("UTF-8"),
            stderr: String::from_utf8(output.stderr).expect("UTF-8"),
        }
    }

    /// `spec --json args`, exit 0: the document.
    fn doc(&self, args: &[&str]) -> Value {
        let mut all = vec!["--json"];
        all.extend_from_slice(args);
        let ran = self.spec(&all);
        assert_eq!(ran.code, 0, "{all:?}: {}{}", ran.stdout, ran.stderr);
        serde_json::from_str(&ran.stdout).expect("one JSON document")
    }

    /// `spec args` as MCP's content: stderr's lines first, then stdout.
    fn text(&self, args: &[&str]) -> String {
        let ran = self.spec(args);
        assert_eq!(ran.code, 0, "{args:?}: {}{}", ran.stdout, ran.stderr);
        format!("{}{}", ran.stderr, ran.stdout)
    }

    /// A file holding `bytes` in the scratch.
    fn file(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let dir = self.scratch.path().join("files");
        fs::create_dir_all(&dir).expect("files");
        let path = dir.join(name);
        fs::write(&path, bytes).expect("write");
        path
    }

    /// `spec ref add --on <on> <file> <extra>`: the reference.
    fn add_file(&self, on: &str, file: &Path, extra: &[&str]) -> Value {
        let mut args = vec!["ref", "add", "--on", on, file.to_str().expect("UTF-8")];
        args.extend_from_slice(extra);
        self.doc(&args)
    }

    fn card(&self, title: &str) -> String {
        id_of(&self.doc(&["card", "new", "--title", title, "--target", "MEC-SPRINT"]))
    }

    fn session(&self) -> Session {
        Session::open(Era::Stateless, &[], Some(&self.root), Home::At(&self.home))
    }
}

fn id_of(document: &Value) -> String {
    document["id"]
        .as_str()
        .unwrap_or_else(|| panic!("an ID: {document}"))
        .to_owned()
}

/// A tool result's content blocks: the text of the first (a text block),
/// then each image block's `(mimeType, decoded bytes)`.
fn blocks(result: &Value) -> (String, Vec<(String, Vec<u8>)>) {
    let content = result["content"].as_array().expect("content");
    assert!(!content.is_empty(), "{result}");
    assert_eq!(
        content[0]["type"],
        json!("text"),
        "the text first: {result}"
    );
    let text = content[0]["text"].as_str().expect("text").to_owned();
    let images = content[1..]
        .iter()
        .map(|block| {
            assert_eq!(
                block["type"],
                json!("image"),
                "only image blocks after the text"
            );
            (
                block["mimeType"].as_str().expect("a mimeType").to_owned(),
                base64_decode(block["data"].as_str().expect("data")),
            )
        })
        .collect();
    (text, images)
}

/// Every property name anywhere in `schema`.
fn property_names(schema: &Value, out: &mut Vec<String>) {
    match schema {
        Value::Object(map) => {
            if let Some(Value::Object(properties)) = map.get("properties") {
                out.extend(properties.keys().cloned());
            }
            for value in map.values() {
                property_names(value, out);
            }
        }
        Value::Array(items) => items.iter().for_each(|item| property_names(item, out)),
        _ => {}
    }
}

// ----------------------------------------------------------------- RF20

/// RF20 on stdio: 38 tools, the three reference tools, no delete; their
/// inputs; no person, hash or file field; the reads' and intake writers'
/// annotations; the descriptions call references people's material; the
/// hub's lists and instructions. M: `as` added to `add_reference`.
#[test]
fn rf20_the_stdio_tools_and_their_schemas() {
    let repo = Repo::new("mcp-rf20-tools");
    let mut session = repo.session();
    let list = session.tools();
    let names = tool_names(&list);
    assert_eq!(names.len(), 38, "{names:?}");
    let mut references: Vec<&String> = names
        .iter()
        .filter(|name| name.contains("reference"))
        .collect();
    references.sort();
    assert_eq!(
        references,
        ["add_reference", "get_reference", "list_references"]
    );
    let inputs = |name: &str| -> Vec<String> {
        tool(&list, name)["inputSchema"]["properties"]
            .as_object()
            .unwrap_or_else(|| panic!("{name}: no properties"))
            .keys()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect()
    };
    assert_eq!(inputs("list_references"), ["before", "limit", "on"]);
    assert_eq!(inputs("get_reference"), ["reference_id"]);
    assert_eq!(
        inputs("add_reference"),
        [
            "author_model",
            "author_role",
            "matters",
            "on",
            "run",
            "title",
            "url"
        ]
    );
    for name in ["list_references", "get_reference", "add_reference"] {
        let mut found = Vec::new();
        property_names(&tool(&list, name)["inputSchema"], &mut found);
        for forbidden in [
            "as", "login", "actor", "b3", "agent_b3", "file", "name", "bytes",
        ] {
            assert!(
                !found.iter().any(|property| property == forbidden),
                "M: `{forbidden}` in {name}'s schema"
            );
        }
        let description = tool(&list, name)["description"]
            .as_str()
            .expect("a description")
            .to_lowercase();
        assert!(
            description.contains("people's material") && description.contains("instructions"),
            "{name}: {description}"
        );
    }
    for name in ["list_references", "get_reference"] {
        assert_eq!(
            tool(&list, name)["annotations"],
            tool(&list, "list_comments")["annotations"],
            "{name}: the reads'"
        );
    }
    assert_eq!(
        tool(&list, "add_reference")["annotations"],
        tool(&list, "add_comment")["annotations"],
        "the intake writers'"
    );
    assert!(
        !names.iter().any(|name| name.contains("delete")),
        "{names:?}"
    );

    assert_eq!(HUB_TOOLS.len(), 24, "{HUB_TOOLS:?}");
    for name in ["list_references", "get_reference", "add_reference"] {
        assert!(HUB_TOOLS.contains(&name), "{name}");
        assert!(
            HUB_INSTRUCTIONS.contains(name),
            "the hub's text names {name}"
        );
    }
    assert_eq!(HUB_READ_TOOLS.len(), 10, "{HUB_READ_TOOLS:?}");
    for name in ["list_references", "get_reference"] {
        assert!(HUB_READ_TOOLS.contains(&name), "{name}");
    }
    assert!(!HUB_READ_TOOLS.contains(&"add_reference"));
    assert!(HUB_INSTRUCTIONS.is_ascii());
    assert!(
        HUB_INSTRUCTIONS.len() <= 2048,
        "{} B",
        HUB_INSTRUCTIONS.len()
    );
    drop(session.finish());
}

/// RF20 on stdio: `add_reference` stores a link authored by the agent,
/// `actor` `agent`, no `login`; an `as`, a `b3` or a `file` argument is
/// refused, nothing stored; `list_references`' content is `spec ref list
/// --on C 2>&1` byte for byte and its `structuredContent` the `--json`
/// document.
#[test]
fn rf20_add_and_list_over_stdio() {
    let repo = Repo::new("mcp-rf20-add");
    let c1 = repo.card("Write the sprint foreword");
    repo.add_file(
        &c1,
        &blob_fixture("lantern.gif"),
        &["--title", "Lamp", "--as", "mira"],
    );
    let mut session = repo.session();
    let reply = session.call(
        "add_reference",
        json!({"on": c1, "url": "https://example.org/board", "title": "Moodboard",
               "matters": "the palette", "author_role": "designer", "author_model": "opus",
               "run": "night-1"}),
    );
    let made = result(&reply);
    assert_ne!(made["isError"], json!(true), "{made}");
    let reference = &made["structuredContent"];
    assert_eq!(reference["kind"], json!("link"));
    assert_eq!(reference["actor"], json!("agent"));
    assert_eq!(
        reference["author"],
        json!({"type": "agent", "role": "designer", "model": "opus", "run": "night-1"})
    );
    for (label, extra) in [
        ("as", json!({"as": "olga"})),
        ("b3", json!({"b3": "0".repeat(64)})),
        ("file", json!({"file": "/etc/hosts"})),
    ] {
        let mut arguments = json!({"on": c1, "url": "https://example.org/x"});
        arguments
            .as_object_mut()
            .expect("an object")
            .extend(extra.as_object().expect("an object").clone());
        let reply = session.call("add_reference", arguments);
        let refused = reply.get("error").is_some()
            || reply
                .get("result")
                .is_some_and(|result| result["isError"] == json!(true));
        assert!(refused, "M: {label} taken: {reply}");
    }
    let reply = session.call("list_references", json!({"on": c1}));
    let listed = result(&reply);
    let (text, images) = blocks(listed);
    assert!(images.is_empty(), "a list carries no image");
    assert_eq!(text, repo.text(&["ref", "list", "--on", &c1]));
    assert_eq!(
        listed["structuredContent"],
        repo.doc(&["ref", "list", "--on", &c1])
    );
    assert_eq!(
        listed["structuredContent"]["total"],
        json!(2),
        "no refused call stored"
    );
    assert!(text.contains(MATERIAL), "{text}");
    drop(session.finish());
}

// ----------------------------------------------------------------- RF11

/// RF11: `get_reference` returns one image: an agent-ready image's own
/// bytes, a large image's copy, else the text alone with "MCP"'s line; a
/// link and an audio file the text alone; the text byte-equal to `spec ref
/// get RF 2>&1`. M: the original's bytes sent over the limits.
#[test]
fn rf11_get_reference_returns_one_image() {
    let repo = Repo::new("mcp-rf11");
    let c1 = repo.card("Draw the harbour");
    let ready_bytes = png(1200, 800, 0x31, Some(300 * 1024));
    let ready = repo.add_file(
        &c1,
        &repo.file("ready.png", &ready_bytes),
        &["--title", "Ready"],
    );
    let big_bytes = png(4032, 3024, 0x32, None);
    let copy_bytes = png(1568, 1176, 0x32, Some(400 * 1024));
    let copied = repo.add_file(
        &c1,
        &repo.file("big.png", &big_bytes),
        &[
            "--title",
            "Harbour at dusk",
            "--agent-copy",
            repo.file("copy.png", &copy_bytes).to_str().expect("UTF-8"),
        ],
    );
    let bare = repo.add_file(
        &c1,
        &repo.file("bare.png", &png(4032, 3024, 0x33, None)),
        &[],
    );
    let wav = repo.add_file(&c1, &blob_fixture("sound.wav"), &["--matters", "the hum"]);
    let link = repo.doc(&["ref", "add", "--on", &c1, "--url", "https://example.org/a"]);
    let mut session = repo.session();
    let get = |session: &mut Session, id: &str| -> (String, Vec<(String, Vec<u8>)>) {
        let reply = session.call("get_reference", json!({"reference_id": id}));
        let answer = result(&reply);
        assert_ne!(answer["isError"], json!(true), "{answer}");
        assert_eq!(
            answer["structuredContent"],
            repo.doc(&["ref", "get", id]),
            "{id}: the document"
        );
        blocks(answer)
    };

    let (text, images) = get(&mut session, &id_of(&ready));
    assert_eq!(
        text,
        repo.text(&["ref", "get", &id_of(&ready)]),
        "the text block's parity"
    );
    assert_eq!(images.len(), 1, "one image block");
    assert_eq!(images[0].0, "image/png");
    assert_eq!(images[0].1, ready_bytes, "the file's bytes");

    let (text, images) = get(&mut session, &id_of(&copied));
    assert_eq!(text, repo.text(&["ref", "get", &id_of(&copied)]));
    assert_eq!(images.len(), 1, "one image block");
    assert_eq!(images[0].0, "image/png", "the copy's media");
    assert_eq!(
        images[0].1, copy_bytes,
        "M: the original's bytes sent over the limits"
    );

    let (text, images) = get(&mut session, &id_of(&bare));
    assert!(images.is_empty(), "no image over the limits without a copy");
    assert!(
        text.contains(
            "  image: none for agents: 4032x3024 is over 1 568 px and no smaller copy was made"
        ),
        "{text}"
    );
    for other in [&wav, &link] {
        let (text, images) = get(&mut session, &id_of(other));
        assert!(images.is_empty(), "{}: no image", id_of(other));
        assert_eq!(text, repo.text(&["ref", "get", &id_of(other)]));
    }
    drop(session.finish());
}

// ----------------------------------------------------------------- RF15

/// RF15 over stdio: `get_card {context: true}` on a card with 5 agent-ready
/// images and a link: the text `spec card show C --marks --context 2>&1`,
/// then the newest 3 images in the `context.images:` line's order, each the
/// file's bytes; the `structuredContent` the `--json` document. `get_task`
/// on a task the card lists: one text block, `references` as `spec task
/// show --json` has them, `schema_version` 1.
#[test]
fn rf15_get_card_attaches_the_newest_three_and_get_task_none() {
    let repo = Repo::new("mcp-rf15");
    let task = id_of(&repo.doc(&[
        "task",
        "new",
        "--nodes",
        "MEC-SPRINT",
        "--title",
        "Tune the sprint",
    ]));
    let c1 = repo.card("Write the sprint foreword");
    repo.doc(&["card", "edit", &c1, "--task", &task]);
    let mut made = Vec::new();
    for n in 0..5u8 {
        let bytes = png(320 + u32::from(n) * 10, 240, 0x40 + n, None);
        let reference = repo.add_file(
            &c1,
            &repo.file(&format!("image-{n}.png"), &bytes),
            &["--title", &format!("Image {n}"), "--as", "mira"],
        );
        made.push((id_of(&reference), bytes));
    }
    repo.doc(&[
        "ref",
        "add",
        "--on",
        &c1,
        "--url",
        "https://example.org/z",
        "--title",
        "Board",
    ]);
    let mut session = repo.session();
    let reply = session.call("get_card", json!({"card_id": c1, "context": true}));
    let card = result(&reply);
    assert_ne!(card["isError"], json!(true), "{card}");
    let (text, images) = blocks(card);
    assert_eq!(
        text,
        repo.text(&["card", "show", &c1, "--marks", "--context"])
    );
    assert_eq!(
        card["structuredContent"],
        repo.doc(&["card", "show", &c1, "--marks", "--context"])
    );
    let line = text
        .lines()
        .find(|line| line.starts_with("context.images: "))
        .unwrap_or_else(|| panic!("an images line: {text}"));
    let named: Vec<String> = line["context.images: ".len()..]
        .split_once("; ")
        .expect("its tail")
        .0
        .split([',', ' '])
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
        .collect();
    let newest: Vec<String> = made
        .iter()
        .rev()
        .take(3)
        .map(|(id, _)| id.clone())
        .collect();
    assert_eq!(named, newest, "the newest three, newest first");
    assert_eq!(
        card["structuredContent"]["context"]["references"]["images"],
        json!(newest)
    );
    assert_eq!(images.len(), 3, "at most 3 (CONTEXT_IMAGES)");
    for ((mime, bytes), (_, want)) in images.iter().zip(made.iter().rev()) {
        assert_eq!(mime, "image/png");
        assert_eq!(bytes, want, "in the line's order");
    }

    let reply = session.call("get_task", json!({"task_id": task}));
    let package = result(&reply);
    assert_ne!(package["isError"], json!(true), "{package}");
    assert_eq!(
        package["content"].as_array().map(Vec::len),
        Some(1),
        "get_task carries no image"
    );
    let document = &package["structuredContent"];
    assert_eq!(document["schema_version"], json!(1));
    let shown = repo.doc(&["task", "show", &task]);
    assert_eq!(document["references"], shown["references"]);
    assert_eq!(document["references"]["total"], json!(6));
    drop(session.finish());
}
