//! git-lfs and hashing helpers of the reference-promotion tests
//! (docs/features/reference-promotion.md, RM-0138), standard library only,
//! so the CLI's, the daemon's and the soak's tests include it by path.
//!
//! - D18: a test needing git-lfs fails naming it when it is missing; none
//!   skips. [`git_lfs`] finds the real `git-lfs` on the test process's
//!   `PATH` (or a usual install place), checks its version is 3.0 or later,
//!   and [`lfs_dir`] links it alone into a scratch directory, so a product's
//!   `PATH` gains git-lfs and nothing else (the sandboxes' `PATH` is the
//!   real git's directory and `/usr/bin:/bin`, where no git-lfs lives).
//! - [`stub_lfs_dir`]: a `git-lfs` stand-in answering `version` as 3.8.0
//!   and `clean` with a pointer of a wrong `oid` (PR3), or `push` with a
//!   failure (PR18).
//! - SHA-256 by the system's `shasum -a 256` (macOS) or `sha256sum`: the
//!   workspace has no SHA-256 crate for tests, and the product's own is the
//!   subject.
//! - [`Pointer`]: a v1 LFS pointer read strictly (its three lines, in
//!   order, LF-ended).

#![allow(dead_code)]

use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The first line of every v1 pointer.
pub const POINTER_VERSION: &str = "version https://git-lfs.github.com/spec/v1";

/// The real `git-lfs`: on the test process's `PATH`, else a usual install
/// place. Missing or older than 3.0: the test fails naming it (D18).
pub fn git_lfs() -> PathBuf {
    let mut candidates: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|path| {
            std::env::split_paths(&path)
                .map(|dir| dir.join("git-lfs"))
                .collect()
        })
        .unwrap_or_default();
    for place in [
        "/opt/homebrew/bin/git-lfs",
        "/usr/local/bin/git-lfs",
        "/usr/bin/git-lfs",
    ] {
        candidates.push(PathBuf::from(place));
    }
    let found = candidates.into_iter().find(|path| {
        fs::metadata(path)
            .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
    });
    let Some(found) = found else {
        panic!(
            "git-lfs is not installed: the reference-promotion tests need git-lfs 3.0 or later \
             on PATH (docs/features/reference-promotion.md D18); install it (`brew install \
             git-lfs`, `apt-get install git-lfs`) and run again"
        );
    };
    let output = Command::new(&found)
        .arg("version")
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", std::env::temp_dir())
        .stdin(Stdio::null())
        .output()
        .unwrap_or_else(|error| panic!("git-lfs at {} does not run: {error}", found.display()));
    let text = String::from_utf8_lossy(&output.stdout).into_owned();
    let major = text
        .strip_prefix("git-lfs/")
        .and_then(|rest| rest.split('.').next())
        .and_then(|major| major.parse::<u32>().ok())
        .unwrap_or_else(|| panic!("git-lfs version unreadable: {text:?}"));
    assert!(
        major >= 3,
        "git-lfs {text:?} is older than 3.0: the tests need 3.0 or later (D18)"
    );
    found
}

/// A scratch directory `<parent>/lfs-bin` holding only a link to the real
/// `git-lfs`: a product's `PATH` with it first gains git-lfs alone.
pub fn lfs_dir(parent: &Path) -> PathBuf {
    let dir = parent.join("lfs-bin");
    fs::create_dir_all(&dir).expect("the lfs directory");
    let link = dir.join("git-lfs");
    if !link.exists() {
        std::os::unix::fs::symlink(git_lfs(), &link).expect("link git-lfs");
    }
    fs::canonicalize(dir).expect("canonical")
}

/// What a `git-lfs` stand-in does unlike the real one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stub {
    /// `clean` reads its stdin and prints a v1 pointer of the right size
    /// and an `oid` of 64 zeros (PR3).
    WrongOid,
    /// `push` (but `push --dry-run`) exits 1 with a line on stderr (PR18).
    FailPush,
    /// Every verb delegated: a logging wrapper (PR18's smudge check).
    Log,
    /// `version` answers 2.13.3, under the 3.0 floor (D17); every other
    /// verb delegated.
    OldVersion,
}

/// A scratch directory `<parent>/<name>` holding a `git-lfs` stand-in: a
/// shell script answering `version` as 3.8.0, doing what `stub` says, and
/// delegating every other call to the real git-lfs. Each call is logged,
/// one line of its arguments, to `<dir>/calls.log`.
pub fn stub_lfs_dir(parent: &Path, name: &str, stub: Stub) -> PathBuf {
    let dir = parent.join(name);
    fs::create_dir_all(&dir).expect("the stub directory");
    let real = git_lfs();
    let log = dir.join("calls.log");
    let special = match stub {
        Stub::WrongOid => {
            "if [ \"$1\" = clean ]; then\n  size=$(wc -c | tr -d ' ')\n  printf 'version https://git-lfs.github.com/spec/v1\\noid sha256:%s\\nsize %s\\n' 0000000000000000000000000000000000000000000000000000000000000000 \"$size\"\n  exit 0\nfi\n"
        }
        Stub::FailPush => {
            "if [ \"$1\" = push ]; then\n  case \" $* \" in\n    *' --dry-run '*) ;;\n    *) echo 'batch request: the stub refused the upload' >&2; exit 1 ;;\n  esac\nfi\n"
        }
        Stub::Log | Stub::OldVersion => "",
    };
    let version = if stub == Stub::OldVersion {
        "git-lfs/2.13.3 (stub; darwin arm64; go 1.16.3)"
    } else {
        "git-lfs/3.8.0 (stub; darwin arm64; go 1.27.0)"
    };
    let script = format!(
        "#!/bin/sh\necho \"$*\" >> '{log}'\nif [ \"$1\" = version ]; then\n  echo '{version}'\n  exit 0\nfi\n{special}exec '{real}' \"$@\"\n",
        log = log.display(),
        real = real.display(),
    );
    let path = dir.join("git-lfs");
    fs::write(&path, script).expect("write the stub");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("chmod the stub");
    fs::canonicalize(dir).expect("canonical")
}

/// The calls a stand-in of [`stub_lfs_dir`] logged, one per line.
pub fn stub_calls(dir: &Path) -> Vec<String> {
    fs::read_to_string(dir.join("calls.log"))
        .unwrap_or_default()
        .lines()
        .map(str::to_owned)
        .collect()
}

/// `dirs`, then the entries of `path` (a `PATH` value), as one `PATH`.
pub fn path_with(dirs: &[&Path], path: &std::ffi::OsStr) -> OsString {
    let mut all: Vec<PathBuf> = dirs.iter().map(|dir| dir.to_path_buf()).collect();
    all.extend(std::env::split_paths(path));
    std::env::join_paths(all).expect("a PATH value")
}

/// The SHA-256 of the file at `path`, 64 lower-case hex, by the system's
/// tool (the file is streamed by it, never read whole here).
pub fn sha256_file(path: &Path) -> String {
    for (program, args) in [
        ("/usr/bin/shasum", vec!["-a", "256"]),
        ("/usr/bin/sha256sum", vec![]),
        ("/bin/sha256sum", vec![]),
    ] {
        if !Path::new(program).exists() {
            continue;
        }
        let output = Command::new(program)
            .args(&args)
            .arg(path)
            .stdin(Stdio::null())
            .output()
            .expect("a SHA-256 tool runs");
        assert!(output.status.success(), "{program} {}", path.display());
        let text = String::from_utf8(output.stdout).expect("UTF-8");
        let hex = text.split_whitespace().next().expect("a hash").to_owned();
        assert_eq!(hex.len(), 64, "{text}");
        return hex;
    }
    panic!("no shasum or sha256sum on this machine");
}

/// The SHA-256 of `bytes` (written to a scratch file under `scratch`).
pub fn sha256_bytes(scratch: &Path, bytes: &[u8]) -> String {
    let file = scratch.join(format!(".sha256-{}", std::process::id()));
    fs::write(&file, bytes).expect("a scratch file");
    let hex = sha256_file(&file);
    let _ = fs::remove_file(&file);
    hex
}

/// A v1 LFS pointer, read strictly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pointer {
    pub oid: String,
    pub size: u64,
}

impl Pointer {
    /// `bytes` as a v1 pointer: exactly `version …`, `oid sha256:<64 hex>`,
    /// `size <n>`, each LF-ended; `None` otherwise.
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        let text = std::str::from_utf8(bytes).ok()?;
        let body = text.strip_suffix('\n')?;
        let lines: Vec<&str> = body.split('\n').collect();
        if lines.len() != 3 || lines[0] != POINTER_VERSION {
            return None;
        }
        let oid = lines[1].strip_prefix("oid sha256:")?;
        if oid.len() != 64
            || !oid
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return None;
        }
        let size = lines[2].strip_prefix("size ")?.parse().ok()?;
        Some(Self {
            oid: oid.to_owned(),
            size,
        })
    }

    /// The pointer's bytes, as git-lfs writes them.
    pub fn bytes(&self) -> Vec<u8> {
        format!(
            "{POINTER_VERSION}\noid sha256:{}\nsize {}\n",
            self.oid, self.size
        )
        .into_bytes()
    }
}

/// Where git-lfs keeps an object of `oid` under a git dir.
pub fn lfs_object(git_dir: &Path, oid: &str) -> PathBuf {
    git_dir
        .join("lfs")
        .join("objects")
        .join(&oid[..2])
        .join(&oid[2..4])
        .join(oid)
}

/// Binary units, one decimal, as SpecEngine prints sizes (`2.1 MiB`,
/// `51.0 MiB`, `1.5 KiB`, `44 B`).
pub fn size_text(bytes: u64) -> String {
    let units = ["KiB", "MiB", "GiB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64 / 1024.0;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < units.len() {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.1} {}", units[unit])
}
