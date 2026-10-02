//! AC-10 of docs/features/mcp-read.md: both eras advertise `resources`;
//! `resources/list` holds `spec://<slug>/tree`, then every live document
//! (the CLI library's `documents`: no Tier 3 path) by path, 200 per page,
//! `nextCursor` = the page's last path; the template
//! `spec://{project}/node/{id}` is listed; a read answers the text of the
//! same tool call (`get_tree {}`, `get_node {id}` with `{id}` percent-decoded
//! once, a raw `#` kept); a foreign slug, a REF naming nothing or neither
//! form is "resource not found" (-32002 in the legacy era; rmcp sends it as
//! -32602 to 2026-07-28 peers, deviation 2, told apart by `data.uri`); a bad
//! percent sequence or non-UTF-8 is -32602 (no `data.uri`); a look-alike ID
//! (the CLI's exit 2) is -32603; stateless list, template list and read
//! carry `ttlMs: 0`, `cacheScope: "public"`.
//!
//! AC-10's `lantern-keep%2FMEC-STAMINA` names a feature slug (deviation
//! 10): the read of it follows `get_node` (exit 1 → not found); the valid
//! scoped form `stamina-tuning%2FAC-07` is read instead.
//!
//! A Tier 3 or `class: generated` document is not listed; a list that
//! cannot run for another reason than no project or `HOME` is -32603.
//! "No project" is the CLI's `locate` failing (an unusable working
//! directory or `--root`, no `specengine.toml`): `[]`, as is an unusable
//! `HOME`; any failure once a project is found (its config unreadable, not
//! UTF-8 or invalid, no slug, the database) is -32603 with the CLI's line.
//!
//! M: `{id}` undecoded; a foreign slug served; Tier 3 or generated listed;
//! `resources/list` swallowing a store failure into `[]`; `[]` when the
//! config cannot be read after `locate` found the project.
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use common::read::{ERAS, Era, Session, cli_env, content_text};
use common::*;
use serde_json::{Value, json};
use specengine_cli::{DocumentEntry, Globals, documents, locate};

/// RFC 3986 unreserved characters kept, every other byte `%XX`.
fn encode(text: &str) -> String {
    let mut out = String::new();
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// The resource of one document as Data writes it.
fn document_resource(slug: &str, entry: &DocumentEntry) -> Value {
    let mut resource = json!({
        "uri": format!("spec://{slug}/node/{}", encode(&entry.path)),
        "name": entry.id.clone().unwrap_or_else(|| entry.path.clone()),
        "mimeType": "text/markdown",
    });
    if let Some(title) = &entry.title {
        resource["title"] = json!(title);
    }
    resource
}

fn tree_resource(slug: &str) -> Value {
    json!({"uri": format!("spec://{slug}/tree"), "name": "tree", "mimeType": "text/plain"})
}

fn assert_cache_hints(era: Era, result: &Value, what: &str) {
    match era {
        Era::Stateless => {
            assert_eq!(result["ttlMs"], json!(0), "{what}: ttlMs");
            assert_eq!(result["cacheScope"], json!("public"), "{what}: cacheScope");
        }
        Era::Legacy => {
            assert!(result.get("ttlMs").is_none(), "{what}: legacy ttlMs");
        }
    }
}

/// A read's text, checked: one `contents` entry, the URI echoed, the MIME
/// type.
fn read_text_of(session: &mut Session, uri: &str, mime: &str) -> String {
    let reply = session.read(uri);
    let result = result(&reply).clone();
    assert_cache_hints(session.era, &result, uri);
    let contents = result["contents"].as_array().expect("contents");
    assert_eq!(contents.len(), 1, "{uri}: {result}");
    assert_eq!(contents[0]["uri"], json!(uri), "{uri}");
    assert_eq!(contents[0]["mimeType"], json!(mime), "{uri}");
    contents[0]["text"].as_str().expect("text").to_owned()
}

/// A tool call's text, which must answer.
fn tool_text(session: &mut Session, tool: &str, args: Value) -> String {
    let reply = session.call(tool, args.clone());
    let result = result(&reply);
    assert_eq!(result["isError"], json!(false), "{tool} {args}: {result}");
    content_text(result).to_owned()
}

/// The not-found code of the era, with `data.uri`.
fn assert_not_found(era: Era, reply: &Value, uri: &str) {
    let want = match era {
        Era::Legacy => RESOURCE_NOT_FOUND,
        Era::Stateless => INVALID_PARAMS,
    };
    assert_eq!(error_code(reply), want, "{era:?} {uri}: {reply}");
    assert_eq!(
        reply["error"]["data"]["uri"],
        json!(uri),
        "{era:?} {uri}: data.uri"
    );
}

fn assert_bad_percent(era: Era, reply: &Value, uri: &str) {
    assert_eq!(error_code(reply), INVALID_PARAMS, "{era:?} {uri}: {reply}");
    assert!(
        reply["error"]["data"]["uri"].is_null(),
        "{era:?} {uri}: a bad sequence is not a not-found: {reply}"
    );
}

#[test]
fn ac10_both_eras_advertise_resources_and_the_template() {
    let (mut legacy, init) = Server::legacy(&[], json!({}));
    assert_eq!(init["capabilities"]["resources"], json!({}), "{init}");
    let templates = result(&legacy.request(1, "resources/templates/list", Some(json!({})))).clone();
    assert_eq!(
        templates["resourceTemplates"],
        json!([{"uriTemplate": "spec://{project}/node/{id}", "name": "node", "mimeType": "text/markdown"}])
    );
    drop(legacy);
    let mut session = Session::open(Era::Stateless, &[], None, Home::Fresh);
    let discover = result(&session.request("server/discover", json!({}))).clone();
    assert!(
        discover["capabilities"]["resources"].is_object(),
        "{discover}"
    );
    let templates = result(&session.request("resources/templates/list", json!({}))).clone();
    assert_cache_hints(Era::Stateless, &templates, "templates");
    assert_eq!(
        templates["resourceTemplates"],
        json!([{"uriTemplate": "spec://{project}/node/{id}", "name": "node", "mimeType": "text/markdown"}])
    );
}

#[test]
fn ac10_the_list_is_the_tree_and_every_live_document() {
    let scratch = Scratch::new("res-list");
    let home = scratch.home("h");
    for (fixture, slug) in [("spec-a", "lantern-keep"), ("spec-b", "zerkalo")] {
        let root = scratch.copy(fixture, fixture);
        let env = cli_env(&root, Some(&home));
        let entries = documents(&env, &Globals::default()).expect("documents");
        let mut want = vec![tree_resource(slug)];
        want.extend(entries.iter().map(|entry| document_resource(slug, entry)));
        for era in ERAS {
            let mut session = Session::open(era, &[], Some(&root), Home::At(&home));
            let reply = session.resources(None);
            let page = result(&reply).clone();
            assert_cache_hints(era, &page, "resources/list");
            assert!(page.get("nextCursor").is_none(), "{page}");
            assert_eq!(page["resources"], json!(want), "{era:?} {fixture}");
            let uris: Vec<&str> = page["resources"]
                .as_array()
                .unwrap()
                .iter()
                .map(|resource| resource["uri"].as_str().unwrap())
                .collect();
            if fixture == "spec-a" {
                assert!(
                    uris.contains(&"spec://lantern-keep/node/docs%2Ffeatures%2Fstamina-tuning.md"),
                    "{uris:?}"
                );
                assert!(
                    uris.contains(&"spec://lantern-keep/node/docs%2Fspec%2Fgame.md"),
                    "{uris:?}"
                );
                assert!(
                    !uris.iter().any(|uri| uri.contains("DEC-0007")),
                    "the Tier 3 record is listed: {uris:?}"
                );
                let game = page["resources"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|resource| resource["name"] == "DOM-GAME")
                    .expect("DOM-GAME listed");
                assert_eq!(
                    game,
                    &json!({"uri": "spec://lantern-keep/node/docs%2Fspec%2Fgame.md",
                            "name": "DOM-GAME", "title": "Lantern Keep",
                            "mimeType": "text/markdown"})
                );
            }
            // Every listed resource reads as the tool call of its REF.
            for resource in page["resources"].as_array().unwrap() {
                let uri = resource["uri"].as_str().unwrap();
                if uri.ends_with("/tree") {
                    let text = read_text_of(&mut session, uri, "text/plain");
                    assert_eq!(text, tool_text(&mut session, "get_tree", json!({})));
                } else {
                    let path = uri.rsplit_once("/node/").unwrap().1.replace("%2F", "/");
                    let text = read_text_of(&mut session, uri, "text/markdown");
                    assert_eq!(
                        text,
                        tool_text(&mut session, "get_node", json!({"id": path})),
                        "{uri}"
                    );
                }
            }
        }
    }
}

/// A project of exactly `count` listed documents.
fn many_documents(root: &Path, count: usize) {
    write(root, "specengine.toml", "[project]\nslug = \"paging\"\n");
    for n in 0..count {
        write(
            root,
            &format!("docs/spec/d-{n:03}.md"),
            format!("---\nclass: canon\n---\n\n# Document {n}\n\nText {n}.\n"),
        );
    }
}

#[test]
fn ac10_two_hundred_fifty_documents_page_two_hundred_and_fifty_one() {
    let scratch = Scratch::new("res-page");
    let home = scratch.home("h");
    let root = scratch.dir("paging");
    many_documents(&root, 250);
    let env = cli_env(&root, Some(&home));
    let entries = documents(&env, &Globals::default()).expect("documents");
    assert_eq!(entries.len(), 250);
    for era in ERAS {
        let mut session = Session::open(era, &[], Some(&root), Home::At(&home));
        let pages = session.all_resources();
        assert_eq!(pages.len(), 2, "{era:?}: two pages");
        let first = pages[0]["resources"].as_array().unwrap();
        let second = pages[1]["resources"].as_array().unwrap();
        assert_eq!((first.len(), second.len()), (200, 51), "{era:?}");
        assert_eq!(first[0], tree_resource("paging"));
        assert_eq!(
            pages[0]["nextCursor"],
            json!("docs/spec/d-198.md"),
            "the cursor is the page's last path"
        );
        assert_eq!(
            first[199]["uri"],
            json!("spec://paging/node/docs%2Fspec%2Fd-198.md")
        );
        assert_eq!(
            second[0]["uri"],
            json!("spec://paging/node/docs%2Fspec%2Fd-199.md"),
            "the next page starts after the cursor"
        );
        assert!(pages[1].get("nextCursor").is_none());
        let mut all: Vec<Value> = first.iter().chain(second).cloned().collect();
        all.remove(0);
        let want: Vec<Value> = entries
            .iter()
            .map(|entry| document_resource("paging", entry))
            .collect();
        assert_eq!(all, want, "{era:?}: every document once, by path");
        // A cursor between two paths starts after it.
        let reply = session.resources(Some("docs/spec/d-2"));
        let page = result(&reply);
        assert_eq!(
            page["resources"][0]["uri"],
            json!("spec://paging/node/docs%2Fspec%2Fd-200.md"),
            "{era:?}"
        );
    }
}

#[test]
fn ac10_reads_decode_the_ref_once_and_answer_the_tool_text() {
    let scratch = Scratch::new("res-read");
    let home = scratch.home("h");
    let a = scratch.copy("spec-a", "a");
    let b = scratch.copy("spec-b", "b");
    for era in ERAS {
        let mut session = Session::open(era, &[], Some(&a), Home::At(&home));
        for (uri, reference) in [
            ("spec://lantern-keep/node/MEC-STAMINA", "MEC-STAMINA"),
            ("spec://lantern-keep/node/R-12", "R-12"),
            ("spec://lantern-keep/node/TERM-tired", "TERM-tired"),
            ("spec://lantern-keep/node/QST-031", "QST-031"),
            (
                "spec://lantern-keep/node/stamina-tuning%2FAC-07",
                "stamina-tuning/AC-07",
            ),
            (
                "spec://lantern-keep/node/stamina-tuning%2fAC-07",
                "stamina-tuning/AC-07",
            ),
            (
                "spec://lantern-keep/node/MEC-STAMINA%23RULE-STAM-REGEN",
                "MEC-STAMINA#RULE-STAM-REGEN",
            ),
            (
                "spec://lantern-keep/node/MEC-STAMINA#RULE-STAM-REGEN",
                "MEC-STAMINA#RULE-STAM-REGEN",
            ),
            (
                "spec://lantern-keep/node/docs/spec/movement/stamina.md",
                "docs/spec/movement/stamina.md",
            ),
            (
                "spec://lantern-keep/node/docs%2Fspec%2Fmovement%2Fstamina.md",
                "docs/spec/movement/stamina.md",
            ),
        ] {
            let text = read_text_of(&mut session, uri, "text/markdown");
            assert_eq!(
                text,
                tool_text(&mut session, "get_node", json!({"id": reference})),
                "{era:?} {uri}"
            );
        }
        // Decoded once: `%2523` is the REF `MEC-STAMINA%23…`, which names
        // nothing.
        let uri = "spec://lantern-keep/node/MEC-STAMINA%2523RULE-STAM-REGEN";
        assert_not_found(era, &session.read(uri), uri);
        let tree = read_text_of(&mut session, "spec://lantern-keep/tree", "text/plain");
        assert_eq!(tree, tool_text(&mut session, "get_tree", json!({})));

        // Not found: a foreign slug, a REF naming nothing, neither form; the
        // invalid feature-slug REF of the spec's example follows get_node.
        for uri in [
            "spec://zerkalo/tree",
            "spec://zerkalo/node/MEC-STAMINA",
            "spec://lantern-keep/node/MEC-NOPE",
            "spec://lantern-keep/node/lantern-keep%2FMEC-STAMINA",
            "spec://lantern-keep/bogus",
            "spec://lantern-keep/node/",
            "spec://lantern-keep",
            "spec:///tree",
            "file:///etc/passwd",
        ] {
            assert_not_found(era, &session.read(uri), uri);
        }
        let reply = session.call("get_node", json!({"id": "lantern-keep/MEC-STAMINA"}));
        assert_eq!(result(&reply)["isError"], json!(true));
        // A bad percent sequence, non-UTF-8 bytes: -32602.
        for uri in [
            "spec://lantern-keep/node/%ZZ",
            "spec://lantern-keep/node/MEC-STAMINA%2",
            "spec://lantern-keep/node/%FF",
            "spec://lantern-keep/node/%C3%28",
        ] {
            assert_bad_percent(era, &session.read(uri), uri);
        }
        // A look-alike ID: the CLI's exit 2, -32603 with its line.
        let uri = "spec://lantern-keep/node/M%D0%95C-STAMINA";
        let reply = session.read(uri);
        assert_eq!(error_code(&reply), INTERNAL_ERROR, "{era:?}: {reply}");
        assert!(
            reply["error"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("MEC-STAMINA")),
            "{reply}"
        );
        // The session goes on.
        let text = read_text_of(
            &mut session,
            "spec://lantern-keep/node/R-12",
            "text/markdown",
        );
        assert!(text.starts_with("R-12 | "), "{text}");
        let done = session.finish();
        assert!(done.status.success(), "{era:?}: {}", done.stderr);
        assert_eq!(done.stderr, "", "{era:?}");

        // spec-b: the encoded Cyrillic legacy prefix is an alias.
        let mut session = Session::open(era, &[], Some(&b), Home::At(&home));
        for (uri, reference) in [
            (
                "spec://zerkalo/node/%D0%A2%D0%A0%D0%91-001",
                "\u{0422}\u{0420}\u{0411}-001",
            ),
            (
                "spec://zerkalo/node/%d0%a2%d0%a0%d0%91-001",
                "\u{0422}\u{0420}\u{0411}-001",
            ),
            ("spec://zerkalo/node/REQ-001", "REQ-001"),
        ] {
            let text = read_text_of(&mut session, uri, "text/markdown");
            assert_eq!(
                text,
                tool_text(&mut session, "get_node", json!({"id": reference})),
                "{era:?} {uri}"
            );
            assert!(text.starts_with("REQ-001 | "), "{text}");
        }
        let uri = "spec://lantern-keep/node/MEC-STAMINA";
        assert_not_found(era, &session.read(uri), uri);
    }
}

/// The URIs of a `resources/list` page.
fn uris_of(page: &Value) -> Vec<String> {
    page["resources"]
        .as_array()
        .expect("resources")
        .iter()
        .map(|resource| resource["uri"].as_str().expect("uri").to_owned())
        .collect()
}

/// AC-10: neither a Tier 3 nor a `class: generated` document is listed:
/// spec-a's superseded `DEC-0007` and a generated `docs/spec/generated-map.md`
/// written into it, spec-b's superseded `ADR-0002` and generated
/// `GLS-task-branch`; every other document is, by path. The lists are
/// written out here, not taken from the CLI's `documents`, so a generated
/// document listed by either side fails. M: Tier 3 or generated listed.
#[test]
fn ac10_generated_and_tier3_documents_are_not_listed() {
    let scratch = Scratch::new("res-live");
    let home = scratch.home("h");
    let a = scratch.copy("spec-a", "spec-a");
    write(
        &a,
        "docs/spec/generated-map.md",
        "---\nid: DOM-GENMAP\nclass: generated\ngenerator: a map script\n---\n\n# Generated map\n\nGenerated.\n",
    );
    let b = scratch.copy("spec-b", "spec-b");
    let listed_a = [
        "docs/features/stamina-tuning.md",
        "docs/records/A/A-101.md",
        "docs/records/A/A-102.md",
        "docs/records/DEC/DEC-0023.md",
        "docs/records/Q/Q-031.md",
        "docs/records/Q/Q-032.md",
        "docs/records/R/R-12.md",
        "docs/records/TERM/TERM-exhausted.md",
        "docs/spec/game.md",
        "docs/spec/movement/README.md",
        "docs/spec/movement/sprint.md",
        "docs/spec/movement/stamina.md",
    ];
    let listed_b = [
        "docs/features/dry-run.md",
        "docs/records/ADR/ADR-0001.md",
        "docs/records/ASM/ASM-01.md",
        "docs/records/GLS/GLS-worktree.md",
        "docs/records/QN/QN-07.md",
        "docs/records/QN/QN-08.md",
        "docs/records/REQ/REQ-001.md",
        "docs/records/REQ/REQ-002.md",
        "docs/spec/cli.md",
    ];
    for (root, slug, listed, left_out) in [
        (
            &a,
            "lantern-keep",
            &listed_a[..],
            ["docs/records/DEC/DEC-0007.md", "docs/spec/generated-map.md"],
        ),
        (
            &b,
            "zerkalo",
            &listed_b[..],
            [
                "docs/records/ADR/ADR-0002.md",
                "docs/records/GLS/GLS-task-branch.md",
            ],
        ),
    ] {
        for path in left_out {
            assert!(root.join(path).is_file(), "{path} exists");
        }
        let mut want = vec![format!("spec://{slug}/tree")];
        want.extend(
            listed
                .iter()
                .map(|path| format!("spec://{slug}/node/{}", encode(path))),
        );
        for era in ERAS {
            let mut session = Session::open(era, &[], Some(root), Home::At(&home));
            let reply = session.resources(None);
            let uris = uris_of(result(&reply));
            assert_eq!(uris, want, "{era:?} {slug}");
            for path in left_out {
                let uri = format!("spec://{slug}/node/{}", encode(path));
                assert!(!uris.contains(&uri), "{era:?}: {uri} is listed");
            }
            let done = session.finish();
            assert!(done.status.success(), "{era:?}: {}", done.stderr);
        }
    }
}

/// Data ("Resources", CLI `documents`): `resources/list` on an error other
/// than no project or `HOME` is -32603 with the CLI's line: a file where
/// the data directory goes, a project without a slug. The tools answer the
/// same line as an error result, `resources/read` -32603. The same project
/// with a usable `HOME` lists. M: `resources/list` swallowing a store
/// failure into `[]`.
#[test]
fn ac10_a_list_that_cannot_run_is_an_internal_error() {
    let scratch = Scratch::new("res-fail");
    let root = scratch.copy("spec-a", "copy");
    let blocked = scratch.home("blocked");
    let data = data_dir(&blocked);
    std::fs::create_dir_all(data.parent().expect("parent")).expect("mkdir");
    std::fs::write(&data, "a file where the data directory goes\n").expect("write");
    let no_slug = scratch.dir("noslug");
    write(
        &no_slug,
        "specengine.toml",
        "[ids]\nDOM = { kind = \"domain\", shape = \"name\" }\n",
    );
    write(
        &no_slug,
        "docs/spec/x.md",
        "---\nid: DOM-X\nclass: canon\n---\n\n# X\n",
    );
    let fresh = scratch.home("fresh");
    for (cwd, home, needle) in [
        (&root, &blocked, "data directory"),
        (&no_slug, &fresh, "slug"),
    ] {
        for era in ERAS {
            let context = format!("{era:?} {needle}");
            let mut session = Session::open(era, &[], Some(cwd), Home::At(home));
            let reply = session.resources(None);
            assert!(reply.get("error").is_some(), "{context}: {reply}");
            assert_eq!(error_code(&reply), INTERNAL_ERROR, "{context}: {reply}");
            assert!(
                reply["error"]["message"]
                    .as_str()
                    .is_some_and(|message| message.contains(needle)),
                "{context}: {reply}"
            );
            let reply = session.call("get_tree", json!({}));
            let tool = result(&reply);
            assert_eq!(tool["isError"], json!(true), "{context}");
            assert!(content_text(tool).contains(needle), "{context}: {tool}");
            let reply = session.read("spec://lantern-keep/tree");
            assert_eq!(error_code(&reply), INTERNAL_ERROR, "{context}: {reply}");
            let done = session.finish();
            assert!(done.status.success(), "{context}: {}", done.stderr);
            assert_eq!(done.stderr, "", "{context}");
        }
    }
    // The same project with a usable HOME lists.
    let mut session = Session::open(Era::Legacy, &[], Some(&root), Home::At(&fresh));
    let reply = session.resources(None);
    assert!(uris_of(result(&reply)).len() > 10, "{reply}");
    assert!(data.is_file(), "the blocking file is left as it was");
}

/// The message of a -32603 reply, failing with `context` on any other.
fn internal_error_message<'a>(reply: &'a Value, context: &str) -> &'a Value {
    assert!(
        reply.get("error").is_some(),
        "{context}: expected -32603, got {reply}"
    );
    assert_eq!(error_code(reply), INTERNAL_ERROR, "{context}: {reply}");
    &reply["error"]["message"]
}

/// One `resources/list` case: the server's flags (`--root`, `--config`),
/// working directory and `HOME` (`None`: unset), the slug a read names.
struct ListCase {
    what: String,
    args: Vec<String>,
    cwd: PathBuf,
    home: Option<PathBuf>,
    slug: &'static str,
}

impl ListCase {
    fn new(what: impl Into<String>, cwd: &Path, home: Option<&Path>, slug: &'static str) -> Self {
        Self {
            what: what.into(),
            args: Vec::new(),
            cwd: cwd.to_path_buf(),
            home: home.map(Path::to_path_buf),
            slug,
        }
    }

    fn flag(mut self, flag: &str, value: impl AsRef<Path>) -> Self {
        self.args.push(flag.to_owned());
        let value = value.as_ref().to_str().expect("UTF-8 path").to_owned();
        self.args.push(value);
        self
    }

    /// The CLI library's globals for the same flags.
    fn globals(&self) -> Globals {
        let mut globals = Globals::default();
        for pair in self.args.chunks(2) {
            let value = PathBuf::from(&pair[1]);
            match pair[0].as_str() {
                "--root" => globals.root = Some(value),
                "--config" => globals.config = Some(value),
                other => panic!("unknown flag {other}"),
            }
        }
        globals
    }

    fn open(&self, era: Era) -> Session {
        let args: Vec<&str> = self.args.iter().map(String::as_str).collect();
        let home = match &self.home {
            Some(home) => Home::At(home),
            None => Home::Unset,
        };
        Session::open(era, &args, Some(&self.cwd), home)
    }
}

/// Data ("Rules": no project or `HOME` → an empty list): `resources/list`
/// is `[]` in both eras, without `nextCursor` and without an error, when the
/// CLI's `locate` finds no project (an empty directory; `--root` missing,
/// relative and missing, a file, a directory without `specengine.toml`
/// while the working directory is a project; `--root` missing beside a good
/// `--config`; a `specengine.toml` that is a directory) or `HOME` is unset,
/// empty or relative. The CLI library cannot list either, and its reason is
/// no project or `HOME`; `resources/read` of the tree is -32603; no index is
/// built. M: `locate` failures answered -32603; `[]` for a usable project.
#[test]
fn ac10_no_project_or_no_usable_home_lists_nothing() {
    let scratch = Scratch::new("res-none");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    let empty = scratch.dir("empty");
    let bare = scratch.dir("bare");
    write(scratch.path(), "a-file", "not a directory\n");
    let file = scratch.join("a-file");
    let missing = scratch.join("missing");
    let config_dir = scratch.dir("config-is-a-directory");
    std::fs::create_dir_all(config_dir.join("specengine.toml")).expect("mkdir");
    let good_config = root.join("specengine.toml");
    let slug = "lantern-keep";
    let cases = [
        ListCase::new("an empty directory", &empty, Some(&home), slug),
        ListCase::new("--root missing", &empty, Some(&home), slug).flag("--root", &missing),
        ListCase::new("--root relative, missing", &root, Some(&home), slug)
            .flag("--root", "nowhere"),
        ListCase::new("--root a file", &root, Some(&home), slug).flag("--root", &file),
        ListCase::new("--root without specengine.toml", &root, Some(&home), slug)
            .flag("--root", &bare),
        ListCase::new("--root missing, --config good", &empty, Some(&home), slug)
            .flag("--root", &missing)
            .flag("--config", &good_config),
        ListCase::new(
            "specengine.toml a directory",
            &config_dir,
            Some(&home),
            slug,
        ),
        ListCase::new("HOME unset", &root, None, slug),
        ListCase::new("HOME empty", &root, Some(Path::new("")), slug),
        ListCase::new(
            "HOME relative",
            &root,
            Some(Path::new("relative/home")),
            slug,
        ),
    ];
    for case in &cases {
        let env = cli_env(&case.cwd, case.home.as_deref());
        let globals = case.globals();
        let cli = documents(&env, &globals).expect_err(&case.what);
        assert!(
            locate(&env, &globals).is_err() || specengine_cli::data_dir(&env).is_err(),
            "{}: the CLI's reason is neither no project nor HOME: {}",
            case.what,
            cli.message
        );
        for era in ERAS {
            let context = format!("{era:?} {}", case.what);
            let mut session = case.open(era);
            let reply = session.resources(None);
            assert!(reply.get("error").is_none(), "{context}: {reply}");
            let page = result(&reply);
            assert_eq!(page["resources"], json!([]), "{context}");
            assert!(page.get("nextCursor").is_none(), "{context}: {page}");
            let reply = session.read(&format!("spec://{}/tree", case.slug));
            internal_error_message(&reply, &context);
            let done = session.finish();
            assert!(done.status.success(), "{context}: {}", done.stderr);
            assert_eq!(done.stderr, "", "{context}");
        }
    }
    assert!(
        !data_dir(&home).exists(),
        "an index was built for a list that found no project"
    );
    // The project these cases stand beside lists, both eras.
    for era in ERAS {
        let mut session = Session::open(era, &[], Some(&root), Home::At(&home));
        let reply = session.resources(None);
        let uris = uris_of(result(&reply));
        assert_eq!(uris[0], "spec://lantern-keep/tree", "{era:?}: {reply}");
        assert!(uris.len() > 10, "{era:?}: {reply}");
    }
}

/// Data ("Resources", CLI `documents`; developer's iteration 3): once
/// `locate` finds a project, every failure of the list is -32603 whose
/// message is the CLI's line exactly (`documents`' error), in both eras:
/// a `[[[ broken` config (at the root, found from a subdirectory, with
/// `HOME` unset: the config is met first), a non-UTF-8 config, a bad
/// `[ids]` value (its line), no slug (also with `HOME` unset), `--config`
/// naming a missing file or a directory, `--root` good with a broken
/// `--config` outside it, an unreadable `specengine.toml`, a file where
/// the data directory goes. `get_tree` answers the same line as an error
/// result, `resources/read` of the tree -32603 with the same line. The
/// config cases run on spec-a and spec-b. M: `resources/list` answering
/// `[]` when `discover`, `slug` or `documents` fails after `locate`
/// succeeded.
#[test]
fn ac10_a_found_project_that_cannot_list_is_an_internal_error() {
    let scratch = Scratch::new("res-cause");
    let fresh = scratch.home("fresh");
    let blocked = scratch.home("blocked");
    let data = data_dir(&blocked);
    std::fs::create_dir_all(data.parent().expect("parent")).expect("mkdir");
    std::fs::write(&data, "a file where the data directory goes\n").expect("write");
    let mut cases: Vec<(ListCase, String)> = Vec::new();
    for (fixture, slug) in [("spec-a", "lantern-keep"), ("spec-b", "zerkalo")] {
        let broken = scratch.copy(fixture, &format!("{fixture}-broken"));
        write(&broken, "specengine.toml", "[[[ broken\n");
        let line_one = "specengine.toml:1: ".to_owned();
        cases.push((
            ListCase::new(format!("{fixture} broken"), &broken, Some(&fresh), slug),
            line_one.clone(),
        ));
        cases.push((
            ListCase::new(
                format!("{fixture} broken, from docs/"),
                &broken.join("docs"),
                Some(&fresh),
                slug,
            ),
            line_one.clone(),
        ));
        cases.push((
            ListCase::new(format!("{fixture} broken, HOME unset"), &broken, None, slug),
            line_one,
        ));
        let latin1 = scratch.copy(fixture, &format!("{fixture}-latin1"));
        let mut bytes = b"# caf\xe9\n".to_vec();
        bytes.extend(read_text(&latin1, "specengine.toml").into_bytes());
        write(&latin1, "specengine.toml", bytes);
        cases.push((
            ListCase::new(format!("{fixture} non-UTF-8"), &latin1, Some(&fresh), slug),
            "spec: specengine.toml: the file is not UTF-8".to_owned(),
        ));
        let no_slug = scratch.copy(fixture, &format!("{fixture}-noslug"));
        replace(
            &no_slug,
            "specengine.toml",
            &format!("slug = \"{slug}\"\n"),
            "",
        );
        assert!(!read_text(&no_slug, "specengine.toml").contains("slug ="));
        cases.push((
            ListCase::new(format!("{fixture} no slug"), &no_slug, Some(&fresh), slug),
            "slug".to_owned(),
        ));
        cases.push((
            ListCase::new(
                format!("{fixture} no slug, HOME unset"),
                &no_slug,
                None,
                slug,
            ),
            "slug".to_owned(),
        ));
        let good = scratch.copy(fixture, &format!("{fixture}-good"));
        cases.push((
            ListCase::new(
                format!("{fixture} data directory a file"),
                &good,
                Some(&blocked),
                slug,
            ),
            "data directory".to_owned(),
        ));
    }
    let root = scratch.copy("spec-a", "spec-a-flags");
    let bad_ids = scratch.copy("spec-a", "spec-a-ids");
    replace(
        &bad_ids,
        "specengine.toml",
        "width = 2, immutable_text",
        "width = \"x\", immutable_text",
    );
    cases.push((
        ListCase::new(
            "spec-a bad [ids] value",
            &bad_ids,
            Some(&fresh),
            "lantern-keep",
        ),
        "specengine.toml:14: ".to_owned(),
    ));
    cases.push((
        ListCase::new("--config missing", &root, Some(&fresh), "lantern-keep")
            .flag("--config", "nope.toml"),
        "spec: cannot read nope.toml: ".to_owned(),
    ));
    let docs = root.join("docs");
    cases.push((
        ListCase::new("--config a directory", &root, Some(&fresh), "lantern-keep")
            .flag("--config", &docs),
        format!("spec: cannot read {}: ", docs.display()),
    ));
    write(scratch.path(), "outside/broken.toml", "[[[ broken\n");
    let outside = scratch.join("outside/broken.toml");
    let empty = scratch.dir("empty");
    cases.push((
        ListCase::new(
            "--root good, --config broken",
            &empty,
            Some(&fresh),
            "lantern-keep",
        )
        .flag("--root", &root)
        .flag("--config", &outside),
        format!("{}:1: ", outside.display()),
    ));
    let unreadable = scratch.copy("spec-a", "spec-a-unreadable");
    let config = unreadable.join("specengine.toml");
    std::fs::set_permissions(&config, std::fs::Permissions::from_mode(0o000)).expect("chmod");
    if std::fs::read(&config).is_err() {
        cases.push((
            ListCase::new(
                "specengine.toml unreadable",
                &unreadable,
                Some(&fresh),
                "lantern-keep",
            ),
            "spec: cannot read specengine.toml: ".to_owned(),
        ));
    }
    for (case, needle) in &cases {
        let env = cli_env(&case.cwd, case.home.as_deref());
        let globals = case.globals();
        assert!(
            locate(&env, &globals).is_ok(),
            "{}: no project found",
            case.what
        );
        let line = documents(&env, &globals).expect_err(&case.what).message;
        assert!(line.contains(needle.as_str()), "{}: {line}", case.what);
        for era in ERAS {
            let context = format!("{era:?} {}", case.what);
            let mut session = case.open(era);
            let reply = session.resources(None);
            let message = internal_error_message(&reply, &context);
            assert_eq!(message, &json!(line), "{context}");
            let reply = session.call("get_tree", json!({}));
            let tool = result(&reply);
            assert_eq!(tool["isError"], json!(true), "{context}: {tool}");
            assert_eq!(content_text(tool), format!("{line}\n"), "{context}");
            let reply = session.read(&format!("spec://{}/tree", case.slug));
            let message = internal_error_message(&reply, &context);
            assert_eq!(message, &json!(line), "{context}");
            let done = session.finish();
            assert!(done.status.success(), "{context}: {}", done.stderr);
            assert_eq!(done.stderr, "", "{context}");
        }
    }
    // One session: broken → -32603, repaired → listed, broken again → -32603.
    let live = scratch.copy("spec-a", "spec-a-live");
    let original = read_text(&live, "specengine.toml");
    for era in ERAS {
        let mut session = Session::open(era, &[], Some(&live), Home::At(&fresh));
        write(&live, "specengine.toml", "[[[ broken\n");
        internal_error_message(&session.resources(None), &format!("{era:?} broken"));
        write(&live, "specengine.toml", &original);
        let reply = session.resources(None);
        let uris = uris_of(result(&reply));
        assert_eq!(uris[0], "spec://lantern-keep/tree", "{era:?}: {reply}");
        assert!(uris.len() > 10, "{era:?}: {reply}");
        write(&live, "specengine.toml", "[[[ broken\n");
        internal_error_message(&session.resources(None), &format!("{era:?} broken"));
        let done = session.finish();
        assert!(done.status.success(), "{era:?}: {}", done.stderr);
        write(&live, "specengine.toml", &original);
    }
}
