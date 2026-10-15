//! docs/features/symbol-discriminator.md (6a-1), the discriminator: RD-1's
//! library tests by the names the spec gives them, TE-2's discriminator
//! cases and TE-3's names under LF and CRLF. Inputs are inline sources
//! named at once by `name_files`, as the CLI's walk names them (CLI
//! `code_walk.rs` `name_tree`): one package `crates/demo` laid out by
//! `layout_targets`, its library root always present, so `src/<m>.rs` is
//! module `<m>` of the primary target; any file outside `crates/demo` is
//! outside every package (`unrooted`).
//!
//! An item is rendered as `spec refs` prints it, without its marker:
//! `<line> <kind> <qpath>[ ambiguous <reason>]`. Every test also checks
//! the rendering's shape ([`check_names`]): a discriminated qpath ends in
//! ` #<discriminator>`, the value non-empty and whitespace-free outside its
//! string literals (the owner's S1: a string literal is kept verbatim); an
//! undiscriminated one holds no ` #`; a file module is never discriminated.
//! The owner's S3 is pinned too: an empty `cfg()` is not recorded,
//! `cfg_attr` and an inner `#![cfg]` are ignored, a group is keyed by the
//! qpath whatever the item kind.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use specengine_code::ron::{self, Anchor};
use specengine_code::{
    Ambiguity, Binding, FileAnalysis, ItemRecord, Name, NamedFile, Names, RustBinding, RustParser,
    analyze_file, bind_file, comment_anchors, layout_targets, name_files,
};

/// The one package of a tree: its directory relative to the root (its
/// label in every qpath) and its Cargo name.
const PACKAGE: &str = "crates/demo";
const PACKAGE_NAME: &str = "demo";

/// The library root every tree holds.
const LIB: &str = "crates/demo/src/lib.rs";

/// The file under test in a one-file tree: module `crates/demo::sys`.
const SYS: &str = "crates/demo/src/sys.rs";

/// A tree: `(path relative to the root, source)`, in walk order.
type Files<'a> = [(&'a str, &'a str)];

/// `[LIB, (SYS, source)]`: the file under test is index 1.
fn one_file(source: &str) -> Vec<(&str, &str)> {
    vec![(LIB, ""), (SYS, source)]
}

fn parser() -> RustParser {
    RustParser::new().expect("the pinned grammar loads")
}

/// The CLI's analysis of one file (the comment binding's).
fn bind(source: &str) -> RustBinding {
    bind_file(&mut parser(), source).expect("the parser gives a tree")
}

/// The eval's analysis of one file (every item hashed).
fn analyze(source: &str) -> FileAnalysis {
    analyze_file(&mut parser(), source).expect("the parser gives a tree")
}

/// `files` named over their analyses: [`PACKAGE`]'s table from its own
/// files, every other file outside every package.
fn name_analyses(files: &Files, analyses: &[FileAnalysis]) -> Names {
    let package = Path::new(PACKAGE);
    let paths: Vec<PathBuf> = files.iter().map(|(path, _)| PathBuf::from(path)).collect();
    let inner: Vec<PathBuf> = paths
        .iter()
        .filter_map(|path| path.strip_prefix(package).ok().map(Path::to_path_buf))
        .collect();
    let mut tables = BTreeMap::new();
    tables.insert(
        package.to_path_buf(),
        layout_targets(PACKAGE_NAME, inner.iter().map(PathBuf::as_path)),
    );
    let named: Vec<NamedFile<'_>> = paths
        .iter()
        .zip(analyses)
        .map(|(path, analysis)| NamedFile {
            path,
            package: path.starts_with(package).then_some(package),
            analysis,
        })
        .collect();
    let names = name_files(&named, &tables);
    check_names(&names);
    names
}

/// Every file bound as the CLI binds it, then named at once.
fn name(files: &Files) -> (Vec<RustBinding>, Names) {
    let bound: Vec<RustBinding> = files.iter().map(|(_, source)| bind(source)).collect();
    let analyses: Vec<FileAnalysis> = bound.iter().map(|b| b.analysis.clone()).collect();
    let names = name_analyses(files, &analyses);
    (bound, names)
}

fn render(item: &ItemRecord, name: &Name) -> String {
    let mut row = format!("{} {} {}", item.line, item.kind, name.qpath);
    if let Some(reason) = name.ambiguity {
        row.push_str(" ambiguous ");
        row.push_str(reason.as_str());
    }
    row
}

/// The rendered items of file `index` of `files`, in item order.
fn rows(files: &Files, index: usize) -> Vec<String> {
    let (bound, names) = name(files);
    bound[index]
        .analysis
        .items
        .iter()
        .zip(&names.items[index])
        .map(|(item, name)| render(item, name))
        .collect()
}

/// The discriminators of file `index`'s items, in item order.
fn discriminators(files: &Files, index: usize) -> Vec<Option<String>> {
    let (_, names) = name(files);
    names.items[index]
        .iter()
        .map(|name| name.discriminator.clone())
        .collect()
}

/// The `cfg` chains of a file's items, in item order.
fn chains(analysis: &FileAnalysis) -> Vec<Vec<String>> {
    analysis.items.iter().map(|item| item.cfg.clone()).collect()
}

/// `value` without its string literals' contents (`"…"`, escapes read).
fn outside_strings(value: &str) -> String {
    let mut out = String::new();
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c != '"' {
            out.push(c);
            continue;
        }
        out.push(c);
        while let Some(inner) = chars.next() {
            if inner == '\\' {
                chars.next();
            } else if inner == '"' {
                out.push(inner);
                break;
            }
        }
    }
    out
}

/// The rendering's shape ("Rendering", the owner's S1): ` #<value>` iff
/// discriminated, the value non-empty and whitespace-free outside its
/// string literals, the undiscriminated part free of ` #`; a file module
/// never discriminated ("Reach").
fn check_names(names: &Names) {
    for module in &names.modules {
        assert_eq!(module.discriminator, None, "a file module: {module:?}");
        assert!(!module.qpath.contains(" #"), "{module:?}");
    }
    for name in names.items.iter().flatten() {
        match &name.discriminator {
            Some(value) => {
                assert!(!value.is_empty(), "an empty discriminator: {name:?}");
                assert!(
                    !outside_strings(value).chars().any(char::is_whitespace),
                    "whitespace outside a string in the value: {name:?}"
                );
                let tail = format!(" #{value}");
                assert!(name.qpath.ends_with(&tail), "{name:?}");
                let base = &name.qpath[..name.qpath.len() - tail.len()];
                assert!(!base.contains(" #"), "{name:?}");
            }
            None => assert!(!name.qpath.contains(" #"), "{name:?}"),
        }
    }
}

/// Borrowed `(path, source)` pairs of an owned tree.
fn view(files: &[(String, String)]) -> Vec<(&str, &str)> {
    files
        .iter()
        .map(|(path, source)| (path.as_str(), source.as_str()))
        .collect()
}

fn owned(rows: &[&str]) -> Vec<String> {
    rows.iter().map(|row| (*row).to_owned()).collect()
}

// ------------------------------------------------------------------ RD-1

/// A file without twins: every qpath as slices 1-4c rendered it, a `cfg`
/// alone discriminating nothing.
const UNIQUE: &str = "\
//! A file without twins.
use std::fmt;

#[cfg(test)]
pub fn run() {}

pub struct Gauge;

impl Gauge {
    pub fn read(&self) -> u32 {
        0
    }
}

impl fmt::Display for Gauge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, \"g\")
    }
}

pub mod inner {
    //! Inline.
    pub fn deep() {}
}
";

/// RD-1 ("Rules and edge cases": no twins, byte-identical names): unique
/// qpaths, the file modules and an inline `mod` are rendered as before,
/// none discriminated, none ambiguous; the `cfg` of a unique item is
/// recorded (Q19) and changes nothing. M: a unique qpath discriminated.
#[test]
fn unique_qpath_is_unchanged() {
    let files = one_file(UNIQUE);
    assert_eq!(
        rows(&files, 1),
        owned(&[
            "5 function_item crates/demo::sys::run",
            "7 struct_item crates/demo::sys::Gauge",
            "9 impl_item crates/demo::sys::impl Gauge",
            "10 function_item crates/demo::sys::Gauge::read",
            "15 impl_item crates/demo::sys::impl <Gauge as fmt::Display>",
            "16 function_item crates/demo::sys::<Gauge as fmt::Display>::fmt",
            "21 mod_item crates/demo::sys::inner",
            "23 function_item crates/demo::sys::inner::deep",
        ])
    );
    assert!(discriminators(&files, 1).iter().all(Option::is_none));
    let (bound, names) = name(&files);
    let modules: Vec<&str> = names
        .modules
        .iter()
        .map(|module| module.qpath.as_str())
        .collect();
    assert_eq!(modules, ["crates/demo", "crates/demo::sys"]);
    assert!(
        names
            .modules
            .iter()
            .all(|module| module.ambiguity.is_none())
    );
    let run_chain: Vec<Vec<String>> = chains(&bound[1].analysis);
    assert_eq!(run_chain[0], ["test"], "`run`'s own cfg is recorded");
    assert!(
        run_chain[1..].iter().all(Vec::is_empty),
        "no other item has a cfg: {run_chain:?}"
    );
}

/// `cfg` twins: two `fn`s and two `impl` blocks of one qpath each, every
/// member with a `cfg` of its own value; the methods take their impl's.
const CFG_TWINS: &str = "\
#[cfg(test)]
pub fn regen() {}

#[cfg(not(test))]
pub fn regen() {}

pub struct Gauge;

#[cfg(feature = \"fast\")]
impl Gauge {
    pub fn read(&self) -> u32 {
        1
    }
}

#[cfg(not(feature = \"fast\"))]
impl Gauge {
    pub fn read(&self) -> u32 {
        2
    }
}
";

/// RD-1 rule 1, TE-2 (cfg twins: fn, impl; `cfg` on an enclosing impl):
/// every member of a group named by `cfg(<p>)`, whitespace removed; a
/// method by its impl's predicate. None is ambiguous. M: the ordinal
/// tried first.
#[test]
fn cfg_twins_are_named_by_cfg() {
    let files = one_file(CFG_TWINS);
    assert_eq!(
        rows(&files, 1),
        owned(&[
            "2 function_item crates/demo::sys::regen #cfg(test)",
            "5 function_item crates/demo::sys::regen #cfg(not(test))",
            "7 struct_item crates/demo::sys::Gauge",
            "10 impl_item crates/demo::sys::impl Gauge #cfg(feature=\"fast\")",
            "11 function_item crates/demo::sys::Gauge::read #cfg(feature=\"fast\")",
            "17 impl_item crates/demo::sys::impl Gauge #cfg(not(feature=\"fast\"))",
            "18 function_item crates/demo::sys::Gauge::read #cfg(not(feature=\"fast\"))",
        ])
    );
    assert_eq!(
        discriminators(&files, 1),
        [
            Some("cfg(test)".to_owned()),
            Some("cfg(not(test))".to_owned()),
            None,
            Some("cfg(feature=\"fast\")".to_owned()),
            Some("cfg(feature=\"fast\")".to_owned()),
            Some("cfg(not(feature=\"fast\"))".to_owned()),
            Some("cfg(not(feature=\"fast\"))".to_owned()),
        ]
    );
    let (bound, _) = name(&files);
    let expected: Vec<Vec<String>> = [
        vec!["test"],
        vec!["not(test)"],
        vec![],
        vec!["feature=\"fast\""],
        vec!["feature=\"fast\""],
        vec!["not(feature=\"fast\")"],
        vec!["not(feature=\"fast\")"],
    ]
    .iter()
    .map(|chain| chain.iter().map(|p| (*p).to_owned()).collect())
    .collect();
    assert_eq!(
        chains(&bound[1].analysis),
        expected,
        "ItemRecord::cfg (Q19)"
    );
}

/// Twins nested three deep: a `cfg` on the mod, on the impl in it and on
/// the method in that, twice over.
const CHAIN: &str = "\
#[cfg(feature = \"net\")]
mod wire {
    #[cfg(unix)]
    impl Gauge {
        #[cfg(test)]
        pub fn probe(&self) {}
    }
}

#[cfg(not(feature = \"net\"))]
mod wire {
    #[cfg(unix)]
    impl Gauge {
        #[cfg(test)]
        pub fn probe(&self) {}
    }
}
";

/// TE-2 (`cfg` on an enclosing impl), "Data" rule 1 and Q19: the chain is
/// the enclosing inline mods' and impl's predicates, outermost first, then
/// the item's own, joined by `,`. M: the item's own predicate put first;
/// the impl's left out.
#[test]
fn cfg_chain_is_outermost_first() {
    let files = one_file(CHAIN);
    assert_eq!(
        rows(&files, 1),
        owned(&[
            "2 mod_item crates/demo::sys::wire #cfg(feature=\"net\")",
            "4 impl_item crates/demo::sys::wire::impl Gauge #cfg(feature=\"net\",unix)",
            "6 function_item crates/demo::sys::wire::Gauge::probe #cfg(feature=\"net\",unix,test)",
            "11 mod_item crates/demo::sys::wire #cfg(not(feature=\"net\"))",
            "13 impl_item crates/demo::sys::wire::impl Gauge #cfg(not(feature=\"net\"),unix)",
            "15 function_item crates/demo::sys::wire::Gauge::probe #cfg(not(feature=\"net\"),unix,test)",
        ])
    );
    let (bound, _) = name(&files);
    assert_eq!(
        bound[1].analysis.items[2].cfg,
        ["feature=\"net\"", "unix", "test"]
    );
}

/// Reading L2 (the spec says "the item's own `#[cfg(..)]`"): two `cfg`
/// attributes on one item give both predicates, in source order, after the
/// enclosing chain. M: only the first (or the last) attribute read.
#[test]
fn two_cfg_attributes_join_in_source_order() {
    let source = "\
#[cfg(unix)]
#[cfg(test)]
pub fn both() {}

#[cfg(windows)]
pub fn both() {}
";
    assert_eq!(
        rows(&one_file(source), 1),
        owned(&[
            "3 function_item crates/demo::sys::both #cfg(unix,test)",
            "6 function_item crates/demo::sys::both #cfg(windows)",
        ])
    );
}

/// Twin inline mods, each documented by a `//!` inside, one holding an
/// item the other lacks.
const MOD_TWINS: &str = "\
#[cfg(unix)]
mod os {
    //! The unix side.
    pub fn open() {}

    pub fn only_unix() {}
}

#[cfg(windows)]
mod os {
    //! The windows side.
    pub fn open() {}
}
";

/// RD-1 ("Reach"), TE-2 (inline mods with `//!` inside): twin mods are
/// named by their `cfg`; the items inside them form their own group,
/// named through the mods' `cfg`; an item only one of them holds stays
/// undiscriminated; a `//!` inside binds the mod's module, which takes its
/// item's discriminated qpath. M: an enclosing mod's `cfg` ignored.
#[test]
fn items_in_cfg_twin_mods_take_the_mods_cfg() {
    let files = one_file(MOD_TWINS);
    assert_eq!(
        rows(&files, 1),
        owned(&[
            "2 mod_item crates/demo::sys::os #cfg(unix)",
            "4 function_item crates/demo::sys::os::open #cfg(unix)",
            "6 function_item crates/demo::sys::os::only_unix",
            "10 mod_item crates/demo::sys::os #cfg(windows)",
            "12 function_item crates/demo::sys::os::open #cfg(windows)",
        ])
    );
    let (bound, names) = name(&files);
    assert_eq!(bound[1].analysis.items[1].cfg, ["unix"]);
    for (doc, module) in [
        ("The unix side.", "crates/demo::sys::os #cfg(unix)"),
        ("The windows side.", "crates/demo::sys::os #cfg(windows)"),
    ] {
        let comment = bound[1]
            .comments
            .iter()
            .find(|comment| MOD_TWINS[comment.range.clone()].contains(doc))
            .unwrap_or_else(|| panic!("the comment {doc:?}"));
        let Binding::Module(Some(index)) = comment.binding else {
            panic!("{doc:?} binds {:?}, not an inline mod", comment.binding);
        };
        assert_eq!(names.items[1][index].qpath, module, "{doc:?}");
        assert_eq!(names.items[1][index].ambiguity, None, "{doc:?}");
    }
}

/// RD-1 rule 2, TE-2 (ordinal twins with one member lacking `cfg`): rule 1
/// holds only when every member has a non-empty value and all are
/// distinct; a member without `cfg`, or two equal `cfg`s, give the whole
/// group ordinals; code-a's adjacent `impl Tally` blocks (no `cfg`) are
/// `#1` and `#2`. M: rule 1 taken with a member lacking `cfg`.
#[test]
fn twins_with_a_cfg_less_member_take_ordinals() {
    let source = "\
#[cfg(test)]
pub fn regen() {}

pub fn regen() {}

#[cfg(test)]
pub fn drain() {}

#[cfg(test)]
pub fn drain() {}

pub struct Tally;

impl Tally {
    pub fn step(&mut self) {}
}

impl Tally {
    pub fn reset(&mut self) {}
}
";
    assert_eq!(
        rows(&one_file(source), 1),
        owned(&[
            "2 function_item crates/demo::sys::regen #1",
            "4 function_item crates/demo::sys::regen #2",
            "7 function_item crates/demo::sys::drain #1",
            "10 function_item crates/demo::sys::drain #2",
            "12 struct_item crates/demo::sys::Tally",
            "14 impl_item crates/demo::sys::impl Tally #1",
            "15 function_item crates/demo::sys::Tally::step",
            "18 impl_item crates/demo::sys::impl Tally #2",
            "19 function_item crates/demo::sys::Tally::reset",
        ])
    );
}

/// Three `impl Gauge` blocks whose contents sort otherwise than their
/// places (zeta, alpha, mid), another item between them.
const THREE: &str = "\
pub struct Gauge;

impl Gauge {
    pub fn zeta(&self) {}
}

pub fn between() {}

impl Gauge {
    pub fn alpha(&self) {}
}

impl Gauge {
    pub fn mid(&self) {}
}
";

/// RD-1 rule 2: ordinals are 1-based, in source order, whatever lies
/// between the members or inside them; moving a block renames by place.
/// M: ordinals off source order.
#[test]
fn ordinals_follow_source_order() {
    assert_eq!(
        rows(&one_file(THREE), 1),
        owned(&[
            "1 struct_item crates/demo::sys::Gauge",
            "3 impl_item crates/demo::sys::impl Gauge #1",
            "4 function_item crates/demo::sys::Gauge::zeta",
            "7 function_item crates/demo::sys::between",
            "9 impl_item crates/demo::sys::impl Gauge #2",
            "10 function_item crates/demo::sys::Gauge::alpha",
            "13 impl_item crates/demo::sys::impl Gauge #3",
            "14 function_item crates/demo::sys::Gauge::mid",
        ])
    );
    let moved = "\
impl Gauge {
    pub fn mid(&self) {}
}

pub struct Gauge;

impl Gauge {
    pub fn zeta(&self) {}
}

impl Gauge {
    pub fn alpha(&self) {}
}
";
    assert_eq!(
        rows(&one_file(moved), 1),
        owned(&[
            "1 impl_item crates/demo::sys::impl Gauge #1",
            "2 function_item crates/demo::sys::Gauge::mid",
            "5 struct_item crates/demo::sys::Gauge",
            "7 impl_item crates/demo::sys::impl Gauge #2",
            "8 function_item crates/demo::sys::Gauge::zeta",
            "11 impl_item crates/demo::sys::impl Gauge #3",
            "12 function_item crates/demo::sys::Gauge::alpha",
        ])
    );
}

/// Twins whose `cfg` holds a line and a block comment across lines.
const COMMENTED: &str = "\
#[cfg(any(
    test, // the suite
    feature = \"fast\" /* hot */
))]
pub fn regen() {}

#[cfg(not(any(test, feature = \"fast\")))]
pub fn regen() {}
";

/// RD-1 (Q20, flagged: these bytes become lock keys): comments inside
/// `cfg(..)` are dropped with the whitespace, so editing one keeps the
/// name. M: a comment inside `cfg(..)` kept.
#[test]
fn cfg_comment_edit_keeps_the_name() {
    let edited = COMMENTED
        .replace("// the suite", "// the whole suite, renamed")
        .replace("/* hot */", "/* a hot path, long */");
    let want = owned(&[
        "5 function_item crates/demo::sys::regen #cfg(any(test,feature=\"fast\"))",
        "8 function_item crates/demo::sys::regen #cfg(not(any(test,feature=\"fast\")))",
    ]);
    assert_eq!(rows(&one_file(COMMENTED), 1), want);
    assert_eq!(rows(&one_file(&edited), 1), want);
    let (bound, _) = name(&one_file(COMMENTED));
    assert_eq!(
        bound[1].analysis.items[0].cfg,
        ["any(test,feature=\"fast\")"]
    );
}

/// TE-2 (a `cfg` holding a comment and a string with spaces), the owner's
/// S1 (overriding reading L1): the tokens join without whitespace and the
/// comment goes, but a string literal is kept verbatim, its spaces
/// included; a `//` inside a string is no comment. M: string text taken
/// for a comment (the value cut at `//`); a string's spaces removed.
#[test]
fn cfg_with_a_comment_and_a_string_with_spaces() {
    let source = "\
#[cfg(feature = /* the flag */ \"fast path\")]
pub fn regen() {}

#[cfg(feature = \"x // not a comment\")]
pub fn regen() {}
";
    assert_eq!(
        rows(&one_file(source), 1),
        owned(&[
            "2 function_item crates/demo::sys::regen #cfg(feature=\"fast path\")",
            "5 function_item crates/demo::sys::regen #cfg(feature=\"x // not a comment\")",
        ])
    );
}

/// The owner's S1: a CR right before an LF inside a `cfg` string literal is
/// dropped, so an LF and a CRLF checkout give one name (as rustc reads the
/// literal); the LF stays. M: the CR kept in the string.
#[test]
fn cfg_string_spanning_lines_is_equal_under_crlf() {
    let source = "\
#[cfg(feature = \"two
lines\")]
pub fn regen() {}

#[cfg(not(test))]
pub fn regen() {}
";
    let want = owned(&[
        "3 function_item crates/demo::sys::regen #cfg(feature=\"two\nlines\")",
        "6 function_item crates/demo::sys::regen #cfg(not(test))",
    ]);
    assert_eq!(rows(&one_file(source), 1), want);
    assert_eq!(rows(&one_file(&source.replace('\n', "\r\n")), 1), want);
}

/// The owner's S3: an empty `cfg()` is not recorded, so its twin group
/// falls to ordinals; `cfg_attr` and an inner `#![cfg]` are no `cfg`. M: an
/// empty predicate recorded (`cfg()`); `cfg_attr`'s predicate read; an
/// inner attribute read as the mod's `cfg`.
#[test]
fn empty_cfg_cfg_attr_and_inner_cfg_are_not_recorded() {
    let source = "\
#[cfg()]
pub fn f() {}

#[cfg(test)]
pub fn f() {}

#[cfg_attr(test, derive(Debug))]
pub struct S;

#[cfg_attr(not(test), derive(Clone))]
pub struct S;

mod m {
    #![cfg(test)]
    pub fn g() {}
}

mod m {
    pub fn g() {}
}
";
    let files = one_file(source);
    assert_eq!(
        rows(&files, 1),
        owned(&[
            "2 function_item crates/demo::sys::f #1",
            "5 function_item crates/demo::sys::f #2",
            "8 struct_item crates/demo::sys::S #1",
            "11 struct_item crates/demo::sys::S #2",
            "13 mod_item crates/demo::sys::m #1",
            "15 function_item crates/demo::sys::m::g #1",
            "18 mod_item crates/demo::sys::m #2",
            "19 function_item crates/demo::sys::m::g #2",
        ])
    );
    let (bound, _) = name(&files);
    let chains = chains(&bound[1].analysis);
    assert_eq!(chains[1], ["test"]);
    for (index, chain) in chains.iter().enumerate().filter(|(index, _)| *index != 1) {
        assert!(chain.is_empty(), "item {index}: {chain:?}");
    }
}

/// The owner's S3: a group is keyed by the rendered qpath whatever the
/// item kind, so a `struct Foo` and an `fn Foo` of one file are twins,
/// named apart and no longer `duplicate`. M: groups keyed by kind too.
#[test]
fn a_group_is_keyed_by_qpath_whatever_the_kind() {
    let source = "\
pub struct Foo;

#[allow(non_snake_case)]
pub fn Foo() {}
";
    assert_eq!(
        rows(&one_file(source), 1),
        owned(&[
            "1 struct_item crates/demo::sys::Foo #1",
            "4 function_item crates/demo::sys::Foo #2",
        ])
    );
}

/// RD-1 ("Ambiguity"), TE-2 (a cross-file pair): `src/x.rs` beside
/// `src/x/mod.rs` render one module; an item of one whose undiscriminated
/// qpath an item of the other renders is `duplicate`, discriminated
/// members too, and only in-file twins are discriminated; in-file twins
/// with no counterpart elsewhere (`y.rs`) stay unambiguous. M: cross-file
/// twins discriminated; a discriminated member spared `duplicate`.
#[test]
fn cross_file_twins_stay_duplicate() {
    let files = [
        (LIB, ""),
        (
            "crates/demo/src/x.rs",
            "#[cfg(unix)]\npub fn f() {}\n\n#[cfg(windows)]\npub fn f() {}\n\npub fn g() {}\n",
        ),
        (
            "crates/demo/src/x/mod.rs",
            "pub fn f() {}\n\npub fn g() {}\n\npub fn h() {}\n",
        ),
        (
            "crates/demo/src/y.rs",
            "#[cfg(unix)]\npub fn k() {}\n\n#[cfg(windows)]\npub fn k() {}\n",
        ),
    ];
    assert_eq!(
        rows(&files, 1),
        owned(&[
            "2 function_item crates/demo::x::f #cfg(unix) ambiguous duplicate",
            "5 function_item crates/demo::x::f #cfg(windows) ambiguous duplicate",
            "7 function_item crates/demo::x::g ambiguous duplicate",
        ])
    );
    assert_eq!(
        rows(&files, 2),
        owned(&[
            "1 function_item crates/demo::x::f ambiguous duplicate",
            "3 function_item crates/demo::x::g ambiguous duplicate",
            "5 function_item crates/demo::x::h",
        ])
    );
    assert_eq!(
        rows(&files, 3),
        owned(&[
            "2 function_item crates/demo::y::k #cfg(unix)",
            "5 function_item crates/demo::y::k #cfg(windows)",
        ])
    );
    let (_, names) = name(&files);
    assert_eq!(names.modules[1].qpath, names.modules[2].qpath);
    assert_eq!(
        names.modules[1].ambiguity, None,
        "a module has no twin by path"
    );
}

/// TE-2 (twins in an unrooted file; "Ambiguity": order unchanged): a twin
/// in a file outside every package is discriminated and stays `unrooted`;
/// unrooted files sharing a name stay `unrooted`, never discriminated
/// across files. M: `unrooted` twins left undiscriminated; `duplicate`
/// ranked above `unrooted`.
#[test]
fn twins_in_an_unrooted_file_stay_unrooted() {
    let files = [
        (LIB, ""),
        (
            "tools/loose.rs",
            "#[cfg(test)]\npub fn seed() {}\n\n#[cfg(not(test))]\npub fn seed() {}\n\npub fn grow() {}\n",
        ),
        ("tools/other.rs", "pub fn grow() {}\n"),
    ];
    assert_eq!(
        rows(&files, 1),
        owned(&[
            "2 function_item .::seed #cfg(test) ambiguous unrooted",
            "5 function_item .::seed #cfg(not(test)) ambiguous unrooted",
            "7 function_item .::grow ambiguous unrooted",
        ])
    );
    assert_eq!(
        rows(&files, 2),
        owned(&["1 function_item .::grow ambiguous unrooted"])
    );
    let (_, names) = name(&files);
    assert_eq!(names.modules[1].qpath, ".");
    assert_eq!(names.modules[1].ambiguity, Some(Ambiguity::Unrooted));
}

/// RD-1 ("Reach": never a RON path): two `step` fields of one struct, both
/// marked, keep the path `root.step`, ambiguous, through `ron::analyze`
/// and `comment_anchors` alike, under LF and CRLF. M: a RON path
/// discriminated.
#[test]
fn ron_paths_are_never_discriminated() {
    let source = "\
Limits(
    cap: 10,
    // @configures LAW-A@1
    step: 1,
    // @configures LAW-B@1
    step: 2,
)
";
    for text in [source.to_owned(), source.replace('\n', "\r\n")] {
        let analysis = ron::analyze(&text);
        assert_eq!(analysis.markers.len(), 2, "{text:?}");
        for marker in &analysis.markers {
            assert!(
                matches!(marker.anchor, Anchor::Ambiguous { .. }),
                "{:?}",
                marker.anchor
            );
            assert_eq!(marker.anchor.as_str(), "root.step");
        }
        let anchors = comment_anchors(&text);
        let paths: Vec<&str> = anchors
            .iter()
            .map(|anchor| anchor.anchor.as_str())
            .collect();
        assert_eq!(paths, ["root.step", "root.step"], "{text:?}");
    }
}

// ----------------------------------------------- Q19, Q21 and TE-3 support

/// Q19: both analyses record the same `cfg` chains, so a caller of
/// `analyze_tree` names twins apart as the CLI's binding does. M: the chain
/// recorded by one analysis only.
#[test]
fn both_analyses_record_the_cfg_chain() {
    for source in [CFG_TWINS, CHAIN, MOD_TWINS, COMMENTED] {
        let files = one_file(source);
        let bound: Vec<FileAnalysis> = files.iter().map(|(_, text)| bind(text).analysis).collect();
        let hashed: Vec<FileAnalysis> = files.iter().map(|(_, text)| analyze(text)).collect();
        assert_eq!(chains(&bound[1]), chains(&hashed[1]), "{source}");
        assert!(
            chains(&bound[1]).iter().any(|chain| !chain.is_empty()),
            "{source}"
        );
        assert_eq!(
            name_analyses(&files, &bound),
            name_analyses(&files, &hashed),
            "{source}"
        );
    }
}

/// Q21 ("For 6a-2": the discriminator needs no other file): a file's
/// discriminators are the same named alone (with its library root) or in
/// a tree holding its cross-file twins; only the ambiguity differs. M:
/// grouping across files.
#[test]
fn discriminators_need_no_other_file() {
    let x = (
        "crates/demo/src/x.rs",
        "#[cfg(unix)]\npub fn f() {}\n\n#[cfg(windows)]\npub fn f() {}\n\nimpl G {}\n\nimpl G {}\n",
    );
    let twin = ("crates/demo/src/x/mod.rs", "pub fn f() {}\n\nimpl G {}\n");
    let alone = [(LIB, ""), x];
    let together = [(LIB, ""), x, twin];
    assert_eq!(discriminators(&alone, 1), discriminators(&together, 1));
    assert_eq!(
        discriminators(&alone, 1),
        [
            Some("cfg(unix)".to_owned()),
            Some("cfg(windows)".to_owned()),
            Some("1".to_owned()),
            Some("2".to_owned()),
        ]
    );
    assert_eq!(discriminators(&together, 2), [None, None]);
}

/// TE-3: one state gives byte-identical names under LF and CRLF (a CR in a
/// multi-line `cfg` is whitespace), and twice over. M: a CR kept in a
/// `cfg` value.
#[test]
fn names_are_equal_under_lf_and_crlf() {
    let multi_line = "\
#[cfg(all(
    test,
    unix
))]
pub fn regen() {}

#[cfg(not(all(test, unix)))]
pub fn regen() {}
";
    let sources = [CFG_TWINS, CHAIN, MOD_TWINS, COMMENTED, THREE, multi_line];
    let lf: Vec<(String, String)> = sources
        .iter()
        .enumerate()
        .map(|(n, source)| (format!("crates/demo/src/m{n}.rs"), (*source).to_owned()))
        .chain([(LIB.to_owned(), String::new())])
        .collect();
    let crlf: Vec<(String, String)> = lf
        .iter()
        .map(|(path, source)| (path.clone(), source.replace('\n', "\r\n")))
        .collect();
    let (lf_bound, lf_names) = name(&view(&lf));
    let (crlf_bound, crlf_names) = name(&view(&crlf));
    assert_eq!(lf_names, crlf_names);
    for (a, b) in lf_bound.iter().zip(&crlf_bound) {
        assert_eq!(chains(&a.analysis), chains(&b.analysis));
    }
    assert!(
        lf_names.items[5]
            .iter()
            .any(|name| name.qpath == "crates/demo::m5::regen #cfg(all(test,unix))"),
        "{:?}",
        lf_names.items[5]
    );
    let (_, again) = name(&view(&lf));
    assert_eq!(again, lf_names, "a second run is identical");
}
