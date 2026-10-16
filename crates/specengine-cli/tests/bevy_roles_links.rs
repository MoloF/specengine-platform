//! docs/features/bevy-roles.md RD-5 (`docs/canon/code-roles.md` "Links"; L1, L3, SR2): a row with a
//! subject links to the worktree's symbols of its `subject_name` and of the
//! item kinds its word takes, own package first, at the best tier only, a
//! `cfg` twin lifted to the tier its undiscriminated qpath reaches; one
//! candidate `linked`, several `ambiguous` on each, none `unlinked`
//! (counted only). A closure sits on its enclosing fn, never looked up.
//! And RD-3's link half: links are computed per read, so renaming a system
//! unlinks its registration (and adding one re-links it) without the
//! registration's own file being re-derived.
//!
//! Each scenario is a probe project written at test time (one package,
//! `crates/probe`, a second where the package matters) or a scratch copy of
//! `fixtures/code-bevy`; what a link gives is read from the `roles` line
//! of `spec index` (the symbols holding each word, the subjects by state:
//! "spec index"), the per-row state itself being 10a-2's `spec symbols`.
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod bevy_roles_common;
mod code_index_common;
mod common;

use bevy_roles_common::{
    PROBE_SLUG, SLUG, app_fn, called, closure, copy, expected, index, line_of, manifest, probe,
    roles_line_of_run, rows_of, stored,
};
use code_index_common::{assert_equals_fresh, counts_of, db};
use common::{Scratch, read_text, replace, write};

/// The `roles` line of a probe project made of `files`.
fn line_for(name: &str, files: &[(&str, &str)]) -> String {
    let scratch = Scratch::new(name);
    let home = scratch.home("h");
    let root = probe(&scratch, "probe", files);
    roles_line_of_run(&index(&home, &root))
}

const WIRE_REGEN: &str = r#"pub mod heal;
pub mod stats;

use bevy::prelude::*;

pub fn wire(app: &mut App) {
    app.add_systems(Update, stats::regen);
}
"#;

const WIRE_BARE_REGEN: &str = r#"pub mod heal;
pub mod stats;

use bevy::prelude::*;

pub fn wire(app: &mut App) {
    app.add_systems(Update, regen);
}
"#;

/// RD-5 (L1). M: 6a-3's union lookup. `stats::regen` meets
/// `stats::regen`'s suffix and only its name elsewhere (`heal::regen`):
/// linked to the best tier alone. Bare, `regen` reaches both at the suffix
/// tier: ambiguous on each.
#[test]
fn qualified_subject_links_at_its_best_tier() {
    let others = [
        ("crates/probe/src/stats.rs", "pub fn regen() {}\n"),
        ("crates/probe/src/heal.rs", "pub fn regen() {}\n"),
    ];
    let mut files = vec![("crates/probe/src/lib.rs", WIRE_REGEN)];
    files.extend(others);
    let counts = [("plugin", 1), ("system", 1)];
    assert_eq!(
        line_for("br-link-qualified", &files),
        line_of(PROBE_SLUG, &counts, [1, 0, 0], 0)
    );
    let mut files = vec![("crates/probe/src/lib.rs", WIRE_BARE_REGEN)];
    files.extend(others);
    let counts = [("plugin", 1), ("system", 2)];
    assert_eq!(
        line_for("br-link-bare", &files),
        line_of(PROBE_SLUG, &counts, [0, 1, 0], 0)
    );
}

const FIELDS_WITH_TWINS: &str = r#"use bevy::prelude::*;

#[cfg(feature = "drip")]
pub fn run() {}

#[cfg(not(feature = "drip"))]
pub fn run() {}

pub fn wire(app: &mut App) {
    app.add_systems(Update, run);
}
"#;

/// RD-5 (L1). M: a plain best tier, twins not lifted. A bare `run`
/// registered beside its `cfg` twins, a plain `run` elsewhere: the twins
/// reach the suffix tier through `run #<discriminator>` as the plain one
/// does, so the row is ambiguous on all three, never linked to the one
/// elsewhere.
#[test]
fn bare_subject_beside_cfg_twins_is_ambiguous() {
    let files = [
        (
            "crates/probe/src/lib.rs",
            "pub mod clock;\npub mod fields;\n",
        ),
        ("crates/probe/src/clock.rs", "pub fn run() {}\n"),
        ("crates/probe/src/fields.rs", FIELDS_WITH_TWINS),
    ];
    let counts = [("plugin", 1), ("system", 3)];
    assert_eq!(
        line_for("br-link-twins", &files),
        line_of(PROBE_SLUG, &counts, [0, 1, 0], 0)
    );
}

/// RD-5 (L1, `code-identity.md`: reported, never guessed). M: the first
/// candidate taken. Two plain `tick`s in two modules, a bare `tick`
/// registered: ambiguous, the role counted on each.
#[test]
fn subject_naming_two_items_is_ambiguous_on_each() {
    let lib = r#"pub mod a;
pub mod b;

use bevy::prelude::*;

pub fn wire(app: &mut App) {
    app.add_systems(Update, tick);
}
"#;
    let files = [
        ("crates/probe/src/lib.rs", lib),
        ("crates/probe/src/a.rs", "pub fn tick() {}\n"),
        ("crates/probe/src/b.rs", "pub fn tick() {}\n"),
    ];
    let counts = [("plugin", 1), ("system", 2)];
    assert_eq!(
        line_for("br-link-two", &files),
        line_of(PROBE_SLUG, &counts, [0, 1, 0], 0)
    );
}

/// RD-5 (WA-11: per-word item kinds are the code crate's data). M: item
/// kinds ignored. A `system` subject `graze` beside a struct `graze`: only
/// the fn (a `function_item`) is a candidate, linked; the struct holds no
/// role.
#[test]
fn system_subject_skips_a_same_named_struct() {
    let lib = r#"pub mod herd;
pub mod weather;

use bevy::prelude::*;

pub fn wire(app: &mut App) {
    app.add_systems(Update, graze);
}
"#;
    let weather = "#[allow(non_camel_case_types)]\npub struct graze {\n    pub minutes: u32,\n}\n";
    let files = [
        ("crates/probe/src/lib.rs", lib),
        ("crates/probe/src/herd.rs", "pub fn graze() {}\n"),
        ("crates/probe/src/weather.rs", weather),
    ];
    let counts = [("plugin", 1), ("system", 1)];
    assert_eq!(
        line_for("br-link-kinds", &files),
        line_of(PROBE_SLUG, &counts, [1, 0, 0], 0)
    );
}

/// RD-5 (L3: `code_files.package` from the store). M: own package not
/// first. Two packages each with a `sense`: the registration in `other`
/// links its own; `only_here`, in `probe` alone, links across (else all
/// packages).
#[test]
fn own_package_link_wins() {
    let other = r#"use bevy::prelude::*;

pub fn sense() {}

pub fn wire(app: &mut App) {
    app.add_systems(Update, (sense, only_here));
}
"#;
    let other_manifest = manifest("other");
    let files = [
        (
            "crates/probe/src/lib.rs",
            "pub fn sense() {}\n\npub fn only_here() {}\n",
        ),
        ("crates/other/Cargo.toml", other_manifest.as_str()),
        ("crates/other/src/lib.rs", other),
    ];
    let counts = [("plugin", 1), ("system", 2)];
    assert_eq!(
        line_for("br-link-package", &files),
        line_of(PROBE_SLUG, &counts, [2, 0, 0], 0)
    );
}

/// Review 1 target, built in iteration 2 (B1: L1 amended, best tier
/// first, own package only breaking ties within it). M: own package
/// filtered before the tier. Package `probe` writes `systems::tick` for
/// package `other`'s module and has an unrelated `elsewhere::tick`: the
/// row links `other`'s `systems::tick` (suffix tier), which `other`'s own
/// registration holds already, so `system` counts one symbol; linked to
/// `probe`'s `elsewhere::tick` (name tier), it would count two.
#[test]
fn best_tier_wins_before_own_package() {
    let probe_lib = r#"pub mod elsewhere;

use bevy::prelude::*;

pub fn wire_probe(app: &mut App) {
    app.add_systems(Update, systems::tick);
}
"#;
    let other_lib = r#"pub mod systems;

use bevy::prelude::*;

pub fn wire_other(app: &mut App) {
    app.add_systems(Update, systems::tick);
}
"#;
    let other_manifest = manifest("other");
    let files = [
        ("crates/probe/src/lib.rs", probe_lib),
        ("crates/probe/src/elsewhere.rs", "pub fn tick() {}\n"),
        ("crates/other/Cargo.toml", other_manifest.as_str()),
        ("crates/other/src/lib.rs", other_lib),
        ("crates/other/src/systems.rs", "pub fn tick() {}\n"),
    ];
    let counts = [("plugin", 2), ("system", 1)];
    assert_eq!(
        line_for("br-link-tier-first", &files),
        line_of(PROBE_SLUG, &counts, [2, 0, 0], 0)
    );
}

/// RD-5 (SR2). M: a closure linked by name. A closure registration is
/// `certain` on the innermost enclosing `function_item` among the file's
/// items, never looked up: `build` beside another `build` holds `system`
/// alone; an observer closure inside a nested `fn` (no item) sits on the
/// outer fn. No row has a subject.
#[test]
fn closure_registration_sits_on_its_enclosing_fn() {
    let lib = r#"pub mod side;

use bevy::prelude::*;

pub fn build(app: &mut App) {
    app.add_systems(Update, || {});
}

pub fn outer(app: &mut App, speed: u8) {
    fn inner(app: &mut App, speed: u8) {
        app.add_observer(move |_: On<Ping>| {
            let _ = speed;
        });
    }
    inner(app, speed);
}
"#;
    let scratch = Scratch::new("br-link-closure");
    let home = scratch.home("h");
    let root = probe(
        &scratch,
        "probe",
        &[
            ("crates/probe/src/lib.rs", lib),
            ("crates/probe/src/side.rs", "pub fn build() {}\n"),
        ],
    );
    let run = index(&home, &root);
    assert_eq!(
        rows_of(&db(&home, PROBE_SLUG), "crates/probe/src/lib.rs"),
        stored(&[
            app_fn(1, 5),
            closure("system", Some("Update"), 1, 6),
            closure("observer", None, 2, 11),
        ])
    );
    let counts = [("observer", 1), ("plugin", 1), ("system", 1)];
    assert_eq!(
        roles_line_of_run(&run),
        line_of(PROBE_SLUG, &counts, [0, 0, 0], 0)
    );
}

/// RD-5 ("Links": `unlinked`, counted only). M: an `unlinked` row on a
/// symbol. `DefaultPlugins` and a dependency's system, registered in a fn
/// that is no plugin: two subjects unlinked, every word 0.
#[test]
fn unlinked_rows_are_counted_only() {
    let lib = r#"use bevy::prelude::*;

pub fn wire(app: &mut App, extra: bool) {
    app.add_plugins(DefaultPlugins);
    app.add_systems(Update, physics::step);
    let _ = extra;
}
"#;
    let scratch = Scratch::new("br-link-unlinked");
    let home = scratch.home("h");
    let root = probe(&scratch, "probe", &[("crates/probe/src/lib.rs", lib)]);
    let run = index(&home, &root);
    let update = Some("Update");
    assert_eq!(
        rows_of(&db(&home, PROBE_SLUG), "crates/probe/src/lib.rs"),
        stored(&[
            called(
                "plugin",
                "path",
                "code",
                None,
                "DefaultPlugins",
                "DefaultPlugins",
                4
            ),
            called("system", "path", "code", update, "physics::step", "step", 5),
        ])
    );
    assert_eq!(
        roles_line_of_run(&run),
        line_of(PROBE_SLUG, &[], [0, 0, 2], 0)
    );
}

/// RD-3, "Rules and edge cases" (L2: links per read, never stored). M: a
/// link stored or a cross-file pass. code-bevy: `herd::spawn_every`
/// renamed, only `herd.rs` re-derived, `lib.rs`'s rows kept byte for byte,
/// its factory registration now `unlinked` (system 6, linked 10, unlinked
/// 3); a `spawn_every` written in `clock.rs` re-links it; each step equals
/// a fresh index.
#[test]
fn renaming_a_system_unlinks_its_registration() {
    let want = expected();
    let scratch = Scratch::new("br-rename");
    let home = scratch.home("h");
    let root = copy(&scratch, "code-bevy");
    let run = index(&home, &root);
    assert_eq!(roles_line_of_run(&run), want["index_roles_line"]);
    let db = db(&home, SLUG);
    let lib = "crates/farm/src/lib.rs";
    let lib_rows = rows_of(&db, lib);
    let unlinked = [
        ("component", 3),
        ("entity_event", 1),
        ("event", 1),
        ("message", 2),
        ("observer", 1),
        ("plugin", 3),
        ("reflect", 2),
        ("resource", 1),
        ("system", 6),
    ];

    replace(
        &root,
        "crates/farm/src/herd.rs",
        "pub fn spawn_every(",
        "pub fn spawn_each(",
    );
    let run = index(&home, &root);
    let counts = counts_of(&run);
    assert_eq!(
        (counts["parsed"], counts["unchanged"]),
        (1, 8),
        "only herd.rs: {}",
        run.show()
    );
    assert_eq!(
        roles_line_of_run(&run),
        line_of(SLUG, &unlinked, [10, 1, 3], 1)
    );
    assert_eq!(rows_of(&db, lib), lib_rows, "lib.rs is not re-derived");
    assert_equals_fresh(&scratch, &home, &root, SLUG, "spawn_every renamed");

    let clock = read_text(&root, "crates/farm/src/clock.rs");
    write(
        &root,
        "crates/farm/src/clock.rs",
        format!(
            "{clock}\n/// Spawns on the clock.\npub fn spawn_every(ticks: u32) -> impl FnMut() {{\n    move || {{\n        let _ = ticks;\n    }}\n}}\n"
        ),
    );
    let run = index(&home, &root);
    assert_eq!(
        counts_of(&run)["parsed"],
        1,
        "only clock.rs: {}",
        run.show()
    );
    assert_eq!(
        roles_line_of_run(&run),
        want["index_roles_line"],
        "re-linked"
    );
    assert_eq!(rows_of(&db, lib), lib_rows, "lib.rs is not re-derived");
    assert_equals_fresh(&scratch, &home, &root, SLUG, "a spawn_every elsewhere");

    write(&root, "crates/farm/src/clock.rs", &clock);
    replace(
        &root,
        "crates/farm/src/herd.rs",
        "pub fn spawn_each(",
        "pub fn spawn_every(",
    );
    let run = index(&home, &root);
    assert_eq!(counts_of(&run)["parsed"], 2, "{}", run.show());
    assert_eq!(roles_line_of_run(&run), want["index_roles_line"]);
    assert_equals_fresh(&scratch, &home, &root, SLUG, "renamed back");
}
