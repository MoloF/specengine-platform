//! docs/features/reference-promotion.md (RM-0138, ADR-0089), the core's
//! half: `[references] homes` read by `ReferencesConfig`
//! (docs/canon/work-references-promotion-surfaces.md "`[references] homes`"),
//! the pure insertion of an entry into a document's `references:` key (core
//! `promote::insert_entry`; docs/canon/work-references-promotion-surfaces.md
//! "The `references:` key"), the typed key's place in `TYPED_KEYS` (35), and
//! PR9's source scan: the promotion code names no kind of any fixture's or
//! preset's `[ids]`, no home directory and no path literal (ADR-0008).
//!
//! - `[references] homes`: a location home with `dir = "media"`, a
//!   persona home without `dir`; `home_for` picks by kind; each error of
//!   the spec's list at its line, naming its entry, kind or directory.
//! - PR9: a source scan of the promotion code (core `reference.rs` and every
//!   `*promot*` source of core, store, CLI, MCP and http) finds no literal
//!   equal to a fixture's or a preset's kind, no `PREFIX-` of a fixture, no
//!   `media` or `art` word and no `docs/` or `spec/` literal; the kind name
//!   `reference` only in `REFERENCE_KIND`. M: a default `dir = "media"` in
//!   core.
//! - "Insertion" (PR14's pure half): the line `- "<entry>"` at the items'
//!   indent after the key's last item, CRLF when the front-matter's first
//!   line has one, the key added before the closing fence, `references: []`
//!   made a block; a flow list, a scalar, no front-matter, a broken one and
//!   a kept path refused with their lines; `\` and `"` escaped; every other
//!   byte equal.
//!
//! Every expected value is worked out from the spec, never read back from a
//! run. Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use specengine_core::front_matter::TYPED_KEYS;
use specengine_core::intake::REFERENCE_KIND;
use specengine_core::reference::{Home, ReferencesConfig};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("the repository root")
}

/// `[ids]` declaring the four kinds of `fixtures/promote-mini`, then `rest`.
fn with_ids(rest: &str) -> String {
    format!(
        "[project]\nslug = \"promote-mini\"\n\n[ids]\nLOC  = {{ kind = \"location\",  shape = \"name\" }}\n\
         CHR  = {{ kind = \"persona\",   shape = \"name\" }}\nMEC  = {{ kind = \"mechanic\",  shape = \"name\" }}\n\
         RULE = {{ kind = \"rule\",      shape = \"name\" }}\n\n{rest}"
    )
}

/// The 1-based line of the first `needle` in `text` at or after line `from`.
fn line_of(text: &str, needle: &str, from: usize) -> usize {
    text.lines()
        .enumerate()
        .skip(from.saturating_sub(1))
        .find(|(_, line)| line.contains(needle))
        .map(|(at, _)| at + 1)
        .unwrap_or_else(|| panic!("{needle:?} in {text}"))
}

/// `ReferencesConfig::from_toml(text)`'s error as `specengine.toml:<line>:
/// <message>`.
fn error_of(text: &str) -> String {
    ReferencesConfig::from_toml(text)
        .expect_err(text)
        .at("specengine.toml")
}

/// The error of `text` is `message` at one of `lines` (the spec says "at
/// their line": the entry's header or the offending key).
#[track_caller]
fn assert_at(text: &str, lines: &[usize], message: &str) {
    let error = error_of(text);
    assert!(
        lines
            .iter()
            .any(|line| error == format!("specengine.toml:{line}: {message}")),
        "expected `{message}` at one of lines {lines:?}, got `{error}` for:\n{text}"
    );
}

// ------------------------------------------------------------ homes

/// "Data", "`[references] homes`": a home per written entry, in order;
/// `dir` omitted is `None` (the document's own directory); `home_for` picks
/// the home listing a kind, none for a kind no home lists; `allow` stays
/// RM-0137's (absent: every type).
#[test]
fn homes_are_read_in_order_and_picked_by_kind() {
    let text = with_ids(
        "[[references.homes]]\nkinds = [\"location\"]\ndir   = \"media\"\n\n\
         [[references.homes]]\nkinds = [\"persona\", \"rule\"]\n",
    );
    let config = ReferencesConfig::from_toml(&text)
        .expect("read")
        .expect("a table with homes");
    assert_eq!(
        config.homes,
        vec![
            Home {
                kinds: vec!["location".to_owned()],
                dir: Some("media".to_owned()),
            },
            Home {
                kinds: vec!["persona".to_owned(), "rule".to_owned()],
                dir: None,
            },
        ]
    );
    assert_eq!(
        config
            .home_for("location")
            .and_then(|home| home.dir.clone()),
        Some("media".to_owned())
    );
    assert_eq!(
        config.home_for("rule").map(|home| home.dir.clone()),
        Some(None),
        "dir omitted: the document's own directory"
    );
    assert!(config.home_for("mechanic").is_none(), "no home lists it");
    assert!(
        config.allows(specengine_core::blob::Media::Wav),
        "no allow: every type, as RM-0137"
    );

    // The fixture's own table reads, with its two homes.
    let fixture =
        fs::read_to_string(repository_root().join("fixtures/promote-mini/specengine.toml"))
            .expect("the fixture's config");
    let homes = ReferencesConfig::from_toml(&fixture)
        .expect("the fixture's table reads")
        .expect("homes")
        .homes;
    assert_eq!(
        homes
            .iter()
            .map(|home| (home.kinds.clone(), home.dir.clone()))
            .collect::<Vec<_>>(),
        vec![
            (vec!["location".to_owned()], Some("media".to_owned())),
            (vec!["persona".to_owned()], Some("art".to_owned())),
        ]
    );
}

/// "Data": each error of the spec's list at its line. A kind is judged
/// against the same text's `[ids]`; a kind in two homes names both entries.
#[test]
fn every_home_error_is_named_at_its_line() {
    // No kind.
    let text = with_ids("[[references.homes]]\nkinds = []\ndir = \"media\"\n");
    let header = line_of(&text, "[[references.homes]]", 1);
    assert_at(
        &text,
        &[header, header + 1],
        "`[[references.homes]]` entry 1 lists no kind",
    );
    // The second entry, no kind key at all.
    let text = with_ids(
        "[[references.homes]]\nkinds = [\"location\"]\n\n[[references.homes]]\ndir = \"art\"\n",
    );
    let second = line_of(&text, "[[references.homes]]", header + 1);
    assert_at(
        &text,
        &[second, second + 1],
        "`[[references.homes]]` entry 2 lists no kind",
    );

    // 33 kinds in one home; 33 homes.
    let many: Vec<String> = (0..33).map(|n| format!("\"k{n}\"")).collect();
    let text = with_ids(&format!(
        "[[references.homes]]\nkinds = [{}]\n",
        many.join(", ")
    ));
    let kinds_line = line_of(&text, "kinds = [", 1);
    assert_at(&text, &[kinds_line], "a list holds at most 32 entries");
    let mut homes = String::new();
    for _ in 0..33 {
        homes.push_str("[[references.homes]]\nkinds = [\"location\"]\n\n");
    }
    let error = error_of(&with_ids(&homes));
    assert!(
        error.contains("a list holds at most 32 entries") || error.contains("is in two homes"),
        "33 homes are refused: {error}"
    );

    // A kind no `[ids]` prefix declares.
    let text = with_ids("[[references.homes]]\nkinds = [\"location\", \"harbour\"]\n");
    assert_at(
        &text,
        &[line_of(&text, "kinds = [", 1)],
        "kind `harbour` is declared by no `[ids]` prefix",
    );
    // A kind in two homes.
    let text = with_ids(
        "[[references.homes]]\nkinds = [\"location\"]\n\n[[references.homes]]\nkinds = [\"persona\", \"location\"]\ndir = \"art\"\n",
    );
    let second_kinds = line_of(&text, "kinds = [\"persona\"", 1);
    assert_at(
        &text,
        &[second_kinds, second_kinds - 1],
        "kind `location` is in two homes (entries 1 and 2)",
    );

    // `dir` no clean relative path: each form of the canon's list.
    for dir in [
        "/media",
        "../media",
        "media/../art",
        "./media",
        "media/.",
        "media//art",
        "media/",
        "media\\\\art",
        "",
        "media\\u0007",
        // git's own directory, any case (the developer's iteration 3: the
        // spec's line for a `.git` part).
        ".git",
        "media/.git",
        "media/.GIT/x",
    ] {
        let text = with_ids(&format!(
            "[[references.homes]]\nkinds = [\"location\"]\ndir = \"{dir}\"\n"
        ));
        let shown: String = toml::from_str::<toml::Table>(&format!("d = \"{dir}\""))
            .expect("a TOML string")["d"]
            .as_str()
            .expect("a string")
            .to_owned();
        let error = error_of(&text);
        let line = line_of(&text, "dir = ", 1);
        assert!(
            error.starts_with(&format!("specengine.toml:{line}: ")),
            "{dir:?}: at the dir's line: {error}"
        );
        assert!(
            error.contains("is no clean relative path"),
            "{dir:?}: {error}"
        );
        if shown.chars().all(|c| !c.is_control()) && !shown.is_empty() {
            assert!(
                error.contains(&format!("`dir` `{shown}` is no clean relative path")),
                "{dir:?}: names the directory: {error}"
            );
        }
    }
    // 255 bytes pass, 256 do not.
    let long = "d".repeat(255);
    let text = with_ids(&format!(
        "[[references.homes]]\nkinds = [\"location\"]\ndir = \"{long}\"\n"
    ));
    ReferencesConfig::from_toml(&text).expect("255 bytes is a dir");
    let longer = "d".repeat(256);
    let text = with_ids(&format!(
        "[[references.homes]]\nkinds = [\"location\"]\ndir = \"{longer}\"\n"
    ));
    assert_at(
        &text,
        &[line_of(&text, "dir = ", 1)],
        "`dir` is 256 bytes: at most 255",
    );
    // An unknown key.
    let text = with_ids("[[references.homes]]\nkinds = [\"location\"]\nfolder = \"media\"\n");
    assert_at(
        &text,
        &[line_of(&text, "folder = ", 1)],
        "unknown key `folder` in `[[references.homes]]`",
    );
}

/// "Data": a home does not touch `allow`'s rules, and `allow`'s errors stay
/// first-class beside homes (RM-0137's table unchanged).
#[test]
fn allow_and_homes_live_together() {
    let text = with_ids(
        "[references]\nallow = [\"image\", \"audio\", \"model\"]\n\n[[references.homes]]\nkinds = [\"location\"]\ndir = \"media\"\n",
    );
    let config = ReferencesConfig::from_toml(&text)
        .expect("read")
        .expect("a table");
    assert!(config.allows(specengine_core::blob::Media::Png));
    assert!(!config.allows(specengine_core::blob::Media::Pdf));
    assert_eq!(config.homes.len(), 1);
    let text = with_ids(
        "[references]\nallow = [\"image/svg+xml\"]\n\n[[references.homes]]\nkinds = [\"location\"]\n",
    );
    let error = error_of(&text);
    assert!(
        error.contains("\"image/svg+xml\" is not a type SpecEngine stores"),
        "{error}"
    );
}

/// "Pins that move": `PROMOTED_MAX` 32, `IN_SPEC_MAX` 64, `PLAIN_FILE_MAX`
/// 50 MiB (D6); "Data": the path at most 1 024 bytes, the text 512
/// characters, the context's 20 lines within 2 048 characters.
#[test]
fn the_constants_are_the_specs() {
    use specengine_core::promote::{
        ENTRY_PATH_MAX, ENTRY_TEXT_MAX, IN_SPEC_MAX, PLAIN_FILE_MAX, PROMOTED_MAX,
        SPEC_REFS_CONTEXT, SPEC_REFS_CONTEXT_CHARS,
    };
    assert_eq!(PROMOTED_MAX, 32);
    assert_eq!(IN_SPEC_MAX, 64);
    assert_eq!(PLAIN_FILE_MAX, 50 * 1024 * 1024);
    assert_eq!(ENTRY_PATH_MAX, 1_024);
    assert_eq!(ENTRY_TEXT_MAX, 512);
    assert_eq!(SPEC_REFS_CONTEXT, 20);
    assert_eq!(SPEC_REFS_CONTEXT_CHARS, 2_048);
}

/// "Pins that move": `TYPED_KEYS` 35, `references` among them, a list.
#[test]
fn references_is_the_35th_typed_key() {
    assert_eq!(TYPED_KEYS.len(), 35);
    assert!(
        TYPED_KEYS.iter().any(|(key, _)| *key == "references"),
        "{:?}",
        TYPED_KEYS.iter().map(|(key, _)| *key).collect::<Vec<_>>()
    );
    assert_eq!(REFERENCE_KIND, "reference");
}

// ------------------------------------------------------------- PR9

/// Every fixture's and preset's kinds (commented examples too) and every
/// fixture's prefixes and aliases.
fn vocabulary() -> (BTreeSet<String>, BTreeSet<String>) {
    fn configs(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(dir).expect("a directory").flatten() {
            let path = entry.path();
            if path.is_dir() && depth > 0 {
                configs(&path, depth - 1, out);
            } else if entry.file_name() == "specengine.toml" {
                out.push(path);
            }
        }
    }
    let mut files = Vec::new();
    configs(&repository_root().join("fixtures"), 2, &mut files);
    let (mut kinds, mut prefixes) = (BTreeSet::new(), BTreeSet::new());
    for file in &files {
        let text = fs::read_to_string(file).expect("a config");
        let Ok(table) = text.parse::<toml::Table>() else {
            continue;
        };
        let Some(ids) = table.get("ids").and_then(toml::Value::as_table) else {
            continue;
        };
        for (prefix, spec) in ids {
            prefixes.insert(prefix.clone());
            if let Some(kind) = spec.get("kind").and_then(toml::Value::as_str) {
                kinds.insert(kind.to_lowercase());
            }
            for alias in spec
                .get("aliases_from")
                .and_then(toml::Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(toml::Value::as_str)
            {
                prefixes.insert(alias.to_owned());
            }
        }
    }
    // The presets' kinds, their commented examples too.
    for entry in fs::read_dir(repository_root().join("scripts/presets"))
        .expect("scripts/presets")
        .flatten()
    {
        let path = entry.path();
        if path.extension().is_none_or(|ext| ext != "toml") {
            continue;
        }
        let text = fs::read_to_string(&path).expect("a preset");
        for line in text.lines() {
            let line = line.trim_start_matches(['#', ' ']);
            for key in ["kind = \"", "kind=\""] {
                let mut rest = line;
                while let Some(start) = rest.find(key) {
                    let after = &rest[start + key.len()..];
                    if let Some(end) = after.find('"') {
                        kinds.insert(after[..end].to_lowercase());
                    }
                    rest = &rest[start + key.len()..];
                }
            }
            if let Some(list) = line.strip_prefix("kinds = [") {
                for kind in list.split('"').skip(1).step_by(2) {
                    kinds.insert(kind.to_lowercase());
                }
            }
        }
    }
    for class in ["canon", "decision", "spec", "generated"] {
        kinds.remove(class);
    }
    (kinds, prefixes)
}

/// The string literals of `line` (none on a comment line).
fn literals(line: &str) -> Vec<&str> {
    if line.trim_start().starts_with("//") {
        return Vec::new();
    }
    line.split('"').skip(1).step_by(2).collect()
}

/// A source up to its own `#[cfg(test)] mod` (over fixtures, by design).
fn scanned(path: &Path) -> String {
    let whole = fs::read_to_string(path).unwrap_or_else(|error| panic!("{path:?}: {error}"));
    whole
        .find("\n#[cfg(test)]\nmod ")
        .map_or(whole.clone(), |at| whole[..at].to_owned())
}

/// The promotion code: core `reference.rs` (the homes' reader) and every
/// source whose name holds `promot` under the five crates' `src`.
fn promotion_sources() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(dir).expect("a src directory").flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|ext| ext == "rs")
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.contains("promot"))
            {
                out.push(path);
            }
        }
    }
    let root = repository_root();
    let mut out = vec![root.join("crates/specengine-core/src/reference.rs")];
    for krate in ["core", "store", "cli", "mcp", "http"] {
        walk(
            &root.join(format!("crates/specengine-{krate}/src")),
            &mut out,
        );
    }
    out.sort();
    out.dedup();
    out
}

/// Whether `literal` names a home directory: the word `media` or `art`,
/// but `media type(s)`, the MIME words RM-0137's messages use (`list
/// classes or media types`), which name no directory.
fn names_home(literal: &str) -> bool {
    let words = words(literal);
    words.iter().enumerate().any(|(at, word)| {
        if word.eq_ignore_ascii_case("art") {
            return true;
        }
        word.eq_ignore_ascii_case("media")
            && !words.get(at + 1).is_some_and(|next| {
                next.eq_ignore_ascii_case("type") || next.eq_ignore_ascii_case("types")
            })
    })
}

/// The words of a literal: runs of letters, digits, `_` and `-`.
fn words(literal: &str) -> Vec<&str> {
    literal
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
        .filter(|word| !word.is_empty())
        .collect()
}

/// PR9: no literal of the promotion code is a fixture's or preset's kind, a
/// fixture prefix's `PREFIX-`, a `media` or `art` word, or holds `docs/` or
/// `spec/`; no literal `"reference"` but `REFERENCE_KIND`'s own. M: a
/// default `dir = "media"` in core.
#[test]
fn pr9_the_promotion_code_names_no_kind_home_or_path() {
    let (kinds, prefixes) = vocabulary();
    for kind in [
        "location", "persona", "mechanic", "rule", "scene", "practice",
    ] {
        assert!(kinds.contains(kind), "{kind} is read: {kinds:?}");
    }
    assert!(prefixes.contains("LOC") && prefixes.contains("CHR"));
    let sources = promotion_sources();
    let root = repository_root();
    let names: Vec<String> = sources
        .iter()
        .map(|path| {
            path.strip_prefix(&root)
                .expect("under the root")
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert!(
        names
            .iter()
            .any(|name| name.starts_with("crates/specengine-core/src/") && name.contains("promot")),
        "core's promotion module is scanned: {names:?}"
    );
    assert!(
        names
            .iter()
            .any(|name| name.starts_with("crates/specengine-cli/src/") && name.contains("promot")),
        "the CLI's promotion module is scanned: {names:?}"
    );
    let mut offenders = Vec::new();
    let mut seen = 0;
    for (path, name) in sources.iter().zip(&names) {
        let text = scanned(path);
        for (number, line) in text.lines().enumerate() {
            for literal in literals(line) {
                seen += 1;
                let at = format!("{name}:{}: {literal:?}", number + 1);
                let lower = literal.trim().to_lowercase();
                if kinds.contains(&lower) {
                    offenders.push(format!("{at} (a kind)"));
                }
                if lower == REFERENCE_KIND {
                    offenders.push(format!("{at} (the kind's name: use REFERENCE_KIND)"));
                }
                if prefixes
                    .iter()
                    .any(|prefix| literal.contains(&format!("{prefix}-")) && prefix.len() > 1)
                {
                    // A prefix of one letter (`R-`) is too common a pair.
                    offenders.push(format!("{at} (a fixture prefix)"));
                }
                if names_home(literal) {
                    offenders.push(format!("{at} (a home directory)"));
                }
                // A URL (`https://git-lfs.github.com/spec/v1`, the pointer's
                // version line) is no repository path.
                if !literal.contains("://")
                    && (literal.contains("docs/") || literal.contains("spec/"))
                {
                    offenders.push(format!("{at} (a path)"));
                }
            }
        }
    }
    assert!(seen > 50, "{seen} literals scanned in {names:?}");
    assert!(
        offenders.is_empty(),
        "project words in the promotion code:\n{}",
        offenders.join("\n")
    );
    // The scan sees M's literal, and the narrowings let only what they name.
    assert_eq!(literals("    dir: Some(\"media\".to_owned()),"), ["media"]);
    assert!(names_home("media"));
    assert!(names_home("media/harbour.png"));
    assert!(names_home("art"));
    assert!(!names_home("list classes or media types, or leave it out"));
    assert!(!names_home("start the part"));
}

// ------------------------------------------------------- insertion
//
// The pure half of PR14 and "Insertion", through core
// `promote::insert_entry(bytes, entry)`.

use specengine_core::promote::insert_entry;

/// `insert_entry`'s bytes, which must be produced.
#[track_caller]
fn inserted(before: &str, entry: &str) -> String {
    let bytes = insert_entry(before.as_bytes(), entry)
        .unwrap_or_else(|refusal| panic!("{entry:?} into {before:?}: {refusal}"));
    String::from_utf8(bytes).expect("UTF-8")
}

/// `insert_entry`'s refusal, as text.
#[track_caller]
fn refusal(before: &str, entry: &str) -> String {
    match insert_entry(before.as_bytes(), entry) {
        Ok(bytes) => panic!(
            "{entry:?} into {before:?} inserted: {:?}",
            String::from_utf8_lossy(&bytes)
        ),
        Err(refusal) => refusal.to_string(),
    }
}

const DOC_HEAD: &str = "---\nid: LOC-HARBOUR\nclass: canon\ntitle: The harbour\n";
const DOC_BODY: &str = "---\n\n# The harbour\n\nreferences:\n  - \"not/front-matter.png\"\n";

/// "Insertion": no key: `references:` and `  - "<entry>"` before the closing
/// fence; a block key: the line after its last item at the items' indent;
/// `references: []` a block of one; every other byte equal (the body holding
/// a `references:` line is no key).
#[test]
fn an_entry_goes_after_the_last_item_or_before_the_fence() {
    let before = format!("{DOC_HEAD}{DOC_BODY}");
    assert_eq!(
        inserted(&before, "media/harbour.png"),
        format!("{DOC_HEAD}references:\n  - \"media/harbour.png\"\n{DOC_BODY}"),
        "no key: added before the closing fence"
    );

    let block = format!(
        "{DOC_HEAD}references:\n    - \"media/a.png\"\n    - media/b.wav | the bell\nowner: owner\n{DOC_BODY}"
    );
    assert_eq!(
        inserted(&block, "media/c.png | the light at dusk"),
        format!(
            "{DOC_HEAD}references:\n    - \"media/a.png\"\n    - media/b.wav | the bell\n    - \"media/c.png | the light at dusk\"\nowner: owner\n{DOC_BODY}"
        ),
        "after the last item, at the items' indent (4 here)"
    );

    let empty = format!("{DOC_HEAD}references: []\nowner: owner\n{DOC_BODY}");
    assert_eq!(
        inserted(&empty, "media/harbour.png"),
        format!("{DOC_HEAD}references:\n  - \"media/harbour.png\"\nowner: owner\n{DOC_BODY}"),
        "`references: []` becomes a block of one"
    );
}

/// "Insertion": `\` and `"` escaped in the written line.
#[test]
fn backslash_and_quote_are_escaped() {
    let before = format!("{DOC_HEAD}{DOC_BODY}");
    assert_eq!(
        inserted(&before, "media/a.png | the \"old\" light \\ dusk"),
        format!(
            "{DOC_HEAD}references:\n  - \"media/a.png | the \\\"old\\\" light \\\\ dusk\"\n{DOC_BODY}"
        )
    );
}

/// "Insertion" (PR14): a CRLF front-matter gets a CRLF line; an LF body
/// after it stays LF.
#[test]
fn a_crlf_front_matter_gets_a_crlf_line() {
    let before = "---\r\nid: LOC-HARBOUR\r\nreferences:\r\n  - \"media/a.png\"\r\nowner: owner\r\n---\n\n# The harbour\n";
    assert_eq!(
        inserted(before, "media/b.png"),
        "---\r\nid: LOC-HARBOUR\r\nreferences:\r\n  - \"media/a.png\"\r\n  - \"media/b.png\"\r\nowner: owner\r\n---\n\n# The harbour\n"
    );
    let no_key = "---\r\nid: LOC-HARBOUR\r\n---\r\n\r\n# The harbour\r\n";
    assert_eq!(
        inserted(no_key, "media/b.png"),
        "---\r\nid: LOC-HARBOUR\r\nreferences:\r\n  - \"media/b.png\"\r\n---\r\n\r\n# The harbour\r\n"
    );
}

/// "Insertion": each refusal of the spec's list, nothing produced. The
/// lines name the document by `<path>`, which the pure function does not
/// know: the messages' fixed words are matched.
#[test]
fn every_other_form_is_refused_with_its_fix() {
    let no_front = "# The harbour\n\nNo front-matter here.\n";
    let said = refusal(no_front, "media/a.png");
    assert!(
        said.contains("has no front-matter: a reference is listed in its `references:` key"),
        "{said}"
    );

    let broken = "---\nid: LOC-HARBOUR\nreferences: [unclosed\n---\n\n# The harbour\n";
    let said = refusal(broken, "media/a.png");
    assert!(
        said.contains("front-matter does not read: fix it first"),
        "{said}"
    );

    for form in [
        "references: [a.png]\n",
        "references: \"a.png\"\n",
        "references: a.png\n",
        "references: &list\n  - a.png\n",
    ] {
        let before = format!("{DOC_HEAD}{form}{DOC_BODY}");
        let said = refusal(&before, "media/b.png");
        assert!(
            said.contains(
                "references: is not a block list: write it as `references:` and `- ` lines"
            ),
            "{form:?}: {said}"
        );
    }

    let kept = format!("{DOC_HEAD}references:\n  - \"media/a.png | the boats\"\n{DOC_BODY}");
    let said = refusal(&kept, "media/a.png | the light");
    assert!(said.contains("is already listed in"), "{said}");
    assert!(said.contains("references"), "{said}");
}

// ------------------------------------------------------ the review's

/// The review's finding 8: the hand-written digests against their
/// published answers (FIPS 180-2's `abc`, the empty input, the two-block
/// 448-bit message, a million `a`s fed in uneven pieces) and git's blob IDs
/// (`git hash-object`'s, SHA-1 and SHA-256 object formats). M: a wrong
/// padding length (the two-block vector).
#[test]
fn review_the_digests_answer_the_known_vectors() {
    use specengine_core::digest::{BlobId, Sha1, Sha256};
    let two_blocks = "abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq";
    let sha256 = |pieces: &[&[u8]]| {
        let mut hasher = Sha256::new();
        for piece in pieces {
            hasher.update(piece);
        }
        hasher.hex()
    };
    let sha1 = |pieces: &[&[u8]]| {
        let mut hasher = Sha1::new();
        for piece in pieces {
            hasher.update(piece);
        }
        hasher.hex()
    };
    assert_eq!(
        sha256(&[]),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        sha256(&[b"abc"]),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        sha256(&[two_blocks.as_bytes()]),
        "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
    );
    assert_eq!(sha1(&[]), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
    assert_eq!(sha1(&[b"abc"]), "a9993e364706816aba3e25717850c26c9cd0d89d");
    assert_eq!(
        sha1(&[two_blocks.as_bytes()]),
        "84983e441c3bd26ebaae4aa1f95129e5e54670f1"
    );
    // A million `a`s in pieces of 1, 63, 64, 65 and the rest: every block
    // boundary crossed from each side.
    let million = vec![b'a'; 1_000_000];
    let pieces: Vec<&[u8]> = vec![
        &million[..1],
        &million[1..64],
        &million[64..128],
        &million[128..193],
        &million[193..],
    ];
    assert_eq!(
        sha256(&pieces),
        "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
    );
    assert_eq!(sha1(&pieces), "34aa973cd4c4daa4f61eeb2bdbad27316534016f");
    // git's blob IDs: `blob <size>` NUL, then the bytes.
    let blob = |format: &str, bytes: &[u8]| {
        let mut id = BlobId::new(format, bytes.len() as u64).expect("a format");
        id.update(bytes);
        id.hex()
    };
    assert_eq!(
        blob("sha1", b""),
        "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391"
    );
    assert_eq!(
        blob("sha1", b"hello\n"),
        "ce013625030ba8dba906f756967f9e9ca394464a"
    );
    assert_eq!(
        blob("sha256", b""),
        "473a0f4c3be8a93681a267e3b1e9a7dcda1185436fe141f7749120a303721813"
    );
    assert_eq!(
        blob("sha256", b"hello\n"),
        "2cf8d83d9ee29543b34a87727421fdecb7e3f3a183d337639025de576db9ebb4"
    );
    assert!(BlobId::new("md5", 0).is_none());
}

/// The review's finding 1 (a blocker): entries whose names end in
/// multibyte characters (Cyrillic `\u{430}\u{440}\u{442}/...`, Latin-1
/// `art/\u{e9}\u{e9}`, a Cyrillic `.md` name) are judged, never a panic:
/// the first two kept, the third a Markdown file. M: the byte slice in `is_markdown`.
#[test]
fn review_a_multibyte_entry_is_judged_without_a_panic() {
    use specengine_core::promote::judge_entries;
    use specengine_model::FmValue;
    let cyrillic = "\u{430}\u{440}\u{442}/\u{43c}\u{438}\u{440}\u{430}";
    let latin = "art/\u{e9}\u{e9}";
    let markdown = "\u{43c}\u{438}\u{440}\u{430}.md";
    let short = "\u{e9}";
    let written: Vec<FmValue> = [cyrillic, latin, markdown, short]
        .iter()
        .map(|entry| FmValue::Str((*entry).to_owned()))
        .collect();
    let judged = std::panic::catch_unwind(|| judge_entries(&written))
        .unwrap_or_else(|_| panic!("M: judge_entries panicked on a multibyte entry"));
    let kept: Vec<&str> = judged
        .kept
        .iter()
        .map(|entry| entry.path.as_str())
        .collect();
    assert_eq!(kept, [cyrillic, latin, short], "{judged:?}");
    assert_eq!(judged.problems.len(), 1, "{judged:?}");
    assert_eq!(judged.problems[0].entry, 3);
    assert!(
        judged.problems[0].message.contains("names a Markdown file"),
        "{judged:?}"
    );
}
