//! docs/features/hash-levels.md (5a-1), the library criteria over
//! `fixtures/hash-levels/` and inline sources:
//!
//! - TE-4: determinism. Two parser instances, two runs, reversed file order
//!   give byte-for-byte the same levels and fingerprints; no offset, line or
//!   neighbouring text enters a stream.
//! - TE-5: AC-15, the library half. A rename keeps `sig`, `body`, `deps`,
//!   `skeleton`, `quote`; a trivial body stays under 124 B in both committed
//!   rustfmt variants; two copies share `skeleton` and `body`; the threshold
//!   is `body_bytes < 124`.
//!
//! PHASE-B: the entry points by the spec's names, read through
//! `levels_common::unit_of` only.

mod levels_common;

use levels_common::{
    ALL, Key, Levels, STABLE, Snapshot, VARIANTS, base_files, changed, new_parser, on_big_stack,
    read, read_variant, snapshot, snapshot_with,
};
use specengine_code::TRIVIAL_BODY_BYTES;

/// Every unit of every snapshot, one line each: file, key, each field in
/// hex or the categories. Two equal dumps are byte-for-byte equal results.
fn dump(snapshots: &[(String, Snapshot)]) -> String {
    let mut out = String::new();
    for (file, snap) in snapshots {
        let units = snap
            .keys
            .iter()
            .map(ToString::to_string)
            .zip(&snap.units)
            .chain(std::iter::once(("module".to_owned(), &snap.module)));
        for (name, unit) in units {
            let line = match unit.levels() {
                Some(l) => format!(
                    "{file}\t{name}\tsig={} body={} deps={} skeleton={} quote={} body_bytes={} trivial={}\n",
                    l.sig, l.body, l.deps, l.skeleton, l.quote, l.body_bytes, l.trivial
                ),
                None => format!("{file}\t{name}\t{unit:?}\n"),
            };
            out.push_str(&line);
        }
    }
    out
}

fn levels_of<'a>(snap: &'a Snapshot, kind: &str, label: &str, owner: Option<&str>) -> &'a Levels {
    let hits: Vec<usize> = snap
        .keys
        .iter()
        .enumerate()
        .filter(|(_, k)| k.kind == kind && k.label == label && k.owner.as_deref() == owner)
        .map(|(index, _)| index)
        .collect();
    assert_eq!(hits.len(), 1, "one {kind} {label} in {:?}", snap.keys);
    snap.units[hits[0]]
        .levels()
        .unwrap_or_else(|| panic!("{kind} {label} is refused: {:?}", snap.units[hits[0]]))
}

// ------------------------------------------------------------------- TE-4

#[test]
fn te4_two_parser_instances_and_two_runs_give_the_same_bytes() {
    on_big_stack(|| {
        let mut first = new_parser();
        let mut second = new_parser();
        let mut runs: [Vec<(String, Snapshot)>; 3] = [Vec::new(), Vec::new(), Vec::new()];
        for file in base_files() {
            let text = read(&file);
            runs[0].push((file.clone(), snapshot_with(&mut first, &text)));
            runs[1].push((file.clone(), snapshot_with(&mut second, &text)));
            // A second run on the first parser, after the second parser's.
            runs[2].push((file, snapshot_with(&mut first, &text)));
        }
        let dumps = runs.map(|run| dump(&run));
        assert!(dumps[0].lines().count() > 80, "every unit dumped");
        assert_eq!(dumps[0], dumps[1], "two parser instances");
        assert_eq!(dumps[0], dumps[2], "two runs of one parser");
    });
}

#[test]
fn te4_reversed_file_order_gives_the_same_bytes() {
    on_big_stack(|| {
        let files = base_files();
        let mut parser = new_parser();
        let forward: Vec<(String, Snapshot)> = files
            .iter()
            .map(|file| (file.clone(), snapshot_with(&mut parser, &read(file))))
            .collect();
        let mut parser = new_parser();
        let mut backward: Vec<(String, Snapshot)> = files
            .iter()
            .rev()
            .map(|file| (file.clone(), snapshot_with(&mut parser, &read(file))))
            .collect();
        backward.reverse();
        assert_eq!(dump(&forward), dump(&backward), "reversed file order");
    });
}

/// No path, line, offset or time enters a stream: blank lines above a file
/// change no unit at all (the context lines skip blank lines), and a file
/// appended to another keeps every level and fingerprint but `quote` (its
/// first item gains a line before it).
#[test]
fn te4_no_offset_or_line_enters_a_level() {
    on_big_stack(|| {
        for file in base_files() {
            let text = read(&file);
            let plain = snapshot(&text);
            let shifted = snapshot(&format!("\n\n\n\n{text}"));
            assert_eq!(plain.keys, shifted.keys, "{file}");
            assert_eq!(
                plain.units, shifted.units,
                "{file}: blank lines above every item change no level or fingerprint"
            );
            match (plain.module.levels(), shifted.module.levels()) {
                (Some(a), Some(b)) => {
                    assert!(changed(a, b, &STABLE).is_empty(), "{file}: module");
                }
                (None, None) => assert_eq!(plain.module, shifted.module, "{file}: module"),
                pair => panic!("{file}: module refusal changed: {pair:?}"),
            }
        }
        let head = read("src/functions.rs");
        let tail = read("src/types.rs");
        let alone = snapshot(&tail);
        let appended = snapshot(&format!("{head}\n{tail}"));
        for (index, key) in alone.keys.iter().enumerate() {
            let at = appended
                .index_of(key)
                .unwrap_or_else(|| panic!("{key} in the appended file"));
            let (Some(a), Some(b)) = (alone.units[index].levels(), appended.units[at].levels())
            else {
                panic!("{key}: refused");
            };
            let moved = changed(a, b, &STABLE);
            assert!(moved.is_empty(), "{key}: {moved:?} changed with its offset");
        }
    });
}

// ------------------------------------------------------------------- TE-5

/// A rename case: the kind, the template, the owner, the old and the new
/// name, what it is.
type Rename = (
    &'static str,
    &'static str,
    Option<&'static str>,
    &'static str,
    &'static str,
    &'static str,
);

/// Templates with `NAME` for the item's name; each is wrapped in context
/// lines, so `quote`'s `before` and `after` are not empty.
const RENAMES: [Rename; 14] = [
    (
        "function_item",
        "/// Adds one.\n#[inline]\npub fn NAME(x: u8) -> u8 { helper(x) + 1 }",
        None,
        "old_name",
        "renamed_item",
        "a function",
    ),
    (
        "struct_item",
        "#[derive(Debug)]\npub struct NAME { field: u8, other: [u8; size()] }",
        None,
        "OldName",
        "RenamedItem",
        "a struct",
    ),
    (
        "struct_item",
        "pub struct NAME(u8, u16);",
        None,
        "OldName",
        "RenamedItem",
        "a tuple struct",
    ),
    (
        "enum_item",
        "#[repr(u8)]\npub enum NAME { A, B(u8) }",
        None,
        "OldName",
        "RenamedItem",
        "an enum",
    ),
    (
        "union_item",
        "pub union NAME { a: u8, b: i8 }",
        None,
        "OldName",
        "RenamedItem",
        "a union",
    ),
    (
        "trait_item",
        "pub trait NAME: Clone { fn run(&self) -> u8 { tick() } }",
        None,
        "OldName",
        "RenamedItem",
        "a trait",
    ),
    (
        "mod_item",
        "pub mod NAME { pub fn f() -> u8 { one() } }",
        None,
        "old_name",
        "renamed_item",
        "an inline mod",
    ),
    (
        "mod_item",
        "mod NAME;",
        None,
        "old_name",
        "renamed_item",
        "a mod declaration",
    ),
    (
        "const_item",
        "pub const NAME: u8 = limit();",
        None,
        "OLD_NAME",
        "RENAMED_ITEM",
        "a const",
    ),
    (
        "static_item",
        "static NAME: &str = \"x\";",
        None,
        "OLD_NAME",
        "RENAMED_ITEM",
        "a static",
    ),
    (
        "type_item",
        "pub type NAME = Vec<u8>;",
        None,
        "OldName",
        "RenamedItem",
        "an alias",
    ),
    (
        "macro_definition",
        "macro_rules! NAME { ($x:expr) => { $x + 1 }; }",
        None,
        "old_name",
        "renamed_item",
        "a macro",
    ),
    (
        "function_signature_item",
        "pub trait Holder { fn NAME(&self) -> u8; }",
        Some("Holder"),
        "old_name",
        "renamed_item",
        "a signature",
    ),
    (
        "function_item",
        "impl Holder for Thing { fn NAME(&self) -> u8 { self.0 } }",
        Some("<Thing as Holder>"),
        "old_name",
        "renamed_item",
        "a method",
    ),
];

#[test]
fn te5_rename_keeps_sig_body_deps_skeleton_and_quote() {
    on_big_stack(|| {
        for (kind, template, owner, old, new, what) in RENAMES {
            assert_eq!(template.matches("NAME").count(), 1, "{what}: one name");
            let wrap = |name: &str| {
                format!(
                    "use std::fmt;\n\n{}\n// the line after\n",
                    template.replace("NAME", name)
                )
            };
            let before = snapshot(&wrap(old));
            let after = snapshot(&wrap(new));
            let a = levels_of(&before, kind, old, owner);
            let b = levels_of(&after, kind, new, owner);
            let moved = changed(a, b, &ALL);
            assert!(moved.is_empty(), "{what}: a rename changed {moved:?}");
            let v2 = |snap: &Snapshot, label: &str| {
                let index = snap
                    .keys
                    .iter()
                    .position(|k: &Key| k.kind == kind && k.label == label)
                    .expect("the renamed item");
                snap.v2_digest(index)
            };
            assert_eq!(
                v2(&before, old),
                v2(&after, new),
                "{what}: v2 keeps a rename too"
            );
        }
    });
}

/// AC-15: triviality is a property of the body, not of its layout.
#[test]
fn te5_trivial_body_stays_under_124_bytes_in_both_rustfmt_variants() {
    on_big_stack(|| {
        let file = "src/functions.rs";
        for (label, trivial) in [
            ("trivial_hp", true),
            ("copy_one", true),
            ("clamp_score", true),
            ("long_tick", false),
        ] {
            let mut counts = Vec::new();
            for text in std::iter::once(read(file))
                .chain(VARIANTS.iter().map(|variant| read_variant(variant, file)))
            {
                let snap = snapshot(&text);
                let levels = levels_of(&snap, "function_item", label, None);
                assert_eq!(levels.trivial, trivial, "{label}: is_trivial");
                assert_eq!(
                    levels.body_bytes < TRIVIAL_BODY_BYTES,
                    trivial,
                    "{label}: {} body bytes",
                    levels.body_bytes
                );
                counts.push(levels.body_bytes);
            }
            assert!(
                counts.windows(2).all(|pair| pair[0] == pair[1]),
                "{label}: body_bytes across the variants {counts:?}"
            );
        }
        // Every item of every variant file keeps its side of 124.
        for file in levels_common::variant_files() {
            let base = snapshot(&read(&file));
            for variant in VARIANTS {
                let other = snapshot(&read_variant(variant, &file));
                for (index, key) in base.keys.iter().enumerate() {
                    let at = other.index_of(key).expect("matched by key");
                    if let (Some(a), Some(b)) =
                        (base.units[index].levels(), other.units[at].levels())
                    {
                        assert_eq!(a.trivial, b.trivial, "{variant}/{file}: {key}");
                    }
                }
            }
        }
    });
}

#[test]
fn te5_two_copies_share_skeleton_and_body() {
    on_big_stack(|| {
        let file = "src/functions.rs";
        for text in std::iter::once(read(file))
            .chain(VARIANTS.iter().map(|variant| read_variant(variant, file)))
        {
            let snap = snapshot(&text);
            let one = levels_of(&snap, "function_item", "copy_one", None);
            let two = levels_of(&snap, "function_item", "copy_two", None);
            assert_eq!(
                changed(one, two, &ALL),
                ["quote"],
                "copies share every level and fingerprint but quote"
            );
        }
        // Copies in two files, with other context lines.
        let first = snapshot("use a::b;\n\n#[derive(Clone)]\nstruct Cell { x: u8, y: [u8; 4] }\n");
        let second = snapshot(
            "// elsewhere\nfn other() {}\n#[derive(Clone)]\nstruct Cell { x: u8, y: [u8; 4] }\n// end\n",
        );
        let a = levels_of(&first, "struct_item", "Cell", None);
        let b = levels_of(&second, "struct_item", "Cell", None);
        assert_eq!(changed(a, b, &ALL), ["quote"], "copies across files");
        // A structural twin: other names, types and literals, the same skeleton.
        let twins = snapshot(
            "fn a(x: u8) -> u8 { let y = x + 1; y * 2 }\nfn b(z: u16) -> u16 { let w = z + 7; w * 3 }\n",
        );
        let a = levels_of(&twins, "function_item", "a", None);
        let b = levels_of(&twins, "function_item", "b", None);
        assert_eq!(
            a.skeleton, b.skeleton,
            "renamed locals, types and literals keep the skeleton"
        );
        assert_ne!(a.body, b.body);
        assert_ne!(a.sig, b.sig);
    });
}

/// `body_bytes` of `{ a; a; ... }` is 2 + 2 per `a;` (`ab;` counts 3):
/// trivial strictly below 124, in any layout.
#[test]
fn te5_triviality_boundary_is_124_body_bytes() {
    assert_eq!(TRIVIAL_BODY_BYTES, 124);
    on_big_stack(|| {
        for (long, short, bytes) in [(0, 60, 122u32), (1, 59, 123), (0, 61, 124), (1, 60, 125)] {
            let statements: Vec<&str> = std::iter::repeat_n("ab;", long)
                .chain(std::iter::repeat_n("a;", short))
                .collect();
            for (layout, source) in [
                (
                    "one line",
                    format!("fn f() {{ {} }}\n", statements.join(" ")),
                ),
                (
                    "one per line",
                    format!("fn f() {{\n    {}\n}}\n", statements.join("\n    ")),
                ),
            ] {
                let snap = snapshot(&source);
                let levels = levels_of(&snap, "function_item", "f", None);
                assert_eq!(levels.body_bytes, bytes, "{layout}: body_bytes");
                assert_eq!(
                    levels.trivial,
                    bytes < 124,
                    "{layout}: {bytes} body bytes is_trivial"
                );
            }
        }
    });
}
