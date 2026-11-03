//! docs/features/hub-accounts.md (RM-0065, Phase 7 step 11c-1) AC1-AC14, AC16,
//! AC18 and AC19: the deployment's account script `scripts/deploy/admin/hub-admin`,
//! with no network, no image build and no Docker.
//!
//! The script runs under `sh` against a scratch `HUB_ADMIN_ROOT` (S7) laid out
//! as the `admin` service mounts it: `users/users.yml`, `mcp-users/users`,
//! `traefik-dynamic/` (a byte copy of the committed `hub.yml`, and the stamp),
//! `var/lib/hub-admin/` (mode 700: `acts.log`, `lock/`), `var/lib/authelia/`
//! (`notification.txt`) and `tmp/`, the container's own `/tmp` (R4). First on
//! `PATH` (D12's pattern):
//! - a stub `authelia` that keeps each call's arguments and stdin and answers in
//!   the shapes "The door's CLI" records (from memory until VF1): `Digest: <d>`
//!   for `crypto hash generate argon2|bcrypt`, a bare exit 0 for `storage user
//!   webauthn|totp delete`. A control file lists the calls that exit 1
//!   (`fail-at`; a delete then answers a recorded "nothing to delete" line),
//!   answer off their shape (`bad-at`) or wait for a release (`hold-at`);
//! - a `base64` that wraps at 76 columns, as busybox's in the pinned image
//!   does ("`mcp rotate`'s output"); macOS's does not wrap, so without it a
//!   missing newline removal would pass on the laptop;
//! - in one test each, an `mv` that signals the script right after renaming
//!   `users.yml` (AC4, R13) and an `od` that fails the byte scan (AC10, R9).
//!
//! A terminal is `script(1)`'s, as `specengine-cli`'s `tests/round.rs` gives
//! one: stdin is the pseudo-terminal, stdout and stderr go to files, so the two
//! stay apart; a `[y/N]` on the terminal or in either file is answered by the
//! next key. "A pipe" and `/dev/null` are plain stdin. Runs use a collating
//! locale (`en_US.UTF-8`), as `tests/deploy.rs` D12 does: the login grammar is
//! bytes, whatever the operator's locale.
//!
//! - AC1 add, AC2 the login and `--name` grammar, AC3 no terminal, AC4
//!   disable (a part-way one included), AC5 enable, AC6 reset, AC7 `mcp
//!   rotate`, AC8 the lock and temp files, AC9 a failing hash, AC10 the users
//!   and MCP files' layout, AC11 orphan MCP lines, AC12 `check` over the built
//!   `spec team`, AC19 a reset with a failing delete: the script.
//! - AC13 the compose service, AC14 the door's configuration, AC16 no account
//!   code in a crate, AC18 `image.yml` and the guide's size: the files as
//!   committed.
//! - Beside the criteria, from "Data": `mcp revoke`, `code`, the bad calls of
//!   "Exits" and `check`'s refusal of a line that is not three fields.
//!
//! AC15 is `specengine-cli`'s `tests/team_config.rs`; D7, D9 and D11 as decided
//! are `tests/deploy.rs`. AC17 is RM-0075's. VF1-VF3 and G1-G10 are by hand.

#![cfg(unix)]

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs;
use std::io::{Read as _, Write as _};
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use common::Scratch;
use regex::Regex;
use serde_json::{Map, Value};

/// The hub's host the `admin` service is given (`HUB_HOST`).
const HOST: &str = "hub.example.org";

/// A script run longer than this is killed and the test fails.
const RUN_TIMEOUT: Duration = Duration::from_secs(60);

/// Every refusal's stderr line opens with this ("Exits").
const REFUSAL: &str = "hub-admin: ";

/// The stub's bcrypt digests: `$2b$10$`, these 50 characters, the call's
/// number in three digits (53 characters in all, the MCP file's shape).
const BCRYPT_FILL: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWX";

/// The stub's answer, on stderr with exit 1, when a passkey delete fails:
/// "nothing to delete", from memory until VF1 records the door's (AC19).
const PASSKEYS_ANSWER: &str = "Error: the user has no WebAuthn credentials to delete";

/// The same for a TOTP delete.
const TOTP_ANSWER: &str = "Error: the user has no TOTP configuration to delete";

/// R10's line when a passkey delete failed.
const NO_ENABLE: &str = "hub-admin: passkeys not deleted: do not enable this account until reset ends without passkeys-left";

/// The stub of the door's CLI; `@DOOR@` is its records' folder, `@BCRYPT@`
/// [`BCRYPT_FILL`], `@PASSKEYS@` and `@TOTP@` the delete answers.
const DOOR_STUB: &str = r#"#!/bin/sh
# The door's CLI as docs/features/hub-accounts.md "The door's CLI" records it
# (from memory until VF1). Each call's arguments and stdin are kept under the
# records' folder; `fail-at`, `bad-at` and `hold-at` there list the calls that
# exit 1, answer off their shape, or wait for `release`.
door='@DOOR@'
listed() {
    case " $(cat "$door/$1" 2>/dev/null) " in
        *" $n "*) return 0 ;;
    esac
    return 1
}
n=1
if [ -f "$door/count" ]; then n=$(( $(cat "$door/count") + 1 )); fi
printf '%s\n' "$n" > "$door/count"
: > "$door/args.$n"
for argument in "$@"; do printf '%s\n' "$argument" >> "$door/args.$n"; done
if [ -t 0 ]; then
    : > "$door/tty.$n"
    : > "$door/stdin.$n"
else
    cat > "$door/stdin.$n"
fi
if listed hold-at; then
    : > "$door/held"
    waited=0
    while [ ! -e "$door/release" ] && [ "$waited" -lt 600 ]; do
        sleep 0.1
        waited=$((waited + 1))
    done
fi
fail=no
if listed fail-at; then fail=yes; fi
bad=no
if listed bad-at; then bad=yes; fi
case "$1 $2 $3 $4" in
    "crypto hash generate "*)
        if [ "$fail" = yes ]; then
            echo "Error: the stub fails call $n" >&2
            exit 1
        fi
        ;;
esac
case "$1 $2 $3 $4" in
    "crypto hash generate argon2")
        if [ "$bad" = yes ]; then
            echo 'Digest: $argon2id$v=19$m=65536,t=3,p=4$c2FsdA'
        else
            printf 'Digest: $argon2id$v=19$m=65536,t=3,p=4$c2FsdA%s$aGFzaA%s\n' "$n" "$n"
        fi
        ;;
    "crypto hash generate bcrypt")
        if [ "$bad" = yes ]; then
            echo 'Digest: $2b$10$short'
        else
            printf 'Digest: $2b$10$@BCRYPT@%03d\n' "$n"
        fi
        ;;
    "storage user webauthn delete")
        if [ "$fail" = yes ]; then
            echo '@PASSKEYS@' >&2
            exit 1
        fi
        ;;
    "storage user totp delete")
        if [ "$fail" = yes ]; then
            echo '@TOTP@' >&2
            exit 1
        fi
        ;;
    *)
        echo "Error: the stub holds no recording for: $*" >&2
        exit 1
        ;;
esac
"#;

/// busybox's `base64` as the pinned image has it: wrapped at 76 columns.
/// `@BASE64@` is the host's own.
const BASE64_SHIM: &str = r#"#!/bin/sh
# busybox's base64 wraps at 76 columns (docs/features/hub-accounts.md
# "`mcp rotate`'s output"); macOS's does not, so this one does on every host.
'@BASE64@' "$@" | fold -w 76
"#;

/// A `mv` that does the rename, then, the first time it renames something
/// over `users/users.yml`, sends SIGTERM to the script that ran it (its
/// parent) and leaves a mark (AC4, R13). `@MV@` is the host's own, `@DOOR@`
/// the mark's folder.
const MV_STUB: &str = r#"#!/bin/sh
'@MV@' "$@" || exit $?
for last in "$@"; do :; done
case $last in
    */users/users.yml)
        if [ ! -e '@DOOR@/mv-signalled' ]; then
            : > '@DOOR@/mv-signalled'
            kill -TERM "$PPID"
        fi
        ;;
esac
exit 0
"#;

/// An `od` that, asked for `-tu1` (the byte scan's numbers, R9), fails
/// printing nothing (`@MODE@` = `fail`), prints the whole scan and then fails
/// (`fail-late`, so the exit status alone tells), or exits 0 printing nothing
/// (`silent`); any other call is the host's own `@OD@`.
const OD_STUB: &str = r#"#!/bin/sh
for argument in "$@"; do
    case $argument in
        -tu1 | u1)
            case '@MODE@' in
                fail)
                    echo "od: -tu1: not supported" >&2
                    exit 1
                    ;;
                fail-late)
                    '@OD@' "$@"
                    echo "od: a read error" >&2
                    exit 1
                    ;;
            esac
            cat > /dev/null
            exit 0
            ;;
    esac
done
exec '@OD@' "$@"
"#;

/// 11b-1's guide example (`scripts/deploy/README.md`, its users file written by
/// hand): a comment after the login, a plain email, no `disabled:`.
const GUIDE_EXAMPLE: &str = "users:
  mira:                         # the login
    displayname: \"Mira K\"
    email: mira@example.org
    password: \"<hash>\"
";

static RUNS: AtomicUsize = AtomicUsize::new(0);

// ------------------------------------------------------------- the files

fn repository() -> PathBuf {
    common::repository_root()
}

fn deploy() -> PathBuf {
    repository().join("scripts/deploy")
}

fn hub_admin() -> PathBuf {
    deploy().join("admin/hub-admin")
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

fn mode(path: &Path) -> u32 {
    fs::metadata(path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()))
        .permissions()
        .mode()
        & 0o777
}

fn text_of(path: &Path) -> &str {
    path.to_str().expect("a UTF-8 path")
}

fn executable(path: &Path, text: &str) {
    fs::write(path, text).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("an executable");
}

fn first_existing(candidates: &[&'static str], what: &str) -> &'static str {
    candidates
        .iter()
        .copied()
        .find(|path| Path::new(path).exists())
        .unwrap_or_else(|| panic!("no {what} among {candidates:?}"))
}

/// The shell that runs `hub-admin`: `/bin/sh` (bash 3.2 on macOS, dash on
/// CI's Ubuntu), or `HUB_ADMIN_SH`, so a laptop can run the suite under CI's
/// dash (`HUB_ADMIN_SH=/bin/dash`): bash runs the `EXIT` trap on an untrapped
/// SIGTERM, dash does not.
fn shell() -> String {
    std::env::var("HUB_ADMIN_SH").unwrap_or_else(|_| "/bin/sh".to_owned())
}

/// `data` single-quoted for `sh`, byte for byte.
fn quote_bytes(data: &[u8]) -> Vec<u8> {
    let mut out = vec![b'\''];
    for byte in data {
        if *byte == b'\'' {
            out.extend_from_slice(b"'\\''");
        } else {
            out.push(*byte);
        }
    }
    out.push(b'\'');
    out
}

/// A file's text, ill-formed UTF-8 replaced (a refusal may echo the bytes it
/// refused).
fn lossy(path: &Path) -> String {
    String::from_utf8_lossy(&bytes(path)).into_owned()
}

/// `text` single-quoted for `sh`.
fn quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

// ------------------------------------------------------------- the users file

/// The stub's argon2 digest of call `call`, without `Digest: `.
fn argon(call: usize) -> String {
    format!("$argon2id$v=19$m=65536,t=3,p=4$c2FsdA{call}$aGFzaA{call}")
}

/// The stub's bcrypt digest of call `call`, without `Digest: `.
fn bcrypt(call: usize) -> String {
    format!("$2b$10${BCRYPT_FILL}{call:03}")
}

/// A digest a test's file held before any act.
fn old_digest(login: &str) -> String {
    format!("$argon2id$v=19$m=65536,t=3,p=4$b2xk{login}$b2xk{login}")
}

/// One entry in the written form of "The users file, written".
fn entry_with(login: &str, name: &str, email: &str, digest: &str, disabled: bool) -> String {
    format!(
        "  {login}:\n    displayname: \"{name}\"\n    email: \"{email}\"\n    password: \"{digest}\"\n    disabled: {disabled}\n"
    )
}

/// [`entry_with`] holding [`old_digest`].
fn entry(login: &str, name: &str, email: &str, disabled: bool) -> String {
    entry_with(login, name, email, &old_digest(login), disabled)
}

fn users_file(entries: &[&str]) -> String {
    let mut text = "users:\n".to_owned();
    for one in entries {
        text.push_str(one);
    }
    text
}

/// An MCP users line, its digest `fill` 53 times.
fn mcp_line(login: &str, fill: char) -> String {
    format!("{login}:$2b$10${}\n", fill.to_string().repeat(53))
}

/// The stamp "The stamp (S1)" gives, holding `n`.
fn stamp_text(n: u64) -> String {
    format!(
        "# Written by hub-admin (docs/canon/hub-deploy.md \"Accounts\"): a new value makes\n\
         # Traefik build its configuration anew, so mcp-credential reads its users file again.\n\
         http:\n  middlewares:\n    mcp-users-stamp:\n      headers:\n        customRequestHeaders:\n          X-Mcp-Users-Stamp: \"{n}\"\n"
    )
}

/// The stamp's value, the stamp checked to be one middleware and nothing
/// else: no router, no service, nothing beside `http`.
fn stamp_value(path: &Path) -> u64 {
    let stamp = yaml(path);
    let top = stamp.as_object().expect("the stamp is a mapping");
    assert_eq!(
        top.keys().map(String::as_str).collect::<Vec<_>>(),
        ["http"],
        "the stamp: {stamp}"
    );
    let http = top["http"].as_object().expect("`http` is a mapping");
    assert_eq!(
        http.keys().map(String::as_str).collect::<Vec<_>>(),
        ["middlewares"],
        "the stamp holds no router: {stamp}"
    );
    let middlewares = http["middlewares"].as_object().expect("middlewares");
    assert_eq!(middlewares.len(), 1, "one middleware: {stamp}");
    let value = stamp
        .pointer("/http/middlewares/mcp-users-stamp/headers/customRequestHeaders/X-Mcp-Users-Stamp")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("the stamp's quoted value: {stamp}"));
    value
        .parse()
        .unwrap_or_else(|error| panic!("the stamp's value {value:?}: {error}"))
}

/// A line of the act log: the UTC time, then `act`, `login`, `detail`.
fn assert_logged(line: &str, act: &str, login: &str, detail: &str) {
    let pattern = Regex::new(&format!(
        r"^[0-9]{{4}}-[0-9]{{2}}-[0-9]{{2}}T[0-9]{{2}}:[0-9]{{2}}:[0-9]{{2}}Z\t{}\t{}\t{}$",
        regex::escape(act),
        regex::escape(login),
        regex::escape(detail)
    ))
    .expect("a pattern");
    assert!(
        pattern.is_match(line),
        "an act log line `<time>\\t{act}\\t{login}\\t{detail}`: {line:?}"
    );
}

/// Every lower-case hex word of exactly `width` characters in `text`.
fn hex_words(text: &str, width: usize) -> Vec<String> {
    let pattern = Regex::new(&format!(r"\b[0-9a-f]{{{width}}}\b")).expect("a pattern");
    pattern
        .find_iter(text)
        .map(|found| found.as_str().to_owned())
        .collect()
}

/// Standard base64, padding allowed, nothing else.
fn base64_decode(text: &str) -> Vec<u8> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::new();
    let mut buffer: u32 = 0;
    let mut held: u32 = 0;
    for byte in text.bytes().filter(|byte| *byte != b'=') {
        let value = ALPHABET
            .iter()
            .position(|letter| *letter == byte)
            .unwrap_or_else(|| panic!("no base64: {text:?}"));
        buffer = (buffer << 6) | u32::try_from(value).expect("six bits");
        held += 6;
        if held >= 8 {
            held -= 8;
            out.push(u8::try_from((buffer >> held) & 0xff).expect("a byte"));
            buffer &= (1 << held) - 1;
        }
    }
    out
}

// ------------------------------------------------------------- the stub's records

/// One call of the stub `authelia`.
#[derive(Debug)]
struct Call {
    args: Vec<String>,
    stdin: String,
    on_terminal: bool,
}

impl Call {
    fn starts(&self, words: &[&str]) -> bool {
        self.args.len() >= words.len() && self.args[..words.len()] == *words
    }

    /// stdin without one final newline.
    fn secret(&self) -> &str {
        self.stdin.strip_suffix('\n').unwrap_or(&self.stdin)
    }

    /// `--`, then `login` right after it (R7).
    fn login_after_dashes(&self, login: &str) -> bool {
        self.args
            .windows(2)
            .any(|pair| pair[0] == "--" && pair[1] == login)
    }

    fn names(&self, word: &str) -> bool {
        self.args.iter().any(|arg| arg == word)
    }

    fn holds(&self, secret: &str) -> bool {
        self.args.iter().any(|arg| arg.contains(secret))
    }
}

// ------------------------------------------------------------- runs

/// One finished run of `hub-admin`.
struct Act {
    label: String,
    code: i32,
    stdout: String,
    stderr: String,
    /// What the terminal showed beside stdout and stderr (`/dev/tty`, echo).
    screen: String,
    asked: bool,
}

impl Act {
    fn show(&self) -> String {
        format!(
            "{}: exit {}\n--- stdout\n{}--- stderr\n{}--- terminal\n{}",
            self.label, self.code, self.stdout, self.stderr, self.screen
        )
    }

    fn code(&self, code: i32) -> &Self {
        assert_eq!(self.code, code, "{}", self.show());
        self
    }

    /// Exit `code` with a `hub-admin: ` line on stderr.
    fn refused(&self, code: i32) -> &Self {
        self.code(code);
        assert!(
            self.stderr.lines().any(|line| line.starts_with(REFUSAL)),
            "a `{REFUSAL}` line on stderr: {}",
            self.show()
        );
        self
    }

    fn lines(&self) -> Vec<&str> {
        self.stdout.lines().collect()
    }

    /// Whether stdout or stderr holds the line `line` (a refusal's prefix
    /// allowed on stderr).
    fn says(&self, line: &str) -> bool {
        self.stdout.lines().any(|one| one == line)
            || self
                .stderr
                .lines()
                .any(|one| one == line || one.strip_prefix(REFUSAL) == Some(line))
    }

    /// Everything the run showed.
    fn everything(&self) -> String {
        format!("{}{}{}", self.stdout, self.stderr, self.screen)
    }
}

/// A scratch `HUB_ADMIN_ROOT`, the stubs first on `PATH`, and the stub's
/// records.
struct Hub {
    scratch: Scratch,
    root: PathBuf,
    bin: PathBuf,
    door: PathBuf,
    runs: PathBuf,
    home: PathBuf,
}

impl Hub {
    fn new(label: &str) -> Self {
        assert_eq!(BCRYPT_FILL.len(), 50, "53 characters with the call's three");
        let scratch = Scratch::new(&format!("admin-{label}"));
        let root = scratch.dir("root");
        for dir in [
            "users",
            "mcp-users",
            "traefik-dynamic",
            "var/lib/hub-admin",
            "var/lib/authelia",
            "tmp",
        ] {
            fs::create_dir_all(root.join(dir)).expect("a mounted folder, or the container's /tmp");
        }
        fs::set_permissions(
            root.join("var/lib/hub-admin"),
            fs::Permissions::from_mode(0o700),
        )
        .expect("the log's folder, 700 as the guide makes it");
        fs::write(
            root.join("traefik-dynamic/hub.yml"),
            bytes(&deploy().join("traefik/dynamic/hub.yml")),
        )
        .expect("hub.yml");
        let bin = scratch.dir("bin");
        let door = scratch.dir("door");
        let runs = scratch.dir("runs");
        let home = scratch.home("h");
        executable(
            &bin.join("authelia"),
            &DOOR_STUB
                .replace("@DOOR@", text_of(&door))
                .replace("@BCRYPT@", BCRYPT_FILL)
                .replace("@PASSKEYS@", PASSKEYS_ANSWER)
                .replace("@TOTP@", TOTP_ANSWER),
        );
        executable(
            &bin.join("base64"),
            &BASE64_SHIM.replace(
                "@BASE64@",
                first_existing(&["/usr/bin/base64", "/bin/base64"], "base64"),
            ),
        );
        Self {
            scratch,
            root,
            bin,
            door,
            runs,
            home,
        }
    }

    fn users(&self) -> PathBuf {
        self.root.join("users/users.yml")
    }

    fn mcp(&self) -> PathBuf {
        self.root.join("mcp-users/users")
    }

    fn stamp(&self) -> PathBuf {
        self.root.join("traefik-dynamic/mcp-stamp.yml")
    }

    fn hub_yml(&self) -> PathBuf {
        self.root.join("traefik-dynamic/hub.yml")
    }

    fn log(&self) -> PathBuf {
        self.root.join("var/lib/hub-admin/acts.log")
    }

    fn lock(&self) -> PathBuf {
        self.root.join("var/lib/hub-admin/lock")
    }

    fn notice(&self) -> PathBuf {
        self.root.join("var/lib/authelia/notification.txt")
    }

    fn put(&self, path: &Path, text: &str) {
        self.put_bytes(path, text.as_bytes());
    }

    fn put_bytes(&self, path: &Path, data: &[u8]) {
        fs::write(path, data).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    }

    /// The paths under `admin-log/` (`var/lib/hub-admin`), relative.
    fn log_folder(&self) -> Vec<String> {
        common::snapshot(&self.root.join("var/lib/hub-admin"))
            .into_keys()
            .collect()
    }

    /// The files under the container's own `/tmp` (`$HUB_ADMIN_ROOT/tmp`).
    fn scratch_files(&self) -> Vec<String> {
        common::snapshot(&self.root.join("tmp"))
            .into_iter()
            .filter(|(_, content)| content.is_some())
            .map(|(path, _)| path)
            .collect()
    }

    /// The lock as another act would hold it.
    fn hold_lock(&self) {
        fs::create_dir_all(self.lock()).expect("the lock");
        fs::write(self.lock().join("taken"), "2026-11-04T09:00:00Z\n").expect("its time");
    }

    /// Every path and file's bytes under the root.
    fn files(&self) -> BTreeMap<String, Option<Vec<u8>>> {
        common::snapshot(&self.root)
    }

    /// The users file, the MCP file, the stamp and `hub.yml`, as they are.
    fn watched(&self) -> Vec<Option<Vec<u8>>> {
        [self.users(), self.mcp(), self.stamp(), self.hub_yml()]
            .iter()
            .map(|path| fs::read(path).ok())
            .collect()
    }

    /// Every path under the root whose name holds `.tmp`, and every file
    /// under the container's `/tmp` (R4): what a finished act may not leave.
    fn temps(&self) -> Vec<String> {
        self.files()
            .into_iter()
            .filter(|(path, content)| {
                path.contains(".tmp") || (content.is_some() && path.starts_with("tmp/"))
            })
            .map(|(path, _)| path)
            .collect()
    }

    fn log_lines(&self) -> Vec<String> {
        fs::read_to_string(self.log())
            .map(|text| text.lines().map(str::to_owned).collect())
            .unwrap_or_default()
    }

    /// The stub's control `name` (`fail-at`, `bad-at`, `hold-at`) listing
    /// `calls`.
    fn control(&self, name: &str, calls: &[usize]) {
        let listed: Vec<String> = calls.iter().map(usize::to_string).collect();
        fs::write(self.door.join(name), listed.join(" ")).expect("a control file");
    }

    fn calls(&self) -> Vec<Call> {
        let Ok(count) = fs::read_to_string(self.door.join("count")) else {
            return Vec::new();
        };
        let count: usize = count.trim().parse().expect("the stub's count");
        (1..=count)
            .map(|n| Call {
                args: read(&self.door.join(format!("args.{n}")))
                    .lines()
                    .map(str::to_owned)
                    .collect(),
                stdin: read(&self.door.join(format!("stdin.{n}"))),
                on_terminal: self.door.join(format!("tty.{n}")).exists(),
            })
            .collect()
    }

    fn env(&self) -> Vec<(OsString, OsString)> {
        let mut path = OsString::from(&self.bin);
        path.push(":/usr/bin:/bin");
        vec![
            ("PATH".into(), path),
            ("HOME".into(), self.home.clone().into()),
            ("HUB_ADMIN_ROOT".into(), self.root.clone().into()),
            ("HUB_HOST".into(), HOST.into()),
            ("LANG".into(), "en_US.UTF-8".into()),
            ("LC_ALL".into(), "en_US.UTF-8".into()),
        ]
    }

    fn terminal(&self, args: &[&str], keys: &[&str]) -> Act {
        self.terminal_with(args, keys, &mut |_: &Path| {})
    }

    fn terminal_with(&self, args: &[&str], keys: &[&str], tick: &mut dyn FnMut(&Path)) -> Act {
        let raw: Vec<&[u8]> = args.iter().map(|arg| arg.as_bytes()).collect();
        self.terminal_raw(&raw, keys, tick)
    }

    /// `sh hub-admin args` with stdin a pseudo-terminal (`script`), stdout
    /// and stderr each in a file; each `[y/N]` answered by the next of `keys`
    /// (past them the terminal's input ends); `tick` is given the file holding
    /// the script's process ID at every poll. A watchdog kills a run past
    /// [`RUN_TIMEOUT`]. The arguments are bytes, so a test can pass
    /// ill-formed UTF-8.
    fn terminal_raw(&self, args: &[&[u8]], keys: &[&str], tick: &mut dyn FnMut(&Path)) -> Act {
        let number = RUNS.fetch_add(1, Ordering::SeqCst);
        let file = |name: &str| self.runs.join(format!("{name}-{number}"));
        let (out, err, code, pid) = (file("out"), file("err"), file("code"), file("pid"));
        let (inner, outer) = (file("inner.sh"), file("outer.sh"));
        let mut wrapper = format!(
            "printf '%s\\n' \"$$\" > {}\nexec {} {}",
            quote(text_of(&pid)),
            quote(&shell()),
            quote(text_of(&hub_admin()))
        )
        .into_bytes();
        for arg in args {
            wrapper.push(b' ');
            wrapper.extend_from_slice(&quote_bytes(arg));
        }
        wrapper.push(b'\n');
        fs::write(&inner, wrapper).expect("the inner wrapper");
        let args: Vec<String> = args
            .iter()
            .map(|arg| String::from_utf8_lossy(arg).into_owned())
            .collect();
        fs::write(
            &outer,
            format!(
                "/bin/sh {} > {} 2> {}\nprintf '%s\\n' \"$?\" > {}\n",
                quote(text_of(&inner)),
                quote(text_of(&out)),
                quote(text_of(&err)),
                quote(text_of(&code))
            ),
        )
        .expect("the outer wrapper");

        let mut command = Command::new(first_existing(
            &["/usr/bin/script", "/bin/script"],
            "`script` to give hub-admin a terminal",
        ));
        if cfg!(target_os = "macos") {
            command
                .arg("-q")
                .arg("/dev/null")
                .arg("/bin/sh")
                .arg(&outer);
        } else {
            command
                .arg("-qec")
                .arg(format!("/bin/sh {}", quote(text_of(&outer))))
                .arg("/dev/null");
        }
        command
            .env_clear()
            .envs(self.env())
            .current_dir(&self.runs)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = command.spawn().expect("spawn script");
        let mut input = child.stdin.take();
        let mut terminal = child.stdout.take().expect("the terminal's output");
        let (sender, chunks) = mpsc::channel::<Vec<u8>>();
        let reader = std::thread::spawn(move || {
            let mut buffer = [0_u8; 4096];
            loop {
                match terminal.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(read) => {
                        if sender.send(buffer[..read].to_vec()).is_err() {
                            break;
                        }
                    }
                }
            }
        });
        let prompts = |screen: &[u8]| {
            [
                String::from_utf8_lossy(screen).into_owned(),
                fs::read_to_string(&out).unwrap_or_default(),
                fs::read_to_string(&err).unwrap_or_default(),
            ]
            .iter()
            .map(|text| text.matches("[y/N]").count())
            .max()
            .unwrap_or(0)
        };
        let mut screen = Vec::new();
        let mut answered = 0;
        let started = Instant::now();
        loop {
            while let Ok(chunk) = chunks.try_recv() {
                screen.extend_from_slice(&chunk);
            }
            let asked = prompts(&screen);
            while answered < asked {
                match (keys.get(answered), input.as_mut()) {
                    (Some(key), Some(stdin)) => {
                        let _ = stdin.write_all(format!("{key}\n").as_bytes());
                        let _ = stdin.flush();
                    }
                    _ => input = None,
                }
                answered += 1;
            }
            tick(&pid);
            if child.try_wait().expect("wait on script").is_some() {
                break;
            }
            if started.elapsed() > RUN_TIMEOUT {
                let _ = child.kill();
                let _ = child.wait();
                panic!(
                    "hub-admin {args:?} on a terminal ran longer than {RUN_TIMEOUT:?}; killed:\n{}",
                    String::from_utf8_lossy(&screen)
                );
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        drop(input);
        reader.join().expect("the terminal's reader");
        while let Ok(chunk) = chunks.try_recv() {
            screen.extend_from_slice(&chunk);
        }
        let asked = prompts(&screen) > 0;
        let screen = String::from_utf8_lossy(&screen).replace("\r\n", "\n");
        let status = fs::read_to_string(&code).unwrap_or_else(|_| {
            panic!("hub-admin {args:?} on a terminal left no exit status; the terminal:\n{screen}")
        });
        Act {
            label: format!("hub-admin {args:?} on a terminal"),
            code: status.trim().parse().expect("an exit status"),
            stdout: lossy(&out),
            stderr: lossy(&err),
            screen,
            asked,
        }
    }

    /// `sh hub-admin args`, stdin a pipe holding `stdin`.
    fn piped(&self, args: &[&str], stdin: &[u8]) -> Act {
        self.plain(args, Some(stdin))
    }

    /// `sh hub-admin args`, stdin `/dev/null`.
    fn null(&self, args: &[&str]) -> Act {
        self.plain(args, None)
    }

    fn plain(&self, args: &[&str], stdin: Option<&[u8]>) -> Act {
        let label = format!(
            "hub-admin {args:?} with stdin {}",
            if stdin.is_some() {
                "a pipe"
            } else {
                "/dev/null"
            }
        );
        let mut command = Command::new(shell());
        command
            .env_clear()
            .envs(self.env())
            .current_dir(&self.runs)
            .arg(hub_admin())
            .args(args)
            .stdin(if stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command
            .spawn()
            .unwrap_or_else(|error| panic!("{label}: {error}"));
        let feeder = child.stdin.take().map(|mut pipe| {
            let input = stdin.unwrap_or_default().to_vec();
            std::thread::spawn(move || {
                let _ = pipe.write_all(&input);
            })
        });
        let mut stdout = child.stdout.take().expect("stdout");
        let mut stderr = child.stderr.take().expect("stderr");
        let out = std::thread::spawn(move || {
            let mut collected = Vec::new();
            let _ = stdout.read_to_end(&mut collected);
            collected
        });
        let err = std::thread::spawn(move || {
            let mut collected = Vec::new();
            let _ = stderr.read_to_end(&mut collected);
            collected
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
        if let Some(feeder) = feeder {
            let _ = feeder.join();
        }
        Act {
            code: status
                .code()
                .unwrap_or_else(|| panic!("{label}: killed by a signal")),
            stdout: String::from_utf8_lossy(&out.join().expect("stdout")).into_owned(),
            stderr: String::from_utf8_lossy(&err.join().expect("stderr")).into_owned(),
            screen: String::new(),
            asked: false,
            label,
        }
    }
}

/// `add`'s arguments for `lena`, a login no test's file holds.
const ADD_LENA: [&str; 6] = [
    "add",
    "lena",
    "--name",
    "Lena",
    "--email",
    "lena@example.org",
];

// ------------------------------------------------------------- AC1-AC2: add

/// AC1: on a terminal, `add mira --name "Mira K" --email mira@example.org`
/// over a users file holding `olek` appends `users.mira` in the written form
/// of "Data" (`displayname`, `email`, `disabled: false`, the stub's digest
/// without `Digest: `), `olek`'s lines byte for byte. `users.yml` is a new
/// inode of mode 600, the old bytes kept as `users.yml.prev`, mode 600. The
/// password, 32 hex, is printed once and is the stub's stdin, in none of its
/// arguments. The act log, mode 600, gains one `add mira -` line holding no
/// password or digest. stdout is "`add`'s output", the `[team]` row included,
/// and the lock is gone. M: keep the prefix; write in place; drop the log line.
#[test]
fn ac01_add_appends_the_entry_through_a_new_file_and_shows_the_password_once() {
    let hub = Hub::new("ac01");
    let before = users_file(&[&entry("olek", "Olek", "olek@example.org", false)]);
    hub.put(&hub.users(), &before);
    // Held open, the old file's inode cannot be reused by the new one.
    let held = fs::File::open(hub.users()).expect("the old users file");
    let inode = held.metadata().expect("its metadata").ino();

    let act = hub.terminal(
        &[
            "add",
            "mira",
            "--name",
            "Mira K",
            "--email",
            "mira@example.org",
        ],
        &[],
    );
    act.code(0);

    let digest = argon(1);
    assert_eq!(
        read(&hub.users()),
        format!(
            "{before}{}",
            entry_with("mira", "Mira K", "mira@example.org", &digest, false)
        ),
        "the entry appended, the digest without `Digest: `: {}",
        act.show()
    );
    let written = fs::metadata(hub.users()).expect("the users file");
    assert_ne!(
        written.ino(),
        inode,
        "users.yml is a temp file renamed over it, never written in place"
    );
    assert_eq!(written.permissions().mode() & 0o777, 0o600, "users.yml");
    let previous = hub.root.join("users/users.yml.prev");
    assert_eq!(read(&previous), before, "the previous file is kept");
    assert_eq!(mode(&previous), 0o600, "users.yml.prev");
    drop(held);

    let prefix = "password, shown once; relay it privately: ";
    let shown: Vec<&str> = act
        .lines()
        .into_iter()
        .filter_map(|line| line.strip_prefix(prefix))
        .collect();
    assert_eq!(shown.len(), 1, "one password line: {}", act.show());
    let password = shown[0].to_owned();
    assert_eq!(
        hex_words(&password, 32),
        std::slice::from_ref(&password),
        "16 random bytes as 32 hex (S6)"
    );
    assert_eq!(
        act.everything().matches(&password).count(),
        1,
        "the password is printed once: {}",
        act.show()
    );
    let calls = hub.calls();
    assert_eq!(calls.len(), 1, "one call of the door's CLI: {calls:?}");
    assert!(
        calls[0].starts(&["crypto", "hash", "generate", "argon2"]),
        "{:?}",
        calls[0]
    );
    assert!(
        !calls[0].on_terminal,
        "the hash reads a pipe, not the terminal"
    );
    assert_eq!(
        calls[0].secret(),
        password,
        "the password reaches the door's CLI on stdin"
    );
    assert!(!calls[0].holds(&password), "the password is no argument");

    let expected = format!(
        "added mira\n\
         {prefix}{password}\n\
         [team] row for a reviewed commit to the project's specengine.toml (ADR-0060):\n\
         [team.mira]\n\
         name  = \"Mira K\"\n\
         role  = \"member\"            # owner | lead | member; a lead adds zones\n\
         email = \"mira@example.org\"\n"
    );
    assert_eq!(act.stdout, expected, "`add`'s output");

    let log = hub.log_lines();
    assert_eq!(log.len(), 1, "one act log line: {log:?}");
    assert_logged(&log[0], "add", "mira", "-");
    assert_eq!(mode(&hub.log()), 0o600, "acts.log");
    let log_text = read(&hub.log());
    assert!(
        !log_text.contains(&password) && !log_text.contains(&digest),
        "no password or digest in the act log: {log_text}"
    );
    assert!(!hub.lock().exists(), "the lock is gone after the act");
    assert!(hub.temps().is_empty(), "no temp file: {:?}", hub.temps());
}

/// AC1, the file's edges ("The users file, read" and "written"): a missing
/// users file is an empty `users:`, so the install's first `add` writes
/// `users:` and the entry; a file whose last line has no newline gets one
/// before the entry.
#[test]
fn ac01_add_starts_a_missing_file_and_ends_an_open_last_line() {
    let hub = Hub::new("ac01-edges");
    hub.terminal(
        &[
            "add",
            "owner",
            "--name",
            "Owner",
            "--email",
            "owner@example.org",
        ],
        &[],
    )
    .code(0);
    assert_eq!(
        read(&hub.users()),
        format!(
            "users:\n{}",
            entry_with("owner", "Owner", "owner@example.org", &argon(1), false)
        )
    );

    let whole = users_file(&[&entry("olek", "Olek", "olek@example.org", false)]);
    let open = whole.strip_suffix('\n').expect("a final newline");
    hub.put(&hub.users(), open);
    hub.terminal(
        &[
            "add",
            "mira",
            "--name",
            "Mira K",
            "--email",
            "mira@example.org",
        ],
        &[],
    )
    .code(0);
    assert_eq!(
        read(&hub.users()),
        format!(
            "{whole}{}",
            entry_with("mira", "Mira K", "mira@example.org", &argon(2), false)
        )
    );
}

/// AC2: on a terminal, `add` exits 2 with a `hub-admin: ` line, writes
/// nothing (the whole root byte for byte, so no lock left) and never calls the
/// stub for `Mira`, `mi:ra`, `mi,ra`, ` mira`, `mira.`, `local`, `agent`,
/// `null`, `true`, `false`, `-`, `1234`, `1mira`, a Cyrillic look-alike of
/// `mira`, `mira` with a zero-width space, a 65-byte login and the existing
/// `olek`; the same for a `--name` holding a lone `\xff`, an overlong
/// `\xc0\xaf`, a surrogate `\xed\xa0\x80`, U+0085, U+009B, U+2028, U+2029,
/// U+FEFF, U+FFFE, U+FFFF or a tab (R1, R2, R8). A 64-byte login starting with a letter is
/// accepted, and so is a name in another script, kept byte for byte in the
/// users file and the row. `mi.ra`'s row is `[team."mi.ra"]` with its name
/// TOML-escaped (and YAML-escaped in the users file). M: drop `:` from the
/// refused set; accept upper case; drop the existing-login check; accept a
/// leading digit; drop `null` from the reserved; accept U+2028 in `--name`;
/// skip the UTF-8 check; accept U+FFFF in `--name`.
#[test]
fn ac02_add_takes_the_ascii_grammar_alone() {
    let hub = Hub::new("ac02");
    hub.put(
        &hub.users(),
        &users_file(&[&entry("olek", "Olek", "olek@example.org", false)]),
    );
    let too_long = "a".repeat(65);
    let refused = [
        "Mira",
        "mi:ra",
        "mi,ra",
        " mira",
        "mira.",
        "local",
        "agent",
        "null",
        "true",
        "false",
        "-",
        "1234",
        "1mira",
        "\u{43c}ira",
        "mi\u{200b}ra",
        too_long.as_str(),
        "olek",
    ];
    let before = hub.files();
    for login in refused {
        let act = hub.terminal(
            &[
                "add",
                login,
                "--name",
                "Mira K",
                "--email",
                "mira@example.org",
            ],
            &[],
        );
        act.refused(2);
        assert_eq!(hub.files(), before, "{login:?}: nothing written");
        assert!(
            hub.calls().is_empty(),
            "{login:?}: the door's CLI is not called"
        );
    }

    let refused_names: [(&str, &[u8]); 11] = [
        ("a lone 0xff", b"Mi\xffra"),
        ("an overlong 0xc0 0xaf", b"Mi\xc0\xafra"),
        ("a surrogate 0xed 0xa0 0x80", b"Mi\xed\xa0\x80ra"),
        ("U+0085", "Mi\u{85}ra".as_bytes()),
        ("U+009B", "Mi\u{9b}ra".as_bytes()),
        ("U+2028", "Mi\u{2028}ra".as_bytes()),
        ("U+2029", "Mi\u{2029}ra".as_bytes()),
        ("U+FEFF", "Mi\u{feff}ra".as_bytes()),
        ("U+FFFE", "Mi\u{fffe}ra".as_bytes()),
        ("U+FFFF", "Mi\u{ffff}ra".as_bytes()),
        ("a tab", b"Mi\tra"),
    ];
    for (label, name) in refused_names {
        let act = hub.terminal_raw(
            &[
                b"add",
                b"lena",
                b"--name",
                name,
                b"--email",
                b"lena@example.org",
            ],
            &[],
            &mut |_: &Path| {},
        );
        assert_eq!(act.code, 2, "--name with {label}: {}", act.show());
        act.refused(2);
        assert_eq!(hub.files(), before, "--name with {label}: nothing written");
        assert!(
            hub.calls().is_empty(),
            "--name with {label}: the door's CLI is not called"
        );
    }

    let script_name = "\u{41c}\u{438}\u{440}\u{430}";
    let act = hub.terminal(
        &[
            "add",
            "mila",
            "--name",
            script_name,
            "--email",
            "mila@example.org",
        ],
        &[],
    );
    act.code(0);
    assert!(
        act.lines()
            .contains(&format!("name  = \"{script_name}\"").as_str()),
        "the row keeps the name: {}",
        act.show()
    );
    assert_eq!(
        yaml(&hub.users())
            .pointer("/users/mila/displayname")
            .and_then(Value::as_str),
        Some(script_name),
        "a name in any script"
    );

    let longest = format!("{}z", "a".repeat(63));
    assert_eq!(longest.len(), 64);
    hub.terminal(
        &[
            "add",
            &longest,
            "--name",
            "Long",
            "--email",
            "long@example.org",
        ],
        &[],
    )
    .code(0);

    let name = r#"Mi "Ra" \ K"#;
    let act = hub.terminal(
        &[
            "add",
            "mi.ra",
            "--name",
            name,
            "--email",
            "mi.ra@example.org",
        ],
        &[],
    );
    act.code(0);
    let lines = act.lines();
    let at = lines
        .iter()
        .position(|line| *line == "[team.\"mi.ra\"]")
        .unwrap_or_else(|| panic!("the row `[team.\"mi.ra\"]`: {}", act.show()));
    let row: toml::Table = toml::from_str(&lines[at..].join("\n"))
        .unwrap_or_else(|error| panic!("the row is TOML: {error}\n{}", act.show()));
    assert_eq!(
        row["team"]["mi.ra"]["name"].as_str(),
        Some(name),
        "the name, TOML-escaped"
    );
    assert_eq!(
        row["team"]["mi.ra"]["email"].as_str(),
        Some("mi.ra@example.org")
    );
    let users = yaml(&hub.users());
    assert_eq!(
        users
            .pointer("/users/mi.ra/displayname")
            .and_then(Value::as_str),
        Some(name),
        "the display name, double-quoted with `\\` and `\"` escaped"
    );
    assert!(
        users.pointer(&format!("/users/{longest}")).is_some(),
        "the 64-byte login: {users}"
    );
}

// ------------------------------------------------------------- AC3: no terminal

/// AC3: with stdin a pipe and with `/dev/null`, `add`, `disable`, `enable`,
/// `reset`, `mcp rotate`, `mcp revoke` and `code` exit 2 with a `hub-admin: `
/// line, write nothing and call nothing; with the lock held and the users file
/// in no known layout they still exit 2, never 1, the lock's `taken` byte for
/// byte: the refusal comes before the lock and before any read. `list`,
/// `check` and `log` answer on both. M: drop `[ -t 0 ]`.
#[test]
fn ac03_without_a_terminal_no_act_that_writes_runs() {
    let hub = Hub::new("ac03");
    hub.put(
        &hub.users(),
        &users_file(&[
            &entry("mira", "Mira K", "mira@example.org", false),
            &entry("olek", "Olek", "olek@example.org", true),
        ]),
    );
    hub.put(&hub.mcp(), &mcp_line("mira", 'M'));
    hub.put(
        &hub.notice(),
        "Date: 2026-11-04\nThe one-time code: 482913\n",
    );
    let logged = "2026-11-04T09:12:03Z\tadd\tmira\t-\n";
    hub.put(&hub.log(), logged);
    let writing: [&[&str]; 7] = [
        &ADD_LENA,
        &["disable", "mira"],
        &["enable", "olek"],
        &["reset", "mira"],
        &["mcp", "rotate", "mira"],
        &["mcp", "revoke", "mira"],
        &["code"],
    ];
    let refuse_all = |round: &str| {
        let before = hub.files();
        for args in writing {
            for piped in [true, false] {
                let act = if piped {
                    hub.piped(args, b"y\n")
                } else {
                    hub.null(args)
                };
                assert_eq!(act.code, 2, "{round}: {}", act.show());
                act.refused(2);
                assert_eq!(hub.files(), before, "{round}: {}", act.label);
                assert!(hub.calls().is_empty(), "{round}: {}", act.label);
            }
        }
    };
    refuse_all("as set");

    let list = hub.null(&["list"]);
    list.code(0);
    assert!(
        list.lines()
            .contains(&"mira\tenabled\tmcp\tmira@example.org\tMira K"),
        "{}",
        list.show()
    );
    hub.piped(&["list"], b"").code(0);
    let log = hub.null(&["log"]);
    log.code(0);
    assert_eq!(log.stdout, logged, "the act log");
    hub.piped(&["log"], b"").code(0);
    let check = hub.piped(
        &["check"],
        b"mira\towner\tmira@example.org\nolek\tmember\tolek@example.org\n",
    );
    check.code(0);
    assert_eq!(
        check.lines().last().copied(),
        Some("check: 2 logins, 1 findings"),
        "{}",
        check.show()
    );
    let empty = hub.null(&["check"]);
    empty.code(0);
    assert_eq!(empty.lines(), ["check: 0 logins, 0 findings"]);

    hub.hold_lock();
    hub.put(&hub.users(), "users:\n  mira:\n\tdisplayname: \"Mira K\"\n");
    refuse_all("the lock held, the layout unknown");
}

// ------------------------------------------------------------- AC4-AC5: disable, enable

/// AC4: one `disable mira` sets `disabled: true`, every other byte kept,
/// removes `mira`'s MCP line, `olek`'s kept, and writes the stamp at 1 (none
/// before), mode 644, one middleware and no router; `hub.yml` stays byte for
/// byte; it prints `disabled mira` and `mcp line removed` and logs `disable
/// mira mcp line removed`, asking nothing while two accounts stay enabled. A
/// second `disable` prints `unchanged mira` and writes nothing. A stamp at 7
/// goes to 8. M: keep the line; skip the stamp.
#[test]
fn ac04_disable_closes_the_account_and_its_mcp_line_and_bumps_the_stamp() {
    let hub = Hub::new("ac04");
    let olek = entry("olek", "Olek", "olek@example.org", false);
    let lena = entry("lena", "Lena", "lena@example.org", false);
    hub.put(
        &hub.users(),
        &users_file(&[
            &entry("mira", "Mira K", "mira@example.org", false),
            &olek,
            &lena,
        ]),
    );
    hub.put(
        &hub.mcp(),
        &format!("{}{}", mcp_line("olek", 'O'), mcp_line("mira", 'M')),
    );
    let hub_yml = bytes(&hub.hub_yml());

    let act = hub.terminal(&["disable", "mira"], &[]);
    act.code(0);
    assert!(!act.asked, "two accounts stay enabled: {}", act.show());
    assert_eq!(
        read(&hub.users()),
        users_file(&[
            &entry("mira", "Mira K", "mira@example.org", true),
            &olek,
            &lena
        ])
    );
    assert_eq!(
        read(&hub.mcp()),
        mcp_line("olek", 'O'),
        "mira's line removed"
    );
    assert_eq!(stamp_value(&hub.stamp()), 1, "the stamp, absent before");
    assert_eq!(mode(&hub.stamp()), 0o644, "the stamp");
    assert_eq!(bytes(&hub.hub_yml()), hub_yml, "hub.yml byte for byte");
    let lines = act.lines();
    let disabled = lines.iter().position(|line| *line == "disabled mira");
    let removed = lines.iter().position(|line| *line == "mcp line removed");
    assert!(
        matches!((disabled, removed), (Some(a), Some(b)) if a < b),
        "`disabled mira`, then `mcp line removed`: {}",
        act.show()
    );
    let log = hub.log_lines();
    assert_eq!(log.len(), 1, "{log:?}");
    assert_logged(&log[0], "disable", "mira", "mcp line removed");
    assert!(hub.calls().is_empty(), "disable calls nothing of the door");

    let before = hub.files();
    let again = hub.terminal(&["disable", "mira"], &[]);
    again.code(0);
    assert!(again.says("unchanged mira"), "{}", again.show());
    assert_eq!(
        hub.files(),
        before,
        "an unchanged act writes and logs nothing"
    );

    hub.put(&hub.stamp(), &stamp_text(7));
    hub.terminal(&["disable", "olek"], &[]).code(0);
    assert_eq!(
        stamp_value(&hub.stamp()),
        8,
        "one more than the file's value"
    );
    assert_eq!(read(&hub.mcp()), "", "olek's line removed");
    assert_eq!(bytes(&hub.hub_yml()), hub_yml, "hub.yml byte for byte");
}

/// AC4: disabling the last enabled account warns (`the last enabled account;
/// the host can still restore one`) and asks `[y/N]` on the terminal: `n`
/// exits 1 with every file unchanged; `y` disables, removes the line and
/// writes the stamp. M: drop the question.
#[test]
fn ac04_disabling_the_last_enabled_account_asks_first() {
    let hub = Hub::new("ac04-last");
    hub.put(
        &hub.users(),
        &users_file(&[
            &entry("mira", "Mira K", "mira@example.org", false),
            &entry("olek", "Olek", "olek@example.org", true),
        ]),
    );
    hub.put(&hub.mcp(), &mcp_line("mira", 'M'));
    let before = hub.files();

    let no = hub.terminal(&["disable", "mira"], &["n"]);
    assert!(no.asked, "a `[y/N]`: {}", no.show());
    assert!(
        no.everything()
            .contains("the last enabled account; the host can still restore one"),
        "the warning: {}",
        no.show()
    );
    no.code(1);
    assert_eq!(hub.files(), before, "`n` writes nothing");

    let yes = hub.terminal(&["disable", "mira"], &["y"]);
    assert!(yes.asked, "{}", yes.show());
    yes.code(0);
    assert_eq!(
        read(&hub.users()),
        users_file(&[
            &entry("mira", "Mira K", "mira@example.org", true),
            &entry("olek", "Olek", "olek@example.org", true),
        ])
    );
    assert_eq!(read(&hub.mcp()), "");
    assert_eq!(stamp_value(&hub.stamp()), 1);
}

/// AC4 over 11b-1's guide example ("The users file, written"): an entry with
/// no `disabled:` line gets `    disabled: true` after its last field, its
/// comment and plain email kept; `enable` then replaces that line.
#[test]
fn ac04_disable_inserts_the_line_an_entry_lacks() {
    let hub = Hub::new("ac04-guide");
    let olek = entry("olek", "Olek", "olek@example.org", false);
    hub.put(&hub.users(), &format!("{GUIDE_EXAMPLE}{olek}"));
    hub.terminal(&["disable", "mira"], &[]).code(0);
    let disabled = format!("{GUIDE_EXAMPLE}    disabled: true\n{olek}");
    assert_eq!(read(&hub.users()), disabled);
    hub.terminal(&["enable", "mira"], &[]).code(0);
    assert_eq!(
        read(&hub.users()),
        format!("{GUIDE_EXAMPLE}    disabled: false\n{olek}")
    );
}

/// AC4, part-way (R5): with the MCP folder unwritable, `disable mira`
/// replaces `users.yml` first (`disabled: true`), leaves the MCP file and the
/// stamp as they were, says `hub-admin: disabled at the door; MCP line left:
/// run disable again` (never "nothing changed"), logs `disable mira
/// mcp-line-left` and exits 1, leaving no temp file and no lock. Run again
/// with the folder writable, the already disabled `mira` still holding a line
/// is no `unchanged`: the line goes, the stamp is written, `mcp line removed`
/// is printed and logged, exit 0. M: the MCP step before the door's; say
/// "nothing changed"; `unchanged` for a disabled login still holding a line.
#[test]
fn ac04_a_part_way_disable_says_so_and_a_rerun_finishes() {
    let hub = Hub::new("ac04-part-way");
    let olek = entry("olek", "Olek", "olek@example.org", false);
    hub.put(
        &hub.users(),
        &users_file(&[&entry("mira", "Mira K", "mira@example.org", false), &olek]),
    );
    let lines = format!("{}{}", mcp_line("olek", 'O'), mcp_line("mira", 'M'));
    hub.put(&hub.mcp(), &lines);
    let folder = hub.root.join("mcp-users");
    fs::set_permissions(&folder, fs::Permissions::from_mode(0o500)).expect("a read-only folder");
    let probe = fs::write(folder.join("probe"), "");
    let act = hub.terminal(&["disable", "mira"], &[]);
    fs::set_permissions(&folder, fs::Permissions::from_mode(0o755)).expect("writable again");
    assert!(
        probe.is_err(),
        "the MCP folder must refuse a new file: run the tests as a user, not root"
    );

    act.code(1);
    assert!(
        act.stderr
            .lines()
            .any(|line| line == "hub-admin: disabled at the door; MCP line left: run disable again"),
        "{}",
        act.show()
    );
    assert!(
        !act.everything().contains("nothing changed"),
        "the door changed: {}",
        act.show()
    );
    let disabled = users_file(&[&entry("mira", "Mira K", "mira@example.org", true), &olek]);
    assert_eq!(read(&hub.users()), disabled, "the door first");
    assert_eq!(read(&hub.mcp()), lines, "the MCP file as it was");
    assert!(!hub.stamp().exists(), "no stamp");
    let log = hub.log_lines();
    assert_eq!(log.len(), 1, "{log:?}");
    assert_logged(&log[0], "disable", "mira", "mcp-line-left");
    assert!(hub.temps().is_empty(), "no temp file: {:?}", hub.temps());
    assert!(!hub.lock().exists(), "the lock is gone");

    let again = hub.terminal(&["disable", "mira"], &[]);
    again.code(0);
    assert!(!again.says("unchanged mira"), "{}", again.show());
    assert!(again.says("mcp line removed"), "{}", again.show());
    assert_eq!(read(&hub.mcp()), mcp_line("olek", 'O'), "the line goes");
    assert_eq!(stamp_value(&hub.stamp()), 1, "the stamp is written");
    assert_eq!(read(&hub.users()), disabled, "users.yml stays");
    let log = hub.log_lines();
    assert_eq!(log.len(), 2, "{log:?}");
    assert_logged(&log[1], "disable", "mira", "mcp line removed");
}

/// AC4 (R13): with an MCP file holding a refused line, `disable` of the
/// enabled `mira` still writes `disabled: true` (the door step comes before the
/// MCP file is read), leaves the MCP file as it was, says `hub-admin: disabled
/// at the door; MCP line left: run disable again`, logs `disable mira
/// mcp-line-left` and exits 1. M: read the MCP file before the door step.
#[test]
fn ac04_a_bad_mcp_line_never_blocks_the_doors_disable() {
    let hub = Hub::new("ac04-bad-mcp");
    let olek = entry("olek", "Olek", "olek@example.org", false);
    hub.put(
        &hub.users(),
        &users_file(&[&entry("mira", "Mira K", "mira@example.org", false), &olek]),
    );
    let lines = format!("{}this is no line\n", mcp_line("mira", 'M'));
    hub.put(&hub.mcp(), &lines);

    let act = hub.terminal(&["disable", "mira"], &[]);
    act.code(1);
    assert!(
        act.stderr
            .lines()
            .any(|line| line == "hub-admin: disabled at the door; MCP line left: run disable again"),
        "{}",
        act.show()
    );
    assert_eq!(
        read(&hub.users()),
        users_file(&[&entry("mira", "Mira K", "mira@example.org", true), &olek]),
        "the door's disable is done"
    );
    assert_eq!(read(&hub.mcp()), lines, "the MCP file as it was");
    assert!(!hub.stamp().exists(), "no stamp");
    let log = hub.log_lines();
    assert_eq!(log.len(), 1, "{log:?}");
    assert_logged(&log[0], "disable", "mira", "mcp-line-left");
    assert!(!hub.lock().exists(), "the lock is gone");
}

/// AC4 (R13): a `mv` first on `PATH` renames `users.yml` and then sends
/// SIGTERM to the script; the log still gains `disable mira mcp-line-left`,
/// `users.yml` holds `disabled: true`, and the lock is gone. M: set the stage
/// only after the MCP step.
#[test]
fn ac04_a_signal_right_after_the_rename_still_logs_the_disable() {
    let hub = Hub::new("ac04-mv-signal");
    executable(
        &hub.bin.join("mv"),
        &MV_STUB
            .replace("@MV@", first_existing(&["/bin/mv", "/usr/bin/mv"], "mv"))
            .replace("@DOOR@", text_of(&hub.door)),
    );
    let olek = entry("olek", "Olek", "olek@example.org", false);
    hub.put(
        &hub.users(),
        &users_file(&[&entry("mira", "Mira K", "mira@example.org", false), &olek]),
    );
    hub.put(&hub.mcp(), &mcp_line("mira", 'M'));

    let act = hub.terminal(&["disable", "mira"], &[]);
    assert!(
        hub.door.join("mv-signalled").exists(),
        "the stub renamed users.yml and signalled: {}",
        act.show()
    );
    assert_ne!(act.code, 0, "a signalled act: {}", act.show());
    assert_eq!(
        read(&hub.users()),
        users_file(&[&entry("mira", "Mira K", "mira@example.org", true), &olek]),
        "users.yml was renamed in place"
    );
    let log = hub.log_lines();
    assert_eq!(log.len(), 1, "{log:?}: {}", act.show());
    assert_logged(&log[0], "disable", "mira", "mcp-line-left");
    assert!(!hub.lock().exists(), "the lock is gone");
}

/// AC5: `enable olek` sets `disabled: false`, every other byte kept, and
/// writes no MCP line or stamp: the MCP file and the stamp byte for byte, the
/// door's CLI never called; it prints `enabled olek` and logs `enable olek
/// -`. A second `enable` prints `unchanged olek` and writes nothing. M:
/// re-issue a line.
#[test]
fn ac05_enable_reopens_the_account_alone() {
    let hub = Hub::new("ac05");
    let mira = entry("mira", "Mira K", "mira@example.org", false);
    hub.put(
        &hub.users(),
        &users_file(&[&mira, &entry("olek", "Olek", "olek@example.org", true)]),
    );
    hub.put(&hub.mcp(), &mcp_line("mira", 'M'));
    hub.put(&hub.stamp(), &stamp_text(4));
    let (mcp, stamp) = (bytes(&hub.mcp()), bytes(&hub.stamp()));

    let act = hub.terminal(&["enable", "olek"], &[]);
    act.code(0);
    assert_eq!(
        read(&hub.users()),
        users_file(&[&mira, &entry("olek", "Olek", "olek@example.org", false)])
    );
    assert_eq!(bytes(&hub.mcp()), mcp, "no MCP line is issued");
    assert_eq!(bytes(&hub.stamp()), stamp, "no stamp is written");
    assert!(hub.calls().is_empty(), "the door's CLI is not called");
    assert!(act.says("enabled olek"), "{}", act.show());
    let log = hub.log_lines();
    assert_eq!(log.len(), 1, "{log:?}");
    assert_logged(&log[0], "enable", "olek", "-");

    let before = hub.files();
    let again = hub.terminal(&["enable", "olek"], &[]);
    again.code(0);
    assert!(again.says("unchanged olek"), "{}", again.show());
    assert_eq!(hub.files(), before);
}

// ------------------------------------------------------------- AC6-AC7: reset, rotate

/// AC6: `reset mira` writes the stub's new digest in `mira`'s `password:`
/// line, every other byte kept. The stub's log shows, in order, the argon2
/// hash (the printed password on stdin), the WebAuthn delete with `--all`,
/// and the TOTP delete, each with `--` right before `mira` (R7), and nothing
/// else. It prints `reset mira` and the password once, and logs `reset mira
/// password passkeys totp`. M: skip the WebAuthn delete; drop `--`.
#[test]
fn ac06_reset_hashes_then_deletes_passkeys_then_totp() {
    let hub = Hub::new("ac06");
    let olek = entry("olek", "Olek", "olek@example.org", false);
    hub.put(
        &hub.users(),
        &users_file(&[&entry("mira", "Mira K", "mira@example.org", false), &olek]),
    );

    let act = hub.terminal(&["reset", "mira"], &[]);
    act.code(0);
    assert_eq!(
        read(&hub.users()),
        users_file(&[
            &entry_with("mira", "Mira K", "mira@example.org", &argon(1), false),
            &olek,
        ]),
        "the password line replaced, every other byte kept"
    );
    let calls = hub.calls();
    assert_eq!(
        calls.len(),
        3,
        "the hash, two deletes, nothing else: {calls:?}"
    );
    assert!(
        calls[0].starts(&["crypto", "hash", "generate", "argon2"]),
        "{:?}",
        calls[0]
    );
    assert!(
        calls[1].starts(&["storage", "user", "webauthn", "delete"])
            && calls[1].login_after_dashes("mira")
            && calls[1].names("--all"),
        "the WebAuthn delete with --all, `--` right before mira: {:?}",
        calls[1]
    );
    assert!(
        calls[2].starts(&["storage", "user", "totp", "delete"])
            && calls[2].login_after_dashes("mira"),
        "the TOTP delete, `--` right before mira: {:?}",
        calls[2]
    );

    assert_eq!(
        act.lines().first().copied(),
        Some("reset mira"),
        "{}",
        act.show()
    );
    let shown = hex_words(&act.stdout, 32);
    assert_eq!(shown.len(), 1, "one password: {}", act.show());
    assert_eq!(calls[0].secret(), shown[0], "the password on stdin");
    assert!(!calls[0].holds(&shown[0]), "the password is no argument");
    assert_eq!(act.everything().matches(&shown[0]).count(), 1);
    let log = hub.log_lines();
    assert_eq!(log.len(), 1, "{log:?}");
    assert_logged(&log[0], "reset", "mira", "password passkeys totp");
}

/// AC7: `mcp rotate mira` prints `mcp secret for mira, shown once; ...` with
/// one 64-hex secret, `on mira's machine, once per project slug:`, and the
/// `claude mcp add` line for `https://hub.example.org/mcp/<slug>`, whose Basic
/// value decodes to `mira:<secret>` on one line (the `base64` first on `PATH`
/// wraps as busybox's does). `mira`'s line is replaced, so the file holds one
/// `mira:` line, the stub's digest, and the others byte for byte in order. The
/// secret is the stub's stdin and in none of its arguments. The stamp goes to
/// 1, and the act log gains `mcp-rotate mira -`, no secret or digest. `rotate`
/// of the disabled `zed` exits 2, writing and calling nothing. M: append a
/// line; pass the secret as an argument; base64 without removing newlines.
#[test]
fn ac07_rotate_replaces_the_line_and_prints_one_usable_header() {
    let hub = Hub::new("ac07");
    hub.put(
        &hub.users(),
        &users_file(&[
            &entry("mira", "Mira K", "mira@example.org", false),
            &entry("olek", "Olek", "olek@example.org", false),
            &entry("zed", "Zed", "zed@example.org", true),
        ]),
    );
    let (olek, ana) = (mcp_line("olek", 'O'), mcp_line("ana", 'A'));
    hub.put(&hub.mcp(), &format!("{olek}{}{ana}", mcp_line("mira", 'M')));

    let act = hub.terminal(&["mcp", "rotate", "mira"], &[]);
    act.code(0);
    let lines = act.lines();
    assert_eq!(lines.len(), 3, "{}", act.show());
    let secret = lines[0]
        .strip_prefix("mcp secret for mira, shown once; relay it privately: ")
        .unwrap_or_else(|| panic!("the secret's line: {}", act.show()))
        .to_owned();
    assert_eq!(
        hex_words(&secret, 64),
        std::slice::from_ref(&secret),
        "32 random bytes as 64 hex (S6)"
    );
    assert_eq!(lines[1], "on mira's machine, once per project slug:");
    let add = Regex::new(&format!(
        r#"^claude mcp add --transport http --scope user hub-<slug> https://{}/mcp/<slug> --header "Authorization: Basic ([A-Za-z0-9+/=]+)"$"#,
        regex::escape(HOST)
    ))
    .expect("a pattern");
    let basic = add
        .captures(lines[2])
        .unwrap_or_else(|| panic!("the `claude mcp add` line: {:?}", lines[2]))[1]
        .to_owned();
    assert_eq!(
        String::from_utf8(base64_decode(&basic)).expect("UTF-8"),
        format!("mira:{secret}"),
        "the Basic value"
    );
    assert_eq!(act.everything().matches(&secret).count(), 1, "shown once");

    let calls = hub.calls();
    assert_eq!(calls.len(), 1, "{calls:?}");
    assert!(
        calls[0].starts(&["crypto", "hash", "generate", "bcrypt"]),
        "{:?}",
        calls[0]
    );
    assert!(
        !calls[0].on_terminal,
        "the hash reads a pipe, not the terminal"
    );
    assert_eq!(calls[0].secret(), secret, "the secret on stdin");
    assert!(!calls[0].holds(&secret), "the secret is no argument");

    let file = read(&hub.mcp());
    let (own, others): (Vec<&str>, Vec<&str>) =
        file.lines().partition(|line| line.starts_with("mira:"));
    assert_eq!(
        own,
        [format!("mira:{}", bcrypt(1))],
        "one mira line: {file}"
    );
    assert_eq!(
        others,
        [olek.trim_end(), ana.trim_end()],
        "the others byte for byte"
    );
    assert_eq!(stamp_value(&hub.stamp()), 1);
    let log = hub.log_lines();
    assert_eq!(log.len(), 1, "{log:?}");
    assert_logged(&log[0], "mcp-rotate", "mira", "-");
    let log_text = read(&hub.log());
    assert!(!log_text.contains(&secret) && !log_text.contains(&bcrypt(1)));

    let before = hub.files();
    hub.terminal(&["mcp", "rotate", "zed"], &[]).refused(2);
    assert_eq!(hub.files(), before, "a disabled login: nothing written");
    assert_eq!(hub.calls().len(), 1, "nor called");
}

/// AC7 (R11): with the stamp's folder unwritable, `mcp rotate mira` leaves
/// the new line in the MCP file (the stub's digest, the others byte for byte),
/// still prints the 64-hex secret and its `claude mcp add` line (whose Basic
/// value decodes to `mira:<secret>`), says to run `docker compose restart
/// traefik`, logs `mcp-rotate mira stamp-not-written`, writes no stamp and
/// exits 1. M: exit without the secret.
#[test]
fn ac07_a_stamp_that_cannot_be_written_still_shows_the_secret() {
    let hub = Hub::new("ac07-stamp");
    hub.put(
        &hub.users(),
        &users_file(&[&entry("mira", "Mira K", "mira@example.org", false)]),
    );
    let olek = mcp_line("olek", 'O');
    hub.put(&hub.mcp(), &format!("{olek}{}", mcp_line("mira", 'M')));
    let folder = hub.root.join("traefik-dynamic");
    fs::set_permissions(&folder, fs::Permissions::from_mode(0o500)).expect("a read-only folder");
    let probe = fs::write(folder.join("probe"), "");
    let act = hub.terminal(&["mcp", "rotate", "mira"], &[]);
    fs::set_permissions(&folder, fs::Permissions::from_mode(0o755)).expect("writable again");
    assert!(
        probe.is_err(),
        "the stamp's folder must refuse a new file: run the tests as a user, not root"
    );

    act.code(1);
    let secret = act
        .lines()
        .into_iter()
        .find_map(|line| line.strip_prefix("mcp secret for mira, shown once; relay it privately: "))
        .unwrap_or_else(|| panic!("the secret is still shown: {}", act.show()))
        .to_owned();
    assert_eq!(hex_words(&secret, 64), std::slice::from_ref(&secret));
    let add = Regex::new(&format!(
        r#"^claude mcp add --transport http --scope user hub-<slug> https://{}/mcp/<slug> --header "Authorization: Basic ([A-Za-z0-9+/=]+)"$"#,
        regex::escape(HOST)
    ))
    .expect("a pattern");
    let basic = act
        .lines()
        .into_iter()
        .find_map(|line| add.captures(line).map(|found| found[1].to_owned()))
        .unwrap_or_else(|| panic!("the `claude mcp add` line: {}", act.show()));
    assert_eq!(
        String::from_utf8(base64_decode(&basic)).expect("UTF-8"),
        format!("mira:{secret}")
    );
    assert!(
        act.everything().contains("docker compose restart traefik"),
        "the restart line: {}",
        act.show()
    );
    let calls = hub.calls();
    assert_eq!(calls.len(), 1, "{calls:?}");
    assert_eq!(
        calls[0].secret(),
        secret,
        "the shown secret is the hashed one"
    );
    assert_eq!(
        read(&hub.mcp()),
        format!("{olek}mira:{}\n", bcrypt(1)),
        "the new line stays"
    );
    assert!(!hub.stamp().exists(), "no stamp");
    let log = hub.log_lines();
    assert_eq!(log.len(), 1, "{log:?}");
    assert_logged(&log[0], "mcp-rotate", "mira", "stamp-not-written");
    assert!(!read(&hub.log()).contains(&secret), "no secret in the log");
    assert!(!hub.lock().exists(), "the lock is gone");
}

// ------------------------------------------------------------- AC8: the lock

/// AC8: while `admin-log/lock/` exists, `add`, `disable`, `enable`, `reset`,
/// `mcp rotate` and `mcp revoke` on a terminal exit 1 naming it (`another act
/// holds admin-log/lock`), write nothing and call nothing, and the holder's
/// `.mcp-stamp.tmp` survives each refused run (R4). M: no lock check; remove
/// temp files without holding the lock.
#[test]
fn ac08_a_held_lock_refuses_every_act_that_writes() {
    let hub = Hub::new("ac08");
    hub.put(
        &hub.users(),
        &users_file(&[
            &entry("mira", "Mira K", "mira@example.org", false),
            &entry("olek", "Olek", "olek@example.org", false),
            &entry("zed", "Zed", "zed@example.org", true),
        ]),
    );
    hub.put(&hub.mcp(), &mcp_line("mira", 'M'));
    hub.hold_lock();
    let holders = hub.root.join("traefik-dynamic/.mcp-stamp.tmp");
    let staged = stamp_text(5);
    hub.put(&holders, &staged);
    let before = hub.files();
    let writing: [&[&str]; 6] = [
        &ADD_LENA,
        &["disable", "olek"],
        &["enable", "zed"],
        &["reset", "mira"],
        &["mcp", "rotate", "mira"],
        &["mcp", "revoke", "mira"],
    ];
    for args in writing {
        let act = hub.terminal(args, &[]);
        act.refused(1);
        assert!(
            act.stderr.contains("another act holds admin-log/lock"),
            "{}",
            act.show()
        );
        assert_eq!(
            fs::read_to_string(&holders).ok().as_deref(),
            Some(staged.as_str()),
            "{}: the holder's .mcp-stamp.tmp survives",
            act.label
        );
        assert_eq!(hub.files(), before, "{}", act.label);
        assert!(hub.calls().is_empty(), "{}", act.label);
    }
}

/// AC8: the lock is gone after a normal act, and after SIGTERM to an act held
/// in the stub (`add`'s hash), during which the lock exists and the door's
/// output is captured under `$HUB_ADMIN_ROOT/tmp`, the container's own, with
/// nothing new in `admin-log/`; afterwards `admin-log/` holds `acts.log` alone
/// and no temp file remains (R4). M: no trap; capture the door's output in
/// `admin-log/`.
#[test]
fn ac08_the_lock_goes_with_the_act_even_on_sigterm() {
    let hub = Hub::new("ac08-trap");
    hub.put(
        &hub.users(),
        &users_file(&[
            &entry("olek", "Olek", "olek@example.org", false),
            &entry("zed", "Zed", "zed@example.org", true),
        ]),
    );
    hub.terminal(&["enable", "zed"], &[]).code(0);
    assert!(!hub.lock().exists(), "the lock is gone after a normal act");
    assert_eq!(
        hub.log_folder(),
        ["acts.log"],
        "admin-log/ after a normal act"
    );

    hub.control("hold-at", &[1]);
    let kill = first_existing(&["/bin/kill", "/usr/bin/kill"], "kill");
    let mut locked_while_held = None;
    let mut captured = Vec::new();
    let mut log_folder = Vec::new();
    let act = hub.terminal_with(&ADD_LENA, &[], &mut |pid: &Path| {
        if locked_while_held.is_some() || !hub.door.join("held").exists() {
            return;
        }
        locked_while_held = Some(hub.lock().is_dir());
        captured = hub.scratch_files();
        log_folder = hub.log_folder();
        let pid = read(pid).trim().to_owned();
        let sent = Command::new(kill)
            .args(["-TERM", &pid])
            .status()
            .expect("kill");
        assert!(sent.success(), "SIGTERM to {pid}");
        std::thread::sleep(Duration::from_millis(300));
        fs::write(hub.door.join("release"), "").expect("the stub's release");
    });
    assert_eq!(
        locked_while_held,
        Some(true),
        "the stub held the act, the lock taken: {}",
        act.show()
    );
    assert!(
        !captured.is_empty(),
        "the door's output is captured under $HUB_ADMIN_ROOT/tmp while it runs"
    );
    let lock = format!("lock{}", std::path::MAIN_SEPARATOR);
    assert!(
        log_folder
            .iter()
            .all(|path| path == "acts.log" || path == "lock" || path.starts_with(&lock)),
        "admin-log/ while the door runs holds the log and the lock alone: {log_folder:?}"
    );
    assert!(
        !hub.lock().exists(),
        "SIGTERM leaves no lock: {}",
        act.show()
    );
    assert_eq!(hub.log_folder(), ["acts.log"], "admin-log/ afterwards");
    assert!(hub.temps().is_empty(), "no temp file: {:?}", hub.temps());
}

// ------------------------------------------------------------- AC9: a failing door

/// AC9: with the stub's hash exiting 1, or answering a `Digest:` line off its
/// shape, `add`, `reset` and `mcp rotate` exit 1 with a `hub-admin: ` line,
/// make no call after the hash (so no delete), keep the users file, the MCP
/// file, the stamp and `hub.yml` byte for byte, log nothing, and leave no temp
/// file and no lock. A failing delete is AC19's. M: write before the hash
/// returns; accept any digest line.
#[test]
fn ac09_a_failing_door_leaves_every_file_whole() {
    let reset: &[&str] = &["reset", "mira"];
    let rotate: &[&str] = &["mcp", "rotate", "mira"];
    let cases: [(&[&str], usize, &str); 6] = [
        (&ADD_LENA, 1, "fail-at"),
        (reset, 1, "fail-at"),
        (rotate, 1, "fail-at"),
        (&ADD_LENA, 1, "bad-at"),
        (reset, 1, "bad-at"),
        (rotate, 1, "bad-at"),
    ];
    for (args, call, control) in cases {
        let label = format!("{args:?}, {control} {call}");
        let hub = Hub::new("ac09");
        hub.put(
            &hub.users(),
            &users_file(&[
                &entry("mira", "Mira K", "mira@example.org", false),
                &entry("olek", "Olek", "olek@example.org", false),
            ]),
        );
        hub.put(&hub.mcp(), &mcp_line("mira", 'M'));
        hub.put(&hub.stamp(), &stamp_text(3));
        let watched = hub.watched();
        hub.control(control, &[call]);

        let act = hub.terminal(args, &[]);
        assert_eq!(act.code, 1, "{label}: {}", act.show());
        act.refused(1);
        assert_eq!(
            hub.calls().len(),
            call,
            "{label}: nothing after the hash, no delete"
        );
        assert_eq!(hub.watched(), watched, "{label}: every file whole");
        assert!(hub.temps().is_empty(), "{label}: {:?}", hub.temps());
        assert!(!hub.lock().exists(), "{label}: the lock is gone");
        let log = hub.log_lines();
        assert!(log.is_empty(), "{label}: nothing to log: {log:?}");
    }
}

// ------------------------------------------------------------- AC19: a failing delete

/// AC19 (R3, R10): `reset mira` with the stub's delete exiting 1 with its
/// recorded "nothing to delete" answer still writes the new digest with every
/// other byte kept and shows the password once, `hub-admin: warning: TOTP not
/// deleted: <the answer>` (or `passkeys not deleted: ...`) on stderr and no
/// warning for a delete that passed, the log's detail `totp-left`,
/// `passkeys-left` or `passkeys-left totp-left`. The TOTP delete still runs
/// after a failed passkey delete: three calls each time. A TOTP failure alone
/// exits 0; a passkey left exits 1 with `hub-admin: passkeys not deleted: do
/// not enable this account until reset ends without passkeys-left`. M: abort
/// on the delete's failure; skip the TOTP delete after a failed passkey
/// delete; exit 0 on `passkeys-left`.
#[test]
fn ac19_reset_completes_with_a_warning_when_a_delete_fails() {
    let passkeys = format!("{REFUSAL}warning: passkeys not deleted: {PASSKEYS_ANSWER}");
    let totp = format!("{REFUSAL}warning: TOTP not deleted: {TOTP_ANSWER}");
    let cases: [(&str, &[usize], &str); 3] = [
        ("the TOTP delete fails", &[3], "totp-left"),
        ("the passkey delete fails", &[2], "passkeys-left"),
        ("both deletes fail", &[2, 3], "passkeys-left totp-left"),
    ];
    for (label, failing, detail) in cases {
        let hub = Hub::new("ac19");
        let olek = entry("olek", "Olek", "olek@example.org", false);
        hub.put(
            &hub.users(),
            &users_file(&[&entry("mira", "Mira K", "mira@example.org", false), &olek]),
        );
        hub.control("fail-at", failing);

        let act = hub.terminal(&["reset", "mira"], &[]);
        let passkeys_left = failing.contains(&2);
        assert_eq!(
            act.code,
            i32::from(passkeys_left),
            "{label}: exit 1 with a passkey left, else 0 (R10): {}",
            act.show()
        );
        assert_eq!(
            act.stderr.lines().any(|line| line == NO_ENABLE),
            passkeys_left,
            "{label}: `{NO_ENABLE}`: {}",
            act.show()
        );
        assert_eq!(
            read(&hub.users()),
            users_file(&[
                &entry_with("mira", "Mira K", "mira@example.org", &argon(1), false),
                &olek,
            ]),
            "{label}: the new password is written"
        );
        let calls = hub.calls();
        assert_eq!(
            calls.len(),
            3,
            "{label}: the hash and both deletes: {calls:?}"
        );
        assert!(
            calls[1].starts(&["storage", "user", "webauthn", "delete"])
                && calls[2].starts(&["storage", "user", "totp", "delete"]),
            "{label}: the passkey delete, then the TOTP delete: {calls:?}"
        );
        assert_eq!(
            act.stderr.lines().any(|line| line == passkeys),
            failing.contains(&2),
            "{label}: `{passkeys}`: {}",
            act.show()
        );
        assert_eq!(
            act.stderr.lines().any(|line| line == totp),
            failing.contains(&3),
            "{label}: `{totp}`: {}",
            act.show()
        );
        let shown = hex_words(&act.stdout, 32);
        assert_eq!(shown.len(), 1, "{label}: one password: {}", act.show());
        assert_eq!(
            calls[0].secret(),
            shown[0],
            "{label}: the password on stdin"
        );
        let log = hub.log_lines();
        assert_eq!(log.len(), 1, "{label}: {log:?}");
        assert_logged(&log[0], "reset", "mira", detail);
        assert!(hub.temps().is_empty(), "{label}: {:?}", hub.temps());
        assert!(!hub.lock().exists(), "{label}: the lock is gone");
    }
}

// ------------------------------------------------------------- AC10-AC11: reading the files

/// AC10: a users file with a tab, a flow mapping, a `groups:` key, six-space
/// indentation, a duplicate login, a display name holding U+0085, U+2028,
/// U+FFFE, U+FFFF (R8), a NUL or `\x01` (R12), or a lone `\xff`, a comment
/// holding a NUL (R12's "every C0 control byte", where the layout reader cannot
/// cover for the scan), or a leading U+FEFF (R1) is refused with exit 1 and
/// `hub-admin: users.yml:<line>: ...` naming its line, by `list` on a pipe and
/// by `add` on a terminal, which writes nothing and calls nothing; 11b-1's
/// guide example (a comment after the login, a plain email) is read. M:
/// accept any file; accept U+2028 in the users file; accept U+FFFE in the
/// users file; accept a NUL.
#[test]
fn ac10_a_users_file_in_no_known_layout_is_refused_at_its_line() {
    let digest = old_digest("mira");
    let mira = entry("mira", "Mira K", "mira@example.org", false);
    let named = |name: &[u8]| -> Vec<u8> {
        let mut text = b"users:\n  mira:\n    displayname: \"".to_vec();
        text.extend_from_slice(name);
        text.extend_from_slice(
            format!("\"\n    email: \"mira@example.org\"\n    password: \"{digest}\"\n").as_bytes(),
        );
        text
    };
    // R12 on any line: in a whole-line comment only the byte scan can see a NUL
    // (an awk may cut the line there and read the rest as a comment).
    let mut nul_comment = b"users:\n# a note\x00 by hand\n".to_vec();
    nul_comment.extend_from_slice(mira.as_bytes());
    let mut bom = "\u{feff}".as_bytes().to_vec();
    bom.extend_from_slice(users_file(&[&mira]).as_bytes());
    let cases: Vec<(&str, Vec<u8>, usize)> = vec![
        (
            "a tab",
            format!(
                "users:\n  mira:\n\tdisplayname: \"Mira K\"\n    email: \"mira@example.org\"\n    password: \"{digest}\"\n"
            )
            .into_bytes(),
            3,
        ),
        (
            "a flow mapping",
            format!(
                "users:\n  mira: {{displayname: \"Mira K\", email: \"mira@example.org\", password: \"{digest}\"}}\n"
            )
            .into_bytes(),
            2,
        ),
        (
            "a groups key",
            format!(
                "users:\n  mira:\n    displayname: \"Mira K\"\n    email: \"mira@example.org\"\n    password: \"{digest}\"\n    groups:\n      - admins\n"
            )
            .into_bytes(),
            6,
        ),
        (
            "six-space indentation",
            format!(
                "users:\n  mira:\n      displayname: \"Mira K\"\n    email: \"mira@example.org\"\n    password: \"{digest}\"\n"
            )
            .into_bytes(),
            3,
        ),
        (
            "a duplicate login",
            format!("users:\n{mira}{mira}").into_bytes(),
            7,
        ),
        ("a name holding U+0085", named("Mi\u{85}ra".as_bytes()), 3),
        ("a name holding U+2028", named("Mi\u{2028}ra".as_bytes()), 3),
        ("a name holding a lone 0xff", named(b"Mi\xffra"), 3),
        ("a leading U+FEFF", bom, 1),
        ("a name holding U+FFFE", named("Mi\u{fffe}ra".as_bytes()), 3),
        ("a name holding U+FFFF", named("Mi\u{ffff}ra".as_bytes()), 3),
        ("a name holding a NUL", named(b"Mi\x00ra"), 3),
        ("a name holding 0x01", named(b"Mi\x01ra"), 3),
        ("a comment holding a NUL", nul_comment, 2),
    ];
    for (label, text, line) in cases {
        let hub = Hub::new("ac10");
        hub.put_bytes(&hub.users(), &text);
        let opening = format!("{REFUSAL}users.yml:{line}: ");
        let list = hub.null(&["list"]);
        list.code(1);
        assert!(
            list.stderr.lines().any(|one| one.starts_with(&opening)),
            "{label}: `{opening}...`: {}",
            list.show()
        );
        let before = hub.files();
        let add = hub.terminal(&ADD_LENA, &[]);
        add.code(1);
        assert!(
            add.stderr.lines().any(|one| one.starts_with(&opening)),
            "{label}: `{opening}...`: {}",
            add.show()
        );
        assert_eq!(hub.files(), before, "{label}: nothing written");
        assert!(hub.calls().is_empty(), "{label}: nothing called");
    }

    let hub = Hub::new("ac10-guide");
    hub.put(&hub.users(), GUIDE_EXAMPLE);
    let list = hub.null(&["list"]);
    list.code(0);
    assert_eq!(
        list.lines(),
        ["mira\tenabled\t-\tmira@example.org\tMira K"],
        "11b-1's guide example reads"
    );
}

/// AC10, the MCP file (R5): blank lines are skipped, as Traefik does, so a
/// file with blank lines and a line of spaces alone is read; a line ` mira:<digest>`, `mira:<digest> ` or
/// one ending in `\r` is refused by `list` with exit 1, naming its line
/// (`...users:<line>: ...`), and `mcp rotate` on a terminal over it exits 1,
/// writing and calling nothing. M: refuse a blank MCP line; accept a trailing
/// space.
#[test]
fn ac10_the_mcp_file_skips_blank_lines_and_refuses_padded_ones() {
    let users = users_file(&[&entry("mira", "Mira K", "mira@example.org", false)]);
    let hub = Hub::new("ac10-mcp-blank");
    hub.put(&hub.users(), &users);
    hub.put(
        &hub.mcp(),
        &format!("\n{}   \n{}\n", mcp_line("mira", 'M'), mcp_line("zed", 'Z')),
    );
    let list = hub.null(&["list"]);
    list.code(0);
    assert_eq!(
        list.lines(),
        [
            "mira\tenabled\tmcp\tmira@example.org\tMira K",
            "orphan mcp line: zed (no account)",
        ],
        "blank MCP lines are read"
    );

    let olek = mcp_line("olek", 'O');
    let mira = mcp_line("mira", 'M');
    let mira = mira.trim_end();
    let cases = [
        ("a leading space", format!("{olek} {mira}\n")),
        ("a trailing space", format!("{olek}{mira} \n")),
        ("a trailing \\r", format!("{olek}{mira}\r\n")),
    ];
    for (label, text) in cases {
        let hub = Hub::new("ac10-mcp");
        hub.put(&hub.users(), &users);
        hub.put(&hub.mcp(), &text);
        let list = hub.null(&["list"]);
        list.code(1);
        assert!(
            list.stderr
                .lines()
                .any(|line| line.starts_with(REFUSAL) && line.contains("users:2: ")),
            "{label}: the refusal names line 2: {}",
            list.show()
        );
        let before = hub.files();
        let rotate = hub.terminal(&["mcp", "rotate", "mira"], &[]);
        rotate.refused(1);
        assert_eq!(hub.files(), before, "{label}: nothing written");
        assert!(hub.calls().is_empty(), "{label}: nothing called");
    }
}

/// AC10 (R9): the byte scan fails closed. With an `od` first on `PATH` that
/// fails for `-tu1` (printing nothing, or its whole scan first), or exits 0
/// printing nothing, `list` over a clean users
/// file exits 1 with a `hub-admin: ` line naming the scan (the word `od` or
/// `scan`),
/// and `add` on a terminal refuses its `--name` the same way, writing and
/// calling nothing. M: ignore `od`'s exit.
#[test]
fn ac10_the_byte_scan_fails_closed() {
    let scan = Regex::new(r"\bod\b|scan").expect("a pattern");
    for mode in ["fail", "fail-late", "silent"] {
        let hub = Hub::new("ac10-od");
        executable(
            &hub.bin.join("od"),
            &OD_STUB
                .replace("@MODE@", mode)
                .replace("@OD@", first_existing(&["/usr/bin/od", "/bin/od"], "od")),
        );
        hub.put(
            &hub.users(),
            &users_file(&[&entry("mira", "Mira K", "mira@example.org", false)]),
        );
        let names_the_scan = |act: &Act| {
            act.stderr
                .lines()
                .any(|line| line.starts_with(REFUSAL) && scan.is_match(line))
        };
        let list = hub.null(&["list"]);
        list.code(1);
        assert!(names_the_scan(&list), "{mode}: {}", list.show());
        let before = hub.files();
        let add = hub.terminal(&ADD_LENA, &[]);
        add.code(1);
        assert!(names_the_scan(&add), "{mode}: {}", add.show());
        assert_eq!(hub.files(), before, "{mode}: nothing written");
        assert!(hub.calls().is_empty(), "{mode}: nothing called");
    }
}

/// AC11: `list` prints the accounts in the users file's order, tab-separated
/// (`mira enabled mcp ...`, `lena enabled - ...`, `olek disabled ...`), then
/// one line per orphan MCP line, `orphan mcp line: zed (no account)` and
/// `orphan mcp line: olek (disabled)`, and exits 0. M: drop that check.
#[test]
fn ac11_list_names_every_orphan_mcp_line() {
    let hub = Hub::new("ac11");
    hub.put(
        &hub.users(),
        &users_file(&[
            &entry("mira", "Mira K", "mira@example.org", false),
            &entry("lena", "Lena", "lena@example.org", false),
            &entry("olek", "Olek", "olek@example.org", true),
        ]),
    );
    hub.put(
        &hub.mcp(),
        &format!(
            "{}{}{}",
            mcp_line("mira", 'M'),
            mcp_line("zed", 'Z'),
            mcp_line("olek", 'O')
        ),
    );
    let list = hub.null(&["list"]);
    list.code(0);
    let lines = list.lines();
    assert_eq!(lines.len(), 5, "{}", list.show());
    assert_eq!(lines[0], "mira\tenabled\tmcp\tmira@example.org\tMira K");
    assert_eq!(lines[1], "lena\tenabled\t-\tlena@example.org\tLena");
    assert!(
        lines[2].starts_with("olek\tdisabled\t") && lines[2].ends_with("\tolek@example.org\tOlek"),
        "{}",
        list.show()
    );
    let orphans: BTreeSet<&str> = lines[3..].iter().copied().collect();
    assert_eq!(
        orphans,
        BTreeSet::from([
            "orphan mcp line: olek (disabled)",
            "orphan mcp line: zed (no account)",
        ]),
        "{}",
        list.show()
    );
}

/// AC11, revoke (R5): `mcp revoke zed`, where `zed` holds a line and no
/// account, removes that line, the others byte for byte, writes the stamp,
/// prints `revoked zed`, logs `mcp-revoke zed -` and calls nothing; a login
/// with neither an account nor a line exits 2, writing nothing. M: refuse a
/// revoke with no account.
#[test]
fn ac11_mcp_revoke_takes_an_orphan_line() {
    let hub = Hub::new("ac11-revoke");
    hub.put(
        &hub.users(),
        &users_file(&[&entry("mira", "Mira K", "mira@example.org", false)]),
    );
    let mira = mcp_line("mira", 'M');
    hub.put(&hub.mcp(), &format!("{mira}{}", mcp_line("zed", 'Z')));
    let act = hub.terminal(&["mcp", "revoke", "zed"], &[]);
    act.code(0);
    assert!(act.says("revoked zed"), "{}", act.show());
    assert_eq!(read(&hub.mcp()), mira, "zed's line removed");
    assert_eq!(stamp_value(&hub.stamp()), 1);
    assert!(hub.calls().is_empty());
    let log = hub.log_lines();
    assert_eq!(log.len(), 1, "{log:?}");
    assert_logged(&log[0], "mcp-revoke", "zed", "-");

    let before = hub.files();
    hub.terminal(&["mcp", "revoke", "nobody"], &[]).refused(2);
    assert_eq!(hub.files(), before, "neither an account nor a line");
}

// ------------------------------------------------------------- AC12: check

/// The `[team]` of AC12: `mira` at another address than the door's, `olek`,
/// `zed`, and `"mi:ra"`.
const AC12_TEAM: &str = "
[team.mira]
role = \"owner\"
email = \"mira@team.example.org\"

[team.olek]
role = \"member\"
email = \"olek@example.org\"

[team.zed]
role = \"member\"
email = \"zed@example.org\"

[team.\"mi:ra\"]
role = \"member\"
email = \"mi.ra@example.org\"
";

/// AC12: the built `spec team --root <copy of fixtures/spec-a>`, its `[team]`
/// holding `mira` (another address than the door's), `olek` (disabled at the
/// door), `zed` (no account) and `"mi:ra"`, its stdout piped into `check`,
/// gives exactly `no account: mi:ra`, `holds ':': mi:ra`, `email differs:
/// mira: team ..., door ...`, `disabled: olek`, `no account: zed`, `check: 4
/// logins, 5 findings`, and exit 0. A stdin line that is not three
/// tab-separated fields exits 2, naming it. M: exit 1 on a finding; change
/// `spec team`'s separator.
#[test]
fn ac12_check_compares_spec_team_with_the_door() {
    let hub = Hub::new("ac12");
    hub.put(
        &hub.users(),
        &users_file(&[
            &entry("mira", "Mira K", "mira@example.org", false),
            &entry("olek", "Olek", "olek@example.org", true),
        ]),
    );
    let project = hub.scratch.copy("spec-a", "project");
    let config = project.join("specengine.toml");
    let mut text = read(&config);
    text.push_str(AC12_TEAM);
    fs::write(&config, text).expect("the project's [team]");
    let home = hub.scratch.home("spec");

    let team = common::spec(
        &home,
        hub.scratch.path(),
        &["team", "--root", text_of(&project)],
    );
    team.code(0);
    let act = hub.piped(&["check"], team.stdout.as_bytes());
    act.code(0);
    assert_eq!(
        act.lines(),
        [
            "no account: mi:ra",
            "holds ':': mi:ra",
            "email differs: mira: team mira@team.example.org, door mira@example.org",
            "disabled: olek",
            "no account: zed",
            "check: 4 logins, 5 findings",
        ],
        "{}\n--- spec team\n{}",
        act.show(),
        team.stdout
    );

    let bad = hub.piped(&["check"], b"mira mira@example.org\n");
    bad.refused(2);
    assert!(
        bad.stderr.contains("mira mira@example.org"),
        "the line named: {}",
        bad.show()
    );
}

// ------------------------------------------------------------- the rest of "Data"

/// "Data", `mcp revoke`: it removes the login's line, the others byte for
/// byte, bumps the stamp, prints `revoked mira`, logs `mcp-revoke mira -` and
/// calls nothing; with no line left it prints `unchanged mira` and writes
/// nothing.
#[test]
fn data_mcp_revoke_removes_the_line_and_bumps_the_stamp() {
    let hub = Hub::new("revoke");
    hub.put(
        &hub.users(),
        &users_file(&[&entry("mira", "Mira K", "mira@example.org", false)]),
    );
    let olek = mcp_line("olek", 'O');
    hub.put(&hub.mcp(), &format!("{}{olek}", mcp_line("mira", 'M')));
    hub.put(&hub.stamp(), &stamp_text(2));

    let act = hub.terminal(&["mcp", "revoke", "mira"], &[]);
    act.code(0);
    assert!(act.says("revoked mira"), "{}", act.show());
    assert_eq!(read(&hub.mcp()), olek);
    assert_eq!(stamp_value(&hub.stamp()), 3);
    assert!(hub.calls().is_empty());
    let log = hub.log_lines();
    assert_eq!(log.len(), 1, "{log:?}");
    assert_logged(&log[0], "mcp-revoke", "mira", "-");

    let before = hub.files();
    let again = hub.terminal(&["mcp", "revoke", "mira"], &[]);
    again.code(0);
    assert!(again.says("unchanged mira"), "{}", again.show());
    assert_eq!(hub.files(), before);
}

/// "Data", `code`: on a terminal it prints the notifier's file as it is and
/// logs `code - -`; with no file it prints `no notice` and exits 0.
#[test]
fn data_code_prints_the_notice_as_it_is() {
    let hub = Hub::new("code");
    let empty = hub.terminal(&["code"], &[]);
    empty.code(0);
    assert!(empty.says("no notice"), "{}", empty.show());

    let notice = "Date: 2026-11-04 09:30:00\nRecipient: mira\nThe one-time code: 482913\n";
    hub.put(&hub.notice(), notice);
    let act = hub.terminal(&["code"], &[]);
    act.code(0);
    assert_eq!(act.stdout, notice, "the notifier's file, as is");
    assert!(
        hub.log_lines()
            .iter()
            .any(|line| Regex::new(r"^\S+\tcode\t-\t-$")
                .expect("a pattern")
                .is_match(line)),
        "{:?}",
        hub.log_lines()
    );
    assert!(
        !read(&hub.log()).contains("482913"),
        "no code enters the log"
    );
}

/// "Exits": an unknown act, an unknown option, a missing argument and an
/// unknown login (any act but `add`) are a bad call, exit 2 with a
/// `hub-admin: ` line, nothing written, the door's CLI not called.
#[test]
fn data_a_bad_call_exits_2_and_touches_nothing() {
    let hub = Hub::new("bad-calls");
    hub.put(
        &hub.users(),
        &users_file(&[&entry("mira", "Mira K", "mira@example.org", false)]),
    );
    let before = hub.files();
    let calls: [&[&str]; 8] = [
        &["frobnicate"],
        &[],
        &["add", "lena", "--name", "Lena"],
        &[
            "add",
            "lena",
            "--name",
            "Lena",
            "--email",
            "lena@example.org",
            "--role",
            "lead",
        ],
        &["disable"],
        &["disable", "nobody"],
        &["reset", "nobody"],
        &["mcp", "rotate", "nobody"],
    ];
    for args in calls {
        let act = hub.terminal(args, &[]);
        act.refused(2);
        assert_eq!(hub.files(), before, "{}", act.label);
        assert!(hub.calls().is_empty(), "{}", act.label);
    }
}

// ------------------------------------------------------------- AC13-AC14: the host

fn compose() -> Value {
    yaml(&deploy().join("docker-compose.yml"))
}

fn service<'a>(compose: &'a Value, name: &str) -> &'a Value {
    compose
        .get("services")
        .and_then(|services| services.get(name))
        .unwrap_or_else(|| panic!("no service `{name}`"))
}

fn object<'a>(value: &'a Value, what: &str) -> &'a Map<String, Value> {
    value
        .as_object()
        .unwrap_or_else(|| panic!("{what} is no mapping: {value}"))
}

fn scalar(value: &Value, what: &str) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(flag) => flag.to_string(),
        other => panic!("{what} is no scalar: {other}"),
    }
}

/// A string, or a list of strings, as a list.
fn strings(value: &Value, what: &str) -> Vec<String> {
    match value {
        Value::String(one) => vec![one.clone()],
        Value::Array(items) => items.iter().map(|item| scalar(item, what)).collect(),
        other => panic!("{what} is neither a string nor a list: {other}"),
    }
}

/// `(source, target, "ro" or "")` of each of a service's volumes, short or
/// long syntax.
fn mounts(service: &Value, what: &str) -> Vec<(String, String, String)> {
    let Some(volumes) = service.get("volumes") else {
        return Vec::new();
    };
    volumes
        .as_array()
        .unwrap_or_else(|| panic!("{what}'s volumes are no list"))
        .iter()
        .map(|volume| match volume {
            Value::String(short) => {
                let parts: Vec<&str> = short.split(':').collect();
                match parts.as_slice() {
                    [source, target] => ((*source).to_owned(), (*target).to_owned(), String::new()),
                    [source, target, mode] => (
                        (*source).to_owned(),
                        (*target).to_owned(),
                        if mode.split(',').any(|flag| flag == "ro") {
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
                if long.get("read_only") == Some(&Value::Bool(true)) {
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

/// The secrets a service is granted.
fn grants(service: &Value, what: &str) -> BTreeSet<String> {
    let Some(secrets) = service.get("secrets") else {
        return BTreeSet::new();
    };
    secrets
        .as_array()
        .unwrap_or_else(|| panic!("{what}'s secrets are no list"))
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

/// Whether each of `paths` is ignored by a copy of `scripts/deploy/.gitignore`
/// in a scratch repository where each is a real, empty file.
fn ignored(paths: &[&str]) -> BTreeMap<String, bool> {
    let scratch = Scratch::new("admin-gitignore");
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

/// AC13: `admin` is as "Data" says: `network_mode: none`; `profiles:
/// [admin]`; no `ports`, `expose`, `networks`, `depends_on`, `restart` or
/// `user`; `image` byte for byte `authelia`'s; `logging` exactly `{driver:
/// none}` (R6); the entrypoint `/bin/sh
/// /opt/hub-admin/hub-admin`; the environment `HUB_HOST` and the storage key's
/// file; exactly the `authelia_storage_key` secret; exactly the six mounts,
/// `./admin` alone read-only. `authelia` mounts `./authelia/users` read-only.
/// `specengine` mounts none of `./admin`, `./admin-log`, `./authelia`,
/// `./traefik`. `.gitignore` ignores `admin-log/acts.log`, the lock and
/// `traefik/dynamic/mcp-stamp.yml`, and not `admin/hub-admin`, which is
/// executable. M: `network_mode: service:traefik`; add `ports`; drop
/// `profiles`; make the users folder writable in `authelia`; drop `logging`.
#[test]
fn ac13_the_admin_service_is_one_shot_with_no_network() {
    let compose = compose();
    let admin = service(&compose, "admin");
    assert_eq!(
        admin.get("network_mode").and_then(Value::as_str),
        Some("none"),
        "admin's network_mode"
    );
    assert_eq!(
        admin
            .get("profiles")
            .map(|profiles| strings(profiles, "admin's profiles")),
        Some(vec!["admin".to_owned()]),
        "`up -d` never starts admin"
    );
    for key in [
        "ports",
        "expose",
        "networks",
        "depends_on",
        "restart",
        "user",
    ] {
        assert!(admin.get(key).is_none(), "admin has `{key}`");
    }
    assert_eq!(
        admin.get("logging"),
        Some(&serde_json::json!({"driver": "none"})),
        "no password, secret or code reaches a log driver (R6)"
    );
    assert_eq!(
        admin.get("image").and_then(Value::as_str),
        service(&compose, "authelia")
            .get("image")
            .and_then(Value::as_str),
        "admin's image is authelia's, byte for byte"
    );
    assert_eq!(
        admin
            .get("entrypoint")
            .map(|entrypoint| strings(entrypoint, "admin's entrypoint")),
        Some(vec![
            "/bin/sh".to_owned(),
            "/opt/hub-admin/hub-admin".to_owned()
        ])
    );
    assert_eq!(
        environment(admin, "admin"),
        BTreeMap::from([
            ("HUB_HOST".to_owned(), "${HUB_HOST}".to_owned()),
            (
                "AUTHELIA_STORAGE_ENCRYPTION_KEY_FILE".to_owned(),
                "/run/secrets/authelia_storage_key".to_owned()
            ),
        ])
    );
    assert_eq!(
        grants(admin, "admin"),
        BTreeSet::from(["authelia_storage_key".to_owned()])
    );
    let mounted = mounts(admin, "admin");
    assert_eq!(mounted.len(), 6, "{mounted:?}");
    let want: BTreeSet<(String, String, String)> = [
        ("./admin", "/opt/hub-admin", "ro"),
        ("./admin-log", "/var/lib/hub-admin", ""),
        ("./authelia/users", "/users", ""),
        ("./traefik/mcp-users", "/mcp-users", ""),
        ("./traefik/dynamic", "/traefik-dynamic", ""),
        ("authelia-data", "/var/lib/authelia", ""),
    ]
    .into_iter()
    .map(|(source, target, ro)| (source.to_owned(), target.to_owned(), ro.to_owned()))
    .collect();
    assert_eq!(
        mounted.into_iter().collect::<BTreeSet<_>>(),
        want,
        "admin's six mounts"
    );

    let users: Vec<(String, String, String)> = mounts(service(&compose, "authelia"), "authelia")
        .into_iter()
        .filter(|(source, _, _)| source == "./authelia/users")
        .collect();
    assert_eq!(
        users,
        [(
            "./authelia/users".to_owned(),
            "/users".to_owned(),
            "ro".to_owned()
        )],
        "authelia reads the users folder read-only (Q4)"
    );
    for (source, _, _) in mounts(service(&compose, "specengine"), "specengine") {
        for private in ["./admin", "./authelia", "./traefik"] {
            assert!(!source.starts_with(private), "specengine mounts {source}");
        }
    }

    let found = ignored(&[
        "admin-log/acts.log",
        "admin-log/lock/taken",
        "traefik/dynamic/mcp-stamp.yml",
        "admin/hub-admin",
    ]);
    assert_eq!(
        found,
        BTreeMap::from([
            ("admin-log/acts.log".to_owned(), true),
            ("admin-log/lock/taken".to_owned(), true),
            ("traefik/dynamic/mcp-stamp.yml".to_owned(), true),
            ("admin/hub-admin".to_owned(), false),
        ])
    );
    assert_eq!(
        mode(&hub_admin()) & 0o111,
        0o111,
        "admin/hub-admin is committed 0755"
    );
}

/// AC14: the door's `authentication_backend.refresh_interval` is `always` and
/// `password_reset.disable` is true. M: drop the key; set `5m`.
#[test]
fn ac14_the_door_reads_a_person_on_every_request() {
    let door = yaml(&deploy().join("authelia/config/configuration.yml"));
    let backend = object(
        door.get("authentication_backend")
            .expect("authentication_backend"),
        "authentication_backend",
    );
    assert_eq!(
        backend.get("refresh_interval").and_then(Value::as_str),
        Some("always"),
        "refresh_interval: {door}"
    );
    assert_eq!(
        door.pointer("/authentication_backend/password_reset/disable"),
        Some(&Value::Bool(true)),
        "password reset stays off (S8)"
    );
}

// ------------------------------------------------------------- AC16: no account code

/// AC16: no file under `crates/*/src` names, in any case, `users.yml`,
/// `mcp-users`, `authelia`, `htpasswd`, `argon2` or `bcrypt`; `Cargo.lock`
/// holds no package `argon2`, `bcrypt` or `password-hash`. M: add a
/// `"users.yml"` constant to the CLI.
#[test]
fn ac16_no_crate_holds_account_code() {
    let words = [
        "users.yml",
        "mcp-users",
        "authelia",
        "htpasswd",
        "argon2",
        "bcrypt",
    ];
    let mut found = Vec::new();
    let mut scanned = 0;
    for krate in fs::read_dir(repository().join("crates"))
        .expect("crates/")
        .flatten()
    {
        let mut stack = vec![krate.path().join("src")];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                scanned += 1;
                let text = String::from_utf8_lossy(&bytes(&path)).to_lowercase();
                for word in words {
                    if text.contains(word) {
                        found.push(format!("{}: {word}", path.display()));
                    }
                }
            }
        }
    }
    assert!(scanned > 100, "the sources were read: {scanned} files");
    assert!(found.is_empty(), "account words in a crate: {found:#?}");

    let lock: toml::Table =
        toml::from_str(&read(&repository().join("Cargo.lock"))).expect("Cargo.lock is TOML");
    let packages: BTreeSet<&str> = lock["package"]
        .as_array()
        .expect("packages")
        .iter()
        .filter_map(|package| package.get("name").and_then(toml::Value::as_str))
        .collect();
    assert!(packages.contains("serde"), "the lock was read");
    for name in ["argon2", "bcrypt", "password-hash"] {
        assert!(!packages.contains(name), "Cargo.lock holds {name}");
    }
}

// ------------------------------------------------------------- AC18: CI and the guide

fn workflow() -> Value {
    yaml(&repository().join(".github/workflows/image.yml"))
}

/// The `run` texts of `image.yml`'s `check` job, in order.
fn check_runs(workflow: &Value) -> Vec<String> {
    workflow
        .pointer("/jobs/check/steps")
        .and_then(Value::as_array)
        .expect("the check job's steps")
        .iter()
        .filter_map(|step| step.get("run").and_then(Value::as_str))
        .map(str::to_owned)
        .collect()
}

/// The `deploy_admin` test line, as AC18 words it.
const ADMIN_TESTS: &str = "cargo test --locked -p specengine-http --test deploy_admin";

/// AC18: `image.yml`'s check job runs `cargo test --locked -p
/// specengine-http --test deploy_admin`; the guide fits Tier 1 (10 240 B).
/// M: drop the file from `image.yml`.
#[test]
fn ac18_the_image_workflow_runs_these_tests_and_the_guide_fits() {
    let runs = check_runs(&workflow());
    let lines: Vec<&str> = runs
        .iter()
        .flat_map(|run| run.lines())
        .map(str::trim)
        .collect();
    assert!(
        lines.contains(&ADMIN_TESTS),
        "the check job runs `{ADMIN_TESTS}`: {runs:#?}"
    );
    let guide = fs::metadata(deploy().join("README.md"))
        .expect("the guide")
        .len();
    assert!(guide <= 10_240, "scripts/deploy/README.md is {guide} B");
}

/// AC18, the `check` job as a runner reads it: AC12 runs the built `spec`,
/// which `cargo test -p specengine-http` does not build, so an earlier cargo
/// line of the job builds `specengine-cli` (by name, `--bin spec` or the
/// whole workspace) before `deploy_admin` runs. Without it AC12 fails on CI.
#[test]
fn ac18_the_check_job_builds_spec_before_deploy_admin() {
    let runs = check_runs(&workflow());
    let lines: Vec<&str> = runs
        .iter()
        .flat_map(|run| run.lines())
        .map(str::trim)
        .collect();
    let at = lines
        .iter()
        .position(|line| *line == ADMIN_TESTS)
        .unwrap_or_else(|| panic!("no `{ADMIN_TESTS}`: {runs:#?}"));
    let builds_spec = lines[..at].iter().any(|line| {
        line.starts_with("cargo ")
            && (line.contains("specengine-cli")
                || line.contains("--bin spec")
                || line.contains("--workspace"))
    });
    assert!(
        builds_spec,
        "no cargo line before `{ADMIN_TESTS}` builds `spec`: {runs:#?}"
    );
}

/// The stub `docker` of the C3 step: `config --images` answers one image per
/// compose service, `admin` repeating `authelia`'s; `run` keeps each argument
/// in its own file.
const DOCKER_STUB: &str = r#"#!/bin/sh
dir='@DIR@'
case "$1" in
    compose)
        for argument in "$@"; do
            if [ "$argument" = --images ]; then
                cat "$dir/images"
                exit 0
            fi
        done
        exit 0
        ;;
    run)
        i=0
        for argument in "$@"; do
            i=$((i + 1))
            printf '%s' "$argument" > "$dir/run.$i"
        done
        printf '%s\n' "$i" > "$dir/run.count"
        exit 0
        ;;
esac
exit 0
"#;

/// AC18: the C3 step ("Authelia accepts its configuration") takes one image
/// line: run as a runner runs it (`bash -e`), its stub `docker`'s `config
/// --images` printing `authelia`'s image twice (two services on it), the image
/// it hands `docker run` is that image, one line.
#[test]
fn ac18_the_c3_step_takes_one_image_line() {
    let compose = compose();
    let images: Vec<String> = object(compose.get("services").expect("services"), "services")
        .iter()
        .map(|(name, service)| {
            scalar(
                service
                    .get("image")
                    .unwrap_or_else(|| panic!("{name} has no image")),
                name,
            )
        })
        .collect();
    let authelia = scalar(
        service(&compose, "authelia")
            .get("image")
            .expect("an image"),
        "authelia's image",
    );
    assert_eq!(
        images.iter().filter(|image| **image == authelia).count(),
        2,
        "two services on authelia's image: {images:?}"
    );

    let runs = check_runs(&workflow());
    let step = runs
        .iter()
        .find(|run| run.contains("validate-config"))
        .expect("the C3 step");
    let scratch = Scratch::new("admin-c3");
    let bin = scratch.dir("bin");
    let records = scratch.dir("docker");
    let temp = scratch.dir("tmp");
    fs::write(
        records.join("images"),
        images
            .iter()
            .map(|image| format!("{image}\n"))
            .collect::<String>(),
    )
    .expect("the images");
    executable(
        &bin.join("docker"),
        &DOCKER_STUB.replace("@DIR@", text_of(&records)),
    );
    let script = scratch.path().join("c3.sh");
    fs::write(&script, step).expect("the step");
    let mut path = OsString::from(&bin);
    path.push(":/usr/bin:/bin");
    let output = Command::new("/bin/bash")
        .env_clear()
        .env("PATH", path)
        .env("HOME", scratch.home("runner"))
        .env("TMPDIR", &temp)
        .current_dir(scratch.dir("work"))
        .args(["--noprofile", "--norc", "-e"])
        .arg(&script)
        .stdin(Stdio::null())
        .output()
        .expect("bash");
    assert!(
        output.status.success(),
        "the C3 step: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let count: usize = read(&records.join("run.count"))
        .trim()
        .parse()
        .expect("docker run's argument count");
    let args: Vec<String> = (1..=count)
        .map(|n| read(&records.join(format!("run.{n}"))))
        .collect();
    let at = args
        .iter()
        .position(|arg| arg == "validate-config")
        .unwrap_or_else(|| panic!("docker run ... validate-config: {args:?}"));
    assert!(at >= 2, "{args:?}");
    assert_eq!(args[at - 1], "authelia", "{args:?}");
    assert_eq!(
        args[at - 2],
        authelia,
        "docker run takes authelia's image, one line"
    );
}
