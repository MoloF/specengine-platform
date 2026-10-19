//! docs/features/layer-c-resolve.md R5 and R10 over the sources, with
//! comments and `#[cfg(test)] mod` blocks left out:
//!
//! - R5 `only_resolve_spawns_layer_c`: the binary's name `specengine-ra` is
//!   a literal of CLI modules only (the command's own data, as `serve.rs`
//!   names `specengine-http`); no other CLI module reaches the module that
//!   spawns it on the group core; and no module of any crate's `src/` (the
//!   daemon's and MCP's included) names a function that reaches the
//!   spawn, but `lib.rs` (its `mod` and `pub use`) and `main.rs` (the
//!   command's one dispatch). M: a call from `health`.
//! - R10 `no_scip_or_cargo_literal_outside_code_and_ra`: no string or char
//!   literal of the store, core or CLI `src/` holds `Cargo.lock`,
//!   `rust-toolchain`, `.cargo/config` (so neither `.toml` form),
//!   `build-script-build` or `build_script_build`, nor `Cargo.toml` but
//!   `code_walk.rs`' `MANIFEST`; and the slice's new CLI and store modules
//!   (every `src/` file not there at 0a03df7, [`BEFORE`]) hold no literal
//!   equal to `/`, `#`, `.`, `!`, `[`, `]`, `().`, nor one holding
//!   `impl#[`. M: `agrees` split at `/` in the CLI; a `Cargo.lock` literal
//!   in the CLI.
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use common::repository_root;

/// The binary `spec resolve` spawns, as a literal names it.
const RA_BIN: &str = "specengine-ra";

/// The crate of the binary itself: its name is its own data there.
const RA_CRATE: &str = "crates/specengine-ra/";

const CLI_SRC: &str = "crates/specengine-cli/src/";
const CLI_LIB: &str = "crates/specengine-cli/src/lib.rs";
const CLI_MAIN: &str = "crates/specengine-cli/src/main.rs";
const STORE_SRC: &str = "crates/specengine-store/src/";

/// The group core (docs/canon/layer-c.md "Run").
const GROUP_SOURCE: &str = "crates/specengine-cli/src/group.rs";

/// 8a-1's test runner: it spawns the project's test command on the group
/// core, never the binary.
const TEST_RUNNER: &str = "crates/specengine-cli/src/test_runner.rs";

/// The CLI's and the store's modules at 0a03df7, before the slice: every
/// other file of their `src/` is the slice's.
const BEFORE: &[&str] = &[
    "crates/specengine-cli/src/amend.rs",
    "crates/specengine-cli/src/apply.rs",
    "crates/specengine-cli/src/bump.rs",
    "crates/specengine-cli/src/bundle.rs",
    "crates/specengine-cli/src/cap.rs",
    "crates/specengine-cli/src/check.rs",
    "crates/specengine-cli/src/code_index.rs",
    "crates/specengine-cli/src/code_walk.rs",
    "crates/specengine-cli/src/corpus.rs",
    "crates/specengine-cli/src/create.rs",
    "crates/specengine-cli/src/decide.rs",
    "crates/specengine-cli/src/documents.rs",
    "crates/specengine-cli/src/drift.rs",
    "crates/specengine-cli/src/drift_apply.rs",
    "crates/specengine-cli/src/events.rs",
    "crates/specengine-cli/src/export.rs",
    "crates/specengine-cli/src/follow_up.rs",
    "crates/specengine-cli/src/gate.rs",
    "crates/specengine-cli/src/graph.rs",
    "crates/specengine-cli/src/health.rs",
    "crates/specengine-cli/src/health_code.rs",
    "crates/specengine-cli/src/inbox.rs",
    "crates/specengine-cli/src/init.rs",
    "crates/specengine-cli/src/intake.rs",
    "crates/specengine-cli/src/interpret.rs",
    "crates/specengine-cli/src/lib.rs",
    "crates/specengine-cli/src/links.rs",
    "crates/specengine-cli/src/location.rs",
    "crates/specengine-cli/src/lock.rs",
    "crates/specengine-cli/src/lock_regen.rs",
    "crates/specengine-cli/src/main.rs",
    "crates/specengine-cli/src/node_set.rs",
    "crates/specengine-cli/src/package.rs",
    "crates/specengine-cli/src/preflight.rs",
    "crates/specengine-cli/src/project.rs",
    "crates/specengine-cli/src/proposals.rs",
    "crates/specengine-cli/src/propose.rs",
    "crates/specengine-cli/src/refresh.rs",
    "crates/specengine-cli/src/refs.rs",
    "crates/specengine-cli/src/review.rs",
    "crates/specengine-cli/src/revisions.rs",
    "crates/specengine-cli/src/roles.rs",
    "crates/specengine-cli/src/round.rs",
    "crates/specengine-cli/src/search.rs",
    "crates/specengine-cli/src/serve.rs",
    "crates/specengine-cli/src/show.rs",
    "crates/specengine-cli/src/stage.rs",
    "crates/specengine-cli/src/state.rs",
    "crates/specengine-cli/src/state_file.rs",
    "crates/specengine-cli/src/symbols.rs",
    "crates/specengine-cli/src/task.rs",
    "crates/specengine-cli/src/test_runner.rs",
    "crates/specengine-cli/src/tree.rs",
    "crates/specengine-cli/src/unmapped.rs",
    "crates/specengine-cli/src/verify.rs",
    "crates/specengine-cli/src/verify_rows.rs",
    "crates/specengine-cli/src/verify_tests.rs",
    "crates/specengine-store/src/base.rs",
    "crates/specengine-store/src/check.rs",
    "crates/specengine-store/src/code.rs",
    "crates/specengine-store/src/dump.rs",
    "crates/specengine-store/src/error.rs",
    "crates/specengine-store/src/git.rs",
    "crates/specengine-store/src/index.rs",
    "crates/specengine-store/src/lib.rs",
    "crates/specengine-store/src/queue.rs",
    "crates/specengine-store/src/queue/gate.rs",
    "crates/specengine-store/src/queue/stage.rs",
    "crates/specengine-store/src/queue/state.rs",
    "crates/specengine-store/src/queue/tasks.rs",
    "crates/specengine-store/src/read.rs",
    "crates/specengine-store/src/roles.rs",
    "crates/specengine-store/src/rows.rs",
    "crates/specengine-store/src/schema.rs",
    "crates/specengine-store/src/search.rs",
    "crates/specengine-store/src/source.rs",
    "crates/specengine-store/src/symbols.rs",
    "crates/specengine-store/src/update.rs",
    "crates/specengine-store/src/verify_git.rs",
    "crates/specengine-store/src/worktree.rs",
    "crates/specengine-store/src/write.rs",
];

/// R10's Cargo words: no literal of the store, core or CLI holds one.
const CARGO_WORDS: [&str; 5] = [
    "Cargo.lock",
    "rust-toolchain",
    ".cargo/config",
    "build-script-build",
    "build_script_build",
];

/// The one `Cargo.toml` literal of the engine: 6a's manifest name.
const MANIFEST_HOLDER: &str = "crates/specengine-cli/src/code_walk.rs";

/// SCIP's descriptor punctuation: no literal of the slice's new modules
/// equals one (the descriptors are read in `specengine-code`).
const SCIP_PUNCTUATION: [&str; 7] = ["/", "#", ".", "!", "[", "]", "()."];

/// A literal of a source (a string or a char): its char offset and its
/// text as written between its quotes.
struct Literal {
    at: usize,
    text: String,
}

/// A source with its comments and literals blanked to spaces (line ends
/// kept, every other non-ASCII character a space: char offsets equal
/// byte offsets), and its literals.
struct Scanned {
    code: String,
    literals: Vec<Literal>,
}

/// Lexes `source`: line and nested block comments; plain, byte and raw
/// strings (any number of `#`); char and byte-char literals, lifetimes
/// told apart.
fn scan(source: &str) -> Scanned {
    let chars: Vec<char> = source.chars().collect();
    let mut code = chars.clone();
    let mut literals = Vec::new();
    let blank = |code: &mut [char], from: usize, to: usize| {
        for c in &mut code[from..to] {
            if *c != '\n' {
                *c = ' ';
            }
        }
    };
    let mut at = 0;
    while at < chars.len() {
        let c = chars[at];
        let next = chars.get(at + 1).copied();
        if c == '/' && next == Some('/') {
            let start = at;
            while at < chars.len() && chars[at] != '\n' {
                at += 1;
            }
            blank(&mut code, start, at);
            continue;
        }
        if c == '/' && next == Some('*') {
            let start = at;
            let mut depth = 1;
            at += 2;
            while at < chars.len() && depth > 0 {
                if chars[at] == '/' && chars.get(at + 1) == Some(&'*') {
                    depth += 1;
                    at += 2;
                } else if chars[at] == '*' && chars.get(at + 1) == Some(&'/') {
                    depth -= 1;
                    at += 2;
                } else {
                    at += 1;
                }
            }
            let stop = at.min(chars.len());
            blank(&mut code, start, stop);
            at = stop;
            continue;
        }
        let word_before = at > 0 && (chars[at - 1].is_alphanumeric() || chars[at - 1] == '_');
        let raw_from = if word_before {
            None
        } else if c == 'r' {
            Some(at + 1)
        } else if c == 'b' && next == Some('r') {
            Some(at + 2)
        } else {
            None
        };
        if let Some(mut quote) = raw_from {
            let mut hashes = 0;
            while chars.get(quote) == Some(&'#') {
                hashes += 1;
                quote += 1;
            }
            if chars.get(quote) == Some(&'"') {
                let start = quote + 1;
                let mut end = start;
                while end < chars.len()
                    && !(chars[end] == '"'
                        && (0..hashes).all(|k| chars.get(end + 1 + k) == Some(&'#')))
                {
                    end += 1;
                }
                literals.push(Literal {
                    at,
                    text: chars[start..end.min(chars.len())].iter().collect(),
                });
                let stop = (end + 1 + hashes).min(chars.len());
                blank(&mut code, at, stop);
                at = stop;
                continue;
            }
        }
        let quote = if c == '"' {
            Some(at)
        } else if !word_before && c == 'b' && next == Some('"') {
            Some(at + 1)
        } else {
            None
        };
        if let Some(quote) = quote {
            let mut text = String::new();
            let mut end = quote + 1;
            while end < chars.len() && chars[end] != '"' {
                if chars[end] == '\\' && end + 1 < chars.len() {
                    text.push(chars[end]);
                    text.push(chars[end + 1]);
                    end += 2;
                } else {
                    text.push(chars[end]);
                    end += 1;
                }
            }
            literals.push(Literal { at, text });
            let stop = (end + 1).min(chars.len());
            blank(&mut code, at, stop);
            at = stop;
            continue;
        }
        let tick = if c == '\'' {
            Some(at)
        } else if !word_before && c == 'b' && next == Some('\'') {
            Some(at + 1)
        } else {
            None
        };
        if let Some(tick) = tick {
            let close = if chars.get(tick + 1) == Some(&'\\') {
                (tick + 3..chars.len()).find(|&i| chars[i] == '\'')
            } else if chars.get(tick + 2) == Some(&'\'') {
                Some(tick + 2)
            } else {
                None
            };
            if let Some(close) = close {
                literals.push(Literal {
                    at,
                    text: chars[tick + 1..close].iter().collect(),
                });
                blank(&mut code, at, close + 1);
                at = close + 1;
                continue;
            }
        }
        at += 1;
    }
    let code: String = code
        .into_iter()
        .map(|c| if c.is_ascii() { c } else { ' ' })
        .collect();
    Scanned { code, literals }
}

/// The byte ranges of `code`'s `#[cfg(test)] mod NAME { … }` blocks.
fn test_modules(code: &str) -> Vec<(usize, usize)> {
    const ATTRIBUTE: &str = "#[cfg(test)]";
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(found) = code[from..].find(ATTRIBUTE) {
        let start = from + found;
        let after = start + ATTRIBUTE.len();
        let rest = &code[after..];
        let body = rest.trim_start();
        let mut next = start + 1;
        let brace = body.strip_prefix("mod ").and_then(|named| {
            named
                .find(['{', ';'])
                .filter(|at| named[*at..].starts_with('{'))
        });
        if let Some(brace) = brace {
            let open = after + (rest.len() - body.len()) + "mod ".len() + brace;
            let mut depth = 0usize;
            let mut end = code.len();
            for (offset, c) in code[open..].char_indices() {
                if c == '{' {
                    depth += 1;
                } else if c == '}' {
                    depth -= 1;
                    if depth == 0 {
                        end = open + offset + 1;
                        break;
                    }
                }
            }
            out.push((start, end));
            next = end;
        }
        from = next;
    }
    out
}

/// A source file's code (comments, literals and test modules blanked) and
/// its literals outside test modules.
fn read_source(file: &str) -> (String, Vec<Literal>) {
    let text = fs::read_to_string(repository_root().join(file))
        .unwrap_or_else(|error| panic!("{file}: {error}"));
    let scanned = scan(&text);
    let tests = test_modules(&scanned.code);
    let inside = |at: usize| {
        tests
            .iter()
            .any(|(start, end)| (*start..*end).contains(&at))
    };
    let literals = scanned
        .literals
        .into_iter()
        .filter(|literal| !inside(literal.at))
        .collect();
    let mut code: Vec<char> = scanned.code.chars().collect();
    for (start, end) in &tests {
        for c in &mut code[*start..*end] {
            if *c != '\n' {
                *c = ' ';
            }
        }
    }
    (code.into_iter().collect(), literals)
}

/// Every `.rs` file under `dir` (repository-relative, `/`), sorted.
fn sources_under(dir: &str) -> Vec<String> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(root, &path, out);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                let relative = path.strip_prefix(root).expect("under the repository");
                out.push(relative.to_str().expect("a UTF-8 path").replace('\\', "/"));
            }
        }
    }
    let root = repository_root();
    let mut out = Vec::new();
    walk(&root, &root.join(dir), &mut out);
    out.sort();
    out
}

/// Every crate's `src/` files, the binary's own crate aside.
fn engine_sources() -> Vec<String> {
    let root = repository_root();
    let mut crates: Vec<String> = fs::read_dir(root.join("crates"))
        .expect("crates/")
        .flatten()
        .filter(|entry| entry.path().join("src").is_dir())
        .map(|entry| format!("crates/{}/src", entry.file_name().to_string_lossy()))
        .filter(|dir| !format!("{dir}/").starts_with(RA_CRATE))
        .collect();
    crates.sort();
    crates.iter().flat_map(|dir| sources_under(dir)).collect()
}

fn is_ident(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// The byte offsets where `word` stands as a whole path segment of `code`
/// (no identifier character on either side).
fn segments(code: &str, word: &str) -> Vec<usize> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(found) = code[from..].find(word) {
        let at = from + found;
        let before = code[..at].chars().next_back();
        let after = code[at + word.len()..].chars().next();
        if !before.is_some_and(is_ident) && !after.is_some_and(is_ident) {
            out.push(at);
        }
        from = at + word.len();
    }
    out
}

/// The path segment before `at` in `code` when `at` follows `::`.
fn segment_before(code: &str, at: usize) -> Option<&str> {
    let head = code[..at].trim_end().strip_suffix("::")?.trim_end();
    let start = head
        .char_indices()
        .rev()
        .find(|(_, c)| !is_ident(*c))
        .map_or(0, |(offset, c)| offset + c.len_utf8());
    Some(&head[start..])
}

/// The identifiers of every `use <prefix>::{…}` group of `code`.
fn use_group_names(code: &str, prefix: &str) -> BTreeSet<String> {
    let opener = format!("use {prefix}::{{");
    let mut names = BTreeSet::new();
    let mut from = 0;
    while let Some(found) = code[from..].find(&opener) {
        let open = from + found + opener.len() - 1;
        let mut depth = 0usize;
        let mut end = code.len();
        for (offset, c) in code[open..].char_indices() {
            if c == '{' {
                depth += 1;
            } else if c == '}' {
                depth -= 1;
                if depth == 0 {
                    end = open + offset;
                    break;
                }
            }
        }
        for name in code[open + 1..end].split(|c: char| !is_ident(c)) {
            if !name.is_empty() {
                names.insert(name.to_owned());
            }
        }
        from = end;
    }
    names
}

/// The module a CLI source file is, as `lib.rs` declares it: its first
/// path component under `src/` (`resolve.rs`, `resolve/run.rs`: `resolve`).
fn module_of(file: &str) -> String {
    let relative = file.strip_prefix(CLI_SRC).expect("a CLI source");
    let first = relative.split('/').next().expect("a component");
    first.strip_suffix(".rs").unwrap_or(first).to_owned()
}

/// The names `code` imports from the crate's module `module`: `use
/// crate::module::{…};`, `use crate::module::name;` (and `super::`), and
/// the names of a `use crate::{…}` group naming `module` (nested paths
/// read loosely: every identifier of the group).
fn imported_from(code: &str, module: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for opener in [
        format!("use crate::{module}::"),
        format!("use super::{module}::"),
    ] {
        let mut from = 0;
        while let Some(found) = code[from..].find(&opener) {
            let start = from + found + opener.len();
            let end = code[start..].find(';').map_or(code.len(), |at| start + at);
            for name in code[start..end].split(|c: char| !is_ident(c)) {
                if !name.is_empty() && name != "self" && name != "as" {
                    names.insert(name.to_owned());
                }
            }
            from = end;
        }
    }
    for prefix in ["crate", "super"] {
        let group = use_group_names(code, prefix);
        if group.contains(module) {
            names.extend(group.into_iter().filter(|name| name != module));
        }
    }
    names
}

/// Whether `code` reaches the CLI module `module`: a path `module::…`
/// (bare, or after `crate::`, `super::`, `self::`) or a `use` naming it.
fn reaches_module(code: &str, module: &str) -> bool {
    let path = segments(code, module).into_iter().any(|at| {
        code[at + module.len()..].starts_with("::")
            && segment_before(code, at)
                .is_none_or(|owner| matches!(owner, "crate" | "super" | "self"))
    });
    path || use_group_names(code, "crate").contains(module)
        || use_group_names(code, "super").contains(module)
}

/// Whether `code` calls `name`: `name(` or `name::<` at a segment.
fn calls(code: &str, name: &str) -> bool {
    segments(code, name).into_iter().any(|at| {
        let rest = code[at + name.len()..].trim_start();
        rest.starts_with('(') || rest.starts_with("::<")
    })
}

/// The `fn` items of `code` with a body: (name, its body's byte range).
fn functions(code: &str) -> Vec<(String, usize, usize)> {
    let mut out = Vec::new();
    for at in segments(code, "fn") {
        let after = at + "fn".len();
        let name: String = code[after..]
            .trim_start()
            .chars()
            .take_while(|c| is_ident(*c))
            .collect();
        if name.is_empty() {
            continue;
        }
        let Some(open) = code[after..]
            .find(['{', ';'])
            .map(|offset| after + offset)
            .filter(|open| code[*open..].starts_with('{'))
        else {
            continue;
        };
        let mut depth = 0usize;
        let mut close = code.len();
        for (offset, c) in code[open..].char_indices() {
            if c == '{' {
                depth += 1;
            } else if c == '}' {
                depth -= 1;
                if depth == 0 {
                    close = open + offset + 1;
                    break;
                }
            }
        }
        out.push((name, open, close));
    }
    out
}

/// R5 (WA-3, ADR-0049). M: a call from `health`; the daemon route
/// spawning. Anchors (docs/features/layer-c-resolve.md "spec resolve",
/// steps 3-4): the binary's name is a literal of CLI modules only (the
/// "holders"); the modules that spawn on the group core (`group::spawn`),
/// 8a-1's test runner aside, are the "runners".
///
/// - no CLI module but a holder (and `lib.rs`' `mod`) reaches a runner;
/// - the holders' functions that reach a runner, directly or through each
///   other, are the entries; outside the holders, `lib.rs` and `main.rs`,
///   no module of any crate names one (in the CLI: `crate::NAME`,
///   `super::NAME`, `holder::NAME`, a `use` of it; elsewhere, the daemon
///   and MCP included: `specengine_cli::NAME` or a `use specengine_cli::{…}`
///   naming it);
/// - `main.rs` dispatches to an entry.
#[test]
fn only_resolve_spawns_layer_c() {
    let sources = engine_sources();
    assert!(
        sources
            .iter()
            .any(|file| file.starts_with("crates/specengine-http/src/"))
            && sources
                .iter()
                .any(|file| file.starts_with("crates/specengine-mcp/src/")),
        "the daemon's and MCP's sources are scanned"
    );
    let mut code_of = std::collections::BTreeMap::new();
    let mut holders = BTreeSet::new();
    for file in &sources {
        let (code, literals) = read_source(file);
        if literals.iter().any(|literal| literal.text.contains(RA_BIN)) {
            holders.insert(file.clone());
        }
        code_of.insert(file.clone(), code);
    }
    assert!(
        !holders.is_empty(),
        "the CLI names the binary it spawns (as serve.rs names specengine-http)"
    );
    let strays: Vec<&String> = holders
        .iter()
        .filter(|file| !file.starts_with(CLI_SRC) || **file == CLI_LIB || **file == CLI_MAIN)
        .collect();
    assert!(
        strays.is_empty(),
        "only `spec resolve`'s modules name {RA_BIN}: {strays:?}"
    );

    let runners: BTreeSet<String> = sources
        .iter()
        .filter(|file| {
            file.starts_with(CLI_SRC) && **file != GROUP_SOURCE && **file != TEST_RUNNER && {
                let code = &code_of[*file];
                segments(code, "group")
                    .into_iter()
                    .any(|at| code[at + "group".len()..].starts_with("::spawn"))
                    || imported_from(code, "group").contains("spawn")
            }
        })
        .cloned()
        .collect();
    assert!(
        !runners.is_empty(),
        "a CLI module spawns on the group core besides 8a-1's runner"
    );
    let runner_modules: BTreeSet<String> = runners.iter().map(|file| module_of(file)).collect();

    let mut offenders = Vec::new();
    for file in &sources {
        if !file.starts_with(CLI_SRC)
            || holders.contains(file)
            || runners.contains(file)
            || file == CLI_LIB
        {
            continue;
        }
        for module in &runner_modules {
            if reaches_module(&code_of[file], module) {
                offenders.push(format!("{file}: reaches the runner `{module}`"));
            }
        }
    }

    let mut entries: BTreeSet<String> = BTreeSet::new();
    for holder in &holders {
        let code = &code_of[holder];
        let fns = functions(code);
        let mut imported = BTreeSet::new();
        for module in &runner_modules {
            imported.extend(imported_from(code, module));
        }
        let mut reaching: BTreeSet<String> = fns
            .iter()
            .filter(|(_, open, close)| {
                let body = &code[*open..*close];
                runner_modules
                    .iter()
                    .any(|module| reaches_module(body, module))
                    || imported.iter().any(|name| calls(body, name))
            })
            .map(|(name, _, _)| name.clone())
            .collect();
        loop {
            let before = reaching.len();
            for (name, open, close) in &fns {
                if !reaching.contains(name)
                    && reaching
                        .iter()
                        .any(|other| calls(&code[*open..*close], other))
                {
                    reaching.insert(name.clone());
                }
            }
            if reaching.len() == before {
                break;
            }
        }
        entries.extend(reaching);
    }
    assert!(
        !entries.is_empty(),
        "the holders {holders:?} call a runner {runner_modules:?}"
    );
    let holder_modules: BTreeSet<String> = holders.iter().map(|file| module_of(file)).collect();

    for file in &sources {
        if holders.contains(file) || file == CLI_LIB || file == CLI_MAIN {
            continue;
        }
        let code = &code_of[file];
        let in_cli = file.starts_with(CLI_SRC);
        let mut names = if in_cli {
            let mut names = use_group_names(code, "crate");
            names.extend(use_group_names(code, "super"));
            for module in &holder_modules {
                names.extend(imported_from(code, module));
            }
            names
        } else {
            use_group_names(code, "specengine_cli")
        };
        names.retain(|name| entries.contains(name));
        for name in &entries {
            let named = segments(code, name).into_iter().any(|at| {
                segment_before(code, at).is_some_and(|owner| {
                    if in_cli {
                        matches!(owner, "crate" | "super") || holder_modules.contains(owner)
                    } else {
                        owner == "specengine_cli"
                    }
                })
            });
            if named || names.contains(name) {
                offenders.push(format!("{file}: names the entry `{name}`"));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "only `spec resolve` reaches what spawns {RA_BIN} (holders {holders:?}, runners {runners:?}, entries {entries:?}):\n{}",
        offenders.join("\n")
    );

    let main = &code_of[CLI_MAIN];
    assert!(
        entries.iter().any(|name| calls(main, name)),
        "main.rs dispatches `spec resolve` to an entry of {entries:?}"
    );
}

/// R10. M: a `Cargo.lock` literal in the CLI; `agrees` split at `/` in the
/// CLI. The store, core and CLI hold no Cargo file name or build-script
/// crate name, and `Cargo.toml` only as `code_walk.rs`' `MANIFEST`; the
/// slice's new CLI and store modules hold no SCIP punctuation literal and
/// no `impl#[`.
#[test]
fn no_scip_or_cargo_literal_outside_code_and_ra() {
    let mut offenders = Vec::new();
    let mut scanned = 0;
    let mut manifests = 0;
    for dir in [
        "crates/specengine-store/src",
        "crates/specengine-core/src",
        "crates/specengine-cli/src",
    ] {
        for file in sources_under(dir) {
            let (_, literals) = read_source(&file);
            for literal in &literals {
                scanned += 1;
                for word in CARGO_WORDS {
                    if literal.text.contains(word) {
                        offenders.push(format!("{file}: {:?} holds {word}", literal.text));
                    }
                }
                if literal.text.contains("Cargo.toml") {
                    if file == MANIFEST_HOLDER && literal.text == "Cargo.toml" {
                        manifests += 1;
                    } else {
                        offenders.push(format!("{file}: {:?} holds Cargo.toml", literal.text));
                    }
                }
            }
            let cli_or_store = file.starts_with(CLI_SRC) || file.starts_with(STORE_SRC);
            if !cli_or_store || BEFORE.contains(&file.as_str()) {
                continue;
            }
            for literal in &literals {
                if SCIP_PUNCTUATION.contains(&literal.text.as_str())
                    || literal.text.contains("impl#[")
                {
                    offenders.push(format!(
                        "{file} (new in the slice): the literal {:?}",
                        literal.text
                    ));
                }
            }
        }
    }
    assert!(scanned > 1000, "{scanned} literals scanned");
    assert_eq!(
        manifests, 1,
        "code_walk.rs' MANIFEST is the one `Cargo.toml`"
    );
    assert!(offenders.is_empty(), "{}", offenders.join("\n"));
}

/// The lexer the scans stand on: strings, raw strings, chars, lifetimes,
/// comments and test modules told apart.
#[test]
fn the_scan_reads_literals_and_skips_comments_and_test_modules() {
    let source = "// \"Cargo.lock\" in a comment\n/* '/' /* nested */ */\nfn f<'a>(x: &'a str) -> char {\n    let _ = r#\"raw \"Cargo.lock\"\"#;\n    let _ = b\"bytes\";\n    let _ = '\\'';\n    let _ = b'#';\n    '/'\n}\n#[cfg(test)]\nmod tests {\n    fn g() { let _ = \"inside\"; }\n}\nconst A: &str = \"after\";\n";
    let scanned = scan(source);
    let texts: Vec<&str> = scanned
        .literals
        .iter()
        .map(|literal| literal.text.as_str())
        .collect();
    assert_eq!(
        texts,
        [
            "raw \"Cargo.lock\"",
            "bytes",
            "\\'",
            "#",
            "/",
            "inside",
            "after"
        ]
    );
    assert!(!scanned.code.contains("Cargo"), "{}", scanned.code);
    assert_eq!(scanned.code.len(), source.len(), "offsets kept");
    let tests = test_modules(&scanned.code);
    assert_eq!(tests.len(), 1, "{tests:?}");
    let (start, end) = tests[0];
    assert!(scanned.code[start..end].starts_with("#[cfg(test)]"));
    assert!(scanned.code[start..end].trim_end().ends_with('}'));
    assert!(scanned.code[end..].contains("const A"));
}
