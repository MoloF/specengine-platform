//! docs/features/reference-promotion.md (RM-0138, ADR-0089) "Acceptance
//! criteria", the daemon's half: a hub (`--public-origin`, the test proxy,
//! `Remote-User`) over a git repository of `fixtures/promote-mini` (its
//! `[team]`: `ann` the owner, `lev` leading the world's documents alone,
//! `mira` everything under the world), git-lfs alone on the daemon's `PATH`
//! where a test says so (D18: missing, the test fails naming it). Uploads
//! are streamed in 64 KiB pieces; every expected value is worked out from
//! the spec and docs/canon/work-references-promotion.md.
//!
//! - PR1, the route: `POST .../references/RF-0001/promote` stores the
//!   CLI's columns but the ID, author and times, writing nothing under the
//!   root; `propose_change {kind: "reference"}` over the hub's `/mcp` alike.
//!   M: the route writing the file at propose.
//! - PR4, the route: no home is a 503 with the line, no row, no event.
//! - PR6, the route: `GET .../references?on=C-0002` carries the `in_spec` row.
//! - PR7, the hub: under `[auto] mode = "hub"` the sole owner's promotion over
//!   `/mcp` is held `proposal-kind` `a reference waits for a person`, open,
//!   no commit. M: `reference` added to the covered kinds.
//! - PR8: PR-0001 staged: `lev`'s GET and POST confirm 403 naming the file,
//!   nothing written; `mira`'s POST 200 `applied` (a pointer committed);
//!   `ann`'s 200 on another. M: the cover judged on `target_path` alone.
//! - PR13: a confirm of a 256 MiB WAV into an LFS home and of a 50 MiB one
//!   into a plain home raises the daemon's RSS (every 50 ms) at most 16 MiB;
//!   the object's SHA-256 and the blob's BLAKE3 are the originals'. M: the
//!   blob read whole.
//! - PR16, the route: an unknown key or no `rationale` 400, another `as`
//!   403, an unknown RF 404, a link 409, no home 503; a 200 is the `--brief`
//!   review and one `proposal.created`; the documents carry `ref_id`. M:
//!   `reference_id` accepted on an `update` (MCP's half: its crate).
//! - PR18, the sync: a `--push` hub over a bare remote whose LFS store is the
//!   standalone `file://` one: a developer's commit bringing a pointer leaves
//!   it (no smudge: a `git-lfs` wrapper logs every call); after a confirm the
//!   sync uploads, then pushes, the remote holding the object; the upload
//!   failing: `retrying`, `error` naming it, no push; every sync child carries
//!   `GIT_LFS_SKIP_SMUDGE=1`. M: the push before the upload.
//!
//! The image's half of PR18 (the Dockerfile, the smoke) is `tests/deploy.rs`.
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;
#[path = "../../specengine-cli/tests/promote_common/lfs.rs"]
mod lfs;
#[path = "../../specengine-cli/tests/work_refs_common/mod.rs"]
mod refs_common;

use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{Read, Write as _};
use std::net::TcpStream;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use common::hub::{ORIGIN, Proxied, events_of, hub_args, through};
use common::mcp_http::{Stateless, content_text, is_error, result_of};
use common::{
    Reply, Scratch, Server, data_dir, product_env, read_text, real_git, spec_bin, spec_json, write,
};
use lfs::{Pointer, Stub};
use refs_common::{SeekWav, blob_fixture, file_equals, png};
use serde_json::{Value, json};

const SLUG: &str = "promote-mini";
const P: &str = "/api/projects/promote-mini";
const MIB: u64 = 1024 * 1024;
const OCTETS: &str = "application/octet-stream";
const HARBOUR_MD: &str = "docs/spec/world/harbour.md";
const HARBOUR_PNG: &str = "docs/spec/world/media/harbour.png";
const MIRA_MD: &str = "docs/spec/cast/mira.md";
const MIRA_PNG: &str = "docs/spec/cast/art/mira.png";

// ------------------------------------------------------------- the scene

/// A git repository of promote-mini with the checkout's identity, its
/// `HOME`, a working directory and a directory holding git-lfs alone.
struct Desk {
    server: Server,
    root: PathBuf,
    home: PathBuf,
    cwd: PathBuf,
    scratch: Scratch,
}

/// The product environment with `dirs` first on `PATH`.
fn env_with(home: &Path, dirs: &[&Path]) -> Vec<(OsString, OsString)> {
    product_env(home)
        .into_iter()
        .map(|(name, value)| {
            if name == "PATH" {
                (name.clone(), lfs::path_with(dirs, &value))
            } else {
                (name, value)
            }
        })
        .collect()
}

impl Desk {
    /// A hub over promote-mini, `edit` applied to its config and committed,
    /// git-lfs on its `PATH` when `with_lfs`.
    fn new(name: &str, with_lfs: bool, edit: impl FnOnce(&str) -> String) -> Self {
        let scratch = Scratch::new(name);
        let root = scratch.repo("promote-mini", "served", "main");
        let git = scratch.git();
        git.run(&root, &["config", "user.name", "Owner"]);
        git.run(&root, &["config", "user.email", "owner@example.invalid"]);
        let config = read_text(&root, "specengine.toml");
        let edited = edit(&config);
        if edited != config {
            write(&root, "specengine.toml", edited);
            git.run(&root, &["commit", "-qam", "the config"]);
        }
        let home = scratch.home("h");
        let cwd = scratch.dir("cwd");
        let lfs = lfs::lfs_dir(scratch.path());
        let env = if with_lfs {
            env_with(&home, &[lfs.as_path()])
        } else {
            product_env(&home)
        };
        let server = Server::try_start_env(&env, &cwd, &hub_args(&[&root], ORIGIN))
            .unwrap_or_else(|refused| panic!("the hub did not start: {refused:?}"));
        Self {
            server,
            root,
            home,
            cwd,
            scratch,
        }
    }

    fn get(&self, login: &str, path: &str) -> Reply {
        through(&self.server, "GET", path, &Proxied::fetch(login), b"")
    }

    /// `POST <P>/path` with the JSON `body` by `login`.
    fn post(&self, login: &str, path: &str, body: &Value) -> Reply {
        self.post_bytes(login, path, body.to_string().as_bytes())
    }

    fn post_bytes(&self, login: &str, path: &str, body: &[u8]) -> Reply {
        through(
            &self.server,
            "POST",
            &format!("{P}/{path}"),
            &Proxied::write(login),
            body,
        )
    }

    /// A card on `targets` made by `login`: its ID.
    fn card(&self, login: &str, title: &str, targets: &[&str]) -> String {
        let reply = self.post(login, "cards", &json!({"title": title, "target": targets}));
        assert_eq!(reply.status, 200, "a card: {}", reply.text());
        id_of(&reply.json())
    }

    /// `PUT <P>/blobs` of `bytes`, then the reference on `on`: its ID.
    fn reference(
        &self,
        login: &str,
        on: &str,
        name: &str,
        bytes: &[u8],
        matters: Option<&str>,
    ) -> String {
        let put = through(
            &self.server,
            "PUT",
            &format!("{P}/blobs"),
            &Proxied::fetch(login)
                .plus("Origin", ORIGIN)
                .plus("Content-Type", OCTETS),
            bytes,
        );
        assert_eq!(put.status, 200, "PUT: {}", put.text());
        let b3 = put.json()["b3"].clone();
        let mut body = json!({"on": on, "b3": b3, "name": name});
        if let Some(matters) = matters {
            body["matters"] = json!(matters);
        }
        let made = self.post(login, "references", &body);
        assert_eq!(made.status, 200, "attach: {}", made.text());
        id_of(&made.json())
    }

    /// `POST <P>/references/<rf>/promote` of `body` by `login`.
    fn promote(&self, login: &str, rf: &str, body: &Value) -> Reply {
        self.post(login, &format!("references/{rf}/promote"), body)
    }

    /// [`Desk::promote`] to `to`, a 200: the proposal's ID.
    fn promoted(&self, login: &str, rf: &str, to: &str) -> String {
        let reply = self.promote(login, rf, &json!({"to": to, "rationale": "Approved."}));
        assert_eq!(reply.status, 200, "promote {rf}: {}", reply.text());
        id_of(&reply.json())
    }

    fn review(&self, id: &str) -> Value {
        spec_json(&self.home, &self.cwd, &self.root, &["review", id]).json()
    }

    /// `id` staged as an approve by `login`.
    fn stage(&self, login: &str, id: &str) {
        let reply = self.post(
            login,
            &format!("proposals/{id}/decision"),
            &json!({"decision": "approve", "updated_at": self.review(id)["updated_at"]}),
        );
        assert_eq!(reply.status, 200, "{login} stages {id}: {}", reply.text());
    }

    /// `GET .../confirm` by `login`.
    fn read(&self, login: &str, id: &str) -> Reply {
        self.get(login, &format!("{P}/proposals/{id}/confirm"))
    }

    /// `POST .../confirm` by `login` of `read`'s question.
    fn confirm(&self, login: &str, id: &str, read: &Value) -> Reply {
        let body = json!({"question": read["question"], "updated_at": read["updated_at"]});
        self.post(login, &format!("proposals/{id}/confirm"), &body)
    }

    /// One click by `login`: the read and its POST, each a 200: the document.
    fn confirmed(&self, login: &str, id: &str) -> Value {
        let read = self.read(login, id);
        assert_eq!(read.status, 200, "{login} reads {id}: {}", read.text());
        let reply = self.confirm(login, id, &read.json());
        assert_eq!(reply.status, 200, "{login} confirms {id}: {}", reply.text());
        let document = reply.json();
        assert_eq!(document["status"], json!("applied"), "{document}");
        document
    }

    fn events(&self) -> Vec<(String, Value)> {
        events_of(&self.home, &self.root)
    }

    fn git(&self, args: &[&str]) -> String {
        self.scratch
            .git()
            .run(&self.root, args)
            .trim_end()
            .to_owned()
    }

    /// `HEAD`, `git status` and every file of the checkout but `.git`'s.
    fn repository(&self) -> Vec<String> {
        let mut files: Vec<String> = common::snapshot(&self.root)
            .into_iter()
            .filter(|(path, _)| path != ".git" && !path.starts_with(".git/"))
            .map(|(path, bytes)| format!("{path} {:?}", bytes.map(|b| b.len())))
            .collect();
        files.sort();
        vec![
            self.git(&["rev-parse", "HEAD"]),
            self.git(&["status", "--porcelain=v1", "--untracked-files=all"]),
            files.join("\n"),
        ]
    }

    /// The proposal rows and the events: what a refusal must leave.
    fn queue(&self) -> (String, Vec<(String, Value)>) {
        let db = data_dir(&self.home).join(format!("{SLUG}.db"));
        let rows = if db.exists() {
            sqlite(&db, "SELECT * FROM proposals ORDER BY id;")
        } else {
            String::new()
        };
        (rows, self.events())
    }

    fn sql(&self, query: &str) -> String {
        sqlite(&data_dir(&self.home).join(format!("{SLUG}.db")), query)
    }
}

fn id_of(document: &Value) -> String {
    document["id"]
        .as_str()
        .unwrap_or_else(|| panic!("an ID: {document}"))
        .to_owned()
}

fn sqlite(db: &Path, sql: &str) -> String {
    let client = ["/usr/bin/sqlite3", "/bin/sqlite3", "/usr/local/bin/sqlite3"]
        .into_iter()
        .find(|candidate| Path::new(candidate).exists())
        .expect("a sqlite3 client");
    let output = std::process::Command::new(client)
        .args(["-cmd", ".timeout 10000"])
        .arg(db)
        .arg(sql)
        .stdin(std::process::Stdio::null())
        .output()
        .expect("sqlite3 runs");
    assert!(
        output.status.success(),
        "sqlite3 {sql}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// The proposal's columns every surface stores alike: all but the ID, the
/// author's and the times.
fn same_columns(db_of: &dyn Fn(&str) -> String, id: &str) -> String {
    db_of(&format!(
        "SELECT kind, status, target_id, target_path, target_ids, base_hash, base_text, \
         new_text, patch_hash, ref_id, task_id, comment_id, record_id FROM proposals \
         WHERE id = '{id}';"
    ))
}

// ----------------------------------------------------------- PR1, PR16

/// PR1, the route: a promotion through the board's door stores the CLI's
/// columns (but ID, author, times), writing nothing under the root; its 200
/// is the `--brief` review; one `proposal.created`. M: the route writing
/// the file at propose.
#[test]
fn pr1_the_route_stores_the_clis_columns_and_writes_nothing() {
    let desk = Desk::new("pr-hub-pr1", true, str::to_owned);
    let card = desk.card("ann", "Light the harbour", &["LOC-HARBOUR"]);
    let harbour = fs::read(blob_fixture("harbour.png")).expect("the fixture");
    let rf = desk.reference("mira", &card, "harbour.png", &harbour, None);
    assert_eq!(rf, "RF-0001");
    let before = desk.repository();
    let events = desk.events().len();
    let reply = desk.promote(
        "mira",
        &rf,
        &json!({"to": "LOC-HARBOUR", "rationale": "Approved by the art lead."}),
    );
    assert_eq!(reply.status, 200, "{}", reply.text());
    let id = id_of(&reply.json());
    assert_eq!(desk.repository(), before, "M: nothing under the root");
    assert_eq!(
        desk.events()[events..]
            .iter()
            .map(|(kind, _)| kind.clone())
            .collect::<Vec<_>>(),
        vec!["proposal.created".to_owned()]
    );
    // The 200 is the `--brief` review, as the CLI prints it.
    // The CLI with the daemon's environment (git-lfs on `PATH`, so both
    // judge the apply alike).
    let output = common::alarmed(
        &spec_bin(),
        &[],
        &env_with(&desk.home, &[lfs::lfs_dir(desk.scratch.path()).as_path()]),
    )
    .current_dir(&desk.cwd)
    .args([
        "--root",
        desk.root.to_str().expect("UTF-8"),
        "--json",
        "review",
        &id,
        "--brief",
    ])
    .stdin(std::process::Stdio::null())
    .output()
    .expect("spec runs");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let brief: Value = serde_json::from_slice(&output.stdout).expect("JSON");
    let (route, cli) = (reply.json(), brief);
    let differing: Vec<(&String, &Value, &Value)> = route
        .as_object()
        .expect("an object")
        .iter()
        .filter(|(key, value)| cli.get(key.as_str()) != Some(value))
        .map(|(key, value)| (key, value, &cli[key.as_str()]))
        .collect();
    assert_eq!(
        differing,
        Vec::new(),
        "the --brief review document, key by key"
    );
    assert_eq!(route, cli, "the --brief review document");
    let author: Value = serde_json::from_str(
        desk.sql(&format!("SELECT author FROM proposals WHERE id = '{id}';"))
            .trim(),
    )
    .expect("an author");
    assert_eq!(
        author["login"],
        json!("mira"),
        "authored as the login: {author}"
    );

    // The CLI on a twin of the repository stores the same columns.
    let twin = Scratch::new("pr-hub-pr1-twin");
    let root = twin.repo("promote-mini", "twin", "main");
    let home = twin.home("h");
    let cwd = twin.dir("cwd");
    let lfs_dir = lfs::lfs_dir(twin.path());
    let spec = |args: &[&str]| {
        let mut command = common::alarmed(&spec_bin(), &[], &env_with(&home, &[lfs_dir.as_path()]));
        let output = command
            .current_dir(&cwd)
            .arg("--root")
            .arg(&root)
            .args(args)
            .stdin(std::process::Stdio::null())
            .output()
            .expect("spec runs");
        assert!(
            output.status.success(),
            "spec {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice::<Value>(&output.stdout).expect("JSON")
    };
    spec(&[
        "--json",
        "card",
        "new",
        "--title",
        "Light the harbour",
        "--target",
        "LOC-HARBOUR",
    ]);
    let harbour_file = twin.dir("files").join("harbour.png");
    fs::write(&harbour_file, &harbour).expect("the file");
    spec(&[
        "--json",
        "ref",
        "add",
        "--on",
        "C-0001",
        harbour_file.to_str().expect("UTF-8"),
    ]);
    let cli = id_of(&spec(&[
        "--json",
        "propose",
        "reference",
        "RF-0001",
        "--to",
        "LOC-HARBOUR",
        "--rationale",
        "Approved by the art lead.",
    ]));
    let twin_db = data_dir(&home).join(format!("{SLUG}.db"));
    assert_eq!(
        same_columns(&|q: &str| desk.sql(q), &id),
        same_columns(&|q: &str| sqlite(&twin_db, q), &cli),
        "the route and the CLI store the same columns"
    );

    // `propose_change {kind: "reference"}` over the hub's `/mcp`, after the
    // route's row is gone from the live set (a second live one is refused).
    let mut agent = Stateless::new(desk.server.port, SLUG, "mira");
    let twice = agent.call(
        "propose_change",
        json!({"kind": "reference", "target": "LOC-HARBOUR", "reference_id": rf, "text": "",
               "rationale": "Approved by the art lead.", "author_role": "art-lead"}),
    );
    let result = result_of(&twice, "propose_change");
    assert!(is_error(result), "a second live promotion: {result}");
    assert!(
        content_text(result).contains(&format!(
            "{rf} is already proposed for `LOC-HARBOUR` by {id} (open)"
        )),
        "{}",
        content_text(result)
    );
    assert_eq!(desk.repository(), before, "nothing under the root");
}

/// PR16, the route: its refusals in their statuses, each storing nothing.
#[test]
fn pr16_the_routes_refusals() {
    let desk = Desk::new("pr-hub-pr16", true, str::to_owned);
    let card = desk.card("ann", "Light the harbour", &["LOC-HARBOUR"]);
    let harbour = fs::read(blob_fixture("harbour.png")).expect("the fixture");
    let rf = desk.reference("mira", &card, "harbour.png", &harbour, None);
    let link = desk.post(
        "mira",
        "references",
        &json!({"on": card, "url": "https://example.org/harbour", "title": "Board"}),
    );
    assert_eq!(link.status, 200, "{}", link.text());
    let link = id_of(&link.json());
    let tide = desk.card("ann", "The tide", &["MEC-TIDE"]);
    let tide_rf = desk.reference("mira", &tide, "tide.png", &png(12, 12, 0x21, None), None);
    let before = (desk.repository(), desk.queue());
    let good = json!({"to": "LOC-HARBOUR", "rationale": "R"});
    let mut unknown = good.clone();
    unknown["dry_run"] = json!(true);
    let cases: Vec<(&str, String, Value, u16)> = vec![
        ("an unknown key", rf.clone(), unknown, 400),
        (
            "no rationale",
            rf.clone(),
            json!({"to": "LOC-HARBOUR"}),
            400,
        ),
        ("no to", rf.clone(), json!({"rationale": "R"}), 400),
        (
            "another as",
            rf.clone(),
            json!({"to": "LOC-HARBOUR", "rationale": "R", "as": "lev"}),
            403,
        ),
        ("an unknown RF", "RF-0099".to_owned(), good.clone(), 404),
        (
            "an unknown target",
            rf.clone(),
            json!({"to": "LOC-NOWHERE", "rationale": "R"}),
            404,
        ),
        ("a link", link.clone(), good.clone(), 409),
        (
            "no home",
            tide_rf.clone(),
            json!({"to": "MEC-TIDE", "rationale": "R"}),
            503,
        ),
    ];
    for (what, id, body, status) in cases {
        let reply = desk.promote("mira", &id, &body);
        assert_eq!(reply.status, status, "{what}: {}", reply.text());
        assert_eq!(
            (desk.repository(), desk.queue()),
            before,
            "{what}: nothing stored"
        );
        match status {
            403 => assert!(
                reply.text().contains("refused: "),
                "{what}: {}",
                reply.text()
            ),
            404 | 409 => {
                let document = reply.json();
                assert!(
                    document["id"].is_string() && document["reason"].is_string(),
                    "{what}: {document}"
                );
            }
            400 => {
                let document = reply.json();
                assert!(document["message"].is_string(), "{what}: {document}");
            }
            _ => {}
        }
        if what == "no home" {
            assert!(
                reply.text().contains(
                    "no [references] home takes kind `mechanic` (`MEC-TIDE`): add one to [[references.homes]] in specengine.toml; nothing stored"
                ),
                "PR4: the line: {}",
                reply.text()
            );
        }
        if what == "a link" {
            assert!(
                reply
                    .text()
                    .contains("is a link: only a file is promoted into git"),
                "{}",
                reply.text()
            );
        }
    }
    // A body declared over 64 KiB is refused before it is read (as a
    // confirm's over its cap), nothing stored.
    let over = 64 * 1024 + 1;
    let page = Proxied::write("mira").plus("Content-Length", &over.to_string());
    let bytes = page.bytes("POST", &format!("{P}/references/{rf}/promote"), b"");
    let reply = desk.server.raw(&bytes);
    assert_eq!(reply.status, 413, "over 64 KiB: {}", reply.text());
    assert_eq!((desk.repository(), desk.queue()), before);
    // `as` equal to the login passes; the documents carry `ref_id`.
    let reply = desk.promote(
        "mira",
        &rf,
        &json!({"to": "LOC-HARBOUR", "rationale": "R", "as": "mira", "name": "harbour.png",
                "matters": "the light at dusk"}),
    );
    assert_eq!(reply.status, 200, "{}", reply.text());
    let id = id_of(&reply.json());
    assert_eq!(reply.json()["ref_id"], json!(rf));
    assert_eq!(desk.review(&id)["ref_id"], json!(rf));
    assert_eq!(
        desk.sql(&format!(
            "SELECT new_text FROM proposals WHERE id = '{id}';"
        ))
        .trim(),
        "media/harbour.png | the light at dusk"
    );
    let inbox = desk.get("ann", &format!("{P}/inbox"));
    assert_eq!(inbox.status, 200);
    assert!(inbox.text().contains(&id), "{}", inbox.text());
}

// ------------------------------------------------------------------ PR6

/// PR6, the route: after a promotion is applied, `GET .../references?on=`
/// a card on the document's section carries the `in_spec` row.
#[test]
fn pr6_the_list_route_carries_in_spec() {
    let desk = Desk::new("pr-hub-pr6", true, str::to_owned);
    let c1 = desk.card("ann", "Light the harbour", &["LOC-HARBOUR"]);
    let c2 = desk.card("ann", "The tides", &["RULE-HARBOUR-TIDES"]);
    let harbour = fs::read(blob_fixture("harbour.png")).expect("the fixture");
    let rf = desk.reference("mira", &c1, "harbour.png", &harbour, None);
    let id = desk.promoted("mira", &rf, "LOC-HARBOUR");
    desk.stage("ann", &id);
    desk.confirmed("ann", &id);
    let reply = desk.get("lev", &format!("{P}/references?on={c2}"));
    assert_eq!(reply.status, 200, "{}", reply.text());
    assert_eq!(
        reply.json()["in_spec"],
        json!([{"node": "LOC-HARBOUR", "document": HARBOUR_MD, "path": HARBOUR_PNG,
                "matters": null}])
    );
    // The page's card read carries the context's counts too.
    let shown = desk.get("lev", &format!("{P}/cards/{c2}"));
    assert_eq!(shown.status, 200);
}

// ------------------------------------------------------------------ PR8

/// PR8: the cover holds both paths. `lev` leads `docs/spec/world/*.md`,
/// which holds the document but not the file: his read and his POST are
/// 403 naming the file, nothing written; `mira`, leading
/// `docs/spec/world/**`, confirms it: a pointer committed; `ann`, the
/// owner, confirms another. M: the cover judged on `target_path` alone.
#[test]
fn pr8_a_confirm_covers_the_document_and_the_file() {
    let desk = Desk::new("pr-hub-pr8", true, str::to_owned);
    let c1 = desk.card("ann", "Light the harbour", &["LOC-HARBOUR"]);
    let c2 = desk.card("ann", "Draw Mira", &["CHR-MIRA"]);
    let harbour = fs::read(blob_fixture("harbour.png")).expect("the fixture");
    let rf1 = desk.reference("mira", &c1, "harbour.png", &harbour, None);
    let mira_png = png(40, 30, 0x2c, None);
    let rf2 = desk.reference("mira", &c2, "mira.png", &mira_png, None);
    let first = desk.promoted("mira", &rf1, "LOC-HARBOUR");
    let second = desk.promoted("mira", &rf2, "CHR-MIRA");
    desk.stage("ann", &first);
    desk.stage("ann", &second);

    let line = format!("refused: lev leads world-docs; {HARBOUR_PNG} lies outside them");
    let before = (desk.repository(), desk.queue());
    let read = desk.read("lev", &first);
    assert_eq!(read.status, 403, "{}", read.text());
    assert_eq!(read.error_message(), line, "M: target_path alone");
    let question = desk.read("ann", &first);
    assert_eq!(question.status, 200, "{}", question.text());
    let refused = desk.confirm("lev", &first, &question.json());
    assert_eq!(refused.status, 403, "{}", refused.text());
    assert_eq!(refused.error_message(), line);
    assert_eq!(desk.repository(), before.0, "nothing written");
    assert_eq!(desk.review(&first)["status"], json!("open"), "still open");

    let document = desk.confirmed("mira", &first);
    assert_eq!(document["id"], json!(first));
    let blob = desk.git(&["cat-file", "blob", &format!("HEAD:{HARBOUR_PNG}")]);
    let pointer = Pointer::parse(format!("{blob}\n").as_bytes())
        .unwrap_or_else(|| panic!("a pointer committed: {blob:?}"));
    assert_eq!(pointer.size, harbour.len() as u64);
    assert_eq!(
        desk.git(&[
            "log",
            "-1",
            "--format=%(trailers:key=Reference,valueonly)",
            "HEAD"
        ])
        .trim(),
        rf1
    );

    let document = desk.confirmed("ann", &second);
    assert_eq!(document["id"], json!(second));
    assert_eq!(
        fs::read(desk.root.join(MIRA_PNG)).expect("the file"),
        mira_png,
        "plain: the bytes"
    );
    assert!(read_text(&desk.root, MIRA_MD).contains("references:\n  - \"art/mira.png\"\n"));
    assert_eq!(
        desk.git(&["status", "--porcelain=v1", "--untracked-files=all"]),
        "",
        "the checkout clean"
    );
}

// ------------------------------------------------------------------- PR7

/// PR7, the hub: `ann` the sole owner, `[auto] mode = "hub"` covering every
/// path: her `update` over `/mcp` applies by rule, her promotion stays open,
/// held `proposal-kind`, no commit of it. M: `reference` added to the
/// covered kinds.
#[test]
fn pr7_the_hub_never_applies_a_promotion_by_rule() {
    let desk = Desk::new("pr-hub-pr7", true, |config| {
        let team = config.find("[team.lev]").expect("lev's entry");
        let mut text = config[..team].replace("[zones]\n", "[zones]\ndocs       = [\"docs/**\"]\n");
        text.push_str("[auto]\nmode  = \"hub\"\nzones = [\"docs\"]\n");
        text
    });
    let card = desk.card("ann", "Light the harbour", &["LOC-HARBOUR"]);
    let harbour = fs::read(blob_fixture("harbour.png")).expect("the fixture");
    let rf = desk.reference("ann", &card, "harbour.png", &harbour, None);
    let mut ann = Stateless::new(desk.server.port, SLUG, "ann");
    let head = desk.git(&["rev-parse", "HEAD"]);
    let held_before = desk
        .events()
        .iter()
        .filter(|(kind, _)| kind == "auto.held")
        .count();
    let made = ann.call(
        "propose_change",
        json!({"kind": "reference", "target": "LOC-HARBOUR", "reference_id": rf, "text": "",
               "rationale": "The harbour at dusk.", "author_role": "art-lead",
               "author_model": "m-1"}),
    );
    let result = result_of(&made, "propose_change").clone();
    assert!(!is_error(&result), "{}", content_text(&result));
    let id = result["structuredContent"]["id"]
        .as_str()
        .unwrap_or_else(|| panic!("stored: {result}"))
        .to_owned();
    // The barrier: a later call of the same login runs after the line's apply.
    let tree = ann.call("get_tree", json!({}));
    assert!(!is_error(result_of(&tree, "get_tree")));
    thread::sleep(Duration::from_millis(300));
    assert_eq!(
        desk.review(&id)["status"],
        json!("open"),
        "M: applied by rule"
    );
    assert_eq!(desk.git(&["rev-parse", "HEAD"]), head, "no commit");
    assert!(!desk.root.join(HARBOUR_PNG).exists());
    let held: Vec<Value> = desk
        .events()
        .into_iter()
        .filter(|(kind, _)| kind == "auto.held")
        .map(|(_, payload)| payload)
        .skip(held_before)
        .collect();
    assert_eq!(held.len(), 1, "one hold: {held:?}");
    assert_eq!(held[0]["proposal"], json!(id));
    assert_eq!(held[0]["reason"], json!("proposal-kind"));
    assert_eq!(held[0]["detail"], json!("a reference waits for a person"));
    let log = desk.git(&["log", "--format=%B"]);
    assert!(!log.contains("Delegated-by:"), "{log}");
}

// ------------------------------------------------------------------ PR13

/// The daemon's RSS in KiB.
fn rss_kib(pid: u32) -> u64 {
    let output = std::process::Command::new("/bin/ps")
        .args(["-o", "rss=", "-p", &pid.to_string()])
        .output()
        .expect("ps runs");
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .unwrap_or_else(|_| panic!("ps rss of {pid}: {output:?}"))
}

/// Samples `pid`'s RSS every 50 ms until stopped: the highest seen.
struct Sampler {
    stop: Arc<AtomicBool>,
    peak: Arc<AtomicU64>,
    handle: Option<thread::JoinHandle<()>>,
}

impl Sampler {
    fn start(pid: u32) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let peak = Arc::new(AtomicU64::new(0));
        let (stopping, peaking) = (Arc::clone(&stop), Arc::clone(&peak));
        let handle = thread::spawn(move || {
            while !stopping.load(Ordering::SeqCst) {
                peaking.fetch_max(rss_kib(pid), Ordering::SeqCst);
                thread::sleep(Duration::from_millis(50));
            }
        });
        Self {
            stop,
            peak,
            handle: Some(handle),
        }
    }

    fn stop(mut self) -> u64 {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            handle.join().expect("the sampler");
        }
        self.peak.load(Ordering::SeqCst)
    }
}

/// The RSS at rest: the median of five readings.
fn rss_at_rest(pid: u32) -> u64 {
    let mut readings: Vec<u64> = (0..5)
        .map(|_| {
            thread::sleep(Duration::from_millis(100));
            rss_kib(pid)
        })
        .collect();
    readings.sort_unstable();
    readings[2]
}

/// A raw answer's status and body (a chunked body joined).
fn parse(raw: &[u8]) -> (u16, Vec<u8>) {
    let at = raw
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .unwrap_or_else(|| panic!("no head end: {:?}", String::from_utf8_lossy(raw)));
    let head = String::from_utf8_lossy(&raw[..at]).to_lowercase();
    let status = head
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse().ok())
        .unwrap_or_else(|| panic!("a status: {head}"));
    let mut rest = &raw[at + 4..];
    if !head.contains("transfer-encoding: chunked") {
        return (status, rest.to_vec());
    }
    let mut body = Vec::new();
    loop {
        let line = rest
            .windows(2)
            .position(|window| window == b"\r\n")
            .expect("a chunk size line");
        let size = usize::from_str_radix(
            String::from_utf8_lossy(&rest[..line])
                .split(';')
                .next()
                .unwrap_or("0")
                .trim(),
            16,
        )
        .expect("a hex chunk size");
        rest = &rest[line + 2..];
        if size == 0 {
            break;
        }
        body.extend_from_slice(&rest[..size]);
        rest = &rest[size + 2..];
    }
    (status, body)
}

/// `PUT <P>/blobs` of `total` bytes of `reader`, in 64 KiB pieces, never
/// held whole: the answer's document.
fn put_stream(desk: &Desk, login: &str, total: u64, mut reader: impl Read) -> Value {
    let length = total.to_string();
    let proxied = Proxied::fetch(login)
        .plus("Origin", ORIGIN)
        .plus("Content-Type", OCTETS)
        .plus("Content-Length", &length)
        .plus("Connection", "close");
    let head = common::request_bytes("PUT", &format!("{P}/blobs"), &proxied.pairs());
    let mut stream = TcpStream::connect(("127.0.0.1", desk.server.port)).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(300)))
        .expect("a timeout");
    stream.write_all(&head).expect("the head");
    let mut buffer = vec![0u8; 65_536];
    loop {
        let read = reader.read(&mut buffer).expect("the generator");
        if read == 0 {
            break;
        }
        stream.write_all(&buffer[..read]).expect("a piece");
    }
    let mut raw = Vec::new();
    let _ = stream.read_to_end(&mut raw);
    let (status, body) = parse(&raw);
    assert_eq!(status, 200, "PUT: {}", String::from_utf8_lossy(&body));
    serde_json::from_slice(&body).expect("a JSON answer")
}

/// PR13: confirms of a 256 MiB WAV into the LFS home and of a 50 MiB one
/// into the plain home each raise the daemon's RSS at most 16 MiB; the LFS
/// object's SHA-256 and the plain blob's BLAKE3 are the originals'. M: the
/// blob read whole.
#[test]
fn pr13_a_large_confirm_keeps_its_pieces() {
    let desk = Desk::new("pr-hub-pr13", true, str::to_owned);
    let c1 = desk.card("ann", "The harbour's hum", &["LOC-HARBOUR"]);
    let c2 = desk.card("ann", "Mira's voice", &["CHR-MIRA"]);
    let pid = desk.server.pid();
    for (card, to, total, seed, name, path) in [
        (
            &c1,
            "LOC-HARBOUR",
            256 * MIB,
            41_u64,
            "hum.wav",
            "docs/spec/world/media/hum.wav",
        ),
        (
            &c2,
            "CHR-MIRA",
            50 * MIB,
            42_u64,
            "voice.wav",
            "docs/spec/cast/art/voice.wav",
        ),
    ] {
        let blob = put_stream(&desk, "mira", total, SeekWav::new(total, seed));
        let attached = desk.post(
            "mira",
            "references",
            &json!({"on": card, "b3": blob["b3"], "name": name, "matters": "the low hum"}),
        );
        assert_eq!(attached.status, 200, "{}", attached.text());
        let rf = id_of(&attached.json());
        let id = desk.promoted("mira", &rf, to);
        desk.stage("ann", &id);
        let read = desk.read("ann", &id);
        assert_eq!(read.status, 200, "{}", read.text());
        let rest = rss_at_rest(pid);
        let sampler = Sampler::start(pid);
        let reply = desk.confirm("ann", &id, &read.json());
        let peak = sampler.stop();
        assert_eq!(reply.status, 200, "{name}: {}", reply.text());
        let rise = peak.saturating_sub(rest);
        assert!(
            rise <= 16 * 1024,
            "{name}: M: the blob read whole: RSS {rest} KiB at rest, {peak} KiB at its peak (+{rise} KiB)"
        );
        let committed = desk.git(&["cat-file", "-s", &format!("HEAD:{path}")]);
        if to == "LOC-HARBOUR" {
            let pointer_text = desk.git(&["cat-file", "blob", &format!("HEAD:{path}")]);
            let pointer = Pointer::parse(format!("{pointer_text}\n").as_bytes())
                .unwrap_or_else(|| panic!("a pointer: {pointer_text:?}"));
            assert_eq!(pointer.size, total);
            let object = lfs::lfs_object(&desk.root.join(".git"), &pointer.oid);
            assert_eq!(
                lfs::sha256_file(&object),
                pointer.oid,
                "the object's SHA-256"
            );
            assert!(
                file_equals(&object, SeekWav::new(total, seed)),
                "the object is the bytes"
            );
        } else {
            assert_eq!(
                committed.trim(),
                total.to_string(),
                "a plain blob of the size"
            );
            assert!(
                file_equals(&desk.root.join(path), SeekWav::new(total, seed)),
                "the file is the original's bytes (its BLAKE3)"
            );
        }
    }
}

// ------------------------------------------------------------------ PR18

/// A `--push` hub's world: a bare `origin.git` (its LFS store the
/// standalone one under it), a developer's clone, the hub's checkout, every
/// git child of the daemon through a wrapper logging its argv and its
/// `GIT_LFS_SKIP_SMUDGE` and `GIT_NO_LAZY_FETCH`, every `git-lfs` call through
/// a wrapper logging its argv.
struct World {
    server: Option<Server>,
    scratch: Scratch,
    hub: PathBuf,
    home: PathBuf,
    cwd: PathBuf,
    bare: PathBuf,
    dev: PathBuf,
    wrap: PathBuf,
    git_log: PathBuf,
    lfs_wrap: PathBuf,
    /// The daemon's `GIT_CONFIG_GLOBAL` when set (a shared hooks
    /// directory); else the product's `/dev/null`.
    global: Option<PathBuf>,
}

impl World {
    fn new(name: &str, lfs_stub: Option<Stub>) -> Self {
        let scratch = Scratch::new(name);
        let git = scratch.git();
        let dev = scratch.repo("promote-mini", "dev", "main");
        let bare = scratch.dir("origin.git");
        git.run(
            &bare,
            &[
                "init",
                "-q",
                "--bare",
                "--template=",
                "--initial-branch=main",
            ],
        );
        git.quiet(&bare);
        git.run(&bare, &["config", "receive.autogc", "false"]);
        git.run(&dev, &["remote", "add", "origin", text(&bare)]);
        git.run(&dev, &["push", "-q", "-u", "origin", "main"]);
        let hub = scratch.join("hub");
        git.run(
            scratch.path(),
            &["clone", "-q", "--template=", text(&bare), text(&hub)],
        );
        let hub = fs::canonicalize(hub).expect("the checkout");
        git.quiet(&hub);
        git.run(&hub, &["config", "user.name", "Owner"]);
        git.run(&hub, &["config", "user.email", "owner@example.invalid"]);
        let wrap = scratch.dir("wrap");
        let git_log = scratch.join("git-children.log");
        script(
            &wrap.join("git"),
            &format!(
                "us=$(printf '\\037')\n\
                 line=\"${{GIT_LFS_SKIP_SMUDGE-unset}}$us${{GIT_NO_LAZY_FETCH-unset}}\"\n\
                 for arg in \"$@\"; do line=\"$line$us$arg\"; done\n\
                 printf '%s\\n' \"$line\" >> '{}'\n\
                 exec '{}' \"$@\"",
                git_log.display(),
                real_git().display()
            ),
        );
        let lfs_wrap = match lfs_stub {
            Some(stub) => lfs::stub_lfs_dir(scratch.path(), "lfs-wrap", stub),
            None => lfs::stub_lfs_dir(scratch.path(), "lfs-wrap", Stub::Log),
        };
        Self {
            server: None,
            home: scratch.home("h"),
            cwd: scratch.dir("cwd"),
            scratch,
            hub,
            bare,
            dev,
            wrap,
            git_log,
            lfs_wrap,
            global: None,
        }
    }

    /// The daemon's environment: the git wrapper, then the git-lfs one, then
    /// the product's `PATH`; no `GIT_LFS_SKIP_SMUDGE` of its own.
    fn env(&self) -> Vec<(OsString, OsString)> {
        env_with(&self.home, &[self.wrap.as_path(), self.lfs_wrap.as_path()])
            .into_iter()
            .filter(|(name, _)| name != "GIT_LFS_SKIP_SMUDGE")
            .map(|(name, value)| match &self.global {
                Some(global) if name == "GIT_CONFIG_GLOBAL" => (name, global.as_os_str().into()),
                _ => (name, value),
            })
            .collect()
    }

    fn start(&mut self) {
        let mut args = hub_args(&[&self.hub], ORIGIN);
        for flag in ["--push", "--push-deadline", "10", "--push-every", "1"] {
            args.push(OsStr::new(flag));
        }
        let server = Server::try_start_env(&self.env(), &self.cwd, &args)
            .unwrap_or_else(|refused| panic!("the hub did not start: {refused:?}"));
        self.server = Some(server);
        self.after(&json!({"at": null}), Duration::from_secs(60));
    }

    fn server(&self) -> &Server {
        self.server.as_ref().expect("a running hub")
    }

    fn post(&self, login: &str, path: &str, body: &Value) -> Reply {
        through(
            self.server(),
            "POST",
            &format!("{P}/{path}"),
            &Proxied::write(login),
            body.to_string().as_bytes(),
        )
    }

    fn status(&self) -> Value {
        let reply = through(
            self.server(),
            "GET",
            &format!("{P}/sync"),
            &Proxied::fetch("ann"),
            b"",
        );
        assert_eq!(reply.status, 200, "GET sync: {}", reply.text());
        reply.json()
    }

    /// The first status after `before`'s whose `at` differs and is set.
    fn after(&self, before: &Value, within: Duration) -> Value {
        let started = Instant::now();
        loop {
            let status = self.status();
            if !status["at"].is_null() && status["at"] != before["at"] {
                return status;
            }
            assert!(
                started.elapsed() < within,
                "no sync ended within {within:?}: {status}\n{}",
                self.server().stderr()
            );
            thread::sleep(Duration::from_millis(100));
        }
    }

    /// The first status whose `state` is `state`, within `within`.
    fn until(&self, state: &str, within: Duration) -> Value {
        let started = Instant::now();
        loop {
            let status = self.status();
            if status["state"] == json!(state) {
                return status;
            }
            assert!(
                started.elapsed() < within,
                "no `{state}` within {within:?}: {status}\n{}",
                self.server().stderr()
            );
            thread::sleep(Duration::from_millis(200));
        }
    }

    fn run(&self, dir: &Path, args: &[&str]) -> String {
        self.scratch.git().run(dir, args).trim_end().to_owned()
    }

    fn remote_main(&self) -> String {
        self.run(&self.bare, &["rev-parse", "refs/heads/main"])
    }

    /// Every git child the wrapper logged: `(GIT_LFS_SKIP_SMUDGE,
    /// GIT_NO_LAZY_FETCH, argv)`.
    fn children(&self) -> Vec<(String, String, Vec<String>)> {
        fs::read_to_string(&self.git_log)
            .unwrap_or_default()
            .lines()
            .map(|line| {
                let mut fields = line.split('\u{1f}');
                let skip = fields.next().unwrap_or_default().to_owned();
                let lazy = fields.next().unwrap_or_default().to_owned();
                (skip, lazy, fields.map(str::to_owned).collect())
            })
            .collect()
    }

    /// `id` staged and confirmed by `ann`: the POST's document.
    fn confirm(&self, id: &str) -> Value {
        let review = spec_json(&self.home, &self.cwd, &self.hub, &["review", id]).json();
        let stage = self.post(
            "ann",
            &format!("proposals/{id}/decision"),
            &json!({"decision": "approve", "updated_at": review["updated_at"]}),
        );
        assert_eq!(stage.status, 200, "{}", stage.text());
        let route = format!("{P}/proposals/{id}/confirm");
        let read = through(self.server(), "GET", &route, &Proxied::fetch("ann"), b"");
        assert_eq!(read.status, 200, "{}", read.text());
        let read = read.json();
        let body = json!({"question": read["question"], "updated_at": read["updated_at"]});
        let reply = through(
            self.server(),
            "POST",
            &route,
            &Proxied::write("ann"),
            body.to_string().as_bytes(),
        );
        assert_eq!(reply.status, 200, "{}", reply.text());
        reply.json()
    }

    /// A promotion of the fixture's harbour.png to LOC-HARBOUR through the
    /// routes: its ID and the bytes.
    fn promotion(&self) -> (String, Vec<u8>) {
        let card = self.post(
            "ann",
            "cards",
            &json!({"title": "Light the harbour", "target": ["LOC-HARBOUR"]}),
        );
        assert_eq!(card.status, 200, "{}", card.text());
        let card = id_of(&card.json());
        let harbour = fs::read(blob_fixture("harbour.png")).expect("the fixture");
        let put = through(
            self.server(),
            "PUT",
            &format!("{P}/blobs"),
            &Proxied::fetch("ann")
                .plus("Origin", ORIGIN)
                .plus("Content-Type", OCTETS),
            &harbour,
        );
        assert_eq!(put.status, 200, "{}", put.text());
        let made = self.post(
            "ann",
            "references",
            &json!({"on": card, "b3": put.json()["b3"], "name": "harbour.png"}),
        );
        assert_eq!(made.status, 200, "{}", made.text());
        let rf = id_of(&made.json());
        let promoted = self.post(
            "ann",
            &format!("references/{rf}/promote"),
            &json!({"to": "LOC-HARBOUR", "rationale": "R"}),
        );
        assert_eq!(promoted.status, 200, "{}", promoted.text());
        (id_of(&promoted.json()), harbour)
    }
}

fn text(path: &Path) -> &str {
    path.to_str().expect("a UTF-8 path")
}

fn script(path: &Path, body: &str) {
    fs::write(path, format!("#!/bin/sh\n{body}\n")).expect("a script");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("chmod");
}

/// The subcommand of a git argv: the first word past git's own options.
fn subcommand(args: &[String]) -> Option<&str> {
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-C" | "-c" => {
                iter.next();
            }
            other if other.starts_with('-') => {}
            other => return Some(other),
        }
    }
    None
}

/// PR18: after a confirm the sync uploads the commit's LFS object, then
/// pushes; the remote holds the object and the commit. M: the push before
/// the upload.
#[test]
fn pr18_the_sync_uploads_then_pushes() {
    let mut world = World::new("pr-hub-pr18-push", None);
    world.start();
    let (id, harbour) = world.promotion();
    let before = world.status();
    thread::sleep(Duration::from_millis(1100));
    let document = world.confirm(&id);
    assert_eq!(document["status"], json!("applied"), "{document}");
    let applied = document["applied_commit"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let status = world.after(&before, Duration::from_secs(60));
    let status = if status["state"] == json!("ok") {
        status
    } else {
        world.until("ok", Duration::from_secs(60))
    };
    assert_eq!(status["state"], json!("ok"), "{status}");
    let tip = world.remote_main();
    if !applied.is_empty() {
        assert!(
            world
                .scratch
                .git()
                .output(
                    &world.bare,
                    &["merge-base", "--is-ancestor", &applied, &tip]
                )
                .status
                .success(),
            "the applied commit reached the remote"
        );
    }
    let sha = lfs::sha256_bytes(world.scratch.path(), &harbour);
    let object = lfs::lfs_object(&world.bare, &sha);
    assert_eq!(
        fs::read(&object).ok(),
        Some(harbour),
        "the remote holds the object"
    );

    // The order: the LFS upload's child before the push's.
    let children = world.children();
    let upload = children.iter().position(|(_, _, args)| {
        subcommand(args) == Some("lfs")
            && args.iter().any(|arg| arg == "push")
            && !args.iter().any(|arg| arg == "--dry-run")
    });
    let push = children
        .iter()
        .position(|(_, _, args)| subcommand(args) == Some("push"));
    assert!(
        matches!((upload, push), (Some(u), Some(p)) if u < p),
        "M: the push before the upload: upload {upload:?}, push {push:?}"
    );
    // Every sync child skips the smudge.
    for (skip, _, args) in &children {
        if matches!(subcommand(args), Some("fetch" | "merge" | "push" | "lfs")) {
            assert_eq!(skip, "1", "GIT_LFS_SKIP_SMUDGE on {args:?}");
        }
    }
}

/// PR18: a developer's commit bringing a pointer: the hub's merge leaves the
/// pointer in its checkout and no `smudge` runs.
#[test]
fn pr18_a_merged_pointer_is_never_smudged() {
    let mut world = World::new("pr-hub-pr18-merge", None);
    world.start();
    // The developer adds an LFS file with git-lfs's own tools.
    let lfs_dir = lfs::lfs_dir(world.scratch.path());
    let git = world.scratch.git();
    let bytes = png(30, 30, 0x55, None);
    let source = world.scratch.dir("files").join("gate.png");
    fs::write(&source, &bytes).expect("the file");
    let path = "docs/spec/world/media/gate.png";
    fs::create_dir_all(world.dev.join("docs/spec/world/media")).expect("the home");
    let mut vars = git.vars();
    for (name, value) in vars.iter_mut() {
        if name == "PATH" {
            *value = lfs::path_with(&[lfs_dir.as_path()], value);
        }
    }
    let run = |args: &[&str], stdin: Option<&Path>| {
        let mut command = std::process::Command::new(real_git());
        command
            .env_clear()
            .envs(vars.iter().map(|(k, v)| (k, v)))
            .current_dir(&world.dev)
            .args(args);
        if let Some(file) = stdin {
            command.stdin(fs::File::open(file).expect("the file"));
        }
        let output = command.output().expect("git runs");
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        output.stdout
    };
    let pointer = run(&["lfs", "clean", "--", path], Some(&source));
    fs::write(world.dev.join(path), &pointer).expect("the pointer");
    run(&["add", path], None);
    run(&["commit", "-q", "-m", "The gate, by a developer."], None);
    run(&["lfs", "push", "origin", "refs/heads/main"], None);
    run(&["push", "-q", "origin", "main"], None);
    let status = world.until("ok", Duration::from_secs(60));
    let started = Instant::now();
    while !world.hub.join(path).exists() && started.elapsed() < Duration::from_secs(60) {
        thread::sleep(Duration::from_millis(200));
    }
    assert_eq!(
        fs::read(world.hub.join(path)).expect("merged"),
        pointer,
        "the pointer stays: {status}"
    );
    let calls = lfs::stub_calls(&world.lfs_wrap);
    assert!(
        !calls
            .iter()
            .any(|call| call.starts_with("smudge") || call.starts_with("filter-process")),
        "no smudge: {calls:?}"
    );
}

/// PR18: the upload failing (a git-lfs refusing `push`): `retrying`, the
/// error naming it, no push; the remote unchanged.
#[test]
fn pr18_a_failed_upload_holds_the_push() {
    let mut world = World::new("pr-hub-pr18-fail", Some(Stub::FailPush));
    world.start();
    let remote = world.remote_main();
    let (id, _) = world.promotion();
    let pushes_before = world
        .children()
        .iter()
        .filter(|(_, _, args)| subcommand(args) == Some("push"))
        .count();
    world.confirm(&id);
    let status = world.until("retrying", Duration::from_secs(60));
    let error = status["error"].as_str().unwrap_or_default();
    assert!(
        error.contains("the LFS upload failed: "),
        "the error names the upload: {status}"
    );
    assert_eq!(world.remote_main(), remote, "the remote unchanged");
    let pushes = world
        .children()
        .iter()
        .filter(|(_, _, args)| subcommand(args) == Some("push"))
        .count();
    assert_eq!(pushes, pushes_before, "no push after the failed upload");
}

// ------------------------------------------------------ the review's

/// The review's finding 4: a local daemon (`spec serve`, no
/// `--public-origin`): a page's promotion with `as` keeps the acting name as
/// its author, as a local comment keeps it.
#[test]
fn review_a_local_promotion_keeps_its_acting_name() {
    let scratch = Scratch::new("pr-local-as");
    let root = scratch.repo("promote-mini", "served", "main");
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let server = Server::try_start_env(
        &env_with(&home, &[lfs::lfs_dir(scratch.path()).as_path()]),
        &cwd,
        &common::root_args(&[&root]),
    )
    .unwrap_or_else(|refused| panic!("the daemon did not start: {refused:?}"));
    let made = server.stage(
        &format!("{P}/cards"),
        &json!({"title": "Light the harbour", "target": ["LOC-HARBOUR"]}).to_string(),
    );
    assert_eq!(made.status, 200, "{}", made.text());
    let harbour = fs::read(blob_fixture("harbour.png")).expect("the fixture");
    let put = server.send(
        "PUT",
        &format!("{P}/blobs"),
        &[
            ("Sec-Fetch-Site", "same-origin"),
            ("Origin", &format!("http://127.0.0.1:{}", server.port)),
            ("Content-Type", OCTETS),
        ],
        &harbour,
    );
    assert_eq!(put.status, 200, "{}", put.text());
    let attached = server.stage(
        &format!("{P}/references"),
        &json!({"on": "C-0001", "b3": put.json()["b3"], "name": "harbour.png", "as": "Mira K"})
            .to_string(),
    );
    assert_eq!(attached.status, 200, "{}", attached.text());
    let rf = id_of(&attached.json());
    let promoted = server.stage(
        &format!("{P}/references/{rf}/promote"),
        &json!({"to": "LOC-HARBOUR", "rationale": "Approved.", "as": "Mira K"}).to_string(),
    );
    assert_eq!(promoted.status, 200, "{}", promoted.text());
    let id = id_of(&promoted.json());
    let db = data_dir(&home).join(format!("{SLUG}.db"));
    let author: Value = serde_json::from_str(
        sqlite(
            &db,
            &format!("SELECT author FROM proposals WHERE id = '{id}';"),
        )
        .trim(),
    )
    .expect("an author");
    assert!(
        author
            .as_object()
            .is_some_and(|map| map.values().any(|value| value == &json!("Mira K"))),
        "the acting name kept: {author}"
    );
    let unnamed = server.stage(
        &format!("{P}/references/{rf}/promote"),
        &json!({"to": "LOC-HARBOUR", "rationale": "Again."}).to_string(),
    );
    assert_eq!(unnamed.status, 409, "a second live one: {}", unnamed.text());
}

/// git-lfs's own hook files, as it writes them into a repository (`git lfs
/// update --force`, the hooks alone): `(name, bytes)`, made in a scratch
/// repository.
fn lfs_hooks(scratch: &Scratch) -> Vec<(String, Vec<u8>)> {
    let repo = scratch.dir("hook-maker");
    let git = scratch.git();
    git.run(
        &repo,
        &["init", "-q", "--template=", "--initial-branch=main"],
    );
    let lfs_dir = lfs::lfs_dir(scratch.path());
    let output = std::process::Command::new(real_git())
        .env_clear()
        .envs(git.vars())
        .env(
            "PATH",
            lfs::path_with(&[lfs_dir.as_path()], &common::product_path()),
        )
        .current_dir(&repo)
        .args(["lfs", "update", "--force"])
        .stdin(std::process::Stdio::null())
        .output()
        .expect("git lfs clean");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut hooks = Vec::new();
    for name in ["pre-push", "post-checkout", "post-commit", "post-merge"] {
        let path = repo.join(".git/hooks").join(name);
        hooks.push((name.to_owned(), fs::read(&path).expect("a git-lfs hook")));
    }
    hooks
}

/// The review's finding 6: a hub whose git speaks German and has no
/// git-lfs: a plain promotion's sync still pushes (the missing git-lfs is
/// known by more than an English message).
#[test]
fn review_a_localized_git_without_git_lfs_still_pushes() {
    let mut world = World::new("pr-hub-localized", None);
    // No git-lfs at all: the logging stand-in's directory left off `PATH`.
    let mut args = hub_args(&[&world.hub], ORIGIN);
    for flag in ["--push", "--push-deadline", "10", "--push-every", "1"] {
        args.push(OsStr::new(flag));
    }
    let mut env: Vec<(OsString, OsString)> = product_env(&world.home)
        .into_iter()
        .map(|(name, value)| {
            if name == "PATH" {
                (name, lfs::path_with(&[world.wrap.as_path()], &value))
            } else {
                (name, value)
            }
        })
        .collect();
    for (name, value) in [
        ("LANG", "de_DE.UTF-8"),
        ("LC_ALL", "de_DE.UTF-8"),
        ("LANGUAGE", "de"),
    ] {
        env.push((name.into(), value.into()));
    }
    world.server = Some(
        Server::try_start_env(&env, &world.cwd, &args)
            .unwrap_or_else(|refused| panic!("the hub did not start: {refused:?}")),
    );
    world.after(&json!({"at": null}), Duration::from_secs(60));
    let card = world.post(
        "ann",
        "cards",
        &json!({"title": "Draw Mira", "target": ["CHR-MIRA"]}),
    );
    assert_eq!(card.status, 200, "{}", card.text());
    let card = id_of(&card.json());
    let mira = png(40, 30, 0x2c, None);
    let put = through(
        world.server(),
        "PUT",
        &format!("{P}/blobs"),
        &Proxied::fetch("ann")
            .plus("Origin", ORIGIN)
            .plus("Content-Type", OCTETS),
        &mira,
    );
    assert_eq!(put.status, 200, "{}", put.text());
    let made = world.post(
        "ann",
        "references",
        &json!({"on": card, "b3": put.json()["b3"], "name": "mira.png"}),
    );
    assert_eq!(made.status, 200, "{}", made.text());
    let rf = id_of(&made.json());
    let promoted = world.post(
        "ann",
        &format!("references/{rf}/promote"),
        &json!({"to": "CHR-MIRA", "rationale": "R"}),
    );
    assert_eq!(promoted.status, 200, "{}", promoted.text());
    let id = id_of(&promoted.json());
    let before = world.status();
    thread::sleep(Duration::from_millis(1100));
    let document = world.confirm(&id);
    assert_eq!(document["status"], json!("applied"), "{document}");
    let after = world.after(&before, Duration::from_secs(60));
    let status = if after["state"] == json!("ok") {
        after
    } else {
        world.until("ok", Duration::from_secs(60))
    };
    assert_eq!(status["state"], json!("ok"), "{status}");
    let applied = document["applied_commit"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    assert!(!applied.is_empty(), "{document}");
    {
        assert!(
            world
                .scratch
                .git()
                .output(
                    &world.bare,
                    &["merge-base", "--is-ancestor", &applied, "refs/heads/main"]
                )
                .status
                .success(),
            "the commit reached the remote"
        );
    }
}

/// The review's finding 6: hooks git-lfs left in the checkout by a killed
/// upload are removed by the next sync; a hook the checkout held before,
/// the owner's own `pre-push`, is never removed.
#[test]
fn review_the_sync_removes_leftover_lfs_hooks_and_keeps_its_owners() {
    let mut world = World::new("pr-hub-hooks", None);
    let hooks = lfs_hooks(&world.scratch);
    let dir = world.hub.join(".git/hooks");
    fs::create_dir_all(&dir).expect("hooks");
    // A killed upload's leftovers: `post-checkout` and `post-commit`, git-lfs's
    // own text byte for byte.
    let template = |name: &str| -> Vec<u8> {
        hooks
            .iter()
            .find(|(hook, _)| hook == name)
            .map(|(_, bytes)| bytes.clone())
            .expect("a git-lfs hook")
    };
    for name in ["post-checkout", "post-commit"] {
        fs::write(dir.join(name), template(name)).expect("a leftover");
        fs::set_permissions(dir.join(name), fs::Permissions::from_mode(0o755)).expect("chmod");
    }
    // A person's own hooks, there before: a `pre-push` that pastes git-lfs's
    // guard line and `git lfs pre-push "$@"` into commands of its own, and a
    // `post-merge` that is git-lfs's plus one line (iteration 3: only a whole
    // text equal to git-lfs's template is git-lfs's).
    let mut own = template("pre-push");
    own.extend_from_slice(b"# the owner's own check\ntrue\n");
    let mut merged = template("post-merge");
    merged.extend_from_slice(b"# and the owner's step\n");
    for (name, bytes) in [("pre-push", &own), ("post-merge", &merged)] {
        fs::write(dir.join(name), bytes).expect("the owner's hook");
        fs::set_permissions(dir.join(name), fs::Permissions::from_mode(0o755)).expect("chmod");
    }
    world.start();
    let (id, _) = world.promotion();
    let before = world.status();
    thread::sleep(Duration::from_millis(1100));
    world.confirm(&id);
    // The sync after the confirm: it uploads the object, then pushes.
    let after = world.after(&before, Duration::from_secs(60));
    let status = if after["state"] == json!("ok") {
        after
    } else {
        world.until("ok", Duration::from_secs(60))
    };
    assert_eq!(status["state"], json!("ok"), "{status}");
    let tip = world.remote_main();
    assert!(
        world
            .scratch
            .git()
            .output(
                &world.bare,
                &["cat-file", "-e", &format!("{tip}:{HARBOUR_PNG}")]
            )
            .status
            .success(),
        "premise: the promotion was pushed"
    );
    assert_eq!(
        fs::read(dir.join("pre-push")).ok(),
        Some(own),
        "the owner's hook kept"
    );
    assert_eq!(
        fs::read(dir.join("post-merge")).ok(),
        Some(merged),
        "the owner's merged hook kept"
    );
    for name in ["post-checkout", "post-commit"] {
        assert!(
            !dir.join(name).exists(),
            "git-lfs's leftover {name} removed by the sync"
        );
    }
}

// ------------------------------------------------- the release gate's hooks

/// The sync after a confirmed LFS promotion: the promotion pushed, the
/// status `ok` within a minute (else the sync hung).
fn synced_promotion(world: &World) {
    let (id, _) = world.promotion();
    let before = world.status();
    thread::sleep(Duration::from_millis(1100));
    world.confirm(&id);
    let after = world.after(&before, Duration::from_secs(60));
    let status = if after["state"] == json!("ok") {
        after
    } else {
        world.until("ok", Duration::from_secs(60))
    };
    assert_eq!(status["state"], json!("ok"), "{status}");
    let tip = world.remote_main();
    assert!(
        world
            .scratch
            .git()
            .output(
                &world.bare,
                &["cat-file", "-e", &format!("{tip}:{HARBOUR_PNG}")]
            )
            .status
            .success(),
        "the promotion was pushed"
    );
}

/// git-lfs's own text of its hook `name`, from a scratch repository.
fn lfs_template(scratch: &Scratch, name: &str) -> Vec<u8> {
    lfs_hooks(scratch)
        .into_iter()
        .find(|(hook, _)| hook == name)
        .map(|(_, bytes)| bytes)
        .expect("a git-lfs hook")
}

/// The release gate: a FIFO named `pre-push` in the checkout's hooks is
/// never read (a read would wait for a writer forever): the sync uploads,
/// pushes and ends `ok`, the FIFO left as it was. M: a hook read whatever
/// its file type.
#[test]
fn gate_a_fifo_hook_is_skipped_and_the_sync_ends() {
    use std::os::unix::fs::FileTypeExt as _;
    let mut world = World::new("pr-gate-fifo", None);
    let dir = world.hub.join(".git/hooks");
    fs::create_dir_all(&dir).expect("hooks");
    let fifo = dir.join("pre-push");
    let made = std::process::Command::new("/usr/bin/mkfifo")
        .arg(&fifo)
        .status()
        .expect("mkfifo runs");
    assert!(made.success(), "mkfifo");
    world.start();
    synced_promotion(&world);
    let meta = fs::symlink_metadata(&fifo).expect("the FIFO stays");
    assert!(meta.file_type().is_fifo(), "still a FIFO: {meta:?}");
}

/// The release gate: a hook that is a symlink to a file holding git-lfs's
/// own text is a person's choice, never removed; its target untouched. M: a
/// hook's file type judged through the link.
#[test]
fn gate_a_symlinked_hook_is_never_removed() {
    let mut world = World::new("pr-gate-symlink", None);
    let template = lfs_template(&world.scratch, "post-checkout");
    let target = world.scratch.join("kept-post-checkout");
    fs::write(&target, &template).expect("the link's target");
    fs::set_permissions(&target, fs::Permissions::from_mode(0o755)).expect("chmod");
    let dir = world.hub.join(".git/hooks");
    fs::create_dir_all(&dir).expect("hooks");
    let link = dir.join("post-checkout");
    std::os::unix::fs::symlink(&target, &link).expect("the symlinked hook");
    world.start();
    synced_promotion(&world);
    let meta = fs::symlink_metadata(&link).expect("the link stays");
    assert!(meta.file_type().is_symlink(), "still a symlink: {meta:?}");
    assert_eq!(fs::read_link(&link).expect("its target"), target);
    assert_eq!(
        fs::read(&target).expect("the target"),
        template,
        "the target untouched"
    );
}

/// The release gate: a hooks directory outside the repository (a global
/// `core.hooksPath`) is shared: a hook holding git-lfs's own text that was
/// there before the upload stays, byte for byte, after the sync. M: a
/// shared directory's template hooks removed as a checkout's are.
#[test]
fn gate_a_shared_hooks_directory_keeps_a_template_hook_it_held() {
    let mut world = World::new("pr-gate-shared", None);
    let shared = world.scratch.dir("shared-hooks");
    let template = lfs_template(&world.scratch, "post-merge");
    let held = shared.join("post-merge");
    fs::write(&held, &template).expect("the shared hook");
    fs::set_permissions(&held, fs::Permissions::from_mode(0o755)).expect("chmod");
    let global = world.scratch.join("global.gitconfig");
    fs::write(
        &global,
        format!("[core]\n\thooksPath = {}\n", shared.display()),
    )
    .expect("the global config");
    world.global = Some(global);
    // Premise: the daemon's git reads the shared directory as its hooks.
    let said = std::process::Command::new(real_git())
        .env_clear()
        .envs(world.env())
        .current_dir(&world.hub)
        .args(["rev-parse", "--git-path", "hooks"])
        .output()
        .expect("git rev-parse");
    assert_eq!(
        String::from_utf8_lossy(&said.stdout).trim(),
        shared.display().to_string(),
        "premise: the hooks directory is the shared one"
    );
    world.start();
    synced_promotion(&world);
    assert_eq!(
        fs::read(&held).ok(),
        Some(template),
        "the shared directory's own hook kept"
    );
}
