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
//!   the shapes "The door's CLI" records (from memory until VF1), a bare exit
//!   0 for `storage user webauthn|totp delete`. `crypto hash generate
//!   argon2|bcrypt` answers as the pinned 4.39.28 was seen to on the owner's
//!   VPS: no password from a pipe (exit 1, its "failed to read the password
//!   from the terminal" and usage), and with `--random` the lines `Random
//!   Password: <value>` (of `--random.characters`, `--random.length` long)
//!   and `Digest: <d>`. A control file lists the calls that exit 1
//!   (`fail-at`; a delete then answers a recorded "nothing to delete" line),
//!   answer a digest off its shape (`bad-at`) or wait for a release
//!   (`hold-at`); `password.<n>` replaces call n's `Random Password:` line;
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
//!
//! docs/features/hub-ops.md (RM-0074, Phase 7 step 11b-2) OP11-OP15, the door's
//! half of the dumps: `hub-admin dump` and `restore`, and `hub-ops door` with the
//! real `hub-admin`. The stub `authelia` gains the shapes "`hub-admin`: `dump`
//! and `restore`" records (from memory until VF4): `storage user webauthn|totp
//! export --file F` writes `# the stub <kind> export, call <n>` to `F`, `storage
//! migrate up --sqlite.path D` makes `D`, and the two imports exit 0; a `watch`
//! control file names a path whose bytes each call keeps (OP14's order). Its
//! hold loop sleeps by `/bin/sleep`, so a stub `sleep` never sees it. `hub-ops`
//! runs as `dump-door` mounts it: the scratch root is both `HUB_ADMIN_ROOT` and
//! `HUB_OPS_ROOT`, `opt/hub-admin/hub-admin` a shim that counts each try and
//! execs the real script, with `tests/deploy_ops.rs`'s stub `date` and `sleep`
//! first on `PATH`, no `HUB_HOST` (compose gives `dump-door` none), and under
//! `/bin/sh` and `/bin/dash` (OP22). OP23 rewrites AC8 for the lock's new
//! message.

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

/// The pinned door's refusal of a hash with neither `--password` nor
/// `--random` and stdin no terminal, its first stderr line (verified on
/// `authelia/authelia:4.39.28`).
const NO_TERMINAL: &str = "Error: failed to read the password from the terminal: you must either use an interactive terminal or use the --password flag";

/// The characters `hub-admin` asks the door's random password of
/// (`--random.characters`).
const HEX: &str = "0123456789abcdef";

/// The stub of the door's CLI; `@DOOR@` is its records' folder, `@BCRYPT@`
/// [`BCRYPT_FILL`], `@PASSKEYS@` and `@TOTP@` the delete answers,
/// `@NO_TERMINAL@` [`NO_TERMINAL`].
const DOOR_STUB: &str = r#"#!/bin/sh
# The door's CLI as docs/features/hub-accounts.md "The door's CLI" records it
# (from memory until VF1). `crypto hash generate argon2|bcrypt` answers as the
# pinned authelia 4.39.28 does: with neither `--password` nor `--random` and
# stdin no terminal it refuses with its usage and exits 1, and never reads a
# password from stdin; `--random` prints `Random Password: <value>` and
# `Digest: <d>`. Each call's arguments and stdin are kept under the records'
# folder, and a random password it printed in `random.<n>`; `fail-at`,
# `bad-at` and `hold-at` there list the calls that exit 1, answer a digest
# off its shape, or wait for `release`; a file `password.<n>` is printed in
# place of call n's `Random Password:` line.
door='@DOOR@'
listed() {
    case " $(cat "$door/$1" 2>/dev/null) " in
        *" $n "*) return 0 ;;
    esac
    return 1
}
# `crypto hash generate <kind> [flags]`, as authelia 4.39.28 parses its flags
# (`--flag value` or `--flag=value`). The defaults (alphanumeric, 72
# characters, cost 12) are assumed, not seen: a script that drops
# `--random.characters`, `--random.length` or `--cost` gets a value or a
# digest off the shape it asked for.
crypto_hash() {
    kind=$4
    shift 4
    random=no
    password=no
    characters=
    charset=alphanumeric
    length=72
    cost=12
    expect=
    for argument in "$@"; do
        if [ -n "$expect" ]; then
            case $expect in
                --password) password=yes ;;
                --random.characters) characters=$argument ;;
                --random.charset) charset=$argument ;;
                --random.length) length=$argument ;;
                --cost) cost=$argument ;;
            esac
            expect=
            continue
        fi
        case $kind:$argument in
            *:--random | *:--random=true) random=yes ;;
            *:--random=false) random=no ;;
            *:--password=*) password=yes ;;
            *:--random.characters=*) characters=${argument#*=} ;;
            *:--random.charset=*) charset=${argument#*=} ;;
            *:--random.length=*) length=${argument#*=} ;;
            bcrypt:--cost=*) cost=${argument#*=} ;;
            *:--password | *:--random.characters | *:--random.charset | *:--random.length | bcrypt:--cost)
                expect=$argument
                ;;
            *)
                echo "Error: unknown flag: $argument" >&2
                exit 1
                ;;
        esac
    done
    if [ -n "$expect" ]; then
        echo "Error: flag needs an argument: $expect" >&2
        exit 1
    fi
    if [ "$random" = no ] && [ "$password" = no ]; then
        if [ -f "$door/tty.$n" ]; then
            echo "Error: the stub does not answer the door's interactive prompt" >&2
            exit 1
        fi
        echo '@NO_TERMINAL@' >&2
        printf 'Usage:\n  authelia crypto hash generate %s [flags]\n\n' "$kind" >&2
        printf 'Flags:\n  -h, --help   help for %s\n      --password string   manually supply the password\n      --random   uses a randomly generated password\n\n' "$kind" >&2
        printf 'Global Flags:\n  -c, --config strings   configuration files or directories to load\n' >&2
        exit 1
    fi
    if [ "$random" = yes ] && [ "$password" = yes ]; then
        echo "Error: the stub holds no recording for --password with --random" >&2
        exit 1
    fi
    case $length:$cost in
        :* | *[!0-9:]* | 0:* | *:)
            echo "Error: the stub holds no recording for --random.length $length --cost $cost" >&2
            exit 1
            ;;
    esac
    if [ -z "$characters" ]; then
        case $charset in
            alphanumeric) characters=ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789 ;;
            *)
                echo "Error: the stub holds no recording for --random.charset $charset" >&2
                exit 1
                ;;
        esac
    fi
    if [ "$random" = yes ]; then
        value=$(awk -v c="$characters" -v l="$length" -v n="$n" 'BEGIN {
            k = length(c); s = ""
            for (i = 0; i < l; i++) s = s substr(c, (n * 5 + i * 3) % k + 1, 1)
            print s
        }')
        printf '%s\n' "$value" > "$door/random.$n"
        if [ -f "$door/password.$n" ]; then
            cat "$door/password.$n"
        else
            printf 'Random Password: %s\n' "$value"
        fi
    fi
    case $kind in
        argon2)
            if [ "$bad" = yes ]; then
                echo 'Digest: $argon2id$v=19$m=65536,t=3,p=4$c2FsdA'
            else
                printf 'Digest: $argon2id$v=19$m=65536,t=3,p=4$c2FsdA%s$aGFzaA%s\n' "$n" "$n"
            fi
            ;;
        bcrypt)
            if [ "$bad" = yes ]; then
                echo 'Digest: $2b$10$short'
            else
                printf 'Digest: $2b$%02d$@BCRYPT@%03d\n' "$cost" "$n"
            fi
            ;;
    esac
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
if [ -f "$door/watch" ]; then
    watched=$(cat "$door/watch")
    if [ -e "$watched" ]; then cat "$watched" > "$door/watch.$n"; else : > "$door/unwatched.$n"; fi
fi
if listed hold-at; then
    : > "$door/held"
    waited=0
    while [ ! -e "$door/release" ] && [ "$waited" -lt 600 ]; do
        /bin/sleep 0.1
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
    "crypto hash generate argon2" | "crypto hash generate bcrypt")
        crypto_hash "$@"
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
    "storage user webauthn export" | "storage user totp export")
        if [ "$fail" = yes ]; then
            echo "Error: the stub fails call $n" >&2
            exit 1
        fi
        file=
        previous=
        for argument in "$@"; do
            if [ "$previous" = --file ]; then file=$argument; fi
            previous=$argument
        done
        if [ -z "$file" ]; then
            echo "Error: the stub's export was given no --file" >&2
            exit 1
        fi
        printf '# the stub %s export, call %s\n' "$3" "$n" > "$file" || exit 1
        ;;
    "storage migrate up"*)
        if [ "$fail" = yes ]; then
            echo "Error: the stub fails call $n" >&2
            exit 1
        fi
        db=
        previous=
        for argument in "$@"; do
            if [ "$previous" = --sqlite.path ]; then db=$argument; fi
            previous=$argument
        done
        if [ -n "$db" ]; then : > "$db"; fi
        ;;
    "storage user webauthn import" | "storage user totp import")
        if [ "$fail" = yes ]; then
            echo "Error: the stub fails call $n" >&2
            exit 1
        fi
        ;;
    *)
        echo "Error: the stub holds no recording for: $*" >&2
        exit 1
        ;;
esac
"#;

/// The door image's `timeout BOUND CMD...`, which macOS lacks
/// (docs/features/hub-ops.md "`hub-admin`: `dump` and `restore`", iteration
/// 2's bound on each export): it keeps the bound and the command, then runs
/// it, or, when `timeout-exit` holds a status, exits with it at once, as a
/// bound that passed does (124, or 143 for the command killed). `@DOOR@` is
/// the stub's records' folder.
const TIMEOUT_SHIM: &str = r#"#!/bin/sh
printf '%s %s\n' "$1" "$2" >> '@DOOR@/timeout.log'
code=$(cat '@DOOR@/timeout-exit' 2>/dev/null)
if [ -n "$code" ]; then exit "$code"; fi
shift
exec "$@"
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
    /// The random password the stub generated (`Random Password: `), if any.
    random: Option<String>,
}

impl Call {
    fn starts(&self, words: &[&str]) -> bool {
        self.args.len() >= words.len() && self.args[..words.len()] == *words
    }

    /// The value of the flag `name`, given as `name value` or `name=value`.
    fn flag(&self, name: &str) -> Option<&str> {
        let joined = format!("{name}=");
        self.args.iter().enumerate().find_map(|(at, arg)| {
            if arg == name {
                self.args.get(at + 1).map(String::as_str)
            } else {
                arg.strip_prefix(&joined)
            }
        })
    }

    /// Whether any argument is `--password`, in either form.
    fn gives_password(&self) -> bool {
        self.args
            .iter()
            .any(|arg| arg == "--password" || arg.starts_with("--password="))
    }

    /// The door's random password the stub printed, which `hub-admin` must
    /// take as the account's password or MCP secret.
    fn generated(&self) -> &str {
        self.random
            .as_deref()
            .unwrap_or_else(|| panic!("the stub generated no random password: {self:?}"))
    }

    /// The hash of `kind` as `hub-admin` must ask it (the pinned door takes
    /// no password on stdin): `crypto hash generate <kind> --random` with
    /// `--random.characters` [`HEX`] and `--random.length` `length`, no
    /// `--password`, nothing on stdin, the door's password in no argument.
    fn assert_random_hash(&self, kind: &str, length: usize, label: &str) {
        assert!(
            self.starts(&["crypto", "hash", "generate", kind]),
            "{label}: the {kind} hash: {self:?}"
        );
        assert!(
            self.names("--random") || self.names("--random=true"),
            "{label}: the door generates the password (--random): {self:?}"
        );
        assert_eq!(
            self.flag("--random.characters"),
            Some(HEX),
            "{label}: lower-case hex: {self:?}"
        );
        assert_eq!(
            self.flag("--random.length"),
            Some(length.to_string().as_str()),
            "{label}: {length} characters: {self:?}"
        );
        assert!(
            !self.gives_password(),
            "{label}: no --password in argv: {self:?}"
        );
        assert_eq!(self.stdin, "", "{label}: nothing on the hash's stdin");
        assert!(
            !self.holds(self.generated()),
            "{label}: the password is in no argument: {self:?}"
        );
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
                .replace("@TOTP@", TOTP_ANSWER)
                .replace("@NO_TERMINAL@", NO_TERMINAL),
        );
        executable(
            &bin.join("base64"),
            &BASE64_SHIM.replace(
                "@BASE64@",
                first_existing(&["/usr/bin/base64", "/bin/base64"], "base64"),
            ),
        );
        executable(
            &bin.join("timeout"),
            &TIMEOUT_SHIM.replace("@DOOR@", text_of(&door)),
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

    fn db(&self) -> PathBuf {
        self.root.join("var/lib/authelia/db.sqlite3")
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

    /// The stub prints `text` in place of call `call`'s `Random Password:`
    /// line (empty: no such line).
    fn password_line(&self, call: usize, text: &str) {
        fs::write(self.door.join(format!("password.{call}")), text).expect("a control file");
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
                random: fs::read_to_string(self.door.join(format!("random.{n}")))
                    .ok()
                    .map(|text| text.trim_end_matches('\n').to_owned()),
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

/// The lock's message for [`Hub::hold_lock`]'s lock (docs/features/hub-ops.md
/// "`hub-admin`: `dump` and `restore`").
const HELD_LOCK: &str = "another act or a dump holds admin-log/lock (taken 2026-11-04T09:00:00Z); if none runs, remove it";

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
/// password, 32 hex, is the door's own (`crypto hash generate argon2
/// --random`, its `Random Password:` line, as the pinned 4.39.28 takes no
/// password on stdin), printed once; the hash is given no `--password`,
/// nothing on stdin and the password in none of its arguments, and no file
/// under the root holds it afterwards. The act log, mode 600, gains one `add
/// mira -` line holding no password or digest. stdout is "`add`'s output",
/// the `[team]` row included, and the lock is gone. M: keep the prefix; write
/// in place; drop the log line; pipe the password to the hash.
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
    calls[0].assert_random_hash("argon2", 32, "add");
    assert!(!calls[0].on_terminal, "the hash never reads the terminal");
    assert_eq!(
        calls[0].generated(),
        password,
        "the shown password is the door's `Random Password:`"
    );
    let holding: Vec<String> = hub
        .files()
        .into_iter()
        .filter(|(_, content)| {
            content
                .as_deref()
                .is_some_and(|data| String::from_utf8_lossy(data).contains(&password))
        })
        .map(|(path, _)| path)
        .collect();
    assert!(
        holding.is_empty(),
        "no file keeps the password: {holding:?}"
    );

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
/// hash (`--random`, the printed password its `Random Password:`, no
/// `--password`, nothing on stdin), the WebAuthn delete with `--all`,
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
    calls[0].assert_random_hash("argon2", 32, "reset");
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
    assert_eq!(
        calls[0].generated(),
        shown[0],
        "the shown password is the door's `Random Password:`"
    );
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
/// `mira:` line, the stub's bcrypt digest at `--cost 10`, and the others byte
/// for byte in order. The secret is the door's own (`crypto hash generate
/// bcrypt --cost 10 --random`, 64 hex, its `Random Password:` line); the hash
/// is given no `--password`, nothing on stdin and the secret in none of its
/// arguments. The stamp goes to 1, and the act log gains `mcp-rotate mira -`,
/// no secret or digest. `rotate` of the disabled `zed` exits 2, writing and
/// calling nothing. M: append a line; pass the secret as an argument; base64
/// without removing newlines; pipe the secret to the hash.
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
    calls[0].assert_random_hash("bcrypt", 64, "mcp rotate");
    assert_eq!(calls[0].flag("--cost"), Some("10"), "{:?}", calls[0]);
    assert!(!calls[0].on_terminal, "the hash never reads the terminal");
    assert_eq!(
        calls[0].generated(),
        secret,
        "the shown secret is the door's `Random Password:`"
    );

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
    calls[0].assert_random_hash("bcrypt", 64, "mcp rotate, no stamp");
    assert_eq!(
        calls[0].generated(),
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
/// `mcp rotate` and `mcp revoke` on a terminal exit 1 naming it, write nothing
/// and call nothing, and the holder's `.mcp-stamp.tmp` survives each refused
/// run (R4). As docs/features/hub-ops.md "Acceptance criteria" OP23 rewrites
/// it, the line is "Data"'s new lock message, the holder's `taken` in it:
/// `another act or a dump holds admin-log/lock (taken <time>); if none runs,
/// remove it`. M: no lock check; remove temp files without holding the lock;
/// the old message.
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
            act.says(HELD_LOCK),
            "the lock's message `{HELD_LOCK}`: {}",
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

// ------------------------------------------------------------- the door's random password

/// The stub answers `crypto hash generate` as the pinned
/// `authelia/authelia:4.39.28` was seen to: with neither `--password` nor
/// `--random` and stdin no terminal (`/dev/null`, or a pipe holding a
/// password), it exits 1 printing [`NO_TERMINAL`] and then its usage on
/// stderr, nothing on stdout; `argon2 --random --random.characters <hex>
/// --random.length 32` prints exactly `Random Password: <32 hex>` and
/// `Digest: $argon2id$v=19$m=65536,t=3,p=4$<salt>$<hash>`; `bcrypt --cost 10
/// --random ... --random.length 64` exactly `Random Password: <64 hex>` and
/// `Digest: $2b$10$<53>`. A stand-in that took a piped password would let
/// `hub-admin` pass here and fail on the VPS, as it did.
#[test]
fn door_stub_answers_crypto_hash_as_the_pinned_cli() {
    let hub = Hub::new("door-stub");
    let door = hub.bin.join("authelia");
    let run = |args: &[&str], stdin: Option<&str>| -> (i32, String, String) {
        let mut child = Command::new(&door)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .args(args)
            .stdin(if stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the stub runs");
        if let Some(text) = stdin {
            let mut pipe = child.stdin.take().expect("its stdin");
            let _ = pipe.write_all(text.as_bytes());
        }
        let output = child.wait_with_output().expect("the stub ends");
        (
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    };

    let argon2: &[&str] = &["crypto", "hash", "generate", "argon2"];
    let bcrypt: &[&str] = &["crypto", "hash", "generate", "bcrypt", "--cost", "10"];
    for (args, stdin) in [
        (argon2, None),
        (argon2, Some("0123456789abcdef0123456789abcdef\n")),
        (bcrypt, None),
        (bcrypt, Some("0123456789abcdef0123456789abcdef\n")),
    ] {
        let (code, stdout, stderr) = run(args, stdin);
        let label = format!("{args:?}, stdin {stdin:?}");
        assert_eq!(code, 1, "{label}: refused: {stderr}");
        assert_eq!(stdout, "", "{label}: nothing on stdout");
        assert_eq!(
            stderr.lines().next(),
            Some(NO_TERMINAL),
            "{label}: the door's own words first: {stderr}"
        );
        assert!(
            stderr.lines().any(|line| line == "Usage:"),
            "{label}: then its usage: {stderr}"
        );
    }

    let (code, stdout, stderr) = run(
        &[
            "crypto",
            "hash",
            "generate",
            "argon2",
            "--random",
            "--random.characters",
            HEX,
            "--random.length",
            "32",
        ],
        None,
    );
    assert_eq!(code, 0, "argon2 --random: {stderr}");
    let shape = Regex::new(
        r"^Random Password: [0-9a-f]{32}\nDigest: \$argon2id\$v=19\$m=65536,t=3,p=4\$[A-Za-z0-9+/]+\$[A-Za-z0-9+/]+\n$",
    )
    .expect("a pattern");
    assert!(shape.is_match(&stdout), "argon2 --random: {stdout:?}");

    let (code, stdout, stderr) = run(
        &[
            "crypto",
            "hash",
            "generate",
            "bcrypt",
            "--cost",
            "10",
            "--random",
            "--random.characters",
            HEX,
            "--random.length",
            "64",
        ],
        None,
    );
    assert_eq!(code, 0, "bcrypt --random: {stderr}");
    let shape =
        Regex::new(r"^Random Password: [0-9a-f]{64}\nDigest: \$2b\$10\$[./A-Za-z0-9]{53}\n$")
            .expect("a pattern");
    assert!(shape.is_match(&stdout), "bcrypt --random: {stdout:?}");
}

/// The hotfix's refusal: with the stub's `Random Password:` line missing,
/// doubled, or off its shape (empty, a hex short or long by one, upper-case,
/// a non-hex character, a `Random password:` spelling), `add`, `reset` and
/// `mcp rotate` exit 1 with a `hub-admin: ` line saying `nothing changed`,
/// show no password or secret, make no call after the hash (so no delete),
/// keep the users file, the MCP file, the stamp and `hub.yml` byte for byte,
/// log nothing, and leave no temp file and no lock. M: take the line
/// unchecked; accept any length; accept upper-case.
#[test]
fn a_missing_or_off_shape_random_password_changes_nothing() {
    let reset: &[&str] = &["reset", "mira"];
    let rotate: &[&str] = &["mcp", "rotate", "mira"];
    let shapes = |width: usize| -> Vec<(String, String)> {
        let good = "0123456789abcdef".repeat(4)[..width].to_owned();
        vec![
            ("no line".to_owned(), String::new()),
            (
                "an empty value".to_owned(),
                "Random Password: \n".to_owned(),
            ),
            (
                "one short".to_owned(),
                format!("Random Password: {}\n", &good[1..]),
            ),
            ("one long".to_owned(), format!("Random Password: {good}0\n")),
            (
                "upper-case".to_owned(),
                format!("Random Password: {}\n", good.to_uppercase()),
            ),
            (
                "a non-hex character".to_owned(),
                format!("Random Password: g{}\n", &good[1..]),
            ),
            (
                "another spelling".to_owned(),
                format!("Random password: {good}\n"),
            ),
            (
                "two lines".to_owned(),
                format!("Random Password: {good}\nRandom Password: {good}\n"),
            ),
        ]
    };
    let mut cases: Vec<(&[&str], String, String)> = Vec::new();
    for (args, width) in [(&ADD_LENA[..], 32), (reset, 32), (rotate, 64)] {
        for (shape, line) in shapes(width) {
            cases.push((args, shape, line));
        }
    }
    for (args, shape, line) in cases {
        let label = format!("{args:?}, {shape}");
        let hub = Hub::new("random-shape");
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
        hub.password_line(1, &line);

        let act = hub.terminal(args, &[]);
        assert_eq!(act.code, 1, "{label}: {}", act.show());
        act.refused(1);
        assert!(
            act.stderr
                .lines()
                .any(|one| one.starts_with(REFUSAL) && one.ends_with("nothing changed")),
            "{label}: `... nothing changed`: {}",
            act.show()
        );
        assert!(
            !act.everything().contains("shown once"),
            "{label}: nothing shown as a password or secret: {}",
            act.show()
        );
        assert_eq!(
            hub.calls().len(),
            1,
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
        calls[0].assert_random_hash("argon2", 32, label);
        assert_eq!(
            calls[0].generated(),
            shown[0],
            "{label}: the shown password is the door's `Random Password:`"
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

// ------------------------------------------------------------- OP11-OP15: dump and restore

/// `tests/deploy_ops.rs`'s stub `date`: the clock file's epoch, in UTC.
const OPS_DATE_STUB: &str = r#"#!/bin/sh
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

/// `tests/deploy_ops.rs`'s stub `sleep`: it keeps each argument, returns at
/// once, and sends TERM to its parent at the call `sleep-term-at` names (or
/// the hundredth).
const OPS_SLEEP_STUB: &str = r#"#!/bin/sh
rec='@REC@'
n=1
if [ -f "$rec/sleep.count" ]; then n=$(( $(cat "$rec/sleep.count") + 1 )); fi
printf '%s\n' "$n" > "$rec/sleep.count"
printf '%s\n' "$*" >> "$rec/sleep.log"
if [ "$(cat "$rec/sleep-term-at" 2>/dev/null)" = "$n" ] || [ "$n" -ge 100 ]; then
    kill -TERM "$PPID"
fi
exit 0
"#;

/// The shim `dump-door` runs as `/opt/hub-admin/hub-admin`: it counts each
/// try, then execs the real script. `@REC@`, `@SHELL@`, `@HUB_ADMIN@`.
const ADMIN_SHIM: &str = r#"#!/bin/sh
printf '%s\n' "$*" >> '@REC@/admin.log'
exec '@SHELL@' '@HUB_ADMIN@' "$@"
"#;

/// 2026-11-04T00:00:00Z, a UTC midnight.
const DAY: i64 = 1_793_750_400;

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

/// The shells `hub-ops` runs under (docs/features/hub-ops.md OP22): `/bin/sh`,
/// `/bin/dash` where it exists, `HUB_OPS_SH` when set; one per binary.
fn ops_shells() -> Vec<String> {
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

fn inode(path: &Path) -> u64 {
    fs::metadata(path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()))
        .ino()
}

fn kill_term(pid: u32) {
    let kill = first_existing(&["/bin/kill", "/usr/bin/kill"], "kill");
    let sent = Command::new(kill)
        .args(["-TERM", &pid.to_string()])
        .status()
        .expect("kill");
    assert!(sent.success(), "SIGTERM to {pid}");
}

/// The names in `dir`, sorted, dot-names included; none when it is missing.
fn names_in(dir: &Path) -> Vec<String> {
    let Ok(found) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = found
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// `hub-ops` beside a [`Hub`], as `dump-door` runs it: the hub's root is also
/// `HUB_OPS_ROOT`, with `dumps/` and the shim at `opt/hub-admin/hub-admin`;
/// the stub `date` and `sleep` first on `PATH`, then the hub's stubs.
struct Ops<'a> {
    hub: &'a Hub,
    bin: PathBuf,
    rec: PathBuf,
    shell: String,
}

impl<'a> Ops<'a> {
    fn new(hub: &'a Hub, ops_shell: &str) -> Self {
        let bin = hub.scratch.dir("ops-bin");
        let rec = hub.scratch.dir("ops-rec");
        executable(
            &bin.join("date"),
            &OPS_DATE_STUB.replace("@REC@", text_of(&rec)),
        );
        executable(
            &bin.join("sleep"),
            &OPS_SLEEP_STUB.replace("@REC@", text_of(&rec)),
        );
        fs::create_dir_all(hub.root.join("dumps")).expect("the hub-dumps mount");
        let opt = hub.root.join("opt/hub-admin");
        fs::create_dir_all(&opt).expect("the ./admin mount");
        executable(
            &opt.join("hub-admin"),
            &ADMIN_SHIM
                .replace("@REC@", text_of(&rec))
                .replace("@SHELL@", &shell())
                .replace("@HUB_ADMIN@", text_of(&hub_admin())),
        );
        fs::write(rec.join("clock"), format!("{}\n", DAY + 12 * 3600)).expect("the clock");
        Self {
            hub,
            bin,
            rec,
            shell: ops_shell.to_owned(),
        }
    }

    fn set_clock(&self, epoch: i64) {
        fs::write(self.rec.join("clock"), format!("{epoch}\n")).expect("the clock");
    }

    fn door_dumps(&self) -> PathBuf {
        self.hub.root.join("dumps/door")
    }

    fn sleeps(&self) -> Vec<String> {
        fs::read_to_string(self.rec.join("sleep.log"))
            .map(|text| text.lines().map(str::to_owned).collect())
            .unwrap_or_default()
    }

    fn tries(&self) -> Vec<String> {
        fs::read_to_string(self.rec.join("admin.log"))
            .map(|text| text.lines().map(str::to_owned).collect())
            .unwrap_or_default()
    }

    /// `sh hub-ops args` with `dump-door`'s environment, stdin `/dev/null`,
    /// under a watchdog; `tick` gets the PID at every poll, then `None` once
    /// it has exited. The run, and when it ended.
    fn run(&self, args: &[&str], tick: &mut dyn FnMut(Option<u32>)) -> (Act, Instant) {
        let mut path = OsString::from(&self.bin);
        path.push(":");
        path.push(&self.hub.bin);
        path.push(":/usr/bin:/bin");
        let label = format!("{} hub-ops {args:?}", self.shell);
        let mut command = Command::new("/usr/bin/perl");
        command
            .env_clear()
            .env("PATH", path)
            .env("HOME", &self.hub.home)
            .env("LANG", "en_US.UTF-8")
            .env("LC_ALL", "en_US.UTF-8")
            .env("HUB_OPS_ROOT", &self.hub.root)
            .env("HUB_ADMIN_ROOT", &self.hub.root)
            .env(
                "AUTHELIA_STORAGE_ENCRYPTION_KEY_FILE",
                "/run/secrets/authelia_storage_key",
            )
            .current_dir(&self.hub.runs)
            .arg("-e")
            .arg("alarm 120; umask 022; exec @ARGV or die \"exec: $!\"")
            .arg(&self.shell)
            .arg(deploy().join("ops/hub-ops"))
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let started = Instant::now();
        let mut child = command
            .spawn()
            .unwrap_or_else(|error| panic!("{label}: {error}"));
        let pid = child.id();
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
        assert!(
            !self.rec.join("date-not-utc").exists(),
            "{label}: a `date` without -u (R10)"
        );
        let act = Act {
            code: status.code().unwrap_or_else(|| {
                -std::os::unix::process::ExitStatusExt::signal(&status).unwrap_or(0)
            }),
            stdout: String::from_utf8_lossy(&out.join().expect("stdout")).into_owned(),
            stderr: String::from_utf8_lossy(&err.join().expect("stderr")).into_owned(),
            screen: String::new(),
            asked: false,
            label,
        };
        (act, ended)
    }

    fn once(&self) -> Act {
        self.run(&["door", "once"], &mut |_: Option<u32>| {}).0
    }
}

/// Every line of a run opens `hub-ops: door <t>: ` (docs/features/hub-ops.md
/// "`hub-ops`", its Lines item).
fn door_prefixed(act: &Act, t: &str) {
    let prefix = format!("hub-ops: door {t}: ");
    for line in act.stdout.lines().chain(act.stderr.lines()) {
        if !line.is_empty() {
            assert!(
                line.starts_with(&prefix),
                "a line not opening `{prefix}`: {line:?}\n{}",
                act.show()
            );
        }
    }
}

/// The door's own files: what a dump must leave as they were.
fn door_state(hub: &Hub) -> BTreeMap<String, Option<Vec<u8>>> {
    hub.files()
        .into_iter()
        .filter(|(path, _)| {
            ["users", "mcp-users", "traefik-dynamic", "var"]
                .iter()
                .any(|top| path == top || path.starts_with(&format!("{top}/")))
        })
        .collect()
}

/// A door with two accounts, an MCP line, an act log and a database.
fn filled_door(hub: &Hub, database: bool) {
    hub.put(
        &hub.users(),
        &users_file(&[
            &entry("mira", "Mira K", "mira@example.org", false),
            &entry("olek", "Olek", "olek@example.org", false),
        ]),
    );
    hub.put(&hub.mcp(), &mcp_line("mira", 'M'));
    hub.put(
        &hub.log(),
        "2026-11-03T09:12:03Z\tadd\tmira\t-\n2026-11-03T09:20:41Z\tadd\tolek\t-\n",
    );
    if database {
        hub.put(&hub.db(), "the door's database, never copied\n");
    }
}

/// The `--file` export call of `kind` into `dir`, as "`hub-admin`: `dump` and
/// `restore`" gives it.
fn export_call(kind: &str, dir: &str, db: &Path) -> Vec<String> {
    [
        "storage",
        "user",
        kind,
        "export",
        "--file",
        &format!("{dir}/{kind}.yml"),
        "--sqlite.path",
        text_of(db),
    ]
    .iter()
    .map(|word| (*word).to_owned())
    .collect()
}

/// docs/features/hub-ops.md "Acceptance criteria" OP11, and "A `door`
/// round": `hub-ops door once` with the real `hub-admin` leaves one
/// generation (mode 700, its files 600) whose `users.yml`, `mcp-users` and
/// `acts.log` are their sources byte for byte and whose `webauthn.yml` and
/// `totp.yml` are the stub's; the stub saw exactly the two `--file` exports
/// into the staging; the door's files and the act log are unchanged and the
/// lock gone. Without `db.sqlite3`, no CLI call and the three files alone. M:
/// copy before the lock; log the dump; skip an export; call the CLI with no
/// database.
#[test]
fn op11_a_door_round_dumps_the_door_with_the_real_hub_admin() {
    for shell in ops_shells() {
        let hub = Hub::new("op11");
        filled_door(&hub, true);
        let ops = Ops::new(&hub, &shell);
        let e = DAY + 12 * 3600 + 15 * 60;
        ops.set_clock(e);
        let t = stamp(e);
        let before = door_state(&hub);

        let act = ops.once();
        act.code(0);
        door_prefixed(&act, &t);
        assert_eq!(
            names_in(&ops.door_dumps()),
            std::slice::from_ref(&t),
            "{}",
            act.show()
        );
        let generation = ops.door_dumps().join(&t);
        assert_eq!(mode(&generation), 0o700, "{shell}: the generation's mode");
        assert_eq!(
            names_in(&generation),
            [
                "acts.log",
                "mcp-users",
                "totp.yml",
                "users.yml",
                "webauthn.yml"
            ],
            "{shell}"
        );
        for name in names_in(&generation) {
            assert_eq!(
                mode(&generation.join(&name)),
                0o600,
                "{shell}: {name}'s mode"
            );
        }
        for (copy, source) in [
            ("users.yml", hub.users()),
            ("mcp-users", hub.mcp()),
            ("acts.log", hub.log()),
        ] {
            assert_eq!(
                bytes(&generation.join(copy)),
                bytes(&source),
                "{shell}: {copy} byte for byte"
            );
        }
        assert_eq!(
            read(&generation.join("webauthn.yml")),
            "# the stub webauthn export, call 1\n"
        );
        assert_eq!(
            read(&generation.join("totp.yml")),
            "# the stub totp export, call 2\n"
        );

        let calls = hub.calls();
        assert_eq!(calls.len(), 2, "{shell}: {calls:?}");
        let file = calls[0]
            .args
            .get(5)
            .unwrap_or_else(|| panic!("{shell}: {calls:?}"));
        let staging = file
            .strip_suffix("/webauthn.yml")
            .unwrap_or_else(|| panic!("{shell}: --file {file}"));
        let shape = Regex::new(&format!(
            r"^{}/dumps/door/\.{t}\.[0-9]+\.tmp$",
            regex::escape(text_of(&hub.root))
        ))
        .expect("a pattern");
        assert!(shape.is_match(staging), "{shell}: the staging {staging}");
        assert_eq!(
            calls[0].args,
            export_call("webauthn", staging, &hub.db()),
            "{shell}"
        );
        assert_eq!(
            calls[1].args,
            export_call("totp", staging, &hub.db()),
            "{shell}"
        );
        assert!(
            calls.iter().all(|call| !call.on_terminal),
            "{shell}: no terminal"
        );
        assert_eq!(
            read(&hub.door.join("timeout.log")),
            format!("300 {0}/authelia\n300 {0}/authelia\n", hub.bin.display()),
            "{shell}: each export under `timeout 300` (DUMP_BOUND)"
        );

        assert_eq!(
            door_state(&hub),
            before,
            "{shell}: the door and the act log unchanged"
        );
        assert!(!hub.lock().exists(), "{shell}: the lock is gone");
        assert!(hub.temps().is_empty(), "{shell}: {:?}", hub.temps());

        let bare = Hub::new("op11-bare");
        filled_door(&bare, false);
        let ops = Ops::new(&bare, &shell);
        ops.set_clock(e);
        ops.once().code(0);
        assert!(bare.calls().is_empty(), "{shell}: no database, no CLI call");
        assert_eq!(
            names_in(&ops.door_dumps().join(&t)),
            ["acts.log", "mcp-users", "users.yml"],
            "{shell}: the three files alone"
        );
    }
}

/// docs/features/hub-ops.md "Acceptance criteria" OP12, and "Rules" R8: with
/// `admin-log/lock/` held, `door once` makes 13 tries, the first and "Data"'s
/// "tries again, 12 times" (the orchestrator's reading of OP12's "12 tries"),
/// `sleep 10` 12 times,
/// leaves no generation and no staging, keeps the lock and its `taken` byte
/// for byte, logs `skipped: <hub-admin's last line>` and exits 1. While a
/// `dump` holds the lock (its export held), `disable` is refused with the new
/// message. TERM to `door` mid-export ends it at once, 143, leaving no lock,
/// no staging and no temp file. M: the lock broken; no retry; no trap; the
/// old message.
#[test]
fn op12_a_dump_and_an_act_never_share_the_lock() {
    for shell in ops_shells() {
        let hub = Hub::new("op12");
        filled_door(&hub, true);
        hub.hold_lock();
        let lock = common::snapshot(&hub.lock());
        let ops = Ops::new(&hub, &shell);
        let act = ops.once();
        act.code(1);
        assert_eq!(
            ops.tries().len(),
            13,
            "{shell}: a try and 12 more: {}",
            act.show()
        );
        assert_eq!(ops.sleeps(), vec!["10"; 12], "{shell}: 10 s apart");
        assert!(
            names_in(&ops.door_dumps()).is_empty(),
            "{shell}: {:?}",
            names_in(&ops.door_dumps())
        );
        assert_eq!(
            common::snapshot(&hub.lock()),
            lock,
            "{shell}: the lock is never broken"
        );
        assert!(hub.calls().is_empty(), "{shell}: nothing called");
        assert!(
            act.stderr
                .lines()
                .any(|line| line.starts_with("hub-ops: door ")
                    && line.contains("skipped: ")
                    && line.contains(HELD_LOCK)),
            "{shell}: `skipped: <hub-admin's last line>`: {}",
            act.show()
        );

        let hub = Hub::new("op12-term");
        filled_door(&hub, true);
        hub.control("hold-at", &[1]);
        let ops = Ops::new(&hub, &shell);
        let mut sent: Option<Instant> = None;
        let (act, ended) = ops.run(&["door", "once"], &mut |pid: Option<u32>| match pid {
            Some(pid) => {
                if sent.is_none() && hub.door.join("held").exists() {
                    kill_term(pid);
                    sent = Some(Instant::now());
                }
            }
            None => fs::write(hub.door.join("release"), "").expect("the stub's release"),
        });
        let sent = sent.unwrap_or_else(|| panic!("{shell}: the export never held: {}", act.show()));
        act.code(143);
        assert!(
            ended - sent < Duration::from_secs(10),
            "{shell}: TERM ends the running dump at once: {}",
            act.show()
        );
        assert!(!hub.lock().exists(), "{shell}: TERM leaves no lock");
        assert!(
            names_in(&ops.door_dumps()).is_empty(),
            "{shell}: no staging, no generation"
        );
        assert!(hub.temps().is_empty(), "{shell}: {:?}", hub.temps());
    }

    let hub = Hub::new("op12-busy");
    filled_door(&hub, true);
    hub.control("hold-at", &[1]);
    let out = hub.scratch.dir("dump-out");
    let users = bytes(&hub.users());
    std::thread::scope(|scope| {
        let dumping = scope.spawn(|| hub.null(&["dump", text_of(&out)]));
        let deadline = Instant::now() + Duration::from_secs(30);
        while !hub.door.join("held").exists() {
            assert!(Instant::now() < deadline, "the dump's export never held");
            std::thread::sleep(Duration::from_millis(10));
        }
        let act = hub.terminal(&["disable", "olek"], &[]);
        fs::write(hub.door.join("release"), "").expect("the stub's release");
        act.refused(1);
        let message = Regex::new(
            r"^another act or a dump holds admin-log/lock \(taken [0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z\); if none runs, remove it$",
        )
        .expect("a pattern");
        assert!(
            act.stderr
                .lines()
                .any(|line| message.is_match(line.strip_prefix(REFUSAL).unwrap_or(line))),
            "the lock's new message: {}",
            act.show()
        );
        assert_eq!(
            bytes(&hub.users()),
            users,
            "the refused disable wrote nothing"
        );
        dumping.join().expect("the dump").code(0);
    });
    assert!(!hub.lock().exists(), "the dump released the lock");
}

/// docs/features/hub-ops.md "Acceptance criteria" OP21, `door`'s twin: with
/// the clock fixed at E, `door` runs a round at start, then asks for `900 -
/// E % 900`.
#[test]
fn op21_the_door_loop_runs_a_round_then_waits_for_the_grid() {
    let e = DAY + 12 * 3600 + 7 * 60 + 30;
    for shell in ops_shells() {
        let hub = Hub::new("op21-door");
        filled_door(&hub, false);
        let ops = Ops::new(&hub, &shell);
        ops.set_clock(e);
        fs::write(ops.rec.join("sleep-term-at"), "1").expect("a control file");
        let (act, _) = ops.run(&["door"], &mut |_: Option<u32>| {});
        act.code(143);
        assert_eq!(
            ops.sleeps(),
            [(900 - e % 900).to_string()],
            "{shell}: {}",
            act.show()
        );
        assert_eq!(ops.tries().len(), 1, "{shell}: one round at start");
        assert_eq!(names_in(&ops.door_dumps()), [stamp(e)], "{shell}");
    }
}

/// docs/features/hub-ops.md "Acceptance criteria" OP13, and "`hub-admin`:
/// `dump` and `restore`": `dump` exits 2, nothing locked, called or written,
/// for no argument, two, a missing, non-empty, `-`-led or non-folder `DIR`,
/// also with the lock held (the check comes first); with the lock held a
/// good call is exit 1, the lock's message, `DIR` empty; a failing export is
/// exit 1 naming it, the lock released, no act log line; with stdin
/// `/dev/null` or a pipe it runs, copies under `umask 077`, prints `dumped
/// <files>`, and dumps each file only when present. M: a non-empty `DIR`
/// accepted; a terminal required; `stopped_log` on `dump`; copy before the
/// lock.
#[test]
fn op13_dump_needs_no_terminal_and_refuses_before_the_lock() {
    let hub = Hub::new("op13");
    filled_door(&hub, true);
    let empty = hub.scratch.dir("empty");
    let other = hub.scratch.dir("other");
    let full = hub.scratch.dir("full");
    fs::write(full.join("left"), "x").expect("a file");
    let file = hub.scratch.path().join("a-file");
    fs::write(&file, "x").expect("a file");
    let missing = hub.scratch.path().join("missing");
    fs::create_dir_all(hub.runs.join("-d")).expect("an empty folder named -d");
    let bad: [&[&str]; 6] = [
        &["dump"],
        &["dump", text_of(&empty), text_of(&other)],
        &["dump", text_of(&missing)],
        &["dump", text_of(&full)],
        &["dump", "-d"],
        &["dump", text_of(&file)],
    ];
    for held in [false, true] {
        if held {
            hub.hold_lock();
        }
        let before = hub.files();
        for args in bad {
            let act = hub.null(args);
            act.refused(2);
            assert_eq!(hub.files(), before, "held {held}: {}", act.label);
            assert!(hub.calls().is_empty(), "held {held}: {}", act.label);
            assert!(names_in(&empty).is_empty() && names_in(&hub.runs.join("-d")).is_empty());
        }
    }

    let act = hub.null(&["dump", text_of(&empty)]);
    act.refused(1);
    assert!(act.says(HELD_LOCK), "the lock's message: {}", act.show());
    assert!(
        names_in(&empty).is_empty(),
        "nothing copied before the lock"
    );
    assert!(hub.calls().is_empty());
    fs::remove_dir_all(hub.lock()).expect("the lock released by hand");

    let logged = bytes(&hub.log());
    for (call, kind) in [(1, "passkey"), (2, "TOTP")] {
        let hub = Hub::new("op13-fail");
        filled_door(&hub, true);
        hub.control("fail-at", &[call]);
        let dir = hub.scratch.dir("out");
        let act = hub.null(&["dump", text_of(&dir)]);
        act.refused(1);
        assert!(
            act.says(&format!(
                "the door's {kind} export failed: Error: the stub fails call {call}"
            )),
            "{}",
            act.show()
        );
        assert!(!hub.lock().exists(), "{kind}: the lock released");
        assert_eq!(bytes(&hub.log()), logged, "{kind}: no act log line");
        assert!(hub.temps().is_empty(), "{kind}: {:?}", hub.temps());
    }

    for piped in [false, true] {
        let hub = Hub::new("op13-run");
        filled_door(&hub, true);
        let dir = hub.scratch.dir("out");
        let before = door_state(&hub);
        let act = if piped {
            hub.piped(&["dump", text_of(&dir)], b"")
        } else {
            hub.null(&["dump", text_of(&dir)])
        };
        act.code(0);
        let found = names_in(&dir);
        assert_eq!(
            found,
            [
                "acts.log",
                "mcp-users",
                "totp.yml",
                "users.yml",
                "webauthn.yml"
            ],
            "{}",
            act.show()
        );
        for name in &found {
            assert_eq!(mode(&dir.join(name)), 0o600, "{name}: umask 077");
        }
        let lines = act.lines();
        assert_eq!(lines.len(), 1, "{}", act.show());
        let listed: BTreeSet<String> = lines[0]
            .strip_prefix("dumped ")
            .unwrap_or_else(|| panic!("`dumped <files>`: {}", act.show()))
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|word| !word.is_empty())
            .map(str::to_owned)
            .collect();
        assert_eq!(listed, found.iter().cloned().collect::<BTreeSet<_>>());
        assert_eq!(
            door_state(&hub),
            before,
            "dump writes nothing of the door's"
        );
        assert!(!hub.lock().exists() && hub.temps().is_empty());
    }

    let hub = Hub::new("op13-some");
    hub.put(
        &hub.users(),
        &users_file(&[&entry("mira", "Mira K", "mira@example.org", false)]),
    );
    let dir = hub.scratch.dir("out");
    hub.null(&["dump", text_of(&dir)]).code(0);
    assert_eq!(names_in(&dir), ["users.yml"], "each file only when present");
    assert!(hub.calls().is_empty(), "no database, no export");

    // Iteration 2: a door with no users file, or an empty one, is no door to
    // dump: exit 1 under the lock, nothing copied, the lock released.
    for users in [None, Some("")] {
        let hub = Hub::new("op13-no-users");
        hub.put(&hub.mcp(), &mcp_line("mira", 'M'));
        hub.put(&hub.log(), "2026-11-03T09:12:03Z\tadd\tmira\t-\n");
        hub.put(&hub.db(), "the door's database\n");
        if let Some(text) = users {
            hub.put(&hub.users(), text);
        }
        let dir = hub.scratch.dir("out");
        let act = hub.null(&["dump", text_of(&dir)]);
        act.refused(1);
        assert!(
            act.stderr.contains("the door has no users file"),
            "{users:?}: {}",
            act.show()
        );
        assert!(names_in(&dir).is_empty(), "{users:?}: nothing dumped");
        assert!(hub.calls().is_empty(), "{users:?}: nothing called");
        assert!(!hub.lock().exists(), "{users:?}: the lock released");
    }
}

/// Beside docs/features/hub-ops.md "Acceptance criteria" OP12, iteration 2:
/// an export that passes its `timeout 300` (the shim exits 124, or 143 for a
/// command it killed) makes `dump` say "the dump timed out", and the round
/// tries no more: no `sleep`, one try, no generation, no staging, no lock, no
/// temp file, exit 1.
#[test]
fn data_a_dump_that_times_out_is_not_tried_again() {
    for code in ["124", "143"] {
        let hub = Hub::new("timed-out");
        filled_door(&hub, true);
        fs::write(hub.door.join("timeout-exit"), code).expect("the shim's exit");
        let dir = hub.scratch.dir("out");
        let act = hub.null(&["dump", text_of(&dir)]);
        act.refused(1);
        assert!(
            act.stderr
                .contains("the dump timed out: the door's passkey export ran past 300 s"),
            "timeout {code}: {}",
            act.show()
        );
        assert!(!hub.lock().exists(), "timeout {code}: the lock released");

        for shell in ops_shells() {
            let hub = Hub::new("timed-out-round");
            filled_door(&hub, true);
            fs::write(hub.door.join("timeout-exit"), code).expect("the shim's exit");
            let ops = Ops::new(&hub, &shell);
            let act = ops.once();
            act.code(1);
            assert_eq!(
                ops.tries().len(),
                1,
                "{shell} {code}: no retry: {}",
                act.show()
            );
            assert!(ops.sleeps().is_empty(), "{shell} {code}: no sleep");
            assert!(
                act.stderr
                    .lines()
                    .any(|line| line.starts_with("hub-ops: door ")
                        && line.contains("skipped: ")
                        && line.contains("the dump timed out")),
                "{shell} {code}: {}",
                act.show()
            );
            assert!(
                names_in(&ops.door_dumps()).is_empty(),
                "{shell} {code}: {:?}",
                names_in(&ops.door_dumps())
            );
            assert!(!hub.lock().exists(), "{shell} {code}: no lock left");
            assert!(hub.temps().is_empty(), "{shell} {code}: {:?}", hub.temps());
        }
    }
}

/// Beside docs/features/hub-ops.md "Acceptance criteria" OP15, iteration 2:
/// a restore stopped by SIGTERM while the door imports says `stopped: remove
/// the door volume's db.sqlite3 before restore runs again`, writes no act
/// log and leaves the users file as it was, the lock gone.
#[test]
fn data_a_stopped_restore_says_what_to_remove() {
    let hub = Hub::new("stopped-restore");
    empty_door(&hub);
    let dir = restore_dir(&hub, true);
    hub.control("hold-at", &[2]);
    let kill = first_existing(&["/bin/kill", "/usr/bin/kill"], "kill");
    let mut sent = false;
    let act = hub.terminal_with(&["restore", text_of(&dir)], &["y"], &mut |pid: &Path| {
        if sent || !hub.door.join("held").exists() {
            return;
        }
        sent = true;
        let pid = read(pid).trim().to_owned();
        let status = Command::new(kill)
            .args(["-TERM", &pid])
            .status()
            .expect("kill");
        assert!(status.success(), "SIGTERM to {pid}");
        std::thread::sleep(Duration::from_millis(300));
        fs::write(hub.door.join("release"), "").expect("the stub's release");
    });
    assert!(sent, "the import never held: {}", act.show());
    assert_eq!(act.code, 143, "{}", act.show());
    assert!(
        act.stderr.contains(
            "hub-admin: stopped: remove the door volume's db.sqlite3 before restore runs again"
        ),
        "{}",
        act.show()
    );
    assert!(!hub.log().exists(), "no act log: {:?}", hub.log_lines());
    assert_eq!(read(&hub.users()), "", "the users file as it was");
    assert!(!hub.lock().exists(), "the lock gone");
}

/// A restore folder as `hub-ops restore` leaves `restore/door/`: two
/// accounts, an MCP line, an act log, and, when `credentials`, the two
/// exports.
fn restore_dir(hub: &Hub, credentials: bool) -> PathBuf {
    let dir = hub.scratch.dir("restore-door");
    fs::write(
        dir.join("users.yml"),
        users_file(&[
            &entry("mira", "Mira K", "mira@example.org", false),
            &entry("olek", "Olek", "olek@example.org", true),
        ]),
    )
    .expect("users.yml");
    fs::write(dir.join("mcp-users"), mcp_line("mira", 'R')).expect("mcp-users");
    fs::write(
        dir.join("acts.log"),
        "2026-11-03T09:12:03Z\tadd\tmira\t-\n2026-11-03T10:01:44Z\tdisable\tolek\t-\n",
    )
    .expect("acts.log");
    if credentials {
        fs::write(dir.join("webauthn.yml"), "webauthn_devices: []\n").expect("webauthn.yml");
        fs::write(dir.join("totp.yml"), "totp_configurations: []\n").expect("totp.yml");
    }
    dir
}

/// The new host's empty door: an empty users file, nothing else.
fn empty_door(hub: &Hub) {
    hub.put(&hub.users(), "");
}

/// docs/features/hub-ops.md "Acceptance criteria" OP14, and "`hub-admin`:
/// `dump` and `restore`": into an empty door, on a terminal, answered `y`,
/// the stub sees `migrate up`, `webauthn import` and `totp import` in that
/// order, `DIR`'s files after `--`, while the users file is still empty; then
/// `users.yml` and the MCP users file are `DIR`'s bytes, each a new inode of
/// mode 600; the stamp goes up; `acts.log` is `DIR`'s bytes plus one
/// `restore` line; stdout is the two lines. Answered `n`: exit 1, nothing
/// called or written. `yes` restores too, and a `DIR` with no exports calls
/// nothing. M: the users file before the imports; no stamp; the log without
/// the `restore` line, or without `DIR`'s lines.
#[test]
fn op14_restore_fills_an_empty_door_after_the_imports() {
    let hub = Hub::new("op14");
    empty_door(&hub);
    let dir = restore_dir(&hub, true);
    let dir_text = text_of(&dir).to_owned();
    let old_inode = inode(&hub.users());
    fs::write(hub.door.join("watch"), text_of(&hub.users())).expect("the watch");

    let act = hub.terminal(&["restore", &dir_text], &["y"]);
    act.code(0);
    assert!(act.asked, "{}", act.show());
    assert!(
        act.everything()
            .contains(&format!("restore 2 account(s) from {dir_text}")),
        "the question: {}",
        act.show()
    );
    let db = text_of(&hub.db()).to_owned();
    let calls: Vec<Vec<String>> = hub.calls().into_iter().map(|call| call.args).collect();
    let want: Vec<Vec<String>> = [
        vec!["storage", "migrate", "up", "--sqlite.path", &db],
        vec![
            "storage",
            "user",
            "webauthn",
            "import",
            "--sqlite.path",
            &db,
            "--",
            &format!("{dir_text}/webauthn.yml"),
        ],
        vec![
            "storage",
            "user",
            "totp",
            "import",
            "--sqlite.path",
            &db,
            "--",
            &format!("{dir_text}/totp.yml"),
        ],
    ]
    .iter()
    .map(|call| call.iter().map(|word| (*word).to_owned()).collect())
    .collect();
    assert_eq!(calls, want, "{}", act.show());
    for n in 1..=3 {
        assert_eq!(
            fs::read(hub.door.join(format!("watch.{n}"))).ok(),
            Some(Vec::new()),
            "the users file is still empty at call {n}: the imports come first"
        );
    }
    for (installed, source) in [
        (hub.users(), dir.join("users.yml")),
        (hub.mcp(), dir.join("mcp-users")),
    ] {
        assert_eq!(bytes(&installed), bytes(&source), "{}", installed.display());
        assert_eq!(mode(&installed), 0o600, "{}", installed.display());
        assert_ne!(
            inode(&installed),
            inode(&source),
            "{}: a new inode",
            installed.display()
        );
    }
    assert_ne!(inode(&hub.users()), old_inode, "users.yml: a new inode");
    assert_eq!(stamp_value(&hub.stamp()), 1, "the stamp went up");
    let log = read(&hub.log());
    let restored = read(&dir.join("acts.log"));
    let added = log
        .strip_prefix(&restored)
        .unwrap_or_else(|| panic!("the act log opens with DIR's bytes: {log:?}"));
    let added: Vec<&str> = added.lines().collect();
    assert_eq!(added.len(), 1, "one line added: {log:?}");
    assert_logged(added[0], "restore", "-", "users mcp passkeys totp");
    assert_eq!(
        act.lines(),
        [
            format!("restored 2 account(s) from {dir_text}"),
            "next: docker compose up -d traefik authelia specengine".to_owned(),
        ],
        "{}",
        act.show()
    );
    assert!(
        !hub.lock().exists() && hub.temps().is_empty(),
        "{:?}",
        hub.temps()
    );

    let hub = Hub::new("op14-no");
    empty_door(&hub);
    let dir = restore_dir(&hub, true);
    let before = hub.files();
    let act = hub.terminal(&["restore", text_of(&dir)], &["n"]);
    // After the question its refusal shares the prompt's stderr line, as
    // `disable`'s does (AC4): the answer's echo ends the line on a terminal.
    act.code(1);
    assert!(act.asked, "{}", act.show());
    assert!(
        act.stderr
            .contains("hub-admin: not restored: the answer was not y; nothing changed"),
        "{}",
        act.show()
    );
    assert!(hub.calls().is_empty(), "answered n: nothing called");
    assert_eq!(hub.files(), before, "answered n: nothing written");

    let hub = Hub::new("op14-yes");
    empty_door(&hub);
    let dir = restore_dir(&hub, false);
    let act = hub.terminal(&["restore", text_of(&dir)], &["yes"]);
    act.code(0);
    assert!(
        hub.calls().is_empty(),
        "no exports in DIR, no CLI call: {}",
        act.show()
    );
    assert_eq!(bytes(&hub.users()), bytes(&dir.join("users.yml")));
    let last = hub.log_lines().last().cloned().expect("a restore line");
    assert_logged(&last, "restore", "-", "users mcp");
}

/// A filled door's path and how a test fills it.
type Fill<'a> = (&'a str, &'a dyn Fn(&Hub));

/// docs/features/hub-ops.md "Acceptance criteria" OP15, and "`hub-admin`:
/// `dump` and `restore`": `restore` exits 2 for stdin a pipe or
/// `/dev/null`, no `DIR`, an extra argument, a `DIR` that is no folder, and a
/// missing or empty `DIR/users.yml`; exit 1 before the question, nothing
/// called or written, naming the path, for a non-empty users file, MCP users
/// file or act log, and an existing `db.sqlite3`; exit 1 for a `DIR/users.yml`
/// in no known layout; a failing step is exit 1, both users files untouched,
/// the database named. M: the database check dropped; a non-empty users file
/// accepted.
#[test]
fn op15_restore_goes_into_an_empty_door_alone() {
    let hub = Hub::new("op15");
    empty_door(&hub);
    let dir = restore_dir(&hub, true);
    let dir_text = text_of(&dir).to_owned();
    let before = hub.files();
    for act in [
        hub.piped(&["restore", &dir_text], b"y\n"),
        hub.null(&["restore", &dir_text]),
    ] {
        act.refused(2);
        assert!(
            hub.calls().is_empty() && hub.files() == before,
            "{}",
            act.label
        );
    }
    let no_users = hub.scratch.dir("no-users");
    fs::write(no_users.join("mcp-users"), mcp_line("mira", 'R')).expect("mcp-users");
    let empty_users = hub.scratch.dir("empty-users");
    fs::write(empty_users.join("users.yml"), "").expect("an empty users.yml");
    let not_a_folder = dir.join("users.yml");
    let bad: [&[&str]; 5] = [
        &["restore"],
        &["restore", &dir_text, "extra"],
        &["restore", text_of(&not_a_folder)],
        &["restore", text_of(&no_users)],
        &["restore", text_of(&empty_users)],
    ];
    for args in bad {
        let act = hub.terminal(args, &["y"]);
        act.refused(2);
        assert!(!act.asked, "{}", act.show());
        assert!(
            hub.calls().is_empty() && hub.files() == before,
            "{}",
            act.label
        );
    }

    let filled: [Fill; 4] = [
        ("users.yml", &|hub: &Hub| {
            hub.put(
                &hub.users(),
                &users_file(&[&entry("zed", "Zed", "zed@example.org", false)]),
            );
        }),
        ("mcp-users/users", &|hub: &Hub| {
            hub.put(&hub.mcp(), &mcp_line("zed", 'Z'))
        }),
        ("acts.log", &|hub: &Hub| {
            hub.put(&hub.log(), "2026-11-04T08:00:00Z\tadd\tzed\t-\n");
        }),
        ("db.sqlite3", &|hub: &Hub| hub.put(&hub.db(), "")),
    ];
    let message = Regex::new(
        r"^the door is not empty: (.+) exists; a restore goes into a new host's empty door; nothing changed$",
    )
    .expect("a pattern");
    for (path, fill) in filled {
        let hub = Hub::new("op15-filled");
        empty_door(&hub);
        fill(&hub);
        let dir = restore_dir(&hub, true);
        let before = hub.files();
        let act = hub.terminal(&["restore", text_of(&dir)], &["y"]);
        act.refused(1);
        assert!(
            !act.asked,
            "{path}: refused before the question: {}",
            act.show()
        );
        let named = act
            .stderr
            .lines()
            .filter_map(|line| message.captures(line.strip_prefix(REFUSAL).unwrap_or(line)))
            .map(|found| found[1].to_owned())
            .next()
            .unwrap_or_else(|| panic!("{path}: the door is not empty: {}", act.show()));
        assert!(named.ends_with(path), "{path}: names {named}");
        assert!(hub.calls().is_empty(), "{path}: nothing called");
        assert_eq!(hub.files(), before, "{path}: nothing written");
    }

    let hub = Hub::new("op15-layout");
    empty_door(&hub);
    let dir = restore_dir(&hub, true);
    fs::write(
        dir.join("users.yml"),
        "users:\n  mira:\n\tdisplayname: \"Mira K\"\n",
    )
    .expect("a users.yml in no known layout");
    let before = hub.files();
    let act = hub.terminal(&["restore", text_of(&dir)], &["y"]);
    act.refused(1);
    assert!(!act.asked, "{}", act.show());
    assert!(
        hub.calls().is_empty() && hub.files() == before,
        "{}",
        act.show()
    );

    for call in 1..=3 {
        let hub = Hub::new("op15-step");
        empty_door(&hub);
        let dir = restore_dir(&hub, true);
        let users = (bytes(&hub.users()), inode(&hub.users()));
        hub.control("fail-at", &[call]);
        let act = hub.terminal(&["restore", text_of(&dir)], &["y"]);
        act.code(1);
        assert!(act.asked, "step {call}: after the question: {}", act.show());
        let line = act
            .stderr
            .lines()
            .find(|line| line.contains("failed: "))
            .unwrap_or_else(|| panic!("step {call}: a failed line: {}", act.show()));
        for part in [
            "the door's ",
            &format!("Error: the stub fails call {call}"),
            "the users files are as they were",
            "db.sqlite3",
        ] {
            assert!(line.contains(part), "step {call}: {part}: {line}");
        }
        assert_eq!(
            hub.calls().len(),
            call,
            "step {call}: nothing after the failure"
        );
        assert_eq!(
            (bytes(&hub.users()), inode(&hub.users())),
            users,
            "step {call}"
        );
        assert!(!hub.mcp().exists(), "step {call}: no MCP users file");
        assert!(!hub.stamp().exists(), "step {call}: no stamp");
        assert!(!hub.lock().exists(), "step {call}: the lock released");
    }
}

/// docs/features/hub-ops.md "Data", "`hub-admin`: `dump` and `restore`": the
/// usage gains `dump <empty folder>` and `restore <folder>`.
#[test]
fn data_the_usage_names_dump_and_restore() {
    let hub = Hub::new("usage");
    let act = hub.null(&[]);
    act.refused(2);
    for line in ["dump <empty folder>", "restore <folder>"] {
        assert!(
            act.stderr.contains(line),
            "the usage: {line}: {}",
            act.show()
        );
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
/// --images` printing `authelia`'s image once per service on it, the image it
/// hands `docker run` is that image, one line. Rewritten beside
/// docs/features/hub-ops.md "Acceptance criteria" OP23 (which omits it): the
/// services on the door's image are `admin`, `authelia` and `dump-door`, three
/// lines where 11c-1 had two.
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
    let on_door: BTreeSet<&str> = object(compose.get("services").expect("services"), "services")
        .iter()
        .filter(|(_, service)| {
            service.get("image").and_then(Value::as_str) == Some(authelia.as_str())
        })
        .map(|(name, _)| name.as_str())
        .collect();
    assert_eq!(
        on_door,
        BTreeSet::from(["admin", "authelia", "dump-door"]),
        "the services on authelia's image: {images:?}"
    );
    assert_eq!(
        images.iter().filter(|image| **image == authelia).count(),
        3,
        "three image lines of authelia's: {images:?}"
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
