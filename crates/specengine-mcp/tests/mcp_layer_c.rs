//! docs/features/layer-c-resolve.md R5 and R12, the MCP half: the code
//! tools `find_symbols`, `refs` and `unmapped` (docs/canon/mcp-code.md)
//! over a scratch `fixtures/code-index`, its index written through the CLI
//! library (`common::read::index_with_library`), a scratch `HOME`.
//!
//! - R5 `mcp_code_tools_never_spawn_layer_c`: a copy of the server with a logging
//!   `specengine-ra` beside it and another first on `PATH`; every code
//!   tool, each form, answers, and neither fake ever starts.
//! - R12 `mcp_code_tools_equal_with_and_without_a_snapshot`: in both
//!   protocol eras each code tool's reply line is byte-identical before
//!   and after a layer C snapshot is in the index (a row of
//!   `code_monikers` at every layer A item's bytes), and none shows a
//!   moniker; the snapshot is still there after the reads.
//!
//! PHASE-B: `spec` is not built for this crate's tests, so the snapshot is
//! written with the system's `sqlite3` in the spec's columns ("Tables":
//! `code_resolve`, `code_resolve_files`, `code_monikers`); a column the
//! store names otherwise fails the insert, naming it.
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

#![cfg(all(unix, not(feature = "probes")))]

mod common;

use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead as _, BufReader, Write as _};
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use specengine_cli::Globals;

use common::read::{ERAS, Session, cli_env, index_with_library};
use common::{BIN, Home, LEGACY_VERSION, Scratch, copy_file, data_dir, parse_message};

/// The fixture and its `[project] slug`.
const FIXTURE: &str = "code-index";
const SLUG: &str = "inkwell";

/// The crate word of every moniker the snapshot holds: no reply may show it.
const MONIKER_WORD: &str = "zzmoniker";

/// How long one reply may take.
const READ_LIMIT: Duration = Duration::from_secs(60);

/// Every code tool, each form.
fn calls() -> Vec<(&'static str, Value)> {
    vec![
        ("find_symbols", json!({"query": "page"})),
        ("find_symbols", json!({"query": "impl Sheet"})),
        ("find_symbols", json!({"node": "TENET-WIDTH"})),
        (
            "find_symbols",
            json!({"file": "crates/inkwell/src/page.rs"}),
        ),
        (
            "find_symbols",
            json!({"at": "crates/inkwell/src/page.rs:3"}),
        ),
        ("refs", json!({"id": "TENET-WIDTH"})),
        ("refs", json!({"id": "SLAB-INK"})),
        ("unmapped", json!({})),
        ("unmapped", json!({"path": "crates"})),
    ]
}

/// A scratch copy of the fixture (its `expected.json` out), a `HOME`, the
/// index written through the library.
fn indexed(scratch: &Scratch) -> (PathBuf, PathBuf) {
    let root = scratch.copy(FIXTURE, FIXTURE);
    let _ = fs::remove_file(root.join("expected.json"));
    let home = scratch.home("h");
    index_with_library(
        &cli_env(&root, Some(&home)),
        &Globals {
            root: Some(root.clone()),
            config: None,
        },
    );
    (root, home)
}

/// A `specengine-ra` that appends its pid and arguments to `log`, then
/// exits 3.
fn logging_fake(program: &Path, log: &Path) {
    let log = log.to_str().expect("a UTF-8 scratch path");
    assert!(!log.contains('\''), "{log}: a quote in a scratch path");
    fs::write(
        program,
        format!("#!/bin/sh\necho \"$$ $*\" >> '{log}'\nexit 3\n"),
    )
    .expect("the fake");
    fs::set_permissions(program, fs::Permissions::from_mode(0o755)).expect("chmod the fake");
}

/// A server process over stdio, killed on drop.
struct Client {
    child: Child,
    stdin: Option<ChildStdin>,
    lines: mpsc::Receiver<String>,
}

impl Client {
    fn spawn(program: &Path, root: &Path, home: &Path, path: &str) -> Client {
        let mut child = Command::new(program)
            .env_clear()
            .env("HOME", home)
            .env("PATH", path)
            .current_dir(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn the copy of specengine-mcp");
        let stdin = child.stdin.take();
        let stdout = child.stdout.take().expect("stdout");
        let (sender, lines) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else {
                    break;
                };
                if sender.send(line).is_err() {
                    break;
                }
            }
        });
        Client {
            child,
            stdin,
            lines,
        }
    }

    fn send(&mut self, message: &Value) {
        let stdin = self.stdin.as_mut().expect("stdin open");
        let mut line = serde_json::to_string(message).expect("a message");
        line.push('\n');
        stdin
            .write_all(line.as_bytes())
            .expect("write to the server");
        stdin.flush().expect("flush");
    }

    /// Sends a request; the reply with its id.
    fn request(&mut self, id: u64, method: &str, params: Value) -> Value {
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        let deadline = Instant::now() + READ_LIMIT;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let line = self
                .lines
                .recv_timeout(left)
                .unwrap_or_else(|_| panic!("no reply to {method} (id {id}) within {READ_LIMIT:?}"));
            let message = parse_message(&line);
            if message.get("id") == Some(&json!(id)) {
                return message;
            }
        }
    }

    fn finish(mut self) {
        drop(self.stdin.take());
        let deadline = Instant::now() + Duration::from_secs(10);
        while self.child.try_wait().expect("poll the server").is_none() {
            if Instant::now() > deadline {
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        if let Ok(None) = self.child.try_wait() {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
    }
}

/// R5 (WA-3). M: an MCP code tool spawning the binary. A copy of the
/// server with a logging `specengine-ra` beside it and another first on
/// `PATH`: every code tool, each form, answers; neither fake starts.
#[test]
fn mcp_code_tools_never_spawn_layer_c() {
    let scratch = Scratch::new("lcr-mcp-never");
    let (root, home) = indexed(&scratch);
    let bin = scratch.dir("bin");
    let server = bin.join("specengine-mcp");
    // A copy, never a hard link: the scratch's drop resets every file's mode
    // to 0644, which a link would carry over to the build's binary.
    copy_file(BIN, &server);
    let beside_log = scratch.join("beside.log");
    logging_fake(&bin.join("specengine-ra"), &beside_log);
    let on_path = scratch.dir("on-path");
    let path_log = scratch.join("on-path.log");
    logging_fake(&on_path.join("specengine-ra"), &path_log);
    let path = format!("{}:/usr/bin:/bin", on_path.to_str().expect("UTF-8"));

    let mut client = Client::spawn(&server, &root, &home, &path);
    let init = client.request(
        0,
        "initialize",
        json!({
            "protocolVersion": LEGACY_VERSION,
            "capabilities": {},
            "clientInfo": {"name": "specengine-mcp-tests", "version": "0.0.0"}
        }),
    );
    assert!(init.get("result").is_some(), "initialized: {init}");
    client.send(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
    for (id, (tool, arguments)) in (1..).zip(calls()) {
        let reply = client.request(
            id,
            "tools/call",
            json!({"name": tool, "arguments": arguments}),
        );
        assert!(
            reply.get("result").is_some(),
            "{tool} {arguments}: a tool result: {reply}"
        );
    }
    client.finish();
    let started = (
        fs::read_to_string(beside_log).ok(),
        fs::read_to_string(path_log).ok(),
    );
    assert_eq!(
        started,
        (None, None),
        "an MCP code tool started specengine-ra (beside, on PATH)"
    );
}

/// `sql` on `db` through the system's `sqlite3 -json`: every row.
fn sqlite(db: &Path, sql: &str) -> Vec<BTreeMap<String, Value>> {
    let program = ["/usr/bin/sqlite3", "/bin/sqlite3", "/usr/local/bin/sqlite3"]
        .into_iter()
        .find(|path| Path::new(path).exists())
        .expect("a sqlite3 client");
    let output = Command::new(program)
        .env_clear()
        .args(["-json", "-cmd", ".timeout 5000"])
        .arg(db)
        .arg(sql)
        .output()
        .expect("sqlite3 runs");
    assert!(
        output.status.success(),
        "sqlite3 {sql}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).expect("UTF-8");
    if text.trim().is_empty() {
        return Vec::new();
    }
    serde_json::from_str(&text).unwrap_or_else(|error| panic!("sqlite3 -json: {error}\n{text}"))
}

/// `text` as an SQL string literal.
fn quoted(text: &str) -> String {
    format!("'{}'", text.replace('\'', "''"))
}

/// The layer C kind of a layer A item kind (the kind map's inverse).
fn layer_c_kind(item_kind: &str) -> &'static str {
    match item_kind {
        "function_signature_item" => "method",
        "type_item" => "type_alias",
        "mod_item" => "mod",
        "macro_definition" => "macro_rules",
        "struct_item" => "struct",
        "enum_item" => "enum",
        "union_item" => "union",
        "trait_item" => "trait",
        "impl_item" => "impl",
        "const_item" => "const",
        "static_item" => "static",
        _ => "function",
    }
}

/// A snapshot of the worktree in the index at `db`: one `code_resolve`
/// row, a `code_resolve_files` row per walked UTF-8 `.rs` file (its
/// stored BLAKE3), a `code_monikers` row at every layer A item's bytes,
/// each a moniker of [`MONIKER_WORD`]. The rows written.
fn write_snapshot(db: &Path) -> u64 {
    let worktrees = sqlite(db, "SELECT wt FROM code_worktrees");
    assert_eq!(worktrees.len(), 1, "one code worktree: {worktrees:?}");
    let wt = worktrees[0]["wt"].as_i64().expect("wt");
    let files = sqlite(
        db,
        "SELECT path, blake3 FROM code_files WHERE path LIKE '%.rs' AND utf8 = 1 ORDER BY path",
    );
    let mut sql = vec!["BEGIN IMMEDIATE;".to_owned()];
    let mut items = 0u64;
    for file in &files {
        let path = file["path"].as_str().expect("a path");
        let blake3 = file["blake3"]
            .as_str()
            .map_or_else(|| "NULL".to_owned(), quoted);
        sql.push(format!(
            "INSERT INTO code_resolve_files (wt, path, blake3, in_crate) VALUES ({wt}, {}, {blake3}, 1);",
            quoted(path)
        ));
        let rows = sqlite(
            db,
            &format!(
                "SELECT s.start_byte AS start, s.end_byte AS \"end\", s.item_kind AS kind \
                 FROM symbols s JOIN code_files f ON f.file_id = s.file \
                 WHERE f.path = {} ORDER BY s.start_byte",
                quoted(path)
            ),
        );
        let mut last = None;
        for row in rows {
            let start = row["start"].as_u64().expect("start_byte");
            if last == Some(start) {
                continue;
            }
            last = Some(start);
            let end = row["end"].as_u64().expect("end_byte");
            let kind = layer_c_kind(row["kind"].as_str().expect("item_kind"));
            sql.push(format!(
                "INSERT INTO code_monikers (wt, path, start, \"end\", kind, state, moniker) VALUES ({wt}, {}, {start}, {end}, '{kind}', 'moniker', '{MONIKER_WORD} m{start}.');",
                quoted(path)
            ));
            items += 1;
        }
    }
    sql.insert(
        1,
        format!(
            "INSERT INTO code_resolve (wt, tree_fp, protocol, ra_ap, host, toolchains, degraded, workspaces, files, items) VALUES ({wt}, 'written by a test', 1, '0.0.352', 'masked', '[\"masked\"]', 0, 1, {}, {items});",
            files.len()
        ),
    );
    sql.push("COMMIT;".to_owned());
    let output = Command::new(
        ["/usr/bin/sqlite3", "/bin/sqlite3", "/usr/local/bin/sqlite3"]
            .into_iter()
            .find(|path| Path::new(path).exists())
            .expect("a sqlite3 client"),
    )
    .env_clear()
    .args(["-cmd", ".timeout 5000"])
    .arg(db)
    .arg(sql.join("\n"))
    .output()
    .expect("sqlite3 runs");
    assert!(
        output.status.success(),
        "the snapshot's rows (the spec's columns): {}",
        String::from_utf8_lossy(&output.stderr)
    );
    items
}

/// Every code tool's reply line, in both eras, each era a session of its
/// own (its ids from 1).
fn answers(root: &Path, home: &Path) -> Vec<String> {
    let mut out = Vec::new();
    for era in ERAS {
        let mut session = Session::open(era, &[], Some(root), Home::At(home));
        for (tool, arguments) in calls() {
            out.push(
                session.request_line("tools/call", json!({"name": tool, "arguments": arguments})),
            );
        }
        session.finish();
    }
    out
}

/// R12. M: a moniker leaking into a code tool. Each reply line, both eras,
/// byte for byte the same without and with a snapshot; none shows a
/// moniker; the snapshot outlives the reads.
#[test]
fn mcp_code_tools_equal_with_and_without_a_snapshot() {
    let scratch = Scratch::new("lcr-mcp-outputs");
    let (root, home) = indexed(&scratch);
    let before = answers(&root, &home);
    let db = data_dir(&home).join(format!("{SLUG}.db"));
    let items = write_snapshot(&db);
    assert!(items > 10, "{items} moniker rows");
    let after = answers(&root, &home);
    assert_eq!(before.len(), after.len());
    for ((was, now), (tool, arguments)) in before.iter().zip(&after).zip(calls().iter().cycle()) {
        assert_eq!(now, was, "{tool} {arguments}: the same with a snapshot");
        assert!(
            !now.contains(MONIKER_WORD),
            "{tool} {arguments} shows a moniker: {now}"
        );
    }
    let kept = sqlite(&db, "SELECT count(*) AS n FROM code_monikers");
    assert_eq!(
        kept[0]["n"].as_u64(),
        Some(items),
        "the snapshot outlives the reads"
    );
}
