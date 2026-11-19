//! Plumbing of docs/features/doc-library.md (RM-0127) DL13-DL15, shared by
//! `library_scale.rs` and `library_soak.rs`:
//!
//! - **The tree** of DC-11 (`catalog_scale.rs`, the same recipe, written out
//!   again here so that file keeps its own tests): `specengine-eval scale
//!   --corpus-only --nodes N --seed 1`, `[paths] spec = "areas"`, committed
//!   whole, then a `git fast-import` stream of `commits` commits, each
//!   editing 1 to 5 part files drawn by splitmix64 seeded 1 (each edit
//!   appends `Edit <n>.`), the last live part file never edited;
//! - **the release binaries** beside the test binary
//!   (`target.noindex/release/{spec,specengine-http}`);
//! - **a hub**: the release daemon over the tree with `--public-origin`,
//!   `--port 0`, `--slow-ms 0` and a scratch `HOME`, under `perl -e 'alarm
//!   N; exec @ARGV'` (it dies by SIGALRM even if this process is killed),
//!   killed and reaped on drop; its stderr kept;
//! - **requests** as `crates/specengine-http/tests/common/hub.rs` builds
//!   them (the public `Host`, `Sec-Fetch-*` of a same-origin fetch, one
//!   `Remote-User`, Traefik's `X-Forwarded-*`), and a stateless MCP call as
//!   `common/mcp_http.rs`'s `Stateless` sends it, over a raw HTTP/1.1 client
//!   (`Connection: close`, read to the end);
//! - **the slow lines** (`specengine-http: slow request … ms= wait_ms=
//!   run_ms= index= …`), parsed by key;
//! - **RSS** samples (`ps -o rss= -p <pid>`, KiB), the Theil-Sen slope (the
//!   median of all pairwise slopes), percentiles.
//!
//! Every run writes under its scratch directory alone, but a hang's `sample`
//! file, which is kept in the system temp directory for reading after the
//! run. Every name is invented.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::git::Sandbox;
use crate::import_support::{Scratch, eval, stderr};

/// The public origin the hub is told it serves.
pub const ORIGIN: &str = "https://hub.example.test";
pub const AUTHORITY: &str = "hub.example.test";
/// The library route as a slow line names it.
pub const LIBRARY_TEMPLATE: &str = "/api/projects/{p}/library";
pub const INBOX_TEMPLATE: &str = "/api/projects/{p}/inbox";
/// The seeded history's length (DC-11).
pub const COMMITS: usize = 10_000;

// ------------------------------------------------------------ the binaries

/// The release profile's directory (the test binary's `deps`' parent).
pub fn release_dir() -> PathBuf {
    let exe = std::env::current_exe().expect("the test binary");
    exe.parent()
        .and_then(Path::parent)
        .expect("the profile directory")
        .to_path_buf()
}

/// A release binary, which must exist and be no older than any source of
/// the crates it is built from (nor `ui/dist`, which the daemon embeds): a
/// stale binary would measure another build.
pub fn release_bin(name: &str) -> PathBuf {
    let path = release_dir().join(name);
    let build = "cargo build --release -p specengine-cli -p specengine-http --bins";
    let built = fs::metadata(&path)
        .and_then(|meta| meta.modified())
        .unwrap_or_else(|_| panic!("{}: build it first: {build}", path.display()));
    let root = crate::import_support::repository_root();
    let mut sources: Vec<PathBuf> = [
        "specengine-model",
        "specengine-core",
        "specengine-store",
        "specengine-code",
        "specengine-cli",
        "specengine-mcp",
        "specengine-http",
    ]
    .iter()
    .map(|name| root.join("crates").join(name).join("src"))
    .collect();
    sources.push(root.join("ui/dist"));
    fn newest(dir: &Path, best: &mut Option<(std::time::SystemTime, PathBuf)>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                newest(&path, best);
            } else if let Ok(at) = entry.metadata().and_then(|meta| meta.modified())
                && best.as_ref().is_none_or(|(known, _)| at > *known)
            {
                *best = Some((at, path));
            }
        }
    }
    let mut best = None;
    for dir in &sources {
        newest(dir, &mut best);
    }
    if let Some((at, source)) = best {
        assert!(
            at <= built,
            "{} is older than {}: rebuild it: {build}",
            path.display(),
            source.display()
        );
    }
    path
}

// ------------------------------------------------------------- the tree

/// splitmix64, seeded: the files each commit edits.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % n as u64).expect("an index")
    }
}

/// Every part file of `tree` (`areas/**/*.md` but the READMEs), relative,
/// sorted.
fn part_files(tree: &Path) -> Vec<String> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
        for entry in fs::read_dir(dir).expect("a readable directory").flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(root, &path, out);
            } else if path.extension().is_some_and(|extension| extension == "md")
                && path.file_name().is_some_and(|name| name != "README.md")
            {
                let relative = path.strip_prefix(root).expect("under the tree");
                out.push(relative.to_str().expect("UTF-8").to_owned());
            }
        }
    }
    let mut out = Vec::new();
    walk(tree, &tree.join("areas"), &mut out);
    out.sort();
    out
}

/// DC-11's committed tree and history.
pub struct Tree {
    pub scratch: Scratch,
    pub git: Sandbox,
    pub root: PathBuf,
    pub nodes: usize,
    pub slug: String,
    /// The part files, relative, sorted.
    pub parts: Vec<String>,
    /// Commits made on top of the history by [`Tree::commit_part`].
    commits: Mutex<usize>,
}

impl Tree {
    pub fn new(label: &str, nodes: usize, commits: usize) -> Self {
        let scratch = Scratch::new(&format!("library-{label}"));
        let home = scratch.join("eval-home");
        fs::create_dir_all(&home).expect("a home");
        let out = scratch.join("out");
        let output = eval()
            .env("HOME", &home)
            .env_remove("XDG_DATA_HOME")
            .arg("scale")
            .arg("--out")
            .arg(&out)
            .args([
                "--corpus-only",
                "--nodes",
                &nodes.to_string(),
                "--seed",
                "1",
            ])
            .output()
            .expect("specengine-eval runs");
        assert!(
            output.status.success(),
            "`scale --corpus-only --nodes {nodes} --seed 1`: {}",
            stderr(&output)
        );
        let root =
            fs::canonicalize(out.join(format!("scale/synthetic/n{nodes}/tree"))).expect("the tree");
        let config = root.join("specengine.toml");
        let text = fs::read_to_string(&config).expect("the config");
        let anchor = "tier1_name = \"README.md\"\n";
        assert_eq!(text.matches(anchor).count(), 1, "{text}");
        fs::write(
            &config,
            text.replacen(anchor, &format!("{anchor}spec = \"areas\"\n"), 1),
        )
        .expect("the config");
        let slug = toml::from_str::<toml::Table>(&fs::read_to_string(&config).expect("config"))
            .expect("TOML")["project"]["slug"]
            .as_str()
            .expect("a [project] slug")
            .to_owned();
        let git = Sandbox::new(&scratch.0);
        git.init(&root);
        git.add_all(&root);
        git.commit(&root, "the whole tree");
        let first = git.git_text(&root, &["rev-parse", "HEAD"]);
        let files = part_files(&root);
        assert!(files.len() > 10, "premise: {} part files", files.len());
        let untouched = files
            .iter()
            .rposition(|path| {
                fs::read_to_string(root.join(path))
                    .expect("a part file")
                    .contains("\nstatus: draft\n")
            })
            .expect("a live part file");
        let mut rng = Rng(1);
        let mut texts: BTreeMap<String, String> = BTreeMap::new();
        let mut stream: Vec<u8> = Vec::new();
        for n in 1..=commits {
            let count = 1 + rng.below(5);
            let mut chosen = std::collections::BTreeSet::new();
            while chosen.len() < count.min(files.len() - 1) {
                let at = rng.below(files.len());
                if at != untouched {
                    chosen.insert(at);
                }
            }
            let message = format!("Edit {n}\n");
            stream.extend_from_slice(
                format!(
                    "commit refs/heads/main\nmark :{n}\ncommitter Seed Writer \
                     <seed@example.org> {} +0000\ndata {}\n{message}",
                    1_780_000_000 + n,
                    message.len()
                )
                .as_bytes(),
            );
            if n == 1 {
                stream.extend_from_slice(format!("from {first}\n").as_bytes());
            }
            for at in chosen {
                let path = &files[at];
                let text = texts
                    .entry(path.clone())
                    .or_insert_with(|| fs::read_to_string(root.join(path)).expect("a part file"));
                text.push_str(&format!("\nEdit {n}.\n"));
                stream.extend_from_slice(
                    format!("M 100644 inline {path}\ndata {}\n", text.len()).as_bytes(),
                );
                stream.extend_from_slice(text.as_bytes());
                stream.push(b'\n');
            }
        }
        if commits > 0 {
            let mut child = git
                .command(git.git_program(), &root, &[])
                .args(["fast-import", "--quiet"])
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()
                .expect("fast-import runs");
            child
                .stdin
                .take()
                .expect("stdin")
                .write_all(&stream)
                .expect("the stream");
            let done = child.wait_with_output().expect("fast-import ends");
            assert!(
                done.status.success(),
                "fast-import: {}",
                String::from_utf8_lossy(&done.stderr)
            );
            git.git(&root, &["reset", "-q", "--hard", "refs/heads/main"]);
        }
        Self {
            scratch,
            git,
            root,
            nodes,
            slug,
            parts: files,
            commits: Mutex::new(0),
        }
    }

    /// The ID of the document of a part file (its stem).
    pub fn part_id(&self, at: usize) -> String {
        Path::new(&self.parts[at % self.parts.len()])
            .file_stem()
            .and_then(OsStr::to_str)
            .expect("a part file named by its ID")
            .to_owned()
    }

    /// One more commit on the served checkout: a paragraph appended to one
    /// part file (round robin), committed at a time after the history's.
    /// The number of commits made so far.
    pub fn commit_part(&self) -> usize {
        let mut made = self.commits.lock().expect("the commit count");
        *made += 1;
        let n = *made;
        let path = &self.parts[(n * 7919) % self.parts.len()];
        let file = self.root.join(path);
        let mut text = fs::read_to_string(&file).expect("a part file");
        text.push_str(&format!("\nSoak edit {n}.\n"));
        fs::write(&file, text).expect("the part file");
        self.git.git(&self.root, &["add", "--", path.as_str()]);
        let date = OsString::from(format!("{} +0000", 1_790_000_000 + n));
        let message = format!("Soak {n}");
        self.git.git_env(
            &self.root,
            &["commit", "-q", "--no-verify", "-m", message.as_str()],
            &[
                ("GIT_AUTHOR_DATE", date.as_os_str()),
                ("GIT_COMMITTER_DATE", date.as_os_str()),
            ],
        );
        n
    }

    /// The variables a release `spec` runs with: the sandbox's, `HOME`
    /// `home`.
    pub fn spec_vars(&self, home: &Path) -> Vec<(OsString, OsString)> {
        self.git
            .vars()
            .into_iter()
            .filter(|(name, _)| name != "HOME")
            .chain([(OsString::from("HOME"), home.as_os_str().to_owned())])
            .collect()
    }
}

// ---------------------------------------------------------------- a hub

/// The release daemon as a hub over a tree; killed and reaped on drop.
pub struct Hub {
    child: Child,
    pub port: u16,
    pub pid: u32,
    stderr: Arc<Mutex<Vec<u8>>>,
}

impl Hub {
    /// The hub over `root` under `home` at `--slow-ms 0`, its self-kill
    /// alarm `alarm_secs` away, until its `listening` line.
    pub fn start(root: &Path, home: &Path, git: &Sandbox, alarm_secs: u64) -> Self {
        fs::create_dir_all(home).expect("the hub's home");
        let program = release_bin("specengine-http");
        let path = std::env::join_paths([
            git.git_program()
                .parent()
                .expect("git's directory")
                .to_path_buf(),
            PathBuf::from("/usr/bin"),
            PathBuf::from("/bin"),
        ])
        .expect("a PATH");
        let mut child = Command::new("/usr/bin/perl")
            .env_clear()
            .env("HOME", home)
            .env("PATH", path)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_ALLOW_PROTOCOL", "file")
            .arg("-e")
            .arg(format!(
                "alarm {alarm_secs}; exec @ARGV or die \"exec: $!\""
            ))
            .arg(&program)
            .arg("--root")
            .arg(root)
            .args(["--port", "0", "--public-origin", ORIGIN, "--slow-ms", "0"])
            .current_dir(home)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the hub starts");
        let pid = child.id();
        let stdout = child.stdout.take().expect("stdout");
        let mut pipe = child.stderr.take().expect("stderr");
        let stderr = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&stderr);
        thread::spawn(move || {
            let mut buffer = [0u8; 8192];
            loop {
                match pipe.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => sink.lock().expect("sink").extend_from_slice(&buffer[..n]),
                }
            }
        });
        let (sender, lines) = mpsc::channel::<String>();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if sender.send(line).is_err() {
                    break;
                }
            }
        });
        let port = loop {
            let line = lines
                .recv_timeout(Duration::from_secs(120))
                .unwrap_or_else(|error| {
                    let _ = child.kill();
                    panic!(
                        "no `listening` line ({error}): {}",
                        String::from_utf8_lossy(&stderr.lock().expect("stderr"))
                    )
                });
            if let Some(port) = line.strip_prefix("listening http://127.0.0.1:") {
                break port.parse::<u16>().expect("a port");
            }
        };
        Self {
            child,
            port,
            pid,
            stderr,
        }
    }

    pub fn stderr(&self) -> String {
        String::from_utf8_lossy(&self.stderr.lock().expect("stderr")).into_owned()
    }

    /// Every complete request line on stderr, parsed.
    pub fn requests(&self) -> Vec<Line> {
        let stderr = self.stderr();
        let complete = stderr.rfind('\n').map_or("", |end| &stderr[..end]);
        complete
            .lines()
            .filter_map(|line| line.strip_prefix("specengine-http: slow request "))
            .map(Line::parse)
            .collect()
    }

    /// The request lines of `template`, once `n` are on stderr (within 60 s).
    pub fn requests_of(&self, template: &str, n: usize) -> Vec<Line> {
        let started = Instant::now();
        loop {
            let lines: Vec<Line> = self
                .requests()
                .into_iter()
                .filter(|line| line.route == template)
                .collect();
            if lines.len() >= n {
                return lines;
            }
            assert!(
                started.elapsed() < Duration::from_secs(60),
                "{n} lines of {template} not on stderr:\n{}",
                self.stderr()
            );
            thread::sleep(Duration::from_millis(20));
        }
    }

    /// The daemon's resident set, KiB (`ps -o rss= -p <pid>`).
    pub fn rss_kib(&self) -> Option<u64> {
        let output = Command::new("/bin/ps")
            .args(["-o", "rss=", "-p", &self.pid.to_string()])
            .output()
            .ok()?;
        String::from_utf8_lossy(&output.stdout).trim().parse().ok()
    }

    /// `sample <pid> 5` into a temp file kept after the run (macOS), its
    /// path; elsewhere, nothing.
    pub fn sample(&self, label: &str) -> Option<PathBuf> {
        if !cfg!(target_os = "macos") || !Path::new("/usr/bin/sample").exists() {
            return None;
        }
        let dir = std::env::temp_dir().join("specengine-library-soak-samples");
        fs::create_dir_all(&dir).ok()?;
        let file = dir.join(format!("{label}-{}-{}.txt", self.pid, unix_secs()));
        let status = Command::new("/usr/bin/sample")
            .arg(self.pid.to_string())
            .arg("5")
            .arg("-file")
            .arg(&file)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .ok()?;
        status.success().then_some(file)
    }
}

impl Drop for Hub {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

// ------------------------------------------------------------- requests

/// One answer: its status, its body, its time.
#[derive(Debug, Clone)]
pub struct Answer {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub elapsed: Duration,
}

impl Answer {
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(held, _)| held.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

/// The proxy's headers of a page's same-origin `fetch` by `login`.
pub fn page_headers(login: &str) -> Vec<(String, String)> {
    let mut headers: Vec<(String, String)> = [
        ("Host", AUTHORITY),
        ("Sec-Fetch-Site", "same-origin"),
        ("Sec-Fetch-Mode", "cors"),
        ("Sec-Fetch-Dest", "empty"),
        ("Remote-User", login),
    ]
    .iter()
    .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
    .collect();
    headers.extend(forwarded());
    headers
}

/// Traefik's `X-Forwarded-*`.
fn forwarded() -> Vec<(String, String)> {
    [
        ("X-Forwarded-For", "203.0.113.7"),
        ("X-Forwarded-Host", AUTHORITY),
        ("X-Forwarded-Port", "443"),
        ("X-Forwarded-Proto", "https"),
        ("X-Forwarded-Server", "edge"),
    ]
    .iter()
    .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
    .collect()
}

/// `method path` with `headers` and `body` on `port`, `Connection: close`,
/// read to its end; `Err` when no answer came within `timeout`.
pub fn exchange(
    port: u16,
    method: &str,
    path: &str,
    headers: &[(String, String)],
    body: &[u8],
    timeout: Duration,
) -> Result<Answer, String> {
    let started = Instant::now();
    let mut stream =
        TcpStream::connect(("127.0.0.1", port)).map_err(|error| format!("connect: {error}"))?;
    stream
        .set_read_timeout(Some(timeout))
        .map_err(|error| error.to_string())?;
    let mut request = format!("{method} {path} HTTP/1.1\r\n");
    for (name, value) in headers {
        request.push_str(&format!("{name}: {value}\r\n"));
    }
    if !body.is_empty() {
        request.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    request.push_str("Connection: close\r\n\r\n");
    let mut bytes = request.into_bytes();
    bytes.extend_from_slice(body);
    stream
        .write_all(&bytes)
        .map_err(|error| format!("write: {error}"))?;
    let mut raw = Vec::new();
    stream
        .read_to_end(&mut raw)
        .map_err(|error| format!("read after {:?}: {error}", started.elapsed()))?;
    let elapsed = started.elapsed();
    let end = raw
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| format!("no head: {}", String::from_utf8_lossy(&raw)))?;
    let head = String::from_utf8_lossy(&raw[..end]).into_owned();
    let mut lines = head.split("\r\n");
    let status = lines
        .next()
        .and_then(|line| line.split(' ').nth(1))
        .and_then(|code| code.parse().ok())
        .ok_or_else(|| format!("no status: {head}"))?;
    let headers: Vec<(String, String)> = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim().to_owned()))
        .collect();
    let mut answer = Answer {
        status,
        headers,
        body: raw[end + 4..].to_vec(),
        elapsed,
    };
    if answer
        .header("transfer-encoding")
        .is_some_and(|value| value.eq_ignore_ascii_case("chunked"))
    {
        answer.body = unchunked(&answer.body)?;
    }
    Ok(answer)
}

fn unchunked(mut body: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    loop {
        let end = body
            .windows(2)
            .position(|window| window == b"\r\n")
            .ok_or("a chunk with no size line")?;
        let size_text = String::from_utf8_lossy(&body[..end]);
        let size = usize::from_str_radix(size_text.split(';').next().unwrap_or("").trim(), 16)
            .map_err(|error| format!("a chunk size {size_text:?}: {error}"))?;
        body = &body[end + 2..];
        if size == 0 {
            return Ok(out);
        }
        if body.len() < size + 2 {
            return Err("a chunk cut short".to_owned());
        }
        out.extend_from_slice(&body[..size]);
        body = &body[size + 2..];
    }
}

/// A page's `GET path` by `login`.
pub fn page_get(port: u16, path: &str, login: &str, timeout: Duration) -> Result<Answer, String> {
    exchange(port, "GET", path, &page_headers(login), b"", timeout)
}

/// The stateless era's protocol version (`common/mcp_http.rs`).
pub const STATELESS: &str = "2026-07-28";

/// A stateless MCP `tools/call` of `tool` with `arguments` by `login` on
/// `/mcp/<slug>`: the HTTP answer and the JSON-RPC response to it, if any.
pub fn mcp_call(
    port: u16,
    slug: &str,
    login: &str,
    id: i64,
    tool: &str,
    arguments: Value,
    timeout: Duration,
) -> Result<(Answer, Option<Value>), String> {
    let params = json!({
        "name": tool,
        "arguments": arguments,
        "_meta": {
            "io.modelcontextprotocol/protocolVersion": STATELESS,
            "io.modelcontextprotocol/clientCapabilities": {},
            "io.modelcontextprotocol/clientInfo": {
                "name": "specengine-eval-soak",
                "version": "0.0.0"
            }
        }
    });
    let message = json!({"jsonrpc": "2.0", "id": id, "method": "tools/call", "params": params});
    let mut headers: Vec<(String, String)> = vec![
        ("Host".to_owned(), AUTHORITY.to_owned()),
        ("Remote-User".to_owned(), login.to_owned()),
    ];
    headers.extend(forwarded());
    for (name, value) in [
        ("Content-Type", "application/json"),
        ("Accept", "application/json, text/event-stream"),
        ("MCP-Protocol-Version", STATELESS),
        ("Mcp-Method", "tools/call"),
        ("Mcp-Name", tool),
    ] {
        headers.push((name.to_owned(), value.to_owned()));
    }
    let answer = exchange(
        port,
        "POST",
        &format!("/mcp/{slug}"),
        &headers,
        message.to_string().as_bytes(),
        timeout,
    )?;
    let messages: Vec<Value> = if answer
        .header("content-type")
        .is_some_and(|kind| kind.starts_with("text/event-stream"))
    {
        let text = answer.text().replace("\r\n", "\n");
        text.split("\n\n")
            .filter_map(|block| {
                let data: Vec<&str> = block
                    .lines()
                    .filter_map(|line| line.strip_prefix("data:"))
                    .map(|rest| rest.strip_prefix(' ').unwrap_or(rest))
                    .collect();
                let joined = data.join("\n");
                (!joined.trim().is_empty())
                    .then(|| serde_json::from_str(&joined).ok())
                    .flatten()
            })
            .collect()
    } else {
        match serde_json::from_slice::<Value>(&answer.body) {
            Ok(Value::Array(items)) => items,
            Ok(value) => vec![value],
            Err(_) => Vec::new(),
        }
    };
    let response = messages
        .into_iter()
        .find(|message| message.get("id") == Some(&json!(id)) && message.get("method").is_none());
    Ok((answer, response))
}

// ------------------------------------------------------------ the lines

/// One `slow request` line, by key.
#[derive(Debug, Clone)]
pub struct Line {
    pub raw: String,
    pub method: String,
    pub route: String,
    pub status: String,
    pub ms: u64,
    pub run_ms: Option<u64>,
    pub index: String,
    pub git: Option<u64>,
    /// The MCP tool a `/mcp` line names, else `-`.
    pub tool: String,
    /// The wait for the project's turn, `None` for `-`.
    pub wait_ms: Option<u64>,
    /// The request's own git time, `None` for `-`.
    pub git_ms: Option<u64>,
}

impl Line {
    fn parse(rest: &str) -> Self {
        let pairs: BTreeMap<&str, &str> = rest
            .split(' ')
            .filter_map(|pair| pair.split_once('='))
            .collect();
        let number = |key: &str| pairs.get(key).and_then(|value| value.parse::<u64>().ok());
        Self {
            raw: rest.to_owned(),
            method: pairs.get("method").copied().unwrap_or("?").to_owned(),
            route: pairs.get("route").copied().unwrap_or("-").to_owned(),
            status: pairs.get("status").copied().unwrap_or("?").to_owned(),
            ms: number("ms").unwrap_or_else(|| panic!("a line with no ms: {rest}")),
            run_ms: number("run_ms"),
            index: pairs.get("index").copied().unwrap_or("-").to_owned(),
            git: number("git"),
            tool: pairs.get("tool").copied().unwrap_or("-").to_owned(),
            wait_ms: number("wait_ms"),
            git_ms: number("git_ms"),
        }
    }
}

// ---------------------------------------------------------- the numbers

/// The `q`-quantile (0..=1) of `values` by nearest rank; 0 when empty.
pub fn quantile(values: &[u64], q: f64) -> u64 {
    if values.is_empty() {
        return 0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let rank = (q * sorted.len() as f64).ceil() as usize;
    sorted[rank.clamp(1, sorted.len()) - 1]
}

/// `{p50, p95, p99, worst}` of `values`.
pub fn spread(values: &[u64]) -> Value {
    json!({
        "p50": quantile(values, 0.50),
        "p95": quantile(values, 0.95),
        "p99": quantile(values, 0.99),
        "worst": values.iter().copied().max().unwrap_or(0),
    })
}

/// The median of `values` (the lower middle of an even count).
pub fn median(values: &[u64]) -> u64 {
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    sorted[(sorted.len() - 1) / 2]
}

/// The Theil-Sen slope of `(seconds, KiB)` samples, MiB per minute: the
/// median of every pairwise slope.
pub fn theil_sen_mib_per_minute(samples: &[(f64, u64)]) -> f64 {
    let mut slopes = Vec::new();
    for (i, (t1, y1)) in samples.iter().enumerate() {
        for (t2, y2) in &samples[i + 1..] {
            let dt = t2 - t1;
            if dt > 0.0 {
                slopes.push((*y2 as f64 - *y1 as f64) / dt);
            }
        }
    }
    if slopes.is_empty() {
        return 0.0;
    }
    slopes.sort_by(f64::total_cmp);
    let middle = slopes.len() / 2;
    let kib_per_second = if slopes.len() % 2 == 0 {
        (slopes[middle - 1] + slopes[middle]) / 2.0
    } else {
        slopes[middle]
    };
    kib_per_second * 60.0 / 1024.0
}

/// A word made of syllables drawn from `seed`: text no other raise shares.
pub fn nonce_words(seed: u64, words: usize) -> String {
    const SYLLABLES: [&str; 16] = [
        "ka", "lo", "mi", "ne", "pu", "ra", "si", "to", "vu", "ze", "bo", "di", "fe", "gu", "ha",
        "jo",
    ];
    let mut rng = Rng(seed ^ 0x005e_ed0f_11b2);
    (0..words)
        .map(|_| {
            (0..4)
                .map(|_| SYLLABLES[rng.below(SYLLABLES.len())])
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// -------------------------------------------------------------- tracking

/// The requests in flight, each with its start, for the hang watchdog.
#[derive(Default)]
pub struct InFlight {
    next: Mutex<u64>,
    open: Mutex<BTreeMap<u64, (Instant, String)>>,
}

impl InFlight {
    pub fn begin(&self, what: &str) -> u64 {
        let mut next = self.next.lock().expect("next");
        *next += 1;
        self.open
            .lock()
            .expect("open")
            .insert(*next, (Instant::now(), what.to_owned()));
        *next
    }

    pub fn end(&self, id: u64) {
        self.open.lock().expect("open").remove(&id);
    }

    /// The requests open longer than `limit`, with what they are.
    pub fn older_than(&self, limit: Duration) -> Vec<(u64, String)> {
        self.open
            .lock()
            .expect("open")
            .iter()
            .filter(|(_, (since, _))| since.elapsed() >= limit)
            .map(|(id, (_, what))| (*id, what.clone()))
            .collect()
    }
}
