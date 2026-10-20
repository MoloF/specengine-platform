//! docs/features/lock-levels-verify.md RD-5 and TE-5: `spec verify` judges
//! every kind by its levels ("Level-aware verify"): C = (L_lock union
//! L_mark) minus `path`; step 2 (the blob equal), step 3 (C empty, or the
//! levels refused), step 4 (C's digests against the entry's: `code_changed
//! <levels>`, `levels_changed <from>-><to>`).
//!
//! Over scratch repositories of `fixtures/verify-ecs` (an ECS-style game
//! crate) made at test time: its four nodes accepted, and for the matrix a
//! cells file (`crates/game/src/cells.rs`, one bound unit per marker form:
//! default, `[sig]`, `[body]`, `[deps]`, `[path]`, two markers on one key,
//! an inline `mod`), a RON cells file (a default value and a `[deps]` one)
//! and a spec document naming their nodes, all written into the scratch
//! copy; the fixture is only read (its baselines stay). Each matrix column
//! is one edit committed (a signature, a body, a call added, a comment,
//! rustfmt `max_width = 60`, a neighbour, every marker narrowed, every
//! marker widened); each cell pins its row's JSON (state, category, detail,
//! `compared`, `levels`, every key) and text line, and where not `ok` its
//! `code` through the raised item's `drift_hash`, built here by hand
//! (`<level>:<digest>` per changed level, this run's digest through
//! `specengine-code`, as `drift_raise.rs`' `regen_drift_hash` does). The
//! expected rows come from the spec's step table, never from the command's
//! own output.

#![cfg(unix)]

mod common;
mod drift_common;
mod lock_common;
mod verify_common;

use std::io::Write as _;
use std::process::{Command, Stdio};

use drift_common::{Raised, proposal, raised};
use lock_common::{LockRepo, item_code, module_code, node_blocks, ron_code};
use serde_json::{Value, json};
use specengine_core::drift::{HashedEntry, HashedRow, drift_hash_input};
use specengine_core::verify::{MarkerRev, State};
use verify_common::{
    ECS, ECS_LIB, ECS_REFS, Key, MOVEMENT, STAMINA, Says, Verify, assert_row, item, levels,
    lock_entry, module, ron, verify,
};

/// verify-ecs' project slug: its queue.
const SLUG: &str = "verify-ecs";

/// The cells file, its spec document and its RON cells.
const CELLS: &str = "crates/game/src/cells.rs";
const CELLS_MD: &str = "docs/spec/cells.md";
const CELLS_RON: &str = "assets/data/cells.ron";

/// No level list: `sig`, `body`.
const DEFAULT: &[&str] = &["sig", "body"];

/// The comment every cell's body carries (a comment edit's subject).
const REST_COMMENT: &str = "// Rest restores a fixed amount per second.";

/// The spec document naming the cells' nodes.
const CELLS_DOC: &str = "\
---
class: canon
id: PERK-CELLS
owner: owner
reviewed: 2026-10-10
---

# Level cells

Each cell binds one unit under one marker form.

## Default {#PERK-DEFAULT}

A cell with no level list.

## Signature {#PERK-SIG}

A cell bound by its signature.

## Body {#PERK-BODY}

A cell bound by its body.

## Calls {#PERK-DEPS}

A cell bound by its calls.

## Path {#PERK-PATH}

A cell bound by its place alone.

## Two {#PERK-TWO}

A cell with two markers on one key.

## Inline {#PERK-INLINE}

An inline module bound by its inner doc comment.
";

/// The cells' nodes, in the order they are accepted.
const CELL_NODES: [&str; 7] = [
    "PERK-DEFAULT",
    "PERK-SIG",
    "PERK-BODY",
    "PERK-DEPS",
    "PERK-PATH",
    "PERK-TWO",
    "PERK-INLINE",
];

/// One bound unit of the cells file.
#[derive(Debug, Clone, Copy)]
struct Cell {
    node: &'static str,
    /// The function, or the inline `mod`, the markers bind.
    name: &'static str,
    /// L_lock: the levels accepted (the markers' union at accept).
    lock: &'static [&'static str],
    /// An inline `mod` (`module`), else a `function_item`.
    inline: bool,
}

const DEFAULT_CELL: Cell = Cell {
    node: "PERK-DEFAULT",
    name: "cell_default",
    lock: DEFAULT,
    inline: false,
};
const SIG_CELL: Cell = Cell {
    node: "PERK-SIG",
    name: "cell_sig",
    lock: &["sig"],
    inline: false,
};
const BODY_CELL: Cell = Cell {
    node: "PERK-BODY",
    name: "cell_body",
    lock: &["body"],
    inline: false,
};
const DEPS_CELL: Cell = Cell {
    node: "PERK-DEPS",
    name: "cell_deps",
    lock: &["deps"],
    inline: false,
};
const PATH_CELL: Cell = Cell {
    node: "PERK-PATH",
    name: "cell_path",
    lock: &["path"],
    inline: false,
};
const TWO_CELL: Cell = Cell {
    node: "PERK-TWO",
    name: "cell_two",
    lock: &["sig", "deps"],
    inline: false,
};
const INLINE_CELL: Cell = Cell {
    node: "PERK-INLINE",
    name: "cell_inline",
    lock: DEFAULT,
    inline: true,
};

/// Every cell, in the order the matrix lists them.
const ALL_CELLS: [Cell; 7] = [
    DEFAULT_CELL,
    SIG_CELL,
    BODY_CELL,
    DEPS_CELL,
    PATH_CELL,
    TWO_CELL,
    INLINE_CELL,
];

/// One cell function: its markers' lines, then the function.
fn cell_fn(markers: &[&str], name: &str) -> String {
    let mut out = String::from("\n");
    for marker in markers {
        out.push_str(marker);
        out.push('\n');
    }
    out.push_str(&format!(
        "pub fn {name}(stamina: &mut Stamina, rate: f32, dt: f32) -> f32 {{\n    \
         {REST_COMMENT}\n    \
         stamina.current = helper(stamina.current + rate * dt, stamina.max);\n    \
         stamina.current\n}}\n"
    ));
    out
}

/// The cells file as committed before any column's edit.
fn cells_source() -> String {
    let mut out = String::from(
        "//! Level cells: one bound unit per marker form.\n\n\
         use crate::stamina::Stamina;\n\n\
         /// Caps a value.\n\
         pub fn helper(value: f32, max: f32) -> f32 {\n    value.min(max)\n}\n\n\
         /// Halves a rate.\n\
         pub fn other(rate: f32) -> f32 {\n    rate * 0.5\n}\n",
    );
    out.push_str(&cell_fn(&["// @implements PERK-DEFAULT@1"], "cell_default"));
    out.push_str(&cell_fn(&["// @implements PERK-SIG@1 [sig]"], "cell_sig"));
    out.push_str(&cell_fn(
        &["// @implements PERK-BODY@1 [body]"],
        "cell_body",
    ));
    out.push_str(&cell_fn(
        &["// @implements PERK-DEPS@1 [deps]"],
        "cell_deps",
    ));
    out.push_str(&cell_fn(
        &["// @implements PERK-PATH@1 [path]"],
        "cell_path",
    ));
    out.push_str(&cell_fn(
        &[
            "// @implements PERK-TWO@1 [sig]",
            "// @implements PERK-TWO@1 [deps]",
        ],
        "cell_two",
    ));
    out.push_str(&format!(
        "\npub mod cell_inline {{\n    \
         //! @implements PERK-INLINE@1\n\n    \
         use super::{{Stamina, helper, other}};\n\n    \
         pub fn inside(stamina: &mut Stamina, rate: f32, dt: f32) -> f32 {{\n        \
         {REST_COMMENT}\n        \
         stamina.current = helper(stamina.current + rate * dt, stamina.max);\n        \
         let _ = other;\n        \
         stamina.current\n    }}\n}}\n"
    ));
    out.push_str("\n/// Seconds before a cell rests.\npub const NEIGHBOUR: f32 = 0.5;\n");
    out
}

/// The RON cells: `rest_rate` default (TUNE-02), `sprint_rate` `[deps]`
/// (TUNE-03), `walk_rate` unbound.
const CELLS_RON_TEXT: &str = "\
// Level cells' tuning.
Cells(
    rest_rate: 2.5, // @configures TUNE-02@1
    sprint_rate: 4.0, // @configures TUNE-03@1 [deps]
    walk_rate: 1.0,
)
";

/// verify-ecs committed with the cells written (`cells.rs` a module of
/// `lib.rs`, its spec document, the RON cells), then every node accepted:
/// the fixture's four, the seven cells', TUNE-02 and TUNE-03.
fn cells_repo(label: &str) -> LockRepo {
    let repo = LockRepo::of(label, ECS, "repo", &[]);
    repo.write(CELLS_MD, CELLS_DOC);
    repo.write(CELLS, cells_source());
    repo.write(CELLS_RON, CELLS_RON_TEXT);
    repo.replace(
        ECS_LIB,
        "pub mod stamina;\n",
        "pub mod cells;\npub mod stamina;\n",
    );
    repo.commit_all("the level cells");
    for reference in ECS_REFS
        .into_iter()
        .chain(CELL_NODES)
        .chain(["TUNE-02", "TUNE-03"])
    {
        repo.accept_ok(reference);
    }
    repo
}

/// verify-ecs with `edits` (`(file, from, to)`, each once) committed
/// (none: the fixture as committed), then its four nodes accepted.
fn ecs_with(label: &str, edits: &[(&str, &str, &str)]) -> LockRepo {
    let repo = LockRepo::of(label, ECS, "repo", &[]);
    for (file, from, to) in edits {
        repo.replace(file, from, to);
    }
    if !edits.is_empty() {
        repo.commit_all("the markers");
    }
    for reference in ECS_REFS {
        repo.accept_ok(reference);
    }
    repo
}

/// `from` replaced by `to` in `file`, every occurrence, exactly `count`.
fn replace_all(repo: &LockRepo, file: &str, from: &str, to: &str, count: usize) {
    let text = repo.read(file);
    assert_eq!(
        text.matches(from).count(),
        count,
        "{file}: {from:?} {count} times"
    );
    repo.write(file, text.replace(from, to));
}

/// `rustfmt --edition 2024 [--config max_width=60]` over `text`, through
/// stdin, outside any project's `rustfmt.toml`.
fn rustfmt(repo: &LockRepo, text: &str, narrow: bool) -> String {
    let mut command = Command::new("rustfmt");
    command
        .args(["--edition", "2024", "--emit", "stdout", "--quiet"])
        .current_dir(repo.scratch.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if narrow {
        command.args(["--config", "max_width=60"]);
    }
    let mut child = command
        .spawn()
        .expect("rustfmt is installed (rustup component)");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(text.as_bytes())
        .expect("feed rustfmt");
    let output = child.wait_with_output().expect("rustfmt ran");
    assert!(
        output.status.success(),
        "rustfmt: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("UTF-8")
}

/// The 1-based line of the first line of `text` starting (after
/// indentation) with `start`, exactly one.
fn line_of(text: &str, start: &str) -> usize {
    let found: Vec<usize> = text
        .lines()
        .enumerate()
        .filter(|(_, line)| line.trim_start().starts_with(start))
        .map(|(at, _)| at + 1)
        .collect();
    assert_eq!(found.len(), 1, "one line starting {start:?}: {found:?}");
    found[0]
}

/// The `(line, rev)` of each marker naming `node` at revision 1 in `text`.
fn marker_lines(text: &str, node: &str) -> Vec<(usize, Option<u32>)> {
    let needle = format!("@implements {node}@1");
    text.lines()
        .enumerate()
        .filter(|(_, line)| {
            line.contains(&needle)
                && line[line.find(&needle).expect("found") + needle.len()..]
                    .chars()
                    .next()
                    .is_none_or(|next| !next.is_ascii_digit())
        })
        .map(|(at, _)| (at + 1, Some(1)))
        .collect()
}

// --------------------------------------------------------- what a row says

/// A cell's outcome, the spec side unchanged ("Level-aware verify").
#[derive(Debug, Clone, Copy)]
enum Outcome {
    /// Step 4, every digest of C equal and the sets equal: `ok` by `ast`,
    /// `changed` empty.
    Ok,
    /// Step 3, C empty (both sides `[path]`): `ok`, `compared` and
    /// `changed` null, nothing parsed.
    PathOnly,
    /// Step 4, these levels' digests differ: `code_changed <levels>`.
    Changed(&'static [&'static str]),
    /// Step 4, no digest differs, the sets do: `levels_changed <detail>`.
    Levels(&'static str),
}

/// What a row says beyond its key: (state, category, detail, compared,
/// `levels.changed`).
type Said = (
    &'static str,
    Option<&'static str>,
    Option<String>,
    Option<&'static str>,
    Option<Vec<&'static str>>,
);

/// Steps 1-4's row for `outcome`.
fn said(outcome: Outcome) -> Said {
    match outcome {
        Outcome::Ok => ("ok", None, None, Some("ast"), Some(vec![])),
        Outcome::PathOnly => ("ok", None, None, None, None),
        Outcome::Changed(levels) => (
            "code_ahead",
            Some("code_changed"),
            Some(levels.join(",")),
            Some("ast"),
            Some(levels.to_vec()),
        ),
        Outcome::Levels(detail) => (
            "code_ahead",
            Some("levels_changed"),
            Some(detail.to_owned()),
            Some("ast"),
            Some(vec![]),
        ),
    }
}

/// `<level>:<digest>` per level of `changed` from `code`, `,`-joined
/// ("Drift evidence": the row's `code`, this run's digests).
fn code_fact(code: &lock_common::Code, changed: &[&str]) -> String {
    changed
        .iter()
        .map(|level| {
            let digest = match *level {
                "sig" => &code.sig,
                "body" => &code.body,
                "deps" => &code.deps,
                other => panic!("no digest for {other}"),
            };
            format!("{level}:{digest}")
        })
        .collect::<Vec<_>>()
        .join(",")
}

/// The `drift_hash` "Drift evidence" gives `key`'s node when its one
/// non-`ok` row is `key`'s, saying `state category detail` with `code`,
/// built by hand from the lock at the worktree (`lock_code`: the entry's
/// `levels`, `sig_hash`, `body_hash`, `deps_hash` lines as written,
/// LF-joined), the node's `norm_hash` and revision `rev`.
fn built_hash(
    repo: &LockRepo,
    key: Key<'_>,
    marker_revs: Vec<MarkerRev>,
    said: (&str, &str, &str),
    code: Option<&str>,
    rev: u32,
) -> String {
    let (state, category, detail) = said;
    let lock = repo.lock_text();
    let symbol_line = format!("\nsymbol = \"{}\"\n", key.symbol.expect("a keyed row"));
    let file_line = format!("\nfile = \"{}\"\n", key.file);
    let block = node_blocks(&lock, key.node)
        .into_iter()
        .find(|block| block.contains(&symbol_line) && block.contains(&file_line))
        .unwrap_or_else(|| panic!("{key:?}'s entry in:\n{lock}"));
    let field = |name: &str| {
        block
            .lines()
            .find_map(|line| line.strip_prefix(&format!("{name} = ")))
            .unwrap_or_else(|| panic!("no {name}"))
            .to_owned()
    };
    let lock_code: Vec<&str> = block
        .lines()
        .filter(|line| {
            ["levels = ", "sig_hash = ", "body_hash = ", "deps_hash = "]
                .iter()
                .any(|prefix| line.starts_with(prefix))
        })
        .collect();
    assert_eq!(lock_code.len(), 4, "{block}");
    let lock_code = lock_code.join("\n");
    let entry_rev: u32 = field("rev").parse().expect("a revision");
    let spec_hash = field("spec_hash").trim_matches('"').to_owned();
    let file_blob = field("file_blob").trim_matches('"').to_owned();
    let norm = repo.norm_hash(key.node);
    let state = match state {
        "code_ahead" => State::CodeAhead,
        "conflict" => State::Conflict,
        other => panic!("no raising state {other} here"),
    };
    let row = HashedRow {
        node: key.node,
        relation: key.relation,
        file: key.file,
        kind: key.kind,
        symbol: key.symbol,
        item_kind: key.item_kind,
        state,
        category: Some(category),
        detail: Some(detail),
        marker_revs,
        entry: Some(HashedEntry {
            rev: entry_rev,
            spec_hash: &spec_hash,
            code: Some(&lock_code),
            file_blob: &file_blob,
        }),
        rev: Some(rev),
        norm_hash: Some(&norm),
        code,
        moved_from: None,
    };
    specengine_store::b3_hash(drift_hash_input(&[row]).as_bytes())
}

/// The `drift_hash` the raise stored for `node`'s item.
fn stored_hash(repo: &LockRepo, raised: &Raised, node: &str) -> String {
    let (id, _) = raised
        .created()
        .into_iter()
        .find(|(_, created)| created == node)
        .unwrap_or_else(|| panic!("no item for {node}: {}", raised.text()));
    proposal(repo, SLUG, &id).drift.expect("a drift item").hash
}

/// The facts one row is checked by.
struct Expect<'a> {
    key: Key<'a>,
    markers: Vec<(usize, Option<u32>)>,
    node_rev: u32,
    lock_levels: &'a [&'a str],
    marker_levels: &'a [&'a str],
    outcome: Outcome,
    /// This run's digests of the unit (for `code`).
    code: Option<lock_common::Code>,
}

/// Asserts `expect`'s row in `run` (every key, the text line) and, when
/// not `ok`, the raised item's `drift_hash` against the one built by hand
/// with its `code` (`None` for `levels_changed`). Collects failures.
fn check_row(
    repo: &LockRepo,
    run: &Verify,
    raised: &Raised,
    expect: &Expect<'_>,
    failures: &mut Vec<String>,
) {
    let (state, category, detail, compared, changed) = said(expect.outcome);
    let says = Says {
        state,
        category,
        detail: detail.as_deref(),
        pre_existing: false,
        markers: &expect.markers,
        node_rev: Some(expect.node_rev),
        lock: lock_entry(&repo.lock_text(), expect.key),
        compared,
        levels: levels(expect.lock_levels, expect.marker_levels, changed.as_deref()),
    };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        assert_row(run, expect.key, &says, None);
    }));
    if let Err(panic) = result {
        let message = panic
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| panic.downcast_ref::<&str>().map(|text| (*text).to_owned()))
            .unwrap_or_default();
        failures.push(format!("{:?}: {message}", expect.key));
        return;
    }
    if state == "ok" {
        return;
    }
    let code = match expect.outcome {
        Outcome::Changed(levels) => Some(code_fact(
            expect.code.as_ref().expect("this run's digests"),
            levels,
        )),
        _ => None,
    };
    let built = built_hash(
        repo,
        expect.key,
        expect
            .markers
            .iter()
            .map(|(_, rev)| rev.map_or(MarkerRev::Unversioned, MarkerRev::Written))
            .collect(),
        (
            state,
            category.expect("a category"),
            detail.as_deref().expect("a detail"),
        ),
        code.as_deref(),
        expect.node_rev,
    );
    let stored = stored_hash(repo, raised, expect.key.node);
    if stored != built {
        failures.push(format!(
            "{:?}: drift_hash stored {stored}, built {built} (code {code:?})",
            expect.key
        ));
    }
}

/// The symbol of a cell.
fn cell_symbol(cell: &Cell) -> String {
    format!("crates/game::cells::{}", cell.name)
}

/// [`Expect`] of a cell of the cells file at the worktree.
fn cell_expect<'a>(
    repo: &LockRepo,
    cell: &'a Cell,
    symbol: &'a str,
    marker_levels: &'a [&'a str],
    outcome: Outcome,
) -> Expect<'a> {
    let text = repo.read(CELLS);
    let key = if cell.inline {
        module(cell.node, "implements", CELLS, symbol)
    } else {
        item(cell.node, "implements", CELLS, symbol, "function_item")
    };
    let code = match outcome {
        Outcome::Changed(_) => Some(if cell.inline {
            item_code(
                text.as_bytes(),
                line_of(&text, &format!("pub mod {} {{", cell.name)),
                "mod_item",
                cell.lock,
            )
        } else {
            item_code(
                text.as_bytes(),
                line_of(&text, &format!("pub fn {}(", cell.name)),
                "function_item",
                cell.lock,
            )
        }),
        _ => None,
    };
    Expect {
        key,
        markers: marker_lines(&text, cell.node),
        node_rev: 1,
        lock_levels: cell.lock,
        marker_levels,
        outcome,
        code,
    }
}

// ---------------------------------------------------------------- TE-5

/// One matrix column: the edit applied to the cells file, then each cell's
/// markers' levels after it and its outcome, in [`ALL_CELLS`]' order.
struct Column {
    label: &'static str,
    edit: fn(&LockRepo),
    cells: [(&'static [&'static str], Outcome); 7],
}

/// Runs a column: the cells' repository, the edit committed, `spec
/// verify` and a sweep raise at `HEAD`; every cell's row and, where not
/// `ok`, its item's hash; the fixture's own rows untouched (`ok` by
/// blob); the cascade counting every compared row by its `compared`.
fn run_column(column: &Column) {
    let repo = cells_repo(&format!("levels-{}", column.label));
    (column.edit)(&repo);
    repo.commit_all(column.label);
    let run = verify(&repo, &[]);
    let raised = raised(&repo, &[]);
    let mut failures = Vec::new();
    for (cell, (marks, outcome)) in ALL_CELLS.iter().zip(column.cells) {
        let symbol = cell_symbol(cell);
        let expect = cell_expect(&repo, cell, &symbol, marks, outcome);
        check_row(&repo, &run, &raised, &expect, &mut failures);
    }
    // The rows of the files the column leaves alone: `ok` by blob.
    for (node, relation, file, symbol) in [
        (
            "PERK-REGEN",
            "implements",
            STAMINA,
            "crates/game::stamina::regen_system",
        ),
        ("PERK-STAMINA", "implements", ECS_LIB, "crates/game"),
        ("TUNE-01", "configures", MOVEMENT, "root.regen_per_second"),
        ("TUNE-02", "configures", CELLS_RON, "root.rest_rate"),
        ("TUNE-03", "configures", CELLS_RON, "root.sprint_rate"),
    ] {
        let row = run.row(node, relation, file, Some(symbol));
        if (&row["state"], &row["compared"]) != (&json!("ok"), &json!("blob")) {
            failures.push(format!("{node} {symbol}: {row}"));
        }
    }
    let by = |compared: &str| run.rows_where(|row| row["compared"] == compared).len();
    assert_eq!(
        run.json["cascade"],
        json!({"by_blob": by("blob"), "by_ast": by("ast")}),
        "{}: `cascade` counts `compared`",
        column.label
    );
    // The raise created an item for each drifting cell alone.
    let drifting: Vec<&str> = ALL_CELLS
        .iter()
        .zip(column.cells)
        .filter(|(_, (_, outcome))| matches!(outcome, Outcome::Changed(_) | Outcome::Levels(_)))
        .map(|(cell, _)| cell.node)
        .collect();
    let mut created = raised.created_nodes();
    created.sort();
    let mut wanted: Vec<String> = drifting.iter().map(|node| (*node).to_owned()).collect();
    wanted.sort();
    if created != wanted {
        failures.push(format!("items {created:?}, wanted {wanted:?}"));
    }
    assert!(
        failures.is_empty(),
        "column {}:\n{}\n{}",
        column.label,
        failures.join("\n"),
        run.text.show()
    );
}

/// Each cell's markers unchanged.
const AS_LOCKED: [&[&str]; 7] = [
    DEFAULT,
    &["sig"],
    &["body"],
    &["deps"],
    &["path"],
    &["sig", "deps"],
    DEFAULT,
];

/// The cells at their markers as locked, each with `outcome`.
fn as_locked(outcomes: [Outcome; 7]) -> [(&'static [&'static str], Outcome); 7] {
    let mut out = [(DEFAULT, Outcome::Ok); 7];
    for (at, outcome) in outcomes.into_iter().enumerate() {
        out[at] = (AS_LOCKED[at], outcome);
    }
    out
}

/// TE-5, a signature edit (`dt: f32` to `f64` in every cell): `sig` in C
/// drifts (`code_changed sig`: default, `[sig]`, two markers `[sig]` and
/// `[deps]`); `[body]`, `[deps]` read `ok` by `ast`; `[path]` compares
/// nothing; the inline `mod`'s `sig` is its `pub mod` alone, the edited
/// function lies in its `body` (`code_changed body`). M6: `sig` and
/// `body` compared whatever the levels; a `[path]` binding parsed.
#[test]
fn matrix_signature_edit() {
    run_column(&Column {
        label: "signature",
        edit: |repo| {
            replace_all(repo, CELLS, "dt: f32) -> f32 {", "dt: f64) -> f32 {", 7);
        },
        cells: as_locked([
            Outcome::Changed(&["sig"]),
            Outcome::Changed(&["sig"]),
            Outcome::Ok,
            Outcome::Ok,
            Outcome::PathOnly,
            Outcome::Changed(&["sig"]),
            Outcome::Changed(&["body"]),
        ]),
    });
}

/// TE-5, a body edit (a literal in every cell's body): `body` in C drifts
/// (default, `[body]`, the inline `mod`); `[sig]`, `[deps]` and `[sig]` +
/// `[deps]` read `ok`. M6.
#[test]
fn matrix_body_edit() {
    run_column(&Column {
        label: "body",
        edit: |repo| {
            replace_all(
                repo,
                CELLS,
                "rate * dt, stamina.max)",
                "rate * dt * 2.0, stamina.max)",
                7,
            );
        },
        cells: as_locked([
            Outcome::Changed(&["body"]),
            Outcome::Ok,
            Outcome::Changed(&["body"]),
            Outcome::Ok,
            Outcome::PathOnly,
            Outcome::Ok,
            Outcome::Changed(&["body"]),
        ]),
    });
}

/// TE-5, a call added (`other(rate)` in every body: `body` and `deps`
/// change): the default drifts by `body` alone (`deps` not in C), `[deps]`
/// and `[sig]` + `[deps]` by `deps`, `[body]` by `body`, `[sig]` `ok`. M:
/// `deps` compared outside C; `deps` never compared.
#[test]
fn matrix_call_added() {
    run_column(&Column {
        label: "call",
        edit: |repo| {
            replace_all(
                repo,
                CELLS,
                "stamina.current + rate * dt",
                "stamina.current + other(rate) * dt",
                7,
            );
        },
        cells: as_locked([
            Outcome::Changed(&["body"]),
            Outcome::Ok,
            Outcome::Changed(&["body"]),
            Outcome::Changed(&["deps"]),
            Outcome::PathOnly,
            Outcome::Changed(&["deps"]),
            Outcome::Changed(&["body"]),
        ]),
    });
}

/// TE-5, comments edited in every cell's body and a doc comment added
/// above each: every cell `ok` by `ast`, `[path]` compares nothing. M:
/// comments hashed.
#[test]
fn matrix_comment_edit() {
    run_column(&Column {
        label: "comment",
        edit: |repo| {
            replace_all(
                repo,
                CELLS,
                REST_COMMENT,
                "// Rest restores a fixed amount every second.",
                7,
            );
            replace_all(
                repo,
                CELLS,
                "\npub fn cell_",
                "\n/// A level cell.\npub fn cell_",
                6,
            );
        },
        cells: as_locked([
            Outcome::Ok,
            Outcome::Ok,
            Outcome::Ok,
            Outcome::Ok,
            Outcome::PathOnly,
            Outcome::Ok,
            Outcome::Ok,
        ]),
    });
}

/// TE-5, rustfmt `max_width = 60` over the cells file (it reflows every
/// cell): every cell `ok` by `ast`. M: whitespace hashed.
#[test]
fn matrix_rustfmt_narrow() {
    run_column(&Column {
        label: "rustfmt",
        edit: |repo| {
            let original = repo.read(CELLS);
            let narrow = rustfmt(repo, &original, true);
            assert_ne!(narrow, original, "max_width = 60 reflows the cells");
            repo.write(CELLS, narrow);
        },
        cells: as_locked([
            Outcome::Ok,
            Outcome::Ok,
            Outcome::Ok,
            Outcome::Ok,
            Outcome::PathOnly,
            Outcome::Ok,
            Outcome::Ok,
        ]),
    });
}

/// TE-5, a neighbour edited (the unbound `NEIGHBOUR` and `helper`'s body):
/// every cell `ok` by `ast`. M: an item's file blob compared.
#[test]
fn matrix_neighbour_edit() {
    run_column(&Column {
        label: "neighbour",
        edit: |repo| {
            repo.replace(
                CELLS,
                "pub const NEIGHBOUR: f32 = 0.5;",
                "pub const NEIGHBOUR: f32 = 0.75;",
            );
            repo.replace(
                CELLS,
                "    value.min(max)\n",
                "    value.min(max).max(0.0)\n",
            );
        },
        cells: as_locked([
            Outcome::Ok,
            Outcome::Ok,
            Outcome::Ok,
            Outcome::Ok,
            Outcome::PathOnly,
            Outcome::Ok,
            Outcome::Ok,
        ]),
    });
}

/// TE-5, every marker narrowed, no code edit (Q5): the entry holds every
/// level's digest, none differs, the sets do: `levels_changed <L_lock> ->
/// <L_mark>`, each set whole, `path` included (`sig->path`); two markers
/// on one key losing one (`sig,deps->sig`); `[path]` alone stays `[path]`
/// (nothing compared). M: L_mark alone compared; the detail without
/// `path`.
#[test]
fn matrix_markers_narrowed() {
    run_column(&Column {
        label: "narrowed",
        edit: |repo| {
            for (from, to) in [
                (
                    "// @implements PERK-DEFAULT@1\n",
                    "// @implements PERK-DEFAULT@1 [sig]\n",
                ),
                (
                    "// @implements PERK-SIG@1 [sig]",
                    "// @implements PERK-SIG@1 [path]",
                ),
                (
                    "// @implements PERK-BODY@1 [body]",
                    "// @implements PERK-BODY@1 [path]",
                ),
                (
                    "// @implements PERK-DEPS@1 [deps]",
                    "// @implements PERK-DEPS@1 [path]",
                ),
                ("// @implements PERK-TWO@1 [deps]\n", ""),
                (
                    "//! @implements PERK-INLINE@1\n",
                    "//! @implements PERK-INLINE@1 [sig]\n",
                ),
            ] {
                repo.replace(CELLS, from, to);
            }
        },
        cells: [
            (&["sig"], Outcome::Levels("sig,body->sig")),
            (&["path"], Outcome::Levels("sig->path")),
            (&["path"], Outcome::Levels("body->path")),
            (&["path"], Outcome::Levels("deps->path")),
            (&["path"], Outcome::PathOnly),
            (&["sig"], Outcome::Levels("sig,deps->sig")),
            (&["sig"], Outcome::Levels("sig,body->sig")),
        ],
    });
}

/// TE-5, every marker widened, no code edit (Q6: class Check): the
/// recorded digests of the levels the lock does not list are compared,
/// none differs: `levels_changed <L_lock>-><L_mark>`; `[path]` widened to
/// `[path, sig]` compares `sig`. M: a widened marker read `ok`; a level
/// the lock does not list never compared.
#[test]
fn matrix_markers_widened() {
    run_column(&Column {
        label: "widened",
        edit: |repo| {
            for (from, to) in [
                (
                    "// @implements PERK-DEFAULT@1\n",
                    "// @implements PERK-DEFAULT@1 [sig, body, deps]\n",
                ),
                (
                    "// @implements PERK-SIG@1 [sig]",
                    "// @implements PERK-SIG@1",
                ),
                (
                    "// @implements PERK-BODY@1 [body]",
                    "// @implements PERK-BODY@1 [body, deps]",
                ),
                (
                    "// @implements PERK-DEPS@1 [deps]",
                    "// @implements PERK-DEPS@1 [sig, deps]",
                ),
                (
                    "// @implements PERK-PATH@1 [path]",
                    "// @implements PERK-PATH@1 [path, sig]",
                ),
                (
                    "// @implements PERK-TWO@1 [deps]",
                    "// @implements PERK-TWO@1 [body, deps]",
                ),
                (
                    "//! @implements PERK-INLINE@1\n",
                    "//! @implements PERK-INLINE@1 [sig, body, deps]\n",
                ),
            ] {
                repo.replace(CELLS, from, to);
            }
        },
        cells: [
            (
                &["sig", "body", "deps"],
                Outcome::Levels("sig,body->sig,body,deps"),
            ),
            (DEFAULT, Outcome::Levels("sig->sig,body")),
            (&["body", "deps"], Outcome::Levels("body->body,deps")),
            (&["sig", "deps"], Outcome::Levels("deps->sig,deps")),
            (&["path", "sig"], Outcome::Levels("path->path,sig")),
            (
                &["sig", "body", "deps"],
                Outcome::Levels("sig,deps->sig,body,deps"),
            ),
            (
                &["sig", "body", "deps"],
                Outcome::Levels("sig,body->sig,body,deps"),
            ),
        ],
    });
}

/// TE-5's baseline: the cells as accepted verify `clean`, every row `ok`
/// by `blob`, `levels` the lock's and the markers' sets, `changed` null
/// (no digest compared). M: a row compared by digest with its blob equal.
#[test]
fn matrix_accepted_cells_are_ok_by_blob() {
    let repo = cells_repo("levels-baseline");
    let run = verify(&repo, &[]);
    assert_eq!(
        (run.verdict(), run.code()),
        ("clean", 0),
        "{}",
        run.text.show()
    );
    let lock = repo.lock_text();
    let text = repo.read(CELLS);
    for (cell, marks) in ALL_CELLS.iter().zip(AS_LOCKED) {
        let symbol = cell_symbol(cell);
        let key = if cell.inline {
            module(cell.node, "implements", CELLS, &symbol)
        } else {
            item(cell.node, "implements", CELLS, &symbol, "function_item")
        };
        let markers = marker_lines(&text, cell.node);
        assert_row(
            &run,
            key,
            &Says {
                state: "ok",
                category: None,
                detail: None,
                pre_existing: false,
                markers: &markers,
                node_rev: Some(1),
                lock: lock_entry(&lock, key),
                compared: Some("blob"),
                levels: levels(cell.lock, marks, None),
            },
            None,
        );
    }
    let rows = run.rows();
    assert!(
        rows.iter().all(|row| row["compared"] == "blob"),
        "every row by blob: {}",
        run.text.show()
    );
}

// ------------------------------------------------------------ RON, module

/// TE-5, RON (Q46): a RON value's `sig` = `body` = its value hash, `deps`
/// the empty set's. Its sibling (`walk_rate`) edited: both bound values
/// `ok` by `ast`; the default value edited: `code_changed sig,body`, its
/// `code` `sig:<value>,body:<value>`; the `[deps]` value edited: `ok`, a
/// `[deps]` RON binding never drifts. M7: RON judged by blob; M: a RON
/// value's `deps` given its hash.
#[test]
fn ron_value_sibling_and_deps() {
    let cases: [(&str, &str, &str, Outcome, Outcome); 3] = [
        (
            "ron-sibling",
            "walk_rate: 1.0,",
            "walk_rate: 1.5,",
            Outcome::Ok,
            Outcome::Ok,
        ),
        (
            "ron-value",
            "rest_rate: 2.5,",
            "rest_rate: 3.0,",
            Outcome::Changed(&["sig", "body"]),
            Outcome::Ok,
        ),
        (
            "ron-deps",
            "sprint_rate: 4.0,",
            "sprint_rate: 5.0,",
            Outcome::Ok,
            Outcome::Ok,
        ),
    ];
    let mut failures = Vec::new();
    for (label, from, to, rest, sprint) in cases {
        let repo = cells_repo(&format!("levels-{label}"));
        repo.replace(CELLS_RON, from, to);
        repo.commit_all(label);
        let run = verify(&repo, &[]);
        let raised = raised(&repo, &[]);
        for expect in [
            ron_expect(
                &repo,
                ("TUNE-02", "root.rest_rate", "rest_rate:"),
                DEFAULT,
                rest,
            ),
            ron_expect(
                &repo,
                ("TUNE-03", "root.sprint_rate", "sprint_rate:"),
                &["deps"],
                sprint,
            ),
        ] {
            let mut mine = Vec::new();
            check_row(&repo, &run, &raised, &expect, &mut mine);
            failures.extend(
                mine.into_iter()
                    .map(|failure| format!("{label}: {failure}")),
            );
        }
        // The fixture's own RON value, in another file: by blob.
        let movement = run.row(
            "TUNE-01",
            "configures",
            MOVEMENT,
            Some("root.regen_per_second"),
        );
        if movement["compared"] != "blob" {
            failures.push(format!("{label}: {movement}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A RON cell's [`Expect`] at the worktree: `(node, field path, the start
/// of its line)`, locked and marked under `levels` (a `ron` entry's `sig` =
/// `body` = the value hash).
fn ron_expect<'a>(
    repo: &LockRepo,
    (node, symbol, start): (&'a str, &'a str, &str),
    levels: &'a [&'a str],
    outcome: Outcome,
) -> Expect<'a> {
    let text = repo.read(CELLS_RON);
    let line = line_of(&text, start);
    Expect {
        key: ron(node, "configures", CELLS_RON, symbol),
        markers: vec![(line, Some(1))],
        node_rev: 1,
        lock_levels: levels,
        marker_levels: levels,
        outcome,
        code: Some(ron_code(text.as_bytes(), line, levels)),
    }
}

/// The file module's [`Expect`] of verify-ecs' `lib.rs` (`//!
/// @implements PERK-STAMINA@2`, marker line 2).
fn module_expect<'a>(
    repo: &LockRepo,
    lock: &'a [&'a str],
    marks: &'a [&'a str],
    outcome: Outcome,
) -> Expect<'a> {
    let text = repo.read(ECS_LIB);
    Expect {
        key: module("PERK-STAMINA", "implements", ECS_LIB, "crates/game"),
        markers: vec![(2, Some(2))],
        node_rev: 2,
        lock_levels: lock,
        marker_levels: marks,
        outcome,
        code: Some(module_code(text.as_bytes(), lock)),
    }
}

/// TE-5, the file module (`code-hash.md` "File module": `sig` the leading
/// run of inner attributes, `body` the file's walk without it): an inner
/// attribute added reads `code_changed sig`, `code` `sig:<digest>`; a
/// field in the module's unbound `App` (its `body` is the file)
/// `code_changed body`; a comment edit `ok` by `ast`; the error-free
/// `build` beside it `ok` by `ast` throughout. M: a module judged by its
/// blob; its `code` a blob OID.
#[test]
fn module_inner_attribute_vs_its_body() {
    let mut failures = Vec::new();
    let attribute = ecs_with("levels-module-attribute", &[]);
    attribute.replace(
        ECS_LIB,
        "//! @implements PERK-STAMINA@2\n",
        "//! @implements PERK-STAMINA@2\n#![allow(dead_code)]\n",
    );
    attribute.commit_all("an inner attribute");
    let run = verify(&attribute, &[]);
    let raised_now = raised(&attribute, &[]);
    check_row(
        &attribute,
        &run,
        &raised_now,
        &module_expect(&attribute, DEFAULT, DEFAULT, Outcome::Changed(&["sig"])),
        &mut failures,
    );

    let body = ecs_with("levels-module-body", &[]);
    body.replace(
        ECS_LIB,
        "/// Registers the stamina systems.",
        "/// Registers every stamina system.",
    );
    let comment = verify(&body, &[]);
    check_row(
        &body,
        &comment,
        &raised(&body, &[]),
        &module_expect(&body, DEFAULT, DEFAULT, Outcome::Ok),
        &mut failures,
    );
    body.replace(
        ECS_LIB,
        "pub systems: Vec<&'static str>,",
        "pub systems: Vec<&'static str>,\n    pub paused: bool,",
    );
    body.commit_all("a field");
    let run = verify(&body, &[]);
    let raised_now = raised(&body, &[]);
    check_row(
        &body,
        &run,
        &raised_now,
        &module_expect(&body, DEFAULT, DEFAULT, Outcome::Changed(&["body"])),
        &mut failures,
    );
    let build = run.row(
        "PERK-STAMINA",
        "implements",
        ECS_LIB,
        Some("crates/game::StaminaPlugin::build"),
    );
    assert_eq!(
        (&build["state"], &build["compared"]),
        (&json!("ok"), &json!("ast"))
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// ---------------------------------------------------------------- RD-5

/// `regen_system`'s key in verify-ecs.
fn regen_key() -> Key<'static> {
    item(
        "PERK-REGEN",
        "implements",
        STAMINA,
        "crates/game::stamina::regen_system",
        "function_item",
    )
}

/// `regen_system`'s [`Expect`] at the worktree (its marker on line 9).
fn regen_expect<'a>(
    repo: &LockRepo,
    lock: &'a [&'a str],
    marks: &'a [&'a str],
    outcome: Outcome,
) -> Expect<'a> {
    let text = repo.read(STAMINA);
    Expect {
        key: regen_key(),
        markers: vec![(9, Some(1))],
        node_rev: 1,
        lock_levels: lock,
        marker_levels: marks,
        outcome,
        code: Some(item_code(
            text.as_bytes(),
            line_of(&text, "pub fn regen_system("),
            "function_item",
            lock,
        )),
    }
}

/// The regen system's body edit.
const REGEN_BODY: (&str, &str) = ("rate * dt).min", "rate * dt * 2.0).min");

/// [`check_row`] over one committed regen edit: verify, a sweep raise,
/// `PERK-REGEN`'s row (and hash), no other node drifting.
fn check_regen(repo: &LockRepo, expect: &Expect<'_>) -> Verify {
    let run = verify(repo, &[]);
    let raised = raised(repo, &[]);
    let mut failures = Vec::new();
    check_row(repo, &run, &raised, expect, &mut failures);
    assert!(
        failures.is_empty(),
        "{}\n{}",
        failures.join("\n"),
        run.text.show()
    );
    let others: Vec<String> = raised
        .created_nodes()
        .into_iter()
        .filter(|node| node != "PERK-REGEN")
        .collect();
    assert!(others.is_empty(), "{others:?}");
    run
}

/// RD-5 (Q5): `regen_system`'s marker narrowed to `[sig]` (not accepted:
/// the lock holds `sig, body`) and its body edited: C is the union, the
/// row `code_ahead code_changed body` by `ast`, `levels` `{"lock": ["sig",
/// "body"], "markers": ["sig"], "changed": ["body"]}` (the spec's JSON
/// example), its text line carrying the detail, its `code`
/// `body:<digest>`. M: L_mark alone compared (the edit hidden).
#[test]
fn narrowed_marker_with_a_body_edit_is_code_changed() {
    let repo = ecs_with("levels-narrowed", &[]);
    repo.replace(
        STAMINA,
        "// @implements PERK-REGEN@1\n",
        "// @implements PERK-REGEN@1 [sig]\n",
    );
    repo.replace(STAMINA, REGEN_BODY.0, REGEN_BODY.1);
    repo.commit_all("narrowed, the body edited");
    let run = check_regen(
        &repo,
        &regen_expect(&repo, DEFAULT, &["sig"], Outcome::Changed(&["body"])),
    );
    run.has_line("check | code_ahead code_changed body | PERK-REGEN | implements | crates/game/src/stamina.rs | item crates/game::stamina::regen_system function_item");
    assert_eq!((run.verdict(), run.code()), ("observed", 0));
}

/// RD-5 (Q5, Q6): `regen_system` accepted under `[sig]`, its marker then
/// widened to the default, no code edit: the entry's recorded `body`
/// digest compared, equal; the sets differ: `code_ahead levels_changed
/// sig->sig,body` by `ast`, class Check, `code` null in its `drift_hash`.
/// Under the widened marker a body edit reads `code_changed body`. M: a
/// widened marker read `ok`.
#[test]
fn widened_marker_is_levels_changed() {
    let repo = ecs_with(
        "levels-widened",
        &[(
            STAMINA,
            "// @implements PERK-REGEN@1\n",
            "// @implements PERK-REGEN@1 [sig]\n",
        )],
    );
    repo.replace(
        STAMINA,
        "// @implements PERK-REGEN@1 [sig]\n",
        "// @implements PERK-REGEN@1\n",
    );
    repo.commit_all("widened");
    let run = check_regen(
        &repo,
        &regen_expect(&repo, &["sig"], DEFAULT, Outcome::Levels("sig->sig,body")),
    );
    run.has_line("check | code_ahead levels_changed sig->sig,body | PERK-REGEN | implements | crates/game/src/stamina.rs | item crates/game::stamina::regen_system function_item");

    let edited = ecs_with(
        "levels-widened-edited",
        &[(
            STAMINA,
            "// @implements PERK-REGEN@1\n",
            "// @implements PERK-REGEN@1 [sig]\n",
        )],
    );
    edited.replace(
        STAMINA,
        "// @implements PERK-REGEN@1 [sig]\n",
        "// @implements PERK-REGEN@1\n",
    );
    edited.replace(STAMINA, REGEN_BODY.0, REGEN_BODY.1);
    edited.commit_all("widened, the body edited");
    check_regen(
        &edited,
        &regen_expect(&edited, &["sig"], DEFAULT, Outcome::Changed(&["body"])),
    );
}

/// RD-5 (Q42): `regen_system` accepted under `[path]`, then its body and
/// its signature edited: C is empty, nothing compared, the row `ok`,
/// `compared` null, `levels.changed` null; `clean`, exit 0; the cascade
/// counts it under neither. M: a `[path]` binding compared.
#[test]
fn path_only_binding_ignores_a_body_edit() {
    let repo = ecs_with(
        "levels-path-only",
        &[(
            STAMINA,
            "// @implements PERK-REGEN@1\n",
            "// @implements PERK-REGEN@1 [path]\n",
        )],
    );
    repo.replace(STAMINA, REGEN_BODY.0, REGEN_BODY.1);
    repo.replace(STAMINA, "rate: f32, dt: f32) {", "rate: f64, dt: f32) {");
    repo.commit_all("the body and the signature edited");
    let run = check_regen(
        &repo,
        &regen_expect(&repo, &["path"], &["path"], Outcome::PathOnly),
    );
    assert_eq!(
        (run.verdict(), run.code()),
        ("clean", 0),
        "{}",
        run.text.show()
    );
    // drain_system and BASE_REGEN by `ast`; the regen row by neither.
    assert_eq!(run.json["cascade"], json!({"by_blob": 3, "by_ast": 2}));
}

/// RD-5 (Q42): `lib.rs`' file module accepted under `[path]`, then a parse
/// error appended to its file: C is empty, the file never parsed for it:
/// `ok`, `compared` null, never `cannot_verify`; `build` beside it `ok` by
/// `ast`; `clean`, exit 0 (Q42's other answer: exit 2). M: a `[path]`
/// module's file parsed.
#[test]
fn path_only_module_ignores_a_parse_error_elsewhere() {
    let repo = ecs_with(
        "levels-path-module",
        &[(
            ECS_LIB,
            "//! @implements PERK-STAMINA@2\n",
            "//! @implements PERK-STAMINA@2 [path]\n",
        )],
    );
    repo.append(ECS_LIB, "\npub fn broken_helper() -> u32 {\n    1 +\n}\n");
    repo.commit_all("a parse error beside the module's items");
    let run = verify(&repo, &[]);
    let key = module("PERK-STAMINA", "implements", ECS_LIB, "crates/game");
    assert_row(
        &run,
        key,
        &Says {
            state: "ok",
            category: None,
            detail: None,
            pre_existing: false,
            markers: &[(2, Some(2))],
            node_rev: Some(2),
            lock: lock_entry(&repo.lock_text(), key),
            compared: None,
            levels: levels(&["path"], &["path"], None),
        },
        None,
    );
    let build = run.row(
        "PERK-STAMINA",
        "implements",
        ECS_LIB,
        Some("crates/game::StaminaPlugin::build"),
    );
    assert_eq!(
        (&build["state"], &build["compared"]),
        (&json!("ok"), &json!("ast"))
    );
    assert_eq!(
        (run.verdict(), run.code()),
        ("clean", 0),
        "{}",
        run.text.show()
    );
    let changed = verify(&repo, &["--changed"]);
    assert_eq!(changed.code(), 0, "--changed:\n{}", changed.text.show());
}

/// RD-5, M6: `regen_system` accepted under `[sig]`, then its body edited:
/// C is `{sig}`, the row `ok` by `ast`, `changed` empty, `clean`; its
/// signature edited too: `code_ahead code_changed sig`, `code`
/// `sig:<digest>`. M6: `sig` and `body` compared whatever the levels (the
/// body edit `code_changed body`).
#[test]
fn sig_marker_ignores_a_body_edit() {
    let repo = ecs_with(
        "levels-sig-marker",
        &[(
            STAMINA,
            "// @implements PERK-REGEN@1\n",
            "// @implements PERK-REGEN@1 [sig]\n",
        )],
    );
    repo.replace(STAMINA, REGEN_BODY.0, REGEN_BODY.1);
    repo.commit_all("the body edited");
    let run = check_regen(&repo, &regen_expect(&repo, &["sig"], &["sig"], Outcome::Ok));
    assert_eq!(
        (run.verdict(), run.code()),
        ("clean", 0),
        "{}",
        run.text.show()
    );
    repo.replace(STAMINA, "rate: f32, dt: f32) {", "rate: f32, dt: f64) {");
    repo.commit_all("the signature edited");
    check_regen(
        &repo,
        &regen_expect(&repo, &["sig"], &["sig"], Outcome::Changed(&["sig"])),
    );
}

/// TE-5, an `Invalid` list (Q43): `regen_system` accepted, its marker's
/// list then made `Invalid` (`[sig body]`) with its body edited: the row
/// is never compared, `bad_marker bad_levels`, `compared`, `levels` null,
/// the entry kept; no item raised for it. The same over `lib.rs`' file
/// module with a parse error elsewhere in its file: `bad_marker
/// bad_levels`, never `cannot_verify` (no exit 2: Q43's other answer).
/// M: an `Invalid` list compared as the default.
#[test]
fn invalid_list_is_never_compared() {
    let repo = ecs_with("levels-invalid", &[]);
    repo.replace(
        STAMINA,
        "// @implements PERK-REGEN@1\n",
        "// @implements PERK-REGEN@1 [sig body]\n",
    );
    repo.replace(STAMINA, REGEN_BODY.0, REGEN_BODY.1);
    repo.replace(
        ECS_LIB,
        "//! @implements PERK-STAMINA@2\n",
        "//! @implements PERK-STAMINA@2 [sig body]\n",
    );
    repo.append(ECS_LIB, "\npub fn broken_helper() -> u32 {\n    1 +\n}\n");
    repo.commit_all("invalid lists");
    let run = verify(&repo, &[]);
    let lock = repo.lock_text();
    for (key, markers, rev) in [
        (regen_key(), [(9, Some(1))], 1),
        (
            module("PERK-STAMINA", "implements", ECS_LIB, "crates/game"),
            [(2, Some(2))],
            2,
        ),
    ] {
        assert_row(
            &run,
            key,
            &Says {
                state: "bad_marker",
                category: Some("bad_levels"),
                detail: None,
                pre_existing: false,
                markers: &markers,
                node_rev: Some(rev),
                lock: lock_entry(&lock, key),
                compared: None,
                levels: Value::Null,
            },
            None,
        );
    }
    assert_ne!(run.code(), 2, "{}", run.text.show());
    assert!(
        run.rows_where(|row| row["state"] == "cannot_verify")
            .is_empty(),
        "{}",
        run.text.show()
    );
    let raised = raised(&repo, &[]);
    assert!(
        !raised.created_nodes().contains(&"PERK-REGEN".to_owned()),
        "{}",
        raised.text()
    );
}
