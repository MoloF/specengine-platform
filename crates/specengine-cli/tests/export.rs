//! AC-09 and AC-11 of docs/features/spec-cli-check.md: `spec export index
//! [--stdout]` on copies of spec-a and spec-b whose configs (written into
//! the copies, never `fixtures/`) register `command = "gen-index"`,
//! `index = true` and a `[paths] index` inside the walk. The written file
//! is the library's `render_index` (header naming the registered command
//! and gate, never the binary), after which `spec check` reports neither
//! `index-drift` nor `index-missing`; equal bytes are not rewritten (mtime
//! kept); `--stdout` prints the same bytes and writes nothing. Refusals exit
//! 2 with nothing on stdout and the index unchanged or absent.
//!
//! docs/features/roadmap-view.md RD-5 (the section at the end): the
//! roadmap view on copies of `roadmap-a` and `roadmap-b`.

#![cfg(unix)]

mod common;

use std::fs;
use std::os::unix::fs::{PermissionsExt as _, symlink};
use std::path::{Path, PathBuf};
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

// ------------------------------------------------------------------ roadmap view
// docs/features/roadmap-view.md RD-5, on scratch copies of `roadmap-a` and
// `roadmap-b` whose configs (written into the copies) add `[paths] index`,
// an archive shard and the `roadmap` view in a subdirectory: the view is
// written last, `unchanged` when equal (mtime kept), alone rewritten when
// edited; `--stdout` with a view uses `==> <path> <==` even without a
// shard; the JSON's top-level `roadmap` (never under `shards`), absent
// without a view; a missing view directory refuses every output; a view
// outside the walk is written with one `warning:`.

/// `(fixture, view)`: the view in a subdirectory of the walk.
const VIEWED: [(&str, &str); 2] = [
    ("roadmap-a", "docs/views/roadmap.md"),
    ("roadmap-b", "docs/generated/work/plan.md"),
];

const VIEW_INDEX: &str = "docs/index.md";
const VIEW_ARCHIVE: &str = "docs/index-archive.md";

/// `base` with `[paths] index` and the index entry: the archive shard when
/// `shard`, the view at `view` when given.
fn view_entry(base: &str, shard: bool, view: Option<&str>) -> String {
    use common::check::quoted;
    let mut text = set_paths_key(base, "index", &quoted(VIEW_INDEX));
    let mut writes = vec![quoted(VIEW_INDEX)];
    if shard {
        writes.push(quoted(VIEW_ARCHIVE));
    }
    if let Some(view) = view {
        writes.push(quoted(view));
    }
    text.push_str(&format!(
        "\n[[generators]]\ncommand = \"gen-index\"\nwrites  = [{}]\nindex   = true\n",
        writes.join(", ")
    ));
    if shard {
        text.push_str(&format!(
            "shards  = [{{ path = {}, tier3 = true }}]\n",
            quoted(VIEW_ARCHIVE)
        ));
    }
    if let Some(view) = view {
        text.push_str(&format!("roadmap = {}\n", quoted(view)));
    }
    text
}

/// A scratch copy of `fixture` configured by [`view_entry`], the view's
/// directory created.
fn view_copy(scratch: &Scratch, fixture: &str, shard: bool, view: Option<&str>) -> PathBuf {
    let root = scratch.copy(fixture, "copy");
    let base = read_text(&root, "specengine.toml");
    write(&root, "specengine.toml", view_entry(&base, shard, view));
    if let Some(view) = view {
        fs::create_dir_all(root.join(view).parent().unwrap()).unwrap();
    }
    root
}

/// The library's render set of `root` with its own config.
fn library_set(root: &Path) -> Vec<specengine_core::check::IndexOutput> {
    use specengine_core::ProjectConfig;
    use specengine_core::check::{CheckConfig, render_index_set};
    use specengine_store::{WorkingTree, check_input};
    let text = read_text(root, "specengine.toml");
    let project = ProjectConfig::from_toml(&text).expect("a valid ProjectConfig");
    let check =
        CheckConfig::from_toml(&text).unwrap_or_else(|e| panic!("{}", e.at("specengine.toml")));
    let generator = check.index_generator().expect("an index = true entry");
    let tree = WorkingTree::new(root, &project.paths).expect("a working tree");
    let input = check_input(&tree, &project.scheme);
    render_index_set(&input, VIEW_INDEX, generator)
}

fn view_index_codes(home: &Path, root: &Path) -> Vec<(String, String)> {
    let check = spec(home, root, &["--json", "check"]);
    let json: serde_json::Value = serde_json::from_str(&check.stdout).expect("a JSON report");
    json["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["code"].as_str().is_some_and(|c| c.starts_with("index-")))
        .map(|f| {
            (
                f["code"].as_str().unwrap().to_owned(),
                f["path"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

/// RD-5: written last, after the root and the shard; a rerun `unchanged`
/// with every mtime kept; the JSON's `roadmap` beside `shards`; a hand edit
/// to the view rewrites it alone; a deleted view recreated, `written` true.
#[test]
fn the_view_is_written_last_and_reported_in_text_and_json() {
    for (fixture, view) in VIEWED {
        let scratch = Scratch::new("export-view-set");
        let home = scratch.home("h");
        let root = view_copy(&scratch, fixture, true, Some(view));
        let outputs = library_set(&root);
        let paths: Vec<&str> = outputs.iter().map(|o| o.path.as_str()).collect();
        assert_eq!(paths, [VIEW_INDEX, VIEW_ARCHIVE, view], "{fixture}");

        let run = spec(&home, &root, &["export", "index"]);
        run.code(0);
        let wrote: String = outputs
            .iter()
            .map(|o| format!("wrote {}: {} bytes\n", o.path, o.bytes.len()))
            .collect();
        assert_eq!(run.stdout, wrote, "{fixture}: the view last");
        assert_eq!(run.stderr, "", "{fixture}");
        for o in &outputs {
            assert!(read(&root, &o.path) == o.bytes.as_bytes(), "{}", o.path);
        }
        assert_eq!(view_index_codes(&home, &root), [], "{fixture}");

        // A rerun: every output `unchanged`, no mtime touched.
        for o in &outputs {
            set_mtime(&root.join(&o.path), long_ago());
        }
        let run = spec(&home, &root, &["export", "index"]);
        run.code(0);
        let unchanged: String = outputs
            .iter()
            .map(|o| format!("unchanged {}: {} bytes\n", o.path, o.bytes.len()))
            .collect();
        assert_eq!(run.stdout, unchanged, "{fixture}");
        for o in &outputs {
            assert_eq!(mtime(&root.join(&o.path)), long_ago(), "{}", o.path);
        }

        // JSON: `roadmap` a top-level key after `shards`, the same three keys.
        let run = spec(&home, &root, &["--json", "export", "index"]);
        run.code(0);
        assert_eq!(
            run.stdout,
            format!(
                "{{\"path\":\"{VIEW_INDEX}\",\"bytes\":{},\"written\":false,\"shards\":[{{\"path\":\"{VIEW_ARCHIVE}\",\"bytes\":{},\"written\":false}}],\"roadmap\":{{\"path\":\"{view}\",\"bytes\":{},\"written\":false}}}}\n",
                outputs[0].bytes.len(),
                outputs[1].bytes.len(),
                outputs[2].bytes.len(),
            ),
            "{fixture}"
        );

        // The view edited by hand: drift on it alone, then it alone rewritten.
        let file = root.join(view);
        fs::write(&file, format!("{}- a hand edit\n", outputs[2].bytes)).unwrap();
        set_mtime(&file, long_ago());
        assert_eq!(
            view_index_codes(&home, &root),
            [("index-drift".to_owned(), view.to_owned())],
            "{fixture}"
        );
        let run = spec(&home, &root, &["export", "index"]);
        run.code(0);
        assert_eq!(
            run.stdout,
            format!(
                "unchanged {VIEW_INDEX}: {} bytes\nunchanged {VIEW_ARCHIVE}: {} bytes\nwrote {view}: {} bytes\n",
                outputs[0].bytes.len(),
                outputs[1].bytes.len(),
                outputs[2].bytes.len()
            ),
            "{fixture}"
        );
        assert!(read(&root, view) == outputs[2].bytes.as_bytes());
        assert_ne!(mtime(&file), long_ago(), "{fixture}: the view rewritten");
        for o in &outputs[..2] {
            assert_eq!(mtime(&root.join(&o.path)), long_ago(), "{}", o.path);
        }

        // Deleted: recreated, `written` true in the JSON.
        fs::remove_file(&file).unwrap();
        assert_eq!(
            view_index_codes(&home, &root),
            [("index-missing".to_owned(), view.to_owned())],
            "{fixture}"
        );
        let run = spec(&home, &root, &["--json", "export", "index"]);
        run.code(0);
        assert_eq!(
            run.json(),
            serde_json::json!({
                "path": VIEW_INDEX,
                "bytes": outputs[0].bytes.len(),
                "written": false,
                "shards": [{"path": VIEW_ARCHIVE, "bytes": outputs[1].bytes.len(), "written": false}],
                "roadmap": {"path": view, "bytes": outputs[2].bytes.len(), "written": true},
            }),
            "{fixture}"
        );
        assert!(read(&root, view) == outputs[2].bytes.as_bytes());
    }
}

/// RD-5: `--stdout` with a view and no shard prints both outputs after
/// their `==> <path> <==` lines, writes nothing; the JSON without shards
/// has `roadmap` and no `shards`.
#[test]
fn stdout_with_a_view_and_no_shard_names_each_output() {
    for (fixture, view) in VIEWED {
        let scratch = Scratch::new("export-view-stdout");
        let home = scratch.home("h");
        let root = view_copy(&scratch, fixture, false, Some(view));
        let outputs = library_set(&root);
        assert_eq!(outputs.len(), 2, "{fixture}");
        let before = snapshot(&root);
        let run = spec(&home, &root, &["export", "index", "--stdout"]);
        run.code(0);
        assert_eq!(
            run.stdout,
            format!(
                "==> {VIEW_INDEX} <==\n{}==> {view} <==\n{}",
                outputs[0].bytes, outputs[1].bytes
            ),
            "{fixture}"
        );
        assert_eq!(run.stderr, "", "{fixture}");
        assert_eq!(snapshot(&root), before, "{fixture}: --stdout wrote");

        let run = spec(&home, &root, &["--json", "export", "index"]);
        run.code(0);
        let json = run.json();
        assert_eq!(
            json,
            serde_json::json!({
                "path": VIEW_INDEX,
                "bytes": outputs[0].bytes.len(),
                "written": true,
                "roadmap": {"path": view, "bytes": outputs[1].bytes.len(), "written": true},
            }),
            "{fixture}"
        );
    }
}

/// RD-5 and "Rules": without the `roadmap` key the export is 1a's — one
/// root line, JSON without `roadmap` — and the index root is the same bytes
/// as with the view registered (the view is listed nowhere).
#[test]
fn without_a_view_the_export_is_unchanged() {
    for (fixture, view) in VIEWED {
        for shard in [false, true] {
            let scratch = Scratch::new("export-view-none");
            let home = scratch.home("h");
            let root = view_copy(&scratch, fixture, shard, None);
            let run = spec(&home, &root, &["--json", "export", "index"]);
            run.code(0);
            let json = run.json();
            let keys: Vec<&String> = json.as_object().unwrap().keys().collect();
            // `serde_json`'s map: keys in byte order.
            let want = if shard {
                vec!["bytes", "path", "shards", "written"]
            } else {
                vec!["bytes", "path", "written"]
            };
            assert_eq!(keys, want, "{fixture} shard {shard}: no `roadmap` key");
            let without = read(&root, VIEW_INDEX);

            // The same copy with the view: the root is the same bytes.
            let base = read_text(&scratch.copy(fixture, "fresh"), "specengine.toml");
            write(
                &root,
                "specengine.toml",
                view_entry(&base, shard, Some(view)),
            );
            fs::create_dir_all(root.join(view).parent().unwrap()).unwrap();
            let run = spec(&home, &root, &["export", "index"]);
            run.code(0);
            assert!(
                run.stdout.starts_with(&format!("unchanged {VIEW_INDEX}: ")),
                "{fixture}: {}",
                run.stdout
            );
            assert!(read(&root, VIEW_INDEX) == without, "{fixture}");
            assert!(
                !String::from_utf8_lossy(&without).contains(view),
                "{fixture}: the index lists the view"
            );
        }
    }
}

/// RD-5 and "Rules": the view's directory missing → exit 2, nothing on
/// stdout, one `spec:` line naming the view, no output written — the index
/// neither created nor, when stale, rewritten.
#[test]
fn a_missing_view_directory_refuses_every_output() {
    for (fixture, view) in VIEWED {
        let scratch = Scratch::new("export-view-nodir");
        let home = scratch.home("h");
        let root = view_copy(&scratch, fixture, true, Some(view));
        fs::remove_dir_all(root.join(view).parent().unwrap()).unwrap();
        let run = spec(&home, &root, &["export", "index"]);
        assert_refused(
            &format!("{fixture}: no view directory"),
            &run,
            &root.join(VIEW_INDEX),
            None,
            &[view],
        );
        assert!(
            !root.join(VIEW_ARCHIVE).exists(),
            "{fixture}: shard written"
        );

        // A stale index and shard: kept byte for byte.
        write(&root, VIEW_INDEX, "stale\n");
        write(&root, VIEW_ARCHIVE, "stale shard\n");
        let run = spec(&home, &root, &["export", "index"]);
        assert_refused(
            &format!("{fixture}: stale index, no view directory"),
            &run,
            &root.join(VIEW_INDEX),
            Some(b"stale\n"),
            &[view],
        );
        assert_eq!(read(&root, VIEW_ARCHIVE), b"stale shard\n", "{fixture}");
        assert!(
            !root.join(view).parent().unwrap().exists(),
            "{fixture}: the directory was created"
        );
    }
}

/// RD-5: a view outside the walk is written with one `warning:` naming it;
/// `spec check` then reports it `index-missing`.
#[test]
fn a_view_outside_the_walk_is_written_with_a_warning() {
    for (fixture, _) in VIEWED {
        let view = "site/roadmap.md";
        let scratch = Scratch::new("export-view-outside");
        let home = scratch.home("h");
        let root = view_copy(&scratch, fixture, false, Some(view));
        let outputs = library_set(&root);
        let run = spec(&home, &root, &["export", "index"]);
        run.code(0);
        let lines = run.stderr_lines();
        assert_eq!(lines.len(), 1, "{fixture}: {}", run.stderr);
        assert!(
            lines[0].starts_with("warning: ") && lines[0].contains(&format!("`{view}`")),
            "{fixture}: {}",
            lines[0]
        );
        for o in &outputs {
            assert!(read(&root, &o.path) == o.bytes.as_bytes(), "{}", o.path);
        }
        assert_eq!(
            view_index_codes(&home, &root),
            [("index-missing".to_owned(), view.to_owned())],
            "{fixture}"
        );
    }
}
