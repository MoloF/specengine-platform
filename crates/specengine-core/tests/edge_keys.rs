//! docs/features/edge-table.md "Acceptance criteria", core's half.
//!
//! - ET-1 `et1_*`: keys are complete. For every file G of 1 000 seeded
//!   inputs (200 seeds of five steps of the store's AC-1 generator,
//!   `crates/specengine-store/tests/common/edge_tree.rs`, the tally
//!   showing every case) and of every fixture with a root
//!   `specengine.toml`, `resolve_files` over G and the holders (by
//!   `given_keys`) of G's reads equals the whole input's: edges with ends
//!   as `(path, ord)`, parent and reads; and `SpecGraph::new`'s edges are
//!   `resolve_files(None)`'s, concatenated in path order. M: the
//!   `aliases:` lookup unrecorded; fallback candidates unrecorded; the
//!   slug's path unrecorded; the `link_base` candidate unrecorded;
//!   `given_keys` without section IDs.
//! - ET-3 `et3_*`: parse-time capture. A scoped, sectioned `parent:` keeps
//!   its qualifiers; an escaped one resolves and `spec check` gives no
//!   `ref-dangling`; a plain one's JSON has no new key; an inline and a
//!   wiki look-alike mention resolve through an `aliases:` entry spelled
//!   the same, with bytes and without. M: the parser drops `scope`; `bare`
//!   from `written`.
//! - docs/features/edge-table.md "Data": `EdgeKey`'s text, `given_keys` and the
//!   reads of the spec's example, `Paths::resolution_json`.
//!
//! Everything runs in memory over the real parser. Non-Latin text is
//! written as escapes only (ADR-0024).

mod common;
#[path = "../../specengine-store/tests/common/edge_tree.rs"]
#[allow(dead_code)]
mod edge_tree;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use common::check::{Config, codes};
use common::{fixture, md_files, repository_root};
use edge_tree::{CASES, Rng, Tree, scheduled_case};
use specengine_core::check::{
    CheckFile, CheckInput, Edge, EdgeKey, Endpoint, NodeAt, Parent, SpecGraph, given_keys,
    resolve_files,
};
use specengine_core::{IdSchemeToml, Paths};
use specengine_model::{EdgeLabel, IdScheme, LinkOrigin, LinkTarget, ParsedFile};

/// A node as a path and a position, so two inputs compare.
type At = (String, usize);

#[derive(Debug, PartialEq, Eq)]
enum End {
    Nodes(Vec<At>),
    Dangling(String),
    Skipped,
    Unchecked,
}

/// An [`Edge`] with every index replaced by a path.
#[derive(Debug, PartialEq, Eq)]
struct PortableEdge {
    link_type: String,
    origin: LinkOrigin,
    file: String,
    holder: At,
    source: End,
    target: End,
    names_source: bool,
    written: String,
    line: usize,
    offset: usize,
    note: Option<String>,
    edge: Option<EdgeLabel>,
}

/// A [`Parent`] with every index replaced by a path.
#[derive(Debug, PartialEq, Eq)]
enum PortableParent {
    None,
    Node(Vec<At>),
    Dangling(String, String),
    Skipped(String),
    Cycle(Vec<At>),
}

fn at(paths: &[&str], node: NodeAt) -> At {
    (paths[node.file].to_owned(), node.ord)
}

fn end(paths: &[&str], endpoint: &Endpoint) -> End {
    match endpoint {
        Endpoint::Nodes(nodes) => End::Nodes(nodes.iter().map(|&n| at(paths, n)).collect()),
        Endpoint::Dangling(reason) => End::Dangling(reason.clone()),
        Endpoint::Skipped => End::Skipped,
        Endpoint::Unchecked => End::Unchecked,
    }
}

fn portable(paths: &[&str], edge: &Edge) -> PortableEdge {
    PortableEdge {
        link_type: edge.link_type.clone(),
        origin: edge.origin,
        file: paths[edge.file].to_owned(),
        holder: at(paths, edge.holder),
        source: end(paths, &edge.source),
        target: end(paths, &edge.target),
        names_source: edge.names_source,
        written: edge.written.clone(),
        line: edge.line,
        offset: edge.offset,
        note: edge.note.clone(),
        edge: edge.edge.clone(),
    }
}

fn portable_parent(paths: &[&str], parent: Option<&Parent>) -> Option<PortableParent> {
    Some(match parent? {
        Parent::None => PortableParent::None,
        Parent::Node { node, others } => PortableParent::Node(
            std::iter::once(node)
                .chain(others)
                .map(|&n| at(paths, n))
                .collect(),
        ),
        Parent::Dangling { written, reason } => {
            PortableParent::Dangling(written.clone(), reason.clone())
        }
        Parent::Skipped { written } => PortableParent::Skipped(written.clone()),
        Parent::Cycle { members } => {
            PortableParent::Cycle(members.iter().map(|&n| at(paths, n)).collect())
        }
    })
}

/// What one completeness check saw, so a run shows it was not vacuous.
#[derive(Default)]
struct Seen {
    files: usize,
    reads: usize,
    /// Holders brought in from other files.
    others: usize,
    resolved: usize,
}

/// ET-1 on one input: every file's resolution over itself and the holders
/// of its reads equals its resolution over the whole input.
fn assert_keys_complete(
    input: &CheckInput,
    scheme: &IdScheme,
    paths: &Paths,
    context: &str,
    seen: &mut Seen,
) {
    let whole = resolve_files(input, scheme, paths, None);
    assert_eq!(
        whole.files.len(),
        input.files.len(),
        "{context}: None resolves every file"
    );
    let mut holders: BTreeMap<EdgeKey, BTreeSet<&str>> = BTreeMap::new();
    for file in &input.files {
        let keys = given_keys(file);
        let mut sorted = keys.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(keys, sorted, "{context}: given_keys of `{}`", file.path);
        assert!(
            keys.contains(&EdgeKey::Path(file.path.clone())),
            "{context}: `{}` gives its path: {keys:?}",
            file.path
        );
        for key in keys {
            holders.entry(key).or_default().insert(file.path.as_str());
        }
    }
    let by_path: BTreeMap<&str, &CheckFile> = input
        .files
        .iter()
        .map(|file| (file.path.as_str(), file))
        .collect();
    for resolved in &whole.files {
        let path = whole.paths[resolved.file];
        let mut subset: BTreeSet<&str> = BTreeSet::from([path]);
        for key in &resolved.reads {
            if let Some(found) = holders.get(key) {
                subset.extend(found.iter().copied());
            }
        }
        seen.files += 1;
        seen.reads += resolved.reads.len();
        seen.others += subset.len() - 1;
        seen.resolved += resolved
            .edges
            .iter()
            .filter(|edge| matches!(edge.written_end(), Endpoint::Nodes(_)))
            .count();
        let sub = CheckInput {
            files: subset.iter().map(|path| by_path[*path].clone()).collect(),
            problems: Vec::new(),
        };
        let only = BTreeSet::from([path]);
        let part = resolve_files(&sub, scheme, paths, Some(&only));
        assert_eq!(part.files.len(), 1, "{context}: `only` names `{path}`");
        let alone = &part.files[0];
        assert_eq!(part.paths[alone.file], path, "{context}");
        let want: Vec<PortableEdge> = resolved
            .edges
            .iter()
            .map(|edge| portable(&whole.paths, edge))
            .collect();
        let got: Vec<PortableEdge> = alone
            .edges
            .iter()
            .map(|edge| portable(&part.paths, edge))
            .collect();
        assert!(
            want == got,
            "{context}: `{path}` over itself and the holders of its reads {subset:?} \
             resolves otherwise than over the whole input (reads {:?})\n\
             whole: {want:#?}\nsubset: {got:#?}",
            resolved.reads
        );
        assert_eq!(
            portable_parent(&whole.paths, resolved.parent.as_ref()),
            portable_parent(&part.paths, alone.parent.as_ref()),
            "{context}: `{path}`'s parent over {subset:?}"
        );
        assert_eq!(
            resolved.reads, alone.reads,
            "{context}: `{path}`'s reads over {subset:?}"
        );
    }
}

/// ET-1's second half: `SpecGraph::new` runs the same per-file routine.
fn assert_graph_is_resolve_files(
    input: &CheckInput,
    scheme: &IdScheme,
    paths: &Paths,
    context: &str,
) {
    let graph = SpecGraph::new(input, scheme, paths);
    let whole = resolve_files(input, scheme, paths, None);
    assert_eq!(
        graph.paths(),
        whole.paths.as_slice(),
        "{context}: path order"
    );
    let concatenated: Vec<Edge> = whole
        .files
        .iter()
        .flat_map(|file| file.edges.iter().cloned())
        .collect();
    assert!(
        graph.edges() == concatenated.as_slice(),
        "{context}: SpecGraph::new's edges are not resolve_files(None)'s in path order\n\
         graph: {:#?}\nresolve_files: {concatenated:#?}",
        graph.edges()
    );
    for file in &whole.files {
        let parent = graph.parent(NodeAt {
            file: file.file,
            ord: 0,
        });
        if matches!(parent, Some(Parent::Cycle { .. })) {
            continue;
        }
        assert_eq!(
            parent, file.parent,
            "{context}: `{}`'s parent before cycle breaking",
            whole.paths[file.file]
        );
    }
}

/// The input of a generated tree: its config read through the real TOML
/// readers, its walked files parsed (a mode-000 file unreadable).
fn tree_input(tree: &Tree) -> (IdScheme, Paths, CheckInput) {
    let text = tree.config.text();
    let scheme = IdScheme::from_toml(&text).unwrap_or_else(|error| panic!("{}", error.at("toml")));
    let paths = Paths::from_toml(&text).unwrap_or_else(|error| panic!("{}", error.at("toml")));
    let files = tree
        .contents()
        .into_iter()
        .filter(|(path, _)| paths.in_walk_scope(path))
        .map(|(path, bytes)| match bytes {
            Some(bytes) => CheckFile::parse(path, bytes, &scheme),
            None => CheckFile::unreadable(path, "Permission denied (os error 13)"),
        })
        .collect();
    (
        scheme,
        paths,
        CheckInput {
            files,
            problems: Vec::new(),
        },
    )
}

/// The seeds of ET-1; five steps each give 1 000 inputs.
const SEEDS: u64 = 200;
const STEPS: usize = 5;

#[test]
fn et1_keys_are_complete_over_seeded_inputs() {
    let mut tally: BTreeMap<&str, usize> = BTreeMap::new();
    let mut seen = Seen::default();
    let mut inputs = 0;
    for seed in 0..SEEDS {
        let mut rng = Rng::new(seed);
        let mut tree = Tree::generate(&mut rng);
        for step in 0..=STEPS {
            if step > 0 {
                let case = scheduled_case(seed, step, &mut rng);
                for performed in tree.step(&mut rng, case).cases {
                    *tally.entry(performed).or_default() += 1;
                }
            }
            let (scheme, paths, input) = tree_input(&tree);
            let context = format!("seed {seed} step {step}");
            assert_keys_complete(&input, &scheme, &paths, &context, &mut seen);
            assert_graph_is_resolve_files(&input, &scheme, &paths, &context);
            inputs += 1;
        }
    }
    eprintln!(
        "{inputs} inputs, {} files, {} reads, {} holders from other files, {} resolved edges; \
         cases {tally:?}",
        seen.files, seen.reads, seen.others, seen.resolved
    );
    assert!(inputs >= 200);
    let missing: Vec<&str> = CASES
        .iter()
        .copied()
        .filter(|case| !tally.contains_key(case))
        .collect();
    assert!(missing.is_empty(), "cases never performed: {missing:?}");
    assert!(seen.others > 0 && seen.resolved > 0, "a vacuous run");
}

/// The review's one-file case: in a `slug/ID` place, and for a
/// feature-scoped bare ID in its own feature document, the written text is
/// looked up as an `aliases:` entry only when no file has the ID itself as
/// one, so the Latin twin in another file is a read. The look-alike is a
/// Cyrillic `A` (U+0410), read as Latin `A`: `AC-52`, written otherwise.
#[test]
fn et1_a_one_file_lookup_reads_the_latin_twin_of_its_written_alias() {
    let config = Config::from_toml(KEYS_CONFIG);
    let look_alike = edge_tree::look_alike_ac();
    let stamina =
        format!("---\naliases: [\"{look_alike}\"]\n---\n# Stamina\n\n### Delay {{#AC-01}}\n");
    let own = format!("{stamina}\nOwn {look_alike} here.\n");
    let citing = format!("# A\n\nPer stamina/{look_alike} there.\n");
    let twin = format!("---\naliases: [{}]\n---\n# X\n", edge_tree::TWIN);
    let mut seen = Seen::default();
    for (name, files) in [
        (
            "slug/ID",
            vec![
                ("docs/features/stamina.md", stamina.as_str()),
                ("docs/a.md", citing.as_str()),
            ],
        ),
        (
            "own feature",
            vec![("docs/features/stamina.md", own.as_str())],
        ),
    ] {
        let alone = config.input(&files);
        let graph = SpecGraph::new(&alone, &config.scheme, &config.paths);
        assert!(
            graph
                .edges()
                .iter()
                .all(|edge| matches!(edge.written_end(), Endpoint::Nodes(_))),
            "{name}: resolves through the written text: {:#?}",
            graph.edges()
        );
        assert_keys_complete(&alone, &config.scheme, &config.paths, name, &mut seen);
        let mut with_twin = files.clone();
        with_twin.push(("docs/x.md", twin.as_str()));
        let input = config.input(&with_twin);
        assert_keys_complete(
            &input,
            &config.scheme,
            &config.paths,
            &format!("{name}, the twin in `docs/x.md`"),
            &mut seen,
        );
        assert_graph_is_resolve_files(&input, &config.scheme, &config.paths, name);
    }
}

/// Every fixture with a root `specengine.toml`, by name.
fn configured_fixtures() -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(repository_root().join("fixtures"))
        .expect("fixtures/ readable")
        .map(|entry| entry.expect("fixture entry").path())
        .filter(|path| path.join("specengine.toml").is_file())
        .map(|path| {
            path.file_name()
                .expect("name")
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}

#[test]
fn et1_keys_are_complete_over_every_fixture() {
    let names = configured_fixtures();
    assert!(names.len() >= 18, "configured fixtures: {names:?}");
    let mut seen = Seen::default();
    for name in &names {
        let corpus = fixture(name);
        let text = fs::read_to_string(corpus.join("specengine.toml")).expect("specengine.toml");
        let scheme =
            IdScheme::from_toml(&text).unwrap_or_else(|error| panic!("{name}: {}", error.at("")));
        let paths = Paths::from_toml(&text).unwrap_or_else(|error| panic!("{name}: {error}"));
        // The walk's files (`corpus-mini`'s config walks none of its own).
        let input = CheckInput {
            files: md_files(&corpus)
                .into_iter()
                .filter(|(path, _)| paths.in_walk_scope(path))
                .map(|(path, bytes)| CheckFile::parse(path, bytes, &scheme))
                .collect(),
            problems: Vec::new(),
        };
        assert_keys_complete(&input, &scheme, &paths, name, &mut seen);
        assert_graph_is_resolve_files(&input, &scheme, &paths, name);
    }
    assert!(seen.others > 0 && seen.resolved > 0, "a vacuous run");
}

// ------------------------------------------------------- "Data", Keys

const KEYS_CONFIG: &str = "[paths]\nroots = [\"docs\"]\nfeatures = \"docs/features\"\n\n\
[ids]\nR = { kind = \"requirement\", width = 2, aliases_from = [\"RQ\"] }\n\
AC = { kind = \"criterion\", width = 2, scope = \"feature\" }\n\
TERM = { kind = \"term\", shape = \"name\" }\n";

/// The example tree of docs/features/edge-table.md "Data".
const EXAMPLE_A: &str = "---\nid: R-01\nlinks:\n  depends_on: [R-02, R-09]\n---\n# A\n\n\
Needs R-02 and [the intro](b.md#intro).\n";
const EXAMPLE_B: &str = "---\nid: R-02\nparent: stamina/AC-01\naliases: [OLD-2]\n---\n# B\n\n\
## Intro\n";
const EXAMPLE_STAMINA: &str = "# Stamina\n\n### Delay {#AC-01}\n";

fn name(text: &str) -> EdgeKey {
    EdgeKey::Name(text.to_owned())
}

fn path(text: &str) -> EdgeKey {
    EdgeKey::Path(text.to_owned())
}

#[test]
fn an_edge_key_reads_back_from_its_text() {
    for key in [
        name("R-02"),
        name("\u{0422}ERM-x"),
        path("docs/sub/two words.md"),
        name(""),
    ] {
        assert_eq!(EdgeKey::parse(&key.text()), Some(key.clone()), "{key:?}");
    }
    assert_eq!(name("R-02").text(), "name:R-02");
    assert_eq!(path("docs/b.md").text(), "path:docs/b.md");
    assert_eq!(EdgeKey::parse("R-02"), None);
    assert_eq!(EdgeKey::parse("names:R-02"), None);
    // Ordered as the text orders.
    let mut keys = [path("a.md"), name("Z-1"), name("A-1")];
    keys.sort();
    let texts: Vec<String> = keys.iter().map(EdgeKey::text).collect();
    let mut sorted = texts.clone();
    sorted.sort();
    assert_eq!(texts, sorted);
}

#[test]
fn given_keys_are_what_the_resolver_indexes_a_file_by() {
    let config = Config::from_toml(KEYS_CONFIG);
    let input = config.input(&[
        ("docs/a.md", EXAMPLE_A),
        ("docs/b.md", EXAMPLE_B),
        ("docs/features/stamina.md", EXAMPLE_STAMINA),
        (
            "docs/c.md",
            "---\nid: R-03\naliases: [R-50, \"\u{0422}ERM-x\", RQ-07]\n---\n# C\n\n\
             ## One {#R-21}\n\n### Two {#TERM-glide}\n\n## Plain\n",
        ),
    ]);
    let keys: Vec<Vec<EdgeKey>> = input.files.iter().map(given_keys).collect();
    assert_eq!(keys[0], vec![name("R-01"), path("docs/a.md")]);
    assert_eq!(
        keys[1],
        vec![name("OLD-2"), name("R-02"), path("docs/b.md")]
    );
    assert_eq!(
        keys[2],
        vec![name("AC-01"), path("docs/features/stamina.md")]
    );
    assert_eq!(
        keys[3],
        vec![
            name("R-03"),
            name("R-21"),
            name("R-50"),
            name("RQ-07"),
            name("TERM-glide"),
            name("\u{0422}ERM-x"),
            path("docs/c.md"),
        ]
    );
    // A file with no parse gives its path alone.
    let unreadable = CheckFile::unreadable("docs/d.md", "Permission denied");
    assert_eq!(given_keys(&unreadable), vec![path("docs/d.md")]);
}

#[test]
fn the_example_records_the_documented_reads() {
    let config = Config::from_toml(KEYS_CONFIG);
    let input = config.input(&[
        ("docs/a.md", EXAMPLE_A),
        ("docs/b.md", EXAMPLE_B),
        ("docs/features/stamina.md", EXAMPLE_STAMINA),
    ]);
    let resolved = resolve_files(&input, &config.scheme, &config.paths, None);
    let reads: BTreeMap<&str, Vec<EdgeKey>> = resolved
        .files
        .iter()
        .map(|file| (resolved.paths[file.file], file.reads.clone()))
        .collect();
    assert_eq!(
        reads["docs/a.md"],
        vec![name("R-02"), name("R-09"), path("docs/b.md")]
    );
    assert_eq!(reads["docs/b.md"], vec![path("docs/features/stamina.md")]);
    assert_eq!(reads["docs/features/stamina.md"], Vec::<EdgeKey>::new());
    let b = resolved
        .files
        .iter()
        .find(|file| resolved.paths[file.file] == "docs/b.md")
        .expect("b.md");
    assert_eq!(
        portable_parent(&resolved.paths, b.parent.as_ref()),
        Some(PortableParent::Node(vec![(
            "docs/features/stamina.md".to_owned(),
            1
        )]))
    );
    // `only` picks its files and keeps every path's index.
    let only = BTreeSet::from(["docs/b.md"]);
    let one = resolve_files(&input, &config.scheme, &config.paths, Some(&only));
    assert_eq!(one.paths, resolved.paths);
    assert_eq!(one.files.len(), 1);
    assert_eq!(one.files[0], *b);
}

#[test]
fn resolution_json_holds_the_four_resolution_keys_in_order() {
    let defaults = Paths::default();
    let text = defaults.resolution_json();
    let value: serde_json::Value = serde_json::from_str(&text).expect("JSON");
    let object = value.as_object().expect("an object");
    assert_eq!(object.len(), 4, "{text}");
    let at = |key: &str| {
        text.find(&format!("\"{key}\":"))
            .unwrap_or_else(|| panic!("{key} in {text}"))
    };
    assert!(
        at("features") < at("link_base")
            && at("link_base") < at("roots")
            && at("roots") < at("exclude"),
        "key order: {text}"
    );
    assert_eq!(value["features"], defaults.features.as_str());
    assert_eq!(value["link_base"], serde_json::Value::Null);
    assert_eq!(value["roots"], serde_json::json!(defaults.roots));
    assert_eq!(value["exclude"], serde_json::json!(defaults.exclude));

    let written = Paths::from_toml(
        "[paths]\nroots = [\"docs\", \"notes\", \"docs\"]\nfeatures = \"docs/feat\"\n\
         link_base = \"docs\"\nexclude = [\"docs/skip/**\"]\n",
    )
    .expect("paths");
    let value: serde_json::Value = serde_json::from_str(&written.resolution_json()).expect("JSON");
    assert_eq!(
        value,
        serde_json::json!({
            "features": "docs/feat",
            "link_base": "docs",
            "roots": ["docs", "notes"],
            "exclude": ["docs/skip/**"],
        })
    );
    // The keys resolution does not read leave it as it is.
    let other = Paths::from_toml(
        "[paths]\nroots = [\"docs\", \"notes\", \"docs\"]\nfeatures = \"docs/feat\"\n\
         link_base = \"docs\"\nexclude = [\"docs/skip/**\"]\ntier0 = \"docs/zero.md\"\n\
         tier1_name = \"README.md\"\nindex = \"docs/index.md\"\n",
    )
    .expect("paths");
    assert_eq!(other.resolution_json(), written.resolution_json());
}

// ------------------------------------------------------------------- ET-3

/// The document of one parsed file.
fn parse(config: &Config, path: &str, text: &str) -> ParsedFile {
    specengine_core::parse(path, text.as_bytes(), &config.scheme)
}

#[test]
fn et3_a_scoped_sectioned_parent_keeps_its_qualifiers() {
    let config = Config::from_toml(KEYS_CONFIG);
    let parsed = parse(
        &config,
        "docs/x.md",
        "---\nid: R-05\nparent: stamina/AC-01#AC-02\n---\n# X\n",
    );
    let parent = parsed
        .document()
        .and_then(|document| document.parent.clone())
        .expect("a parent");
    assert_eq!(parent.id, "AC-01");
    assert_eq!(parent.scope.as_deref(), Some("stamina"));
    assert_eq!(parent.section.as_deref(), Some("AC-02"));
    assert_eq!(parent.project, None);
    assert_eq!(parent.alias_of, None);
    assert_eq!(parent.written_id, None);

    for (value, check) in [
        (
            "other:R-01",
            (|p: &specengine_model::ParentRef| p.project.as_deref() == Some("other"))
                as fn(&specengine_model::ParentRef) -> bool,
        ),
        ("RQ-03", |p| p.alias_of.as_deref() == Some("R")),
        ("\"stamina\\/AC-01\"", |p| {
            p.scope.as_deref() == Some("stamina") && p.id == "AC-01"
        }),
        ("\u{0422}ERM-glide", |p| {
            p.id == "TERM-glide" && p.written_id.as_deref() == Some("\u{0422}ERM-glide")
        }),
    ] {
        let parsed = parse(
            &config,
            "docs/x.md",
            &format!("---\nid: R-05\nparent: {value}\n---\n# X\n"),
        );
        let parent = parsed
            .document()
            .and_then(|document| document.parent.clone())
            .unwrap_or_else(|| panic!("`parent: {value}` parses"));
        assert!(check(&parent), "`parent: {value}`: {parent:?}");
    }
    // A section's containment parent carries none of them.
    let parsed = parse(
        &config,
        "docs/y.md",
        "---\nid: R-06\n---\n# Y\n\n## A {#R-21}\n\n### B {#R-22}\n",
    );
    for node in parsed.nodes.iter().skip(1) {
        if let Some(parent) = &node.parent {
            assert_eq!(
                (
                    &parent.alias_of,
                    &parent.project,
                    &parent.scope,
                    &parent.section,
                    &parent.written_id
                ),
                (&None, &None, &None, &None, &None),
                "{parent:?}"
            );
        }
    }
}

/// `input` as the index gives it: each parse read back from its JSON, no
/// bytes.
fn without_bytes(input: &CheckInput) -> CheckInput {
    CheckInput {
        files: input
            .files
            .iter()
            .map(|file| CheckFile {
                parsed: file.parsed.as_ref().map(|parsed| {
                    serde_json::from_str(&serde_json::to_string(parsed).expect("encodes"))
                        .expect("decodes")
                }),
                bytes: Vec::new(),
                ..file.clone()
            })
            .collect(),
        problems: Vec::new(),
    }
}

#[test]
fn et3_an_escaped_parent_resolves_and_the_check_finds_it() {
    let config = Config::from_toml(KEYS_CONFIG);
    let files = [
        (
            "docs/r.md",
            "---\nid: R-05\nparent: \"stamina\\/AC-01\"\n---\n# R\n",
        ),
        (
            "docs/features/stamina.md",
            "# Stamina\n\n### Delay {#AC-01}\n",
        ),
    ];
    let report = config.check(&files);
    assert!(
        !codes(&report).contains(&"ref-dangling"),
        "an escaped `parent:` keeps its qualifiers: {:?}",
        common::check::triples(&report)
    );
    let input = config.input(&files);
    for (what, input) in [
        ("bytes", input.clone()),
        ("no bytes", without_bytes(&input)),
    ] {
        let graph = SpecGraph::new(&input, &config.scheme, &config.paths);
        let r = graph.file_of("docs/r.md").expect("r.md");
        let stamina = graph.file_of("docs/features/stamina.md").expect("stamina");
        assert_eq!(
            graph.parent(NodeAt { file: r, ord: 0 }),
            Some(Parent::Node {
                node: NodeAt {
                    file: stamina,
                    ord: 1
                },
                others: Vec::new()
            }),
            "{what}"
        );
    }
}

#[test]
fn et3_a_plain_parent_encodes_as_before() {
    let config = Config::from_toml(KEYS_CONFIG);
    let parsed = parse(
        &config,
        "docs/b.md",
        "---\nid: R-02\nparent: R-01\n---\n# B\n",
    );
    let parent = parsed
        .document()
        .and_then(|document| document.parent.as_ref())
        .expect("a parent");
    assert_eq!(
        serde_json::to_string(parent).expect("encodes"),
        "{\"id\":\"R-01\",\"span\":[21,25]}"
    );
}

#[test]
fn et3_a_look_alike_mention_resolves_through_its_alias_with_and_without_bytes() {
    let config = Config::from_toml(KEYS_CONFIG);
    let input = config.input(&[
        (
            "docs/term.md",
            "---\naliases: [\"\u{0422}ERM-x\"]\n---\n# Term\n",
        ),
        (
            "docs/use.md",
            "# Use\n\nInline \u{0422}ERM-x here, wiki [[\u{0422}ERM-x]] too.\n",
        ),
    ]);
    // The parse keeps the ID as written where it differs from `id`.
    let used = input.files[1].parsed.as_ref().expect("parsed");
    let written: Vec<(String, Option<String>)> = used
        .links
        .iter()
        .filter_map(|link| match &link.dst {
            LinkTarget::Reference(reference) => {
                Some((reference.id.clone(), reference.written_id.clone()))
            }
            LinkTarget::Path(_) => None,
        })
        .collect();
    assert_eq!(
        written,
        vec![
            ("TERM-x".to_owned(), Some("\u{0422}ERM-x".to_owned())),
            ("TERM-x".to_owned(), Some("\u{0422}ERM-x".to_owned())),
        ]
    );
    for (what, input) in [
        ("bytes", input.clone()),
        ("no bytes", without_bytes(&input)),
    ] {
        let graph = SpecGraph::new(&input, &config.scheme, &config.paths);
        let term = graph.file_of("docs/term.md").expect("term.md");
        let ends: Vec<&Endpoint> = graph.edges().iter().map(Edge::written_end).collect();
        assert_eq!(
            ends,
            vec![
                &Endpoint::Nodes(vec![NodeAt { file: term, ord: 0 }]),
                &Endpoint::Nodes(vec![NodeAt { file: term, ord: 0 }]),
            ],
            "{what}: {:#?}",
            graph.edges()
        );
    }
    // A plain mention keeps no `written_id`.
    let plain = parse(&config, "docs/p.md", "# P\n\nSee R-01.\n");
    let ids: Vec<Option<String>> = plain
        .links
        .iter()
        .filter_map(|link| match &link.dst {
            LinkTarget::Reference(reference) => Some(reference.written_id.clone()),
            LinkTarget::Path(_) => None,
        })
        .collect();
    assert_eq!(ids, vec![None]);
}
