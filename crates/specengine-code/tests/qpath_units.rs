//! `qpath` target units (docs/features/layer-a-identity.md, "Units and
//! `qpath`", AC-01): `file_role` against a package's target table — the one
//! `cargo metadata --no-deps` gives for `fixtures/cargo-units/app` and the
//! one `layout_targets` finds — and the primary-target and shared-unit rules.
//!
//! The metadata table is written out here as Cargo reports it for the
//! fixture (`ast_hash_units.rs` checks the real `cargo metadata` answer end
//! to end); paths are relative to the package directory.

use std::path::{Path, PathBuf};

use specengine_code::qpath::{
    self, FileRole, PackageTargets, Target, TargetIndex, TargetSource, Unit, UnitKind,
    layout_targets,
};

fn target(kind: &str, name: &str, root: &str) -> Target {
    Target {
        kind: kind.to_owned(),
        name: name.to_owned(),
        root: PathBuf::from(root),
    }
}

/// `cargo metadata` of `fixtures/cargo-units/app`, in Cargo's order.
fn metadata_table() -> PackageTargets {
    PackageTargets {
        source: TargetSource::Metadata,
        targets: vec![
            target("lib", "unit_app", "src/lib.rs"),
            target("bin", "app-cli", "src/main.rs"),
            target("bin", "m", "src/bin/m/main.rs"),
            target("bin", "t", "src/bin/t.rs"),
            target("bin", "x", "tools/x.rs"),
            target("example", "d", "examples/d.rs"),
            target("example", "e", "examples/e/main.rs"),
            target("test", "a", "tests/a.rs"),
            target("test", "s", "tests/s/main.rs"),
            target("bench", "b", "benches/b.rs"),
            target("custom-build", "build-script-build", "build.rs"),
        ],
    }
}

/// Every file of the AC-01 table, relative to the package directory.
const FILES: [&str; 23] = [
    "build.rs",
    "benches/b.rs",
    "examples/d.rs",
    "examples/e/main.rs",
    "lib.rs",
    "scripts/x.rs",
    "src/a.rs",
    "src/a/b.rs",
    "src/a/mod.rs",
    "src/bin/m/cli.rs",
    "src/bin/m/main.rs",
    "src/bin/t.rs",
    "src/lib.rs",
    "src/main.rs",
    "tests/a.rs",
    "tests/common/house.rs",
    "tests/common/mod.rs",
    "tests/s/h.rs",
    "tests/s/main.rs",
    "tools/x.rs",
    "src/notes.txt",
    "tests/loose/deeper/x.rs",
    "benches/data/mod.rs",
];

fn unit(kind: &str, name: &str) -> Option<Unit> {
    Some(Unit {
        kind: UnitKind::Target(kind.to_owned()),
        name: name.to_owned(),
    })
}

fn shared(dir: &str) -> Option<Unit> {
    Some(Unit {
        kind: UnitKind::Shared,
        name: dir.to_owned(),
    })
}

fn module(unit: Option<Unit>, parts: &[&str]) -> FileRole {
    FileRole::Module(unit, parts.iter().map(|p| (*p).to_owned()).collect())
}

fn role(path: &str, table: &PackageTargets) -> FileRole {
    qpath::file_role(Path::new(path), table)
}

/// The AC-01 table; `main_rs` and `tools_x` are what differs between the
/// metadata table and the layout.
fn ac01_cases(main_rs: FileRole, tools_x: FileRole) -> Vec<(&'static str, FileRole)> {
    vec![
        ("src/lib.rs", FileRole::CrateRoot(None)),
        ("src/a/b.rs", module(None, &["a", "b"])),
        ("src/a.rs", module(None, &["a"])),
        ("src/a/mod.rs", module(None, &["a"])),
        ("src/main.rs", main_rs),
        (
            "build.rs",
            FileRole::CrateRoot(unit("custom-build", "build-script-build")),
        ),
        ("src/bin/t.rs", FileRole::CrateRoot(unit("bin", "t"))),
        ("src/bin/m/main.rs", FileRole::CrateRoot(unit("bin", "m"))),
        ("src/bin/m/cli.rs", module(unit("bin", "m"), &["cli"])),
        ("examples/d.rs", FileRole::CrateRoot(unit("example", "d"))),
        (
            "examples/e/main.rs",
            FileRole::CrateRoot(unit("example", "e")),
        ),
        ("tests/a.rs", FileRole::CrateRoot(unit("test", "a"))),
        ("tests/s/main.rs", FileRole::CrateRoot(unit("test", "s"))),
        ("tests/s/h.rs", module(unit("test", "s"), &["h"])),
        ("tests/common/mod.rs", module(shared("tests/common"), &[])),
        (
            "tests/common/house.rs",
            module(shared("tests/common"), &["house"]),
        ),
        // A deeper directory of a shared dir: modules below `<x>/`.
        (
            "tests/loose/deeper/x.rs",
            module(shared("tests/loose"), &["deeper", "x"]),
        ),
        ("benches/data/mod.rs", module(shared("benches/data"), &[])),
        ("benches/b.rs", FileRole::CrateRoot(unit("bench", "b"))),
        ("tools/x.rs", tools_x),
        ("scripts/x.rs", FileRole::Unrooted),
        ("lib.rs", FileRole::Unrooted),
        ("src/notes.txt", FileRole::Unrooted),
    ]
}

#[test]
fn metadata_table_puts_every_file_in_its_unit() {
    let table = metadata_table();
    let cases = ac01_cases(
        FileRole::CrateRoot(unit("bin", "app-cli")),
        FileRole::CrateRoot(unit("bin", "x")),
    );
    let mut failures = Vec::new();
    for (path, expected) in &cases {
        let got = role(path, &table);
        if &got != expected {
            failures.push(format!("{path}: expected {expected:?}, got {got:?}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn shared_directory_file_is_never_a_crate_root() {
    // `tests/common/mod.rs` is what several test crates `mod`-include: a
    // module of the shared unit, empty module path, not a root.
    let table = metadata_table();
    let got = role("tests/common/mod.rs", &table);
    assert!(
        !matches!(got, FileRole::CrateRoot(_)),
        "tests/common/mod.rs must not be a crate root: {got:?}"
    );
    assert_eq!(got.unit(), shared("tests/common").as_ref());
}

#[test]
fn layout_targets_follow_cargo_auto_discovery() {
    let table = layout_targets("unit-app", FILES.iter().map(Path::new));
    assert_eq!(table.source, TargetSource::Layout);
    assert_eq!(
        table.targets,
        vec![
            target("lib", "unit_app", "src/lib.rs"),
            target("bin", "unit-app", "src/main.rs"),
            target("bin", "m", "src/bin/m/main.rs"),
            target("bin", "t", "src/bin/t.rs"),
            target("example", "d", "examples/d.rs"),
            target("example", "e", "examples/e/main.rs"),
            target("test", "a", "tests/a.rs"),
            target("test", "s", "tests/s/main.rs"),
            target("bench", "b", "benches/b.rs"),
            target("custom-build", "build-script-build", "build.rs"),
        ],
        "a `[[bin]]` at a custom path is unknown to the layout"
    );
    // The order of the listed files does not matter.
    let mut reversed: Vec<&str> = FILES.to_vec();
    reversed.reverse();
    assert_eq!(
        layout_targets("unit-app", reversed.iter().map(Path::new)),
        table
    );
}

#[test]
fn layout_table_gives_the_same_units_but_the_custom_path_bin_is_unrooted() {
    let table = layout_targets("unit-app", FILES.iter().map(Path::new));
    let cases = ac01_cases(
        FileRole::CrateRoot(unit("bin", "unit-app")),
        FileRole::Unrooted,
    );
    let mut failures = Vec::new();
    for (path, expected) in &cases {
        let got = role(path, &table);
        if &got != expected {
            failures.push(format!("{path}: expected {expected:?}, got {got:?}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn default_bin_is_primary_without_a_library() {
    let files = ["src/main.rs", "src/cli.rs", "src/bin/t.rs", "tests/a.rs"];
    for table in [
        layout_targets("binonly", files.iter().map(Path::new)),
        PackageTargets {
            source: TargetSource::Metadata,
            targets: vec![
                target("bin", "binonly", "src/main.rs"),
                target("bin", "t", "src/bin/t.rs"),
                target("test", "a", "tests/a.rs"),
            ],
        },
    ] {
        assert_eq!(role("src/main.rs", &table), FileRole::CrateRoot(None));
        assert_eq!(role("src/cli.rs", &table), module(None, &["cli"]));
        assert_eq!(
            role("src/bin/t.rs", &table),
            FileRole::CrateRoot(unit("bin", "t"))
        );
        assert_eq!(
            role("tests/a.rs", &table),
            FileRole::CrateRoot(unit("test", "a"))
        );
    }
}

#[test]
fn any_library_kind_is_the_primary_target() {
    for kind in ["lib", "rlib", "proc-macro", "cdylib", "staticlib", "dylib"] {
        let table = PackageTargets {
            source: TargetSource::Metadata,
            targets: vec![
                target("bin", "tool", "src/main.rs"),
                target(kind, "core", "src/lib.rs"),
            ],
        };
        assert_eq!(
            role("src/lib.rs", &table),
            FileRole::CrateRoot(None),
            "{kind}"
        );
        assert_eq!(
            role("src/main.rs", &table),
            FileRole::CrateRoot(unit("bin", "tool")),
            "{kind}"
        );
        assert_eq!(role("src/m.rs", &table), module(None, &["m"]), "{kind}");
    }
}

#[test]
fn sole_bin_elsewhere_is_primary_and_several_leave_none() {
    let sole = PackageTargets {
        source: TargetSource::Metadata,
        targets: vec![target("bin", "tool", "src/tool.rs")],
    };
    assert_eq!(role("src/tool.rs", &sole), FileRole::CrateRoot(None));
    assert_eq!(role("src/util.rs", &sole), module(None, &["util"]));

    // "Open": several bins, none at `src/main.rs`, no lib → no primary;
    // the rest of `src/` is unrooted, the roots keep their units.
    let several = PackageTargets {
        source: TargetSource::Metadata,
        targets: vec![
            target("bin", "one", "src/one.rs"),
            target("bin", "two", "src/two.rs"),
        ],
    };
    assert_eq!(
        role("src/one.rs", &several),
        FileRole::CrateRoot(unit("bin", "one"))
    );
    assert_eq!(role("src/util.rs", &several), FileRole::Unrooted);
}

#[test]
fn file_rooting_two_targets_is_a_shared_unit_named_by_the_file() {
    let table = PackageTargets {
        source: TargetSource::Metadata,
        targets: vec![
            target("lib", "core", "src/lib.rs"),
            target("example", "one", "examples/both.rs"),
            target("test", "two", "examples/both.rs"),
        ],
    };
    assert_eq!(
        role("examples/both.rs", &table),
        FileRole::CrateRoot(shared("examples/both.rs"))
    );
}

#[test]
fn directory_holding_two_roots_is_shared() {
    let table = PackageTargets {
        source: TargetSource::Metadata,
        targets: vec![
            target("bin", "a", "src/bin/pair/a.rs"),
            target("bin", "b", "src/bin/pair/b.rs"),
        ],
    };
    assert_eq!(
        role("src/bin/pair/a.rs", &table),
        FileRole::CrateRoot(unit("bin", "a"))
    );
    assert_eq!(
        role("src/bin/pair/util.rs", &table),
        module(shared("src/bin/pair"), &["util"])
    );
}

#[test]
fn loose_file_of_a_target_dir_is_unrooted() {
    // Rule 4: a file directly in a target dir that roots no target (here
    // `autotests = false`, so Cargo lists no `tests/loose.rs` target).
    let table = PackageTargets {
        source: TargetSource::Metadata,
        targets: vec![target("lib", "core", "src/lib.rs")],
    };
    assert_eq!(role("tests/loose.rs", &table), FileRole::Unrooted);
    assert_eq!(role("src/bin/stray.rs", &table), FileRole::Unrooted);
    assert_eq!(role("examples/mod.rs", &table), FileRole::Unrooted);
}

#[test]
fn table_roots_decide_not_the_path() {
    // A bin declared at a custom path inside `src/` is its own unit, not a
    // module of the library; without the table the same file is a module.
    let table = PackageTargets {
        source: TargetSource::Metadata,
        targets: vec![
            target("lib", "core", "src/lib.rs"),
            target("bin", "gen", "src/gen.rs"),
        ],
    };
    assert_eq!(
        role("src/gen.rs", &table),
        FileRole::CrateRoot(unit("bin", "gen"))
    );
    let layout = layout_targets("core", ["src/lib.rs", "src/gen.rs"].map(Path::new));
    assert_eq!(role("src/gen.rs", &layout), module(None, &["gen"]));
}

/// Reading 9 of the developer's report: `tests/a.rs` beside `tests/a/`,
/// `src/bin/t.rs` beside `src/bin/t/`. rustc treats a crate root as a
/// `mod.rs`-like file: `mod helpers;` in `tests/a.rs` looks for
/// `tests/helpers.rs` or `tests/helpers/mod.rs` (E0583 names exactly these),
/// never `tests/a/helpers.rs`. A file under `tests/a/` is reached only when
/// another crate includes `tests/a.rs` as a module (`mod a;`), so it is
/// shared, not a module of `test:a`: rule 3, `shared:tests/a`.
#[test]
fn directory_beside_a_single_file_target_is_shared() {
    let files = [
        "src/lib.rs",
        "tests/a.rs",
        "tests/a/helpers.rs",
        "src/bin/t.rs",
        "src/bin/t/x.rs",
        "examples/d.rs",
        "examples/d/part.rs",
    ];
    let layout = layout_targets("pkg", files.iter().map(Path::new));
    let metadata = PackageTargets {
        source: TargetSource::Metadata,
        targets: vec![
            target("lib", "pkg", "src/lib.rs"),
            target("bin", "t", "src/bin/t.rs"),
            target("example", "d", "examples/d.rs"),
            target("test", "a", "tests/a.rs"),
        ],
    };
    for table in [&layout, &metadata] {
        assert_eq!(
            role("tests/a.rs", table),
            FileRole::CrateRoot(unit("test", "a"))
        );
        assert_eq!(
            role("tests/a/helpers.rs", table),
            module(shared("tests/a"), &["helpers"])
        );
        assert_eq!(
            role("src/bin/t/x.rs", table),
            module(shared("src/bin/t"), &["x"])
        );
        assert_eq!(
            role("examples/d/part.rs", table),
            module(shared("examples/d"), &["part"])
        );
    }
}

#[test]
fn units_render_kind_colon_name_and_qpaths_keep_module_suffixes() {
    assert_eq!(unit("bin", "tool").unwrap().to_string(), "bin:tool");
    assert_eq!(
        unit("custom-build", "build-script-build")
            .unwrap()
            .to_string(),
        "custom-build:build-script-build"
    );
    assert_eq!(
        shared("tests/common").unwrap().to_string(),
        "shared:tests/common"
    );
    assert_eq!(shared("tests/common").unwrap().kind_str(), "shared");
    assert_eq!(unit("example", "d").unwrap().kind_str(), "example");
}

#[test]
fn unusable_paths_are_unrooted() {
    let table = metadata_table();
    for path in ["../src/lib.rs", "src/../../x.rs", "/abs/src/lib.rs", ""] {
        assert_eq!(role(path, &table), FileRole::Unrooted, "{path:?}");
    }
    // `./` prefixes are harmless.
    assert_eq!(role("./src/lib.rs", &table), FileRole::CrateRoot(None));
}

/// Rule 3 needs a target (spec "Units and `qpath`": "No targets (a virtual
/// root's empty Metadata table, a target-less layout): rules 2–4 give
/// `Unrooted`").
#[test]
fn empty_table_leaves_every_file_unrooted() {
    // Both sources: a virtual root's empty Metadata table and an empty
    // Layout table; rule 3 (`tests/common/mod.rs`, `examples/e/main.rs`'s
    // dir, `benches/data/mod.rs`) gives `Unrooted` under either.
    for source in [TargetSource::Metadata, TargetSource::Layout] {
        let empty = PackageTargets {
            source,
            targets: Vec::new(),
        };
        let mut failures = Vec::new();
        for path in FILES {
            let got = role(path, &empty);
            if got != FileRole::Unrooted {
                failures.push(format!("{path}: {got:?}"));
            }
            let indexed = TargetIndex::new(&empty).file_role(Path::new(path));
            if indexed != FileRole::Unrooted {
                failures.push(format!("{path} (TargetIndex): {indexed:?}"));
            }
        }
        assert!(
            failures.is_empty(),
            "an empty {:?} table leaves every file Unrooted:\n{}",
            empty.source,
            failures.join("\n")
        );
    }
}

/// The same for a target-less layout: files under target dirs, none a
/// Cargo root, give an empty `Layout` table and every file `Unrooted` —
/// `examples/x/y.rs` (AC-01) and a `tests/common/` dir included.
#[test]
fn target_less_layout_leaves_rule_three_dirs_unrooted() {
    let files = [
        "examples/x/y.rs",
        "tests/common/mod.rs",
        "tests/common/house.rs",
        "benches/data/mod.rs",
        "src/bin/m/cli.rs",
        "src/a.rs",
    ];
    let layout = layout_targets("pkg", files.iter().map(Path::new));
    assert_eq!(layout.source, TargetSource::Layout);
    assert!(layout.targets.is_empty(), "{:?}", layout.targets);
    for path in files {
        assert_eq!(role(path, &layout), FileRole::Unrooted, "{path}");
    }
}

/// Not vacuous: one target anywhere in the package is enough for rule 3,
/// so the two cases above are the empty table's doing, not a lost rule.
#[test]
fn one_target_brings_rule_three_back() {
    let lib_only = PackageTargets {
        source: TargetSource::Metadata,
        targets: vec![target("lib", "pkg", "src/lib.rs")],
    };
    let layout = layout_targets("pkg", ["src/lib.rs", "examples/x/y.rs"].map(Path::new));
    for table in [&lib_only, &layout] {
        assert_eq!(
            role("tests/common/mod.rs", table),
            module(shared("tests/common"), &[])
        );
        assert_eq!(
            role("examples/x/y.rs", table),
            module(shared("examples/x"), &["y"])
        );
        // Rule 4 is unchanged: a loose file of a target dir.
        assert_eq!(role("examples/y.rs", table), FileRole::Unrooted);
    }
}

/// `TargetIndex` (built once per package) answers exactly as `file_role`.
#[test]
fn target_index_answers_as_file_role() {
    let tables = [
        metadata_table(),
        layout_targets("unit-app", FILES.iter().map(Path::new)),
        PackageTargets {
            source: TargetSource::Metadata,
            targets: Vec::new(),
        },
    ];
    for table in &tables {
        let index = TargetIndex::new(table);
        for path in FILES {
            assert_eq!(
                index.file_role(Path::new(path)),
                role(path, table),
                "{path} under {:?}",
                table.source
            );
        }
    }
}
