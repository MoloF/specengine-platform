//! docs/features/index-shards.md (ADR-0030), CLI part, on scratch copies of
//! spec-a and spec-b whose configs (written into the copies, never
//! `fixtures/`) register `command = "gen-index"`, `[paths] index` and the
//! shards of each case; every run gets a scratch `HOME`.
//!
//! AC-04: claimed live documents in the claiming shard, the rest plus the
//! pointers in the root; overlapping claims → the first shard in config
//! order (whose path sorts after the second's); a Tier 3 document a claim
//! matches → the archive shard, or without one the claiming shard's Archive
//! section. AC-03 on the same corpora: every walked document but a
//! generated one on exactly one line across the outputs; the outputs, seeded
//! with `class: canon` and with an unclosed front-matter, never listed.
//!
//! AC-09: every output written is `render_index_set`'s; `spec check` then
//! reports no `index-*`; a rerun → every output `unchanged`, every mtime
//! kept; one output edited → only it rewritten; text, JSON (`shards`) and
//! `--stdout` (`==> <path> <==`) in config order, `--stdout` writing nothing.
//!
//! AC-10: a symlink on the way to a shard, a missing shard parent, a shard
//! path that is a directory or a symlink → exit 2, empty stdout, no output
//! created or modified (the stale root's bytes and mtime kept).
//!
//! Also (reported, not a criterion; iteration 2): a read-only stale shard
//! after a stale root → refused at inspection, `cannot open … for writing`,
//! nothing written; a read-only shard holding its render → `unchanged`, not
//! opened; two outputs that are one file (an existing case variant, a hard
//! link, a name in another Unicode form) or whose paths differ only in case
//! → refused, nothing written; an output outside the walk → one `warning:`
//! naming it, `spec check` then `index-missing` on it.

#![cfg(unix)]

mod common;

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _, symlink};
use std::path::Path;
use std::time::{Duration, SystemTime};

use common::check::{codes, index_path, quoted, set_paths_key};
use common::{FIXTURES, Run, Scratch, read, read_text, spec, write};
use specengine_core::ProjectConfig;
use specengine_core::check::{
    CheckConfig, CheckFile, CheckInput, IndexOutput, is_tier3_file, render_index_set,
};
use specengine_store::{WorkingTree, check_input};

const COMMAND: &str = "gen-index";
const ARCHIVE_HEADING: &str = "## Archive \u{2014} Tier 3, by id only";

#[derive(Debug, Clone)]
enum Kind {
    Tier3,
    /// Directories: each claim is `<dir>/**`.
    Claims(Vec<String>),
}

type Shards = Vec<(String, Kind)>;

/// The archive shard in another directory than the root, then a live shard
/// claiming the records.
fn archive_and_claims() -> Shards {
    vec![
        ("docs/features/index-archive.md".to_owned(), Kind::Tier3),
        (
            "docs/spec/index-records.md".to_owned(),
            Kind::Claims(vec!["docs/records".to_owned()]),
        ),
    ]
}

/// No archive shard; two live shards whose claims overlap on the
/// requirement records, the first in config order sorting after the second
/// by path.
fn overlapping(fixture: &str) -> Shards {
    let requirements = if fixture == "spec-a" {
        "docs/records/R"
    } else {
        "docs/records/REQ"
    };
    vec![
        (
            "docs/spec/index-records.md".to_owned(),
            Kind::Claims(vec!["docs/records".to_owned()]),
        ),
        (
            "docs/features/index-records.md".to_owned(),
            Kind::Claims(vec![requirements.to_owned(), "docs/features".to_owned()]),
        ),
    ]
}

/// `base` with `[paths] index = root` and the index entry writing the root
/// and `shards`.
fn config(base: &str, root: &str, shards: &Shards) -> String {
    let mut text = set_paths_key(base, "index", &quoted(root));
    let mut writes = vec![quoted(root)];
    writes.extend(shards.iter().map(|(path, _)| quoted(path)));
    text.push_str(&format!(
        "\n[[generators]]\ncommand = {}\nwrites  = [{}]\nindex   = true\nshards  = [\n",
        quoted(COMMAND),
        writes.join(", ")
    ));
    for (path, kind) in shards {
        match kind {
            Kind::Tier3 => {
                text.push_str(&format!("  {{ path = {}, tier3 = true }},\n", quoted(path)))
            }
            Kind::Claims(dirs) => {
                let claims: Vec<String> = dirs.iter().map(|d| quoted(&format!("{d}/**"))).collect();
                text.push_str(&format!(
                    "  {{ path = {}, claims = [{}] }},\n",
                    quoted(path),
                    claims.join(", ")
                ));
            }
        }
    }
    text.push_str("]\n");
    text
}

/// A scratch copy of `fixture` configured with `shards`; returns the copy
/// and its root output path.
fn sharded_copy(
    scratch: &Scratch,
    fixture: &str,
    dir: &str,
    shards: &Shards,
) -> (std::path::PathBuf, &'static str) {
    let root = scratch.copy(fixture, dir);
    let base = read_text(&root, "specengine.toml");
    let index = index_path(fixture);
    write(&root, "specengine.toml", config(&base, index, shards));
    (root, index)
}

/// The library's walk of `root` and its render set, with its own config.
fn library_set(root: &Path) -> (CheckInput, Vec<IndexOutput>) {
    let text = read_text(root, "specengine.toml");
    let project = ProjectConfig::from_toml(&text).expect("a valid ProjectConfig");
    let check =
        CheckConfig::from_toml(&text).unwrap_or_else(|e| panic!("{}", e.at("specengine.toml")));
    let generator = check.index_generator().expect("an index = true entry");
    let index = project.paths.index.as_deref().expect("[paths] index");
    let tree = WorkingTree::new(root, &project.paths).expect("a working tree");
    let input = check_input(&tree, &project.scheme);
    let outputs = render_index_set(&input, index, generator);
    (input, outputs)
}

/// `link` from the directory of `from`, root-relative.
fn resolve(from: &str, link: &str) -> String {
    let mut parts: Vec<&str> = from.split('/').collect();
    parts.pop();
    for part in link.split('/') {
        match part {
            ".." => assert!(parts.pop().is_some(), "{link} from {from} leaves the root"),
            "." | "" => {}
            part => parts.push(part),
        }
    }
    parts.join("/")
}

/// `target → (output, heading)` of every entry line, pointers aside; `Err`
/// when a target is listed twice.
fn listing(outputs: &[IndexOutput]) -> Result<BTreeMap<String, (String, String)>, String> {
    let mut seen: BTreeMap<String, (String, String)> = BTreeMap::new();
    for output in outputs {
        let mut heading = String::new();
        for line in output.bytes.lines() {
            if line.starts_with("## ") {
                heading = line.to_owned();
                continue;
            }
            let Some(rest) = line.strip_prefix("- [") else {
                continue;
            };
            if heading == "## Shards" {
                continue;
            }
            let (_, rest) = rest.split_once("](").expect("`](`");
            let link = rest.split_once(')').expect("`)`").0;
            let target = resolve(&output.path, link);
            if let Some((first, _)) =
                seen.insert(target.clone(), (output.path.clone(), heading.clone()))
            {
                return Err(format!("{target} in {first} and {}", output.path));
            }
        }
    }
    Ok(seen)
}

fn is_generated(file: &CheckFile) -> bool {
    file.parsed
        .as_ref()
        .and_then(|parsed| parsed.document())
        .and_then(|document| document.fields.as_ref())
        .and_then(|fields| fields.class.as_deref())
        == Some("generated")
}

/// Where AC-04 places `file`: Tier 3 → the archive shard when configured;
/// else the first shard with a matching claim; else the root.
fn expected_output(file: &CheckFile, root: &str, shards: &Shards) -> String {
    let tier3 = file.parsed.as_ref().is_some_and(is_tier3_file);
    if tier3 && let Some((path, _)) = shards.iter().find(|(_, kind)| matches!(kind, Kind::Tier3)) {
        return path.clone();
    }
    for (path, kind) in shards {
        if let Kind::Claims(dirs) = kind
            && dirs
                .iter()
                .any(|dir| file.path.starts_with(&format!("{dir}/")))
        {
            return path.clone();
        }
    }
    root.to_owned()
}

/// Every pointer of the root, in order: `(shard path, link)`.
fn pointers(root: &IndexOutput) -> Vec<(String, String)> {
    let tail = root
        .bytes
        .split_once("\n## Shards\n\n")
        .unwrap_or_else(|| panic!("no `## Shards` in the root:\n{}", root.bytes))
        .1;
    tail.lines()
        .map(|line| {
            let rest = line.strip_prefix("- [").expect("a pointer line");
            let (path, rest) = rest.split_once("](").expect("`](`");
            let link = rest.split_once(')').expect("`)`").0;
            (path.to_owned(), link.to_owned())
        })
        .collect()
}

fn index_codes(home: &Path, root: &Path) -> Vec<String> {
    let check = spec(home, root, &["--json", "check"]);
    codes(&check.stdout)
        .into_iter()
        .filter(|code| code.starts_with("index-"))
        .collect()
}

/// AC-04 and AC-03, then AC-09's first write, on both fixtures and both
/// layouts.
#[test]
fn claims_place_each_document_once_on_both_fixtures() {
    for (fixture, _) in FIXTURES {
        for (layout, shards) in [
            ("archive + claims", archive_and_claims()),
            ("overlapping, no archive", overlapping(fixture)),
        ] {
            let what = format!("{fixture}, {layout}");
            let scratch = Scratch::new("export-shards-place");
            let home = scratch.home("h");
            let (root, index) = sharded_copy(&scratch, fixture, "copy", &shards);
            // The outputs seeded with front-matter that would list them were
            // they excluded by class: the root canon, the first shard unclosed.
            write(
                &root,
                index,
                "---\nclass: canon\ntier: 2\nscope: [x]\n---\n\n# Seeded\n",
            );
            write(
                &root,
                &shards[0].0,
                "---\nclass: spec\nstatus: draft\n\n# Unclosed\n",
            );
            let (input, outputs) = library_set(&root);
            let paths: Vec<&str> = outputs.iter().map(|o| o.path.as_str()).collect();
            let mut want = vec![index];
            want.extend(shards.iter().map(|(path, _)| path.as_str()));
            assert_eq!(paths, want, "{what}: config order");
            assert!(
                input.files.iter().any(|f| f.path == index)
                    && input.files.iter().any(|f| f.path == shards[0].0),
                "{what}: the seeded outputs are walked"
            );

            let listed = listing(&outputs).unwrap_or_else(|e| panic!("{what}: {e}"));
            let mut failures = Vec::new();
            let mut tier3_seen = 0;
            for file in &input.files {
                if paths.contains(&file.path.as_str()) || is_generated(file) {
                    if listed.contains_key(&file.path) {
                        failures.push(format!("{} listed", file.path));
                    }
                    continue;
                }
                let Some((at, heading)) = listed.get(&file.path) else {
                    failures.push(format!("{} not listed", file.path));
                    continue;
                };
                let want = expected_output(file, index, &shards);
                if *at != want {
                    failures.push(format!("{} in {at}, expected {want}", file.path));
                }
                let tier3 = file.parsed.as_ref().is_some_and(is_tier3_file);
                if tier3 {
                    tier3_seen += 1;
                }
                if tier3 != (heading == ARCHIVE_HEADING) {
                    failures.push(format!("{} under {heading:?} (Tier 3: {tier3})", file.path));
                }
            }
            for target in listed.keys() {
                let walked = input.files.iter().find(|f| &f.path == target);
                if !walked.is_some_and(|f| !is_generated(f) && !paths.contains(&f.path.as_str())) {
                    failures.push(format!("{target} listed, not a walked document"));
                }
            }
            assert!(failures.is_empty(), "{what}:\n{}", failures.join("\n"));
            assert_eq!(
                tier3_seen, 1,
                "{what}: the fixture's one superseded decision"
            );

            // The overlap: the requirement records went to the first shard,
            // the claimed Tier 3 to the claiming shard's Archive section.
            if layout.starts_with("overlapping") {
                let requirement = if fixture == "spec-a" {
                    "docs/records/R/R-12.md"
                } else {
                    "docs/records/REQ/REQ-001.md"
                };
                assert_eq!(
                    listed[requirement].0, shards[0].0,
                    "{what}: overlapping claims, the first shard"
                );
                let tier3 = if fixture == "spec-a" {
                    "docs/records/DEC/DEC-0007.md"
                } else {
                    "docs/records/ADR/ADR-0002.md"
                };
                assert_eq!(
                    listed[tier3],
                    (shards[0].0.clone(), ARCHIVE_HEADING.to_owned()),
                    "{what}: the claimed Tier 3 in the claiming shard's Archive"
                );
            } else {
                let tier3 = if fixture == "spec-a" {
                    "docs/records/DEC/DEC-0007.md"
                } else {
                    "docs/records/ADR/ADR-0002.md"
                };
                assert_eq!(
                    listed[tier3].0, shards[0].0,
                    "{what}: a claimed Tier 3 goes to the archive shard"
                );
            }

            // The root: pointers in config order, linked from its directory;
            // no claimed live document and no Tier 3 when archived.
            let root_output = &outputs[0];
            let got = pointers(root_output);
            assert_eq!(
                got.iter().map(|(p, _)| p.as_str()).collect::<Vec<_>>(),
                &want[1..],
                "{what}"
            );
            for (path, link) in &got {
                assert_eq!(&resolve(index, link), path, "{what}: pointer link");
            }
            assert!(
                !listed
                    .iter()
                    .any(|(target, (at, _))| at == index && target.starts_with("docs/records/")),
                "{what}: a claimed record in the root:\n{}",
                root_output.bytes
            );

            // Written as rendered (AC-09), then `spec check` has no `index-*`.
            let run = spec(&home, &root, &["export", "index"]);
            run.code(0);
            let lines: String = outputs
                .iter()
                .map(|o| format!("wrote {}: {} bytes\n", o.path, o.bytes.len()))
                .collect();
            assert_eq!(run.stdout, lines, "{what}");
            assert_eq!(run.stderr, "", "{what}");
            for o in &outputs {
                assert!(
                    read(&root, &o.path) == o.bytes.as_bytes(),
                    "{what}: {}",
                    o.path
                );
            }
            assert_eq!(
                library_set(&root).1,
                outputs,
                "{what}: the render is stable"
            );
            assert!(index_codes(&home, &root).is_empty(), "{what}");
        }
    }
}

fn long_ago() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000_000)
}

fn set_mtime(path: &Path, time: SystemTime) {
    fs::File::options()
        .write(true)
        .open(path)
        .expect("the output opens")
        .set_modified(time)
        .expect("mtime set");
}

fn mtime(path: &Path) -> SystemTime {
    fs::symlink_metadata(path)
        .expect("exists")
        .modified()
        .expect("mtime")
}

/// One entry of the copy: kind, bytes, mtime, permission bits, inode.
type Entry = (String, Option<Vec<u8>>, Option<SystemTime>, u32, u64);

/// Root-relative path → its entry.
type State = BTreeMap<String, Entry>;

/// Every entry under `dir` with its kind, bytes, mtime, permission bits and
/// inode.
fn state(dir: &Path) -> State {
    fn walk(root: &Path, dir: &Path, out: &mut State) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let relative = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .into_owned();
            let metadata = fs::symlink_metadata(&path).unwrap();
            let kind = metadata.file_type();
            let (mode, inode) = (metadata.mode() & 0o7777, metadata.ino());
            if kind.is_symlink() {
                let target = fs::read_link(&path).unwrap();
                out.insert(
                    relative,
                    (
                        format!("link -> {}", target.display()),
                        None,
                        None,
                        mode,
                        inode,
                    ),
                );
            } else if kind.is_dir() {
                out.insert(relative, ("dir".to_owned(), None, None, mode, inode));
                walk(root, &path, out);
            } else {
                out.insert(
                    relative,
                    (
                        "file".to_owned(),
                        fs::read(&path).ok(),
                        metadata.modified().ok(),
                        mode,
                        inode,
                    ),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

/// AC-09.
#[test]
fn export_writes_reports_and_prints_the_whole_set() {
    for (fixture, _) in FIXTURES {
        let scratch = Scratch::new("export-shards-set");
        let home = scratch.home("h");
        let shards = archive_and_claims();
        let (root, index) = sharded_copy(&scratch, fixture, "copy", &shards);
        let (_, outputs) = library_set(&root);
        assert_eq!(outputs.len(), 3, "{fixture}");

        // --stdout: each output after its `==> <path> <==` line, nothing written.
        let before = state(&root);
        let run = spec(&home, &root, &["export", "index", "--stdout"]);
        run.code(0);
        let printed: String = outputs
            .iter()
            .map(|o| format!("==> {} <==\n{}", o.path, o.bytes))
            .collect();
        assert!(
            run.stdout == printed,
            "{fixture}: --stdout:\n{}",
            run.stdout
        );
        assert_eq!(run.stderr, "", "{fixture}");
        assert_eq!(state(&root), before, "{fixture}: --stdout wrote");
        // `--stdout --json` stays a usage error, nothing written.
        let run = spec(&home, &root, &["--json", "export", "index", "--stdout"]);
        assert_eq!(run.code, 2, "{fixture}: {}", run.show());
        assert_eq!(state(&root), before, "{fixture}: --stdout --json wrote");

        // Written, in config order.
        let run = spec(&home, &root, &["export", "index"]);
        run.code(0);
        let wrote: String = outputs
            .iter()
            .map(|o| format!("wrote {}: {} bytes\n", o.path, o.bytes.len()))
            .collect();
        assert_eq!(run.stdout, wrote, "{fixture}");
        for o in &outputs {
            assert!(
                read(&root, &o.path) == o.bytes.as_bytes(),
                "{fixture}: {}",
                o.path
            );
        }
        assert!(index_codes(&home, &root).is_empty(), "{fixture}");

        // A rerun: every output `unchanged`, every mtime kept.
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
            assert_eq!(
                mtime(&root.join(&o.path)),
                long_ago(),
                "{fixture}: {} rewritten",
                o.path
            );
        }
        // JSON: the root's three keys and `shards`, in config order.
        let run = spec(&home, &root, &["--json", "export", "index"]);
        run.code(0);
        assert_eq!(
            run.stdout,
            format!(
                "{{\"path\":\"{index}\",\"bytes\":{},\"written\":false,\"shards\":[{{\"path\":\"{}\",\"bytes\":{},\"written\":false}},{{\"path\":\"{}\",\"bytes\":{},\"written\":false}}]}}\n",
                outputs[0].bytes.len(),
                outputs[1].path,
                outputs[1].bytes.len(),
                outputs[2].path,
                outputs[2].bytes.len(),
            ),
            "{fixture}"
        );
        for o in &outputs {
            assert_eq!(
                mtime(&root.join(&o.path)),
                long_ago(),
                "{fixture}: {} rewritten by --json",
                o.path
            );
        }

        // One output edited: only it rewritten.
        let edited = &outputs[2];
        let file = root.join(&edited.path);
        fs::write(&file, format!("{}x\n", edited.bytes)).unwrap();
        set_mtime(&file, long_ago());
        assert_eq!(index_codes(&home, &root), ["index-drift"], "{fixture}");
        let run = spec(&home, &root, &["export", "index"]);
        run.code(0);
        assert_eq!(
            run.stdout,
            format!(
                "unchanged {}: {} bytes\nunchanged {}: {} bytes\nwrote {}: {} bytes\n",
                outputs[0].path,
                outputs[0].bytes.len(),
                outputs[1].path,
                outputs[1].bytes.len(),
                edited.path,
                edited.bytes.len()
            ),
            "{fixture}"
        );
        assert!(read(&root, &edited.path) == edited.bytes.as_bytes());
        assert_ne!(
            mtime(&file),
            long_ago(),
            "{fixture}: the edited one rewritten"
        );
        for o in &outputs[..2] {
            assert_eq!(
                mtime(&root.join(&o.path)),
                long_ago(),
                "{fixture}: {} rewritten",
                o.path
            );
        }

        // JSON of a write: the deleted shard recreated, `written` per output.
        fs::remove_file(root.join(&outputs[1].path)).unwrap();
        let run = spec(&home, &root, &["export", "--json", "index"]);
        run.code(0);
        assert_eq!(
            run.json(),
            serde_json::json!({
                "path": index,
                "bytes": outputs[0].bytes.len(),
                "written": false,
                "shards": [
                    {"path": outputs[1].path, "bytes": outputs[1].bytes.len(), "written": true},
                    {"path": outputs[2].path, "bytes": outputs[2].bytes.len(), "written": false},
                ]
            }),
            "{fixture}"
        );
        assert!(read(&root, &outputs[1].path) == outputs[1].bytes.as_bytes());
    }
}

/// Exit 2, nothing on stdout, one `spec: …` line holding every piece of
/// `says`, the copy as before (kinds, bytes, mtimes, permissions, inodes).
fn assert_refused(what: &str, run: &Run, root: &Path, before: &State, says: &[&str]) {
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
    assert!(state(root) == *before, "{what}: the copy changed");
}

/// AC-10.
#[test]
fn a_refused_shard_writes_nothing_not_even_the_root() {
    for (fixture, _) in FIXTURES {
        let scratch = Scratch::new("export-shards-refused");
        let home = scratch.home("h");
        let outside = scratch.dir("outside");
        fs::write(outside.join("target.md"), "outside\n").unwrap();
        type Prepare = fn(&Path, &Path);
        let cases: [(&str, &str, Prepare, &[&str]); 4] = [
            (
                "a symlink on the way",
                "docs/linked/index-archive.md",
                |root, _| symlink(root.join("docs/features"), root.join("docs/linked")).unwrap(),
                &["`docs/linked` is a symlink", "nothing was written"],
            ),
            (
                "a missing shard parent",
                "docs/nowhere/index-archive.md",
                |_, _| {},
                &["`docs/nowhere`", "does not exist", "nothing was written"],
            ),
            (
                "a shard path that is a directory",
                "docs/features/index-archive.md",
                |root, _| fs::create_dir(root.join("docs/features/index-archive.md")).unwrap(),
                &[
                    "`docs/features/index-archive.md` is not a regular file",
                    "nothing was written",
                ],
            ),
            (
                "a shard that is a symlink",
                "docs/features/index-archive.md",
                |root, outside| {
                    symlink(
                        outside.join("target.md"),
                        root.join("docs/features/index-archive.md"),
                    )
                    .unwrap()
                },
                &[
                    "`docs/features/index-archive.md` is a symlink",
                    "nothing was written",
                ],
            ),
        ];
        for (at, (what, shard, prepare, says)) in cases.into_iter().enumerate() {
            let what = format!("{fixture}: {what}");
            // The archive shard last, after a live shard that would be created.
            let shards: Shards = vec![
                (
                    "docs/spec/index-records.md".to_owned(),
                    Kind::Claims(vec!["docs/records".to_owned()]),
                ),
                (shard.to_owned(), Kind::Tier3),
            ];
            let (root, index) = sharded_copy(&scratch, fixture, &format!("copy-{at}"), &shards);
            write(&root, index, "stale\n");
            set_mtime(&root.join(index), long_ago());
            prepare(&root, &outside);
            let before = state(&root);
            let run = spec(&home, &root, &["export", "index"]);
            assert_refused(&what, &run, &root, &before, says);
            assert!(
                !root.join("docs/spec/index-records.md").exists(),
                "{what}: a shard created"
            );
            assert_eq!(read(&root, index), b"stale\n", "{what}: the root");
            assert_eq!(
                mtime(&root.join(index)),
                long_ago(),
                "{what}: the root's mtime"
            );
            assert_eq!(
                fs::read(outside.join("target.md")).unwrap(),
                b"outside\n",
                "{what}"
            );
        }
    }
}

const NOTHING_WRITTEN: &str = "; nothing was written";

/// Reported for the review against AC-10 (iteration 2, not a criterion of
/// its own): an existing output whose bytes differ and that cannot be opened
/// for writing — a read-only stale archive shard after a stale root — is
/// refused at inspection, before the first write: exit 2, empty stdout,
/// `cannot open … for writing: …; nothing was written`, every output as it
/// was (bytes, mtime, permissions, inode), the root still stale. A read-only
/// shard that holds its render already is never opened: exit 0, the stale
/// root written, the shard `unchanged` with its mtime, mode and inode kept.
#[test]
fn a_read_only_stale_shard_is_refused_at_inspection_and_a_current_one_is_left_alone() {
    let scratch = Scratch::new("export-shards-read-only");
    let home = scratch.home("h");
    let shards = archive_and_claims();
    let (root, index) = sharded_copy(&scratch, "spec-b", "copy", &shards);
    let (_, outputs) = library_set(&root);
    spec(&home, &root, &["export", "index"]).code(0);

    // A stale root, then a stale read-only archive shard, then the records
    // shard as rendered.
    let archive = root.join(&outputs[1].path);
    write(&root, index, "stale\n");
    fs::write(&archive, "stale shard\n").unwrap();
    for o in &outputs {
        set_mtime(&root.join(&o.path), long_ago());
    }
    fs::set_permissions(&archive, fs::Permissions::from_mode(0o444)).unwrap();
    let before = state(&root);
    let run = spec(&home, &root, &["export", "index"]);
    assert_refused("a read-only stale shard", &run, &root, &before, &[]);
    assert_eq!(
        run.stderr_lines()[0],
        format!(
            "spec: cannot open `{}` for writing: Permission denied (os error 13){NOTHING_WRITTEN}",
            outputs[1].path
        ),
        "{}",
        run.show()
    );
    assert_eq!(read(&root, index), b"stale\n", "the root is still stale");
    assert_eq!(mtime(&root.join(index)), long_ago(), "the root's mtime");

    // The shard current and still read-only, the root stale: the root
    // written, the shard not opened.
    fs::set_permissions(&archive, fs::Permissions::from_mode(0o644)).unwrap();
    fs::write(&archive, &outputs[1].bytes).unwrap();
    set_mtime(&archive, long_ago());
    fs::set_permissions(&archive, fs::Permissions::from_mode(0o444)).unwrap();
    let before = state(&root);
    let run = spec(&home, &root, &["export", "index"]);
    run.code(0);
    assert_eq!(run.stderr, "", "{}", run.show());
    assert_eq!(
        run.stdout,
        format!(
            "wrote {index}: {} bytes\nunchanged {}: {} bytes\nunchanged {}: {} bytes\n",
            outputs[0].bytes.len(),
            outputs[1].path,
            outputs[1].bytes.len(),
            outputs[2].path,
            outputs[2].bytes.len(),
        )
    );
    assert!(
        read(&root, index) == outputs[0].bytes.as_bytes(),
        "the root was written"
    );
    let after = state(&root);
    for o in &outputs[1..] {
        assert_eq!(after[&o.path], before[&o.path], "{} touched", o.path);
    }
    assert_eq!(after[&outputs[1].path].3, 0o444, "the shard's mode");
    assert_eq!(
        after[&outputs[1].path].2,
        Some(long_ago()),
        "the shard's mtime"
    );
}

/// Whether names in `dir` ignore case, and Unicode form, as APFS's default
/// does.
fn insensitive(dir: &Path) -> (bool, bool) {
    let probe = dir.join("probe-caf\u{e9}");
    fs::write(&probe, "probe\n").unwrap();
    let case = dir.join("PROBE-CAF\u{c9}").exists();
    let form = dir.join("probe-cafe\u{301}").exists();
    fs::remove_file(&probe).unwrap();
    (case, form)
}

/// Reported for the review against AC-10 (iteration 2, not a criterion of
/// its own): two outputs that are one file are refused before any write —
/// exit 2, empty stdout, nothing created or modified: an existing shard
/// whose name differs from the root's only in case (APFS: one file), a
/// shard hard-linked to the root, two shards whose names differ only in
/// Unicode form (APFS: one file); and two absent shards whose paths differ
/// only in case.
#[test]
fn two_outputs_that_are_one_file_are_refused_before_any_write() {
    let scratch = Scratch::new("export-shards-one-file");
    let home = scratch.home("h");
    let (case_insensitive, form_insensitive) = insensitive(&scratch.dir("probe"));
    let records = || Kind::Claims(vec!["docs/records".to_owned()]);
    let one_file = |a: &str, b: &str| {
        format!(
            "spec: the index outputs `{a}` and `{b}` are one file on disk (names differing \
             in case or Unicode form, or a hard link): each output needs a file of its \
             own{NOTHING_WRITTEN}"
        )
    };
    let case_only = |a: &str, b: &str| {
        format!(
            "spec: the index outputs `{a}` and `{b}` differ only in case: one file on a \
             case-insensitive file system; each output needs a path of its own{NOTHING_WRITTEN}"
        )
    };
    type Prepare = fn(&Path, &str);
    let nfc = "docs/features/caf\u{e9}.md";
    let nfd = "docs/features/cafe\u{301}.md";
    let mut cases: Vec<(&str, Shards, bool, Prepare, String)> = vec![
        (
            "an existing case variant of the stale root",
            vec![
                ("docs/INDEX.md".to_owned(), Kind::Tier3),
                ("docs/spec/index-records.md".to_owned(), records()),
            ],
            true,
            |root, _| {
                if !root.join("docs/INDEX.md").exists() {
                    write(root, "docs/INDEX.md", "stale\n");
                }
            },
            if case_insensitive {
                one_file("docs/index.md", "docs/INDEX.md")
            } else {
                case_only("docs/index.md", "docs/INDEX.md")
            },
        ),
        (
            "an existing case variant of the current root",
            vec![
                ("docs/INDEX.md".to_owned(), Kind::Tier3),
                ("docs/spec/index-records.md".to_owned(), records()),
            ],
            false,
            |root, _| {
                if !root.join("docs/INDEX.md").exists() {
                    write(root, "docs/INDEX.md", "stale\n");
                }
            },
            if case_insensitive {
                one_file("docs/index.md", "docs/INDEX.md")
            } else {
                case_only("docs/index.md", "docs/INDEX.md")
            },
        ),
        (
            "a shard hard-linked to the root",
            archive_and_claims(),
            true,
            |root, index| {
                fs::hard_link(
                    root.join(index),
                    root.join("docs/features/index-archive.md"),
                )
                .unwrap()
            },
            one_file("docs/index.md", "docs/features/index-archive.md"),
        ),
        (
            "two absent shards differing only in case",
            vec![
                ("docs/spec/index-records.md".to_owned(), records()),
                (
                    "docs/spec/Index-Records.md".to_owned(),
                    Kind::Claims(vec!["docs/features".to_owned()]),
                ),
            ],
            true,
            |_, _| {},
            case_only("docs/spec/index-records.md", "docs/spec/Index-Records.md"),
        ),
    ];
    if form_insensitive {
        cases.push((
            "two existing shards differing only in Unicode form",
            vec![(nfc.to_owned(), Kind::Tier3), (nfd.to_owned(), records())],
            true,
            |root, _| write(root, "docs/features/caf\u{e9}.md", "stale shard\n"),
            one_file(nfc, nfd),
        ));
    } else {
        eprintln!("not exercised here: names in another Unicode form are other files");
    }
    // Every case runs; the failing ones are named at the end.
    let mut failed = Vec::new();
    for (at, (what, shards, stale, prepare, expected)) in cases.into_iter().enumerate() {
        let (root, index) = sharded_copy(&scratch, "spec-b", &format!("copy-{at}"), &shards);
        let (_, outputs) = library_set(&root);
        if stale {
            write(&root, index, "stale\n");
        } else {
            write(&root, index, &outputs[0].bytes);
        }
        prepare(&root, index);
        let before = state(&root);
        let run = spec(&home, &root, &["export", "index"]);
        let checked = std::panic::catch_unwind(|| {
            assert_refused(what, &run, &root, &before, &[]);
            assert_eq!(run.stderr_lines()[0], expected, "{what}");
            assert!(
                !root.join("docs/spec/index-records.md").exists(),
                "{what}: a shard created"
            );
        });
        if checked.is_err() {
            failed.push(what);
        }
    }
    assert!(failed.is_empty(), "failed: {failed:?}");
}

/// AC-06 through the CLI (iteration 2): a backtick in a claim, or a control
/// character in a shard path, refuses `spec export index` and `spec check`
/// at the config line, the value shown escaped; nothing written.
#[test]
fn a_claim_or_shard_path_that_would_break_the_markdown_is_refused_at_its_line() {
    let scratch = Scratch::new("export-shards-markdown");
    let home = scratch.home("h");
    let cases: [(&str, Shards, &str); 2] = [
        (
            "a backtick in a claim",
            vec![(
                "docs/spec/index-records.md".to_owned(),
                Kind::Claims(vec!["docs/rec`ords".to_owned()]),
            )],
            r#"shard `claims`: "docs/rec`ords/**" contains a backtick: the index renders it into Markdown"#,
        ),
        (
            "a tab in a shard path",
            vec![(
                "docs/spec/index\trecords.md".to_owned(),
                Kind::Claims(vec!["docs/records".to_owned()]),
            )],
            r#"shard `path`: "docs/spec/index\trecords.md" contains a newline or a control character: the index renders it into Markdown"#,
        ),
    ];
    for (at, (what, shards, message)) in cases.into_iter().enumerate() {
        let (root, index) = sharded_copy(&scratch, "spec-b", &format!("copy-{at}"), &shards);
        write(&root, index, "stale\n");
        let text = read_text(&root, "specengine.toml");
        let line = 1 + text
            .lines()
            .position(|l| l.starts_with("  { path = "))
            .expect("the shard line");
        let expected = format!("specengine.toml:{line}: {message}");
        let before = state(&root);
        let run = spec(&home, &root, &["export", "index"]);
        assert_eq!(run.code, 2, "{what}:\n{}", run.show());
        assert_eq!(run.stdout, "", "{what}");
        assert_eq!(run.stderr_lines(), [expected.as_str()], "{what}");
        assert!(state(&root) == before, "{what}: the copy changed");
        let check = spec(&home, &root, &["check"]);
        assert_eq!(check.code, 2, "{what}: check:\n{}", check.show());
        assert!(
            check.stderr_lines().contains(&expected.as_str())
                || check.stdout.lines().any(|l| l.contains(&expected)),
            "{what}: check names it:\n{}",
            check.show()
        );
    }
}

/// "One `warning:` per output outside the walk, naming it": written anyway,
/// and `spec check` reports it `index-missing`.
#[test]
fn a_shard_outside_the_walk_is_written_with_a_warning() {
    let scratch = Scratch::new("export-shards-outside");
    let home = scratch.home("h");
    let shards: Shards = vec![
        ("site/index-archive.md".to_owned(), Kind::Tier3),
        (
            "docs/spec/index-records.md".to_owned(),
            Kind::Claims(vec!["docs/records".to_owned()]),
        ),
    ];
    // spec-b walks `docs` only.
    let (root, _) = sharded_copy(&scratch, "spec-b", "copy", &shards);
    fs::create_dir(root.join("site")).unwrap();
    let (_, outputs) = library_set(&root);
    let run = spec(&home, &root, &["export", "index"]);
    run.code(0);
    let lines = run.stderr_lines();
    assert_eq!(lines.len(), 1, "{}", run.stderr);
    assert!(
        lines[0].starts_with("warning: ") && lines[0].contains("`site/index-archive.md`"),
        "{}",
        lines[0]
    );
    for o in &outputs {
        assert!(read(&root, &o.path) == o.bytes.as_bytes(), "{}", o.path);
    }
    let check = spec(&home, &root, &["--json", "check"]);
    let json: serde_json::Value = serde_json::from_str(&check.stdout).unwrap();
    let missing: Vec<&str> = json["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["code"].as_str().is_some_and(|c| c.starts_with("index-")))
        .map(|f| f["path"].as_str().unwrap())
        .collect();
    assert_eq!(missing, ["site/index-archive.md"]);
}
