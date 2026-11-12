//! docs/features/hub-ops.md (RM-0074, Phase 7 step 11b-2) OP1-OP5, OP7-OP10
//! and OP16-OP22: the hub's operations as committed in `scripts/deploy/`, with
//! no network, no image build and no Docker. OP6 and OP23 are
//! `tests/deploy.rs`; OP11-OP15, the door's half, are `tests/deploy_admin.rs`.
//!
//! - OP1-OP5: `docker-compose.yml`, the Dockerfile, its dockerignore and
//!   `image.yml`, read as `tests/deploy.rs` reads them (`serde-saphyr` into
//!   `serde_json::Value`, the Dockerfile's instructions by line).
//! - OP7: `health.sh` under `bash` against `std::net::TcpListener`s on free
//!   loopback ports, given through its two variables. Beside the criteria, C4's
//!   twin on the laptop: the probe against this build's `specengine-http`,
//!   with and without `--public-origin`, which also leaves nothing in the
//!   daemon (HO12's twin).
//! - OP8-OP10 and OP16-OP21: `ops/hub-ops` under `sh` against a scratch
//!   `HUB_OPS_ROOT` laid out as the three sidecars mount it (`projects/`,
//!   `data/`, `dumps/`, `snapshot/`, `state/`, `cache/`, `run/secrets/`), with
//!   stubs first on `PATH`: `spec` (each call's arguments kept; `--out` written
//!   with a line naming the project and the call; a project listed in
//!   `spec-fail` exits 2 with one stderr line, one in `spec-hold` waits for
//!   `spec-release`), `restic` (each call's arguments and environment kept, a
//!   `backup`'s folder copied as it was at the call; `restic-exit` gives a
//!   subcommand's exit), `date` (the clock file, `-u` noted when missing) and
//!   `sleep` (each argument kept; it advances the clock when `sleep-advance`
//!   exists, and sends TERM to its parent at the call `sleep-term-at` names).
//!   Each run's environment is its service's in "Compose": no `HUB_HOST` for
//!   `queue`, the restic variables for `backup`'s modes; a collating locale,
//!   so only the script's own `LC_ALL=C` gives byte order. Every run is under
//!   `perl`'s self-kill alarm with `umask 022`, so the files' modes are the
//!   script's own `umask`.
//! - Ages: "older than 35 minutes" is `find -mmin`, the files' real times, and
//!   a generation's name is the stub clock's. A test's generation is named as
//!   old as its real time says, so either reading gives the same age.
//! - OP22: every script test runs under each shell of [`shells`] (`/bin/sh`,
//!   and `/bin/dash` where it exists: macOS ships one, CI's `/bin/sh` is
//!   dash), and every probe test under each `bash` found.
//! - Beside the criteria, from "Rules": R3's two rounds in one second, R11's
//!   removed folder, R2's untouched projects; and a `queue` round over two
//!   real projects of different nature (`fixtures/spec-a`, `fixtures/spec-b`)
//!   with the built `spec`, whose dumps are format 4 queues.
//!
//! C4 and C5 run on CI alone; OP5 checks their steps exist. VF4-VF9 and
//! HO1-HO17 are the owner's.

#![cfg(unix)]

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs;
use std::io::{Read as _, Write as _};
use std::net::TcpListener;
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::os::unix::process::ExitStatusExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use common::Scratch;
use regex::Regex;
use serde_json::{Map, Value};

/// The hub's host `backup` is given (`HUB_HOST`).
const HOST: &str = "hub.example.org";

/// A script run longer than this is killed and the test fails.
const RUN_TIMEOUT: Duration = Duration::from_secs(60);

/// `perl`'s self-kill alarm over every script run, seconds.
const ALARM_SECS: u32 = 120;

/// 2026-11-04T00:00:00Z, a UTC midnight: `DAY + 11_100` is the 03:05 slot.
const DAY: i64 = 1_793_750_400;

/// The default `restic_env`, "Secrets"' example with values a test can look
/// for: none may ever be printed (R5).
const RESTIC_ENV: &str = "# the hub's dumps, an S3-compatible store
RESTIC_REPOSITORY=s3:https://s3.example.org/hub-dumps
AWS_ACCESS_KEY_ID=key-id-4f1e
AWS_SECRET_ACCESS_KEY=secret-key-9c2d
";

/// The values of [`RESTIC_ENV`], and the repository password.
const SECRETS: [&str; 4] = [
    "s3:https://s3.example.org/hub-dumps",
    "key-id-4f1e",
    "secret-key-9c2d",
    "restic-password-77aa",
];

/// The stub `spec`: the CLI's `spec export state --root R --out F`
/// (docs/canon/queue-backup.md "Commands"). `@REC@` is its records' folder.
const SPEC_STUB: &str = r#"#!/bin/sh
rec='@REC@'
n=1
if [ -f "$rec/spec.count" ]; then n=$(( $(cat "$rec/spec.count") + 1 )); fi
printf '%s\n' "$n" > "$rec/spec.count"
: > "$rec/spec.args.$n"
for argument in "$@"; do printf '%s\n' "$argument" >> "$rec/spec.args.$n"; done
root=
out=
while [ "$#" -gt 0 ]; do
    case $1 in
        --root) root=$2; shift 2 ;;
        --out) out=$2; shift 2 ;;
        *) shift ;;
    esac
done
name=${root##*/}
listed() {
    case " $(cat "$rec/$1" 2>/dev/null) " in
        *" $name "*) return 0 ;;
    esac
    return 1
}
if listed spec-hold; then
    : > "$rec/spec-held"
    waited=0
    while [ ! -e "$rec/spec-release" ] && [ "$waited" -lt 600 ]; do
        /bin/sleep 0.1
        waited=$((waited + 1))
    done
fi
if listed spec-fail; then
    echo "spec: export state: the stub refuses (call $n)" >&2
    exit 2
fi
[ -n "$out" ] || { echo "spec: the stub was given no --out" >&2; exit 2; }
printf '{"format":3,"stub":"%s","call":%s}\n' "$name" "$n" > "$out" || exit 2
printf 'wrote %s: 0 proposal(s), 0 task(s), 0 run(s), 0 card(s), 0 event(s)\n' "$out"
"#;

/// The stub `restic`. `@REC@` is its records' folder; `restic-said`, when
/// present, is what every call prints on stderr, its last line also a
/// failing call's.
const RESTIC_STUB: &str = r#"#!/bin/sh
rec='@REC@'
n=1
if [ -f "$rec/restic.count" ]; then n=$(( $(cat "$rec/restic.count") + 1 )); fi
printf '%s\n' "$n" > "$rec/restic.count"
: > "$rec/restic.args.$n"
for argument in "$@"; do printf '%s\n' "$argument" >> "$rec/restic.args.$n"; done
env > "$rec/restic.env.$n"
sub=${1:-}
case $sub in
    backup)
        for last in "$@"; do :; done
        if [ -d "$last" ]; then cp -R "$last" "$rec/restic.saw.$n"; fi
        ;;
    restore)
        target=
        while [ "$#" -gt 0 ]; do
            case $1 in
                --target) target=$2; shift 2 ;;
                *) shift ;;
            esac
        done
        if [ -n "$target" ]; then
            mkdir -p "$target" && printf 'restored by the stub, call %s\n' "$n" > "$target/restored"
        fi
        ;;
esac
if [ -f "$rec/restic-said" ]; then cat "$rec/restic-said" >&2; fi
code=
if [ -f "$rec/restic-exit" ]; then
    code=$(awk -v want="$sub" '$1 == want { print $2; exit }' "$rec/restic-exit")
fi
if [ -n "$code" ] && [ "$code" != 0 ]; then
    if [ ! -f "$rec/restic-said" ]; then echo "Fatal: the stub's restic $sub exits $code" >&2; fi
    exit "$code"
fi
exit 0
"#;

/// The stub `date`: the clock file's epoch, formatted in UTC by the host's
/// own `date` (`-r` on macOS, `-d @` on GNU). A call without `-u` is noted
/// (R10). `-d @N` and `-r N` format `N` instead.
const DATE_STUB: &str = r#"#!/bin/sh
rec='@REC@'
when=$(cat "$rec/clock")
utc=no
format=
while [ "$#" -gt 0 ]; do
    case $1 in
        -u) utc=yes ;;
        -d) shift; when=${1#@} ;;
        -r) shift; when=$1 ;;
        +*) format=$1 ;;
        *) printf '%s\n' "$1" >> "$rec/date-odd" ;;
    esac
    shift
done
[ "$utc" = yes ] || printf 'date without -u\n' >> "$rec/date-not-utc"
[ -n "$format" ] || format='+%a %b %e %H:%M:%S UTC %Y'
if [ "$format" = +%s ]; then
    printf '%s\n' "$when"
    exit 0
fi
/bin/date -u -r "$when" "$format" 2>/dev/null || /bin/date -u -d "@$when" "$format"
"#;

/// The stub `sleep`: it returns at once. A hundredth call ends a loop that
/// was never meant to run that long.
const SLEEP_STUB: &str = r#"#!/bin/sh
rec='@REC@'
n=1
if [ -f "$rec/sleep.count" ]; then n=$(( $(cat "$rec/sleep.count") + 1 )); fi
printf '%s\n' "$n" > "$rec/sleep.count"
printf '%s\n' "$*" >> "$rec/sleep.log"
if [ -f "$rec/sleep-advance" ]; then
    case ${1:-} in
        '' | *[!0-9]*) ;;
        *) printf '%s\n' "$(( $(cat "$rec/clock") + $1 ))" > "$rec/clock" ;;
    esac
fi
if [ "$(cat "$rec/sleep-term-at" 2>/dev/null)" = "$n" ] || [ "$n" -ge 100 ]; then
    kill -TERM "$PPID"
fi
exit 0
"#;

// ------------------------------------------------------------- the files

fn repository() -> PathBuf {
    common::repository_root()
}

fn deploy() -> PathBuf {
    repository().join("scripts/deploy")
}

fn hub_ops() -> PathBuf {
    deploy().join("ops/hub-ops")
}

fn health_sh() -> PathBuf {
    deploy().join("health.sh")
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn bytes(path: &Path) -> Vec<u8> {
    fs::read(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn yaml(path: &Path) -> Value {
    serde_saphyr::from_str(&read(path))
        .unwrap_or_else(|error| panic!("{}: no YAML document: {error}", path.display()))
}

fn compose() -> Value {
    yaml(&deploy().join("docker-compose.yml"))
}

fn workflow() -> Value {
    yaml(&repository().join(".github/workflows/image.yml"))
}

fn mode(path: &Path) -> u32 {
    fs::metadata(path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()))
        .permissions()
        .mode()
        & 0o777
}

fn inode(path: &Path) -> u64 {
    fs::metadata(path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()))
        .ino()
}

fn text_of(path: &Path) -> &str {
    path.to_str().expect("a UTF-8 path")
}

fn executable(path: &Path, text: &str) {
    fs::write(path, text).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("an executable");
}

/// `.env.example`'s keys, in order.
fn env_example_keys() -> Vec<String> {
    let path = deploy().join(".env.example");
    read(&path)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            line.split_once('=')
                .unwrap_or_else(|| panic!("{}: no `KEY=value`: {line}", path.display()))
                .0
                .to_owned()
        })
        .collect()
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

/// A string, or a list of scalars, as a list.
fn strings(value: &Value, what: &str) -> Vec<String> {
    match value {
        Value::String(one) => vec![one.clone()],
        Value::Array(items) => items.iter().map(|item| scalar(item, what)).collect(),
        other => panic!("{what} is neither a string nor a list: {other}"),
    }
}

fn is_true(value: Option<&Value>) -> bool {
    matches!(value, Some(Value::Bool(true))) || value.and_then(Value::as_str) == Some("true")
}

fn every_string(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::String(text) => out.push(text.clone()),
        Value::Array(items) => items.iter().for_each(|item| every_string(item, out)),
        Value::Object(map) => map.values().for_each(|item| every_string(item, out)),
        _ => {}
    }
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

// ------------------------------------------------------------- compose

fn services(compose: &Value) -> &Map<String, Value> {
    object(at(compose, &["services"]), "services")
}

fn service<'a>(compose: &'a Value, name: &str) -> &'a Value {
    at(compose, &["services", name])
}

fn keys(value: &Value, what: &str) -> BTreeSet<String> {
    object(value, what).keys().cloned().collect()
}

fn image<'a>(compose: &'a Value, name: &str) -> &'a str {
    string(at(service(compose, name), &["image"]), name)
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
                let text = scalar(item, what);
                let (name, value) = text.split_once('=').unwrap_or((&text, ""));
                (name.to_owned(), value.to_owned())
            })
            .collect(),
        Some(other) => panic!("{what}'s environment: {other}"),
    }
}

/// `(source, target, "ro" or "")` of each of a service's volumes, short or
/// long syntax, trailing slashes dropped.
fn mounts(service: &Value, what: &str) -> BTreeSet<(String, String, String)> {
    let Some(volumes) = service.get("volumes") else {
        return BTreeSet::new();
    };
    list(volumes, what)
        .iter()
        .map(|volume| match volume {
            Value::String(short) => {
                let parts: Vec<&str> = short.split(':').collect();
                match parts.as_slice() {
                    [target] => (String::new(), (*target).to_owned(), String::new()),
                    [source, target] => ((*source).to_owned(), (*target).to_owned(), String::new()),
                    [source, target, flags] => (
                        (*source).to_owned(),
                        (*target).to_owned(),
                        if flags.split(',').any(|flag| flag == "ro") {
                            "ro".to_owned()
                        } else {
                            String::new()
                        },
                    ),
                    _ => panic!("{what}: a volume {short}"),
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
                if is_true(long.get("read_only")) {
                    "ro".to_owned()
                } else {
                    String::new()
                },
            ),
            other => panic!("{what}: a volume {other}"),
        })
        .map(|(source, target, ro)| {
            (
                source.trim_end_matches('/').to_owned(),
                target.trim_end_matches('/').to_owned(),
                ro,
            )
        })
        .collect()
}

fn mount_set(entries: &[(&str, &str, &str)]) -> BTreeSet<(String, String, String)> {
    entries
        .iter()
        .map(|(source, target, ro)| ((*source).to_owned(), (*target).to_owned(), (*ro).to_owned()))
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

/// The networks a service joins by `networks:`, list or mapping form.
fn joined(service: &Value, what: &str) -> BTreeSet<String> {
    match service.get("networks") {
        None => BTreeSet::new(),
        Some(Value::Array(items)) => items.iter().map(|item| scalar(item, what)).collect(),
        Some(Value::Object(map)) => map.keys().cloned().collect(),
        Some(other) => panic!("{what}'s networks: {other}"),
    }
}

fn set(items: &[&str]) -> BTreeSet<String> {
    items.iter().map(|item| (*item).to_owned()).collect()
}

/// Whether each of `paths` is ignored by a copy of `scripts/deploy/.gitignore`
/// in a scratch repository where each is a real, empty file.
fn ignored(paths: &[&str]) -> BTreeMap<String, bool> {
    let scratch = Scratch::new("ops-gitignore");
    let repo = scratch.dir("repo");
    let git = scratch.git();
    git.run(&repo, &["init", "-q", "--template="]);
    fs::write(repo.join(".gitignore"), bytes(&deploy().join(".gitignore")))
        .expect("a copy of the .gitignore");
    for path in paths {
        let file = repo.join(path);
        fs::create_dir_all(file.parent().expect("a parent")).expect("its folder");
        fs::write(file, "").expect("a file");
    }
    paths
        .iter()
        .map(|path| {
            let check = git.output(&repo, &["check-ignore", "-q", "--", path]);
            let code = check.status.code().expect("git exited");
            assert!(code == 0 || code == 1, "git check-ignore {path}: {code}");
            ((*path).to_owned(), code == 0)
        })
        .collect()
}

// ------------------------------------------------------------- the Dockerfile

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
    body: Vec<Instruction>,
}

impl Stage {
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
            stages.push(Stage { body: Vec::new() });
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

// ------------------------------------------------------------- the workflow

fn steps<'a>(workflow: &'a Value, job: &str) -> Vec<&'a Value> {
    list(at(workflow, &["jobs", job, "steps"]), "a job's steps")
        .iter()
        .collect()
}

fn run_text(step: &Value) -> Option<&str> {
    step.get("run").and_then(Value::as_str)
}

/// Every `run` line of `job`, trimmed, in order.
fn run_lines(workflow: &Value, job: &str) -> Vec<String> {
    steps(workflow, job)
        .into_iter()
        .filter_map(run_text)
        .flat_map(str::lines)
        .map(|line| line.trim().to_owned())
        .collect()
}

/// Whether `text` holds `word` as a whole word.
fn has_word(text: &str, word: &str) -> bool {
    Regex::new(&format!(
        r"(^|[^A-Za-z0-9_-]){}($|[^A-Za-z0-9_-])",
        regex::escape(word)
    ))
    .expect("a pattern")
    .is_match(text)
}

// ------------------------------------------------------------- time

/// `epoch` as `date -u +%Y%m%dT%H%M%SZ` prints it: a generation's name.
fn stamp(epoch: i64) -> String {
    let days = epoch.div_euclid(86_400);
    let secs = epoch.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}{month:02}{day:02}T{:02}{:02}{:02}Z",
        secs / 3600,
        secs / 60 % 60,
        secs % 60
    )
}

fn real_now() -> i64 {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("after 1970")
            .as_secs(),
    )
    .expect("a time")
}

/// Sets `path`'s times `seconds` back from now.
fn age(path: &Path, seconds: i64) {
    let back = Duration::from_secs(u64::try_from(seconds.max(0)).expect("seconds"));
    let when = SystemTime::now() - back;
    let file = fs::File::open(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    file.set_times(fs::FileTimes::new().set_modified(when).set_accessed(when))
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
}

/// How long ago `path` was modified, seconds.
fn seconds_old(path: &Path) -> i64 {
    let modified = fs::metadata(path)
        .and_then(|meta| meta.modified())
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    let at = i64::try_from(
        modified
            .duration_since(UNIX_EPOCH)
            .expect("after 1970")
            .as_secs(),
    )
    .expect("a time");
    real_now() - at
}

// ------------------------------------------------------------- processes

/// The shells `hub-ops` runs under (OP22): `/bin/sh` (bash 3.2 in POSIX mode
/// on macOS, dash on CI's Ubuntu), `/bin/dash` where it exists, and
/// `HUB_OPS_SH` when set; one entry per distinct binary.
fn shells() -> Vec<String> {
    let mut candidates = vec!["/bin/sh".to_owned(), "/bin/dash".to_owned()];
    if let Ok(extra) = std::env::var("HUB_OPS_SH") {
        candidates.push(extra);
    }
    let mut found: Vec<(PathBuf, String)> = Vec::new();
    for shell in candidates {
        let Ok(real) = fs::canonicalize(&shell) else {
            continue;
        };
        if !found.iter().any(|(seen, _)| *seen == real) {
            found.push((real, shell));
        }
    }
    found.into_iter().map(|(_, shell)| shell).collect()
}

/// The bashes `health.sh` runs under (OP22): `/bin/bash`, every `bash` on the
/// test's `PATH`, and `HUB_HEALTH_BASH` when set; one per distinct binary.
fn bashes() -> Vec<String> {
    let mut candidates = vec!["/bin/bash".to_owned()];
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            if dir.is_absolute() {
                candidates.push(dir.join("bash").display().to_string());
            }
        }
    }
    if let Ok(extra) = std::env::var("HUB_HEALTH_BASH") {
        candidates.push(extra);
    }
    let mut found: Vec<(PathBuf, String)> = Vec::new();
    for bash in candidates {
        let Ok(real) = fs::canonicalize(&bash) else {
            continue;
        };
        if !found.iter().any(|(seen, _)| *seen == real) {
            found.push((real, bash));
        }
    }
    found.into_iter().map(|(_, bash)| bash).collect()
}

/// One finished run.
struct Ran {
    label: String,
    /// The exit status; a death by signal `N` is `-N`, so a trapped TERM's
    /// `exit 143` and an untrapped one differ.
    code: i32,
    stdout: String,
    stderr: String,
    elapsed: Duration,
    ended: Instant,
}

impl Ran {
    fn show(&self) -> String {
        format!(
            "{}: exit {} after {:?}\n--- stdout\n{}--- stderr\n{}",
            self.label, self.code, self.elapsed, self.stdout, self.stderr
        )
    }

    fn code(&self, code: i32) -> &Self {
        assert_eq!(self.code, code, "{}", self.show());
        self
    }

    fn all(&self) -> String {
        format!("{}{}", self.stdout, self.stderr)
    }

    fn lines(&self) -> Vec<&str> {
        self.stdout
            .lines()
            .chain(self.stderr.lines())
            .filter(|line| !line.is_empty())
            .collect()
    }

    /// Every line opens `hub-ops: <mode> <t>: ` ("`hub-ops`", its Lines item).
    fn prefixed(&self, mode: &str, t: &str) -> &Self {
        let prefix = format!("hub-ops: {mode} {t}: ");
        for line in self.lines() {
            assert!(
                line.starts_with(&prefix),
                "a line not opening `{prefix}`: {line:?}\n{}",
                self.show()
            );
        }
        self
    }

    /// No line holds any of `secrets` (R5).
    fn holds_none(&self, secrets: &[&str]) -> &Self {
        for secret in secrets {
            assert!(
                !self.all().contains(secret),
                "printed {secret:?}: {}",
                self.show()
            );
        }
        self
    }

    /// A `hub-ops: ` stderr line holding every one of `words`.
    fn names(&self, words: &[&str]) -> &Self {
        assert!(
            self.stderr.lines().any(|line| line.starts_with("hub-ops: ")
                && words.iter().all(|word| line.contains(word))),
            "no `hub-ops: ` stderr line naming {words:?}: {}",
            self.show()
        );
        self
    }
}

fn drain(mut pipe: impl std::io::Read + Send + 'static) -> JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = pipe.read_to_end(&mut bytes);
        bytes
    })
}

/// `command` with stdin null, under a watchdog: `tick` is given the child's
/// PID at every poll while it runs, and `None` once when it has exited,
/// before its pipes are read to their end (a stub it left may hold them).
fn watched(label: &str, command: &mut Command, tick: &mut dyn FnMut(Option<u32>)) -> Ran {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let started = Instant::now();
    let mut child = command
        .spawn()
        .unwrap_or_else(|error| panic!("{label}: {error}"));
    let pid = child.id();
    let out = drain(child.stdout.take().expect("stdout"));
    let err = drain(child.stderr.take().expect("stderr"));
    let status = loop {
        if let Some(status) = child.try_wait().expect("wait") {
            break status;
        }
        if started.elapsed() > RUN_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            tick(None);
            panic!("{label} ran longer than {RUN_TIMEOUT:?}; killed");
        }
        tick(Some(pid));
        std::thread::sleep(Duration::from_millis(5));
    };
    let ended = Instant::now();
    tick(None);
    Ran {
        label: label.to_owned(),
        code: status
            .code()
            .unwrap_or_else(|| -status.signal().unwrap_or(0)),
        stdout: String::from_utf8_lossy(&out.join().expect("stdout")).into_owned(),
        stderr: String::from_utf8_lossy(&err.join().expect("stderr")).into_owned(),
        elapsed: ended - started,
        ended,
    }
}

fn kill_term(pid: u32) {
    let sent = Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .status()
        .expect("kill");
    assert!(sent.success(), "SIGTERM to {pid}");
}

/// What a `backup` call of the stub saw.
#[derive(Debug)]
struct Restic {
    args: Vec<String>,
    env: BTreeMap<String, String>,
    /// The folder a `backup` was given, as it was at the call.
    saw: Option<PathBuf>,
}

/// A scratch `HUB_OPS_ROOT` as the sidecars mount it, the stubs first on
/// `PATH`, their records, and the shell `hub-ops` runs under.
struct Ops {
    scratch: Scratch,
    root: PathBuf,
    bin: PathBuf,
    rec: PathBuf,
    home: PathBuf,
    cwd: PathBuf,
    shell: String,
}

impl Ops {
    fn new(label: &str, shell: &str) -> Self {
        let scratch = Scratch::new(&format!("ops-{label}"));
        let root = scratch.dir("root");
        for dir in [
            "projects",
            "data",
            "dumps",
            "snapshot",
            "state",
            "cache",
            "run/secrets",
        ] {
            fs::create_dir_all(root.join(dir)).expect("a mounted folder");
        }
        let bin = scratch.dir("bin");
        let rec = scratch.dir("rec");
        let home = scratch.home("h");
        let cwd = scratch.dir("cwd");
        for (name, stub) in [
            ("spec", SPEC_STUB),
            ("restic", RESTIC_STUB),
            ("date", DATE_STUB),
            ("sleep", SLEEP_STUB),
        ] {
            executable(&bin.join(name), &stub.replace("@REC@", text_of(&rec)));
        }
        fs::write(rec.join("clock"), format!("{}\n", real_now())).expect("the clock");
        fs::write(root.join("run/secrets/restic_env"), RESTIC_ENV).expect("restic_env");
        fs::write(
            root.join("run/secrets/restic_password"),
            "restic-password-77aa\n",
        )
        .expect("restic_password");
        Self {
            scratch,
            root,
            bin,
            rec,
            home,
            cwd,
            shell: shell.to_owned(),
        }
    }

    fn clock(&self) -> i64 {
        read(&self.rec.join("clock"))
            .trim()
            .parse()
            .expect("the clock")
    }

    fn set_clock(&self, epoch: i64) {
        fs::write(self.rec.join("clock"), format!("{epoch}\n")).expect("the clock");
    }

    fn control(&self, name: &str, text: &str) {
        fs::write(self.rec.join(name), text).expect("a control file");
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    fn put(&self, relative: &str, text: &str) {
        let path = self.path(relative);
        fs::create_dir_all(path.parent().expect("a parent")).expect("its folder");
        fs::write(&path, text).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    }

    /// A folder of `/projects` holding a `specengine.toml`.
    fn project(&self, name: &str) {
        self.put(&format!("projects/{name}/specengine.toml"), "");
    }

    /// A complete generation `dumps/<part>/<stamp(t)>/` holding `files`, the
    /// folder 700 and the files 600, as old in real time as `t` is before
    /// the clock.
    fn generation(&self, part: &str, t: i64, files: &[(&str, &str)]) -> PathBuf {
        let dir = self.path(&format!("dumps/{part}/{}", stamp(t)));
        fs::create_dir_all(&dir).expect("a generation");
        let back = self.clock() - t;
        for (name, text) in files {
            let file = dir.join(name);
            fs::write(&file, text).expect("a dump");
            fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).expect("600");
            age(&file, back);
        }
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).expect("700");
        age(&dir, back);
        dir
    }

    /// A fresh queue and door generation, five minutes old.
    fn fresh_parts(&self) -> (PathBuf, PathBuf) {
        let t = self.clock() - 300;
        let queue = self.generation(
            "queue",
            t,
            &[
                ("a.jsonl", "{\"queue\":\"a\"}\n"),
                ("a-1.jsonl", "{\"queue\":\"a-1\"}\n"),
            ],
        );
        let door = self.generation(
            "door",
            t,
            &[
                ("users.yml", "users:\n"),
                ("mcp-users", "mira:$2b$10$x\n"),
                ("acts.log", "2026-11-04T09:00:00Z\tadd\tmira\t-\n"),
                ("webauthn.yml", "webauthn: []\n"),
                ("totp.yml", "totp: []\n"),
            ],
        );
        (queue, door)
    }

    /// The names under `dumps/<part>`, sorted, dot-names included.
    fn entries(&self, part: &str) -> Vec<String> {
        let Ok(found) = fs::read_dir(self.path(&format!("dumps/{part}"))) else {
            return Vec::new();
        };
        let mut names: Vec<String> = found
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    /// The environment of `args`' service ("Compose"): `queue` is
    /// `dump-queue`'s, every other mode `backup`'s, `HUB_HOST` given when
    /// `host`.
    fn env(&self, args: &[&str], host: bool) -> Vec<(OsString, OsString)> {
        let mut path = OsString::from(&self.bin);
        path.push(":/usr/bin:/bin");
        let mut env: Vec<(OsString, OsString)> = vec![
            ("PATH".into(), path),
            ("HOME".into(), self.home.clone().into()),
            ("LANG".into(), "en_US.UTF-8".into()),
            ("LC_ALL".into(), "en_US.UTF-8".into()),
            ("HUB_OPS_ROOT".into(), self.root.clone().into()),
        ];
        if args.first() == Some(&"queue") {
            env.push(("XDG_DATA_HOME".into(), self.path("data").into()));
        } else {
            if host {
                env.push(("HUB_HOST".into(), HOST.into()));
            }
            env.push((
                "RESTIC_PASSWORD_FILE".into(),
                self.path("run/secrets/restic_password").into(),
            ));
            env.push(("RESTIC_CACHE_DIR".into(), self.path("cache").into()));
        }
        env
    }

    fn command(&self, args: &[&str], env: &[(OsString, OsString)]) -> Command {
        let mut command = Command::new("/usr/bin/perl");
        command
            .env_clear()
            .envs(env.iter().map(|(name, value)| (name, value)))
            .current_dir(&self.cwd)
            .arg("-e")
            .arg(format!(
                "alarm {ALARM_SECS}; umask 022; exec @ARGV or die \"exec: $!\""
            ))
            .arg(&self.shell)
            .arg(hub_ops())
            .args(args);
        command
    }

    fn run(&self, args: &[&str]) -> Ran {
        self.run_with(args, true, &mut |_: Option<u32>| {})
    }

    fn run_with(&self, args: &[&str], host: bool, tick: &mut dyn FnMut(Option<u32>)) -> Ran {
        let env = self.env(args, host);
        let label = format!("{} hub-ops {args:?}", self.shell);
        let ran = watched(&label, &mut self.command(args, &env), tick);
        assert!(
            !self.rec.join("date-not-utc").exists(),
            "{label}: a `date` without -u (R10)"
        );
        ran
    }

    fn count(&self, name: &str) -> usize {
        fs::read_to_string(self.rec.join(format!("{name}.count")))
            .map(|text| text.trim().parse().expect("a count"))
            .unwrap_or(0)
    }

    fn lines_of(&self, name: &str) -> Vec<String> {
        read(&self.rec.join(name))
            .lines()
            .map(str::to_owned)
            .collect()
    }

    fn spec_calls(&self) -> Vec<Vec<String>> {
        (1..=self.count("spec"))
            .map(|n| self.lines_of(&format!("spec.args.{n}")))
            .collect()
    }

    fn restic_calls(&self) -> Vec<Restic> {
        (1..=self.count("restic"))
            .map(|n| {
                let saw = self.rec.join(format!("restic.saw.{n}"));
                Restic {
                    args: self.lines_of(&format!("restic.args.{n}")),
                    env: self
                        .lines_of(&format!("restic.env.{n}"))
                        .iter()
                        .filter_map(|line| line.split_once('='))
                        .map(|(name, value)| (name.to_owned(), value.to_owned()))
                        .collect(),
                    saw: saw.is_dir().then_some(saw),
                }
            })
            .collect()
    }

    fn restic_args(&self) -> Vec<Vec<String>> {
        self.restic_calls()
            .into_iter()
            .map(|call| call.args)
            .collect()
    }

    fn sleeps(&self) -> Vec<String> {
        if self.rec.join("sleep.log").exists() {
            self.lines_of("sleep.log")
        } else {
            Vec::new()
        }
    }
}

fn words(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_owned()).collect()
}

/// `restic backup --host <HUB_HOST> --tag <part> <root>/snapshot/<part>`, as
/// "An `upload` round" step 3 runs it.
fn backup_call(root: &Path, part: &str) -> Vec<String> {
    words(&[
        "backup",
        "--host",
        HOST,
        "--tag",
        part,
        &format!("{}/snapshot/{part}", root.display()),
    ])
}

/// `restic forget` of `part`, as "An `upload` round" step 4 runs it.
fn forget_call(part: &str) -> Vec<String> {
    words(&[
        "forget",
        "--host",
        HOST,
        "--tag",
        part,
        "--keep-within",
        "48h",
        "--keep-daily",
        "30",
    ])
}

// ------------------------------------------------------------- OP1-OP3: compose

/// docs/features/hub-ops.md "Acceptance criteria" OP1, and "Compose": each
/// new service has exactly its keys, mounts, secrets and environment;
/// `dump-queue` runs `specengine`'s image and `dump-door` `authelia`'s, byte
/// for byte; `backup` a pinned restic of 0.17 or later; none publishes,
/// escalates or shares a namespace. `dump-door` and `admin`, on the door's
/// image, carry exactly `healthcheck: {disable: true}`, so the image's own
/// `HEALTHCHECK` is not inherited (the orchestrator's decision of
/// 2026-11-04, OP1 amended at ship); any other healthcheck there fails. M:
/// `network_mode: service:traefik` on `dump-queue`; `backup` on `default`;
/// `/projects` writable in `dump-queue`; `restic_env` granted to
/// `dump-door`; `hub-dumps` writable in `backup`; `init` dropped from one;
/// `restic/restic:latest`; the `disable` removed from `dump-door` or
/// `admin`.
#[test]
fn op01_each_new_service_has_exactly_what_compose_gives_it() {
    let compose = compose();
    let new = ["dump-queue", "dump-door", "backup"];
    for name in new {
        let service = service(&compose, name);
        for key in [
            "ports",
            "expose",
            "privileged",
            "cap_add",
            "pid",
            "ipc",
            "devices",
        ] {
            assert!(service.get(key).is_none(), "{name} has `{key}`");
        }
        let network = service
            .get("network_mode")
            .map(|mode| scalar(mode, name))
            .unwrap_or_default();
        assert!(
            !network.starts_with("service:")
                && !network.starts_with("container:")
                && network != "host",
            "{name}: network_mode {network}"
        );
        assert_eq!(
            service.get("restart").and_then(Value::as_str),
            Some("unless-stopped"),
            "{name}'s restart"
        );
        assert!(is_true(service.get("init")), "{name}: init: true (S5)");
        assert_eq!(
            service
                .get("entrypoint")
                .map(|entrypoint| strings(entrypoint, name)),
            Some(words(&["/bin/sh", "/opt/hub-ops/hub-ops"])),
            "{name}'s entrypoint"
        );
    }

    let queue = service(&compose, "dump-queue");
    assert_eq!(
        keys(queue, "dump-queue"),
        set(&[
            "image",
            "restart",
            "init",
            "network_mode",
            "entrypoint",
            "command",
            "volumes"
        ]),
        "dump-queue's keys"
    );
    assert_eq!(
        image(&compose, "dump-queue"),
        image(&compose, "specengine"),
        "dump-queue runs the hub's image, byte for byte"
    );
    assert_eq!(
        queue.get("network_mode").and_then(Value::as_str),
        Some("none")
    );
    assert_eq!(strings(at(queue, &["command"]), "command"), ["queue"]);
    assert_eq!(
        mounts(queue, "dump-queue"),
        mount_set(&[
            ("./ops", "/opt/hub-ops", "ro"),
            ("./projects", "/projects", "ro"),
            ("specengine-data", "/data", ""),
            ("hub-dumps", "/dumps", ""),
        ]),
        "dump-queue's mounts"
    );

    let door = service(&compose, "dump-door");
    for name in ["dump-door", "admin"] {
        assert_eq!(
            service(&compose, name).get("healthcheck"),
            Some(&serde_json::json!({"disable": true})),
            "{name}: exactly `healthcheck: {{disable: true}}`, the door image's own one off"
        );
    }
    assert_eq!(
        keys(door, "dump-door"),
        set(&[
            "image",
            "restart",
            "init",
            "network_mode",
            "entrypoint",
            "command",
            "environment",
            "secrets",
            "volumes",
            "healthcheck"
        ]),
        "dump-door's keys"
    );
    assert_eq!(
        image(&compose, "dump-door"),
        image(&compose, "authelia"),
        "dump-door runs the door's image, byte for byte"
    );
    assert_eq!(
        door.get("network_mode").and_then(Value::as_str),
        Some("none")
    );
    assert_eq!(strings(at(door, &["command"]), "command"), ["door"]);
    assert_eq!(
        environment(door, "dump-door"),
        BTreeMap::from([(
            "AUTHELIA_STORAGE_ENCRYPTION_KEY_FILE".to_owned(),
            "/run/secrets/authelia_storage_key".to_owned()
        )])
    );
    assert_eq!(grants(door, "dump-door"), set(&["authelia_storage_key"]));
    assert_eq!(
        mounts(door, "dump-door"),
        mount_set(&[
            ("./ops", "/opt/hub-ops", "ro"),
            ("./admin", "/opt/hub-admin", "ro"),
            ("./admin-log", "/var/lib/hub-admin", ""),
            ("./authelia/users", "/users", "ro"),
            ("./traefik/mcp-users", "/mcp-users", "ro"),
            ("authelia-data", "/var/lib/authelia", ""),
            ("hub-dumps", "/dumps", ""),
        ]),
        "dump-door's mounts"
    );

    let backup = service(&compose, "backup");
    assert_eq!(
        keys(backup, "backup"),
        set(&[
            "image",
            "restart",
            "init",
            "hostname",
            "networks",
            "entrypoint",
            "command",
            "environment",
            "secrets",
            "tmpfs",
            "volumes",
            "healthcheck"
        ]),
        "backup's keys"
    );
    let pinned = Regex::new(r"^restic/restic:([0-9]+)\.([0-9]+)\.([0-9]+)@sha256:[0-9a-f]{64}$")
        .expect("a pattern");
    let restic = image(&compose, "backup");
    let found = pinned
        .captures(restic)
        .unwrap_or_else(|| panic!("backup: restic/restic:<x.y.z>@sha256:<64 hex>: {restic}"));
    let version: (u64, u64) = (
        found[1].parse().expect("a major"),
        found[2].parse().expect("a minor"),
    );
    assert!(version >= (0, 17), "restic 0.17 or later: {restic}");
    assert_eq!(
        backup.get("hostname").and_then(Value::as_str),
        Some("hub-backup"),
        "restic's lock host"
    );
    assert_eq!(
        joined(backup, "backup"),
        set(&["backup"]),
        "backup's network"
    );
    assert!(backup.get("network_mode").is_none());
    assert_eq!(strings(at(backup, &["command"]), "command"), ["upload"]);
    assert_eq!(
        environment(backup, "backup"),
        BTreeMap::from([
            ("HUB_HOST".to_owned(), "${HUB_HOST}".to_owned()),
            (
                "RESTIC_PASSWORD_FILE".to_owned(),
                "/run/secrets/restic_password".to_owned()
            ),
            ("RESTIC_CACHE_DIR".to_owned(), "/cache".to_owned()),
        ])
    );
    assert_eq!(
        grants(backup, "backup"),
        set(&["restic_password", "restic_env"])
    );
    assert_eq!(
        strings(at(backup, &["tmpfs"]), "backup's tmpfs")
            .into_iter()
            .collect::<BTreeSet<_>>(),
        set(&["/snapshot", "/state"])
    );
    assert_eq!(
        mounts(backup, "backup"),
        mount_set(&[
            ("./ops", "/opt/hub-ops", "ro"),
            ("hub-dumps", "/dumps", "ro"),
            ("restic-cache", "/cache", ""),
        ]),
        "backup's mounts"
    );
    let check = at(backup, &["healthcheck"]);
    assert_eq!(
        keys(check, "backup's healthcheck"),
        set(&["test", "interval", "timeout", "retries", "start_period"])
    );
    assert_eq!(
        strings(at(check, &["test"]), "test"),
        words(&["CMD", "/bin/sh", "/opt/hub-ops/hub-ops", "fresh"])
    );
    for (key, want) in [
        ("interval", 300.0),
        ("timeout", 10.0),
        ("start_period", 1200.0),
    ] {
        let value = scalar(at(check, &[key]), key);
        assert!(
            (seconds(&value) - want).abs() < f64::EPSILON,
            "backup's healthcheck {key}: {value}"
        );
    }
    assert_eq!(scalar(at(check, &["retries"]), "retries"), "1");
}

/// docs/features/hub-ops.md "Acceptance criteria" OP2, and "Compose": the
/// dumps' volume is tmpfs (`size=`, `uid=10001` for the dumper's user, and
/// `mode=0700`); the network `backup` holds `backup` alone; the two restic
/// secrets are files under `./secrets/`; `.env.example` gains no key and its
/// keys are still exactly the compose file's reads (D10); `.gitignore`
/// ignores `restore/`. M: `hub-dumps` a plain volume; `authelia` on `backup`;
/// `RESTIC_REPOSITORY` in `.env.example`; `restore/` dropped.
#[test]
fn op02_the_dumps_live_on_tmpfs_and_backup_has_its_network_alone() {
    let compose = compose();
    let volumes = at(&compose, &["volumes"]);
    assert_eq!(
        keys(volumes, "volumes"),
        set(&[
            "specengine-data",
            "authelia-data",
            "traefik-acme",
            "hub-dumps",
            "restic-cache"
        ])
    );
    let dumps = at(volumes, &["hub-dumps"]);
    assert_eq!(
        dumps.get("driver").and_then(Value::as_str),
        Some("local"),
        "hub-dumps' driver"
    );
    let options = at(dumps, &["driver_opts"]);
    assert_eq!(
        options.get("type").and_then(Value::as_str),
        Some("tmpfs"),
        "hub-dumps is tmpfs: no dump touches the host's disk (S5, R4)"
    );
    assert_eq!(options.get("device").and_then(Value::as_str), Some("tmpfs"));
    let o = scalar(at(options, &["o"]), "o");
    let o: Vec<&str> = o.split(',').collect();
    assert!(o.iter().any(|item| item.starts_with("size=")), "{o:?}");
    assert!(o.contains(&"mode=0700"), "{o:?}");
    assert!(
        o.contains(&"uid=10001"),
        "the image's user writes it: {o:?}"
    );

    assert_eq!(
        keys(at(&compose, &["networks"]), "networks"),
        set(&["backup"])
    );
    let on_backup: BTreeSet<String> = services(&compose)
        .iter()
        .filter(|(name, service)| joined(service, name).contains("backup"))
        .map(|(name, _)| name.clone())
        .collect();
    assert_eq!(
        on_backup,
        set(&["backup"]),
        "the network backup holds backup alone"
    );
    for (name, service) in services(&compose) {
        if name != "backup" {
            assert!(
                service.get("networks").is_none(),
                "{name} joins {:?}",
                joined(service, name)
            );
        }
    }

    for secret in ["restic_password", "restic_env"] {
        assert_eq!(
            at(&compose, &["secrets", secret]),
            &serde_json::json!({"file": format!("./secrets/{secret}")}),
            "{secret}"
        );
    }

    let keys_now = env_example_keys();
    assert_eq!(
        keys_now,
        words(&[
            "HUB_DOMAIN",
            "HUB_HOST",
            "DOOR_HOST",
            "ACME_EMAIL",
            "ACME_CA_SERVER",
            "SPECENGINE_VERSION",
            "SPECENGINE_PUSH"
        ]),
        ".env.example gains no key: no key material in .env (S1)"
    );
    let mut values = Vec::new();
    every_string(&compose, &mut values);
    let reads: BTreeSet<String> = values
        .iter()
        .flat_map(|value| interpolated(value.as_str()))
        .collect();
    assert_eq!(
        reads,
        keys_now.into_iter().collect::<BTreeSet<_>>(),
        "D10: the compose file's reads are .env.example's keys"
    );

    let found = ignored(&[
        "restore/queue/a.jsonl",
        "restore/queue/CARRIED",
        "restore/door/users.yml",
        "ops/hub-ops",
        "health.sh",
    ]);
    assert_eq!(
        found,
        BTreeMap::from([
            ("restore/queue/a.jsonl".to_owned(), true),
            ("restore/queue/CARRIED".to_owned(), true),
            ("restore/door/users.yml".to_owned(), true),
            ("ops/hub-ops".to_owned(), false),
            ("health.sh".to_owned(), false),
        ])
    );
}

/// docs/features/hub-ops.md "Acceptance criteria" OP3, and "Rules" R7: the
/// trust items stay. `specengine` is alone in Traefik's namespace; no
/// `docker.sock` anywhere; each secret reaches its services alone; `backup`
/// mounts no live volume and no host folder but `./ops`. M:
/// `specengine-data` into `backup`; the socket into `backup`; `deploy_key`
/// to `dump-queue`.
#[test]
fn op03_the_trust_items_stay() {
    let compose = compose();
    let namespace: BTreeSet<String> = services(&compose)
        .iter()
        .filter(|(_, service)| {
            service
                .get("network_mode")
                .and_then(Value::as_str)
                .is_some_and(|mode| mode.starts_with("service:") || mode.starts_with("container:"))
        })
        .map(|(name, _)| name.clone())
        .collect();
    assert_eq!(namespace, set(&["specengine"]), "Traefik's namespace");
    assert_eq!(
        service(&compose, "specengine")
            .get("network_mode")
            .and_then(Value::as_str),
        Some("service:traefik")
    );

    let mut values = Vec::new();
    every_string(&compose, &mut values);
    for value in &values {
        assert!(!value.contains("docker.sock"), "the compose file: {value}");
    }
    for script in [hub_ops(), health_sh(), deploy().join("admin/hub-admin")] {
        let text = read(&script);
        for word in ["docker.sock", "docker exec"] {
            assert!(!text.contains(word), "{}: {word}", script.display());
        }
    }

    let owners: BTreeMap<&str, BTreeSet<String>> = BTreeMap::from([
        ("authelia_jwt_secret", set(&["authelia"])),
        ("authelia_session_secret", set(&["authelia"])),
        (
            "authelia_storage_key",
            set(&["authelia", "admin", "dump-door"]),
        ),
        ("deploy_key", set(&["specengine"])),
        ("known_hosts", set(&["specengine"])),
        ("restic_password", set(&["backup"])),
        ("restic_env", set(&["backup"])),
    ]);
    assert_eq!(
        keys(at(&compose, &["secrets"]), "secrets"),
        owners
            .keys()
            .map(|name| (*name).to_owned())
            .collect::<BTreeSet<_>>()
    );
    for (name, service) in services(&compose) {
        let want: BTreeSet<String> = owners
            .iter()
            .filter(|(_, granted)| granted.contains(name))
            .map(|(secret, _)| (*secret).to_owned())
            .collect();
        assert_eq!(grants(service, name), want, "{name}'s secrets");
    }

    let sources: BTreeSet<String> = mounts(service(&compose, "backup"), "backup")
        .into_iter()
        .map(|(source, _, _)| source)
        .collect();
    for live in [
        "specengine-data",
        "authelia-data",
        "./projects",
        "./authelia/users",
        "./traefik/mcp-users",
        "./admin-log",
    ] {
        assert!(!sources.contains(live), "backup mounts {live}");
    }
    assert!(
        sources.is_subset(&set(&["./ops", "hub-dumps", "restic-cache"])),
        "backup mounts no host folder but ./ops (R7): {sources:?}"
    );
}

// ------------------------------------------------------------- OP4-OP5: the image, CI

/// The default `health.sh` gives its variable `name`: one
/// `${NAME:-N}`-style default.
fn probe_default(text: &str, name: &str) -> u16 {
    let pattern = Regex::new(&format!(r"\$\{{{name}:?[-=]([0-9]+)\}}")).expect("a pattern");
    let found: Vec<u16> = pattern
        .captures_iter(text)
        .map(|found| found[1].parse().expect("a port"))
        .collect();
    let distinct: BTreeSet<u16> = found.iter().copied().collect();
    assert_eq!(distinct.len(), 1, "one default for {name}: {found:?}");
    found[0]
}

/// docs/features/hub-ops.md "Acceptance criteria" OP4, and "Image and CI":
/// `specengine`'s healthcheck is "Compose"'s; the runtime stage copies
/// `health.sh` as `specengine-health` with `--chmod=0755`; no stage has a
/// `HEALTHCHECK` or installs `curl` or `wget`; the context keeps the probe;
/// its defaults are the entrypoint's port and `websecure`'s; it is bash. M:
/// no `COPY`; `curl` installed; the default 7778; the proxy default 80.
#[test]
fn op04_the_probe_is_in_the_image_and_its_timing_in_compose() {
    let compose = compose();
    let check = at(service(&compose, "specengine"), &["healthcheck"]);
    assert_eq!(
        keys(check, "specengine's healthcheck"),
        set(&["test", "interval", "timeout", "retries", "start_period"])
    );
    assert_eq!(
        strings(at(check, &["test"]), "test"),
        words(&["CMD", "specengine-health"])
    );
    for (key, want) in [
        ("interval", 30.0),
        ("timeout", 10.0),
        ("start_period", 120.0),
    ] {
        let value = scalar(at(check, &[key]), key);
        assert!(
            (seconds(&value) - want).abs() < f64::EPSILON,
            "specengine's healthcheck {key}: {value}"
        );
    }
    assert_eq!(scalar(at(check, &["retries"]), "retries"), "3");

    let stages = stages(instructions(&read(&deploy().join("Dockerfile"))));
    let runtime = stages.last().expect("a runtime stage");
    let copies: Vec<(BTreeMap<String, String>, Vec<String>)> = runtime
        .of("COPY")
        .map(|copy| copy_parts(&copy.args))
        .filter(|(flags, paths)| {
            !flags.contains_key("from")
                && paths
                    .first()
                    .is_some_and(|source| source.ends_with("health.sh"))
        })
        .collect();
    assert_eq!(copies.len(), 1, "one COPY of health.sh: {copies:?}");
    let (flags, paths) = &copies[0];
    assert_eq!(
        paths,
        &words(&[
            "scripts/deploy/health.sh",
            "/usr/local/bin/specengine-health"
        ])
    );
    assert_eq!(
        flags,
        &BTreeMap::from([("chmod".to_owned(), "0755".to_owned())])
    );
    for stage in &stages {
        for instruction in &stage.body {
            assert_ne!(
                instruction.keyword, "HEALTHCHECK",
                "line {}: compose holds the timing",
                instruction.line
            );
        }
        let packages = installed(stage);
        for client in ["curl", "wget"] {
            assert!(
                !packages.contains(client),
                "{client} installed: {packages:?}"
            );
        }
    }
    for run in runtime.of("RUN") {
        for client in ["curl", "wget"] {
            assert!(!has_word(&run.args, client), "line {}: {client}", run.line);
        }
    }
    let patterns = ignore_patterns(&read(&deploy().join("Dockerfile.dockerignore")));
    assert!(
        !excluded(&patterns, "scripts/deploy/health.sh"),
        "the build context keeps the probe"
    );

    let probe = read(&health_sh());
    assert_eq!(
        probe.lines().next(),
        Some("#!/bin/bash"),
        "health.sh is bash"
    );
    let entrypoint_port: Vec<u16> = Regex::new(r"(?m)^port=([0-9]+)$")
        .expect("a pattern")
        .captures_iter(&read(&deploy().join("entrypoint.sh")))
        .map(|found| found[1].parse().expect("a port"))
        .collect();
    assert_eq!(entrypoint_port.len(), 1, "the entrypoint's one port");
    assert_eq!(
        probe_default(&probe, "SPECENGINE_HEALTH_PORT"),
        entrypoint_port[0],
        "the probe's default port is the entrypoint's"
    );
    let websecure: Vec<u16> = strings(at(service(&compose, "traefik"), &["command"]), "command")
        .iter()
        .filter_map(|flag| flag.strip_prefix("--entrypoints.websecure.address="))
        .map(|address| {
            address
                .rsplit(':')
                .next()
                .expect("a port")
                .parse()
                .expect("a port")
        })
        .collect();
    assert_eq!(websecure.len(), 1, "websecure's one address");
    assert_eq!(
        probe_default(&probe, "SPECENGINE_HEALTH_PROXY_PORT"),
        websecure[0],
        "the probe's default proxy port is websecure's"
    );
}

/// The `deploy_ops` test line, as "Image and CI" words it.
const OPS_TESTS: &str = "cargo test --locked -p specengine-http --test deploy_ops";

/// docs/features/hub-ops.md "Acceptance criteria" OP5, and "Image and CI":
/// the check job runs `deploy_ops`; C5 is a step of the check job (the door
/// image's five `--help`s, restic's version and its six tools) and C4 one of
/// `build`'s, before its push (the probe against the smoke's daemon and a
/// second one with `--public-origin https://hub.example.org`, its ports
/// through the two variables). M: the test dropped.
#[test]
fn op05_the_check_job_runs_deploy_ops_and_c4_c5_are_steps() {
    let workflow = workflow();
    let lines = run_lines(&workflow, "check");
    assert!(
        lines.iter().any(|line| line == OPS_TESTS),
        "the check job runs `{OPS_TESTS}`: {lines:#?}"
    );

    let c5: Vec<&str> = steps(&workflow, "check")
        .into_iter()
        .filter_map(run_text)
        .filter(|run| {
            [
                "storage user webauthn export",
                "storage user webauthn import",
                "storage user totp export",
                "storage user totp import",
                "storage migrate up",
            ]
            .iter()
            .all(|command| run.contains(command))
        })
        .collect();
    assert_eq!(c5.len(), 1, "one C5 step in the check job");
    let c5 = c5[0];
    assert!(c5.contains("--help"), "C5 asks each command's --help: {c5}");
    assert!(
        c5.contains("restic") && c5.contains("version"),
        "C5 prints restic's version: {c5}"
    );
    for tool in ["sh", "cp", "find", "ln", "touch", "date"] {
        assert!(
            has_word(c5, tool),
            "C5 looks for {tool} in restic's image: {c5}"
        );
    }

    let build = steps(&workflow, "build");
    let c4 = build
        .iter()
        .position(|step| {
            run_text(step).is_some_and(|run| {
                run.contains("specengine-health")
                    && run.contains("SPECENGINE_HEALTH_PORT")
                    && run.contains("SPECENGINE_HEALTH_PROXY_PORT")
                    && run.contains("--public-origin https://hub.example.org")
            })
        })
        .unwrap_or_else(|| panic!("no C4 step in build: {build:#?}"));
    let push = build
        .iter()
        .position(|step| run_text(step).is_some_and(|run| run.contains("docker push")))
        .expect("build's push");
    assert!(c4 < push, "C4 runs before the push");
}

/// Beside docs/features/hub-ops.md "Acceptance criteria" OP5, the check job
/// as a runner reads it: one test of `deploy_ops` runs the built `spec`, which
/// `cargo test -p specengine-http` does not build, so an earlier cargo line
/// of the job builds `specengine-cli` before it, as for `deploy_admin`.
#[test]
fn op05_the_check_job_builds_spec_before_deploy_ops() {
    let lines = run_lines(&workflow(), "check");
    let at = lines
        .iter()
        .position(|line| line == OPS_TESTS)
        .unwrap_or_else(|| panic!("no `{OPS_TESTS}`: {lines:#?}"));
    assert!(
        lines[..at].iter().any(|line| line.starts_with("cargo ")
            && (line.contains("specengine-cli")
                || line.contains("--bin spec")
                || line.contains("--workspace"))),
        "no cargo line before `{OPS_TESTS}` builds `spec`: {lines:#?}"
    );
}

// ------------------------------------------------------------- OP7: the probe

/// How a stand-in daemon answers its one connection.
#[derive(Clone, Copy)]
enum Answer {
    /// The status line, then an empty response's headers.
    Status(&'static str),
    /// It reads the request and closes with no answer.
    Close,
    /// It reads the request and writes nothing until the probe hangs up, or
    /// 15 s pass.
    Silent,
}

/// A stand-in on a free loopback port: it accepts one connection within
/// 20 s, reads the request to its blank line and answers as `answer`. The
/// thread returns the request's bytes.
fn stand_in(answer: Answer) -> (u16, JoinHandle<Vec<u8>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a free port");
    let port = listener.local_addr().expect("its address").port();
    listener
        .set_nonblocking(true)
        .expect("a non-blocking accept");
    let handle = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if Instant::now() > deadline {
                        return Vec::new();
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(error) => panic!("accept: {error}"),
            }
        };
        stream.set_nonblocking(false).expect("a blocking stream");
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .expect("a read timeout");
        let mut request = Vec::new();
        let mut buffer = [0_u8; 1024];
        while !request.windows(4).any(|window| window == b"\r\n\r\n") {
            match stream.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(read) => request.extend_from_slice(&buffer[..read]),
            }
        }
        match answer {
            Answer::Status(line) => {
                let _ = stream.write_all(
                    format!("{line}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").as_bytes(),
                );
            }
            Answer::Close => {}
            Answer::Silent => {
                stream
                    .set_read_timeout(Some(Duration::from_secs(15)))
                    .expect("a read timeout");
                let _ = stream.read(&mut buffer);
            }
        }
        request
    });
    (port, handle)
}

/// A loopback port nothing listens on.
fn closed_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a free port");
    listener.local_addr().expect("its address").port()
}

/// `bash health.sh` with its two ports.
fn probe(bash: &str, port: u16, proxy: u16) -> Ran {
    let mut command = Command::new("/usr/bin/perl");
    command
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("SPECENGINE_HEALTH_PORT", port.to_string())
        .env("SPECENGINE_HEALTH_PROXY_PORT", proxy.to_string())
        .arg("-e")
        .arg("alarm 60; exec @ARGV or die \"exec: $!\"")
        .arg(bash)
        .arg(health_sh());
    watched(
        &format!("{bash} health.sh, port {port}, proxy {proxy}"),
        &mut command,
        &mut |_: Option<u32>| {},
    )
}

/// docs/features/hub-ops.md "Acceptance criteria" OP7, and
/// "`specengine-health`": alive is exit 0 for `HTTP/1.1 403` with the proxy
/// listening; a 200, a 502, a close with no answer, the proxy port closed and
/// no listener are exit 1 with their one line; a listener that never writes
/// is exit 1 after about 5 s; the request is exactly "Data"'s, `Host`
/// loopback and no other header. Beside the criteria ("Data": the line must
/// start `HTTP/1.1 403 `): an `HTTP/1.0 403` is not alive. M: any status
/// accepted; the proxy test dropped; `Host: hub.example.org`; a
/// `Remote-User` sent; no `-t`.
#[test]
fn op07_the_probe_is_alive_on_a_loopback_403_and_a_listening_proxy() {
    for bash in bashes() {
        let proxy = TcpListener::bind("127.0.0.1:0").expect("the proxy's port");
        let proxy_port = proxy.local_addr().expect("its address").port();

        let (port, request) = stand_in(Answer::Status("HTTP/1.1 403 Forbidden"));
        let ran = probe(&bash, port, proxy_port);
        ran.code(0);
        assert_eq!(
            ran.stdout,
            "alive: HTTP/1.1 403 Forbidden\n",
            "{}",
            ran.show()
        );
        let request = String::from_utf8(request.join().expect("the stand-in")).expect("ASCII");
        assert_eq!(
            request,
            format!("GET / HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"),
            "{bash}: the probe's request (R6)"
        );
        for header in ["remote-user", "origin", "sec-fetch-site"] {
            assert!(
                !request.to_ascii_lowercase().contains(header),
                "{bash}: the probe sends {header}"
            );
        }

        for status in [
            "HTTP/1.1 200 OK",
            "HTTP/1.1 502 Bad Gateway",
            "HTTP/1.0 403 Forbidden",
        ] {
            let (port, request) = stand_in(Answer::Status(status));
            let ran = probe(&bash, port, proxy_port);
            ran.code(1);
            assert_eq!(
                ran.stdout,
                format!(
                    "not alive: 127.0.0.1:{port} answered {status}: a hub answers a loopback probe 403\n"
                ),
                "{}",
                ran.show()
            );
            let _ = request.join();
        }

        let (port, request) = stand_in(Answer::Close);
        let ran = probe(&bash, port, proxy_port);
        ran.code(1);
        assert_eq!(
            ran.stdout,
            format!("not alive: no answer from 127.0.0.1:{port}\n"),
            "{}",
            ran.show()
        );
        let _ = request.join();

        let nothing = closed_port();
        let ran = probe(&bash, nothing, proxy_port);
        ran.code(1);
        assert_eq!(
            ran.stdout,
            format!("not alive: no answer from 127.0.0.1:{nothing}\n"),
            "{}",
            ran.show()
        );

        let shut = closed_port();
        let (port, request) = stand_in(Answer::Status("HTTP/1.1 403 Forbidden"));
        let ran = probe(&bash, port, shut);
        ran.code(1);
        assert_eq!(
            ran.stdout,
            format!(
                "not alive: nothing listens on 127.0.0.1:{shut}: the proxy left this namespace; run docker compose up -d\n"
            ),
            "{}",
            ran.show()
        );
        let _ = request.join();

        let (port, request) = stand_in(Answer::Silent);
        let ran = probe(&bash, port, proxy_port);
        ran.code(1);
        assert_eq!(
            ran.stdout,
            format!("not alive: no answer from 127.0.0.1:{port}\n"),
            "{}",
            ran.show()
        );
        assert!(
            (Duration::from_millis(4500)..Duration::from_secs(9)).contains(&ran.elapsed),
            "a silent listener: exit 1 after about 5 s (`read -t 5`): {}",
            ran.show()
        );
        let _ = request.join();
        drop(proxy);
    }
}

/// The two answers a local daemon gives the probe's loopback `GET /`
/// (`docs/canon/hub.md` "Routes"): 200, the page, when this build embeds a
/// web UI; 503, the not-built line, when `ui/dist` held no `index.html` as it
/// was compiled (`src/build.rs`). A hub's 403 is neither.
const LOCAL_ANSWERS: [u16; 2] = [200, 503];

/// Beside docs/features/hub-ops.md "Acceptance criteria" OP7: C4's twin on
/// the laptop, with this build's `specengine-http` and no image. A hub
/// (`--public-origin`) answers the probe's loopback `GET /` 403, so the probe
/// is alive; a local daemon answers it one of `LOCAL_ANSWERS`, so it is not
/// ("Decisions" S7, `docs/canon/hub.md` "Fence" step 1). Which one is the
/// build's, not the tree's (a `ui/dist` put back with old mtimes reruns no
/// build script): 200 with a web UI (the image builds one: C4's 200), the
/// not-built 503 without (CI's `check` job builds no UI). So both are
/// accepted, each only as the daemon's own `GET /` answers it, the 503 only
/// as the not-built page, and no other status, a hub's 403 above all. HO12's
/// twin: the probes leave the hub no event and no stderr line.
#[test]
fn c4_twin_the_probe_tells_a_hub_from_a_local_daemon() {
    let bash = "/bin/bash";
    let proxy = TcpListener::bind("127.0.0.1:0").expect("the proxy's port");
    let proxy_port = proxy.local_addr().expect("its address").port();

    let hub = common::hub::Setup::new("ops-probe-hub", "spec-a").start();
    std::thread::sleep(Duration::from_millis(300));
    let events = hub.events();
    let stderr = hub.server.stderr();
    for _ in 0..3 {
        let ran = probe(bash, hub.server.port, proxy_port);
        ran.code(0);
        assert!(
            ran.stdout.starts_with("alive: HTTP/1.1 403 ") && ran.stdout.lines().count() == 1,
            "{}",
            ran.show()
        );
    }
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(hub.events(), events, "a probe records no event (HO12)");
    assert_eq!(hub.server.stderr(), stderr, "a probe logs nothing (HO12)");

    let scratch = Scratch::new("ops-probe-local");
    let root = scratch.repo("spec-b", "served", "main");
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let local = common::Server::serve(&home, &cwd, &[root.as_path()]);
    let ran = probe(bash, local.port, proxy_port);
    ran.code(1);
    let answers: Vec<String> = LOCAL_ANSWERS.iter().map(u16::to_string).collect();
    let shape = Regex::new(&format!(
        r"^not alive: 127\.0\.0\.1:{} answered HTTP/1\.1 ({}) [^\n]*: a hub answers a loopback probe 403\n$",
        local.port,
        answers.join("|")
    ))
    .expect("a pattern");
    let status: u16 = shape
        .captures(&ran.stdout)
        .unwrap_or_else(|| panic!("{}", ran.show()))[1]
        .parse()
        .expect("a status");
    // The pattern admits the two answers and no other, a hub's 403 above all.
    for other in [200, 403, 404, 500, 502, 503] {
        let line = format!(
            "not alive: 127.0.0.1:{} answered HTTP/1.1 {other} Reason: a hub answers a loopback probe 403\n",
            local.port
        );
        assert_eq!(
            shape.is_match(&line),
            matches!(other, 200 | 503),
            "the pattern on a {other}: a local daemon answers 200 or 503, a hub 403"
        );
    }
    // The probe saw what the daemon's own `GET /` answers: the page, or the
    // not-built line, not some other refusal.
    let page = local.get("/");
    page.status(status);
    if status == 503 {
        let message = page.error_message();
        assert!(
            message.starts_with("this specengine-http holds no web UI"),
            "the not-built page: {message}"
        );
    } else {
        assert_eq!(
            page.header("content-type"),
            Some("text/html; charset=utf-8"),
            "the page"
        );
    }
    drop(proxy);
}

// ------------------------------------------------------------- OP8-OP10: the queue's rounds

/// The `queue` staging path of a call `--out <staging>/<name>.jsonl`.
fn staging_of(call: &[String], root: &Path, t: &str, name: &str) -> String {
    let out = call
        .last()
        .unwrap_or_else(|| panic!("a call with no argument: {call:?}"));
    let staging = out
        .strip_suffix(&format!("/{name}.jsonl"))
        .unwrap_or_else(|| panic!("--out {out} is not <staging>/{name}.jsonl"));
    let shape = Regex::new(&format!(
        r"^{}/dumps/queue/\.{t}\.[0-9]+\.tmp$",
        regex::escape(text_of(root))
    ))
    .expect("a pattern");
    assert!(
        shape.is_match(staging),
        "the staging `.<t>.<pid>.tmp` under /dumps/queue: {staging}"
    );
    staging.to_owned()
}

/// docs/features/hub-ops.md "Acceptance criteria" OP8, and "A `queue`
/// round": over projects `a`, `a-1`, `c`, a folder `b` with no
/// `specengine.toml` and a file `z`, `queue once` calls `spec export state
/// --root /projects/<n> --out <staging>/<n>.jsonl` for `a`, `a-1`, `c` in
/// that order and never for `b` or `z`, logs each line, and leaves one
/// generation named as the clock (`^[0-9]{8}T[0-9]{6}Z$`), mode 700, its
/// files 600, and no `.tmp`; the projects and `/data` are untouched (R2).
/// M: the `*/` glob; `--out` inside a project; no rename; no `umask`.
#[test]
fn op08_a_queue_round_exports_each_project_in_byte_order_into_one_generation() {
    let name_shape = Regex::new(r"^[0-9]{8}T[0-9]{6}Z$").expect("a pattern");
    for shell in shells() {
        let ops = Ops::new("op08", &shell);
        let e = DAY + 12 * 3600 + 15 * 60;
        ops.set_clock(e);
        let t = stamp(e);
        for name in ["a", "a-1", "c"] {
            ops.project(name);
        }
        ops.put("projects/b/README.md", "no config\n");
        ops.put("projects/z", "a file\n");
        let projects = common::snapshot(&ops.path("projects"));
        let data = common::snapshot(&ops.path("data"));

        let ran = ops.run(&["queue", "once"]);
        ran.code(0).prefixed("queue", &t).holds_none(&SECRETS);
        assert_eq!(
            ops.entries("queue"),
            std::slice::from_ref(&t),
            "{}",
            ran.show()
        );
        assert!(name_shape.is_match(&t), "{shell}: {t}");
        let generation = ops.path(&format!("dumps/queue/{t}"));
        assert_eq!(mode(&generation), 0o700, "{shell}: the generation's mode");

        let calls = ops.spec_calls();
        assert_eq!(calls.len(), 3, "{shell}: {calls:?}");
        let staging = staging_of(&calls[0], &ops.root, &t, "a");
        for (call, name) in calls.iter().zip(["a", "a-1", "c"]) {
            assert_eq!(
                call,
                &words(&[
                    "export",
                    "state",
                    "--root",
                    &format!("{}/projects/{name}", ops.root.display()),
                    "--out",
                    &format!("{staging}/{name}.jsonl"),
                ]),
                "{shell}: the export of {name}"
            );
            assert!(
                ran.lines()
                    .iter()
                    .any(|line| line.contains(&format!("wrote {staging}/{name}.jsonl"))),
                "{shell}: the log holds the export's line for {name}: {}",
                ran.show()
            );
        }
        let mut files: Vec<String> = fs::read_dir(&generation)
            .expect("the generation")
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        files.sort();
        assert_eq!(files, ["a-1.jsonl", "a.jsonl", "c.jsonl"], "{shell}");
        for (call, name) in [(1, "a"), (2, "a-1"), (3, "c")] {
            let file = generation.join(format!("{name}.jsonl"));
            assert_eq!(mode(&file), 0o600, "{shell}: {name}.jsonl's mode");
            assert_eq!(
                read(&file),
                format!("{{\"format\":3,\"stub\":\"{name}\",\"call\":{call}}}\n"),
                "{shell}: {name}.jsonl is its export"
            );
        }
        assert_eq!(
            common::snapshot(&ops.path("projects")),
            projects,
            "{shell}: R2"
        );
        assert_eq!(common::snapshot(&ops.path("data")), data, "{shell}: R2");
    }
}

/// Beside docs/features/hub-ops.md "Acceptance criteria" OP8: "A `queue`
/// round" takes byte order (`LC_ALL=C`) under the run's collating locale, so
/// `D` comes before `a`.
#[test]
fn data_a_queue_round_takes_byte_order_under_any_locale() {
    for shell in shells() {
        let ops = Ops::new("byte-order", &shell);
        for name in ["c", "a", "D"] {
            ops.project(name);
        }
        ops.run(&["queue", "once"]).code(0);
        let roots: Vec<String> = ops
            .spec_calls()
            .iter()
            .map(|call| call[3].rsplit('/').next().expect("a name").to_owned())
            .collect();
        assert_eq!(roots, ["D", "a", "c"], "{shell}: byte order");
    }
}

/// docs/features/hub-ops.md "Acceptance criteria" OP9, and "Rules" R12: when
/// the export of `a-1` fails (exit 2, one stderr line), the generation holds
/// the new `a.jsonl`, the previous generation's `a-1.jsonl` byte for byte
/// (a hard link, "A `queue` round" step 3) and `CARRIED` reading `a-1`; the
/// log names `a-1` and the line; the exit is 1. With no previous generation,
/// `a-1.jsonl` is absent and `CARRIED` lists it. M: no carry; exit 0; no
/// `CARRIED`.
#[test]
fn op09_a_failed_export_carries_the_previous_dump_and_names_it() {
    for shell in shells() {
        let ops = Ops::new("op09", &shell);
        let e = DAY + 9 * 3600;
        for name in ["a", "a-1", "c"] {
            ops.project(name);
        }
        ops.set_clock(e);
        ops.run(&["queue", "once"]).code(0);
        let first = ops.path(&format!("dumps/queue/{}", stamp(e)));
        let carried = bytes(&first.join("a-1.jsonl"));

        ops.set_clock(e + 900);
        ops.control("spec-fail", "a-1");
        let t = stamp(e + 900);
        let ran = ops.run(&["queue", "once"]);
        ran.code(1);
        let line = "spec: export state: the stub refuses (call 5)";
        assert!(
            ran.lines()
                .iter()
                .any(|one| one.starts_with(&format!("hub-ops: queue {t}: "))
                    && one.contains("a-1")
                    && one.contains(line)),
            "{shell}: the log names a-1 and its line: {}",
            ran.show()
        );
        assert_eq!(
            ops.entries("queue"),
            [stamp(e), t.clone()],
            "{}",
            ran.show()
        );
        let second = ops.path(&format!("dumps/queue/{t}"));
        assert_eq!(
            read(&second.join("a.jsonl")),
            "{\"format\":3,\"stub\":\"a\",\"call\":4}\n",
            "{shell}: the new a.jsonl"
        );
        assert_eq!(
            read(&second.join("c.jsonl")),
            "{\"format\":3,\"stub\":\"c\",\"call\":6}\n",
            "{shell}: the new c.jsonl"
        );
        assert_eq!(
            bytes(&second.join("a-1.jsonl")),
            carried,
            "{shell}: the previous a-1.jsonl, byte for byte"
        );
        assert_eq!(
            inode(&second.join("a-1.jsonl")),
            inode(&first.join("a-1.jsonl")),
            "{shell}: hard-linked"
        );
        assert_eq!(read(&second.join("CARRIED")), "a-1\n", "{shell}: CARRIED");

        let fresh = Ops::new("op09-none", &shell);
        for name in ["a", "a-1", "c"] {
            fresh.project(name);
        }
        fresh.set_clock(e);
        fresh.control("spec-fail", "a-1");
        let ran = fresh.run(&["queue", "once"]);
        ran.code(1);
        let only = fresh.path(&format!("dumps/queue/{}", stamp(e)));
        assert!(
            only.is_dir(),
            "{shell}: the round still lands: {}",
            ran.show()
        );
        assert!(
            !only.join("a-1.jsonl").exists(),
            "{shell}: nothing to carry"
        );
        assert!(only.join("a.jsonl").is_file() && only.join("c.jsonl").is_file());
        assert_eq!(read(&only.join("CARRIED")), "a-1\n", "{shell}: CARRIED");
    }
}

/// docs/features/hub-ops.md "Acceptance criteria" OP10, and "Staging,
/// snapshots, restore": after five `once` rounds a quarter apart, the newest
/// three generations remain; a `.tmp` two hours old is removed and one ten
/// minutes old kept. M: keep all; a young `.tmp` removed.
#[test]
fn op10_a_dumper_keeps_three_generations_and_young_staging() {
    for shell in shells() {
        let ops = Ops::new("op10", &shell);
        ops.project("a");
        let e = DAY + 6 * 3600;
        for round in 0..5 {
            let now = e + round * 900;
            ops.set_clock(now);
            if round == 4 {
                for (back, pid) in [(7200, 4711), (600, 4712)] {
                    let dir = ops.path(&format!("dumps/queue/.{}.{pid}.tmp", stamp(now - back)));
                    fs::create_dir_all(&dir).expect("a staging");
                    fs::write(dir.join("a.jsonl"), "{}\n").expect("a dump");
                    age(&dir.join("a.jsonl"), back);
                    age(&dir, back);
                }
            }
            ops.run(&["queue", "once"]).code(0);
        }
        let last = e + 4 * 900;
        assert_eq!(
            ops.entries("queue"),
            [
                format!(".{}.4712.tmp", stamp(last - 600)),
                stamp(e + 2 * 900),
                stamp(e + 3 * 900),
                stamp(last),
            ],
            "{shell}: the newest three, the young staging kept, the old one gone"
        );
    }
}

/// docs/features/hub-ops.md "Rules and edge cases" R3: two rounds in one
/// second: the second's rename fails, its staging is removed, and it exits
/// 1; the first generation stays whole.
#[test]
fn r03_two_rounds_in_one_second_leave_the_first_alone() {
    for shell in shells() {
        let ops = Ops::new("r03", &shell);
        ops.project("a");
        let e = DAY + 7 * 3600;
        ops.set_clock(e);
        ops.run(&["queue", "once"]).code(0);
        let generation = ops.path(&format!("dumps/queue/{}", stamp(e)));
        let first = common::snapshot(&generation);
        let ran = ops.run(&["queue", "once"]);
        ran.code(1);
        assert_eq!(ops.entries("queue"), [stamp(e)], "{shell}: {}", ran.show());
        assert_eq!(
            common::snapshot(&generation),
            first,
            "{shell}: the first whole"
        );
    }
}

/// docs/features/hub-ops.md "Rules and edge cases" R11: a folder removed
/// from `/projects` is absent from the next generation, which carries
/// nothing.
#[test]
fn r11_a_removed_project_leaves_the_next_generation() {
    for shell in shells() {
        let ops = Ops::new("r11", &shell);
        ops.project("a");
        ops.project("c");
        let e = DAY + 8 * 3600;
        ops.set_clock(e);
        ops.run(&["queue", "once"]).code(0);
        fs::remove_dir_all(ops.path("projects/c")).expect("c removed");
        ops.set_clock(e + 900);
        ops.run(&["queue", "once"]).code(0);
        let names: BTreeSet<String> =
            common::snapshot(&ops.path(&format!("dumps/queue/{}", stamp(e + 900))))
                .into_keys()
                .collect();
        assert_eq!(names, set(&["a.jsonl"]), "{shell}");
    }
}

/// Beside docs/features/hub-ops.md "Acceptance criteria" OP8-OP9: a `queue`
/// round over two real projects of different nature (a game design,
/// `fixtures/spec-a`, and a command-line tool, `fixtures/spec-b`) with this
/// build's `spec`: each generation file is that project's format-4 dump
/// (`docs/canon/queue-backup.md` "Format"; format 4 since card-tasks,
/// RM-0092), holding its one question, and
/// the projects are untouched (R2).
#[test]
fn data_a_queue_round_dumps_real_queues_with_the_built_spec() {
    let spec = common::spec_bin();
    for shell in shells() {
        let ops = Ops::new("real", &shell);
        let a = ops.scratch.repo("spec-a", "root/projects/a", "main");
        let b = ops.scratch.repo("spec-b", "root/projects/b", "main");
        common::ask(&ops.home, &a, &["EDGE-STAM-ZERO"], "Is the delay 1.5 s?");
        common::ask(
            &ops.home,
            &b,
            &["CMD-STATUS"],
            "Does status print the branch?",
        );
        let before_a = common::snapshot(&a);
        let before_b = common::snapshot(&b);
        fs::remove_file(ops.bin.join("spec")).expect("the stub spec out of the way");
        let e = DAY + 10 * 3600;
        ops.set_clock(e);

        let mut path = OsString::from(&ops.bin);
        path.push(":");
        path.push(spec.parent().expect("spec's folder"));
        path.push(":");
        path.push(common::product_path());
        let env: Vec<(OsString, OsString)> = vec![
            ("PATH".into(), path),
            ("HOME".into(), ops.home.clone().into()),
            ("LANG".into(), "en_US.UTF-8".into()),
            ("LC_ALL".into(), "en_US.UTF-8".into()),
            ("HUB_OPS_ROOT".into(), ops.root.clone().into()),
            ("GIT_CONFIG_NOSYSTEM".into(), "1".into()),
            ("GIT_CONFIG_GLOBAL".into(), "/dev/null".into()),
        ];
        let label = format!("{shell} hub-ops queue once, the built spec");
        let ran = watched(
            &label,
            &mut ops.command(&["queue", "once"], &env),
            &mut |_: Option<u32>| {},
        );
        ran.code(0).prefixed("queue", &stamp(e));
        let generation = ops.path(&format!("dumps/queue/{}", stamp(e)));
        for name in ["a", "b"] {
            let dump = read(&generation.join(format!("{name}.jsonl")));
            let header: Value =
                serde_json::from_str(dump.lines().next().expect("a header")).expect("JSON");
            // Format 5 since docs/features/card-questions.md S5; 6 since
            // docs/features/task-mandate.md M5; 7 since
            // docs/features/plain-questions.md PQ8.
            assert_eq!(header["format"], 7, "{shell}: {name}: {dump}");
            assert_eq!(header["proposals"], 1, "{shell}: {name}: {dump}");
            if name == "a" {
                assert_eq!(header["project"], "lantern-keep", "{shell}: {dump}");
            }
            assert_eq!(
                dump.lines()
                    .filter(|line| line.contains("\"proposals\":{"))
                    .count(),
                1,
                "{shell}: {name}: one question row: {dump}"
            );
        }
        assert_eq!(common::snapshot(&a), before_a, "{shell}: R2");
        assert_eq!(common::snapshot(&b), before_b, "{shell}: R2");
    }
}

// ------------------------------------------------------------- OP16-OP19: upload

/// docs/features/hub-ops.md "Acceptance criteria" OP16, and "An `upload`
/// round": given fresh complete `queue/<t1>` and `door/<t2>` (an older
/// generation and a newer staging beside them), `upload once` copies the
/// newest of each to `/snapshot/<part>`, runs `restic backup --host <h> --tag
/// <part> /snapshot/<part>` for `queue` then `door`, each seeing the copy,
/// with each `restic_env` key and `RESTIC_PASSWORD_FILE` in its environment,
/// touches `/state/ok`, and runs no `init`, `forget` or `prune`. A second
/// round replaces each copy whole. M: back up `/dumps` itself; skip the copy.
#[test]
fn op16_an_upload_round_sends_the_newest_generation_of_each_part() {
    for shell in shells() {
        let ops = Ops::new("op16", &shell);
        let now = ops.clock();
        ops.generation("queue", now - 1200, &[("old.jsonl", "{\"old\":1}\n")]);
        let (queue, door) = ops.fresh_parts();
        let staging = ops.path(&format!("dumps/queue/.{}.99.tmp", stamp(now - 60)));
        fs::create_dir_all(&staging).expect("a staging");
        fs::write(staging.join("partial.jsonl"), "{").expect("a partial dump");

        let ran = ops.run(&["upload", "once"]);
        ran.code(0)
            .prefixed("upload", &stamp(now))
            .holds_none(&SECRETS);
        assert_eq!(
            common::snapshot(&ops.path("snapshot/queue")),
            common::snapshot(&queue),
            "{shell}: /snapshot/queue holds the newest queue generation"
        );
        assert_eq!(
            common::snapshot(&ops.path("snapshot/door")),
            common::snapshot(&door),
            "{shell}: /snapshot/door holds the door's"
        );
        let calls = ops.restic_calls();
        assert_eq!(
            calls
                .iter()
                .map(|call| call.args.clone())
                .collect::<Vec<_>>(),
            [
                backup_call(&ops.root, "queue"),
                backup_call(&ops.root, "door")
            ],
            "{shell}: two backups, no init, forget or prune"
        );
        for (call, generation) in calls.iter().zip([&queue, &door]) {
            let saw = call.saw.as_ref().expect("the folder at the call");
            assert_eq!(
                common::snapshot(saw),
                common::snapshot(generation),
                "{shell}: restic saw the copy"
            );
            for (name, value) in [
                ("RESTIC_REPOSITORY", "s3:https://s3.example.org/hub-dumps"),
                ("AWS_ACCESS_KEY_ID", "key-id-4f1e"),
                ("AWS_SECRET_ACCESS_KEY", "secret-key-9c2d"),
            ] {
                assert_eq!(
                    call.env.get(name).map(String::as_str),
                    Some(value),
                    "{shell}: {name}"
                );
            }
            assert_eq!(
                call.env.get("RESTIC_PASSWORD_FILE").map(PathBuf::from),
                Some(ops.path("run/secrets/restic_password")),
                "{shell}: restic reads the password from its file"
            );
            assert!(!call.env.contains_key("RESTIC_PASSWORD"), "{shell}: R5");
        }
        let ok = ops.path("state/ok");
        assert!(ok.is_file(), "{shell}: /state/ok touched");
        assert!(seconds_old(&ok) < 60, "{shell}: /state/ok is new");
        // Iteration 2, `cp -Rp`: the copies keep the dump's times, so a
        // restore gives them back.
        for part in ["queue", "door"] {
            let copy = ops.path(&format!(
                "snapshot/{part}/{}",
                if part == "queue" {
                    "a.jsonl"
                } else {
                    "users.yml"
                }
            ));
            assert!(
                (250..400).contains(&seconds_old(&copy)),
                "{shell}: {} keeps its five-minute-old time: {} s",
                copy.display(),
                seconds_old(&copy)
            );
        }

        ops.set_clock(now + 900);
        let newer = ops.generation(
            "queue",
            now + 600,
            &[("a.jsonl", "{\"queue\":\"newer\"}\n")],
        );
        ops.run(&["upload", "once"]).code(0);
        assert_eq!(
            common::snapshot(&ops.path("snapshot/queue")),
            common::snapshot(&newer),
            "{shell}: a second round's copy replaces the first whole"
        );
    }
}

/// docs/features/hub-ops.md "Acceptance criteria" OP17: a round that is not
/// whole leaves `/state/ok` untouched and exits 1, naming why: a door
/// generation 40 minutes old (the queue still sent); a `CARRIED` file;
/// `restic` exiting 10 (the `init` line and no `init`) or 12; no generation
/// at all (no backup). `fresh` is 0 for an `ok` 10 minutes old, 1 for 40 or
/// none. M: `ok` on a stale part; `fresh` always 0; 350 minutes for 35.
#[test]
fn op17_a_round_that_is_not_whole_leaves_ok_alone_and_says_why() {
    for shell in shells() {
        let case = |label: &str, setup: &dyn Fn(&Ops)| -> (Ops, Ran) {
            let ops = Ops::new(label, &shell);
            ops.put("state/ok", "");
            age(&ops.path("state/ok"), 1200);
            setup(&ops);
            let ran = ops.run(&["upload", "once"]);
            ran.code(1).holds_none(&SECRETS);
            assert!(
                (1190..1300).contains(&seconds_old(&ops.path("state/ok"))),
                "{shell} {label}: /state/ok untouched: {}",
                ran.show()
            );
            (ops, ran)
        };

        let (ops, ran) = case("op17-stale", &|ops: &Ops| {
            let now = ops.clock();
            ops.generation("queue", now - 300, &[("a.jsonl", "{}\n")]);
            ops.generation("door", now - 2400, &[("users.yml", "users:\n")]);
        });
        ran.names(&["door"]);
        assert_eq!(
            ops.restic_args(),
            [
                backup_call(&ops.root, "queue"),
                backup_call(&ops.root, "door")
            ],
            "{shell}: the queue is still sent, and the stale door too (the orchestrator's decision)"
        );

        let (ops, ran) = case("op17-carried", &|ops: &Ops| {
            let now = ops.clock();
            ops.generation(
                "queue",
                now - 300,
                &[("a.jsonl", "{}\n"), ("CARRIED", "a-1\n")],
            );
            ops.generation("door", now - 300, &[("users.yml", "users:\n")]);
        });
        assert!(
            ran.stderr.lines().any(|line| line.starts_with("hub-ops: ")
                && line.to_ascii_lowercase().contains("carried")),
            "{shell}: the round names CARRIED: {}",
            ran.show()
        );
        assert_eq!(
            ops.restic_args(),
            [
                backup_call(&ops.root, "queue"),
                backup_call(&ops.root, "door")
            ],
            "{shell}: a CARRIED generation is still sent (R12)"
        );

        for (code, line) in [
            (
                10,
                "the repository is not initialised: run docker compose run --rm backup init",
            ),
            (12, "the repository password is wrong"),
        ] {
            let (ops, ran) = case(&format!("op17-exit-{code}"), &|ops: &Ops| {
                ops.fresh_parts();
                ops.control("restic-exit", &format!("backup {code}\n"));
            });
            ran.names(&[line]);
            assert!(
                ops.restic_args().iter().all(|args| args[0] == "backup"),
                "{shell}: exit {code}: never an init: {:?}",
                ops.restic_args()
            );
        }

        let (ops, _) = case("op17-none", &|_: &Ops| {});
        assert!(
            ops.restic_args().is_empty(),
            "{shell}: no generation, no backup"
        );

        // Iteration 2: `fresh` is 0 only with an `ok` younger than 35
        // minutes and a generation younger than that in each part.
        let ops = Ops::new("op17-fresh", &shell);
        let fresh = ops.run(&["fresh"]);
        fresh.code(1);
        assert!(
            fresh
                .stderr
                .lines()
                .any(|line| line.starts_with("hub-ops: ")),
            "{shell}: a just-started hub, before its first generation: {}",
            fresh.show()
        );
        let now = ops.clock();
        ops.generation("queue", now - 300, &[("a.jsonl", "{}\n")]);
        ops.generation("door", now - 300, &[("users.yml", "users:\n")]);
        ops.run(&["fresh"]).code(1);
        ops.put("state/ok", "");
        for (back, code) in [(600, 0), (2400, 1)] {
            age(&ops.path("state/ok"), back);
            let ran = ops.run(&["fresh"]);
            assert_eq!(
                ran.code,
                code,
                "{shell}: fresh, ok {back} s old: {}",
                ran.show()
            );
        }
        assert!(
            ops.restic_args().is_empty(),
            "{shell}: fresh calls no restic"
        );
        for part in ["queue", "door"] {
            let ops = Ops::new("op17-fresh-part", &shell);
            let now = ops.clock();
            let other = if part == "queue" { "door" } else { "queue" };
            ops.generation(other, now - 300, &[("a.jsonl", "{}\n")]);
            ops.put("state/ok", "");
            age(&ops.path("state/ok"), 600);
            let ran = ops.run(&["fresh"]);
            assert_eq!(ran.code, 1, "{shell}: no {part} generation: {}", ran.show());
            ran.names(&[part]);
            ops.generation(part, now - 2400, &[("a.jsonl", "{}\n")]);
            let ran = ops.run(&["fresh"]);
            assert_eq!(
                ran.code,
                1,
                "{shell}: a stale {part} generation: {}",
                ran.show()
            );
            ran.names(&[part]);
            ops.generation(part, now - 120, &[("a.jsonl", "{}\n")]);
            ops.run(&["fresh"]).code(0);
        }
    }
}

/// docs/features/hub-ops.md "Acceptance criteria" OP18: a loop round at the
/// 03:05 UTC slot runs, after both backups, `forget --host <h> --tag queue
/// --keep-within 48h --keep-daily 30`, the same for `door`, then `prune`;
/// the 03:20 slot runs none of them, and neither does `once` at 03:05. The
/// stub `sleep` advances the clock, as a real one does. M: `--prune` in
/// `forget`; `--keep-daily 300`; a prune every round.
#[test]
fn op18_the_0305_round_forgets_per_tag_then_prunes() {
    for shell in shells() {
        let backups = |ops: &Ops| {
            vec![
                backup_call(&ops.root, "queue"),
                backup_call(&ops.root, "door"),
            ]
        };

        let ops = Ops::new("op18", &shell);
        ops.set_clock(DAY + 11_000);
        ops.fresh_parts();
        ops.control("sleep-advance", "");
        ops.control("sleep-term-at", "3");
        let ran = ops.run(&["upload"]);
        ran.code(143);
        assert_eq!(ops.sleeps(), ["60", "40", "900"], "{shell}: {}", ran.show());
        let mut want = backups(&ops);
        want.extend(backups(&ops));
        want.push(forget_call("queue"));
        want.push(forget_call("door"));
        want.push(words(&["prune"]));
        assert_eq!(ops.restic_args(), want, "{shell}: the 03:05 round");

        let ops = Ops::new("op18-0320", &shell);
        ops.set_clock(DAY + 11_900);
        ops.fresh_parts();
        ops.control("sleep-advance", "");
        ops.control("sleep-term-at", "3");
        let ran = ops.run(&["upload"]);
        ran.code(143);
        assert_eq!(ops.sleeps(), ["60", "40", "900"], "{shell}: {}", ran.show());
        let mut want = backups(&ops);
        want.extend(backups(&ops));
        assert_eq!(
            ops.restic_args(),
            want,
            "{shell}: the 03:20 round prunes nothing"
        );

        let ops = Ops::new("op18-once", &shell);
        ops.set_clock(DAY + 11_100);
        ops.fresh_parts();
        ops.run(&["upload", "once"]).code(0);
        assert_eq!(
            ops.restic_args(),
            backups(&ops),
            "{shell}: once never prunes"
        );
    }
}

/// The values of OP19's refused lines: none may be printed.
const REFUSED_VALUES: [&str; 10] = [
    "x-value-path",
    "x-value-ld",
    "x-value-password",
    "x-value-command",
    "y-value-export",
    "y-value-lower",
    "x-value-cache",
    "s3:https://s3.example.org/second",
    "x-value-first",
    "x-value-dup",
];

/// docs/features/hub-ops.md "Acceptance criteria" OP19, and "Secrets": a
/// `restic_env` line that is not `RESTIC_REPOSITORY`, `RESTIC_REST_*` or a
/// backend's prefix, `[A-Z0-9_]`, refuses the run with exit 2 naming its line
/// number and key, never its value, before `restic` or any copy; so do no
/// `RESTIC_REPOSITORY` and two. Comments and empty lines are skipped; a value
/// holding `$HOME`, spaces, quotes, a backslash and `=` reaches `restic`
/// verbatim, the backends' prefixes too. M: the file sourced with `.`; the
/// value printed; `RESTIC_PASSWORD` accepted.
#[test]
fn op19_restic_env_is_read_as_data_and_refused_line_by_line() {
    for shell in shells() {
        let head = "# the hub's dumps\n\nRESTIC_REPOSITORY=s3:https://s3.example.org/hub-dumps\n";
        // (the line, its number, the key named or None, what of the line
        // may never show). Iteration 2: a key is named only when it is an
        // upper-case shell name of at most 64 bytes; any other refused line
        // is `restic_env:<n>: not KEY=value`, nothing of it said.
        let refused: [(&str, &str, Option<&str>, &[&str]); 15] = [
            ("PATH=/x-value-path\n", "4", Some("PATH"), &[]),
            ("LD_PRELOAD=x-value-ld\n", "4", Some("LD_PRELOAD"), &[]),
            (
                "RESTIC_PASSWORD=x-value-password\n",
                "4",
                Some("RESTIC_PASSWORD"),
                &[],
            ),
            (
                "RESTIC_PASSWORD_COMMAND=x-value-command\n",
                "4",
                Some("RESTIC_PASSWORD_COMMAND"),
                &[],
            ),
            (
                "export AWS_X=y-value-export\n",
                "4",
                None,
                &["export", "AWS_X"],
            ),
            ("aws_x=y-value-lower\n", "4", None, &["aws_x"]),
            (
                "RESTIC_CACHE_DIR=x-value-cache\n",
                "4",
                Some("RESTIC_CACHE_DIR"),
                &[],
            ),
            (
                "no-equals-sign-here\n",
                "4",
                None,
                &["no-equals-sign-here", "equals"],
            ),
            (
                "RESTIC_REPOSITORY=s3:https://s3.example.org/second\n",
                "4",
                Some("RESTIC_REPOSITORY"),
                &[],
            ),
            (
                "AWS_ACCESS_KEY_ID=x-value-first\nAWS_ACCESS_KEY_ID=x-value-dup\n",
                "5",
                Some("AWS_ACCESS_KEY_ID"),
                &[],
            ),
            (
                "AWS_SECRET_ACCESS_KEY=x-value-cr\r\n",
                "4",
                Some("AWS_SECRET_ACCESS_KEY"),
                &["x-value-cr"],
            ),
            (
                "AWS_SECRET_ACCESS_KEY: wJalrXUtnFEMI/K7MDENG=bPxRfiCYEXAMPLEKEY\n",
                "4",
                None,
                &["AWS_SECRET_ACCESS_KEY", "wJalr", "K7MDENG", "bPxRfi"],
            ),
            (
                "dGhpcyBpcyBhIHNlY3JldA==\n",
                "4",
                None,
                &["dGhpcy", "Y3JldA"],
            ),
            (
                "RESTIC_PASSWORD: hunter2=x\n",
                "4",
                None,
                &["RESTIC_PASSWORD", "hunter2"],
            ),
            (
                "ZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZ=v\n",
                "4",
                None,
                &["ZZZZZZZZZZ"],
            ),
        ];
        let number = |line: &str, n: &str| {
            Regex::new(&format!(r"(^|[^0-9]){n}([^0-9]|$)"))
                .expect("a pattern")
                .is_match(line)
        };
        for (bad, n, key, hidden) in refused {
            let ops = Ops::new("op19", &shell);
            ops.fresh_parts();
            ops.put("run/secrets/restic_env", &format!("{head}{bad}"));
            let ran = ops.run(&["upload", "once"]);
            ran.code(2)
                .holds_none(&SECRETS)
                .holds_none(&REFUSED_VALUES)
                .holds_none(hidden);
            match key {
                Some(key) => assert!(
                    ran.stderr.lines().any(|line| line.starts_with("hub-ops: ")
                        && number(line, n)
                        && line.contains(key)),
                    "{shell}: {bad:?}: a line naming line {n} and {key}: {}",
                    ran.show()
                ),
                None => assert!(
                    ran.stderr.lines().any(|line| line.starts_with("hub-ops: ")
                        && line.ends_with(&format!(": restic_env:{n}: not KEY=value"))),
                    "{shell}: {bad:?}: `restic_env:{n}: not KEY=value`, nothing of the line: {}",
                    ran.show()
                ),
            }
            assert!(
                ops.restic_args().is_empty(),
                "{shell}: {bad:?}: restic called"
            );
            assert!(
                common::snapshot(&ops.path("snapshot")).is_empty(),
                "{shell}: {bad:?}: nothing copied before the refusal"
            );
            assert!(!ops.path("state/ok").exists());
        }

        let ops = Ops::new("op19-missing", &shell);
        ops.fresh_parts();
        ops.put("run/secrets/restic_env", "AWS_ACCESS_KEY_ID=key-id-4f1e\n");
        let acts: [&[&str]; 3] = [&["upload", "once"], &["init"], &["snapshots"]];
        for args in acts {
            let ran = ops.run(args);
            ran.code(2)
                .names(&["RESTIC_REPOSITORY"])
                .holds_none(&SECRETS);
        }
        assert!(
            ops.restic_args().is_empty(),
            "{shell}: no RESTIC_REPOSITORY"
        );

        let ops = Ops::new("op19-verbatim", &shell);
        ops.fresh_parts();
        let odd = r#"a $HOME b 'c' "d" e\f=g *"#;
        ops.put(
            "run/secrets/restic_env",
            &format!(
                "# a comment\n\n# AWS_IGNORED=x\nRESTIC_REPOSITORY=rest:https://backup.example.org/hub?x=1\n\
                 RESTIC_REST_USERNAME=hub\nRESTIC_REST_PASSWORD=rest-pass-31\n\
                 AWS_SECRET_ACCESS_KEY={odd}\nB2_ACCOUNT_ID=b2-id\nAZURE_ACCOUNT_NAME=az\n\
                 GOOGLE_PROJECT_ID=gp\nOS_USERNAME=os\nST_AUTH=st\n"
            ),
        );
        let ran = ops.run(&["upload", "once"]);
        ran.code(0).holds_none(&[odd, "rest-pass-31"]);
        let calls = ops.restic_calls();
        assert_eq!(calls.len(), 2, "{shell}: {}", ran.show());
        for (name, value) in [
            (
                "RESTIC_REPOSITORY",
                "rest:https://backup.example.org/hub?x=1",
            ),
            ("RESTIC_REST_USERNAME", "hub"),
            ("RESTIC_REST_PASSWORD", "rest-pass-31"),
            ("AWS_SECRET_ACCESS_KEY", odd),
            ("B2_ACCOUNT_ID", "b2-id"),
            ("AZURE_ACCOUNT_NAME", "az"),
            ("GOOGLE_PROJECT_ID", "gp"),
            ("OS_USERNAME", "os"),
            ("ST_AUTH", "st"),
        ] {
            assert_eq!(
                calls[0].env.get(name).map(String::as_str),
                Some(value),
                "{shell}: {name} verbatim"
            );
        }
        assert!(
            !calls[0].env.contains_key("AWS_IGNORED"),
            "{shell}: a comment is skipped"
        );
    }
}

/// Beside docs/features/hub-ops.md "Acceptance criteria" OP8, the
/// orchestrator's decision on an edge "A `queue` round" leaves open: with no
/// served project a round makes no generation and exits 1, naming it.
#[test]
fn data_a_queue_round_with_no_project_makes_no_generation() {
    for shell in shells() {
        let ops = Ops::new("no-project", &shell);
        ops.put("projects/b/README.md", "no config\n");
        let ran = ops.run(&["queue", "once"]);
        ran.code(1);
        assert!(
            ran.stderr
                .lines()
                .any(|line| line.starts_with("hub-ops: queue ")),
            "{shell}: named on stderr: {}",
            ran.show()
        );
        assert!(
            ops.entries("queue").is_empty(),
            "{shell}: {:?}",
            ops.entries("queue")
        );
        assert!(ops.spec_calls().is_empty(), "{shell}: nothing to export");
    }
}

/// Beside docs/features/hub-ops.md "Acceptance criteria" OP16, the
/// orchestrator's decision: an empty door generation, should one land, is
/// complete, so a fresh one is sent and the round is whole (`hub-admin dump`
/// itself refuses a door with no users file since iteration 2).
#[test]
fn data_an_empty_door_generation_is_sent() {
    for shell in shells() {
        let ops = Ops::new("empty-door", &shell);
        let now = ops.clock();
        ops.generation("queue", now - 300, &[("a.jsonl", "{}\n")]);
        ops.generation("door", now - 300, &[]);
        let ran = ops.run(&["upload", "once"]);
        ran.code(0);
        assert_eq!(
            ops.restic_args(),
            [
                backup_call(&ops.root, "queue"),
                backup_call(&ops.root, "door")
            ],
            "{shell}: {}",
            ran.show()
        );
        assert!(
            ops.path("snapshot/door").is_dir(),
            "{shell}: the empty copy"
        );
        assert!(common::snapshot(&ops.path("snapshot/door")).is_empty());
        assert!(ops.path("state/ok").is_file(), "{shell}: whole");
    }
}

/// docs/features/hub-ops.md "Rules and edge cases" R3, iteration 2's claim:
/// a round whose name `.<t>.claim.tmp` another round holds is dropped, exit
/// 1, no staging and no generation, the rival's claim left; a claim a
/// SIGKILL left is a `.*.tmp`, pruned once 60 minutes old, a young one kept.
#[test]
fn r03_the_claim_drops_a_rival_round_and_a_dead_claim_is_pruned() {
    for shell in shells() {
        let ops = Ops::new("claim", &shell);
        ops.project("a");
        let e = DAY + 13 * 3600;
        ops.set_clock(e);
        let rival = format!(".{}.claim.tmp", stamp(e));
        fs::create_dir_all(ops.path(&format!("dumps/queue/{rival}"))).expect("a rival's claim");
        let ran = ops.run(&["queue", "once"]);
        ran.code(1).names(&["claimed"]);
        assert_eq!(
            ops.entries("queue"),
            std::slice::from_ref(&rival),
            "{shell}: {}",
            ran.show()
        );

        fs::remove_dir(ops.path(&format!("dumps/queue/{rival}"))).expect("the rival ends");
        let next = e + 900;
        ops.set_clock(next);
        let mut kept = Vec::new();
        for (back, name) in [(7200, "dead"), (600, "young")] {
            let claim = format!(".{}.claim.tmp", stamp(next - back));
            let path = ops.path(&format!("dumps/queue/{claim}"));
            fs::create_dir_all(&path).expect("a claim");
            age(&path, back);
            if name == "young" {
                kept.push(claim);
            }
        }
        ops.run(&["queue", "once"]).code(0);
        kept.push(stamp(next));
        assert_eq!(
            ops.entries("queue"),
            kept,
            "{shell}: the dead claim pruned, the young kept"
        );
    }
}

/// docs/features/hub-ops.md "Rules and edge cases" R5, iteration 2's
/// redaction: what restic says is logged with every `restic_env` value
/// covered, the longest first, overlapping and nested ones too
/// (`RESTIC_REST_USERNAME=hub` inside `rest:https://hub:s3cr3t@...`), so no
/// value shows, not even in part, on a round restic answers 0 or 1.
#[test]
fn r05_restics_lines_show_no_value_even_in_part() {
    let env = "RESTIC_REPOSITORY=rest:https://hub:s3cr3t@backup.example.org/repo
RESTIC_REST_USERNAME=hub
RESTIC_REST_PASSWORD=s3cr3t
AWS_ACCESS_KEY_ID=abcdef
AWS_SECRET_ACCESS_KEY=defghi
";
    let said = "Fatal: unable to open rest:https://hub:s3cr3t@backup.example.org/repo as hub with s3cr3t; keys abcdefghi\n";
    let parts = [
        "hub",
        "s3cr3t",
        "3cr3",
        "rest:",
        "backup.example",
        "/repo",
        "abc",
        "def",
        "ghi",
        "cdefg",
        "@",
    ];
    for shell in shells() {
        for code in ["0", "1"] {
            let ops = Ops::new("redact", &shell);
            ops.fresh_parts();
            ops.put("run/secrets/restic_env", env);
            ops.control("restic-said", said);
            ops.control("restic-exit", &format!("backup {code}\n"));
            let ran = ops.run(&["upload", "once"]);
            ran.holds_none(&["s3cr3t", "abcdef", "defghi", "3cr3", "backup.example.org"]);
            let relayed: Vec<&str> = ran
                .stderr
                .lines()
                .filter(|line| line.contains("Fatal: unable to open"))
                .collect();
            assert_eq!(
                relayed.len(),
                2,
                "{shell} exit {code}: each backup's line: {}",
                ran.show()
            );
            for line in relayed {
                let (_, told) = line
                    .split_once("Fatal: unable to open")
                    .expect("the relayed line");
                for part in parts {
                    assert!(
                        !told.contains(part),
                        "{shell} exit {code}: {part:?} shows in {line:?}"
                    );
                }
                assert!(told.contains("<RESTIC_REPOSITORY>"), "{shell}: {line:?}");
            }
        }
    }
}

/// docs/features/hub-ops.md "Rules and edge cases" R5, iteration 2's
/// redaction, longest value first (the orchestrator's decision): a covered
/// run is one `<KEY>`, named by the longest value covering its first byte, so
/// `abcdefg` is `<AWS_SECRET_ACCESS_KEY>` though `AWS_ACCESS_KEY_ID=abc`
/// comes first in the file. The cover itself leaves no part of any value
/// whatever the order (`r05_restics_lines_show_no_value_even_in_part`); this
/// is what the order decides.
#[test]
fn r05_a_covered_run_is_named_by_its_longest_value() {
    let env = "RESTIC_REPOSITORY=s3:https://s3.example.org/hub-dumps
AWS_ACCESS_KEY_ID=abc
AWS_SECRET_ACCESS_KEY=abcdefg
";
    for shell in shells() {
        let ops = Ops::new("redact-name", &shell);
        ops.fresh_parts();
        ops.put("run/secrets/restic_env", env);
        ops.control("restic-said", "Fatal: keys abcdefg refused\n");
        ops.control("restic-exit", "backup 1\n");
        let ran = ops.run(&["upload", "once"]);
        ran.code(1).holds_none(&["abc", "defg"]);
        let relayed: Vec<&str> = ran
            .stderr
            .lines()
            .filter(|line| line.contains("Fatal: keys "))
            .collect();
        assert_eq!(relayed.len(), 2, "{shell}: {}", ran.show());
        for line in relayed {
            assert!(
                line.ends_with("Fatal: keys <AWS_SECRET_ACCESS_KEY> refused"),
                "{shell}: the longest value names the run: {line:?}"
            );
        }
    }
}

// ------------------------------------------------------------- OP20: the acts

/// docs/features/hub-ops.md "Acceptance criteria" OP20, and "`hub-ops`": `init`
/// runs `restic init` once; `snapshots` `restic snapshots --host <h>`; `check`
/// `restic check`; `restore DIR` restores `latest:/snapshot/<part>` with the
/// host and tag filter into `DIR/queue` then `DIR/door`, and `restore DIR Q
/// D` the two IDs with no filter. Exit 2, nothing run, for a missing or
/// non-empty `DIR`, an unknown mode, an extra argument, one ID alone, and an
/// absent `HUB_HOST`. The path inside a snapshot is `backup`'s:
/// `/snapshot/<part>`, or under `HUB_OPS_ROOT` as the test's backups give it.
/// M: `restore latest` with no subfolder; a non-empty `DIR` accepted.
#[test]
fn op20_backups_acts_run_restic_as_written() {
    for shell in shells() {
        for (act, want) in [
            ("init", words(&["init"])),
            ("snapshots", words(&["snapshots", "--host", HOST])),
            ("check", words(&["check"])),
        ] {
            let ops = Ops::new("op20", &shell);
            ops.run(&[act]).code(0).holds_none(&SECRETS);
            assert_eq!(ops.restic_args(), [want], "{shell}: {act}");
        }

        let ops = Ops::new("op20-restore", &shell);
        let target = ops.scratch.dir("restore");
        let dir = text_of(&target).to_owned();
        ops.run(&["restore", &dir]).code(0);
        let calls = ops.restic_args();
        assert_eq!(calls.len(), 2, "{shell}: {calls:?}");
        for (call, part) in calls.iter().zip(["queue", "door"]) {
            let inside = call
                .get(1)
                .and_then(|arg| arg.strip_prefix("latest:"))
                .unwrap_or_else(|| panic!("{shell}: restore latest:<path>: {call:?}"));
            assert!(
                inside == format!("/snapshot/{part}")
                    || inside == format!("{}/snapshot/{part}", ops.root.display()),
                "{shell}: the snapshot's subfolder {part}: {call:?}"
            );
            assert_eq!(
                call,
                &words(&[
                    "restore",
                    &format!("latest:{inside}"),
                    "--host",
                    HOST,
                    "--tag",
                    part,
                    "--target",
                    &format!("{dir}/{part}"),
                ]),
                "{shell}: restore latest of {part}"
            );
        }

        let ops = Ops::new("op20-ids", &shell);
        let target = ops.scratch.dir("restore");
        let dir = text_of(&target).to_owned();
        ops.run(&["restore", &dir, "4f1e2d3c", "9a8b7c6d"]).code(0);
        let calls = ops.restic_args();
        assert_eq!(calls.len(), 2, "{shell}: {calls:?}");
        for (call, (part, id)) in calls
            .iter()
            .zip([("queue", "4f1e2d3c"), ("door", "9a8b7c6d")])
        {
            let inside = call
                .get(1)
                .and_then(|arg| arg.strip_prefix(&format!("{id}:")))
                .unwrap_or_else(|| panic!("{shell}: restore {id}:<path>: {call:?}"));
            assert!(
                inside == format!("/snapshot/{part}")
                    || inside == format!("{}/snapshot/{part}", ops.root.display()),
                "{shell}: {call:?}"
            );
            assert_eq!(
                call,
                &words(&[
                    "restore",
                    &format!("{id}:{inside}"),
                    "--target",
                    &format!("{dir}/{part}"),
                ]),
                "{shell}: restore {id}, no filter"
            );
        }

        let ops = Ops::new("op20-bad", &shell);
        let full = ops.scratch.dir("full");
        fs::write(full.join("left"), "x").expect("a file");
        let empty = ops.scratch.dir("empty");
        let missing = ops.scratch.path().join("missing");
        let (full, empty, missing) = (
            text_of(&full).to_owned(),
            text_of(&empty).to_owned(),
            text_of(&missing).to_owned(),
        );
        let before = common::snapshot(&ops.root);
        let bad: Vec<Vec<&str>> = vec![
            vec![],
            vec!["frobnicate"],
            vec!["restore"],
            vec!["restore", &missing],
            vec!["restore", &full],
            vec!["restore", &empty, "4f1e2d3c"],
            vec!["restore", &empty, "4f1e2d3c", "9a8b7c6d", "x"],
            vec!["init", "x"],
            vec!["snapshots", "x"],
            vec!["check", "x"],
            vec!["fresh", "x"],
            vec!["upload", "once", "x"],
            vec!["upload", "twice"],
            vec!["queue", "once", "x"],
        ];
        for args in &bad {
            let ran = ops.run(args);
            assert_eq!(ran.code, 2, "{shell}: {args:?}: {}", ran.show());
            assert!(
                ran.stderr.lines().any(|line| line.starts_with("hub-ops: ")),
                "{shell}: {args:?}: a `hub-ops: ` line: {}",
                ran.show()
            );
        }
        assert_eq!(
            common::snapshot(&ops.root),
            before,
            "{shell}: a bad call writes nothing"
        );
        assert!(
            common::snapshot(Path::new(&empty)).is_empty(),
            "{shell}: the empty DIR stays empty"
        );
        ops.fresh_parts();
        let parts = common::snapshot(&ops.root);
        let ran = ops.run_with(&["upload", "once"], false, &mut |_: Option<u32>| {});
        assert_eq!(ran.code, 2, "{shell}: no HUB_HOST: {}", ran.show());
        let ran = ops.run_with(&["snapshots"], false, &mut |_: Option<u32>| {});
        assert_eq!(ran.code, 2, "{shell}: no HUB_HOST: {}", ran.show());
        assert_eq!(
            common::snapshot(&ops.root),
            parts,
            "{shell}: nothing written"
        );
        assert!(ops.restic_args().is_empty(), "{shell}: restic never called");
        assert!(ops.spec_calls().is_empty(), "{shell}: spec never called");

        // The orchestrator's decision: `HUB_HOST` is needed by `upload`,
        // `snapshots` and a `latest` restore alone.
        let ops = Ops::new("op20-no-host", &shell);
        for act in ["init", "check"] {
            let ran = ops.run_with(&[act], false, &mut |_: Option<u32>| {});
            assert_eq!(
                ran.code,
                0,
                "{shell}: {act} without HUB_HOST: {}",
                ran.show()
            );
        }
        let ran = ops.run_with(&["fresh"], false, &mut |_: Option<u32>| {});
        assert_eq!(
            ran.code,
            1,
            "{shell}: fresh without HUB_HOST, no ok: {}",
            ran.show()
        );
        let latest = ops.scratch.dir("latest");
        let ran = ops.run_with(&["restore", text_of(&latest)], false, &mut |_: Option<
            u32,
        >| {});
        assert_eq!(
            ran.code,
            2,
            "{shell}: restore latest needs HUB_HOST: {}",
            ran.show()
        );
        let ids = ops.scratch.dir("ids");
        let ran = ops.run_with(
            &["restore", text_of(&ids), "4f1e2d3c", "9a8b7c6d"],
            false,
            &mut |_: Option<u32>| {},
        );
        assert_eq!(
            ran.code,
            0,
            "{shell}: restore by IDs needs no HUB_HOST: {}",
            ran.show()
        );
        assert_eq!(
            ops.restic_args()
                .iter()
                .map(|call| call[0].clone())
                .collect::<Vec<_>>(),
            ["init", "check", "restore", "restore"],
            "{shell}"
        );
    }
}

// ------------------------------------------------------------- OP21: the loops

/// docs/features/hub-ops.md "Acceptance criteria" OP21, and "The grid": with
/// the clock fixed at E, `queue` runs a round at start, then asks for `900 -
/// E % 900`; `upload` asks for 60, runs a round, then asks for `900 - (E -
/// 300) % 900`; TERM during an export (a `spec` that waits) ends the run at
/// once with 143 and no `.tmp`. M: no round at start; a fixed `sleep 900`;
/// no trap.
#[test]
fn op21_the_loops_run_on_the_grid_and_end_on_term() {
    let e = DAY + 12 * 3600 + 7 * 60 + 30;
    for shell in shells() {
        let ops = Ops::new("op21-queue", &shell);
        ops.project("a");
        ops.set_clock(e);
        ops.control("sleep-term-at", "1");
        let ran = ops.run(&["queue"]);
        ran.code(143);
        assert_eq!(
            ops.sleeps(),
            [(900 - e % 900).to_string()],
            "{shell}: {}",
            ran.show()
        );
        assert_eq!(ops.spec_calls().len(), 1, "{shell}: one round at start");
        assert_eq!(
            ops.entries("queue"),
            [stamp(e)],
            "{shell}: the round landed"
        );

        let ops = Ops::new("op21-upload", &shell);
        ops.set_clock(e);
        ops.fresh_parts();
        ops.control("sleep-term-at", "2");
        let ran = ops.run(&["upload"]);
        ran.code(143);
        assert_eq!(
            ops.sleeps(),
            ["60".to_owned(), (900 - (e - 300) % 900).to_string()],
            "{shell}: {}",
            ran.show()
        );
        assert_eq!(
            ops.restic_args(),
            [
                backup_call(&ops.root, "queue"),
                backup_call(&ops.root, "door")
            ],
            "{shell}: one round between the two"
        );

        let ops = Ops::new("op21-term", &shell);
        ops.project("a");
        ops.set_clock(e);
        ops.control("spec-hold", "a");
        let release = ops.rec.join("spec-release");
        let held = ops.rec.join("spec-held");
        let mut sent: Option<Instant> = None;
        let ran = ops.run_with(&["queue"], true, &mut |pid: Option<u32>| match pid {
            Some(pid) => {
                if sent.is_none() && held.exists() {
                    kill_term(pid);
                    sent = Some(Instant::now());
                }
            }
            None => fs::write(&release, "").expect("the stub's release"),
        });
        let sent = sent.unwrap_or_else(|| panic!("{shell}: the export never held: {}", ran.show()));
        ran.code(143);
        assert!(
            ran.ended - sent < Duration::from_secs(10),
            "{shell}: TERM ends the running export at once: {}",
            ran.show()
        );
        assert!(
            ops.entries("queue")
                .iter()
                .all(|name| !name.ends_with(".tmp")),
            "{shell}: no staging left: {:?}",
            ops.entries("queue")
        );
        assert!(ops.entries("queue").is_empty(), "{shell}: no generation");
    }
}

// ------------------------------------------------------------- OP22: the shells

/// docs/features/hub-ops.md "Acceptance criteria" OP22: the script tests run
/// under dash too (`/bin/dash`, or a `/bin/sh` that is dash), `hub-ops` is a
/// POSIX sh script each shell parses, and `health.sh` a bash one. M: a `[[`
/// or `${v:0:1}` in `hub-ops` (dash rejects both, at run time, in the tests
/// above).
#[test]
fn op22_hub_ops_is_posix_sh_and_the_tests_run_under_dash() {
    let shells = shells();
    assert!(
        shells.iter().any(|shell| fs::canonicalize(shell)
            .ok()
            .and_then(|real| real
                .file_name()
                .map(|name| name.to_string_lossy().into_owned()))
            .is_some_and(|name| name == "dash")),
        "no dash among {shells:?}: OP22 needs one"
    );
    assert_eq!(
        read(&hub_ops()).lines().next(),
        Some("#!/bin/sh"),
        "hub-ops is sh"
    );
    for shell in &shells {
        let parsed = Command::new(shell)
            .arg("-n")
            .arg(hub_ops())
            .output()
            .expect("sh -n");
        assert!(
            parsed.status.success(),
            "{shell} -n hub-ops: {}",
            String::from_utf8_lossy(&parsed.stderr)
        );
    }
    for bash in bashes() {
        let parsed = Command::new(&bash)
            .arg("-n")
            .arg(health_sh())
            .output()
            .expect("bash -n");
        assert!(
            parsed.status.success(),
            "{bash} -n health.sh: {}",
            String::from_utf8_lossy(&parsed.stderr)
        );
    }
}
