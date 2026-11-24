//! docs/features/work-references.md (RM-0137, ADR-0088) "Acceptance
//! criteria", the daemon's half: a hub (`--public-origin`, the test proxy,
//! `Remote-User`) over a git repository of `fixtures/card-press` (a print
//! shop, no game word) with a `[team]` of an owner, a lead and members, and
//! a local daemon over `fixtures/trace-almanac` (a village almanac); the
//! files from `fixtures/blob-types/` or generated as they are sent. Uploads
//! are written to the socket in 64 KiB pieces, never held whole; answers of
//! files are read as they stream. Every expected value is worked out from
//! the spec and docs/canon/work-references.md, never read back from a run.
//!
//! - RF2, the route: `[ids]` claiming `RF` makes `PUT .../blobs`, `POST
//!   .../references` and the list 503 with "Data"'s line.
//! - RF3, the hub: each stored fixture's `BlobStored` (its 10 keys in
//!   order), each refused one a 415 with its line, no file and no `.part`.
//! - RF4: `SPECENGINE_BLOB_FILE_MAX=1048576` refuses a PUT declaring
//!   1 048 577 bytes with 413 before a `.part` exists; `SPECENGINE_BLOB_QUOTA=
//!   3145728` with 2 MiB stored refuses 2 MiB more; the 20 001st distinct
//!   file is a 413 while equal bytes still store; `SPECENGINE_BLOB_QUOTA=
//!   20000000000` and `SPECENGINE_BLOB_FLOOR=1` exit 2 at the daemon's start.
//! - RF5 `rf5_*`: one 200 MiB PUT, its body generated as sent, raises the
//!   daemon's RSS (sampled every 50 ms) at most 16 MiB; the stored file is
//!   the generator's bytes, named by the answer's BLAKE3. M: the body
//!   collected whole before hashing.
//! - RF6, the hub: `stored: new` then `existing`, one file, its mtime the
//!   second's; deleting one of two references keeps it, the other removes
//!   it.
//! - RF7: the serving headers of a PNG, a GLB and a PDF (`filename` and
//!   `filename*` from the stored name); `If-None-Match` a 304; a hash no live
//!   reference of the project names (unattached, deleted, another
//!   project's) a 404; a GET answers while a probe holds the project's turn
//!   (`probes` build, ignored by default). M: `inline` for a PDF.
//! - RF8: Range on a 3 MiB file: `bytes=a-b`, `-n`, an unsatisfiable one, two
//!   ranges and a malformed header; `If-Range`; ten concurrent readers of a
//!   200 MiB file raise RSS at most 16 MiB. M: the file read whole.
//! - RF9, the route: a WAV, MP4, GLB or PDF without or with a blank "what
//!   matters" a 400 with "Data"'s line; an image and a link without it; a
//!   `.blend` with its warning, served as an attachment. M: the rule dropped
//!   for `model`.
//! - RF11, the route: `agent_b3` naming a 1 600 px copy, a 1.1 MiB copy,
//!   another aspect, or given for an agent-ready image, a 400 each.
//! - RF12: with `[team]` the author and the `owner` delete (200), the lead,
//!   a member and an outsider 403 with the line; with no or a bad `[team]`
//!   the author alone; `GET .../team` lists `delete_reference` for the owner
//!   alone, after `delete_comment`. M: any `[team]` login may delete.
//! - RF13, the daemon: with `.swept` over an hour old an upload sweeps an
//!   unattached file 24 h + 1 s old and a `.part` 6 h + 1 s old, keeping a
//!   23 h one; a `POST` naming a candidate between the sweep's scan (the
//!   upload's start) and its turn keeps the file. M: the re-check inside the
//!   turn dropped.
//! - RF16, the hub: one `reference.*` event an attach or a delete, `as` the
//!   login, no text or hash; a PUT none; the repository unchanged.
//! - RF17, the route: after an import with no blob store a file
//!   reference's GET is a 404.
//! - RF18: the three writes on the fence's plain list (`hub.rs` pins
//!   `GET_ROUTES`); a PUT of another `Content-Type` or with `Content-Encoding:
//!   gzip` 415, no or a foreign `Origin` 403, `Sec-Fetch-Site: cross-site`
//!   403, no `Content-Length` 411, each before a byte is read and with no
//!   `.part`; `OPTIONS` no `Access-Control-*`; a login's third concurrent
//!   upload and the hub's ninth 429 with `Retry-After: 5`; a dropped upload
//!   frees its slot. M: the slot kept after a dropped body.
//! - RF19: while two 200 MiB PUTs stream to a hub, `GET .../cards` and
//!   `add_comment` each answer within 1 000 ms; the slow lines show each PUT
//!   holding the turn only for its rename. M: the body streamed inside the
//!   turn.
//! - RF20, the hub: `tools/list` 24 tools with the three, no delete; no
//!   schema holds `b3`, `file` or `as`; descriptions ASCII within 2 048 B,
//!   references called people's material; `add_reference` stores a link
//!   authored `{type: agent, role, model, run, login}`; `list_references`'
//!   content byte-equal to `spec ref list … 2>&1`. M: `as` added to
//!   `add_reference`.
//! - "Data": an unwritable store a 503 naming the fix while other routes
//!   go on; a bad `[references]` a 503 on the two writes alone.
//! - The review's findings (iteration 1): every route but a file's answers
//!   `Cache-Control: no-store` once, the events stream and the hub's `/mcp`
//!   among them, a file's GET its own `private, no-cache`; a stalled reader
//!   holds no thread past the runtime's keep-alive and stalls no write;
//!   concurrent starts cannot pass the quota, refused ones give their bytes
//!   back; the file count holds at commit; `PUT .../blobs?x=1` refused. A
//!   writer's panic answering 500 has no black-box seam (see the report).
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;
#[path = "../../specengine-cli/tests/work_refs_common/mod.rs"]
mod refs_common;

use std::ffi::OsString;
use std::fs;
use std::io::{Read, Write as _};
use std::net::{Shutdown, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use common::hub::{Hub, ORIGIN, P, Proxied, Setup, hub_args, through};
use common::mcp_http::{Stateless, content_text, is_error, result_of, tool, tool_names};
use common::slow::{requests_of, slow_ms};
use common::{Reply, Scratch, Server, data_dir, product_env, read_text, spec_json, write};
use refs_common::{
    BLOB_STORED_KEYS, MATERIAL, MATTERS_REQUIRED, REFERENCE_KEYS, WavStream, age, age_of,
    blob_expected, blob_fixture, file_equals, keys_in_order, needs_matters, parts, png,
    refused_line, store_files,
};
use serde_json::{Value, json};

/// The print shop's team: `olga` the owner, `lena` a lead, `tomas` and
/// `mira` members. `eve` is outside it.
const TEAM: &str = r#"
[zones]
sheets = ["docs/spec/**"]

[team.olga]
name = "Olga R"
role = "owner"
email = "olga@example.org"

[team.lena]
name = "Lena V"
role = "lead"
zones = ["sheets"]
email = "lena@example.org"

[team.tomas]
name = "Tomas B"
role = "member"
email = "tomas@example.org"

[team.mira]
name = "Mira K"
role = "member"
email = "mira@example.org"
"#;

/// A `[team]` no reader takes: a role off the three.
const BAD_TEAM: &str = r#"
[team.olga]
name = "Olga R"
role = "editor"
email = "olga@example.org"
"#;

const SLUG: &str = "card-press";
const MIB: u64 = 1024 * 1024;
const WARNING: &str = "a .blend is only downloaded here; Blender can run the scripts it holds: \
                       open it with Auto Run Python Scripts off";
const OCTETS: &str = "application/octet-stream";

// ------------------------------------------------------------- the scene

/// `table` at the end of `root`'s config, committed.
fn with_table(setup: &Setup, table: &str) {
    let mut text = read_text(&setup.root, "specengine.toml");
    text.push_str(table);
    write(&setup.root, "specengine.toml", text);
    setup
        .scratch
        .git()
        .run(&setup.root, &["commit", "-qam", "a table"]);
}

/// A hub over card-press with `table` (none: no `[team]`).
fn hub(name: &str, table: Option<&str>) -> Hub {
    let setup = Setup::new(name, "card-press");
    if let Some(table) = table {
        with_table(&setup, table);
    }
    setup.start()
}

/// A hub over card-press with `TEAM` and `envs` added to the daemon's
/// environment (the three settings), `--slow-ms` when `slow`.
fn hub_with(name: &str, envs: &[(&str, &str)], slow: bool) -> Hub {
    let setup = Setup::new(name, "card-press");
    with_table(&setup, TEAM);
    let mut env = product_env(&setup.home);
    env.extend(
        envs.iter()
            .map(|(key, value)| (OsString::from(key), OsString::from(value))),
    );
    let args = hub_args(&[&setup.root], ORIGIN);
    let args = if slow { slow_ms(args, "0") } else { args };
    let server = Server::try_start_env(&env, &setup.cwd, &args)
        .unwrap_or_else(|refused| panic!("the hub did not start: {refused:?}"));
    Hub { server, setup }
}

/// A `plates` card made by `ana`: its ID.
fn card(hub: &Hub, title: &str) -> String {
    let made = hub.done(
        "ana",
        "cards",
        &json!({"title": title, "discipline": "plates", "target": ["SHT-FRONT"]}),
    );
    made["id"].as_str().expect("a card's ID").to_owned()
}

fn id_of(document: &Value) -> String {
    document["id"]
        .as_str()
        .unwrap_or_else(|| panic!("an ID: {document}"))
        .to_owned()
}

/// The hub's blob store.
fn store_of(hub: &Hub) -> PathBuf {
    data_dir(&hub.setup.home).join("blobs").join(SLUG)
}

fn sqlite(db: &Path, sql: &str) -> String {
    let client = ["/usr/bin/sqlite3", "/bin/sqlite3", "/usr/local/bin/sqlite3"]
        .into_iter()
        .find(|candidate| Path::new(candidate).exists())
        .expect("a sqlite3 client");
    // A daemon's write (a delete's WAL truncate) holds the file a moment:
    // the client waits for it rather than failing `database is locked`.
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

/// What a hub keeps of references: the rows, the events, the stored files,
/// the `.part` files.
type Kept = (String, Vec<(String, Value)>, Vec<String>, Vec<String>);

/// The reference rows, the events, the store's files and `.part`s: what a
/// refusal must leave.
fn kept(hub: &Hub) -> Kept {
    let db = data_dir(&hub.setup.home).join(format!("{SLUG}.db"));
    let rows = if db.exists() {
        sqlite(&db, "SELECT * FROM refs ORDER BY id;")
    } else {
        String::new()
    };
    (
        rows,
        hub.events(),
        store_files(&store_of(hub)),
        parts(&store_of(hub)),
    )
}

// ---------------------------------------------------- the binary client

/// A page's upload headers through the proxy: its `Origin` and
/// `application/octet-stream`.
fn uploading(login: &str) -> Proxied {
    Proxied::fetch(login)
        .plus("Origin", ORIGIN)
        .plus("Content-Type", OCTETS)
}

/// `PUT <P>/blobs` of `bytes` by `login`, the answer.
fn put(hub: &Hub, login: &str, bytes: &[u8]) -> Reply {
    through(
        &hub.server,
        "PUT",
        &format!("{P}/blobs"),
        &uploading(login),
        bytes,
    )
}

/// [`put`], a 200: the stored blob.
fn stored(hub: &Hub, login: &str, bytes: &[u8]) -> Value {
    let reply = put(hub, login, bytes);
    assert_eq!(reply.status, 200, "PUT by {login}: {}", reply.text());
    reply.json()
}

/// `POST <P>/references` of `body` by `login`.
fn attach(hub: &Hub, login: &str, body: &Value) -> Reply {
    hub.post(login, "references", body)
}

/// [`attach`], a 200: the reference.
fn attached(hub: &Hub, login: &str, body: &Value) -> Value {
    let reply = attach(hub, login, body);
    assert_eq!(reply.status, 200, "{login} {body}: {}", reply.text());
    reply.json()
}

/// `DELETE <P>/references/<id>` by `login`, no body.
fn delete(hub: &Hub, login: &str, id: &str) -> Reply {
    through(
        &hub.server,
        "DELETE",
        &format!("{P}/references/{id}"),
        &Proxied::fetch(login).plus("Origin", ORIGIN),
        b"",
    )
}

/// The fixture `name` put and attached to `on` by `login` with `extra`
/// keys: the reference.
fn fixture_ref(hub: &Hub, login: &str, on: &str, name: &str, extra: Value) -> Value {
    let bytes = fs::read(blob_fixture(name)).expect("a fixture");
    let blob = stored(hub, login, &bytes);
    let mut body = json!({"on": on, "b3": blob["b3"], "name": name});
    if let (Some(body), Some(extra)) = (body.as_object_mut(), extra.as_object()) {
        body.extend(extra.clone());
    }
    attached(hub, login, &body)
}

/// One answer's head and the rest of its socket.
struct Head {
    status: u16,
    headers: Vec<(String, String)>,
    /// Body bytes read with the head.
    early: Vec<u8>,
    stream: TcpStream,
}

impl Head {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    /// The whole answer, its body read to the end and de-chunked.
    fn reply(mut self) -> Reply {
        let mut body = std::mem::take(&mut self.early);
        let _ = self.stream.set_read_timeout(Some(Duration::from_secs(120)));
        let _ = self.stream.read_to_end(&mut body);
        let chunked = self
            .header("transfer-encoding")
            .is_some_and(|value| value.contains("chunked"));
        Reply {
            status: self.status,
            headers: self.headers,
            body: if chunked { dechunk(&body) } else { body },
        }
    }

    /// The body streamed to `sink` in pieces as it arrives; its length.
    fn drain(mut self, mut sink: impl FnMut(&[u8])) -> u64 {
        let chunked = self
            .header("transfer-encoding")
            .is_some_and(|value| value.contains("chunked"));
        assert!(!chunked, "a file's answer streams with a Content-Length");
        let mut total = self.early.len() as u64;
        sink(&self.early);
        let _ = self.stream.set_read_timeout(Some(Duration::from_secs(120)));
        let mut buffer = vec![0u8; 65_536];
        loop {
            match self.stream.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => {
                    sink(&buffer[..n]);
                    total += n as u64;
                }
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                Err(error) => panic!("read the file: {error}"),
            }
        }
        total
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn dechunk(mut rest: &[u8]) -> Vec<u8> {
    let mut body = Vec::new();
    while let Some(end) = find(rest, b"\r\n") {
        let size_text = std::str::from_utf8(&rest[..end]).expect("chunk size");
        let size = usize::from_str_radix(size_text.split(';').next().unwrap_or("0").trim(), 16)
            .expect("hex chunk size");
        rest = &rest[end + 2..];
        if size == 0 {
            break;
        }
        body.extend_from_slice(&rest[..size]);
        rest = &rest[size + 2..];
    }
    body
}

/// Reads one answer's head off `stream`, which must come within `within`.
fn read_head(mut stream: TcpStream, within: Duration) -> Head {
    stream
        .set_read_timeout(Some(Duration::from_millis(50)))
        .expect("timeout");
    let started = Instant::now();
    let mut raw = Vec::new();
    let mut buffer = [0u8; 65_536];
    loop {
        if let Some(end) = find(&raw, b"\r\n\r\n") {
            let head = std::str::from_utf8(&raw[..end]).expect("an ASCII head");
            let mut lines = head.split("\r\n");
            let status_line = lines.next().expect("a status line");
            let status = status_line
                .split(' ')
                .nth(1)
                .and_then(|code| code.parse().ok())
                .unwrap_or_else(|| panic!("a status line: {status_line:?}"));
            let headers = lines
                .map(|line| {
                    let (name, value) = line.split_once(':').expect("a header line");
                    (name.trim().to_ascii_lowercase(), value.trim().to_owned())
                })
                .collect();
            return Head {
                status,
                headers,
                early: raw[end + 4..].to_vec(),
                stream,
            };
        }
        assert!(
            started.elapsed() < within,
            "no answer's head within {within:?}: {:?}",
            String::from_utf8_lossy(&raw)
        );
        match stream.read(&mut buffer) {
            Ok(0) => panic!("closed before a head: {:?}", String::from_utf8_lossy(&raw)),
            Ok(n) => raw.extend_from_slice(&buffer[..n]),
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(error) => panic!("read a head: {error}"),
        }
    }
}

/// `method path` through the proxy with `proxied`'s headers and a
/// `Content-Length` of `length` when given, its head alone sent.
fn send_head(
    server: &Server,
    method: &str,
    path: &str,
    proxied: &Proxied,
    length: Option<u64>,
) -> TcpStream {
    let mut headers = proxied.pairs();
    let length = length.map(|length| length.to_string());
    if let Some(length) = &length {
        headers.push(("Content-Length", length.as_str()));
    }
    let bytes = common::request_bytes(method, path, &headers);
    let mut stream = TcpStream::connect(("127.0.0.1", server.port)).expect("connect");
    stream.write_all(&bytes).expect("write the head");
    stream
}

/// An upload started with `Expect: 100-continue`: `Ok` the socket once the
/// daemon asks for the body (its door, slots, caps and quota passed, its
/// flight and `.part` taken), `Err` the final answer it sent instead of the
/// `100` (review iteration 3: at once, the body unread, `Connection: close`).
fn begin_upload(hub: &Hub, login: &str, length: u64) -> Result<TcpStream, Reply> {
    let stream = send_head(
        &hub.server,
        "PUT",
        &format!("{P}/blobs"),
        &uploading(login).plus("Expect", "100-continue"),
        Some(length),
    );
    let head = read_head(stream, Duration::from_secs(30));
    if head.status == 100 {
        let stream = head.stream;
        // A 100 is a head of its own: what followed it is the next answer's.
        assert!(head.early.is_empty(), "bytes after a 100: {:?}", head.early);
        stream.set_nodelay(true).expect("nodelay");
        Ok(stream)
    } else {
        assert_eq!(
            head.header("connection"),
            Some("close"),
            "a refused start: {}",
            head.status
        );
        Err(head.reply())
    }
}

/// Writes `reader`'s bytes to `stream` in 64 KiB pieces, at most `rate`
/// bytes a second when given.
fn send_body(stream: &mut TcpStream, mut reader: impl Read, rate: Option<u64>) {
    let mut buffer = vec![0u8; 65_536];
    let started = Instant::now();
    let mut sent = 0u64;
    loop {
        let n = reader.read(&mut buffer).expect("generate");
        if n == 0 {
            break;
        }
        stream.write_all(&buffer[..n]).expect("send a piece");
        sent += n as u64;
        if let Some(rate) = rate {
            let due = Duration::from_secs_f64(sent as f64 / rate as f64);
            if let Some(wait) = due.checked_sub(started.elapsed()) {
                thread::sleep(wait);
            }
        }
    }
}

/// A whole upload of `reader` (`length` bytes) by `login`, streamed: the
/// answer.
fn upload(hub: &Hub, login: &str, length: u64, reader: impl Read, rate: Option<u64>) -> Reply {
    match begin_upload(hub, login, length) {
        Ok(mut stream) => {
            send_body(&mut stream, reader, rate);
            read_head(stream, Duration::from_secs(120)).reply()
        }
        Err(reply) => reply,
    }
}

/// `GET <P>/blobs/<b3>` by `login` with `extra` headers: its head.
fn get_blob(hub: &Hub, login: &str, b3: &str, extra: &[(&str, &str)]) -> Head {
    let mut proxied = Proxied::fetch(login);
    for (name, value) in extra {
        proxied = proxied.plus(name, value);
    }
    let stream = send_head(
        &hub.server,
        "GET",
        &format!("{P}/blobs/{b3}"),
        &proxied,
        None,
    );
    read_head(stream, Duration::from_secs(60))
}

/// The daemon's resident set in KiB (`ps`).
fn rss_kib(pid: u32) -> u64 {
    let output = std::process::Command::new("/bin/ps")
        .args(["-o", "rss=", "-p", &pid.to_string()])
        .output()
        .expect("ps runs");
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .unwrap_or_else(|_| panic!("ps rss of {pid}: {:?}", output))
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

/// The daemon's resident set at rest: the median of five readings.
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

// ------------------------------------------------------------------ RF2

/// RF2, the route: `[ids]` claiming `RF`: `PUT .../blobs`, `POST
/// .../references` and the list answer 503 with "Data"'s line; nothing
/// stored.
#[test]
fn rf2_ids_claiming_rf_is_a_503_on_the_reference_routes() {
    let setup = Setup::new("rf-hub-rf2", "card-press");
    let config = read_text(&setup.root, "specengine.toml");
    let config = if config.contains("[ids]") {
        config.replacen("[ids]\n", "[ids]\nRF = { kind = \"rule\", width = 4 }\n", 1)
    } else {
        format!("{config}\n[ids]\nRF = {{ kind = \"rule\", width = 4 }}\n")
    };
    write(&setup.root, "specengine.toml", config);
    setup
        .scratch
        .git()
        .run(&setup.root, &["commit", "-qam", "RF claimed"]);
    let hub = setup.start();
    let c1 = card(&hub, "Front page");
    let before = kept(&hub);
    let line = "[ids] claims RF, which names references here: rename that prefix";
    let bytes = fs::read(blob_fixture("lantern.gif")).expect("gif");
    for reply in [
        put(&hub, "mira", &bytes),
        attach(
            &hub,
            "mira",
            &json!({"on": c1, "url": "https://example.org/a"}),
        ),
        hub.get("mira", &format!("{P}/references?on={c1}")),
    ] {
        assert_eq!(reply.status, 503, "{}", reply.text());
        assert!(reply.text().contains(line), "{}", reply.text());
    }
    assert_eq!(kept(&hub), before);
}

// ------------------------------------------------------------------ RF3

/// RF3 on a hub: each stored fixture's `BlobStored`, its 10 keys in order,
/// `stored: new`; each refused one a 415 with its line, nothing left. M:
/// the type taken from the name's extension.
#[test]
fn rf3_the_hub_stores_by_sniffed_bytes_alone() {
    let hub = hub("rf-hub-rf3", Some(TEAM));
    let expected = blob_expected();
    for (name, want) in expected["stored"].as_object().expect("stored") {
        let bytes = fs::read(blob_fixture(name)).expect("a fixture");
        let reply = put(&hub, "mira", &bytes);
        assert_eq!(reply.status, 200, "{name}: {}", reply.text());
        assert_eq!(
            keys_in_order(reply.text()),
            BLOB_STORED_KEYS,
            "{name}: {}",
            reply.text()
        );
        let blob = reply.json();
        for key in [
            "media",
            "class",
            "inline",
            "meta",
            "size",
            "agent_ready",
            "needs_matters",
            "warning",
        ] {
            assert_eq!(blob[key], want[key], "{name}: {key}");
        }
        assert_eq!(blob["stored"], json!("new"), "{name}");
        let file = store_of(&hub).join(blob["b3"].as_str().expect("a b3"));
        assert_eq!(
            fs::read(&file).expect("stored"),
            bytes,
            "{name}: named by its b3"
        );
    }
    assert_eq!(parts(&store_of(&hub)), Vec::<String>::new());
    let before = kept(&hub);
    for name in expected["refused"].as_object().expect("refused").keys() {
        let bytes = fs::read(blob_fixture(name)).expect("a fixture");
        let reply = put(&hub, "mira", &bytes);
        assert_eq!(reply.status, 415, "{name}: {}", reply.text());
        // The PUT names no file: its line names the upload.
        let line = refused_line(&expected, name);
        let tail = line.split_once(": ").map(|(_, rest)| rest).expect("a line");
        let tail = tail.split_once(": ").map_or(tail, |(_, rest)| rest);
        // A PUT carries no name: no `.blend` hint.
        let tail = tail.trim_end_matches("; save the .blend uncompressed");
        assert!(
            reply.text().contains(tail),
            "{name}: {tail} in {}",
            reply.text()
        );
        assert_eq!(kept(&hub), before, "{name}: nothing left");
    }
}

// ------------------------------------------------------------------ RF4

/// RF4: with `SPECENGINE_BLOB_FILE_MAX=1048576` a PUT declaring 1 048 577
/// bytes is a 413 answered before the body is sent, no `.part` created; at
/// the cap it stores. No refusal stores or emits anything.
#[test]
fn rf4_the_file_cap_judges_before_reading() {
    let hub = hub_with(
        "rf-hub-rf4-cap",
        &[("SPECENGINE_BLOB_FILE_MAX", "1048576")],
        false,
    );
    let before = kept(&hub);
    match begin_upload(&hub, "mira", MIB + 1) {
        Ok(_) => panic!("a body over the cap was asked for"),
        Err(reply) => {
            assert_eq!(reply.status, 413, "{}", reply.text());
            assert!(
                reply
                    .text()
                    .contains("is 1.0 MiB; a file here is at most 1 MiB"),
                "{}",
                reply.text()
            );
        }
    }
    assert!(parts(&store_of(&hub)).is_empty(), "no .part");
    assert_eq!(kept(&hub), before);
    let at_cap = upload(&hub, "mira", MIB, WavStream::new(MIB, 2), None);
    assert_eq!(at_cap.status, 200, "{}", at_cap.text());
}

/// The rest of `head`'s answer, the connection then closed by the daemon
/// (EOF within 10 s, not a read left waiting): its body's text.
fn closed_after(head: Head) -> String {
    let chunked = head
        .header("transfer-encoding")
        .is_some_and(|value| value.contains("chunked"));
    let Head {
        early, mut stream, ..
    } = head;
    let mut body = early;
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("timeout");
    if let Err(error) = stream.read_to_end(&mut body) {
        panic!("the daemon kept the connection open after `Connection: close`: {error}");
    }
    let body = if chunked { dechunk(&body) } else { body };
    String::from_utf8_lossy(&body).into_owned()
}

/// The review of work-references (iteration 3), an early answer: a start
/// refused before its body (here the file cap) and asked with `Expect:
/// 100-continue` gets its final status instead of the `100`, with
/// `Connection: close`, and the connection closes with no byte of the body
/// sent or read; no `.part`, nothing kept. M: an unpolled body drained (the
/// `100` sent, then the refusal).
#[test]
fn an_expected_refusal_is_the_final_status_at_once() {
    let hub = hub_with(
        "rf-hub-early",
        &[("SPECENGINE_BLOB_FILE_MAX", "1048576")],
        false,
    );
    let before = kept(&hub);
    for _ in 0..2 {
        let stream = send_head(
            &hub.server,
            "PUT",
            &format!("{P}/blobs"),
            &uploading("mira").plus("Expect", "100-continue"),
            Some(MIB + 1),
        );
        let head = read_head(stream, Duration::from_secs(30));
        assert_eq!(head.status, 413, "the final status, not a 100");
        assert_eq!(head.header("connection"), Some("close"));
        let text = closed_after(head);
        assert!(text.contains("a file here is at most 1 MiB"), "{text}");
        assert!(parts(&store_of(&hub)).is_empty(), "no .part");
    }
    assert_eq!(kept(&hub), before);
    // The refused starts held no slot: mira's two still start.
    let first = begin_upload(&hub, "mira", 1024).expect("a slot");
    let second = begin_upload(&hub, "mira", 1024).expect("a slot");
    drop((first, second));
}

/// The review of work-references (iteration 3), a refusal after the body
/// began: a PUT without `Expect` whose first bytes are an SVG, its sender
/// stalled after 256 KiB of a declared 8 MiB, gets its 415 from the bounded
/// drain (2 s at most, well before the 60 s idle cut), with `Connection:
/// close`, the connection then closed; no `.part`, nothing kept, mira's
/// slots free again. M: the drain unbounded in time.
#[test]
fn a_refusal_after_the_body_began_drains_a_bounded_part() {
    let hub = hub("rf-hub-drain", Some(TEAM));
    let before = kept(&hub);
    let mut body = fs::read(blob_fixture("map.svg")).expect("the SVG");
    body.resize(256 * 1024, b' ');
    let mut stream = send_head(
        &hub.server,
        "PUT",
        &format!("{P}/blobs"),
        &uploading("mira"),
        Some(8 * MIB),
    );
    stream.write_all(&body).expect("the first 256 KiB");
    let sent = Instant::now();
    // The sender stalls with 7.75 MiB still declared.
    let head = read_head(stream, Duration::from_secs(15));
    let waited = sent.elapsed();
    assert_eq!(head.status, 415, "the type refused after its first bytes");
    assert_eq!(head.header("connection"), Some("close"));
    assert!(
        waited < Duration::from_secs(15),
        "answered after {waited:?}"
    );
    closed_after(head);
    let started = Instant::now();
    while !parts(&store_of(&hub)).is_empty() && started.elapsed() < Duration::from_secs(10) {
        thread::sleep(Duration::from_millis(50));
    }
    assert!(parts(&store_of(&hub)).is_empty(), "no .part");
    assert_eq!(kept(&hub), before);
    let first = begin_upload(&hub, "mira", 1024).expect("a slot");
    let second = begin_upload(&hub, "mira", 1024).expect("a slot");
    drop((first, second));
}

/// RF4: with `SPECENGINE_BLOB_QUOTA=3145728` and 2 MiB stored a 2 MiB PUT is
/// a 413 with "Data"'s line, nothing stored, no event (judged on the
/// declared size at the upload's start, equal bytes or not); 1 MiB more
/// stores.
#[test]
fn rf4_the_quota_counts_what_is_stored() {
    let hub = hub_with(
        "rf-hub-rf4-quota",
        &[("SPECENGINE_BLOB_QUOTA", "3145728")],
        false,
    );
    let first = upload(&hub, "mira", 2 * MIB, WavStream::new(2 * MIB, 1), None);
    assert_eq!(first.status, 200, "{}", first.text());
    let before = kept(&hub);
    let over = upload(&hub, "tomas", 2 * MIB, WavStream::new(2 * MIB, 4), None);
    assert_eq!(over.status, 413, "{}", over.text());
    assert!(
        over.text().contains(
            "refused: this project's files would pass 3 MiB (2.0 MiB stored); delete some first"
        ),
        "{}",
        over.text()
    );
    assert_eq!(kept(&hub), before, "nothing stored, no event");
    let fits = upload(&hub, "tomas", MIB, WavStream::new(MIB, 5), None);
    assert_eq!(fits.status, 200, "{}", fits.text());
}

/// RF4: a setting that would widen a cap exits 2 at the daemon's start,
/// naming it.
#[test]
fn rf4_a_widening_setting_stops_the_daemon() {
    for (name, value) in [
        ("SPECENGINE_BLOB_QUOTA", "20000000000"),
        ("SPECENGINE_BLOB_FLOOR", "1"),
        ("SPECENGINE_BLOB_FILE_MAX", "268435457"),
    ] {
        let setup = Setup::new("rf-hub-rf4-start", "card-press");
        let mut env = product_env(&setup.home);
        env.push((OsString::from(name), OsString::from(value)));
        match Server::try_start_env(&env, &setup.cwd, &hub_args(&[&setup.root], ORIGIN)) {
            Ok(_) => panic!("{name}={value}: the daemon started"),
            Err(refused) => {
                assert_eq!(refused.code, Some(2), "{name}: {refused:?}");
                assert!(
                    refused.stderr.contains(&format!("{name}: {value}")),
                    "{name}: {refused:?}"
                );
            }
        }
    }
}

/// RF4: the store's file count: with 20 000 files a PUT of new bytes is a
/// 413 with "Data"'s line; equal bytes of a stored file still store.
#[test]
fn rf4_the_20_001st_file_is_refused() {
    let hub = hub("rf-hub-rf4-files", Some(TEAM));
    let gif = fs::read(blob_fixture("lantern.gif")).expect("gif");
    let first = stored(&hub, "mira", &gif);
    let store = store_of(&hub);
    // 19 999 more files, named as the store names them.
    for n in 0..19_999u64 {
        fs::write(store.join(format!("{n:064x}")), b"x").expect("plant a file");
    }
    assert_eq!(store_files(&store).len(), 20_000);
    let reply = put(
        &hub,
        "mira",
        &fs::read(blob_fixture("lantern.gif")).expect("gif"),
    );
    assert_eq!(
        reply.status,
        200,
        "equal bytes are no new file: {}",
        reply.text()
    );
    assert_eq!(reply.json()["b3"], first["b3"]);
    let before = kept(&hub);
    let reply = put(
        &hub,
        "mira",
        &fs::read(blob_fixture("sea-lossy.webp")).expect("webp"),
    );
    assert_eq!(reply.status, 413, "{}", reply.text());
    assert!(
        reply
            .text()
            .contains("refused: this project stores 20 000 files; delete some first"),
        "{}",
        reply.text()
    );
    assert_eq!(kept(&hub), before);
}

// ------------------------------------------------------------------ RF5

/// RF5: one 200 MiB PUT, its body a WAV generated as it is sent, raises the
/// daemon's RSS, sampled every 50 ms, at most 16 MiB over its value at
/// rest; the stored file is the generator's bytes, named by the answered
/// BLAKE3, and attaches. M: the body collected whole before hashing.
#[test]
fn rf5_a_200_mib_upload_keeps_its_buffer() {
    let hub = hub("rf-hub-rf5", Some(TEAM));
    let c1 = card(&hub, "Front page");
    // Warm the routes once.
    stored(
        &hub,
        "mira",
        &fs::read(blob_fixture("lantern.gif")).expect("gif"),
    );
    let pid = hub.server.pid();
    let rest = rss_at_rest(pid);
    let sampler = Sampler::start(pid);
    let total = 200 * MIB;
    let reply = upload(&hub, "mira", total, WavStream::new(total, 77), None);
    let peak = sampler.stop();
    assert_eq!(reply.status, 200, "{}", reply.text());
    let blob = reply.json();
    assert_eq!(blob["size"], json!(total));
    assert_eq!(blob["media"], json!("audio/wav"));
    assert_eq!(
        blob["meta"]["duration_ms"],
        json!((total - 44) * 1000 / 192_000)
    );
    let rise = peak.saturating_sub(rest);
    assert!(
        rise <= 16 * 1024,
        "M: the body held whole: RSS {rest} KiB at rest, {peak} KiB at its peak (+{rise} KiB)"
    );
    let b3 = blob["b3"].as_str().expect("a b3");
    let file = store_of(&hub).join(b3);
    assert!(
        file_equals(&file, WavStream::new(total, 77)),
        "the stored file is the generator's bytes"
    );
    let made = attached(
        &hub,
        "mira",
        &json!({"on": c1, "b3": b3, "name": "harbour-hum.wav", "matters": "the low hum"}),
    );
    assert_eq!(made["size"], json!(total));
}

// ------------------------------------------------------------------ RF6

/// RF6 on a hub: two PUTs of equal bytes answer `new` then `existing`, one
/// file whose mtime is the second's; two references share it; deleting one
/// keeps it, the other removes it in that call.
#[test]
fn rf6_equal_bytes_one_file() {
    let hub = hub("rf-hub-rf6", Some(TEAM));
    let c1 = card(&hub, "Front page");
    let c2 = card(&hub, "Back page");
    let bytes = fs::read(blob_fixture("harbour.png")).expect("png");
    let first = stored(&hub, "mira", &bytes);
    assert_eq!(first["stored"], json!("new"));
    let b3 = first["b3"].as_str().expect("a b3").to_owned();
    let file = store_of(&hub).join(&b3);
    age(&file, 7200);
    let second = stored(&hub, "tomas", &bytes);
    assert_eq!(second["stored"], json!("existing"));
    assert_eq!(second["b3"], json!(b3));
    assert_eq!(store_files(&store_of(&hub)), std::slice::from_ref(&b3));
    assert!(
        age_of(&file) < 120,
        "the second write's mtime: {}",
        age_of(&file)
    );
    let one = attached(&hub, "mira", &json!({"on": c1, "b3": b3, "name": "a.png"}));
    let two = attached(&hub, "tomas", &json!({"on": c2, "b3": b3, "name": "b.png"}));
    assert_eq!(delete(&hub, "mira", &id_of(&one)).status, 200);
    assert!(file.is_file());
    assert_eq!(get_blob(&hub, "lev", &b3, &[]).status, 200, "still served");
    assert_eq!(delete(&hub, "tomas", &id_of(&two)).status, 200);
    assert!(!file.exists(), "gone in that call");
    assert_eq!(get_blob(&hub, "lev", &b3, &[]).status, 404);
}

// ------------------------------------------------------------------ RF7

/// "Serving": `filename="<ASCII, others _>"; filename*=UTF-8''<encoded>`.
fn disposition(kind: &str, name: &str) -> (String, String) {
    let ascii: String = name
        .chars()
        .map(|c| {
            if c.is_ascii() && c != '"' && c != '\\' && !c.is_ascii_control() {
                c
            } else {
                '_'
            }
        })
        .collect();
    let mut encoded = String::new();
    for byte in name.bytes() {
        if byte.is_ascii_alphanumeric() || b"!#$&+-.^_`|~".contains(&byte) {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    (
        format!("{kind}; filename=\"{ascii}\""),
        format!("filename*=UTF-8''{encoded}"),
    )
}

/// RF7: the serving headers of a PNG (inline), a GLB and a PDF
/// (attachment, `filename` and `filename*` from the stored name); the body
/// the file's bytes; `If-None-Match` equal a 304; a hash that no live
/// reference of the project names a 404: unattached, deleted, another
/// project's, not 64 lower-case hex. M: `inline` for a PDF.
#[test]
fn rf7_files_are_served_sandboxed() {
    let hub = hub("rf-hub-rf7", Some(TEAM));
    let c1 = card(&hub, "Front page");
    for (fixture, name, kind, media) in [
        ("harbour.png", "harbour.png", "inline", "image/png"),
        (
            "crate.glb",
            "crate model.glb",
            "attachment",
            "model/gltf-binary",
        ),
        (
            "plan.pdf",
            "plan d\u{e9}p\u{f4}t.pdf",
            "attachment",
            "application/pdf",
        ),
        ("sound.wav", "hum.wav", "inline", "audio/wav"),
    ] {
        let bytes = fs::read(blob_fixture(fixture)).expect("a fixture");
        let blob = stored(&hub, "mira", &bytes);
        let b3 = blob["b3"].as_str().expect("a b3").to_owned();
        assert_eq!(
            get_blob(&hub, "lev", &b3, &[]).status,
            404,
            "{fixture}: an unattached file is no reference's"
        );
        attached(
            &hub,
            "mira",
            &json!({"on": c1, "b3": b3, "name": name, "matters": "the shape of it"}),
        );
        let reply = get_blob(&hub, "lev", &b3, &[]).reply();
        assert_eq!(reply.status, 200, "{fixture}");
        assert_eq!(reply.body, bytes, "{fixture}: the bytes");
        assert_eq!(reply.header("content-type"), Some(media), "{fixture}");
        assert_eq!(reply.header("x-content-type-options"), Some("nosniff"));
        assert_eq!(reply.header("content-security-policy"), Some("sandbox"));
        assert_eq!(reply.header("cache-control"), Some("private, no-cache"));
        assert_eq!(reply.header("etag"), Some(format!("\"{b3}\"").as_str()));
        assert_eq!(reply.header("accept-ranges"), Some("bytes"));
        assert_eq!(
            reply.header("content-length"),
            Some(bytes.len().to_string().as_str())
        );
        let (head, star) = disposition(kind, name);
        let got = reply.header("content-disposition").expect("a disposition");
        assert!(
            got.starts_with(&head) && got.contains(&star),
            "{fixture}: {got:?}, not {head:?} … {star:?}. M: inline for a PDF"
        );
        let cached = get_blob(&hub, "lev", &b3, &[("If-None-Match", &format!("\"{b3}\""))]).reply();
        assert_eq!(cached.status, 304, "{fixture}");
        assert!(cached.body.is_empty());
    }
    // Deleted: its file collected and its hash no reference's.
    let gif = fixture_ref(&hub, "mira", &c1, "lantern.gif", json!({}));
    let b3 = gif["b3"].as_str().expect("a b3").to_owned();
    assert_eq!(get_blob(&hub, "lev", &b3, &[]).status, 200);
    assert_eq!(delete(&hub, "mira", &id_of(&gif)).status, 200);
    assert_eq!(get_blob(&hub, "lev", &b3, &[]).status, 404, "deleted");
    for bad in ["ABCDEF", "0123", &b3.to_uppercase()] {
        assert_eq!(get_blob(&hub, "lev", bad, &[]).status, 404, "{bad}");
    }
    // Another method on the route: the hub's fence.
    let post = through(
        &hub.server,
        "POST",
        &format!("{P}/blobs/{}", "0".repeat(64)),
        &Proxied::write("mira"),
        b"{}",
    );
    assert_eq!(post.status, 403, "{}", post.text());
}

/// RF7, "Serving": `GET .../blobs/:b3` takes no turn: while a probe holds
/// the project's turn for 3 s (`probe_hold` over the hub's `/mcp`), the
/// file answers whole well before the hold ends. Run on the `probes` build:
/// `cargo nextest run -p specengine-http --features specengine-mcp/probes
/// --test work_references --run-ignored ignored-only`.
#[test]
#[ignore = "probes build: cargo nextest run -p specengine-http --features specengine-mcp/probes \
            --test work_references --run-ignored ignored-only"]
fn rf7_a_file_answers_while_the_turn_is_held() {
    let hub = hub("rf-hub-rf7-held", Some(TEAM));
    let c1 = card(&hub, "Front page");
    let made = fixture_ref(&hub, "mira", &c1, "harbour.png", json!({}));
    let b3 = made["b3"].as_str().expect("b3").to_owned();
    let port = hub.server.port;
    let holder = thread::spawn(move || {
        let mut agent = Stateless::new(port, SLUG, "lev");
        let started = Instant::now();
        let answer = agent.call("probe_hold", json!({"ms": 3000, "tag": "rf7"}));
        (answer, started.elapsed())
    });
    // The hold is taken: its `held` line, or a short wait for the call.
    thread::sleep(Duration::from_millis(500));
    let started = Instant::now();
    let reply = get_blob(&hub, "olga", &b3, &[]).reply();
    let took = started.elapsed();
    let (answer, held) = holder.join().expect("the holder");
    assert!(
        answer.get("result").is_some(),
        "probe_hold answered: {answer}"
    );
    assert_eq!(reply.status, 200);
    assert_eq!(
        reply.body,
        fs::read(blob_fixture("harbour.png")).expect("png")
    );
    assert!(
        took < Duration::from_millis(1500) && held >= Duration::from_millis(2900),
        "the GET waited for the turn: {took:?} (the hold {held:?})"
    );
}

/// RF7: another project's file is no reference of this one: two projects
/// on one daemon, a file attached in the first, its hash a 404 in the
/// second.
#[test]
fn rf7_another_projects_file_is_a_404() {
    let scratch = Scratch::new("rf-local-rf7-two");
    let first = scratch.repo("card-press", "first", "main");
    let second = scratch.repo("trace-almanac", "second", "main");
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let server = Server::serve(&home, &cwd, &[&first, &second]);
    let own = format!("http://127.0.0.1:{}", server.port);
    let send = |method: &str, path: &str, kind: Option<&str>, body: &[u8]| -> Reply {
        let mut headers = vec![("Sec-Fetch-Site", "same-origin"), ("Origin", own.as_str())];
        if let Some(kind) = kind {
            headers.push(("Content-Type", kind));
        }
        server.send(method, path, &headers, body)
    };
    let made = send(
        "POST",
        "/api/projects/card-press/cards",
        Some("application/json"),
        json!({"title": "Front page", "discipline": "plates"})
            .to_string()
            .as_bytes(),
    );
    assert_eq!(made.status, 200, "{}", made.text());
    let c1 = id_of(&made.json());
    let bytes = fs::read(blob_fixture("lantern.gif")).expect("gif");
    let blob = send(
        "PUT",
        "/api/projects/card-press/blobs",
        Some(OCTETS),
        &bytes,
    );
    assert_eq!(blob.status, 200, "{}", blob.text());
    let b3 = blob.json()["b3"].as_str().expect("a b3").to_owned();
    let reference = send(
        "POST",
        "/api/projects/card-press/references",
        Some("application/json"),
        json!({"on": c1, "b3": b3, "name": "lamp.gif", "as": "mira"})
            .to_string()
            .as_bytes(),
    );
    assert_eq!(reference.status, 200, "{}", reference.text());
    assert_eq!(reference.json()["actor"], json!("mira"), "a local `as`");
    assert_eq!(
        server
            .get(&format!("/api/projects/card-press/blobs/{b3}"))
            .status,
        200
    );
    assert_eq!(
        server
            .get(&format!("/api/projects/trace-almanac/blobs/{b3}"))
            .status,
        404,
        "another project's file"
    );
    // Another method on a local daemon: a 405.
    let deleted = send(
        "DELETE",
        &format!("/api/projects/card-press/blobs/{b3}"),
        None,
        b"",
    );
    assert_eq!(deleted.status, 405, "{}", deleted.text());
}

// ------------------------------------------------------------------ RF8

/// A 3 MiB WAV attached on a hub: the hub, its bytes and its hash.
fn three_mib(name: &str) -> (Hub, Vec<u8>, String) {
    let hub = hub(name, Some(TEAM));
    let c1 = card(&hub, "Front page");
    let mut bytes = Vec::new();
    WavStream::new(3 * MIB, 3)
        .read_to_end(&mut bytes)
        .expect("generate");
    let blob = stored(&hub, "mira", &bytes);
    let b3 = blob["b3"].as_str().expect("a b3").to_owned();
    attached(
        &hub,
        "mira",
        &json!({"on": c1, "b3": b3, "name": "tide.wav", "matters": "the tide"}),
    );
    (hub, bytes, b3)
}

/// RF8: one range is a 206 with `Content-Range` and exactly its bytes; a
/// suffix the last bytes; an unsatisfiable one a 416 `bytes */size`; two
/// ranges or a malformed header the whole file, 200; `If-Range` equal to
/// the ETag honoured, another the whole file. M: the file read whole into
/// the answer.
#[test]
fn rf8_one_range_is_served() {
    let (hub, bytes, b3) = three_mib("rf-hub-rf8");
    let size = bytes.len();
    assert_eq!(size, 3_145_728);
    let ranged = get_blob(&hub, "lev", &b3, &[("Range", "bytes=1048576-2097151")]).reply();
    assert_eq!(ranged.status, 206);
    assert_eq!(
        ranged.header("content-range"),
        Some("bytes 1048576-2097151/3145728")
    );
    assert_eq!(ranged.body, bytes[1_048_576..=2_097_151]);
    let tail = get_blob(&hub, "lev", &b3, &[("Range", "bytes=-100")]).reply();
    assert_eq!(tail.status, 206);
    assert_eq!(
        tail.header("content-range"),
        Some("bytes 3145628-3145727/3145728")
    );
    assert_eq!(tail.body, bytes[size - 100..]);
    let open = get_blob(&hub, "lev", &b3, &[("Range", "bytes=3145000-")]).reply();
    assert_eq!(open.status, 206);
    assert_eq!(
        open.header("content-range"),
        Some("bytes 3145000-3145727/3145728")
    );
    assert_eq!(open.body, bytes[3_145_000..]);
    let past = get_blob(&hub, "lev", &b3, &[("Range", "bytes=3145728-")]).reply();
    assert_eq!(past.status, 416);
    assert_eq!(past.header("content-range"), Some("bytes */3145728"));
    for whole in ["bytes=0-0,5-9", "bytes=abc", "chars=0-10", "bytes=10-5"] {
        let reply = get_blob(&hub, "lev", &b3, &[("Range", whole)]).reply();
        assert_eq!(reply.status, 200, "{whole}");
        assert_eq!(reply.body.len(), size, "{whole}: the whole file");
        assert_eq!(reply.body, bytes, "{whole}");
    }
    let honoured = get_blob(
        &hub,
        "lev",
        &b3,
        &[("Range", "bytes=0-9"), ("If-Range", &format!("\"{b3}\""))],
    )
    .reply();
    assert_eq!(honoured.status, 206);
    assert_eq!(honoured.body, bytes[..10]);
    let stale = get_blob(
        &hub,
        "lev",
        &b3,
        &[("Range", "bytes=0-9"), ("If-Range", "\"other\"")],
    )
    .reply();
    assert_eq!(stale.status, 200);
    assert_eq!(stale.body.len(), size);
}

/// RF8: ten concurrent readers of a 200 MiB file, each reading it whole as
/// it streams, raise the daemon's RSS at most 16 MiB. M: the file read
/// whole into the answer.
#[test]
fn rf8_ten_readers_of_200_mib_keep_their_pieces() {
    let hub = hub("rf-hub-rf8-mem", Some(TEAM));
    let c1 = card(&hub, "Front page");
    let total = 200 * MIB;
    let reply = upload(&hub, "mira", total, WavStream::new(total, 8), None);
    assert_eq!(reply.status, 200, "{}", reply.text());
    let b3 = reply.json()["b3"].as_str().expect("a b3").to_owned();
    attached(
        &hub,
        "mira",
        &json!({"on": c1, "b3": b3, "name": "long.wav", "matters": "the swell"}),
    );
    let pid = hub.server.pid();
    let rest = rss_at_rest(pid);
    let sampler = Sampler::start(pid);
    let readers: Vec<_> = (0..10)
        .map(|n| {
            let head = get_blob(&hub, &format!("reader-{n}"), &b3, &[]);
            assert_eq!(head.status, 200);
            thread::spawn(move || {
                let mut first = Vec::new();
                let length = head.drain(|piece| {
                    if first.len() < 44 {
                        first.extend_from_slice(&piece[..piece.len().min(44 - first.len())]);
                    }
                    // Read slowly enough that ten overlap.
                    thread::sleep(Duration::from_micros(200));
                });
                (length, first)
            })
        })
        .collect();
    for reader in readers {
        let (length, first) = reader.join().expect("a reader");
        assert_eq!(length, total);
        assert_eq!(first, refs_common::wav_header(total - 44));
    }
    let peak = sampler.stop();
    let rise = peak.saturating_sub(rest);
    assert!(
        rise <= 16 * 1024,
        "M: a file read whole: RSS {rest} KiB at rest, {peak} KiB at its peak (+{rise} KiB)"
    );
}

// ------------------------------------------------------------------ RF9

/// RF9 on a hub: a WAV, an MP4, a GLB and a PDF attached without "what
/// matters", or with a blank one, a 400 with "Data"'s line, nothing stored;
/// with it, stored; an image and a link without it; a `.blend` stored with
/// its warning and served as an attachment. M: the rule dropped for
/// `model`.
#[test]
fn rf9_what_matters_on_the_route() {
    let hub = hub("rf-hub-rf9", Some(TEAM));
    let c1 = card(&hub, "Front page");
    let expected = blob_expected();
    for name in [
        "sound.wav",
        "walk.mp4",
        "crate.glb",
        "plan.pdf",
        "scene.blend",
    ] {
        assert!(needs_matters(
            expected["stored"][name]["class"].as_str().expect("a class")
        ));
        let blob = stored(
            &hub,
            "mira",
            &fs::read(blob_fixture(name)).expect("a fixture"),
        );
        assert_eq!(blob["needs_matters"], json!(true), "{name}");
        let before = kept(&hub);
        for body in [
            json!({"on": c1, "b3": blob["b3"], "name": name}),
            json!({"on": c1, "b3": blob["b3"], "name": name, "matters": " \n "}),
        ] {
            let reply = attach(&hub, "mira", &body);
            assert_eq!(reply.status, 400, "{name}: {}", reply.text());
            assert!(
                reply.text().contains(MATTERS_REQUIRED),
                "{name}: {}",
                reply.text()
            );
            assert_eq!(kept(&hub), before, "{name}: nothing stored");
        }
        let made = attached(
            &hub,
            "mira",
            &json!({"on": c1, "b3": blob["b3"], "name": name, "matters": "the pace"}),
        );
        assert_eq!(made["matters"], json!("the pace"));
        if name == "scene.blend" {
            assert_eq!(made["warning"], json!(WARNING));
            let served = get_blob(&hub, "lev", blob["b3"].as_str().expect("b3"), &[]);
            assert!(
                served
                    .header("content-disposition")
                    .is_some_and(|value| value.starts_with("attachment;")),
                "{:?}",
                served.headers
            );
        }
    }
    fixture_ref(&hub, "mira", &c1, "lantern.gif", json!({}));
    attached(
        &hub,
        "mira",
        &json!({"on": c1, "url": "https://example.org/board"}),
    );
}

// ----------------------------------------------------------------- RF11

/// RF11, the route: an image over the agents' limits takes a copy; a copy
/// of 1 600 px, of 1.1 MiB, of another aspect (not the transpose), a copy
/// given for an agent-ready image, a link or a non-image: a 400 each with
/// `agent_b3: …`; the 1568x1176 copy and its transpose are taken.
#[test]
fn rf11_agent_copies_are_judged_on_the_route() {
    let hub = hub("rf-hub-rf11", Some(TEAM));
    let c1 = card(&hub, "Front page");
    let big = stored(&hub, "mira", &png(4032, 3024, 0x11, None));
    assert_eq!(big["agent_ready"], json!(false));
    let ready = stored(&hub, "mira", &png(1200, 800, 0x11, Some(300 * 1024)));
    assert_eq!(ready["agent_ready"], json!(true));
    let wav = stored(
        &hub,
        "mira",
        &fs::read(blob_fixture("sound.wav")).expect("wav"),
    );
    let copies: Vec<(&str, Value)> = vec![
        (
            "1600 px",
            stored(&hub, "mira", &png(1600, 1200, 0x22, None)),
        ),
        (
            "1.1 MiB",
            stored(&hub, "mira", &png(1568, 1176, 0x33, Some(1_153_434))),
        ),
        (
            "another aspect",
            stored(&hub, "mira", &png(1568, 1568, 0x44, None)),
        ),
    ];
    let good = stored(&hub, "mira", &png(1568, 1176, 0x55, Some(400 * 1024)));
    let turned = stored(&hub, "mira", &png(1176, 1568, 0x66, None));
    let before = kept(&hub);
    let refused = |body: Value, label: &str| {
        let reply = attach(&hub, "mira", &body);
        assert_eq!(reply.status, 400, "{label}: {}", reply.text());
        assert!(
            reply.text().contains("agent_b3: "),
            "{label}: {}",
            reply.text()
        );
        assert_eq!(kept(&hub), before, "{label}: nothing stored");
    };
    for (label, copy) in &copies {
        refused(
            json!({"on": c1, "b3": big["b3"], "agent_b3": copy["b3"], "name": "big.png"}),
            label,
        );
    }
    refused(
        json!({"on": c1, "b3": ready["b3"], "agent_b3": good["b3"], "name": "ready.png"}),
        "an agent-ready image",
    );
    refused(
        json!({"on": c1, "b3": wav["b3"], "agent_b3": good["b3"], "name": "a.wav", "matters": "x"}),
        "a non-image",
    );
    let link = attach(
        &hub,
        "mira",
        &json!({"on": c1, "url": "https://example.org/a", "agent_b3": good["b3"]}),
    );
    assert_eq!(link.status, 400, "a link: {}", link.text());
    let made = attached(
        &hub,
        "mira",
        &json!({"on": c1, "b3": big["b3"], "agent_b3": good["b3"], "name": "big.png"}),
    );
    assert_eq!(made["agent_b3"], good["b3"]);
    let made = attached(
        &hub,
        "mira",
        &json!({"on": c1, "b3": big["b3"], "agent_b3": turned["b3"], "name": "turned.png"}),
    );
    assert_eq!(
        made["agent_b3"], turned["b3"],
        "the transpose (EXIF rotation)"
    );
    // The copy is served inline, with no name.
    let served = get_blob(&hub, "lev", good["b3"].as_str().expect("b3"), &[]);
    assert_eq!(served.status, 200);
    let disposition = served
        .header("content-disposition")
        .unwrap_or("inline")
        .to_owned();
    assert!(
        disposition.starts_with("inline") && !disposition.contains("filename"),
        "{disposition}"
    );
}

// ----------------------------------------------------------------- RF12

fn delete_refused(reply: &Reply, id: &str, author: &str) {
    assert_eq!(reply.status, 403, "{}", reply.text());
    assert!(
        reply.text().contains("refused: ")
            && reply.text().contains(id)
            && reply.text().contains(author)
            && reply
                .text()
                .contains("only its author or an `owner` deletes it"),
        "{}",
        reply.text()
    );
}

/// RF12 with `[team]`: the author and the `owner` delete, the lead, a
/// member and a login outside `[team]` get 403 with the line, nothing
/// changed; `GET .../team`'s `allowed` ends `delete_comment`,
/// `delete_reference` for the owner alone. M: any `[team]` login may
/// delete.
#[test]
fn rf12_the_author_and_the_owner_delete_on_a_hub() {
    let hub = hub("rf-hub-rf12", Some(TEAM));
    let c1 = card(&hub, "Front page");
    let first = attached(
        &hub,
        "mira",
        &json!({"on": c1, "url": "https://example.org/1"}),
    );
    let second = attached(
        &hub,
        "mira",
        &json!({"on": c1, "url": "https://example.org/2"}),
    );
    let before = kept(&hub);
    for login in ["lena", "tomas", "eve"] {
        delete_refused(
            &delete(&hub, login, &id_of(&second)),
            &id_of(&second),
            "mira",
        );
        assert_eq!(kept(&hub), before, "{login}: nothing changed");
    }
    let events = hub.events().len();
    let own = delete(&hub, "mira", &id_of(&first));
    assert_eq!(own.status, 200, "{}", own.text());
    assert_eq!(own.json()["deleted"]["by"], json!("mira"));
    let owner = delete(&hub, "olga", &id_of(&second));
    assert_eq!(owner.status, 200, "{}", owner.text());
    assert_eq!(owner.json()["deleted"]["by"], json!("olga"));
    assert_eq!(
        hub.events()[events..].to_vec(),
        [
            (
                "reference.deleted".to_owned(),
                json!({"id": id_of(&first), "as": "mira"})
            ),
            (
                "reference.deleted".to_owned(),
                json!({"id": id_of(&second), "as": "olga"})
            ),
        ]
    );
    let again = delete(&hub, "olga", &id_of(&second));
    assert_eq!(again.status, 409, "{}", again.text());
    let allowed =
        |login: &str| hub.get(login, &format!("{P}/team")).status(200).json()["allowed"].clone();
    let owners = allowed("olga");
    let owners: Vec<&str> = owners
        .as_array()
        .expect("allowed")
        .iter()
        .map(|value| value.as_str().expect("a word"))
        .collect();
    assert_eq!(
        owners[owners.len() - 2..],
        ["delete_comment", "delete_reference"],
        "{owners:?}"
    );
    for login in ["lena", "tomas", "eve"] {
        let list = allowed(login);
        assert!(
            !list
                .as_array()
                .expect("allowed")
                .contains(&json!("delete_reference")),
            "{login}: {list}"
        );
    }
}

/// RF12 with no `[team]`, then a bad one: the author alone deletes.
#[test]
fn rf12_without_a_team_only_the_author_deletes() {
    for (name, table) in [
        ("rf-hub-rf12-none", None),
        ("rf-hub-rf12-bad", Some(BAD_TEAM)),
    ] {
        let hub = hub(name, table);
        let c1 = card(&hub, "Front page");
        let made = attached(
            &hub,
            "mira",
            &json!({"on": c1, "url": "https://example.org/1"}),
        );
        let id = id_of(&made);
        delete_refused(&delete(&hub, "olga", &id), &id, "mira");
        assert_eq!(delete(&hub, "mira", &id).status, 200, "{name}");
    }
}

// ----------------------------------------------------------------- RF13

/// RF13 on a hub: with `.swept` over an hour old, an upload sweeps an
/// unattached file 24 h + 1 s old and a `.part` 6 h + 1 s old, keeping an
/// unattached one 23 h old; within the hour nothing.
#[test]
fn rf13_an_upload_sweeps() {
    let hub = hub("rf-hub-rf13-sweep", Some(TEAM));
    let first = stored(
        &hub,
        "mira",
        &fs::read(blob_fixture("lantern.gif")).expect("gif"),
    );
    let store = store_of(&hub);
    let old = stored(&hub, "mira", &png(64, 64, 0x01, None));
    let young = stored(&hub, "mira", &png(64, 64, 0x02, None));
    let old_file = store.join(old["b3"].as_str().expect("b3"));
    let young_file = store.join(young["b3"].as_str().expect("b3"));
    age(&old_file, 24 * 3600 + 1);
    age(&young_file, 23 * 3600);
    fs::create_dir_all(store.join(".incoming")).expect("incoming");
    let old_part = store.join(".incoming").join("0011223344556677.part");
    let young_part = store.join(".incoming").join("8899aabbccddeeff.part");
    fs::write(&old_part, b"half").expect("part");
    fs::write(&young_part, b"half").expect("part");
    age(&old_part, 6 * 3600 + 1);
    age(&young_part, 5 * 3600);
    let swept = store.join(".swept");
    assert!(swept.is_file(), "the first upload swept and touched .swept");
    age(&swept, 1800);
    stored(&hub, "mira", &png(64, 64, 0x03, None));
    assert!(old_file.exists() && old_part.exists(), "within the hour");
    age(&swept, 3601);
    stored(&hub, "mira", &png(64, 64, 0x04, None));
    assert!(!old_file.exists(), "24 h + 1 s, named by no row");
    assert!(!old_part.exists(), "a .part 6 h + 1 s old");
    assert!(
        young_file.exists() && young_part.exists(),
        "younger ones kept"
    );
    assert!(store.join(first["b3"].as_str().expect("b3")).exists());
    assert!(age_of(&swept) < 120);
}

/// RF13: the sweep's race: an unattached file 25 h old is a candidate of
/// the upload whose start lists it; a `POST` attaching it lands before that
/// upload's turn; the turn's re-check keeps the file, and it is served. M:
/// the re-check inside the turn dropped.
#[test]
fn rf13_an_attach_between_the_scan_and_the_turn_keeps_the_file() {
    let hub = hub("rf-hub-rf13-race", Some(TEAM));
    let c1 = card(&hub, "Front page");
    stored(
        &hub,
        "mira",
        &fs::read(blob_fixture("lantern.gif")).expect("gif"),
    );
    let candidate = stored(&hub, "mira", &png(320, 200, 0x0C, None));
    let b3 = candidate["b3"].as_str().expect("b3").to_owned();
    let file = store_of(&hub).join(&b3);
    age(&file, 25 * 3600);
    age(&store_of(&hub).join(".swept"), 2 * 3600);
    let body = png(64, 64, 0x0D, None);
    // The upload's start: its walk lists the candidate; the 100 says the
    // body is being read, past the start.
    let mut stream = begin_upload(&hub, "tomas", body.len() as u64).expect("a 100");
    let made = attached(
        &hub,
        "mira",
        &json!({"on": c1, "b3": b3, "name": "kept.png"}),
    );
    assert_eq!(made["file"], json!("stored"));
    send_body(&mut stream, &body[..], None);
    let reply = read_head(stream, Duration::from_secs(60)).reply();
    assert_eq!(reply.status, 200, "{}", reply.text());
    assert!(file.exists(), "M: the re-check inside the turn dropped");
    assert_eq!(get_blob(&hub, "lev", &b3, &[]).status, 200);
}

// ----------------------------------------------------------------- RF16

/// RF16 on a hub: an attach and a delete one event each, `as` the login,
/// no title, URL, name, hash or "what matters"; a PUT none; the repository
/// unchanged.
#[test]
fn rf16_events_name_the_login_and_no_text() {
    let hub = hub("rf-hub-rf16", Some(TEAM));
    let c1 = card(&hub, "Front page");
    let repository = hub.repository();
    let events = hub.events().len();
    let blob = stored(
        &hub,
        "mira",
        &fs::read(blob_fixture("sound.wav")).expect("wav"),
    );
    assert_eq!(hub.events().len(), events, "a PUT emits none");
    let made = attached(
        &hub,
        "mira",
        &json!({"on": c1, "b3": blob["b3"], "name": "hum.wav", "title": "Harbour hum",
                "matters": "the low hum"}),
    );
    assert_eq!(delete(&hub, "mira", &id_of(&made)).status, 200);
    let after = hub.events()[events..].to_vec();
    assert_eq!(
        after,
        [
            (
                "reference.created".to_owned(),
                json!({"id": id_of(&made), "on": c1, "as": "mira"})
            ),
            (
                "reference.deleted".to_owned(),
                json!({"id": id_of(&made), "as": "mira"})
            ),
        ]
    );
    let text = serde_json::to_string(&after).expect("JSON");
    for word in ["Harbour", "hum", blob["b3"].as_str().expect("b3")] {
        assert!(!text.contains(word), "{word}: {text}");
    }
    assert_eq!(hub.repository(), repository, "the repository unchanged");
}

// ----------------------------------------------------------------- RF18

/// RF18, the binary door, each refusal answered from the head alone (the
/// body never sent) and leaving no `.part`: another `Content-Type` 415, a
/// `Content-Encoding` 415, no or a foreign `Origin` 403, `Sec-Fetch-Site:
/// cross-site` 403, no `Content-Length` 411; `OPTIONS` answers no
/// `Access-Control-*`.
#[test]
fn rf18_the_binary_door_refuses_from_the_head() {
    let hub = hub("rf-hub-rf18-door", Some(TEAM));
    let before = kept(&hub);
    let path = format!("{P}/blobs");
    let cases: Vec<(&str, Proxied, Option<u64>, u16, &str)> = vec![
        (
            "multipart",
            uploading("mira").set("Content-Type", "multipart/form-data; boundary=x"),
            Some(10),
            415,
            "refused: an upload is application/octet-stream, never multipart",
        ),
        (
            "JSON",
            uploading("mira").set("Content-Type", "application/json"),
            Some(10),
            415,
            "refused: an upload is application/octet-stream, never multipart",
        ),
        (
            "a sent type",
            uploading("mira").set("Content-Type", "image/png"),
            Some(10),
            415,
            "refused: an upload is application/octet-stream, never multipart",
        ),
        (
            "gzip",
            uploading("mira").plus("Content-Encoding", "gzip"),
            Some(10),
            415,
            "",
        ),
        (
            "no Origin",
            uploading("mira").without("Origin"),
            Some(10),
            403,
            "refused: an upload needs the Origin header",
        ),
        (
            "a foreign Origin",
            uploading("mira").set("Origin", "https://evil.example"),
            Some(10),
            403,
            "",
        ),
        (
            "cross-site",
            uploading("mira").set("Sec-Fetch-Site", "cross-site"),
            Some(10),
            403,
            "",
        ),
        ("no Content-Length", uploading("mira"), None, 411, ""),
    ];
    for (label, proxied, length, status, line) in cases {
        let stream = send_head(&hub.server, "PUT", &path, &proxied, length);
        let head = read_head(stream, Duration::from_secs(30));
        let reply = head.reply();
        assert_eq!(
            reply.status,
            status,
            "{label}: {}",
            String::from_utf8_lossy(&reply.body)
        );
        if !line.is_empty() {
            assert!(
                String::from_utf8_lossy(&reply.body).contains(line),
                "{label}: {line} in {}",
                String::from_utf8_lossy(&reply.body)
            );
        }
        assert_eq!(kept(&hub), before, "{label}: nothing stored, no .part");
    }
    let identity = put(&hub, "mira", b"");
    assert_ne!(identity.status, 200, "an empty body is no type");
    let options = through(
        &hub.server,
        "OPTIONS",
        &path,
        &Proxied::fetch("mira")
            .plus("Origin", "https://evil.example")
            .plus("Access-Control-Request-Method", "PUT"),
        b"",
    );
    assert!(
        options.cors_headers().is_empty(),
        "OPTIONS: {:?}",
        options.headers
    );
    // `Content-Encoding: identity` is no encoding.
    let gif = fs::read(blob_fixture("lantern.gif")).expect("gif");
    let plain = through(
        &hub.server,
        "PUT",
        &path,
        &uploading("mira").plus("Content-Encoding", "identity"),
        &gif,
    );
    assert_eq!(plain.status, 200, "{}", plain.text());
}

/// RF18: slots: a login's third concurrent upload and the hub's ninth are
/// 429 with `Retry-After: 5` and "Data"'s lines; a dropped upload frees its
/// slot and its `.part`, so the next succeeds. M: the slot kept after a
/// dropped body.
#[test]
fn rf18_uploads_take_slots() {
    let hub = hub("rf-hub-rf18-slots", Some(TEAM));
    let length = 4 * MIB;
    let holding = |login: &str| -> TcpStream {
        let mut stream = begin_upload(&hub, login, length)
            .unwrap_or_else(|reply| panic!("{login}: {} {}", reply.status, reply.text()));
        stream
            .write_all(&refs_common::wav_header(length - 44))
            .expect("a start");
        stream.write_all(&[0u8; 1024]).expect("some bytes");
        stream
    };
    let mira_one = holding("mira");
    let mira_two = holding("mira");
    let third = begin_upload(&hub, "mira", length).expect_err("a third of mira's");
    assert_eq!(third.status, 429, "{}", third.text());
    assert_eq!(third.header("retry-after"), Some("5"));
    assert!(
        third
            .text()
            .contains("refused: mira runs 2 uploads; one must end first"),
        "{}",
        third.text()
    );
    let mut held = vec![mira_two];
    for login in ["tomas", "tomas", "lena", "lena", "olga", "olga"] {
        held.push(holding(login));
    }
    // mira's first and seven others: eight.
    let ninth = begin_upload(&hub, "lev", length).expect_err("the hub's ninth");
    assert_eq!(ninth.status, 429, "{}", ninth.text());
    assert_eq!(ninth.header("retry-after"), Some("5"));
    assert!(
        ninth
            .text()
            .contains("refused: this hub runs 8 uploads; try again in a moment"),
        "{}",
        ninth.text()
    );
    // A dropped body frees its slot: mira's first closed mid-body.
    mira_one.shutdown(Shutdown::Both).expect("drop");
    drop(mira_one);
    let started = Instant::now();
    let reply = loop {
        match begin_upload(&hub, "lev", 200) {
            Ok(mut stream) => {
                let mut bytes = refs_common::wav_header(200 - 44);
                bytes.resize(200, 0);
                stream.write_all(&bytes).expect("the body");
                break read_head(stream, Duration::from_secs(30)).reply();
            }
            Err(reply) => {
                assert_eq!(reply.status, 429, "{}", reply.text());
                assert!(
                    started.elapsed() < Duration::from_secs(10),
                    "M: the slot kept after a dropped body"
                );
                thread::sleep(Duration::from_millis(100));
            }
        }
    };
    assert_eq!(reply.status, 200, "{}", reply.text());
    for stream in held {
        let _ = stream.shutdown(Shutdown::Both);
    }
    let started = Instant::now();
    while !parts(&store_of(&hub)).is_empty() {
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "every dropped upload's .part removed: {:?}",
            parts(&store_of(&hub))
        );
        thread::sleep(Duration::from_millis(50));
    }
}

// ----------------------------------------------------------------- RF19

/// RF19: while two 200 MiB PUTs stream to a hub at about 25 MiB/s each,
/// `GET .../cards` and an `add_comment` over `/mcp` each answer within
/// 1 000 ms; the PUTs' slow lines show the turn held only for the rename
/// (`run_ms` a sliver of `ms`). M: the body streamed inside the turn.
#[test]
fn rf19_an_upload_in_flight_holds_no_turn() {
    let hub = hub_with("rf-hub-rf19", &[], true);
    let c1 = card(&hub, "Front page");
    let total = 200 * MIB;
    let rate = Some(25 * MIB);
    let port = hub.server.port;
    let uploads: Vec<_> = ["mira", "tomas"]
        .into_iter()
        .enumerate()
        .map(|(n, login)| {
            let mut stream = begin_upload(&hub, login, total).expect("a 100");
            thread::spawn(move || {
                send_body(&mut stream, WavStream::new(total, 90 + n as u64), rate);
                read_head(stream, Duration::from_secs(120)).reply()
            })
        })
        .collect();
    thread::sleep(Duration::from_secs(2));
    let mut agent = Stateless::new(port, SLUG, "lev");
    for round in 0..3 {
        let started = Instant::now();
        let cards = hub.get("olga", &format!("{P}/cards"));
        let read = started.elapsed();
        assert_eq!(cards.status, 200);
        assert!(
            read <= Duration::from_millis(1000),
            "round {round}: GET cards {read:?}"
        );
        let started = Instant::now();
        let comment = agent.call(
            "add_comment",
            json!({"on": c1, "body": format!("Round {round}: the plates are wet."), "author_role": "printer"}),
        );
        let wrote = started.elapsed();
        let result = result_of(&comment, "add_comment");
        assert!(!is_error(result), "{}", content_text(result));
        assert!(
            wrote <= Duration::from_millis(1000),
            "round {round}: add_comment {wrote:?}"
        );
        thread::sleep(Duration::from_secs(1));
    }
    for upload in uploads {
        let reply = upload.join().expect("an upload");
        assert_eq!(reply.status, 200, "{}", reply.text());
    }
    let lines = requests_of(&hub.server, "/api/projects/{p}/blobs", 2);
    let puts: Vec<_> = lines.iter().filter(|line| line.method == "PUT").collect();
    assert_eq!(puts.len(), 2, "{lines:?}");
    for line in puts {
        assert!(
            line.ms >= 4000,
            "premise: the upload streamed: {}",
            line.line
        );
        let held = line.run_ms.unwrap_or(0);
        assert!(
            held * 10 <= line.ms && held <= 2000,
            "M: the body streamed inside the turn: {}",
            line.line
        );
    }
}

// ----------------------------------------------------------------- RF20

/// RF20 on the hub's wire: 24 tools with the three, no delete; no schema
/// holds `b3`, `file`, `as`, `login` or `actor`; every description ASCII
/// within 2 048 B, the three calling references people's material;
/// `add_reference` stores a link authored by the agent for the login;
/// `list_references`' content byte-equal to `spec ref list … 2>&1` on the
/// hub's queue and its `structuredContent` the `--json` document. M: `as`
/// added to `add_reference`.
#[test]
fn rf20_the_hubs_reference_tools() {
    let hub = hub("rf-hub-rf20", Some(TEAM));
    let c1 = card(&hub, "Front page");
    let mut agent = Stateless::new(hub.server.port, SLUG, "lev");
    let list = agent.tools();
    let names = tool_names(&list);
    assert_eq!(names.len(), 24, "{names:?}");
    for name in ["list_references", "get_reference", "add_reference"] {
        assert!(names.iter().any(|known| known == name), "{name}: {names:?}");
    }
    fn properties(schema: &Value, out: &mut Vec<String>) {
        match schema {
            Value::Object(map) => {
                if let Some(Value::Object(found)) = map.get("properties") {
                    out.extend(found.keys().cloned());
                }
                for value in map.values() {
                    properties(value, out);
                }
            }
            Value::Array(items) => items.iter().for_each(|item| properties(item, out)),
            _ => {}
        }
    }
    for name in &names {
        assert!(!name.contains("delete"), "{name}");
        let described = tool(&list, name);
        let description = described["description"].as_str().expect("a description");
        assert!(description.is_ascii(), "{name}: not ASCII");
        assert!(description.len() <= 2048, "{name}: {} B", description.len());
        let mut found = Vec::new();
        properties(&described["inputSchema"], &mut found);
        let reference = name.contains("reference");
        for forbidden in [
            "as", "login", "actor", "b3", "agent_b3", "file", "bytes", "data",
        ] {
            if !reference && !["as", "login", "actor"].contains(&forbidden) {
                continue;
            }
            assert!(
                !found.iter().any(|property| property == forbidden),
                "M: `{forbidden}` in {name}'s schema"
            );
        }
    }
    for name in ["list_references", "get_reference", "add_reference"] {
        let description = tool(&list, name)["description"]
            .as_str()
            .expect("a description")
            .to_lowercase();
        assert!(
            description.contains("people's material") && description.contains("instructions"),
            "{name}: {description}"
        );
    }
    // "MCP": the reads' annotations and the intake writers'.
    for name in ["list_references", "get_reference"] {
        assert_eq!(
            tool(&list, name)["annotations"],
            tool(&list, "list_comments")["annotations"],
            "{name}: the reads' annotations"
        );
    }
    assert_eq!(
        tool(&list, "add_reference")["annotations"],
        tool(&list, "add_comment")["annotations"],
        "the intake writers' annotations"
    );

    let made = agent.call(
        "add_reference",
        json!({"on": c1, "url": "https://example.org/board", "title": "Moodboard",
               "matters": "the palette", "author_role": "designer", "author_model": "opus",
               "run": "night-1"}),
    );
    let result = result_of(&made, "add_reference");
    assert!(!is_error(result), "{}", content_text(result));
    let reference = &result["structuredContent"];
    let mut keys: Vec<&str> = reference
        .as_object()
        .expect("a reference")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    let mut want = REFERENCE_KEYS.to_vec();
    want.sort_unstable();
    assert_eq!(keys, want, "the 21 keys");
    assert_eq!(reference["actor"], json!("lev"));
    assert_eq!(
        reference["author"],
        json!({"type": "agent", "role": "designer", "model": "opus", "run": "night-1", "login": "lev"})
    );
    for (label, arguments) in [
        (
            "as",
            json!({"on": c1, "url": "https://example.org/x", "as": "olga"}),
        ),
        (
            "b3",
            json!({"on": c1, "url": "https://example.org/x", "b3": "0".repeat(64)}),
        ),
        (
            "file",
            json!({"on": c1, "url": "https://example.org/x", "file": "/etc/hosts"}),
        ),
    ] {
        let response = agent.call("add_reference", arguments);
        let refused = response.get("error").is_some()
            || response
                .get("result")
                .is_some_and(|result| result["isError"] == json!(true));
        assert!(refused, "{label} taken: {response}");
    }
    assert_eq!(
        sqlite(
            &data_dir(&hub.setup.home).join(format!("{SLUG}.db")),
            "SELECT count(*) FROM refs;"
        )
        .trim(),
        "1",
        "no refused call stored"
    );

    let listed = agent.call("list_references", json!({"on": c1}));
    let result = result_of(&listed, "list_references");
    let setup = &hub.setup;
    let text = spec_json(
        &setup.home,
        &setup.cwd,
        &setup.root,
        &["ref", "list", "--on", &c1],
    );
    let both = format!("{}{}", text.stdout, text.stderr);
    let cli = common::spec(
        &setup.home,
        &setup.cwd,
        &[
            "--root",
            setup.root.to_str().expect("UTF-8"),
            "ref",
            "list",
            "--on",
            &c1,
        ],
    );
    assert_eq!(cli.code, 0, "{both}");
    assert_eq!(
        content_text(result),
        format!("{}{}", cli.stdout, cli.stderr)
    );
    assert_eq!(result["structuredContent"], text.json());
    assert!(content_text(result).contains(MATERIAL));
}

// ---------------------------------------------------------------- RF17

/// RF17, the route: a queue imported where the files are absent: the
/// reference reads `file: lost` and its GET is a 404; nothing else fails.
#[test]
fn rf17_a_lost_files_get_is_a_404() {
    let hub = hub("rf-hub-rf17", Some(TEAM));
    let c1 = card(&hub, "Front page");
    let made = fixture_ref(&hub, "mira", &c1, "lantern.gif", json!({}));
    let b3 = made["b3"].as_str().expect("b3").to_owned();
    // The file gone as a restore without the store leaves it.
    let file = store_of(&hub).join(&b3);
    fs::remove_file(&file).expect("remove");
    let listed = hub
        .get("mira", &format!("{P}/references?on={c1}"))
        .status(200)
        .json();
    assert_eq!(listed["references"][0]["file"], json!("lost"));
    assert_eq!(get_blob(&hub, "lev", &b3, &[]).status, 404);
    assert_eq!(hub.get("mira", &format!("{P}/cards")).status, 200);
}

// ---------------------------------------------------------------- "Data"

/// "The blob store": a store that cannot be written answers 503 with the
/// fix, nothing kept; reads and every other write go on.
#[test]
fn an_unwritable_store_is_a_503_and_nothing_else_stops() {
    let hub = hub("rf-hub-unwritable", Some(TEAM));
    let c1 = card(&hub, "Front page");
    let blobs = data_dir(&hub.setup.home).join("blobs");
    fs::create_dir_all(&blobs).expect("blobs");
    fs::set_permissions(
        &blobs,
        <fs::Permissions as std::os::unix::fs::PermissionsExt>::from_mode(0o500),
    )
    .expect("chmod");
    let reply = put(
        &hub,
        "mira",
        &fs::read(blob_fixture("lantern.gif")).expect("gif"),
    );
    fs::set_permissions(
        &blobs,
        <fs::Permissions as std::os::unix::fs::PermissionsExt>::from_mode(0o700),
    )
    .expect("chmod back");
    assert_eq!(reply.status, 503, "{}", reply.text());
    assert!(
        reply
            .text()
            .contains("is not writable: on a hub, chown 10001:10001 /data/specengine/blobs"),
        "{}",
        reply.text()
    );
    assert_eq!(hub.get("mira", &format!("{P}/cards")).status, 200);
    attached(
        &hub,
        "mira",
        &json!({"on": c1, "url": "https://example.org/a"}),
    );
    let comment = hub.post(
        "mira",
        "comments",
        &json!({"on": c1, "body": "Still here."}),
    );
    assert_eq!(comment.status, 200, "{}", comment.text());
}

/// `[references]`: a bad table refuses `PUT .../blobs` and `POST
/// .../references` (503, its line) and nothing else; `allow` narrows a
/// PUT to a 415 with "Types"' line.
#[test]
fn a_bad_references_table_refuses_the_two_writes_alone() {
    let setup = Setup::new("rf-hub-allow", "card-press");
    with_table(&setup, "\n[references]\nallow = [\"image\"]\n");
    let hub = setup.start();
    let c1 = card(&hub, "Front page");
    let wav = put(
        &hub,
        "mira",
        &fs::read(blob_fixture("sound.wav")).expect("wav"),
    );
    assert_eq!(wav.status, 415, "{}", wav.text());
    assert!(
        wav.text()
            .contains("audio/wav is not allowed in this project ([references] allow)"),
        "{}",
        wav.text()
    );
    stored(
        &hub,
        "mira",
        &fs::read(blob_fixture("lantern.gif")).expect("gif"),
    );
    drop(hub);

    let setup = Setup::new("rf-hub-bad-allow", "card-press");
    with_table(&setup, "\n[references]\nallow = [\"image/svg+xml\"]\n");
    let hub = setup.start();
    let c1_bad = card(&hub, "Front page");
    let line = "[references] allow: \\\"image/svg+xml\\\" is not a type SpecEngine stores";
    for reply in [
        put(
            &hub,
            "mira",
            &fs::read(blob_fixture("lantern.gif")).expect("gif"),
        ),
        attach(
            &hub,
            "mira",
            &json!({"on": c1_bad, "url": "https://example.org/a"}),
        ),
    ] {
        assert_eq!(reply.status, 503, "{}", reply.text());
        assert!(reply.text().contains(line), "{}", reply.text());
    }
    assert_eq!(
        hub.get("mira", &format!("{P}/references?on={c1_bad}"))
            .status,
        200,
        "the list goes on"
    );
    assert_eq!(hub.get("mira", &format!("{P}/cards")).status, 200);
    let _ = c1;
}

/// "HTTP": the `POST .../references` body's caps, a 400 each with nothing
/// stored: both `b3` and `url`, neither; `name` with `/`, `..`, a bidi
/// control, 256 bytes; a two-line title, a title of 257 bytes; "what
/// matters" of 2 049 bytes or holding U+0000; a URL of `ftp:`, with no host,
/// with a space, of 2 049 bytes; an unknown key; `as` naming another login
/// a 403. An unknown blob a 404 `blob <b3> is not stored here: upload it
/// again`.
#[test]
fn the_attach_body_is_judged() {
    let hub = hub("rf-hub-body", Some(TEAM));
    let c1 = card(&hub, "Front page");
    let gif = stored(
        &hub,
        "mira",
        &fs::read(blob_fixture("lantern.gif")).expect("gif"),
    );
    let b3 = gif["b3"].clone();
    let before = kept(&hub);
    let long_url = format!("https://example.org/{}", "a".repeat(2049 - 20));
    for (label, body) in [
        (
            "both",
            json!({"on": c1, "b3": b3, "url": "https://example.org/a"}),
        ),
        ("neither", json!({"on": c1})),
        ("a slash", json!({"on": c1, "b3": b3, "name": "a/b.gif"})),
        ("dot dot", json!({"on": c1, "b3": b3, "name": ".."})),
        (
            "bidi",
            json!({"on": c1, "b3": b3, "name": "a\u{202e}fig.gif"}),
        ),
        (
            "256 B name",
            json!({"on": c1, "b3": b3, "name": "n".repeat(256)}),
        ),
        (
            "two lines",
            json!({"on": c1, "b3": b3, "name": "a.gif", "title": "one\ntwo"}),
        ),
        (
            "257 B title",
            json!({"on": c1, "b3": b3, "name": "a.gif", "title": "t".repeat(257)}),
        ),
        (
            "2 049 B matters",
            json!({"on": c1, "b3": b3, "name": "a.gif", "matters": "m".repeat(2049)}),
        ),
        (
            "NUL",
            json!({"on": c1, "b3": b3, "name": "a.gif", "matters": "a\u{0}b"}),
        ),
        ("ftp", json!({"on": c1, "url": "ftp://example.org/a"})),
        ("no host", json!({"on": c1, "url": "https:///a"})),
        (
            "a space",
            json!({"on": c1, "url": "https://example.org/a b"}),
        ),
        ("2 049 B URL", json!({"on": c1, "url": long_url})),
        (
            "unknown key",
            json!({"on": c1, "url": "https://example.org/a", "colour": "red"}),
        ),
        (
            "name with url",
            json!({"on": c1, "url": "https://example.org/a", "name": "a.gif"}),
        ),
    ] {
        let reply = attach(&hub, "mira", &body);
        assert_eq!(reply.status, 400, "{label}: {}", reply.text());
        assert_eq!(kept(&hub), before, "{label}: nothing stored");
    }
    let other = attach(
        &hub,
        "mira",
        &json!({"on": c1, "url": "https://example.org/a", "as": "olga"}),
    );
    assert_eq!(other.status, 403, "{}", other.text());
    let unknown = "f".repeat(64);
    let reply = attach(
        &hub,
        "mira",
        &json!({"on": c1, "b3": unknown, "name": "x.gif"}),
    );
    assert_eq!(reply.status, 404, "{}", reply.text());
    assert!(
        reply.text().contains(&format!(
            "blob {unknown} is not stored here: upload it again"
        )),
        "{}",
        reply.text()
    );
    let missing = attach(
        &hub,
        "mira",
        &json!({"on": "C-0099", "url": "https://example.org/a"}),
    );
    assert_eq!(missing.status, 404, "{}", missing.text());
    assert_eq!(kept(&hub), before);
    let name_ok = attached(
        &hub,
        "mira",
        &json!({"on": c1, "b3": b3, "name": "n".repeat(255)}),
    );
    assert_eq!(name_ok["name"].as_str().map(str::len), Some(255));
}

/// "Documents": the HTTP answer of an attach is the 21-key reference, the
/// list `{on, total, references, next_before, notes}`; a delete answers its
/// tombstone.
#[test]
fn the_routes_answer_the_documents() {
    let hub = hub("rf-hub-docs", Some(TEAM));
    let c1 = card(&hub, "Front page");
    let gif = stored(
        &hub,
        "mira",
        &fs::read(blob_fixture("lantern.gif")).expect("gif"),
    );
    let reply = attach(
        &hub,
        "mira",
        &json!({"on": c1, "b3": gif["b3"], "name": "lamp.gif", "title": "Lamp"}),
    );
    assert_eq!(reply.status, 200, "{}", reply.text());
    assert_eq!(keys_in_order(reply.text()), REFERENCE_KEYS);
    let made = reply.json();
    assert_eq!(made["actor"], json!("mira"));
    assert_eq!(made["author"]["login"], json!("mira"));
    assert_eq!(made["file"], json!("stored"));
    assert_eq!(made["inline"], json!(true));
    let list = hub.get("mira", &format!("{P}/references?on={c1}"));
    assert_eq!(list.status, 200);
    assert_eq!(
        keys_in_order(list.text()),
        // `in_spec` since docs/features/reference-promotion.md.
        [
            "on",
            "total",
            "references",
            "next_before",
            "notes",
            "in_spec"
        ]
    );
    let gone = delete(&hub, "mira", &id_of(&made));
    assert_eq!(gone.status, 200, "{}", gone.text());
    assert_eq!(keys_in_order(gone.text()), REFERENCE_KEYS);
    assert_eq!(gone.json()["deleted"]["by"], json!("mira"));
    assert_eq!(gone.json()["title"], Value::Null);
}

// ------------------------------------------------- the review's findings

/// Every `Cache-Control` header of an answer, in order.
fn cache_controls(headers: &[(String, String)]) -> Vec<String> {
    headers
        .iter()
        .filter(|(name, _)| name == "cache-control")
        .map(|(_, value)| value.clone())
        .collect()
}

/// The review of work-references (iteration 1), the fence's header: every
/// route but a file's answers `Cache-Control: no-store` once, the events
/// stream and the hub's `/mcp` among them; a file's GET keeps its own
/// `private, no-cache`, once. M: the fence's `or_insert` (a route's own
/// header kept, or none added).
#[test]
fn every_route_but_a_files_answers_no_store() {
    let hub = hub("rf-hub-no-store", Some(TEAM));
    let c1 = card(&hub, "Front page");
    let gif = fs::read(blob_fixture("lantern.gif")).expect("gif");
    let blob = put(&hub, "mira", &gif);
    assert_eq!(blob.status, 200, "{}", blob.text());
    let b3 = blob.json()["b3"].as_str().expect("b3").to_owned();
    let made = attach(
        &hub,
        "mira",
        &json!({"on": c1, "b3": b3, "name": "lamp.gif"}),
    );
    let list = hub.get("mira", &format!("{P}/references?on={c1}"));
    let cards = hub.get("mira", &format!("{P}/cards"));
    let refused = put(&hub, "mira", b"tide tables\n");
    let mut agent = Stateless::new(hub.server.port, SLUG, "lev");
    let (_, mcp) = agent.send_as("lev", "tools/list", json!({}));
    let events = common::Stream::open_raw(
        hub.server.port,
        &format!("{P}/events"),
        &Proxied::fetch("mira").pairs(),
    );
    for (label, headers) in [
        ("PUT blobs", &blob.headers),
        ("PUT blobs refused", &refused.headers),
        ("POST references", &made.headers),
        ("GET references", &list.headers),
        ("GET cards", &cards.headers),
        ("POST /mcp", &mcp.reply.headers),
        ("GET events", &events.headers),
    ] {
        assert_eq!(
            cache_controls(headers),
            ["no-store"],
            "{label}: {headers:?}"
        );
    }
    assert_eq!(events.status, 200);
    let file = get_blob(&hub, "lev", &b3, &[]);
    assert_eq!(file.status, 200);
    assert_eq!(
        cache_controls(&file.headers),
        ["private, no-cache"],
        "a file's own header, once: {:?}",
        file.headers
    );
    let gone = get_blob(&hub, "lev", &"0".repeat(64), &[]);
    assert_eq!(gone.status, 404);
    assert_eq!(
        cache_controls(&gone.headers),
        ["no-store"],
        "{:?}",
        gone.headers
    );
}

/// The daemon's threads (`ps -M` on macOS, `/proc` elsewhere).
fn threads_of(pid: u32) -> usize {
    if cfg!(target_os = "macos") {
        let output = std::process::Command::new("/bin/ps")
            .args(["-M", "-p", &pid.to_string()])
            .output()
            .expect("ps runs");
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .count()
            .saturating_sub(1)
    } else {
        fs::read_to_string(format!("/proc/{pid}/status"))
            .ok()
            .and_then(|status| {
                status
                    .lines()
                    .find_map(|line| line.strip_prefix("Threads:"))
                    .and_then(|count| count.trim().parse().ok())
            })
            .unwrap_or(0)
    }
}

/// The review of work-references (iteration 1), a stalled reader: 24
/// readers of a 32 MiB file each read their first piece and then stop
/// reading, their sockets open. Past the runtime's 10 s keep-alive of an
/// idle blocking thread, the daemon holds no thread per stalled reader (it
/// had one each, blocked in its send); meanwhile a comment and a reference
/// written by a page answer within 1 000 ms. M: a blocking thread held per
/// answer for its whole length.
#[test]
fn a_stalled_reader_holds_no_thread_and_stalls_no_write() {
    let hub = hub("rf-hub-stalled", Some(TEAM));
    let c1 = card(&hub, "Front page");
    let total = 32 * MIB;
    let reply = upload(&hub, "mira", total, WavStream::new(total, 32), None);
    assert_eq!(reply.status, 200, "{}", reply.text());
    let b3 = reply.json()["b3"].as_str().expect("b3").to_owned();
    attached(
        &hub,
        "mira",
        &json!({"on": c1, "b3": b3, "name": "swell.wav", "matters": "the swell"}),
    );
    let pid = hub.server.pid();
    // At rest: the blocking threads of the writes so far gone idle and ended.
    thread::sleep(Duration::from_secs(11));
    let rest = threads_of(pid);
    let stalled: Vec<Head> = (0..24)
        .map(|n| {
            let mut head = get_blob(&hub, &format!("reader-{n}"), &b3, &[]);
            assert_eq!(head.status, 200);
            if head.early.is_empty() {
                let mut piece = [0u8; 65_536];
                let read = head.stream.read(&mut piece).expect("a first piece");
                assert!(read > 0);
            }
            head
        })
        .collect();
    let started = Instant::now();
    let comment = hub.post(
        "olga",
        "comments",
        &json!({"on": c1, "body": "Still writable?"}),
    );
    let wrote = started.elapsed();
    assert_eq!(comment.status, 200, "{}", comment.text());
    assert!(
        wrote <= Duration::from_millis(1000),
        "a comment waited {wrote:?}"
    );
    let started = Instant::now();
    attached(
        &hub,
        "olga",
        &json!({"on": c1, "url": "https://example.org/still"}),
    );
    assert!(
        started.elapsed() <= Duration::from_millis(1000),
        "{:?}",
        started.elapsed()
    );
    thread::sleep(Duration::from_secs(12));
    let now = threads_of(pid);
    assert!(
        now < rest + 12,
        "M: a thread held per stalled reader: {rest} threads at rest, {now} with 24 stalled"
    );
    drop(stalled);
}

/// The review of work-references (iteration 1), the quota under concurrent
/// starts: with `SPECENGINE_BLOB_QUOTA=10485760`, four uploads of 4 MiB by
/// four logins begin at once: exactly two are asked for their bodies, the
/// other two refused with 413 at their start; the refused ones' bytes are
/// given back (a 2 MiB start then fits beside the two in flight); dropped
/// ones give theirs back too. M: the quota read before the flight is taken
/// (every start passes).
#[test]
fn concurrent_starts_cannot_pass_the_quota() {
    let hub = hub_with(
        "rf-hub-quota-race",
        &[("SPECENGINE_BLOB_QUOTA", "10485760")],
        false,
    );
    let barrier = Arc::new(std::sync::Barrier::new(4));
    let port = hub.server.port;
    let starts: Vec<_> = ["mira", "tomas", "lena", "olga"]
        .into_iter()
        .map(|login| {
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                let head = uploading(login).plus("Expect", "100-continue");
                let mut headers = head.pairs();
                let length = (4 * MIB).to_string();
                headers.push(("Content-Length", length.as_str()));
                let bytes = common::request_bytes("PUT", &format!("{P}/blobs"), &headers);
                let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
                barrier.wait();
                stream.write_all(&bytes).expect("the head");
                read_head(stream, Duration::from_secs(30))
            })
        })
        .collect();
    let heads: Vec<Head> = starts
        .into_iter()
        .map(|start| start.join().expect("a start"))
        .collect();
    let statuses: Vec<u16> = heads.iter().map(|head| head.status).collect();
    assert_eq!(
        statuses.iter().filter(|status| **status == 100).count(),
        2,
        "M: the flight read late: {statuses:?}"
    );
    assert_eq!(
        statuses.iter().filter(|status| **status == 413).count(),
        2,
        "{statuses:?}"
    );
    let (running, refused): (Vec<Head>, Vec<Head>) =
        heads.into_iter().partition(|head| head.status == 100);
    for head in refused {
        assert_eq!(head.header("connection"), Some("close"));
        let reply = head.reply();
        assert!(
            reply.text().contains("would pass 10 MiB"),
            "{}",
            reply.text()
        );
    }
    // 8 MiB in flight: a 2 MiB start fits, the refused starts gave theirs back.
    let fits = upload(&hub, "eve", 2 * MIB, WavStream::new(2 * MIB, 61), None);
    assert_eq!(fits.status, 200, "{}", fits.text());
    // 10 MiB now taken: 1 more byte does not fit.
    let over = begin_upload(&hub, "lev", 1024).expect_err("past the quota");
    assert_eq!(over.status, 413, "{}", over.text());
    for head in running {
        let _ = head.stream.shutdown(Shutdown::Both);
    }
    // The dropped uploads give their 8 MiB back: 8 MiB more fits beside the
    // 2 MiB stored.
    let started = Instant::now();
    let reply = loop {
        let reply = upload(&hub, "lev", 8 * MIB, WavStream::new(8 * MIB, 62), None);
        if reply.status != 413 || started.elapsed() > Duration::from_secs(10) {
            break reply;
        }
        thread::sleep(Duration::from_millis(100));
    };
    assert_eq!(reply.status, 200, "{}", reply.text());
}

/// The review of work-references (iteration 1), the file count at commit:
/// with 19 999 files stored, two uploads of new bytes both pass their start
/// (the walk counts 19 999), and at their renames one stores and the other
/// is refused with "Data"'s line: the store never holds 20 001 files. M:
/// the count judged on the start's walk alone.
#[test]
fn the_file_count_holds_at_commit() {
    let hub = hub("rf-hub-count-race", Some(TEAM));
    let store = store_of(&hub);
    stored(
        &hub,
        "mira",
        &fs::read(blob_fixture("lantern.gif")).expect("gif"),
    );
    for n in 0..19_998u64 {
        fs::write(store.join(format!("{n:064x}")), b"x").expect("plant a file");
    }
    assert_eq!(store_files(&store).len(), 19_999);
    let first = png(64, 64, 0x71, None);
    let second = png(64, 64, 0x72, None);
    let mut one = begin_upload(&hub, "mira", first.len() as u64).expect("a start");
    let mut two = begin_upload(&hub, "tomas", second.len() as u64).expect("a start");
    one.write_all(&first).expect("a body");
    two.write_all(&second).expect("a body");
    let answers = [
        read_head(one, Duration::from_secs(30)).reply(),
        read_head(two, Duration::from_secs(30)).reply(),
    ];
    let statuses: Vec<u16> = answers.iter().map(|reply| reply.status).collect();
    assert_eq!(
        statuses.iter().filter(|status| **status == 200).count(),
        1,
        "M: both stored: {statuses:?}"
    );
    let refused = answers
        .iter()
        .find(|reply| reply.status == 413)
        .unwrap_or_else(|| panic!("no 413: {statuses:?}"));
    assert!(
        refused
            .text()
            .contains("refused: this project stores 20 000 files; delete some first"),
        "{}",
        refused.text()
    );
    assert_eq!(store_files(&store).len(), 20_000, "never 20 001");
    assert_eq!(parts(&store), Vec::<String>::new());
}

/// The review of work-references (iteration 1): `PUT .../blobs` takes no
/// query: `?x=1` is refused before a byte is read, no `.part`.
#[test]
fn an_upload_takes_no_query() {
    let hub = hub("rf-hub-query", Some(TEAM));
    let before = kept(&hub);
    let gif = fs::read(blob_fixture("lantern.gif")).expect("gif");
    let reply = through(
        &hub.server,
        "PUT",
        &format!("{P}/blobs?x=1"),
        &uploading("mira"),
        &gif,
    );
    assert!(
        (400..500).contains(&reply.status),
        "a query taken: {} {}",
        reply.status,
        reply.text()
    );
    assert_eq!(kept(&hub), before, "nothing stored, no .part");
}
