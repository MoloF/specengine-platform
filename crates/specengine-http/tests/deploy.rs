//! docs/features/hub-deploy.md (RM-0064, Phase 7 step 11b-1) D1-D15: the
//! deployment's files in `scripts/deploy/` and `.github/workflows/image.yml`
//! as committed, read with no network, no image build and no Docker. YAML is
//! parsed with `serde-saphyr` into `serde_json::Value` (its re-exported event
//! parser finds anchors, aliases and merge keys), the root `Cargo.toml` with
//! `toml`. Trust item 5's rule is the one fenced `^/api` line of
//! `docs/canon/hub.md` "Trust", item 6's the one fenced `^/` line of
//! `docs/canon/hub-mcp.md` "Trust", read as `src/hub.rs`'s units read them.
//! The scripts run under `sh` in scratch folders: `entrypoint.sh` against a
//! stub `specengine-http` that prints its arguments one per line and leaves a
//! mark; `tag-check.sh` in place and in a scratch copy of its tree.
//!
//! A router "to the daemon" is one whose service is `specengine` ("Data");
//! its middlewares are read through any `chain`, so a router carries "the
//! door" when it meets a `forwardAuth` and "the credential" when it meets a
//! `basicAuth`, whatever their names. A router's priority is its own or, as
//! Traefik defaults it, its rule's length.
//!
//! Beside the criteria, from "Data": the two-factor rule has no condition
//! but its domain and its resources ("by its regex alone"); a wildcard or
//! regex domain counts as naming the hub's host; the dynamic folder holds
//! `hub.yml` alone, and `hub.yml` an `http` section alone (any other file or
//! a `tcp` router would be loaded beside it); every Traefik service is a
//! load balancer and only `specengine`'s points at the loopback; no
//! `TRAEFIK_*` variable adds static configuration; no service mounts an
//! absolute host path; every `{{ ... }}` template is a plain `env` read; the
//! Dockerfile has three stages, sets `XDG_DATA_HOME=/data` and
//! `HOME=/var/lib/specengine/home`, installs the two scripts under their
//! image names and runs the entrypoint; its context keeps every manifest, `Cargo.lock`,
//! `crates/`, `ui/` and the two scripts, and every source a stage copies
//! from it, each an existing path, and drops `ui/dist`;
//! `.gitignore` keeps every committed deploy file; `image.yml` runs
//! `tag-check.sh` as its check job's first step, names no secret but
//! `GITHUB_TOKEN`, comments each pinned `uses:` with its version, and uses no
//! Rust toolchain action.
//!
//! D7, D9 and D11 are as docs/features/hub-accounts.md "Acceptance criteria"
//! (RM-0065, its decided changes to 11b-1's tests) rewrites them, for the one-shot `admin` service;
//! that slice's own criteria are `tests/deploy_admin.rs`.
//!
//! Iteration 2's hardenings: Traefik rejects alias headers (D4); the runtime
//! carries the source label (D14); every pushing job refuses a published
//! version before it pushes, its check's step run as a runner runs it with a
//! stub `docker` (D15); `build` and `merge` run on the owner's repository
//! alone (D15).
//!
//! Where a criterion cannot run as written: D12's folders `b` and `B` are one
//! folder on a case-insensitive file system (macOS's default), so `c` stands
//! for `b`; byte order is then `B a a-1 c`, a collating locale's `a a-1 B c`.
//! The entrypoint runs with `LC_ALL` and `LANG` set to `en_US.UTF-8`, so only
//! the script's own `LC_ALL=C` gives byte order.

#![cfg(unix)]

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs;
use std::io::Read as _;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use regex::Regex;
use serde_json::{Map, Value, json};
use serde_saphyr::granit_parser::{Event, Parser, ScalarStyle};

/// How Traefik's and Authelia's files name the hub's host.
const HUB_HOST: &str = r#"{{ env "HUB_HOST" }}"#;

/// The Traefik service, and the compose service, of the daemon.
const DAEMON: &str = "specengine";

/// The one-shot account service (docs/features/hub-accounts.md "Data").
const ADMIN: &str = "admin";

/// The host D12 gives the entrypoint.
const HOST: &str = "hub.example.org";

/// A script run longer than this is killed and the test fails.
const RUN_TIMEOUT: Duration = Duration::from_secs(60);

// ------------------------------------------------------------- the files

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root")
}

fn deploy() -> PathBuf {
    repository().join("scripts/deploy")
}

fn workflow_path() -> PathBuf {
    repository().join(".github/workflows/image.yml")
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn yaml(path: &Path) -> Value {
    serde_saphyr::from_str(&read(path))
        .unwrap_or_else(|error| panic!("{}: no YAML document: {error}", path.display()))
}

fn compose() -> Value {
    yaml(&deploy().join("docker-compose.yml"))
}

fn dynamic() -> Value {
    yaml(&deploy().join("traefik/dynamic/hub.yml"))
}

fn door() -> Value {
    yaml(&deploy().join("authelia/config/configuration.yml"))
}

fn workflow() -> Value {
    yaml(&workflow_path())
}

/// The one fenced line of `docs/canon/<page>`'s "Trust" section that opens
/// with `^/`; it must open with `prefix`.
fn trust_rule(page: &str, prefix: &str) -> String {
    let path = repository().join("docs/canon").join(page);
    let text = read(&path);
    let mut in_section = false;
    let mut in_fence = false;
    let mut found = Vec::new();
    for line in text.lines() {
        if line.starts_with("## ") {
            in_section = line.trim_end() == "## Trust";
            in_fence = false;
            continue;
        }
        let line = line.trim();
        if !in_section {
            continue;
        }
        if line.starts_with("```") {
            in_fence = !in_fence;
        } else if in_fence && line.starts_with("^/") {
            found.push(line.to_owned());
        }
    }
    assert_eq!(
        found.len(),
        1,
        "one fenced `^/` line in {} \"Trust\": {found:?}",
        path.display()
    );
    let rule = found.remove(0);
    assert!(rule.starts_with(prefix), "{}: {rule}", path.display());
    rule
}

/// `.env.example`'s `KEY=value` lines, in order; comments on their own lines.
fn env_example() -> Vec<(String, String)> {
    let path = deploy().join(".env.example");
    read(&path)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let (key, value) = line
                .split_once('=')
                .unwrap_or_else(|| panic!("{}: no `KEY=value`: {line}", path.display()));
            (key.to_owned(), value.to_owned())
        })
        .collect()
}

fn plugin_version() -> String {
    let path = repository().join("plugin/specengine/.claude-plugin/plugin.json");
    let plugin: Value = serde_json::from_str(&read(&path)).expect("plugin.json is JSON");
    string(at(&plugin, &["version"]), "plugin.json's version").to_owned()
}

// ------------------------------------------------------------- values

/// `value` at `path`, a key per step; a missing step panics naming it.
fn at<'a>(value: &'a Value, path: &[&str]) -> &'a Value {
    let mut here = value;
    for (step, key) in path.iter().enumerate() {
        here = here
            .get(key)
            .unwrap_or_else(|| panic!("no `{}`", path[..=step].join(".")));
    }
    here
}

fn find<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    path.iter().try_fold(value, |here, key| here.get(key))
}

fn object<'a>(value: &'a Value, what: &str) -> &'a Map<String, Value> {
    value
        .as_object()
        .unwrap_or_else(|| panic!("{what} is no mapping: {value}"))
}

fn list<'a>(value: &'a Value, what: &str) -> &'a [Value] {
    value
        .as_array()
        .unwrap_or_else(|| panic!("{what} is no list: {value}"))
}

fn string<'a>(value: &'a Value, what: &str) -> &'a str {
    value
        .as_str()
        .unwrap_or_else(|| panic!("{what} is no string: {value}"))
}

/// A string, or a list of strings, as a list.
fn strings(value: &Value, what: &str) -> Vec<String> {
    match value {
        Value::String(one) => vec![one.clone()],
        Value::Array(items) => items
            .iter()
            .map(|item| string(item, what).to_owned())
            .collect(),
        other => panic!("{what} is neither a string nor a list: {other}"),
    }
}

/// A scalar as text: a string, a number, a bool; null is empty.
fn scalar(value: &Value, what: &str) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(flag) => flag.to_string(),
        Value::Null => String::new(),
        other => panic!("{what} is no scalar: {other}"),
    }
}

fn is_true(value: Option<&Value>) -> bool {
    matches!(value, Some(Value::Bool(true))) || value.and_then(Value::as_str) == Some("true")
}

fn is_false(value: Option<&Value>) -> bool {
    matches!(value, Some(Value::Bool(false))) || value.and_then(Value::as_str) == Some("false")
}

/// Every string value under `value` (keys are not values).
fn every_string(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::String(text) => out.push(text.clone()),
        Value::Array(items) => items.iter().for_each(|item| every_string(item, out)),
        Value::Object(map) => map.values().for_each(|item| every_string(item, out)),
        _ => {}
    }
}

/// What in `text` is a YAML anchor, an alias or a merge key, with its line.
fn anchors_aliases_merges(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    for event in Parser::new_from_str(text) {
        let (event, span) = event.unwrap_or_else(|error| panic!("YAML: {error:?}"));
        let line = span.start.line();
        match event {
            Event::Alias(_) => found.push(format!("an alias at line {line}")),
            Event::Scalar(value, style, anchor, _) => {
                if anchor != 0 {
                    found.push(format!("an anchor at line {line}"));
                }
                if value == "<<" && matches!(style, ScalarStyle::Plain) {
                    found.push(format!("a merge key at line {line}"));
                }
            }
            Event::SequenceStart(_, anchor, _) | Event::MappingStart(_, anchor, _)
                if anchor != 0 =>
            {
                found.push(format!("an anchor at line {line}"));
            }
            _ => {}
        }
    }
    found
}

/// Every `.yml` or `.yaml` file under `scripts/deploy/`, and `image.yml`.
fn yaml_files() -> Vec<PathBuf> {
    let mut files = vec![workflow_path()];
    let mut stack = vec![deploy()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(dir).expect("a deploy folder").flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path
                .extension()
                .is_some_and(|ext| ext == "yml" || ext == "yaml")
            {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

// ------------------------------------------------------------- Traefik

/// One middleware as a router meets it, a chain unfolded.
#[derive(Debug)]
struct Middleware {
    name: String,
    kind: String,
    config: Value,
}

#[derive(Debug)]
struct Router {
    name: String,
    rule: String,
    service: String,
    priority: i64,
    middlewares: Vec<Middleware>,
    entry_points: Option<Value>,
    tls: Option<Value>,
}

impl Router {
    fn to_daemon(&self) -> bool {
        self.service == DAEMON
    }

    fn of(&self, kind: &str) -> Vec<&Middleware> {
        self.middlewares
            .iter()
            .filter(|middleware| middleware.kind == kind)
            .collect()
    }

    fn names(&self) -> Vec<&str> {
        self.middlewares
            .iter()
            .map(|middleware| middleware.name.as_str())
            .collect()
    }
}

/// A Traefik reference without its `@provider`.
fn unprovided(name: &str) -> &str {
    name.split_once('@').map_or(name, |(bare, _)| bare)
}

/// The one key of a Traefik definition, its kind, and its configuration.
fn kind_of<'a>(definition: &'a Value, what: &str) -> (&'a str, &'a Value) {
    let map = object(definition, what);
    assert_eq!(map.len(), 1, "{what}: one kind: {definition}");
    let (kind, config) = map.iter().next().expect("one kind");
    (kind.as_str(), config)
}

fn unfold(dynamic: &Value, name: &str, depth: usize, out: &mut Vec<Middleware>) {
    assert!(depth < 8, "a middleware chain loops at {name}");
    let name = unprovided(name);
    let definition = at(dynamic, &["http", "middlewares", name]);
    let (kind, config) = kind_of(definition, name);
    if kind == "chain" {
        for inner in strings(at(config, &["middlewares"]), name) {
            unfold(dynamic, &inner, depth + 1, out);
        }
    } else {
        out.push(Middleware {
            name: name.to_owned(),
            kind: kind.to_owned(),
            config: config.clone(),
        });
    }
}

fn routers(dynamic: &Value) -> Vec<Router> {
    let routers = object(at(dynamic, &["http", "routers"]), "http.routers");
    assert!(!routers.is_empty(), "no router in hub.yml");
    routers
        .iter()
        .map(|(name, router)| {
            let rule = string(at(router, &["rule"]), name).to_owned();
            let service = unprovided(string(at(router, &["service"]), name)).to_owned();
            let priority = router.get("priority").map_or_else(
                || i64::try_from(rule.len()).expect("a rule's length"),
                |priority| {
                    priority
                        .as_i64()
                        .unwrap_or_else(|| panic!("{name}: priority {priority}"))
                },
            );
            let mut middlewares = Vec::new();
            if let Some(names) = router.get("middlewares") {
                for middleware in strings(names, name) {
                    unfold(dynamic, &middleware, 0, &mut middlewares);
                }
            }
            Router {
                name: name.clone(),
                rule,
                service,
                priority,
                middlewares,
                entry_points: router.get("entryPoints").cloned(),
                tls: router.get("tls").cloned(),
            }
        })
        .collect()
}

/// The quoted argument of every `call(` in `rule`, in order.
fn rule_arguments(rule: &str, call: &str) -> Vec<String> {
    let opening = format!("{call}(");
    let mut found = Vec::new();
    let mut rest = rule;
    while let Some(at) = rest.find(&opening) {
        let whole = rest[..at]
            .chars()
            .last()
            .is_none_or(|before| !before.is_alphanumeric());
        let after = &rest[at + opening.len()..];
        if whole {
            let argument = after.trim_start();
            let quote = argument.chars().next().unwrap_or(' ');
            assert!(
                quote == '`' || quote == '"',
                "{call}'s argument is quoted: {rule}"
            );
            let body = &argument[1..];
            let end = body
                .find(quote)
                .unwrap_or_else(|| panic!("a closed argument: {rule}"));
            found.push(body[..end].to_owned());
        }
        rest = after;
    }
    found
}

// ------------------------------------------------------------- compose

fn services(compose: &Value) -> &Map<String, Value> {
    object(at(compose, &["services"]), "services")
}

fn service<'a>(compose: &'a Value, name: &str) -> &'a Value {
    at(compose, &["services", name])
}

/// Traefik's static configuration: its `command:` list (S1).
fn command(compose: &Value) -> Vec<String> {
    strings(
        at(service(compose, "traefik"), &["command"]),
        "traefik's command",
    )
}

/// Each flag of `command` as `(name lowercased, value)`; a bare flag is
/// `true`.
fn flags(command: &[String]) -> Vec<(String, String)> {
    command
        .iter()
        .map(|flag| match flag.split_once('=') {
            Some((name, value)) => (name.to_ascii_lowercase(), value.to_owned()),
            None => (flag.to_ascii_lowercase(), "true".to_owned()),
        })
        .collect()
}

fn flag_values(flags: &[(String, String)], name: &str) -> Vec<String> {
    flags
        .iter()
        .filter(|(flag, _)| flag == name)
        .map(|(_, value)| value.clone())
        .collect()
}

/// A service's `environment`, a mapping or a `NAME=value` list.
fn environment(service: &Value, what: &str) -> BTreeMap<String, String> {
    match service.get("environment") {
        None => BTreeMap::new(),
        Some(Value::Object(map)) => map
            .iter()
            .map(|(name, value)| (name.clone(), scalar(value, name)))
            .collect(),
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| {
                let text = string(item, what);
                let (name, value) = text.split_once('=').unwrap_or((text, ""));
                (name.to_owned(), value.to_owned())
            })
            .collect(),
        Some(other) => panic!("{what}'s environment: {other}"),
    }
}

/// `(source, target)` of each of a service's volumes, short or long syntax;
/// an anonymous volume's source is empty.
fn mounts(service: &Value, what: &str) -> Vec<(String, String)> {
    let Some(volumes) = service.get("volumes") else {
        return Vec::new();
    };
    list(volumes, what)
        .iter()
        .map(|volume| match volume {
            Value::String(short) => {
                let parts: Vec<&str> = short.split(':').collect();
                match parts.as_slice() {
                    [target] => (String::new(), (*target).to_owned()),
                    [source, target, ..] => ((*source).to_owned(), (*target).to_owned()),
                    [] => panic!("{what}: an empty volume"),
                }
            }
            Value::Object(long) => (
                long.get("source")
                    .map(|source| scalar(source, what))
                    .unwrap_or_default(),
                scalar(
                    long.get("target")
                        .unwrap_or_else(|| panic!("{what}: a volume with no target")),
                    what,
                ),
            ),
            other => panic!("{what}: a volume {other}"),
        })
        .collect()
}

/// The secrets a service is granted.
fn grants(service: &Value, what: &str) -> BTreeSet<String> {
    let Some(secrets) = service.get("secrets") else {
        return BTreeSet::new();
    };
    list(secrets, what)
        .iter()
        .map(|grant| match grant {
            Value::String(name) => name.clone(),
            Value::Object(long) => scalar(
                long.get("source")
                    .unwrap_or_else(|| panic!("{what}: a grant with no source")),
                what,
            ),
            other => panic!("{what}: a grant {other}"),
        })
        .collect()
}

/// The `${NAME...}` and `$NAME` reads of `text`, `$$` an escape.
fn interpolated(text: &str) -> Vec<String> {
    let text = text.replace("$$", "");
    let read = Regex::new(r"\$\{?([A-Za-z_][A-Za-z0-9_]*)").expect("a pattern");
    read.captures_iter(&text)
        .map(|found| found[1].to_owned())
        .collect()
}

/// A Go duration, `300s`, `5m`, `1h0m0s` or a bare count of seconds, in
/// seconds.
fn seconds(duration: &str) -> f64 {
    if let Ok(bare) = duration.parse::<f64>() {
        return bare;
    }
    let part = Regex::new(r"([0-9.]+)(ms|h|m|s)").expect("a pattern");
    let mut total = 0.0;
    let mut covered = 0;
    for found in part.captures_iter(duration) {
        let count: f64 = found[1].parse().expect("a number");
        total += count
            * match &found[2] {
                "h" => 3600.0,
                "m" => 60.0,
                "s" => 1.0,
                _ => 0.001,
            };
        covered += found[0].len();
    }
    assert_eq!(covered, duration.len(), "a duration: {duration}");
    total
}

// ------------------------------------------------------------- processes

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// A scratch directory under the system temp dir, canonical, removed on drop.
struct Scratch {
    root: PathBuf,
}

impl Scratch {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "specengine-deploy-{label}-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("a scratch directory");
        Self {
            root: path.canonicalize().expect("a canonical scratch"),
        }
    }

    fn dir(&self, relative: &str) -> PathBuf {
        let path = self.root.join(relative);
        fs::create_dir_all(&path).expect("a scratch folder");
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

struct Ran {
    label: String,
    code: i32,
    stdout: String,
    stderr: String,
}

impl Ran {
    fn show(&self) -> String {
        format!(
            "{}: exit {}\n--- stdout\n{}--- stderr\n{}",
            self.label, self.code, self.stdout, self.stderr
        )
    }

    fn lines(&self) -> Vec<&str> {
        self.stdout.lines().collect()
    }

    /// The stderr lines the script writes itself, `<prefix>: ...`.
    fn own(&self, prefix: &str) -> Vec<&str> {
        self.stderr
            .lines()
            .filter(|line| line.starts_with(prefix))
            .collect()
    }
}

/// `program args` in `cwd` with exactly `env`, stdin null, under a watchdog.
fn run(
    label: &str,
    program: &str,
    args: &[OsString],
    cwd: &Path,
    env: &[(String, OsString)],
) -> Ran {
    let mut command = Command::new(program);
    command
        .env_clear()
        .current_dir(cwd)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (name, value) in env {
        command.env(name, value);
    }
    let mut child = command
        .spawn()
        .unwrap_or_else(|error| panic!("{label}: {error}"));
    let mut stdout = child.stdout.take().expect("stdout");
    let mut stderr = child.stderr.take().expect("stderr");
    let out = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stdout.read_to_end(&mut bytes);
        bytes
    });
    let err = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stderr.read_to_end(&mut bytes);
        bytes
    });
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().expect("wait") {
            break status;
        }
        if started.elapsed() > RUN_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            panic!("{label} ran longer than {RUN_TIMEOUT:?}; killed");
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    Ran {
        label: label.to_owned(),
        code: status
            .code()
            .unwrap_or_else(|| panic!("{label}: killed by a signal")),
        stdout: String::from_utf8(out.join().expect("stdout")).expect("UTF-8 stdout"),
        stderr: String::from_utf8(err.join().expect("stderr")).expect("UTF-8 stderr"),
    }
}

/// A scratch projects folder for `entrypoint.sh`, and a stub
/// `specengine-http` first on `PATH` that prints its arguments one per
/// line and leaves a mark.
struct Entry {
    scratch: Scratch,
    projects: PathBuf,
    bin: PathBuf,
    home: PathBuf,
    mark: PathBuf,
}

impl Entry {
    fn new(label: &str) -> Self {
        let scratch = Scratch::new(label);
        let projects = scratch.dir("projects");
        let bin = scratch.dir("bin");
        let home = scratch.dir("home");
        let mark = scratch.root.join("stub-ran");
        let stub = bin.join("specengine-http");
        fs::write(
            &stub,
            format!(
                "#!/bin/sh\n: > '{}'\nfor argument in \"$@\"; do printf '%s\\n' \"$argument\"; done\n",
                mark.display()
            ),
        )
        .expect("the stub");
        fs::set_permissions(stub, fs::Permissions::from_mode(0o755)).expect("an executable stub");
        Self {
            scratch,
            projects,
            bin,
            home,
            mark,
        }
    }

    /// A folder holding a regular `specengine.toml`.
    fn project(&self, name: &str) {
        let folder = self.projects.join(name);
        fs::create_dir_all(&folder).expect("a project folder");
        fs::write(folder.join("specengine.toml"), "").expect("a config");
    }

    fn folder(&self, relative: &str) {
        fs::create_dir_all(self.projects.join(relative)).expect("a folder");
    }

    fn dir(&self) -> String {
        self.projects.display().to_string()
    }

    /// `sh entrypoint.sh` with `HUB_HOST` and `SPECENGINE_PUSH` as given
    /// (`None`: unset), and whether the stub ran.
    fn run(&self, hub_host: Option<&str>, push: Option<&str>) -> (Ran, bool) {
        let _ = fs::remove_file(&self.mark);
        let mut path = OsString::from(&self.bin);
        path.push(":/usr/bin:/bin");
        let mut env: Vec<(String, OsString)> = vec![
            ("PATH".to_owned(), path),
            ("HOME".to_owned(), self.home.clone().into()),
            // A collating locale: only the script's own `LC_ALL=C` gives
            // byte order.
            ("LANG".to_owned(), "en_US.UTF-8".into()),
            ("LC_ALL".to_owned(), "en_US.UTF-8".into()),
            (
                "SPECENGINE_PROJECTS".to_owned(),
                self.projects.clone().into(),
            ),
        ];
        if let Some(host) = hub_host {
            env.push(("HUB_HOST".to_owned(), host.into()));
        }
        if let Some(push) = push {
            env.push(("SPECENGINE_PUSH".to_owned(), push.into()));
        }
        let script = deploy().join("entrypoint.sh");
        let ran = run(
            &format!("entrypoint HUB_HOST={hub_host:?} SPECENGINE_PUSH={push:?}"),
            "/bin/sh",
            &[script.into_os_string()],
            &self.scratch.root,
            &env,
        );
        let stub_ran = self.mark.exists();
        (ran, stub_ran)
    }

    /// The daemon's arguments for `roots`, in order.
    fn expected(&self, roots: &[&str], push: bool) -> Vec<String> {
        let mut args = Vec::new();
        for root in roots {
            args.push("--root".to_owned());
            args.push(format!("{}/{root}", self.dir()));
        }
        for arg in [
            "--public-origin",
            &format!("https://{HOST}"),
            "--port",
            "7777",
        ] {
            args.push(arg.to_owned());
        }
        if push {
            args.push("--push".to_owned());
        }
        args
    }
}

/// Every path under `dir`, relative, sorted: what a run may not change.
fn listing(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(here) = stack.pop() {
        for entry in fs::read_dir(here).expect("a folder").flatten() {
            let path = entry.path();
            out.push(
                path.strip_prefix(dir)
                    .expect("under the folder")
                    .display()
                    .to_string(),
            );
            if path.is_dir() {
                stack.push(path);
            }
        }
    }
    out.sort();
    out
}

const ENTRYPOINT: &str = "specengine-entrypoint:";

// ------------------------------------------------------------- D1-D3: the door

/// The rules of Authelia's `access_control`.
fn access_rules(door: &Value) -> Vec<Value> {
    list(
        at(door, &["access_control", "rules"]),
        "access_control.rules",
    )
    .to_vec()
}

/// Whether a rule can reach the hub's host: it names it, or a wildcard or a
/// regex domain could match it.
fn names_hub_host(rule: &Value) -> bool {
    rule.get("domain_regex").is_some()
        || rule.get("domain").is_some_and(|domain| {
            strings(domain, "a rule's domain")
                .iter()
                .any(|domain| domain == HUB_HOST || domain.contains('*'))
        })
}

fn policy(rule: &Value) -> &str {
    string(at(rule, &["policy"]), "a rule's policy")
}

/// D1: Authelia has exactly one `two_factor` rule naming the hub's host,
/// the first of the host's rules; its `resources` is one entry, byte for
/// byte item 5's fenced line in `docs/canon/hub.md` "Trust", and it has no
/// condition but its domain (by its regex alone). M: delete `|trace/clear`;
/// move the rule below `one_factor`.
#[test]
fn d01_the_one_step_up_rule_is_item_5s_regex_and_comes_first() {
    let item5 = trust_rule("hub.md", "^/api");
    let rules = access_rules(&door());
    let hub: Vec<&Value> = rules.iter().filter(|rule| names_hub_host(rule)).collect();
    assert!(!hub.is_empty(), "no rule names the hub's host: {rules:?}");
    let step_up: Vec<&&Value> = hub
        .iter()
        .filter(|rule| policy(rule) == "two_factor")
        .collect();
    assert_eq!(
        step_up.len(),
        1,
        "one two_factor rule names the hub's host: {hub:?}"
    );
    let first = hub[0];
    assert_eq!(
        policy(first),
        "two_factor",
        "the step-up rule is the first naming the hub's host: {hub:?}"
    );
    assert_eq!(
        first.get("resources"),
        Some(&json!([item5])),
        "the step-up's resources are item 5's fenced line alone"
    );
    let keys: BTreeSet<&str> = object(first, "the step-up rule")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        BTreeSet::from(["domain", "policy", "resources"]),
        "by its regex alone: no method, network, subject or query narrows the step-up"
    );
    assert_eq!(
        strings(at(first, &["domain"]), "the step-up's domain"),
        [HUB_HOST]
    );
}

/// D2: `default_policy` is `deny`; no rule anywhere is `bypass`; the hub's
/// host has exactly two rules, `two_factor` then `one_factor`. M: add a
/// `bypass` rule for `^/assets/`.
#[test]
fn d02_the_door_denies_by_default_and_bypasses_nothing() {
    let door = door();
    assert_eq!(
        at(&door, &["access_control", "default_policy"]).as_str(),
        Some("deny")
    );
    let rules = access_rules(&door);
    for rule in &rules {
        assert_ne!(policy(rule), "bypass", "a bypass rule: {rule}");
    }
    let hub: Vec<&str> = rules
        .iter()
        .filter(|rule| names_hub_host(rule))
        .map(policy)
        .collect();
    assert_eq!(hub, ["two_factor", "one_factor"], "the hub host's rules");
}

/// D3: the forward-auth endpoint Traefik's door calls takes the cookie
/// session alone: `authn_strategies` is exactly `CookieSession`, so no Basic
/// header logs in at the door. M: add `HeaderAuthorization`.
#[test]
fn d03_the_forward_auth_endpoint_takes_the_cookie_session_alone() {
    let dynamic = dynamic();
    let door = door();
    let middlewares = object(at(&dynamic, &["http", "middlewares"]), "http.middlewares");
    let mut endpoints = BTreeSet::new();
    for (name, definition) in middlewares {
        let (kind, config) = kind_of(definition, name);
        if kind == "forwardAuth" {
            let address = string(at(config, &["address"]), name);
            let endpoint = address
                .strip_prefix("http://authelia:9091/api/authz/")
                .unwrap_or_else(|| panic!("{name}: the door is Authelia's authz: {address}"));
            endpoints.insert(endpoint.to_owned());
        }
    }
    assert!(!endpoints.is_empty(), "no forward-auth middleware");
    for endpoint in endpoints {
        let config = at(&door, &["server", "endpoints", "authz", endpoint.as_str()]);
        assert_eq!(
            at(config, &["implementation"]).as_str(),
            Some("ForwardAuth"),
            "{endpoint}"
        );
        let strategies: Vec<String> = list(at(config, &["authn_strategies"]), &endpoint)
            .iter()
            .map(|strategy| string(at(strategy, &["name"]), &endpoint).to_owned())
            .collect();
        assert_eq!(
            strategies,
            ["CookieSession"],
            "{endpoint}: the cookie session alone"
        );
    }
}

// ------------------------------------------------------------- D4-D6: the routers

/// D4: every router to the daemon carries the door (forward-auth listing
/// `Remote-User` in `authResponseHeaders`, `trustForwardHeader` not true) or
/// the credential (`basicAuth` with `headerField: Remote-User`), exactly one;
/// no middleware on a door's router touches `Authorization`; no
/// `--entrypoints.*.forwardedheaders.*` or `proxyprotocol` flag;
/// `--entrypoints.websecure.http.aliasHeadersStrategy=reject` is set, so an
/// alias header such as `Remote_User` never reaches the daemon (iteration
/// 2). Beside it: the dynamic folder holds `hub.yml` alone, `hub.yml` an
/// `http` section alone. M: remove `door` from `hub`; remove `Remote-User`
/// from `authResponseHeaders`; set `trustForwardHeader: true`; drop the
/// alias-headers flag.
#[test]
fn d04_every_router_to_the_daemon_takes_the_door_or_the_credential_never_both() {
    let dynamic = dynamic();
    let routers = routers(&dynamic);
    let daemon: Vec<&Router> = routers.iter().filter(|router| router.to_daemon()).collect();
    assert!(!daemon.is_empty(), "no router to the daemon");
    for router in daemon {
        let doors = router.of("forwardAuth");
        let credentials = router.of("basicAuth");
        assert_eq!(
            doors.len() + credentials.len(),
            1,
            "{}: the door or the credential, exactly one: {:?}",
            router.name,
            router.names()
        );
        for door in &doors {
            let listed = door
                .config
                .get("authResponseHeaders")
                .map(|headers| strings(headers, &door.name))
                .unwrap_or_default();
            assert!(
                listed.iter().any(|header| header == "Remote-User"),
                "{}: {} lists Remote-User: {listed:?}",
                router.name,
                door.name
            );
            assert!(
                !is_true(door.config.get("trustForwardHeader")),
                "{}: {} trusts a client's X-Forwarded-*",
                router.name,
                door.name
            );
            for headers in router.of("headers") {
                let custom = headers
                    .config
                    .get("customRequestHeaders")
                    .map(|custom| object(custom, &headers.name).clone())
                    .unwrap_or_default();
                assert!(
                    !custom
                        .keys()
                        .any(|name| name.eq_ignore_ascii_case("authorization")),
                    "{}: {} touches Authorization on a door's router",
                    router.name,
                    headers.name
                );
            }
        }
        for credential in &credentials {
            assert_eq!(
                credential.config.get("headerField").and_then(Value::as_str),
                Some("Remote-User"),
                "{}: {} sets Remote-User",
                router.name,
                credential.name
            );
        }
    }
    let static_flags = flags(&command(&compose()));
    for (name, _) in &static_flags {
        assert!(
            !(name.starts_with("--entrypoints.")
                && (name.contains(".forwardedheaders.") || name.contains(".proxyprotocol"))),
            "a flag trusting a client's forwarding: {name}"
        );
    }
    // Flag names are compared lowercased (`flags`); Traefik reads them so.
    assert_eq!(
        flag_values(
            &static_flags,
            "--entrypoints.websecure.http.aliasheadersstrategy"
        ),
        ["reject"],
        "websecure rejects alias headers such as Remote_User"
    );
    let keys: Vec<&String> = object(&dynamic, "hub.yml").keys().collect();
    assert_eq!(keys, ["http"], "hub.yml: an http section alone");
    let folder = deploy().join("traefik/dynamic");
    let files: Vec<String> = fs::read_dir(&folder)
        .expect("the dynamic folder")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        files,
        ["hub.yml"],
        "the file provider loads every file of {}",
        folder.display()
    );
}

/// D5: every router carrying the credential has exactly one `PathRegexp`,
/// byte for byte item 6's fenced line in `docs/canon/hub-mcp.md` "Trust",
/// no `||` or `!`, and a priority above every door router of the same host;
/// no other router carries `basicAuth`; `removeHeader` is true. M: the regex
/// `^/mcp/`; `mcp-credential` on `hub`; the two priorities swapped.
#[test]
fn d05_the_credential_opens_item_6s_routers_alone_and_outranks_the_door() {
    let item6 = trust_rule("hub-mcp.md", "^/");
    let routers = routers(&dynamic());
    let credentialed: Vec<&Router> = routers
        .iter()
        .filter(|router| !router.of("basicAuth").is_empty())
        .collect();
    assert!(!credentialed.is_empty(), "no router carries the credential");
    for router in credentialed {
        assert_eq!(
            rule_arguments(&router.rule, "PathRegexp"),
            std::slice::from_ref(&item6),
            "{}: item 6's matcher alone: {}",
            router.name,
            router.rule
        );
        assert!(
            !router.rule.contains("||") && !router.rule.contains('!'),
            "{}: nothing widens item 6's matcher: {}",
            router.name,
            router.rule
        );
        let hosts = rule_arguments(&router.rule, "Host");
        assert_eq!(hosts, [HUB_HOST], "{}: {}", router.name, router.rule);
        let doors: Vec<&Router> = routers
            .iter()
            .filter(|other| {
                !other.of("forwardAuth").is_empty() && rule_arguments(&other.rule, "Host") == hosts
            })
            .collect();
        assert!(
            !doors.is_empty(),
            "{}: no door router of its host",
            router.name
        );
        for door in doors {
            assert!(
                router.priority > door.priority,
                "{} ({}) outranks {} ({})",
                router.name,
                router.priority,
                door.name,
                door.priority
            );
        }
        for credential in router.of("basicAuth") {
            assert!(
                is_true(credential.config.get("removeHeader")),
                "{}: the secret stops at the proxy",
                credential.name
            );
        }
    }
}

/// D6: the `specengine` service has one server, `http://127.0.0.1:<N>`,
/// where `N` is the `--port` the entrypoint passes; `passHostHeader` is
/// never false. Beside it: every service is a load balancer, and no other
/// points at the loopback. M: `passHostHeader: false`;
/// `http://specengine:7777`; the entrypoint's port 7778.
#[test]
fn d06_the_daemons_one_server_is_the_entrypoints_loopback_port() {
    let entry = Entry::new("d06");
    entry.project("p");
    let (ran, _) = entry.run(Some(HOST), None);
    assert_eq!(ran.code, 0, "{}", ran.show());
    let args = ran.lines();
    let port = args
        .iter()
        .position(|arg| *arg == "--port")
        .and_then(|at| args.get(at + 1))
        .unwrap_or_else(|| panic!("no --port: {}", ran.show()));
    assert!(
        !port.is_empty() && port.chars().all(|c| c.is_ascii_digit()),
        "a port: {port}"
    );

    let dynamic = dynamic();
    let services = object(at(&dynamic, &["http", "services"]), "http.services");
    let daemon = at(&services[DAEMON], &["loadBalancer"]);
    assert_eq!(
        at(daemon, &["servers"]),
        &json!([{"url": format!("http://127.0.0.1:{port}")}]),
        "the daemon's one server, the entrypoint's port"
    );
    let loopback = Regex::new(r"^https?://(127\.|localhost|\[::1\])").expect("a pattern");
    for (name, definition) in services {
        let (kind, config) = kind_of(definition, name);
        assert_eq!(kind, "loadBalancer", "{name}: a plain load balancer");
        assert!(
            !is_false(config.get("passHostHeader")),
            "{name}: passHostHeader is false"
        );
        if name != DAEMON {
            for server in list(at(config, &["servers"]), name) {
                let url = string(at(server, &["url"]), name);
                assert!(
                    !loopback.is_match(url),
                    "{name} reaches the loopback: {url}"
                );
            }
        }
    }
}

// ------------------------------------------------------------- D7-D11: the host

/// D7, as docs/features/hub-accounts.md "Acceptance criteria" (its decided
/// changes to 11b-1's tests) rewrites it: `specengine` has `network_mode:
/// service:traefik` and no `ports`, `expose` or `networks`; `admin`'s
/// `network_mode` is `none`, and no service but `specengine` and `admin` has
/// one; only `traefik` has `ports`, exactly `80:80` and `443:443`. M: publish
/// 7777; give `authelia` the namespace; put `admin` on `service:traefik`.
#[test]
fn d07_the_daemon_lives_in_traefiks_namespace_and_only_traefik_publishes() {
    let compose = compose();
    let daemon = service(&compose, DAEMON);
    assert_eq!(
        daemon.get("network_mode").and_then(Value::as_str),
        Some("service:traefik")
    );
    for key in ["ports", "expose", "networks"] {
        assert!(daemon.get(key).is_none(), "specengine has `{key}`");
    }
    assert_eq!(
        service(&compose, ADMIN)
            .get("network_mode")
            .and_then(Value::as_str),
        Some("none"),
        "admin has no network (hub-accounts)"
    );
    for (name, service) in services(&compose) {
        if name != DAEMON && name != ADMIN {
            assert!(
                service.get("network_mode").is_none(),
                "{name} has a network_mode"
            );
        }
        if name != "traefik" {
            assert!(service.get("ports").is_none(), "{name} publishes a port");
        }
    }
    let ports: Vec<String> = list(
        at(service(&compose, "traefik"), &["ports"]),
        "traefik's ports",
    )
    .iter()
    .map(|port| scalar(port, "a port"))
    .collect();
    let published: BTreeSet<&str> = ports.iter().map(String::as_str).collect();
    assert_eq!(ports.len(), 2, "{ports:?}");
    assert_eq!(published, BTreeSet::from(["80:80", "443:443"]));
}

/// D8: no volume names `docker.sock` (nor any absolute host path); Traefik's
/// only provider flags are `--providers.file.directory` (the mounted
/// `./traefik/dynamic`) and `--providers.file.watch`, no `TRAEFIK_*`
/// variable adds one; no `--api*` flag; `checknewversion` and
/// `sendanonymoususage` are false; no service is privileged or has `pid`,
/// `ipc` or `network_mode` `host`. M: mount the socket; add
/// `--providers.docker=true`; add `--api.dashboard=true`.
#[test]
fn d08_nothing_on_the_host_but_the_file_provider() {
    let compose = compose();
    for (name, service) in services(&compose) {
        for (source, target) in mounts(service, name) {
            assert!(
                !source.contains("docker.sock") && !target.contains("docker.sock"),
                "{name} mounts the Docker socket"
            );
            assert!(
                !source.starts_with('/'),
                "{name} mounts the host path {source}: the compose folder and named volumes alone"
            );
        }
        assert!(!is_true(service.get("privileged")), "{name} is privileged");
        for key in ["pid", "ipc", "network_mode"] {
            assert_ne!(
                service.get(key).and_then(Value::as_str),
                Some("host"),
                "{name}: {key} host"
            );
        }
    }
    let traefik = service(&compose, "traefik");
    for name in environment(traefik, "traefik").keys() {
        assert!(
            !name.starts_with("TRAEFIK_"),
            "traefik's environment configures it: {name}"
        );
    }
    let flags = flags(&command(&compose));
    let providers: BTreeSet<&str> = flags
        .iter()
        .map(|(name, _)| name.as_str())
        .filter(|name| name.starts_with("--providers."))
        .collect();
    assert_eq!(
        providers,
        BTreeSet::from(["--providers.file.directory", "--providers.file.watch"])
    );
    let directory = flag_values(&flags, "--providers.file.directory");
    assert_eq!(directory.len(), 1, "{directory:?}");
    assert!(
        mounts(traefik, "traefik")
            .iter()
            .any(
                |(source, target)| source.trim_end_matches('/') == "./traefik/dynamic"
                    && target.trim_end_matches('/') == directory[0].trim_end_matches('/')
            ),
        "the file provider reads the mounted ./traefik/dynamic"
    );
    for (name, _) in &flags {
        assert!(!name.starts_with("--api"), "an API flag: {name}");
    }
    for name in ["--global.checknewversion", "--global.sendanonymoususage"] {
        assert_eq!(flag_values(&flags, name), ["false"], "{name}");
    }
}

/// D9, as docs/features/hub-accounts.md "Acceptance criteria" (its decided
/// changes to 11b-1's tests) rewrites it: the services are exactly `admin`,
/// `authelia`, `specengine`, `traefik`, with `admin`'s image equal to
/// `authelia`'s; `specengine`'s image is
/// `ghcr.io/molof/specengine:${SPECENGINE_VERSION:?...}`, every other an exact
/// `x.y.z` and `@sha256:` + 64 hex, never `latest`; no YAML file under
/// `scripts/deploy/` or `image.yml` holds an anchor, an alias or a `<<` merge
/// key; `.env.example`'s `SPECENGINE_VERSION` is `plugin.json`'s version. M:
/// `traefik:latest`; drop the `:?`; add a service; bump `plugin.json` alone;
/// give `admin` another image.
#[test]
fn d09_four_services_pinned_images_no_anchors_and_one_version() {
    let compose = compose();
    let names: BTreeSet<&str> = services(&compose).keys().map(String::as_str).collect();
    assert_eq!(
        names,
        BTreeSet::from([ADMIN, "authelia", "specengine", "traefik"])
    );
    assert_eq!(
        string(at(service(&compose, ADMIN), &["image"]), "admin's image"),
        string(
            at(service(&compose, "authelia"), &["image"]),
            "authelia's image"
        ),
        "admin runs the door's own image, byte for byte (hub-accounts)"
    );
    let required = Regex::new(r"^ghcr\.io/molof/specengine:\$\{SPECENGINE_VERSION:\?[^}]*\}$")
        .expect("a pattern");
    let daemon = string(
        at(service(&compose, DAEMON), &["image"]),
        "specengine's image",
    );
    assert!(required.is_match(daemon), "specengine's image: {daemon}");
    let pinned =
        Regex::new(r"^[a-z0-9][a-z0-9._/-]*:v?[0-9]+\.[0-9]+\.[0-9]+@sha256:[0-9a-f]{64}$")
            .expect("a pattern");
    for (name, service) in services(&compose) {
        if name != DAEMON {
            let image = string(at(service, &["image"]), name);
            assert!(
                pinned.is_match(image) && !image.contains("latest"),
                "{name}: an exact version and digest: {image}"
            );
        }
    }
    let files = yaml_files();
    for file in [
        "scripts/deploy/docker-compose.yml",
        "scripts/deploy/traefik/dynamic/hub.yml",
        "scripts/deploy/authelia/config/configuration.yml",
        ".github/workflows/image.yml",
    ] {
        assert!(files.contains(&repository().join(file)), "{file}");
    }
    for file in files {
        let found = anchors_aliases_merges(&read(&file));
        assert!(found.is_empty(), "{}: {found:?}", file.display());
    }
    let versions: Vec<String> = env_example()
        .into_iter()
        .filter(|(key, _)| key == "SPECENGINE_VERSION")
        .map(|(_, value)| value)
        .collect();
    assert_eq!(
        versions,
        [plugin_version()],
        ".env.example's SPECENGINE_VERSION is the plugin's version"
    );
}

/// D10: the `${NAME...}` names of `docker-compose.yml` are exactly
/// `.env.example`'s keys; every `{{ env "NAME" }}` of `hub.yml` or
/// `configuration.yml` is a name its service's `environment` sets from
/// `${NAME}`, and every template is such a read; no value of `.env.example`
/// or the compose file holds `-----BEGIN`. M: add `${HUB_EXTRA}` to the
/// compose file; drop `DOOR_HOST` from `traefik`'s `environment`; put a
/// `-----BEGIN` value in `.env.example`.
#[test]
fn d10_every_key_is_read_and_every_read_is_a_key() {
    let compose = compose();
    let example = env_example();
    let keys: BTreeSet<String> = example.iter().map(|(key, _)| key.clone()).collect();
    assert_eq!(keys.len(), example.len(), "a key twice: {example:?}");
    let mut values = Vec::new();
    every_string(&compose, &mut values);
    let reads: BTreeSet<String> = values
        .iter()
        .flat_map(|value| interpolated(value.as_str()))
        .collect();
    assert_eq!(
        reads, keys,
        "the compose file's reads and .env.example's keys"
    );

    let template =
        Regex::new(r#"\{\{-?\s*env\s+"([A-Za-z_][A-Za-z0-9_]*)"\s*-?\}\}"#).expect("a pattern");
    for (file, owner) in [
        ("traefik/dynamic/hub.yml", "traefik"),
        ("authelia/config/configuration.yml", "authelia"),
    ] {
        let text = read(&deploy().join(file));
        let names: Vec<String> = template
            .captures_iter(&text)
            .map(|found| found[1].to_owned())
            .collect();
        assert!(!names.is_empty(), "{file} reads no variable");
        assert_eq!(
            text.matches("{{").count(),
            names.len(),
            "{file}: every template is a plain env read"
        );
        let environment = environment(service(&compose, owner), owner);
        for name in names {
            assert_eq!(
                environment.get(&name),
                Some(&format!("${{{name}}}")),
                "{file} reads {name}: {owner}'s environment sets it from ${{{name}}}"
            );
        }
    }
    for (key, value) in &example {
        assert!(
            !value.contains("-----BEGIN"),
            ".env.example's {key} holds a key"
        );
    }
    for value in &values {
        assert!(
            !value.contains("-----BEGIN"),
            "the compose file holds a key"
        );
    }
}

/// Whether each of `paths` is ignored by a copy of the `gitignore` file, in
/// a scratch repository where each other path is a real, empty file (the
/// copy itself is never written over).
fn ignored(gitignore: &Path, paths: &[&str]) -> Vec<(String, bool)> {
    let scratch = Scratch::new("gitignore");
    let repo = scratch.dir("repo");
    let home = scratch.dir("home");
    let env: Vec<(String, OsString)> = vec![
        ("PATH".to_owned(), "/usr/bin:/bin".into()),
        ("HOME".to_owned(), home.into()),
        ("GIT_CONFIG_GLOBAL".to_owned(), "/dev/null".into()),
        ("GIT_CONFIG_NOSYSTEM".to_owned(), "1".into()),
        (
            "GIT_CEILING_DIRECTORIES".to_owned(),
            scratch.root.clone().into(),
        ),
    ];
    let git = |args: &[&str]| {
        let args: Vec<OsString> = args.iter().map(OsString::from).collect();
        run("git", "git", &args, &repo, &env)
    };
    let init = git(&["init", "-q", "--template="]);
    assert_eq!(init.code, 0, "{}", init.show());
    fs::copy(gitignore, repo.join(".gitignore")).expect("a copy of the .gitignore");
    for path in paths.iter().filter(|path| **path != ".gitignore") {
        let file = repo.join(path);
        fs::create_dir_all(file.parent().expect("a parent")).expect("its folder");
        fs::write(file, "").expect("a file");
    }
    paths
        .iter()
        .map(|path| {
            let check = git(&["check-ignore", "-q", "--", path]);
            assert!(check.code == 0 || check.code == 1, "{}", check.show());
            ((*path).to_owned(), check.code == 0)
        })
        .collect()
}

/// D11, as docs/features/hub-accounts.md "Acceptance criteria" (its decided
/// changes to 11b-1's tests) rewrites it: `deploy_key` and `known_hosts` are
/// granted to `specengine` alone, `authelia_jwt_secret` and
/// `authelia_session_secret` to `authelia` alone, `authelia_storage_key` to
/// `authelia` and `admin`, each a `file:` under `./secrets/`; `specengine`
/// mounts only `./projects` and `specengine-data`, nothing of `./authelia`,
/// `./traefik` or `./secrets`; every `environment` name holding `SECRET`,
/// `PASSWORD`, `TOKEN` or `KEY` ends `_FILE` with a value under
/// `/run/secrets/`; `scripts/deploy/.gitignore` ignores `.env`, `secrets/`,
/// `projects/`, `authelia/users/`, `traefik/mcp-users/`, `admin-log/acts.log`
/// and `traefik/dynamic/mcp-stamp.yml`, and none of the committed files,
/// `admin/hub-admin` among them. M: grant `deploy_key` to `authelia`; mount
/// `./authelia/users` into `specengine`; set `AUTHELIA_SESSION_SECRET` to a
/// literal; drop `.env` from `.gitignore`; grant `authelia_session_secret` to
/// `admin`.
#[test]
fn d11_secrets_reach_their_service_alone_and_stay_out_of_git() {
    let compose = compose();
    let owners: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::from([
        ("authelia_jwt_secret", BTreeSet::from(["authelia"])),
        ("authelia_session_secret", BTreeSet::from(["authelia"])),
        ("authelia_storage_key", BTreeSet::from(["authelia", ADMIN])),
        ("deploy_key", BTreeSet::from([DAEMON])),
        ("known_hosts", BTreeSet::from([DAEMON])),
    ]);
    let declared = object(at(&compose, &["secrets"]), "secrets");
    let names: BTreeSet<&str> = declared.keys().map(String::as_str).collect();
    assert_eq!(names, owners.keys().copied().collect::<BTreeSet<_>>());
    for (name, definition) in declared {
        let file = string(at(definition, &["file"]), name);
        assert!(
            file.starts_with("./secrets/") && !file.contains(".."),
            "{name}: a file under ./secrets/: {file}"
        );
    }
    for (name, service) in services(&compose) {
        let want: BTreeSet<String> = owners
            .iter()
            .filter(|(_, granted)| granted.contains(name.as_str()))
            .map(|(secret, _)| (*secret).to_owned())
            .collect();
        assert_eq!(grants(service, name), want, "{name}'s secrets");
    }

    let daemon = service(&compose, DAEMON);
    let sources: BTreeSet<String> = mounts(daemon, DAEMON)
        .into_iter()
        .map(|(source, _)| source.trim_end_matches('/').to_owned())
        .collect();
    assert_eq!(
        sources,
        BTreeSet::from(["./projects".to_owned(), "specengine-data".to_owned()]),
        "specengine's mounts"
    );
    for source in &sources {
        for private in ["./authelia", "./traefik", "./secrets"] {
            assert!(!source.starts_with(private), "specengine mounts {source}");
        }
    }
    assert!(daemon.get("volumes_from").is_none());

    for (owner, service) in services(&compose) {
        for (name, value) in environment(service, owner) {
            if ["SECRET", "PASSWORD", "TOKEN", "KEY"]
                .iter()
                .any(|word| name.contains(word))
            {
                assert!(
                    name.ends_with("_FILE") && value.starts_with("/run/secrets/"),
                    "{owner}: {name}={value}: a secret is a file under /run/secrets/"
                );
            }
        }
    }

    let gitignore = deploy().join(".gitignore");
    let private = [
        ".env",
        "secrets/deploy_key",
        "secrets/known_hosts",
        "projects/forge/specengine.toml",
        "authelia/users/users.yml",
        "traefik/mcp-users/users",
        "admin-log/acts.log",
        "traefik/dynamic/mcp-stamp.yml",
    ];
    let committed = [
        ".env.example",
        ".gitignore",
        "Dockerfile",
        "Dockerfile.dockerignore",
        "README.md",
        "docker-compose.yml",
        "entrypoint.sh",
        "clone.sh",
        "tag-check.sh",
        "traefik/dynamic/hub.yml",
        "authelia/config/configuration.yml",
        "admin/hub-admin",
    ];
    let mut paths = private.to_vec();
    paths.extend_from_slice(&committed);
    for (path, is_ignored) in ignored(&gitignore, &paths) {
        assert_eq!(
            is_ignored,
            private.contains(&path.as_str()),
            "{path}: ignored {is_ignored}"
        );
    }
}

// ------------------------------------------------------------- D12: the entrypoint

/// D12: `entrypoint.sh` under `sh`, the stub first on `PATH`: folders `c`,
/// `a`, `B`, `a-1` and `with space` holding a regular `specengine.toml`,
/// `empty` holding none, a dump file beside them, give exactly `--root` each
/// in byte order, one argument each, `--public-origin https://hub.example.org
/// --port 7777`, and one `skipped .../empty` line on stderr; `SPECENGINE_PUSH`
/// `1` adds `--push` last, `0`, empty and unset add nothing; `yes`, an unset
/// and an empty `HUB_HOST` exit 2 with "Data"'s lines and never run the stub;
/// nothing is created. M: pass `--push` always; expand the roots unquoted;
/// glob without `LC_ALL=C`.
#[test]
fn d12_the_entrypoint_serves_each_project_folder_in_byte_order() {
    let entry = Entry::new("d12");
    for name in ["c", "a", "B", "a-1", "with space"] {
        entry.project(name);
    }
    entry.folder("empty");
    fs::write(entry.projects.join("dump.jsonl"), "{}\n").expect("a dump");
    let before = listing(&entry.projects);
    let roots = ["B", "a", "a-1", "c", "with space"];
    let skipped = format!("{ENTRYPOINT} skipped {}/empty", entry.dir());
    let skip_lines = [
        format!("{skipped}: no specengine.toml"),
        format!("{skipped}/: no specengine.toml"),
    ];

    for push in [None, Some("0"), Some("")] {
        let (ran, stub) = entry.run(Some(HOST), push);
        assert_eq!(ran.code, 0, "{}", ran.show());
        assert!(stub, "the daemon runs: {}", ran.show());
        assert_eq!(ran.lines(), entry.expected(&roots, false), "{}", ran.show());
        let own = ran.own(ENTRYPOINT);
        assert!(
            own.len() == 1 && skip_lines.iter().any(|line| line == own[0]),
            "one skipped line for empty: {}",
            ran.show()
        );
    }

    let (ran, stub) = entry.run(Some(HOST), Some("1"));
    assert_eq!(ran.code, 0, "{}", ran.show());
    assert!(stub);
    assert_eq!(ran.lines(), entry.expected(&roots, true), "{}", ran.show());

    let refusals = [
        (
            Some(HOST),
            Some("yes"),
            format!("{ENTRYPOINT} SPECENGINE_PUSH is yes: give 0 or 1"),
        ),
        (
            None,
            None,
            format!("{ENTRYPOINT} HUB_HOST is not set: give it in .env"),
        ),
        (
            Some(""),
            None,
            format!("{ENTRYPOINT} HUB_HOST is not set: give it in .env"),
        ),
    ];
    for (host, push, line) in refusals {
        let (ran, stub) = entry.run(host, push);
        assert_eq!(ran.code, 2, "{}", ran.show());
        assert!(!stub, "the daemon never runs: {}", ran.show());
        assert_eq!(ran.stdout, "", "{}", ran.show());
        let own = ran.own(ENTRYPOINT);
        assert!(own.contains(&line.as_str()), "{line}: {}", ran.show());
        for other in own {
            assert!(
                other == line || skip_lines.iter().any(|skip| skip == other),
                "{other}: {}",
                ran.show()
            );
        }
    }
    assert_eq!(listing(&entry.projects), before, "nothing is created");
}

/// D12: no folder holding a regular `specengine.toml` (none at all; `empty`
/// and a `specengine.toml` that is a folder) exits 2 with "Data"'s line and
/// never runs the stub; an empty projects folder prints nothing for its
/// unmatched glob.
#[test]
fn d12_no_project_exits_2_and_never_runs_the_daemon() {
    let entry = Entry::new("d12-none");
    let line = format!(
        "{ENTRYPOINT} no project under {} holds a specengine.toml: bring one over first (scripts/deploy/README.md)",
        entry.dir()
    );
    let (ran, stub) = entry.run(Some(HOST), None);
    assert_eq!(ran.code, 2, "{}", ran.show());
    assert!(!stub);
    assert_eq!(ran.stdout, "", "{}", ran.show());
    assert_eq!(ran.own(ENTRYPOINT), [line.as_str()], "{}", ran.show());

    entry.folder("empty");
    entry.folder("folder-config/specengine.toml");
    let (ran, stub) = entry.run(Some(HOST), None);
    assert_eq!(ran.code, 2, "{}", ran.show());
    assert!(!stub);
    let own = ran.own(ENTRYPOINT);
    assert!(own.contains(&line.as_str()), "{}", ran.show());
    for folder in ["empty", "folder-config"] {
        let skipped = format!("{ENTRYPOINT} skipped {}/{folder}", entry.dir());
        assert!(
            own.iter().any(|line| line.starts_with(&skipped)),
            "{folder} skipped: {}",
            ran.show()
        );
    }
}

/// D12, the compose half: `specengine` has `init: true`, `restart:
/// unless-stopped` and the named volume `specengine-data` at `/data`. M:
/// drop `init`; bind-mount `./data` at `/data`.
#[test]
fn d12_compose_gives_the_daemon_init_restart_and_its_named_volume() {
    let compose = compose();
    let daemon = service(&compose, DAEMON);
    assert!(is_true(daemon.get("init")), "init: true");
    assert_eq!(
        daemon.get("restart").and_then(Value::as_str),
        Some("unless-stopped")
    );
    let data: Vec<String> = mounts(daemon, DAEMON)
        .into_iter()
        .filter(|(_, target)| target.trim_end_matches('/') == "/data")
        .map(|(source, _)| source)
        .collect();
    assert_eq!(data, ["specengine-data"], "/data is the named volume");
    assert!(
        find(&compose, &["volumes", "specengine-data"]).is_some(),
        "the volume is declared"
    );
}

// ------------------------------------------------------------- D13: the edge

/// D13: every router to the daemon carries a headers middleware setting
/// `contentTypeNosniff: true`, a non-zero `stsSeconds` and
/// `Cross-Origin-Opener-Policy: same-origin`; no middleware is `buffering`
/// or `compress`; every router's `entryPoints` is exactly `[websecure]`,
/// with TLS; `web` redirects to `websecure` and `https`; `websecure`'s
/// `readtimeout` is at least 300 s; no `writetimeout`. M: drop
/// `contentTypeNosniff`; add a `buffering` middleware; drop the redirect;
/// set `readtimeout=60s`.
#[test]
fn d13_hardening_headers_tls_the_redirect_and_the_timeouts() {
    let dynamic = dynamic();
    for router in routers(&dynamic) {
        if router.to_daemon() {
            let hardened = router.of("headers").into_iter().any(|headers| {
                let config = &headers.config;
                is_true(config.get("contentTypeNosniff"))
                    && config
                        .get("stsSeconds")
                        .and_then(Value::as_f64)
                        .is_some_and(|seconds| seconds > 0.0)
                    && find(
                        config,
                        &["customResponseHeaders", "Cross-Origin-Opener-Policy"],
                    )
                    .and_then(Value::as_str)
                        == Some("same-origin")
            });
            assert!(
                hardened,
                "{}: the hardening headers: {:?}",
                router.name, router.middlewares
            );
        }
        assert_eq!(
            router
                .entry_points
                .as_ref()
                .map(|points| strings(points, &router.name)),
            Some(vec!["websecure".to_owned()]),
            "{}: websecure alone",
            router.name
        );
        assert!(
            router
                .tls
                .as_ref()
                .is_some_and(|tls| !tls.is_null() && tls != &json!(false)),
            "{}: TLS",
            router.name
        );
    }
    for (name, definition) in object(at(&dynamic, &["http", "middlewares"]), "http.middlewares") {
        let (kind, _) = kind_of(definition, name);
        assert!(
            kind != "buffering" && kind != "compress",
            "{name}: {kind} holds a stream"
        );
    }
    let flags = flags(&command(&compose()));
    assert_eq!(
        flag_values(&flags, "--entrypoints.web.http.redirections.entrypoint.to"),
        ["websecure"]
    );
    assert_eq!(
        flag_values(
            &flags,
            "--entrypoints.web.http.redirections.entrypoint.scheme"
        ),
        ["https"]
    );
    let read = flag_values(
        &flags,
        "--entrypoints.websecure.transport.respondingtimeouts.readtimeout",
    );
    assert_eq!(read.len(), 1, "one readtimeout: {read:?}");
    assert!(seconds(&read[0]) >= 300.0, "readtimeout {}", read[0]);
    for (name, _) in &flags {
        assert!(!name.contains("writetimeout"), "{name}");
    }
}

// ------------------------------------------------------------- D14: the image

#[derive(Debug)]
struct Instruction {
    keyword: String,
    args: String,
    line: usize,
}

/// A Dockerfile's instructions: comments and blank lines dropped,
/// continuations joined, heredoc bodies kept in their instruction.
fn instructions(text: &str) -> Vec<Instruction> {
    let heredoc = Regex::new(r#"<<-?\s*['"]?([A-Za-z_][A-Za-z0-9_]*)['"]?"#).expect("a pattern");
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    let mut at = 0;
    while at < lines.len() {
        let first = lines[at].trim();
        let line = at + 1;
        at += 1;
        if first.is_empty() || first.starts_with('#') {
            continue;
        }
        let mut logical = String::new();
        let mut current = first.to_owned();
        while let Some(joined) = current.trim_end().strip_suffix('\\') {
            logical.push_str(joined);
            logical.push(' ');
            current = String::new();
            while at < lines.len() {
                let next = lines[at].trim();
                at += 1;
                if !next.starts_with('#') {
                    next.clone_into(&mut current);
                    break;
                }
            }
        }
        logical.push_str(&current);
        let words: Vec<String> = heredoc
            .captures_iter(&logical)
            .map(|found| found[1].to_owned())
            .collect();
        for word in words {
            while at < lines.len() {
                let body = lines[at];
                at += 1;
                if body.trim() == word {
                    break;
                }
                logical.push('\n');
                logical.push_str(body);
            }
        }
        let (keyword, args) = logical
            .split_once(char::is_whitespace)
            .unwrap_or((logical.as_str(), ""));
        out.push(Instruction {
            keyword: keyword.to_ascii_uppercase(),
            args: args.trim().to_owned(),
            line,
        });
    }
    out
}

#[derive(Debug)]
struct Stage {
    image: String,
    name: Option<String>,
    body: Vec<Instruction>,
}

impl Stage {
    /// What a `COPY --from=` names this stage by: its name, its index.
    fn refs(&self, index: usize) -> Vec<String> {
        let mut refs = vec![index.to_string()];
        refs.extend(self.name.clone());
        refs
    }

    fn of(&self, keyword: &str) -> impl Iterator<Item = &Instruction> {
        self.body
            .iter()
            .filter(move |instruction| instruction.keyword == keyword)
    }
}

fn stages(instructions: Vec<Instruction>) -> Vec<Stage> {
    let mut stages: Vec<Stage> = Vec::new();
    for instruction in instructions {
        if instruction.keyword == "FROM" {
            let words: Vec<&str> = instruction
                .args
                .split_whitespace()
                .filter(|word| !word.starts_with("--"))
                .collect();
            let image = (*words.first().expect("FROM names an image")).to_owned();
            let name = words
                .iter()
                .position(|word| word.eq_ignore_ascii_case("as"))
                .and_then(|at| words.get(at + 1))
                .map(|name| (*name).to_owned());
            stages.push(Stage {
                image,
                name,
                body: Vec::new(),
            });
        } else {
            stages
                .last_mut()
                .unwrap_or_else(|| panic!("{} before any FROM", instruction.keyword))
                .body
                .push(instruction);
        }
    }
    stages
}

/// An image reference's name, tag and digest.
fn image_parts(reference: &str) -> (String, Option<String>, Option<String>) {
    let (rest, digest) = match reference.split_once('@') {
        Some((rest, digest)) => (rest, Some(digest.to_owned())),
        None => (reference, None),
    };
    let after_slash = rest.rfind('/').map_or(0, |slash| slash + 1);
    match rest[after_slash..].rfind(':') {
        Some(colon) => (
            rest[..after_slash + colon].to_owned(),
            Some(rest[after_slash + colon + 1..].to_owned()),
            digest,
        ),
        None => (rest.to_owned(), None, digest),
    }
}

fn last_component(name: &str) -> &str {
    name.rsplit('/').next().unwrap_or(name)
}

/// A `COPY`'s flags (`--from=x` as `from`) and its paths, the last the
/// destination.
fn copy_parts(args: &str) -> (BTreeMap<String, String>, Vec<String>) {
    let mut flags = BTreeMap::new();
    let mut rest = args.trim();
    while let Some(flag) = rest.strip_prefix("--") {
        let (word, after) = flag.split_once(char::is_whitespace).unwrap_or((flag, ""));
        let (name, value) = word.split_once('=').unwrap_or((word, ""));
        flags.insert(name.to_owned(), value.to_owned());
        rest = after.trim_start();
    }
    let paths = if rest.starts_with('[') {
        serde_json::from_str::<Vec<String>>(rest).expect("a COPY's JSON form")
    } else {
        rest.split_whitespace().map(str::to_owned).collect()
    };
    (flags, paths)
}

/// The words of every `apt-get install` of a stage, a version pin dropped.
fn installed(stage: &Stage) -> BTreeSet<String> {
    stage
        .of("RUN")
        .filter(|run| run.args.contains("apt-get") && run.args.contains("install"))
        .flat_map(|run| {
            run.args
                .split(|c: char| c.is_whitespace() || c == ';' || c == '&' || c == '|')
                .map(|word| word.split('=').next().unwrap_or(word).to_owned())
                .collect::<Vec<_>>()
        })
        .collect()
}

/// A stage's `ENV` (or `LABEL`) settings, both forms.
fn image_env(stage: &Stage) -> BTreeMap<String, String> {
    settings(stage, "ENV")
}

fn settings(stage: &Stage, keyword: &str) -> BTreeMap<String, String> {
    let mut env = BTreeMap::new();
    for set in stage.of(keyword) {
        let first = set.args.split_whitespace().next().unwrap_or_default();
        if first.contains('=') {
            for word in set.args.split_whitespace() {
                if let Some((name, value)) = word.split_once('=') {
                    env.insert(name.to_owned(), value.trim_matches('"').to_owned());
                }
            }
        } else if let Some((name, value)) = set.args.split_once(char::is_whitespace) {
            env.insert(name.to_owned(), value.trim().trim_matches('"').to_owned());
        }
    }
    env
}

/// `.dockerignore` patterns: `(negated, segments)`, cleaned as Docker does.
fn ignore_patterns(text: &str) -> Vec<(bool, Vec<String>)> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let (negated, pattern) = match line.strip_prefix('!') {
                Some(pattern) => (true, pattern.trim()),
                None => (false, line),
            };
            let segments = pattern
                .split('/')
                .filter(|segment| !segment.is_empty() && *segment != ".")
                .map(str::to_owned)
                .collect();
            (negated, segments)
        })
        .collect()
}

/// A single path segment against a pattern segment of `*` and `?`.
fn glob(pattern: &[u8], text: &[u8]) -> bool {
    match (pattern.split_first(), text.split_first()) {
        (None, None) => true,
        (Some((b'*', rest)), _) => {
            glob(rest, text) || (!text.is_empty() && glob(pattern, &text[1..]))
        }
        (Some((b'?', rest)), Some((_, tail))) => glob(rest, tail),
        (Some((want, rest)), Some((have, tail))) => want == have && glob(rest, tail),
        _ => false,
    }
}

fn segments_match(pattern: &[String], path: &[&str]) -> bool {
    match pattern.split_first() {
        None => path.is_empty(),
        Some((first, rest)) if first == "**" => {
            (0..=path.len()).any(|skip| segments_match(rest, &path[skip..]))
        }
        Some((first, rest)) => {
            !path.is_empty()
                && glob(first.as_bytes(), path[0].as_bytes())
                && segments_match(rest, &path[1..])
        }
    }
}

/// Whether the build context leaves `path` out: the last pattern matching it
/// or one of its parents decides.
fn excluded(patterns: &[(bool, Vec<String>)], path: &str) -> bool {
    let segments: Vec<&str> = path.split('/').collect();
    let mut out = false;
    for (negated, pattern) in patterns {
        if (1..=segments.len()).any(|depth| segments_match(pattern, &segments[..depth])) {
            out = !negated;
        }
    }
    out
}

/// D14: every `FROM` holds a version and `@sha256:` + 64 hex; the build
/// stage's Rust tag starts with the root `Cargo.toml`'s `rust-version`; the
/// Node stage's tag starts `24.`; the UI is built, and its `dist` copied into
/// the build stage, before `cargo build --locked`; the last stage is Debian
/// 13 slim, installs `git`, `openssh-client` and `ca-certificates` and never
/// `git-lfs`, copies from the build stage exactly `spec` and
/// `specengine-http`, and has a `USER` neither `root` nor `0` and the label
/// `org.opencontainers.image.source` =
/// `https://github.com/MoloF/specengine-platform` (iteration 2);
/// `Dockerfile.dockerignore` excludes `target`, `target.noindex`, `.git`
/// and `node_modules`. M: add `git-lfs`; remove `USER`; build the UI after
/// cargo; `rust:1.91`; drop the source label.
#[test]
fn d14_the_image_three_pinned_stages_and_its_context() {
    let path = deploy().join("Dockerfile");
    let stages = stages(instructions(&read(&path)));
    assert_eq!(stages.len(), 3, "three stages: {stages:?}");
    let digest = Regex::new(r"^sha256:[0-9a-f]{64}$").expect("a pattern");
    for stage in &stages {
        let (name, tag, pin) = image_parts(&stage.image);
        let tag = tag.unwrap_or_else(|| panic!("{name}: no tag"));
        assert!(
            tag.chars().any(|c| c.is_ascii_digit()) && !tag.contains("latest"),
            "{}: a version",
            stage.image
        );
        assert!(
            pin.as_deref().is_some_and(|pin| digest.is_match(pin)),
            "{}: a digest",
            stage.image
        );
    }
    let position = |image: &str| {
        stages
            .iter()
            .position(|stage| last_component(&image_parts(&stage.image).0) == image)
            .unwrap_or_else(|| panic!("no {image} stage: {stages:?}"))
    };
    let (node, rust) = (position("node"), position("rust"));
    assert!(node < rust, "the UI stage comes first");
    let tag_of = |stage: &Stage| image_parts(&stage.image).1.expect("a tag");

    let manifest: toml::Table =
        toml::from_str(&read(&repository().join("Cargo.toml"))).expect("the root Cargo.toml");
    let version = manifest["workspace"]["package"]["rust-version"]
        .as_str()
        .expect("the workspace's rust-version");
    let rust_tag = tag_of(&stages[rust]);
    assert!(
        rust_tag == version
            || rust_tag.starts_with(&format!("{version}."))
            || rust_tag.starts_with(&format!("{version}-")),
        "the builder is Rust {version}: {rust_tag}"
    );
    assert!(tag_of(&stages[node]).starts_with("24."), "Node 24");

    assert!(
        stages[node]
            .of("RUN")
            .any(|run| run.args.contains("pnpm") && run.args.contains("build")),
        "the UI stage builds the UI"
    );
    let ui_refs = stages[node].refs(node);
    let build = &stages[rust];
    let copy = build
        .body
        .iter()
        .position(|instruction| {
            instruction.keyword == "COPY"
                && copy_parts(&instruction.args)
                    .0
                    .get("from")
                    .is_some_and(|from| ui_refs.contains(from))
        })
        .unwrap_or_else(|| panic!("the build stage copies no dist: {build:?}"));
    let cargo = build
        .body
        .iter()
        .position(|instruction| {
            instruction.keyword == "RUN" && instruction.args.contains("cargo build")
        })
        .unwrap_or_else(|| panic!("the build stage runs no cargo build: {build:?}"));
    assert!(
        copy < cargo,
        "ui/dist is in place before cargo build (line {} after {})",
        build.body[copy].line,
        build.body[cargo].line
    );
    let (_, copied) = copy_parts(&build.body[copy].args);
    assert!(
        copied.last().is_some_and(|dest| dest.contains("ui/dist")),
        "the dist lands in ui/dist: {copied:?}"
    );
    for word in ["--locked", "--release", "specengine-cli", "specengine-http"] {
        assert!(
            build.body[cargo].args.contains(word),
            "cargo build {word}: {}",
            build.body[cargo].args
        );
    }

    let runtime = stages.last().expect("a last stage");
    let (base, tag, _) = image_parts(&runtime.image);
    let tag = tag.expect("a tag");
    assert!(
        last_component(&base) == "debian"
            && tag.contains("slim")
            && (tag.starts_with("trixie") || tag.starts_with("13")),
        "the runtime is Debian 13 slim: {}",
        runtime.image
    );
    let packages = installed(runtime);
    for package in ["git", "openssh-client", "ca-certificates"] {
        assert!(packages.contains(package), "{package}: {packages:?}");
    }
    for stage in &stages {
        for instruction in &stage.body {
            assert!(
                !instruction.args.contains("git-lfs"),
                "line {}: git-lfs",
                instruction.line
            );
        }
    }
    let build_refs = stages[rust].refs(rust);
    let mut from_build: Vec<String> = runtime
        .of("COPY")
        .map(|copy| copy_parts(&copy.args))
        .filter(|(flags, _)| {
            flags
                .get("from")
                .is_some_and(|from| build_refs.contains(from))
        })
        .flat_map(|(_, mut paths)| {
            paths.pop();
            paths
        })
        .map(|source| last_component(source.trim_end_matches('/')).to_owned())
        .collect();
    from_build.sort();
    assert_eq!(
        from_build,
        ["spec", "specengine-http"],
        "the build stage gives the two binaries alone"
    );
    let user = runtime
        .of("USER")
        .last()
        .map(|user| {
            user.args
                .split(':')
                .next()
                .unwrap_or_default()
                .trim()
                .to_owned()
        })
        .unwrap_or_else(|| panic!("the runtime has no USER"));
    assert!(user != "root" && user != "0", "USER {user}");
    assert_eq!(
        settings(runtime, "LABEL")
            .get("org.opencontainers.image.source")
            .map(String::as_str),
        Some("https://github.com/MoloF/specengine-platform"),
        "the runtime's source label links the package to its repository"
    );

    // "Data": the data directory and HOME, the two scripts, the entrypoint.
    let env = image_env(runtime);
    assert_eq!(env.get("XDG_DATA_HOME").map(String::as_str), Some("/data"));
    // The image user's home lies outside every root and outside `/data`.
    assert_eq!(
        env.get("HOME").map(String::as_str),
        Some("/var/lib/specengine/home")
    );
    let installs: Vec<Vec<String>> = runtime
        .of("COPY")
        .map(|copy| copy_parts(&copy.args))
        .filter(|(flags, _)| !flags.contains_key("from"))
        .map(|(_, paths)| paths)
        .collect();
    for (script, name) in [
        ("entrypoint.sh", "/usr/local/bin/specengine-entrypoint"),
        ("clone.sh", "/usr/local/bin/specengine-clone"),
    ] {
        assert!(
            installs
                .iter()
                .any(|paths| paths.len() == 2 && paths[0].ends_with(script) && paths[1] == name),
            "{script} as {name}: {installs:?}"
        );
    }
    assert!(
        runtime
            .of("ENTRYPOINT")
            .last()
            .is_some_and(|entry| entry.args.contains("specengine-entrypoint")),
        "the image runs specengine-entrypoint"
    );

    let patterns = ignore_patterns(&read(&deploy().join("Dockerfile.dockerignore")));
    for path in [
        "target/release/spec",
        "target.noindex/release/spec",
        ".git/HEAD",
        "ui/node_modules/react/package.json",
        "ui/dist/index.html",
    ] {
        assert!(excluded(&patterns, path), "the context holds {path}");
    }
    let mut kept = vec![
        "Cargo.toml".to_owned(),
        "Cargo.lock".to_owned(),
        "ui/package.json".to_owned(),
        "ui/pnpm-lock.yaml".to_owned(),
        "ui/src/main.tsx".to_owned(),
        "scripts/deploy/entrypoint.sh".to_owned(),
        "scripts/deploy/clone.sh".to_owned(),
    ];
    for member in fs::read_dir(repository().join("crates"))
        .expect("crates/")
        .flatten()
    {
        if member.path().join("Cargo.toml").is_file() {
            let name = member.file_name().to_string_lossy().into_owned();
            kept.push(format!("crates/{name}/Cargo.toml"));
            kept.push(format!("crates/{name}/src/lib.rs"));
        }
    }
    // Every source a stage copies from the context exists and is in it.
    for stage in &stages {
        for copy in stage.of("COPY") {
            let (flags, mut paths) = copy_parts(&copy.args);
            if flags.contains_key("from") {
                continue;
            }
            paths.pop();
            for source in paths {
                let source = source.trim_end_matches('/').to_owned();
                assert!(
                    repository().join(&source).exists(),
                    "line {}: {source} is no repository path",
                    copy.line
                );
                kept.push(source);
            }
        }
    }
    for path in &kept {
        assert!(!excluded(&patterns, path), "the context leaves out {path}");
    }
}

// ------------------------------------------------------------- D15: the workflow

fn steps(job: &Value) -> Vec<&Value> {
    job.get("steps")
        .map(|steps| list(steps, "a job's steps").iter().collect())
        .unwrap_or_default()
}

fn run_text(step: &Value) -> Option<&str> {
    step.get("run").and_then(Value::as_str)
}

fn uses(step: &Value) -> Option<&str> {
    step.get("uses").and_then(Value::as_str)
}

/// The first byte of a push command in a `run` text.
fn push_at(run: &str) -> Option<usize> {
    ["docker push", "imagetools create", "--push"]
        .iter()
        .filter_map(|command| run.find(command))
        .min()
}

/// Whether a step pushes to the registry.
fn step_pushes(step: &Value) -> bool {
    run_text(step).is_some_and(|run| push_at(run).is_some())
        || (uses(step).is_some_and(|uses| uses.starts_with("docker/build-push-action"))
            && is_true(find(step, &["with", "push"])))
}

/// Whether a job pushes to the registry.
fn pushes(job: &Value) -> bool {
    steps(job).iter().any(|step| step_pushes(step))
}

/// Whether a job builds an image: `docker build --check` (C2) only lints
/// the Dockerfile and builds nothing.
fn builds(job: &Value) -> bool {
    steps(job).iter().any(|step| {
        run_text(step).is_some_and(|run| {
            run.lines().any(|line| {
                (line.contains("docker build") || line.contains("buildx build"))
                    && !line.split_whitespace().any(|word| word == "--check")
            })
        }) || uses(step).is_some_and(|uses| uses.starts_with("docker/build-push-action"))
    })
}

/// Every job `name` waits for, through `needs`.
fn waits_for(jobs: &Map<String, Value>, name: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut stack = vec![name.to_owned()];
    while let Some(job) = stack.pop() {
        if let Some(needs) = jobs.get(&job).and_then(|job| job.get("needs")) {
            for need in strings(needs, &job) {
                if found.insert(need.clone()) {
                    stack.push(need);
                }
            }
        }
    }
    found
}

/// D15: `image.yml` runs on `push` of `tags: [v*]` alone; its permissions
/// are `contents: read`, and only the jobs that push add `packages: write`;
/// its build matrix holds `linux/amd64` on `ubuntu-24.04` and `linux/arm64`
/// on `ubuntu-24.04-arm`; the check job runs `tag-check.sh` first, both
/// deploy tests and `compose config`, and every other job waits for it;
/// every `uses:` ends `@` + 40 hex, a version in its comment; the pushed name
/// is the compose file's `ghcr.io/molof/specengine`; no step names `latest`;
/// no secret but `GITHUB_TOKEN`, no Rust toolchain action. M: add `branches:
/// [main]`; drop arm64; `actions/checkout@v4`.
#[test]
fn d15_the_image_workflow_runs_on_version_tags_alone_after_its_checks() {
    let path = workflow_path();
    let text = read(&path);
    let workflow = workflow();
    let on = workflow
        .get("on")
        .or_else(|| workflow.get("true"))
        .expect("`on`");
    assert_eq!(on, &json!({"push": {"tags": ["v*"]}}), "version tags alone");
    assert_eq!(
        workflow.get("permissions"),
        Some(&json!({"contents": "read"}))
    );
    let jobs = object(at(&workflow, &["jobs"]), "jobs");
    for (name, job) in jobs {
        let permissions = job
            .get("permissions")
            .map(|permissions| object(permissions, name).clone())
            .unwrap_or_default();
        for (scope, level) in &permissions {
            match (scope.as_str(), level.as_str()) {
                ("contents", Some("read")) => {}
                ("packages", Some("write")) => {
                    assert!(
                        pushes(job),
                        "{name} adds packages: write and pushes nothing"
                    );
                }
                other => panic!("{name}: {other:?}"),
            }
        }
        if pushes(job) {
            assert_eq!(
                permissions.get("packages").and_then(Value::as_str),
                Some("write"),
                "{name} pushes"
            );
        }
    }

    let build = at(&workflow, &["jobs", "build"]);
    let include = list(
        at(build, &["strategy", "matrix", "include"]),
        "the build matrix",
    );
    let entries: Vec<BTreeSet<String>> = include
        .iter()
        .map(|entry| {
            object(entry, "a matrix entry")
                .values()
                .map(|value| scalar(value, "a matrix value"))
                .collect()
        })
        .collect();
    for (platform, runner) in [
        ("linux/amd64", "ubuntu-24.04"),
        ("linux/arm64", "ubuntu-24.04-arm"),
    ] {
        assert!(
            entries
                .iter()
                .any(|entry| entry.contains(platform) && entry.contains(runner)),
            "{platform} on {runner}: {entries:?}"
        );
    }
    assert!(
        build
            .get("runs-on")
            .is_some_and(|runner| scalar(runner, "runs-on").contains("matrix.")),
        "each platform on its own runner"
    );

    let check = at(&workflow, &["jobs", "check"]);
    let runs: Vec<&str> = steps(check).into_iter().filter_map(run_text).collect();
    assert!(
        runs.first().is_some_and(|run| run.contains("tag-check.sh")),
        "tag-check.sh first: {runs:?}"
    );
    let deploy_test = Regex::new(r"--test\s+deploy(\s|$)").expect("a pattern");
    let clone_test = Regex::new(r"--test\s+deploy_clone(\s|$)").expect("a pattern");
    assert!(
        runs.iter().any(|run| run.contains("--locked")
            && run.contains("-p specengine-http")
            && deploy_test.is_match(run)),
        "the http deploy test: {runs:?}"
    );
    assert!(
        runs.iter().any(|run| run.contains("--locked")
            && run.contains("-p specengine-cli")
            && clone_test.is_match(run)),
        "the CLI clone test: {runs:?}"
    );
    assert!(
        runs.iter()
            .any(|run| run.contains("compose") && run.contains("config")),
        "compose config: {runs:?}"
    );
    assert!(!builds(check), "the check job builds no image");
    for name in jobs.keys().filter(|name| *name != "check") {
        assert!(
            waits_for(jobs, name).contains("check"),
            "{name} runs before the checks"
        );
    }

    let pinned = Regex::new(r"^[^@\s]+@[0-9a-f]{40}$").expect("a pattern");
    let mut all_uses = Vec::new();
    for (name, job) in jobs {
        all_uses.extend(job.get("uses").and_then(Value::as_str));
        all_uses.extend(steps(job).into_iter().filter_map(uses));
        for step in steps(job) {
            let mut values = Vec::new();
            every_string(step, &mut values);
            assert!(
                !values.iter().any(|value| value.contains("latest")),
                "{name}: a step names latest: {step}"
            );
        }
    }
    assert!(!all_uses.is_empty(), "no action");
    for action in &all_uses {
        assert!(pinned.is_match(action), "{action}: a 40-hex commit");
        for toolchain in ["rust-toolchain", "actions-rs", "setup-rust"] {
            assert!(!action.contains(toolchain), "{action}: the runner's cargo");
        }
    }
    let commented =
        Regex::new(r"^\s*(-\s+)?uses:\s*\S+@[0-9a-f]{40}\s+#\s*v?[0-9]").expect("a pattern");
    for line in text.lines().filter(|line| line.contains("uses:")) {
        assert!(commented.is_match(line), "a version comment: {line}");
    }
    let secrets = Regex::new(r"secrets\.([A-Za-z_][A-Za-z0-9_]*)").expect("a pattern");
    for found in secrets.captures_iter(&text) {
        assert_eq!(&found[1], "GITHUB_TOKEN", "no secret but GITHUB_TOKEN");
    }

    let compose = compose();
    let image = string(
        at(service(&compose, DAEMON), &["image"]),
        "specengine's image",
    );
    let (name, _) = image.split_once(':').expect("a tag");
    let mut values = Vec::new();
    every_string(&workflow, &mut values);
    let registry = Regex::new(r"ghcr\.io/[A-Za-z0-9._/-]*").expect("a pattern");
    let pushed: Vec<&str> = values
        .iter()
        .flat_map(|value| registry.find_iter(value).map(|found| found.as_str()))
        .collect();
    assert!(!pushed.is_empty(), "image.yml names no GHCR image");
    for found in pushed {
        assert_eq!(found, name, "the pushed name is the compose file's");
    }
}

/// The release check's command (docs/features/hub-deploy.md "`image.yml`").
const INSPECT: &str = r#"docker buildx imagetools inspect "$IMAGE:$version""#;

/// A stub `docker`: it logs each call and answers `buildx imagetools
/// inspect` with success when `STUB_PUBLISHED` is 1 or once something was
/// pushed or a list made, else with 1; everything else succeeds.
const DOCKER_STUB: &str = r#"#!/bin/sh
log='LOG'
printf '%s\n' "$*" >> "$log"
case "$*" in
    "buildx imagetools inspect "*)
        if [ "${STUB_PUBLISHED:-0}" = 1 ] || grep -q -e '^push ' -e '^buildx imagetools create ' "$log"; then
            exit 0
        fi
        exit 1
        ;;
esac
exit 0
"#;

/// `text` with each `${{ expression }}` replaced by `value(expression)`.
fn expressions(text: &str, value: &dyn Fn(&str) -> String) -> String {
    let pattern = Regex::new(r"\$\{\{\s*(.*?)\s*\}\}").expect("a pattern");
    pattern
        .replace_all(text, |found: &regex::Captures| value(&found[1]))
        .into_owned()
}

/// A GitHub expression's text: its `${{ }}` dropped, white space collapsed.
fn expression_text(text: &str) -> String {
    let trimmed = text.trim();
    let inner = trimmed
        .strip_prefix("${{")
        .and_then(|rest| rest.strip_suffix("}}"))
        .unwrap_or(trimmed);
    inner.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `image.yml`'s `run` steps run as a runner runs them (bash, `-e`, or
/// `-eo pipefail` under `shell: bash`), the workflow's, the job's and the
/// step's `env` given, `docker` the stub, the tag `v` + the plugin's version.
struct Runner {
    scratch: Scratch,
    bin: PathBuf,
    log: PathBuf,
    published: bool,
}

impl Runner {
    fn new(published: bool) -> Self {
        let scratch = Scratch::new("d15-runner");
        let bin = scratch.dir("bin");
        let log = scratch.root.join("docker.log");
        fs::write(&log, "").expect("the stub's log");
        let stub = bin.join("docker");
        fs::write(
            &stub,
            DOCKER_STUB.replace("LOG", &log.display().to_string()),
        )
        .expect("the stub");
        fs::set_permissions(stub, fs::Permissions::from_mode(0o755)).expect("an executable stub");
        Self {
            scratch,
            bin,
            log,
            published,
        }
    }

    fn log(&self) -> String {
        read(&self.log)
    }

    /// One step, the outputs of earlier steps in `outputs`: its run and the
    /// outputs it wrote.
    fn execute(
        &self,
        workflow: &Value,
        job: &Value,
        step: &Value,
        outputs: &BTreeMap<String, String>,
    ) -> (Ran, BTreeMap<String, String>) {
        let version = plugin_version();
        let matrix = find(job, &["strategy", "matrix", "include"])
            .and_then(Value::as_array)
            .and_then(|include| include.first())
            .cloned()
            .unwrap_or(Value::Null);
        let value = |expression: &str| -> String {
            if let Some(key) = expression.strip_prefix("matrix.") {
                return scalar(at(&matrix, &[key]), expression);
            }
            match expression {
                "github.ref_name" => format!("v{version}"),
                "github.repository_owner" | "github.actor" => "MoloF".to_owned(),
                _ => outputs
                    .get(expression)
                    .cloned()
                    .unwrap_or_else(|| panic!("an expression the test cannot give: {expression}")),
            }
        };
        let number = COUNTER.fetch_add(1, Ordering::SeqCst);
        let script = self.scratch.root.join(format!("step-{number}.sh"));
        let output = self.scratch.root.join(format!("output-{number}"));
        let body = run_text(step).unwrap_or_else(|| panic!("no run: {step}"));
        fs::write(&script, expressions(body, &value)).expect("the step's script");
        fs::write(&output, "").expect("GITHUB_OUTPUT");
        let temp = self.scratch.dir("runner-temp");
        let mut path = OsString::from(&self.bin);
        path.push(":/usr/bin:/bin");
        let mut env: Vec<(String, OsString)> = vec![
            ("PATH".to_owned(), path),
            ("HOME".to_owned(), self.scratch.root.clone().into()),
            ("GITHUB_REF_NAME".to_owned(), format!("v{version}").into()),
            (
                "GITHUB_REF".to_owned(),
                format!("refs/tags/v{version}").into(),
            ),
            ("GITHUB_REPOSITORY_OWNER".to_owned(), "MoloF".into()),
            ("GITHUB_OUTPUT".to_owned(), output.clone().into()),
            ("RUNNER_TEMP".to_owned(), temp.into()),
            (
                "STUB_PUBLISHED".to_owned(),
                OsString::from(if self.published { "1" } else { "0" }),
            ),
        ];
        for level in [workflow.get("env"), job.get("env"), step.get("env")]
            .into_iter()
            .flatten()
        {
            for (name, setting) in object(level, "an env") {
                env.push((
                    name.clone(),
                    expressions(&scalar(setting, name), &value).into(),
                ));
            }
        }
        let shell = step
            .get("shell")
            .or_else(|| find(job, &["defaults", "run", "shell"]))
            .or_else(|| find(workflow, &["defaults", "run", "shell"]))
            .and_then(Value::as_str);
        let mut args: Vec<OsString> = match shell {
            None => vec!["-e".into()],
            Some("bash") => ["--noprofile", "--norc", "-eo", "pipefail"]
                .iter()
                .map(OsString::from)
                .collect(),
            Some(other) => panic!("a shell the test cannot run: {other}"),
        };
        args.push(script.into());
        let ran = run(
            "a step of image.yml",
            "/bin/bash",
            &args,
            &self.scratch.root,
            &env,
        );
        let written = read(&output)
            .lines()
            .filter_map(|line| line.split_once('='))
            .map(|(name, value)| (name.to_owned(), value.to_owned()))
            .collect();
        (ran, written)
    }

    /// Step `at` of `job`, after each earlier step whose outputs it names.
    fn run(&self, workflow: &Value, job: &Value, at: usize) -> Ran {
        let steps = steps(job);
        let mut named = Vec::new();
        every_string(steps[at], &mut named);
        let reference =
            Regex::new(r"steps\.([A-Za-z0-9_-]+)\.outputs\.([A-Za-z0-9_-]+)").expect("a pattern");
        let ids: BTreeSet<String> = named
            .iter()
            .flat_map(|text| {
                reference
                    .captures_iter(text)
                    .map(|found| found[1].to_owned())
                    .collect::<Vec<_>>()
            })
            .collect();
        let mut outputs = BTreeMap::new();
        for id in ids {
            let earlier = steps[..at]
                .iter()
                .find(|step| step.get("id").and_then(Value::as_str) == Some(id.as_str()))
                .unwrap_or_else(|| panic!("no earlier step {id}"));
            let (ran, written) = self.execute(workflow, job, earlier, &BTreeMap::new());
            assert_eq!(ran.code, 0, "step {id}: {}", ran.show());
            for (name, value) in written {
                outputs.insert(format!("steps.{id}.outputs.{name}"), value);
            }
        }
        self.execute(workflow, job, steps[at], &outputs).0
    }
}

/// D15 (iteration 2): every job that pushes runs `docker buildx imagetools
/// inspect "$IMAGE:$version"` before its first push, and fails when it
/// succeeds. Run as a runner would, with a stub `docker`, the check's step
/// exits non-zero for a published version, having asked for the compose
/// file's image at the tag's version and pushed nothing, and 0 for a new
/// one. M: drop the existing-release check.
#[test]
fn d15_every_pushing_job_refuses_a_published_version_before_it_pushes() {
    let workflow = workflow();
    let jobs = object(at(&workflow, &["jobs"]), "jobs");
    let pushing: Vec<(&String, &Value)> = jobs.iter().filter(|(_, job)| pushes(job)).collect();
    let names: BTreeSet<&str> = pushing.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(
        names,
        BTreeSet::from(["build", "merge"]),
        "the jobs that push"
    );
    let compose = compose();
    let image = string(
        at(service(&compose, DAEMON), &["image"]),
        "specengine's image",
    );
    let (name, _) = image.split_once(':').expect("a tag");
    let asked = format!("buildx imagetools inspect {name}:{}", plugin_version());
    for (job_name, job) in pushing {
        let steps = steps(job);
        let first_push = steps
            .iter()
            .position(|step| step_pushes(step))
            .expect("a push");
        let check = steps
            .iter()
            .position(|step| run_text(step).is_some_and(|run| run.contains(INSPECT)))
            .unwrap_or_else(|| panic!("{job_name}: no `{INSPECT}`"));
        assert!(
            check <= first_push,
            "{job_name}: the release check (step {check}) comes before the first push (step {first_push})"
        );
        if check == first_push {
            let run = run_text(steps[check]).expect("a run");
            let inspect = run.find(INSPECT).expect("the check");
            let push = push_at(run).expect("the push");
            assert!(
                inspect < push,
                "{job_name}: the release check comes before the push in its step"
            );
        }

        let published = Runner::new(true);
        let ran = published.run(&workflow, job, check);
        let log = published.log();
        assert_ne!(
            ran.code,
            0,
            "{job_name}: a published version fails: {}",
            ran.show()
        );
        assert!(
            log.lines().any(|line| line == asked),
            "{job_name}: asks for `{asked}`: {log}"
        );
        assert!(
            !log.lines()
                .any(|line| line.starts_with("push ")
                    || line.starts_with("buildx imagetools create ")),
            "{job_name}: pushed over a published version: {log}"
        );
        let new = Runner::new(false);
        let ran = new.run(&workflow, job, check);
        assert_eq!(
            ran.code,
            0,
            "{job_name}: a new version passes: {}\n{}",
            ran.show(),
            new.log()
        );
    }
}

/// D15 (iteration 2): `build` and `merge`, and any job that builds or
/// pushes, carry `if: github.repository_owner == 'MoloF'`, so a fork's tag
/// publishes nothing. M: drop the owner condition.
#[test]
fn d15_build_and_merge_run_on_the_owners_repository_alone() {
    let workflow = workflow();
    let jobs = object(at(&workflow, &["jobs"]), "jobs");
    let mut gated = BTreeSet::from(["build", "merge"]);
    gated.extend(
        jobs.iter()
            .filter(|(_, job)| pushes(job) || builds(job))
            .map(|(name, _)| name.as_str()),
    );
    for name in gated {
        let job = at(&workflow, &["jobs", name]);
        assert_eq!(
            job.get("if")
                .map(|condition| expression_text(&scalar(condition, name)))
                .as_deref(),
            Some("github.repository_owner == 'MoloF'"),
            "{name} runs on the owner's repository alone"
        );
    }
}

/// `tag-check.sh <tag>` from `cwd`, `PATH` the system's alone.
fn tag_check(script: &Path, tag: &str, cwd: &Path) -> Ran {
    let env: Vec<(String, OsString)> = vec![
        ("PATH".to_owned(), "/usr/bin:/bin".into()),
        ("HOME".to_owned(), cwd.into()),
    ];
    run(
        &format!("tag-check.sh {tag}"),
        "/bin/sh",
        &[script.into(), tag.into()],
        cwd,
        &env,
    )
}

fn refused_tag(ran: &Ran, tag: &str, version: &str) {
    assert_eq!(ran.code, 1, "{}", ran.show());
    assert_eq!(ran.stdout, "", "{}", ran.show());
    let line = format!("tag-check: {tag} is not v{version} (plugin.json, .env.example)");
    assert!(
        ran.stderr.lines().any(|own| own == line),
        "{line}: {}",
        ran.show()
    );
}

/// D15: `tag-check.sh` exits 0 printing the plugin's version for `v` + it,
/// and 1 with "Data"'s line for the next patch, the bare version and an
/// `-rc1`; in a scratch copy of its tree, run from elsewhere, it reads the
/// copy's `plugin.json` and `.env.example`, and a `.env.example` off the
/// plugin's version fails. M: compare the tag with a prefix match.
#[test]
fn d15_tag_check_takes_v_and_the_plugins_version_exactly() {
    let version = plugin_version();
    let script = deploy().join("tag-check.sh");
    let scratch = Scratch::new("d15");
    let elsewhere = scratch.dir("elsewhere");
    let ran = tag_check(&script, &format!("v{version}"), &elsewhere);
    assert_eq!(ran.code, 0, "{}", ran.show());
    assert_eq!(ran.stdout, format!("{version}\n"));
    let (head, patch) = version.rsplit_once('.').expect("x.y.z");
    let next = format!("{head}.{}", patch.parse::<u64>().expect("a patch") + 1);
    for tag in [
        format!("v{next}"),
        version.clone(),
        format!("v{version}-rc1"),
    ] {
        refused_tag(&tag_check(&script, &tag, &elsewhere), &tag, &version);
    }

    let tree = scratch.dir("tree");
    let copy = tree.join("scripts/deploy/tag-check.sh");
    fs::create_dir_all(copy.parent().expect("a folder")).expect("the copy's folder");
    fs::copy(script, &copy).expect("a copy of tag-check.sh");
    let plugin = read(&repository().join("plugin/specengine/.claude-plugin/plugin.json"));
    let example = read(&deploy().join(".env.example"));
    let other = "7.8.9";
    let write_tree = |plugin_version: &str, example_version: &str| {
        let manifest = tree.join("plugin/specengine/.claude-plugin/plugin.json");
        fs::create_dir_all(manifest.parent().expect("a folder")).expect("plugin's folder");
        let pinned = format!("\"version\": \"{version}\"");
        assert!(plugin.contains(&pinned), "plugin.json spells {pinned}");
        fs::write(
            manifest,
            plugin.replace(&pinned, &format!("\"version\": \"{plugin_version}\"")),
        )
        .expect("plugin.json");
        let line = format!("SPECENGINE_VERSION={version}");
        assert!(example.contains(&line), ".env.example spells {line}");
        fs::write(
            tree.join("scripts/deploy/.env.example"),
            example.replace(&line, &format!("SPECENGINE_VERSION={example_version}")),
        )
        .expect(".env.example");
    };
    write_tree(other, other);
    let ran = tag_check(&copy, &format!("v{other}"), &elsewhere);
    assert_eq!(ran.code, 0, "the copy's own files: {}", ran.show());
    assert_eq!(ran.stdout, format!("{other}\n"));
    refused_tag(
        &tag_check(&copy, &format!("v{version}"), &elsewhere),
        &format!("v{version}"),
        other,
    );
    write_tree(other, &version);
    let ran = tag_check(&copy, &format!("v{other}"), &elsewhere);
    assert_eq!(
        ran.code,
        1,
        ".env.example off the plugin's version: {}",
        ran.show()
    );
}
