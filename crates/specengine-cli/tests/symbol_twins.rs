//! docs/features/symbol-discriminator.md (6a-1) end to end: RD-1's CLI
//! tests (a marked adjacent impl locked by its discriminated key; a locked
//! item gaining a twin; a locked twin's ordinal shifted), the
//! cross-file `duplicate` that stays unlockable, the same rules on the
//! ECS-style `fixtures/verify-ecs` (ADR-0008: two projects of different
//! nature), and TE-3 (`spec refs` byte-identical under LF and CRLF, from
//! two roots and a subdirectory).
//!
//! Scratch git repositories through slice 3's `lock_common` (`LockRepo`:
//! sandboxed git, fixed dates, a scratch `HOME`), `spec verify` through
//! `verify_common`; every twin is written at test time into the scratch
//! copy, the fixtures are only read. Expected rows are built from the
//! spec's "Rules and edge cases" and `docs/canon/spec-verify.md` "States",
//! never from the command's own output.

mod common;
mod lock_common;
mod verify_common;

use std::path::Path;

use serde_json::Value;

use common::{Scratch, snapshot, spec, write};
use lock_common::{LockRepo, blocks, item_code, value};
use verify_common::{
    COUNTER, Key, STAMINA, Says, assert_row, ecs, golden, golden_lock, item, lock_entry, verify,
};

/// Says with no entry and no comparison.
fn says<'a>(
    state: &'a str,
    category: Option<&'a str>,
    detail: Option<&'a str>,
    markers: &'a [(usize, Option<u32>)],
    node_rev: Option<u32>,
) -> Says<'a> {
    Says {
        state,
        category,
        detail,
        pre_existing: false,
        markers,
        node_rev,
        lock: Value::Null,
        compared: None,
    }
}

/// The marker of the first `impl Tally` block, made current (LAW-BOUND is
/// at rev 2), so the block is lockable.
const FIRST_IMPL: &str = "impl Tally { // @implements LAW-BOUND@3";
const FIRST_IMPL_CURRENT: &str = "impl Tally { // @implements LAW-BOUND@2";

const IMPL_1: &str = "crates/tally::counter::impl Tally #1";
const IMPL_2: &str = "crates/tally::counter::impl Tally #2";
const IMPL_3: &str = "crates/tally::counter::impl Tally #3";

/// code-a with the first `impl Tally`'s marker current, committed, and
/// LAW-BOUND accepted: the lock holds `impl Tally #1`.
fn locked_first_impl(label: &str) -> LockRepo {
    let repo = LockRepo::code_a(label);
    repo.replace(COUNTER, FIRST_IMPL, FIRST_IMPL_CURRENT);
    repo.commit_all("the first impl Tally's marker at the node's revision");
    repo.accept_ok("LAW-BOUND");
    repo
}

/// The `lock` object of `text`'s entry for `key`. The lock writes TOML
/// basic strings, and `lock_common::blocks` reads them back unescaped only
/// of their quotes, so a `"` of a `cfg` value in a symbol is looked up as
/// `\"`.
fn lock_of(text: &str, key: Key<'_>) -> Value {
    let escaped = key
        .symbol
        .map(|symbol| symbol.replace('\\', "\\\\").replace('"', "\\\""));
    lock_entry(
        text,
        Key {
            symbol: escaped.as_deref(),
            ..key
        },
    )
}

/// The 1-based line of the `nth` (1-based) line of `text` equal to `line`.
fn nth_line(text: &str, line: &str, nth: usize) -> usize {
    text.lines()
        .enumerate()
        .filter(|(_, l)| *l == line)
        .nth(nth - 1)
        .map(|(at, _)| at + 1)
        .unwrap_or_else(|| panic!("no {nth}. line {line:?} in:\n{text}"))
}

// ------------------------------------------------------------------ RD-1

/// RD-1 ("Who sees it", spec-lock "Lockability"): a marker on one of two
/// adjacent `impl Tally` blocks is locked under its discriminated key
/// (`impl Tally #1`, its levels' digests by `item_levels`: docs/features/
/// lock-levels.md "Format 2"), the other twin's marker
/// (`@3a`) stays `not locked … bad_rev` under `#2`; verify reads the
/// locked twin `ok` by blob. M: in-file twins kept ambiguous.
#[test]
fn marked_adjacent_impl_is_lockable() {
    let repo = LockRepo::code_a("twin-lockable");
    repo.replace(COUNTER, FIRST_IMPL, FIRST_IMPL_CURRENT);
    repo.commit_all("the first impl Tally's marker at the node's revision");
    let attempt = repo.accept("LAW-BOUND", true);
    attempt.committed("accept LAW-BOUND");
    let rows = attempt.rows();
    for row in [
        format!("new | implements | {COUNTER} | item {IMPL_1} impl_item"),
        format!("not locked | implements | {COUNTER} | item {IMPL_2} impl_item | bad_rev"),
    ] {
        assert!(
            rows.lines().any(|line| line == row),
            "no {row:?} in:\n{rows}"
        );
    }
    let lock = repo.lock_text();
    let twins: Vec<Vec<(String, String)>> = blocks(&lock)
        .into_iter()
        .filter(|block| {
            value(block, "symbol").is_some_and(|symbol| symbol.contains("::impl Tally"))
        })
        .collect();
    assert_eq!(twins.len(), 1, "{lock}");
    assert_eq!(value(&twins[0], "symbol"), Some(IMPL_1));
    assert_eq!(value(&twins[0], "item_kind"), Some("impl_item"));
    let code = item_code(
        repo.read(COUNTER).as_bytes(),
        22,
        "impl_item",
        &["sig", "body"],
    );
    for (key, digest) in [
        ("sig_hash", &code.sig),
        ("body_hash", &code.body),
        ("deps_hash", &code.deps),
    ] {
        assert_eq!(value(&twins[0], key), Some(digest.as_str()), "{key}");
    }
    assert_eq!(value(&twins[0], "ast_hash"), None);
    let run = verify(&repo, &[]);
    let first = item("LAW-BOUND", "implements", COUNTER, IMPL_1, "impl_item");
    assert_row(
        &run,
        first,
        &Says {
            lock: lock_entry(&lock, first),
            compared: Some("blob"),
            ..says("ok", None, None, &[(22, Some(2))], Some(2))
        },
        None,
    );
    assert_row(
        &run,
        item("LAW-BOUND", "implements", COUNTER, IMPL_2, "impl_item"),
        &says("bad_marker", Some("bad_rev"), None, &[(45, None)], Some(2)),
        None,
    );
}

/// RD-1 ("Rules and edge cases": an item locked under a unique qpath gains
/// an in-file twin): the golden lock's `Tally::value` (GOAL-01) gains a
/// `cfg(test)` twin in the second `impl Tally`; the original lacks a
/// `cfg`, so the two take ordinals. The entry reads `broken_binding
/// symbol_gone`, its marker's new key `Tally::value #1` `unlocked`; the
/// unmarked twin makes no row. M: an undiscriminated locked key matched to
/// `#1` once a twin appears.
#[test]
fn twin_added_after_lock_breaks_the_entry() {
    let repo = golden("twin-after-lock");
    repo.replace(
        COUNTER,
        "        self.count = 0;\n    }\n",
        "        self.count = 0;\n    }\n\n    #[cfg(test)]\n    pub fn value(&self) -> u32 {\n        0\n    }\n",
    );
    let run = verify(&repo, &[]);
    let entry = item(
        "GOAL-01",
        "implements",
        COUNTER,
        "crates/tally::counter::Tally::value",
        "function_item",
    );
    assert_row(
        &run,
        entry,
        &Says {
            lock: golden_lock(entry),
            ..says("broken_binding", Some("symbol_gone"), None, &[], Some(1))
        },
        None,
    );
    assert_row(
        &run,
        item(
            "GOAL-01",
            "implements",
            COUNTER,
            "crates/tally::counter::Tally::value #1",
            "function_item",
        ),
        &says("unlocked", None, None, &[(29, Some(1))], Some(1)),
        None,
    );
    assert!(
        run.rows_where(|row| row["symbol"] == "crates/tally::counter::Tally::value #2")
            .is_empty(),
        "{}",
        run.text.show()
    );
}

/// RD-1 ("Rules and edge cases", Risks "Ordinals shift"): `impl Tally #1`
/// locked, then a third `impl Tally` inserted above it. The locked key now
/// names the inserted, unmarked block: `broken_binding marker_gone`; the
/// marker's new key `#2` is `unlocked`; the old `#2` is `#3` (`bad_marker
/// bad_rev`); no twin row reads `ok`. M: a shifted key matched by ordinal
/// fallback.
#[test]
fn inserted_twin_never_reads_all_ok() {
    let repo = locked_first_impl("twin-shift");
    let lock = repo.lock_text();
    repo.replace(
        COUNTER,
        FIRST_IMPL_CURRENT,
        &format!("impl Tally {{}}\n\n{FIRST_IMPL_CURRENT}"),
    );
    let text = repo.read(COUNTER);
    let marker = nth_line(&text, FIRST_IMPL_CURRENT, 1);
    let closing = nth_line(&text, "} // @implements LAW-BOUND@3a", 1);
    assert_eq!((marker, closing), (24, 47), "two lines inserted above");
    let run = verify(&repo, &[]);
    let first = item("LAW-BOUND", "implements", COUNTER, IMPL_1, "impl_item");
    assert_row(
        &run,
        first,
        &Says {
            lock: lock_entry(&lock, first),
            ..says("broken_binding", Some("marker_gone"), None, &[], Some(2))
        },
        None,
    );
    assert_row(
        &run,
        item("LAW-BOUND", "implements", COUNTER, IMPL_2, "impl_item"),
        &says("unlocked", None, None, &[(marker, Some(2))], Some(2)),
        None,
    );
    assert_row(
        &run,
        item("LAW-BOUND", "implements", COUNTER, IMPL_3, "impl_item"),
        &says(
            "bad_marker",
            Some("bad_rev"),
            None,
            &[(closing, None)],
            Some(2),
        ),
        None,
    );
    let ok: Vec<&Value> = run.rows_where(|row| {
        row["node"] == "LAW-BOUND"
            && row["symbol"]
                .as_str()
                .is_some_and(|symbol| symbol.starts_with("crates/tally::counter::impl Tally"))
            && row["state"] == "ok"
    });
    assert!(ok.is_empty(), "{ok:?}\n{}", run.text.show());
}

/// "Who sees it": `spec verify` keys the twins' rows by the discriminated
/// qpath on every side it reads, the disk, `--changed`'s base (HEAD) and
/// `--staged`'s index: over the golden repository, unchanged, each side
/// gives `impl Tally #1` `predated` and `impl Tally #2` `bad_marker
/// bad_rev`, Pre-existing where a base is read, and no row keys the
/// undiscriminated `impl Tally`. M: one side naming through its own,
/// undiscriminated path.
#[test]
fn twin_keys_are_the_same_on_every_side() {
    let repo = golden("twin-sides");
    for (side, args, pre_existing) in [
        ("disk", &[][..], false),
        ("--changed", &["--changed"][..], true),
        ("--staged", &["--staged"][..], true),
    ] {
        let run = verify(&repo, args);
        for (symbol, state, category) in [
            (IMPL_1, "predated", Value::Null),
            (IMPL_2, "bad_marker", Value::from("bad_rev")),
        ] {
            let row = run.row("LAW-BOUND", "implements", COUNTER, Some(symbol));
            assert_eq!(
                (&row["state"], &row["category"], &row["pre_existing"]),
                (&Value::from(state), &category, &Value::from(pre_existing)),
                "{side}: {symbol}\n{}",
                run.text.show()
            );
        }
        assert!(
            run.rows_where(|row| row["symbol"] == "crates/tally::counter::impl Tally")
                .is_empty(),
            "{side}:\n{}",
            run.text.show()
        );
    }
}

/// The two files of a cross-file pair (`src/x.rs` beside `src/x/mod.rs`),
/// each with one current marker on `fn twin`.
const PAIR_FILES: [&str; 2] = ["crates/tally/src/pair.rs", "crates/tally/src/pair/mod.rs"];
const PAIR_SOURCE: &str = "// @implements LAW-STEP@1\npub fn twin() {}\n";

/// RD-1 ("Ambiguity": `duplicate` means another file), spec-lock
/// "Lockability": a cross-file pair stays `ambiguous duplicate`, never
/// discriminated, never an entry; the rest of LAW-STEP is locked. M:
/// cross-file twins discriminated.
#[test]
fn cross_file_twin_is_never_locked() {
    let repo = LockRepo::code_a("twin-cross-file");
    for file in PAIR_FILES {
        repo.write(file, PAIR_SOURCE);
    }
    repo.commit_all("a cross-file pair");
    let attempt = repo.accept("LAW-STEP", true);
    attempt.committed("accept LAW-STEP");
    let rows = attempt.rows();
    for file in PAIR_FILES {
        let row = format!(
            "not locked | implements | {file} | item crates/tally::pair::twin function_item | ambiguous duplicate"
        );
        assert!(
            rows.lines().any(|line| line == row),
            "no {row:?} in:\n{rows}"
        );
    }
    let lock = repo.lock_text();
    assert!(!lock.contains("crates/tally/src/pair"), "{lock}");
    assert!(!lock.contains("pair::twin #"), "{lock}");
}

/// ADR-0008 on the ECS-style fixture (RD-1 rule 1, "Locks"): the accepted
/// `regen_system` gains a `cfg(feature = "dev")` twin and itself a
/// `cfg(not(feature = "dev"))`. Before a new accept its entry reads
/// `symbol_gone` and both twins' keys, named by `cfg`, `unlocked`; after
/// one, both twins are entries and the fixture verifies `clean`. M: the
/// ordinal tried first; in-file twins kept ambiguous.
#[test]
fn ecs_system_cfg_twins_lock_and_verify_clean() {
    let repo = ecs("twin-ecs");
    let before_lock = repo.lock_text();
    repo.replace(
        STAMINA,
        "// @implements PERK-REGEN@1\npub fn regen_system",
        "// @implements PERK-REGEN@1\n#[cfg(not(feature = \"dev\"))]\npub fn regen_system",
    );
    repo.append(
        STAMINA,
        "\n// @implements PERK-REGEN@1\n#[cfg(feature = \"dev\")]\npub fn regen_system(stamina: &mut Stamina, _rate: f32, _dt: f32) {\n    stamina.current = stamina.max;\n}\n",
    );
    let text = repo.read(STAMINA);
    let rest_line = nth_line(&text, "// @implements PERK-REGEN@1", 1);
    let dev_line = nth_line(&text, "// @implements PERK-REGEN@1", 2);
    let old = item(
        "PERK-REGEN",
        "implements",
        STAMINA,
        "crates/game::stamina::regen_system",
        "function_item",
    );
    let rest = item(
        "PERK-REGEN",
        "implements",
        STAMINA,
        "crates/game::stamina::regen_system #cfg(not(feature=\"dev\"))",
        "function_item",
    );
    let dev = item(
        "PERK-REGEN",
        "implements",
        STAMINA,
        "crates/game::stamina::regen_system #cfg(feature=\"dev\")",
        "function_item",
    );
    let before = verify(&repo, &[]);
    assert_row(
        &before,
        old,
        &Says {
            lock: lock_of(&before_lock, old),
            ..says("broken_binding", Some("symbol_gone"), None, &[], Some(1))
        },
        None,
    );
    for (key, line) in [(rest, rest_line), (dev, dev_line)] {
        assert_row(
            &before,
            key,
            &says("unlocked", None, None, &[(line, Some(1))], Some(1)),
            None,
        );
    }
    repo.commit_all("cfg twins of the regen system");
    repo.accept_ok("PERK-REGEN");
    let lock = repo.lock_text();
    assert!(
        !lock.contains("symbol = \"crates/game::stamina::regen_system\""),
        "the old key is dropped:\n{lock}"
    );
    let after = verify(&repo, &[]);
    assert_eq!(
        (after.verdict(), after.code()),
        ("clean", 0),
        "{}",
        after.text.show()
    );
    for (key, line) in [(rest, rest_line), (dev, dev_line)] {
        assert_row(
            &after,
            key,
            &Says {
                lock: lock_of(&lock, key),
                compared: Some("blob"),
                ..says("ok", None, None, &[(line, Some(1))], Some(1))
            },
            None,
        );
    }
}

// ------------------------------------------------------------------ TE-3

/// Twins of every rule, written into a code-a copy at test time: `cfg`
/// twins whose first `cfg` spans lines, twin inline mods (a `//!` marker
/// inside one), ordinal impl twins.
const TWINS: &str = "\
//! Twins named apart.

// @implements LAW-STEP
#[cfg(all(
    test,
    unix
))]
pub fn regen() {}

// @implements LAW-STEP
#[cfg(not(all(test, unix)))]
pub fn regen() {}

#[cfg(unix)]
mod os {
    //! @implements LAW-STEP
    pub fn open() {}
}

#[cfg(not(unix))]
mod os {
    pub fn open() {}
}

// @implements LAW-STEP
impl Gauge {}

impl Gauge {
    pub fn read(&self) {}
}
";

/// The rows `spec refs LAW-STEP` prints for [`TWINS`].
const TWIN_ROWS: [&str; 4] = [
    "implements | crates/tally/src/twins.rs:3:4 | LAW-STEP | resolved | unversioned | default | item crates/tally::twins::regen #cfg(all(test,unix)) function_item",
    "implements | crates/tally/src/twins.rs:10:4 | LAW-STEP | resolved | unversioned | default | item crates/tally::twins::regen #cfg(not(all(test,unix))) function_item",
    "implements | crates/tally/src/twins.rs:16:9 | LAW-STEP | resolved | unversioned | default | module crates/tally::twins::os #cfg(unix)",
    "implements | crates/tally/src/twins.rs:25:4 | LAW-STEP | resolved | unversioned | default | item crates/tally::twins::impl Gauge #1 impl_item",
];

/// Every `.rs` and `.ron` file under `root`, its LF made CRLF.
fn to_crlf(root: &Path) {
    for (path, bytes) in snapshot(root) {
        let Some(bytes) = bytes else { continue };
        if path.ends_with(".rs") || path.ends_with(".ron") {
            let text = String::from_utf8(bytes).expect("UTF-8 code");
            assert!(!text.contains('\r'), "{path} is LF to begin with");
            write(root, &path, text.replace('\n', "\r\n"));
        }
    }
}

/// TE-3 ("Rules and edge cases": deterministic under LF or CRLF, from any
/// root or cwd): two copies of code-a with [`TWINS`] in different places,
/// one LF read from its root, one CRLF read from a subdirectory, print the
/// same `spec refs` bytes, text and JSON, and a second run the same again;
/// the twins' rows are discriminated. M: a CR kept in a `cfg` value; a
/// name depending on the root.
#[test]
fn twin_names_are_equal_under_lf_crlf_and_two_roots() {
    let scratch = Scratch::new("twin-determinism");
    let lf = scratch.copy("code-a", "one/code-a");
    let crlf = scratch.copy("code-a", "two/deeper/copy");
    for root in [&lf, &crlf] {
        write(root, "crates/tally/src/twins.rs", TWINS);
    }
    to_crlf(&crlf);
    let (lf_home, crlf_home) = (scratch.home("lf"), scratch.home("crlf"));
    let below = crlf.join("crates/tally");
    for args in [
        &["refs", "LAW-STEP"][..],
        &["--json", "refs", "LAW-STEP"][..],
    ] {
        let a = spec(&lf_home, &lf, args);
        a.code(0);
        let b = spec(&crlf_home, &below, args);
        b.code(0);
        assert_eq!(a.stdout, b.stdout, "{args:?}");
        let again = spec(&lf_home, &lf, args);
        assert_eq!(again.stdout, a.stdout, "a second run of {args:?}");
        if args[0] == "refs" {
            for row in TWIN_ROWS {
                assert!(
                    a.stdout.lines().any(|line| line == row),
                    "no {row:?} in:\n{}",
                    a.stdout
                );
            }
        }
    }
}
