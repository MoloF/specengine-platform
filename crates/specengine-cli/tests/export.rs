//! AC-09 and AC-11 of docs/features/spec-cli-check.md: `spec export index
//! [--stdout]` on copies of spec-a and spec-b whose configs (written into
//! the copies, never `fixtures/`) register `command = "gen-index"`,
//! `index = true` and a `[paths] index` inside the walk. The written file
//! is the library's `render_index` (header naming the registered command
//! and gate, never the binary), after which `spec check` reports neither
//! `index-drift` nor `index-missing`; equal bytes are not rewritten (mtime
//! kept); `--stdout` prints the same bytes and writes nothing. Refusals exit
//! 2 with nothing on stdout and the index unchanged or absent.

#![cfg(unix)]

mod common;

use std::fs;
use std::os::unix::fs::{PermissionsExt as _, symlink};
use std::path::Path;
use std::time::{Duration, SystemTime};

use common::check::{codes, index_path, library_render, registered, set_paths_key};
use common::{FIXTURES, Run, Scratch, read, read_text, snapshot, spec, write};

/// A time long past, set on the index to see whether it is rewritten.
fn long_ago() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000_000)
}

fn set_mtime(path: &Path, time: SystemTime) {
    fs::File::options()
        .write(true)
        .open(path)
        .expect("the index opens")
        .set_modified(time)
        .expect("mtime set");
}

fn mtime(path: &Path) -> SystemTime {
    fs::symlink_metadata(path)
        .expect("the index exists")
        .modified()
        .expect("mtime")
}

/// The copy's config with the registry for `command` and `gate`.
fn register(root: &Path, base: &str, index: &str, command: &str, gate: Option<&str>) {
    write(
        root,
        "specengine.toml",
        registered(base, index, command, gate),
    );
}

/// AC-09.
#[test]
fn the_written_index_is_the_library_s_render() {
    for (fixture, _) in FIXTURES {
        let scratch = Scratch::new("export");
        let home = scratch.home("h");
        let root = scratch.copy(fixture, "copy");
        let base = read_text(&root, "specengine.toml");
        let index = index_path(fixture);
        register(&root, &base, index, "gen-index", None);
        let expected = library_render(&root);
        assert!(
            expected.contains("\ngenerator: gen-index\n")
                && expected.contains("Built by `gen-index`")
                && expected.contains("`spec check` rejects them"),
            "{fixture}: the library's header"
        );

        // --stdout: the render, nothing written.
        let before = snapshot(&root);
        let run = spec(&home, &root, &["export", "index", "--stdout"]);
        run.code(0);
        assert!(run.stdout == expected, "{fixture}: --stdout is the render");
        assert_eq!(run.stderr, "", "{fixture}");
        assert_eq!(snapshot(&root), before, "{fixture}: --stdout wrote");

        // Written.
        let run = spec(&home, &root, &["export", "index"]);
        run.code(0);
        assert_eq!(
            run.stdout,
            format!("wrote {index}: {} bytes\n", expected.len())
        );
        assert_eq!(run.stderr, "", "{fixture}");
        assert!(
            read(&root, index) == expected.as_bytes(),
            "{fixture}: the file is the library's render"
        );
        assert!(
            !expected.contains("spec export"),
            "{fixture}: the binary named"
        );

        // `spec check` then sees neither drift nor a missing index.
        let check = spec(&home, &root, &["--json", "check"]);
        let found = codes(&check.stdout);
        assert!(
            !found
                .iter()
                .any(|code| code == "index-drift" || code == "index-missing"),
            "{fixture}: {found:?}"
        );

        // A rerun: unchanged, the file not rewritten.
        let file = root.join(index);
        set_mtime(&file, long_ago());
        let run = spec(&home, &root, &["export", "index"]);
        run.code(0);
        assert_eq!(
            run.stdout,
            format!("unchanged {index}: {} bytes\n", expected.len())
        );
        assert_eq!(mtime(&file), long_ago(), "{fixture}: equal bytes rewritten");
        let run = spec(&home, &root, &["--json", "export", "index"]);
        run.code(0);
        assert_eq!(
            run.stdout,
            format!(
                "{{\"path\":\"{index}\",\"bytes\":{},\"written\":false}}\n",
                expected.len()
            )
        );
        assert_eq!(mtime(&file), long_ago(), "{fixture}: --json rewrote");
        let run = spec(&home, &root, &["export", "index", "--stdout"]);
        assert!(run.stdout == expected, "{fixture}: --stdout after writing");

        // JSON of a write.
        fs::remove_file(&file).unwrap();
        let run = spec(&home, &root, &["export", "--json", "index"]);
        run.code(0);
        let document = run.json();
        assert_eq!(
            document,
            serde_json::json!({"path": index, "bytes": expected.len(), "written": true}),
            "{fixture}"
        );
        assert!(read(&root, index) == expected.as_bytes());

        // A registered gate reaches the header.
        register(&root, &base, index, "gen-index", Some("gen-check"));
        let gated = library_render(&root);
        assert!(gated.contains("`gen-check` rejects them"), "{fixture}");
        let run = spec(&home, &root, &["export", "index"]);
        run.code(0);
        assert!(run.stdout.starts_with(&format!("wrote {index}: ")));
        assert!(read(&root, index) == gated.as_bytes(), "{fixture}: gate");
    }
}

/// Exit 2, nothing on stdout, the index as before (absent or its bytes),
/// the one stderr line (`spec: …`) holding every piece of `says`.
fn assert_refused(what: &str, run: &Run, index: &Path, before: Option<&[u8]>, says: &[&str]) {
    assert_eq!(run.code, 2, "{what}:\n{}", run.show());
    assert_eq!(run.stdout, "", "{what}");
    let lines = run.stderr_lines();
    assert_eq!(lines.len(), 1, "{what}: one stderr line:\n{}", run.stderr);
    assert!(lines[0].starts_with("spec: "), "{what}: {}", run.stderr);
    for piece in says {
        assert!(
            lines[0].contains(piece),
            "{what}: {piece:?} in {}",
            lines[0]
        );
    }
    match before {
        None => assert!(
            fs::symlink_metadata(index).is_err(),
            "{what}: an index appeared"
        ),
        Some(bytes) => assert_eq!(fs::read(index).unwrap(), bytes, "{what}: index changed"),
    }
}

/// AC-11.
#[test]
fn refusals_write_nothing() {
    for (fixture, _) in FIXTURES {
        let scratch = Scratch::new("export-refused");
        let home = scratch.home("h");
        let root = scratch.copy(fixture, "copy");
        let base = read_text(&root, "specengine.toml");
        let index = index_path(fixture);
        let file = root.join(index);
        let stale: &[u8] = b"stale index\n";

        // No [[generators]] (the index path set).
        write(
            &root,
            "specengine.toml",
            set_paths_key(&base, "index", &format!("\"{index}\"")),
        );
        for bytes in [None, Some(stale)] {
            if let Some(bytes) = bytes {
                write(&root, index, bytes);
            }
            let run = spec(&home, &root, &["export", "index"]);
            assert_refused("no [[generators]]", &run, &file, bytes, &["[[generators]]"]);
        }
        fs::remove_file(&file).unwrap();

        // An entry without `index = true`.
        write(
            &root,
            "specengine.toml",
            format!(
                "{}\n[[generators]]\ncommand = \"gen-other\"\nwrites  = [\"docs/other.md\"]\n",
                set_paths_key(&base, "index", &format!("\"{index}\""))
            ),
        );
        let run = spec(&home, &root, &["export", "index"]);
        assert_refused("no index = true", &run, &file, None, &["index = true"]);

        register(&root, &base, index, "gen-index", None);

        // A mode-000 document; `--stdout` refused too.
        let document = "docs/spec/game.md";
        let document = if root.join(document).exists() {
            document
        } else {
            "docs/spec/cli.md"
        };
        fs::set_permissions(root.join(document), fs::Permissions::from_mode(0o000)).unwrap();
        for args in [
            &["export", "index"][..],
            &["export", "index", "--stdout"][..],
        ] {
            let run = spec(&home, &root, args);
            assert_refused("a mode-000 document", &run, &file, None, &[document]);
        }
        fs::set_permissions(root.join(document), fs::Permissions::from_mode(0o644)).unwrap();

        // A mode-000 directory.
        let dir = root.join("docs/records");
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o000)).unwrap();
        for args in [
            &["export", "index"][..],
            &["export", "index", "--stdout"][..],
        ] {
            let run = spec(&home, &root, args);
            assert_refused("a mode-000 directory", &run, &file, None, &["docs/records"]);
        }
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();

        // A missing written root.
        let text = registered(&base, index, "gen-index", None);
        write(
            &root,
            "specengine.toml",
            set_paths_key(&text, "roots", "[\"docs\", \"nowhere\"]"),
        );
        for args in [
            &["export", "index"][..],
            &["export", "index", "--stdout"][..],
        ] {
            let run = spec(&home, &root, args);
            assert_refused("a missing written root", &run, &file, None, &["nowhere"]);
        }

        // The index a symlink: its target untouched.
        register(&root, &base, index, "gen-index", None);
        let target = scratch.join("target.md");
        fs::write(&target, "the target\n").unwrap();
        symlink(&target, &file).unwrap();
        let run = spec(&home, &root, &["export", "index"]);
        assert_eq!(run.code, 2, "{}", run.show());
        assert_eq!(run.stdout, "");
        assert_eq!(run.stderr_lines().len(), 1, "{}", run.stderr);
        assert!(run.stderr.starts_with("spec: ") && run.stderr.contains(index));
        assert_eq!(fs::read_to_string(&target).unwrap(), "the target\n");
        assert!(
            fs::symlink_metadata(&file)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        fs::remove_file(&file).unwrap();

        // Its directory a symlink: nothing created behind it.
        let elsewhere = scratch.dir("elsewhere");
        symlink(&elsewhere, root.join("docs/gen")).unwrap();
        register(&root, &base, "docs/gen/index.md", "gen-index", None);
        let run = spec(&home, &root, &["export", "index"]);
        assert_refused(
            "a symlinked directory",
            &run,
            &elsewhere.join("index.md"),
            None,
            &["docs/gen"],
        );
        assert!(fs::read_dir(&elsewhere).unwrap().next().is_none());
        fs::remove_file(root.join("docs/gen")).unwrap();

        // A missing parent: not created.
        register(&root, &base, "docs/nothere/index.md", "gen-index", None);
        let run = spec(&home, &root, &["export", "index"]);
        assert_refused(
            "a missing parent",
            &run,
            &root.join("docs/nothere/index.md"),
            None,
            &["docs/nothere"],
        );
        assert!(
            !root.join("docs/nothere").exists(),
            "the parent was created"
        );

        // A config error: one `<config>:<line>:` line per cause.
        let text = registered(&base, index, "gen-index", None);
        write(
            &root,
            "specengine.toml",
            format!("{text}\n[bogus]\nx = 1\n"),
        );
        let run = spec(&home, &root, &["export", "index"]);
        assert_eq!(run.code, 2, "{}", run.show());
        assert_eq!(run.stdout, "");
        let line = text.lines().count() + 2;
        let lines = run.stderr_lines();
        assert_eq!(lines.len(), 1, "{fixture}: {}", run.stderr);
        assert!(
            lines[0].starts_with(&format!("specengine.toml:{line}: "))
                && lines[0].contains("bogus"),
            "{fixture}: {}",
            run.stderr
        );
        assert!(fs::symlink_metadata(&file).is_err());
    }
}

/// AC-11: a name that is not UTF-8 is skipped with one `warning:`, the
/// index written anyway. APFS refuses such names (EILSEQ): there the case
/// cannot be set up and is reported skipped.
#[test]
fn a_non_utf8_name_is_written_with_one_warning() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt as _;

    for (fixture, _) in FIXTURES {
        let scratch = Scratch::new("export-non-utf8");
        let home = scratch.home("h");
        let root = scratch.copy(fixture, "copy");
        let base = read_text(&root, "specengine.toml");
        let index = index_path(fixture);
        register(&root, &base, index, "gen-index", None);
        let name = root
            .join("docs/spec")
            .join(OsStr::from_bytes(b"bad\xff.md"));
        if let Err(error) = fs::write(&name, "# Bad\n") {
            eprintln!(
                "skipped: the file system refuses a non-UTF-8 name ({error}); AC-11's warning case is not exercised here"
            );
            return;
        }
        let run = spec(&home, &root, &["export", "index"]);
        run.code(0);
        let lines = run.stderr_lines();
        assert_eq!(lines.len(), 1, "{}", run.stderr);
        assert!(lines[0].starts_with("warning: "), "{}", run.stderr);
        assert!(read(&root, index) == library_render(&root).as_bytes());
    }
}

/// Iteration 2: an index whose bytes differ is rewritten in place — the
/// same file (device and inode), truncated and written, as the retired
/// generator did;
/// no temp file, no rename. (A file replaced between the inspection and
/// the open is refused by a dev+ino comparison inside one process; that
/// window cannot be hit from outside without a race, so it is not
/// exercised here.)
#[test]
fn a_rewrite_keeps_the_file() {
    use std::os::unix::fs::MetadataExt as _;

    for (fixture, _) in FIXTURES {
        let scratch = Scratch::new("export-inode");
        let home = scratch.home("h");
        let root = scratch.copy(fixture, "copy");
        let base = read_text(&root, "specengine.toml");
        let index = index_path(fixture);
        register(&root, &base, index, "gen-index", None);
        let expected = library_render(&root);
        let file = root.join(index);
        write(&root, index, "a hand edit\n");
        let identity = |path: &Path| {
            let metadata = fs::symlink_metadata(path).unwrap();
            (metadata.dev(), metadata.ino())
        };
        let before = identity(&file);
        set_mtime(&file, long_ago());
        let run = spec(&home, &root, &["export", "index"]);
        run.code(0);
        assert_eq!(
            run.stdout,
            format!("wrote {index}: {} bytes\n", expected.len())
        );
        assert!(read(&root, index) == expected.as_bytes(), "{fixture}");
        assert_eq!(
            identity(&file),
            before,
            "{fixture}: the index was replaced, not rewritten"
        );
        assert_ne!(mtime(&file), long_ago(), "{fixture}: not written");
        // Nothing else appeared beside it (no temp file left).
        let parent = file.parent().unwrap();
        let names: Vec<String> = fs::read_dir(parent)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.contains("index"))
            .collect();
        assert_eq!(names, ["index.md"], "{fixture}");
    }
}
