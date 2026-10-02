//! AC-05 of docs/features/spec-check.md (the core half): the check tables of
//! `specengine.toml`. The Data example loads; an unknown key or class, a
//! wrong type, a cap below 1 and an unknown mode fail as
//! `specengine.toml:<line>: message`; `[paths]` gains `tier0`, `tier1_name`
//! and `index`. The re-parse half (only new tables edited → nothing
//! re-parsed) is in the store's `tests/check_config.rs`.

mod common;

use specengine_core::check::config::{
    DEFAULT_DECISION_BYTES, DEFAULT_INDEX_BYTES, DEFAULT_TIER0_BYTES, DEFAULT_TIER1_BYTES,
};
use specengine_core::check::{CheckConfig, ClassContract, DocClass, Mode};
use specengine_core::{IdSchemeToml, Paths};
use specengine_model::IdScheme;

/// The Data example of the spec, verbatim but for the elided keys.
const DATA_EXAMPLE: &str = r#"[paths]                       # + existing keys
records    = "docs/decisions" # file named after its id
tier0      = "CLAUDE.md"      # the only canon tier 0
tier1_name = "README.md"      # canon tier 1 only so named
index      = "docs/index.md"  # capped by index_bytes
exclude    = ["**/_*.md"]     # + SKIP_DIRS, AC-19

[ids]
ADR = { kind = "decision", width = 4 }

[budgets]                     # bytes; absent → §4, canon_bytes → none
tier0_bytes    = 16384
tier1_bytes    = 10240
index_bytes    = 10240
decision_bytes = 1536
canon_bytes    = 12288

[classes]                     # replaces that default; here all four, closed,
                              # as docs/README.md "Front-matter contract"
decision = { required = ["class", "id", "title", "status", "date", "scope"], optional = ["canon", "supersedes", "ref"], closed = true }

[check]
mode = "enforce"              # observe | enforce (default)
"#;

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn the_data_example_loads() {
    let config = CheckConfig::from_toml(DATA_EXAMPLE)
        .unwrap_or_else(|e| panic!("{}", e.at("specengine.toml")));
    assert_eq!(config.budgets.tier0_bytes, 16384);
    assert_eq!(config.budgets.tier1_bytes, 10240);
    assert_eq!(config.budgets.index_bytes, 10240);
    assert_eq!(config.budgets.decision_bytes, 1536);
    assert_eq!(config.budgets.canon_bytes, Some(12288));
    assert_eq!(config.budgets.bundle_node, None);
    assert_eq!(config.mode, Mode::Enforce);
    let decision = config.classes.get(DocClass::Decision);
    assert_eq!(
        decision,
        &ClassContract {
            required: strings(&["class", "id", "title", "status", "date", "scope"]),
            optional: strings(&["canon", "supersedes", "ref"]),
            closed: true,
        }
    );
    assert!(decision.allows("canon") && decision.allows("class"));
    assert!(!decision.allows("owner"));
    // A class not written keeps its default contract.
    assert_eq!(
        config.classes.get(DocClass::Canon),
        &ClassContract::default_for(DocClass::Canon)
    );

    let paths =
        Paths::from_toml(DATA_EXAMPLE).unwrap_or_else(|e| panic!("{}", e.at("specengine.toml")));
    assert_eq!(paths.records, "docs/decisions");
    assert_eq!(paths.tier0.as_deref(), Some("CLAUDE.md"));
    assert_eq!(paths.tier1_name.as_deref(), Some("README.md"));
    assert_eq!(paths.index.as_deref(), Some("docs/index.md"));
    assert_eq!(paths.exclude, ["**/_*.md"]);
    assert!(!paths.roots_written, "no `roots` key in the example");
    let scheme = IdScheme::from_toml(DATA_EXAMPLE).expect("[ids]");
    assert_eq!(scheme.prefixes().len(), 1);
}

#[test]
fn defaults_without_the_tables() {
    let config = CheckConfig::from_toml("[ids]\nX = { kind = \"x\" }\n").expect("no check tables");
    assert_eq!(config, CheckConfig::default());
    assert_eq!(config.mode, Mode::Enforce, "enforce is the default mode");
    assert_eq!(config.budgets.tier0_bytes, DEFAULT_TIER0_BYTES);
    assert_eq!(config.budgets.tier1_bytes, DEFAULT_TIER1_BYTES);
    assert_eq!(config.budgets.index_bytes, DEFAULT_INDEX_BYTES);
    assert_eq!(config.budgets.decision_bytes, DEFAULT_DECISION_BYTES);
    assert_eq!(
        (
            DEFAULT_TIER0_BYTES,
            DEFAULT_TIER1_BYTES,
            DEFAULT_INDEX_BYTES,
            DEFAULT_DECISION_BYTES
        ),
        (16384, 10240, 10240, 1536),
        "the caps of ADR-0022"
    );
    assert_eq!(
        config.budgets.canon_bytes, None,
        "canon_bytes absent: no cap"
    );
    // Default contracts: open, the load-bearing keys plus `class`.
    let required = |class| config.classes.get(class).required.clone();
    assert_eq!(
        required(DocClass::Canon),
        strings(&["class", "owner", "reviewed"])
    );
    assert_eq!(
        required(DocClass::Decision),
        strings(&["class", "id", "status", "scope"])
    );
    assert_eq!(
        required(DocClass::Spec),
        strings(&["class", "status", "scope"])
    );
    assert_eq!(required(DocClass::Generated), strings(&["class"]));
    for class in DocClass::ALL {
        assert!(
            !config.classes.get(class).closed,
            "{class}: open by default"
        );
        assert!(config.classes.get(class).allows("anything"));
    }
    let paths = Paths::from_toml("").expect("empty");
    assert_eq!(
        (paths.tier0, paths.tier1_name, paths.index),
        (None, None, None)
    );
}

#[test]
fn token_bundles_and_observe_are_accepted() {
    let config = CheckConfig::from_toml(
        "[budgets]\nbundle_node = 4000\nbundle_task = 12000\n\n[check]\nmode = \"observe\"\n",
    )
    .expect("valid");
    assert_eq!(config.budgets.bundle_node, Some(4000));
    assert_eq!(config.budgets.bundle_task, Some(12000));
    assert_eq!(config.mode, Mode::Observe);
}

/// docs/features/spec-cli-bundle.md, R9 / AC-13: `bundle_node` is read as
/// a `u32` by `spec bundle`, so the gate takes it from 1 to `u32::MAX`
/// and refuses the rest at its line with that range; every other cap
/// keeps "at least 1" and no upper bound of its own.
#[test]
fn bundle_node_is_capped_at_u32_max_and_no_other_key_is() {
    let config = CheckConfig::from_toml("[budgets]\nbundle_node = 4294967295\n").expect("u32::MAX");
    assert_eq!(config.budgets.bundle_node, Some(u64::from(u32::MAX)));
    let config = CheckConfig::from_toml("[budgets]\nbundle_node = 1\n").expect("1");
    assert_eq!(config.budgets.bundle_node, Some(1));

    for (value, text, line) in [
        ("4294967296", "[budgets]\n\nbundle_node = 4294967296\n", 3),
        ("0", "[budgets]\nbundle_node = 0\n", 2),
        (
            "-1",
            "[ids]\n\n[budgets]\ntier0_bytes = 100\nbundle_node = -1\n",
            5,
        ),
        (
            "9223372036854775807",
            "[budgets]\nbundle_node = 9223372036854775807\n",
            2,
        ),
    ] {
        let shown = CheckConfig::from_toml(text)
            .err()
            .unwrap_or_else(|| panic!("bundle_node = {value}: accepted"))
            .at("specengine.toml");
        assert_eq!(
            shown,
            format!(
                "specengine.toml:{line}: `bundle_node` must be from 1 to 4294967295, not {value}"
            )
        );
    }

    // Past `u32::MAX`, the other caps still load.
    let config = CheckConfig::from_toml(
        "[budgets]\ntier0_bytes = 5000000000\ntier1_bytes = 5000000000\n\
         decision_bytes = 5000000000\nindex_bytes = 5000000000\n\
         canon_bytes = 5000000000\nbundle_task = 5000000000\nbundle_node = 4294967295\n",
    )
    .expect("other caps past u32::MAX");
    assert_eq!(config.budgets.tier0_bytes, 5_000_000_000);
    assert_eq!(config.budgets.tier1_bytes, 5_000_000_000);
    assert_eq!(config.budgets.decision_bytes, 5_000_000_000);
    assert_eq!(config.budgets.index_bytes, 5_000_000_000);
    assert_eq!(config.budgets.canon_bytes, Some(5_000_000_000));
    assert_eq!(config.budgets.bundle_task, Some(5_000_000_000));
    assert_eq!(config.budgets.bundle_node, Some(4_294_967_295));

    // And keep their own wording below 1.
    for (key, line) in [("tier0_bytes", 2), ("tier1_bytes", 2), ("bundle_task", 2)] {
        let shown = CheckConfig::from_toml(&format!("[budgets]\n{key} = 0\n"))
            .err()
            .unwrap_or_else(|| panic!("{key} = 0: accepted"))
            .at("specengine.toml");
        assert_eq!(
            shown,
            format!("specengine.toml:{line}: `{key}` must be at least 1, not 0")
        );
    }
}

/// `(case, text, line)`: each must fail at that line of the file.
const INVALID: &[(&str, &str, usize)] = &[
    (
        "unknown key in [budgets]",
        "[ids]\nX = { kind = \"x\" }\n\n[budgets]\ntier0_bytes = 100\ntier9_bytes = 5\n",
        6,
    ),
    (
        "unknown key in [check]",
        "[check]\nmode = \"enforce\"\nstrict = true\n",
        3,
    ),
    (
        "unknown key in a contract",
        "[classes]\ncanon = { required = [\"owner\"] }\nspec = { required = [\"status\"], forbidden = [\"x\"] }\n",
        3,
    ),
    (
        "unknown class",
        "[classes]\ncanon = { required = [\"owner\"] }\n\nmemo = { required = [\"class\"] }\n",
        4,
    ),
    (
        "wrong type of a cap",
        "[budgets]\n\ntier1_bytes = \"10k\"\n",
        3,
    ),
    (
        "wrong type of closed",
        "[classes]\ndecision = { closed = \"yes\" }\n",
        2,
    ),
    ("tier0_bytes = 0", "[budgets]\ntier0_bytes = 0\n", 2),
    ("a negative cap", "[budgets]\n\n\ndecision_bytes = -1\n", 4),
    ("mode = strict", "[ids]\n\n[check]\nmode = \"strict\"\n", 4),
];

#[test]
fn invalid_check_tables_fail_with_file_line_message() {
    let mut failures = Vec::new();
    for (case, text, line) in INVALID {
        match CheckConfig::from_toml(text) {
            Ok(config) => failures.push(format!("{case}: accepted as {config:?}")),
            Err(error) => {
                let shown = error.at("specengine.toml");
                let prefix = format!("specengine.toml:{line}: ");
                if !shown.starts_with(&prefix) || shown.len() <= prefix.len() {
                    failures.push(format!("{case}: {shown:?} does not start with {prefix:?}"));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn the_messages_name_the_problem() {
    let message = |text: &str| {
        CheckConfig::from_toml(text)
            .unwrap_err()
            .at("specengine.toml")
    };
    let unknown = message("[budgets]\ntier9_bytes = 5\n");
    assert!(unknown.contains("tier9_bytes"), "{unknown}");
    let class = message("[classes]\nmemo = {}\n");
    assert!(class.contains("memo"), "{class}");
    let cap = message("[budgets]\ntier0_bytes = 0\n");
    assert!(cap.contains("tier0_bytes"), "{cap}");
    let mode = message("[check]\nmode = \"strict\"\n");
    assert!(mode.contains("strict"), "{mode}");
}

#[test]
fn invalid_new_paths_keys_fail_with_file_line_message() {
    for (case, text, line) in [
        (
            "tier1_name with a slash",
            "[paths]\n\ntier1_name = \"docs/README.md\"\n",
            3,
        ),
        ("tier0 absolute", "[paths]\ntier0 = \"/CLAUDE.md\"\n", 2),
        (
            "index with ..",
            "[paths]\nindex = \"../docs/index.md\"\n",
            2,
        ),
        ("unknown paths key", "[paths]\ntier2 = \"x.md\"\n", 2),
    ] {
        let error = Paths::from_toml(text)
            .err()
            .unwrap_or_else(|| panic!("{case}: accepted"));
        let shown = error.at("specengine.toml");
        assert!(
            shown.starts_with(&format!("specengine.toml:{line}: ")),
            "{case}: {shown}"
        );
    }
}

// ------------------------------------------------------------------ the generator registry
// AC-05 of docs/features/spec-check-graph.md (the core half): the
// `[[generators]]` table. The rule half is in `check_generated.rs`, the
// re-parse half in the store's `tests/check_config.rs`.

/// The canon holding the registry example (§11.6).
const REGISTRY_CANON: &str = "docs/canon/spec-check-graph.md";

/// The `[paths]` the example's `index = true` entry needs; three lines, so
/// the entry's `[[generators]]` header is line 4 of [`registry_example`].
const REGISTRY_PATHS: &str = "[paths]\nindex = \"docs/index.md\"\n\n";

/// The registry example of docs/canon/spec-check-graph.md §11.6, verbatim:
/// read from the canon at test time (its first ```` ```toml ```` block
/// under the §11.6 heading), so the test and the canon cannot drift apart;
/// [`REGISTRY_PATHS`] in front of it.
fn registry_example() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(REGISTRY_CANON);
    let canon =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let heading = "\n## §11.6: the generator registry\n";
    let section = &canon[canon
        .find(heading)
        .unwrap_or_else(|| panic!("{REGISTRY_CANON}: no {heading:?}"))..];
    let open = "\n```toml\n";
    let block = &section[section
        .find(open)
        .unwrap_or_else(|| panic!("{REGISTRY_CANON} §11.6: no toml block"))
        + open.len()..];
    let block = &block[..block
        .find("\n```\n")
        .unwrap_or_else(|| panic!("{REGISTRY_CANON} §11.6: the toml block is unclosed"))
        + 1];
    assert!(
        block.starts_with("[[generators]]"),
        "{REGISTRY_CANON} §11.6: {block}"
    );
    format!("{REGISTRY_PATHS}{block}")
}

#[test]
fn the_registry_example_loads() {
    use specengine_core::check::{DEFAULT_GATE, Generator, Shard, ShardKind};
    let example = registry_example();
    let config = CheckConfig::from_toml(&example)
        .unwrap_or_else(|e| panic!("{}\n{example}", e.at("specengine.toml")));
    // This repository's block (#index-shards): the root plus the archive shard.
    let want = Generator {
        command: "cargo run -q -p specengine-cli -- export index".to_owned(),
        writes: strings(&["docs/index.md", "docs/index-archive.md"]),
        index: true,
        gate: Some("cargo run -q -p specengine-cli -- check".to_owned()),
        line: 4,
        shards: vec![Shard {
            path: "docs/index-archive.md".to_owned(),
            kind: ShardKind::Tier3,
            line: 9,
        }],
    };
    assert_eq!(
        config.generators.as_deref(),
        Some(std::slice::from_ref(&want))
    );
    assert_eq!(config.index_generator(), Some(&want));
    assert_eq!(want.gate(), "cargo run -q -p specengine-cli -- check");
    assert_eq!(DEFAULT_GATE, "spec check");
    // The other tables keep their defaults.
    assert_eq!(config.mode, Mode::Enforce);
    assert_eq!(config.budgets, CheckConfig::default().budgets);
}

#[test]
fn registry_entries_keep_their_order_lines_and_default_gate() {
    let text = "\
[paths]
index = \"site/toc.md\"

[[generators]]
command = \"make toc\"
writes = [\"site/toc.md\", \"site/toc.md\"]
index = true

[[generators]]
command = \"tool gen\"
writes = [\"b.md\", \"site/extra.md\", \"b.md\"]
index = false
";
    let config =
        CheckConfig::from_toml(text).unwrap_or_else(|e| panic!("{}", e.at("specengine.toml")));
    let generators = config.generators.as_deref().expect("the registry");
    let seen: Vec<(&str, Vec<&str>, bool, usize)> = generators
        .iter()
        .map(|g| {
            (
                g.command.as_str(),
                g.writes.iter().map(String::as_str).collect(),
                g.index,
                g.line,
            )
        })
        .collect();
    assert_eq!(
        seen,
        [
            ("make toc", vec!["site/toc.md"], true, 4),
            ("tool gen", vec!["b.md", "site/extra.md"], false, 9),
        ],
        "order written, repeats in `writes` dropped, the header's line"
    );
    let index = config.index_generator().expect("the index entry");
    assert_eq!(index.command, "make toc");
    assert_eq!(index.gate, None);
    assert_eq!(index.gate(), "spec check", "the default gate");
    // No `index = true` entry: none.
    let plain = CheckConfig::from_toml("[[generators]]\ncommand = \"a\"\nwrites = [\"a.md\"]\n")
        .expect("valid");
    assert_eq!(plain.index_generator(), None);
}

#[test]
fn absent_and_empty_registries_differ() {
    let absent = CheckConfig::from_toml("[ids]\nX = { kind = \"x\" }\n").expect("valid");
    assert_eq!(absent.generators, None, "no table: the rules are off");
    let empty =
        CheckConfig::from_toml("generators = []\n\n[ids]\nX = { kind = \"x\" }\n").expect("valid");
    assert_eq!(
        empty.generators,
        Some(Vec::new()),
        "empty: the rules are on"
    );
    assert_eq!(empty.index_generator(), None);
}

/// `(case, text, line, fragment of the message)`: each must fail as
/// `specengine.toml:<line>: message`, the message naming the problem.
const INVALID_REGISTRY: &[(&str, &str, usize, &str)] = &[
    (
        "unknown key",
        "[[generators]]\ncommand = \"a\"\nwrites = [\"a.md\"]\nrun = true\n",
        4,
        "run",
    ),
    (
        "command of the wrong type",
        "[[generators]]\ncommand = 5\nwrites = [\"a.md\"]\n",
        2,
        "",
    ),
    (
        "writes of the wrong type",
        "[[generators]]\ncommand = \"a\"\nwrites = \"a.md\"\n",
        3,
        "",
    ),
    (
        "a writes item of the wrong type",
        "[[generators]]\ncommand = \"a\"\nwrites = [\"a.md\", 3]\n",
        3,
        "",
    ),
    (
        "index of the wrong type",
        "[paths]\nindex = \"a.md\"\n\n[[generators]]\ncommand = \"a\"\nwrites = [\"a.md\"]\nindex = \"yes\"\n",
        7,
        "",
    ),
    (
        "gate of the wrong type",
        "[paths]\nindex = \"a.md\"\n\n[[generators]]\ncommand = \"a\"\nwrites = [\"a.md\"]\nindex = true\ngate = 1\n",
        8,
        "",
    ),
    (
        "a [generators] table instead of an array of tables",
        "[ids]\nX = { kind = \"x\" }\n\n[generators]\ncommand = \"a\"\nwrites = [\"a.md\"]\n",
        4,
        "",
    ),
    (
        "command missing",
        "[ids]\n\n[[generators]]\nwrites = [\"a.md\"]\n",
        3,
        "command",
    ),
    (
        "command blank",
        "[[generators]]\nwrites = [\"a.md\"]\ncommand = \"  \"\n",
        3,
        "command",
    ),
    (
        "command empty",
        "[[generators]]\ncommand = \"\"\nwrites = [\"a.md\"]\n",
        2,
        "command",
    ),
    (
        "command repeated",
        "[[generators]]\ncommand = \"a\"\nwrites = [\"a.md\"]\n\n[[generators]]\ncommand = \"a\"\nwrites = [\"b.md\"]\n",
        6,
        "repeated",
    ),
    (
        "writes missing",
        "[[generators]]\ncommand = \"a\"\n",
        1,
        "writes",
    ),
    (
        "writes empty",
        "[[generators]]\ncommand = \"a\"\n\nwrites = []\n",
        4,
        "writes",
    ),
    (
        "writes absolute",
        "[[generators]]\ncommand = \"a\"\nwrites = [\"/a.md\"]\n",
        3,
        "writes",
    ),
    (
        "writes with ..",
        "[[generators]]\ncommand = \"a\"\nwrites = [\"../a.md\"]\n",
        3,
        "writes",
    ),
    (
        "writes with a . component",
        "[[generators]]\ncommand = \"a\"\nwrites = [\"docs/./a.md\"]\n",
        3,
        "writes",
    ),
    (
        "writes with an empty component",
        "[[generators]]\ncommand = \"a\"\nwrites = [\"docs//a.md\"]\n",
        3,
        "writes",
    ),
    (
        "writes a directory",
        "[[generators]]\ncommand = \"a\"\nwrites = [\"docs/\"]\n",
        3,
        "writes",
    ),
    (
        "writes empty string",
        "[[generators]]\ncommand = \"a\"\nwrites = [\"\"]\n",
        3,
        "writes",
    ),
    (
        "writes shared with another entry",
        "[[generators]]\ncommand = \"a\"\nwrites = [\"a.md\"]\n\n[[generators]]\ncommand = \"b\"\nwrites = [\"b.md\",\n  \"a.md\"]\n",
        8,
        "a.md",
    ),
    (
        "index = true twice",
        "[paths]\nindex = \"a.md\"\n\n[[generators]]\ncommand = \"a\"\nwrites = [\"a.md\"]\nindex = true\n\n[[generators]]\ncommand = \"b\"\nwrites = [\"b.md\"]\nindex = true\n",
        12,
        "index",
    ),
    (
        "gate without index",
        "[[generators]]\ncommand = \"a\"\nwrites = [\"a.md\"]\ngate = \"make check\"\n",
        4,
        "gate",
    ),
    (
        "gate with index = false",
        "[paths]\nindex = \"a.md\"\n\n[[generators]]\ncommand = \"a\"\nwrites = [\"a.md\"]\nindex = false\ngate = \"make check\"\n",
        8,
        "gate",
    ),
    (
        "index = true without [paths] index",
        "[paths]\nrecords = \"r\"\n\n[[generators]]\ncommand = \"a\"\nwrites = [\"a.md\"]\nindex = true\n",
        7,
        "[paths] index",
    ),
    (
        "index = true without any [paths]",
        "[[generators]]\ncommand = \"a\"\nwrites = [\"a.md\"]\nindex = true\n",
        4,
        "[paths] index",
    ),
    (
        "index = true while writes lacks [paths] index",
        "[paths]\nindex = \"docs/index.md\"\n\n[[generators]]\ncommand = \"a\"\nwrites = [\"docs/other.md\"]\nindex = true\n",
        7,
        "docs/index.md",
    ),
];

#[test]
fn invalid_registries_fail_with_file_line_message() {
    let mut failures = Vec::new();
    for (case, text, line, fragment) in INVALID_REGISTRY {
        match CheckConfig::from_toml(text) {
            Ok(config) => failures.push(format!("{case}: accepted as {config:?}")),
            Err(error) => {
                let shown = error.at("specengine.toml");
                let prefix = format!("specengine.toml:{line}: ");
                if !shown.starts_with(&prefix) || shown.len() <= prefix.len() {
                    failures.push(format!("{case}: {shown:?} does not start with {prefix:?}"));
                } else if !shown[prefix.len()..].contains(fragment) {
                    failures.push(format!("{case}: {shown:?} does not name {fragment:?}"));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A `[paths]` error is `Paths::from_toml`'s to report: the registry's
/// cross-check waits for it rather than guessing the index path.
#[test]
fn a_paths_error_is_reported_by_paths_not_by_the_registry() {
    let text = "[paths]\nindex = \"../x.md\"\n\n[[generators]]\ncommand = \"a\"\nwrites = [\"a.md\"]\nindex = true\n";
    assert!(Paths::from_toml(text).is_err());
    let config =
        CheckConfig::from_toml(text).unwrap_or_else(|e| panic!("{}", e.at("specengine.toml")));
    assert!(config.index_generator().is_some());
}

/// The registry is not part of the `[ids]` scheme: editing it changes no
/// scheme (so no fingerprint; the store half checks `parsed: 0`).
#[test]
fn the_registry_does_not_touch_the_scheme() {
    let base = "[ids]\nADR = { kind = \"decision\", width = 4 }\n";
    let with = format!(
        "{base}\n[[generators]]\ncommand = \"a\"\nwrites = [\"a.md\"]\n\n[[generators]]\ncommand = \"b\"\nwrites = [\"b.md\"]\n"
    );
    assert_eq!(
        IdScheme::from_toml(base).expect("valid"),
        IdScheme::from_toml(&with).expect("valid")
    );
}

// ------------------------------------------------------------------ iteration 2
// `command` and `gate` are plain YAML scalars that read back as themselves
// from the rendered `generator:` line; a blank `gate` is an error.

/// `(value as a TOML basic string body, the value, the problem named)`.
const NOT_PLAIN: &[(&str, &str, &str)] = &[
    (
        "just docs # regen",
        "just docs # regen",
        "contains `: ` or ` #`",
    ),
    ("a: b", "a: b", "contains `: ` or ` #`"),
    ("make docs: all", "make docs: all", "contains `: ` or ` #`"),
    (" x", " x", "has leading or trailing whitespace"),
    ("x ", "x ", "has leading or trailing whitespace"),
    ("x\\t", "x\t", "contains a newline or a control character"),
    ("-x", "-x", "starts with the YAML indicator `-`"),
    ("[x]", "[x]", "starts with the YAML indicator `[`"),
    ("x:", "x:", "ends with `:`"),
    ("make docs:", "make docs:", "ends with `:`"),
    ("a\\nb", "a\nb", "contains a newline or a control character"),
    ("a\\tb", "a\tb", "contains a newline or a control character"),
    (
        "a\\u0007b",
        "a\u{7}b",
        "contains a newline or a control character",
    ),
    ("a\\rb", "a\rb", "contains a newline or a control character"),
];

/// Plain scalars that must be accepted (and read back as themselves).
const PLAIN: &[&str] = &[
    "cargo run -q -p specengine-cli -- export index",
    "cargo run -q -p specengine-cli -- check",
    "make docs",
    "npm run docs:build",
    "a:b",
    "x#y",
    "tool --out=docs/index.md",
    "a - b",
    "x?",
    "just docs-index",
    "spec check",
];

fn command_entry(body: &str) -> String {
    format!("[[generators]]\ncommand = \"{body}\"\nwrites = [\"a.md\"]\n")
}

fn gate_entry(body: &str) -> String {
    format!(
        "[paths]\nindex = \"a.md\"\n\n[[generators]]\ncommand = \"make\"\nwrites = [\"a.md\"]\nindex = true\ngate = \"{body}\"\n"
    )
}

#[test]
fn command_and_gate_must_be_plain_scalars() {
    let mut failures = Vec::new();
    for (key, entry, line) in [
        ("command", command_entry as fn(&str) -> String, 2),
        ("gate", gate_entry, 8),
    ] {
        for (body, value, problem) in NOT_PLAIN {
            let text = entry(body);
            match CheckConfig::from_toml(&text) {
                Ok(config) => failures.push(format!("{key} {value:?}: accepted as {config:?}")),
                Err(error) => {
                    let shown = error.at("specengine.toml");
                    let prefix =
                        format!("specengine.toml:{line}: generator `{key}` {value:?} {problem}");
                    if !shown.starts_with(&prefix) {
                        failures.push(format!("{key} {value:?}: {shown:?}\n  want {prefix:?}"));
                    }
                }
            }
        }
        // Every YAML indicator, first.
        for indicator in [
            '-', '[', ']', '{', '}', ',', '&', '*', '!', '|', '>', '\'', '"', '%', '@', '`', '#',
            '?', ':',
        ] {
            let value = format!("{indicator}x");
            let body = if indicator == '"' {
                "\\\"x".to_owned()
            } else {
                value.clone()
            };
            let want = format!(
                "specengine.toml:{line}: generator `{key}` {value:?} starts with the YAML indicator `{indicator}`"
            );
            match CheckConfig::from_toml(&entry(&body)) {
                Ok(_) => failures.push(format!("{key} {value:?}: accepted")),
                Err(error) if !error.at("specengine.toml").starts_with(&want) => {
                    failures.push(format!("{key} {value:?}: {}", error.at("specengine.toml")))
                }
                Err(_) => {}
            }
        }
        for value in PLAIN {
            if let Err(error) = CheckConfig::from_toml(&entry(value)) {
                failures.push(format!(
                    "{key} {value:?} rejected: {}",
                    error.at("specengine.toml")
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn an_accepted_command_reads_back_from_the_rendered_header() {
    use specengine_core::check::{CheckInput, render_index};
    let scheme = IdScheme::from_toml("").expect("empty scheme");
    for value in PLAIN {
        let config = CheckConfig::from_toml(&format!(
            "[paths]\nindex = \"a.md\"\n\n[[generators]]\ncommand = \"{value}\"\nwrites = [\"a.md\"]\nindex = true\ngate = \"{value}\"\n"
        ))
        .unwrap_or_else(|e| panic!("{}", e.at("specengine.toml")));
        let generator = config.index_generator().unwrap();
        let render = render_index(&CheckInput::default(), "a.md", generator);
        let parsed = specengine_core::parse("a.md", render.as_bytes(), &scheme);
        let codes: Vec<&str> = parsed.diagnostics.iter().map(|d| d.code.as_str()).collect();
        assert!(
            !codes.iter().any(|code| code.starts_with("frontmatter-")),
            "{value:?}: {codes:?}"
        );
        let generator_read = parsed
            .document()
            .and_then(|d| d.fields.as_ref())
            .and_then(|f| f.generator.as_deref());
        assert_eq!(generator_read, Some(*value), "`generator:` reads back");
        assert!(
            render.contains(&format!("and `{value}` rejects them")),
            "the gate as written"
        );
    }
}

#[test]
fn a_blank_gate_is_an_error() {
    for (body, line) in [("", 8), ("   ", 8), ("\\t", 8)] {
        let text = gate_entry(body);
        let error = CheckConfig::from_toml(&text)
            .err()
            .unwrap_or_else(|| panic!("gate {body:?}: accepted"));
        assert_eq!(
            error.at("specengine.toml"),
            format!("specengine.toml:{line}: generator `gate` is blank"),
            "gate {body:?}"
        );
    }
    // A blank gate without `index = true` is still the `gate`-without-index
    // error (checked first).
    let error = CheckConfig::from_toml(
        "[[generators]]\ncommand = \"make\"\nwrites = [\"a.md\"]\ngate = \"\"\n",
    )
    .unwrap_err()
    .at("specengine.toml");
    assert!(error.starts_with("specengine.toml:4: "), "{error}");
    assert!(error.contains("`gate` is only for"), "{error}");
}

// ------------------------------------------------------------------ iteration 3
// Values the YAML 1.2 core schema reads as a null, a boolean or a number,
// and values holding `-->`, are rejected for `command` and `gate`; every
// accepted value must read back as the same string through this parser
// (serde-saphyr), from the rendered `generator:` line.

const NON_STRING: &str = "is read by YAML as a null, a boolean or a number, not a string";
const CLOSES_COMMENT: &str = "contains `-->`, which would close the index header's comment";

/// `(value, problem)`: rejected for both keys.
const REJECTED_3: &[(&str, &str)] = &[
    ("true", NON_STRING),
    ("True", NON_STRING),
    ("FALSE", NON_STRING),
    ("null", NON_STRING),
    ("NULL", NON_STRING),
    ("~", NON_STRING),
    ("123", NON_STRING),
    ("+1", NON_STRING),
    ("0", NON_STRING),
    ("007", NON_STRING),
    ("0x1F", NON_STRING),
    ("0o17", NON_STRING),
    ("1.5", NON_STRING),
    ("1e3", NON_STRING),
    ("1E-3", NON_STRING),
    ("+1.5e+3", NON_STRING),
    (".5", NON_STRING),
    ("5.", NON_STRING),
    (".inf", NON_STRING),
    ("+.INF", NON_STRING),
    (".Inf", NON_STRING),
    (".nan", NON_STRING),
    (".NaN", NON_STRING),
    (".NAN", NON_STRING),
    // Iteration 4: what serde-saphyr reads as a non-string beyond the core
    // schema (any-case booleans and null, uppercase and binary prefixes,
    // `_` separators, an any-case infinity normalised to `.inf`).
    ("tRUE", NON_STRING),
    ("TrUe", NON_STRING),
    ("nULL", NON_STRING),
    ("1_000", NON_STRING),
    ("0b101", NON_STRING),
    ("0O17", NON_STRING),
    ("0X1F", NON_STRING),
    (".INf", NON_STRING),
    ("a-->b", CLOSES_COMMENT),
    ("make --> docs", CLOSES_COMMENT),
];

/// The ending each key's message carries after the problem.
fn ending(key: &str) -> &'static str {
    if key == "command" {
        ": the index header carries it as written (`generator:` and the build comment)"
    } else {
        ": the index header names it as written"
    }
}

#[test]
fn non_string_and_comment_closing_values_are_rejected() {
    let mut failures = Vec::new();
    for (key, entry, line) in [
        ("command", command_entry as fn(&str) -> String, 2),
        ("gate", gate_entry, 8),
    ] {
        for (value, problem) in REJECTED_3 {
            let want = format!(
                "specengine.toml:{line}: generator `{key}` {value:?} {problem}{}",
                ending(key)
            );
            match CheckConfig::from_toml(&entry(value)) {
                Ok(_) => failures.push(format!("{key} {value:?}: accepted")),
                Err(error) if error.at("specengine.toml") != want => failures.push(format!(
                    "{key} {value:?}: {:?}\n  want {want:?}",
                    error.at("specengine.toml")
                )),
                Err(_) => {}
            }
        }
        // The earlier problems carry the same ending.
        let error = CheckConfig::from_toml(&entry("a: b"))
            .unwrap_err()
            .at("specengine.toml");
        assert!(error.ends_with(ending(key)), "{error}");
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Accepted near-misses of the core schema, and YAML 1.1 spellings: each
/// reads back through serde-saphyr as the same string.
const NEAR_MISSES: &[&str] = &[
    // The coordinator's list (`tRUE` moved to the rejected table).
    "make e1",
    "v1.0",
    "1e",
    "0x",
    "0o8",
    "e3",
    ".",
    "1.2.3",
    "nan",
    "inf",
    // YAML 1.1 and other spellings the rule accepts.
    "yes",
    "No",
    "on",
    "OFF",
    "y",
    "1:30",
    "2026-09-29",
    "NaN",
    "+inf",
    "0x1G",
    "1e+",
    ".e3",
    "1__0",
];

/// What this parser reads from `generator: <value>` in the rendered header:
/// `Ok(string)`, or `Err(what it read instead)`.
fn read_back(value: &str) -> Result<String, String> {
    use specengine_core::check::{CheckInput, Generator, render_index};
    let generator = Generator {
        command: value.to_owned(),
        writes: vec!["a.md".to_owned()],
        index: true,
        gate: None,
        line: 1,
        shards: Vec::new(),
    };
    let render = render_index(&CheckInput::default(), "a.md", &generator);
    let scheme = IdScheme::from_toml("").expect("empty scheme");
    let parsed = specengine_core::parse("a.md", render.as_bytes(), &scheme);
    let document = parsed.document().expect("a document");
    if let Some(read) = document.fields.as_ref().and_then(|f| f.generator.clone()) {
        return Ok(read);
    }
    let extra = document
        .extra
        .iter()
        .flatten()
        .find(|entry| entry.key == "generator")
        .map(|entry| format!("{:?}", entry.value));
    Err(extra.unwrap_or_else(|| "null (no value)".to_owned()))
}

#[test]
fn every_accepted_near_miss_reads_back_as_the_same_string() {
    let mut mismatches = Vec::new();
    for value in NEAR_MISSES {
        for (key, entry) in [
            ("command", command_entry as fn(&str) -> String),
            ("gate", gate_entry),
        ] {
            if let Err(error) = CheckConfig::from_toml(&entry(value)) {
                mismatches.push(format!(
                    "{key} {value:?} rejected: {}",
                    error.at("specengine.toml")
                ));
            }
        }
        match read_back(value) {
            Ok(read) if read == *value => {}
            Ok(read) => mismatches.push(format!(
                "{value:?} accepted, but `generator:` reads back as the string {read:?}"
            )),
            Err(read) => mismatches.push(format!(
                "{value:?} accepted, but `generator:` reads back as {read}, not a string"
            )),
        }
    }
    assert!(
        mismatches.is_empty(),
        "the rule and serde-saphyr disagree:\n{}",
        mismatches.join("\n")
    );
}

#[test]
fn rejected_values_do_not_read_back_but_the_normalised_spellings() {
    // Rejecting is safe; this pins where the rule is stricter than the
    // parser: `.inf` and `.nan` are the spellings the parser normalises
    // every infinity and NaN to, so they alone read back as themselves.
    let mut reads_back: Vec<&str> = REJECTED_3
        .iter()
        .filter(|(_, problem)| *problem == NON_STRING)
        .map(|(value, _)| *value)
        .filter(|value| read_back(value).as_deref() == Ok(*value))
        .collect();
    reads_back.sort_unstable();
    assert_eq!(reads_back, [".inf", ".nan"]);
}

// ------------------------------------------------------------------ link_base
// AC-04 of docs/features/spec-check-links.md (the core half): the optional
// `[paths] link_base`, checked like a `roots` item; no default. The
// re-parse and cannot-check halves are in the store's `tests/check_config.rs`.

#[test]
fn link_base_is_an_optional_root_relative_directory_without_default() {
    let base = Paths::from_toml("[paths]\nroots = [\"docs\"]\nlink_base = \"docs\"\n")
        .unwrap_or_else(|error| panic!("{}", error.at("specengine.toml")));
    assert_eq!(base.link_base.as_deref(), Some("docs"));
    let slash = Paths::from_toml("[paths]\nroots = [\"docs\"]\nlink_base = \"docs/\"\n")
        .unwrap_or_else(|error| panic!("{}", error.at("specengine.toml")));
    assert_eq!(slash, base, "one trailing `/` dropped: equal to `docs`");
    let nested = Paths::from_toml("[paths]\nlink_base = \"docs/spec\"\n")
        .unwrap_or_else(|error| panic!("{}", error.at("specengine.toml")));
    assert_eq!(nested.link_base.as_deref(), Some("docs/spec"));
    // Existence is not checked: a base outside every root loads.
    let elsewhere = Paths::from_toml("[paths]\nroots = [\"docs\"]\nlink_base = \"wiki\"\n")
        .unwrap_or_else(|error| panic!("{}", error.at("specengine.toml")));
    assert_eq!(elsewhere.link_base.as_deref(), Some("wiki"));
    // No default anywhere (ADR-0008).
    assert_eq!(Paths::default().link_base, None);
    for text in ["", "[paths]\n", "[paths]\nroots = [\"docs\"]\n"] {
        let paths = Paths::from_toml(text).expect("loads");
        assert_eq!(paths.link_base, None, "{text:?}");
    }
    // The other readers ignore the key: the scheme and the check tables load.
    let text = "[paths]\nroots = [\"docs\"]\nlink_base = \"docs\"\n\n[ids]\nADR = { kind = \"decision\", width = 4 }\n";
    let scheme =
        IdScheme::from_toml(text).unwrap_or_else(|error| panic!("{}", error.at("specengine.toml")));
    let without = IdScheme::from_toml("[ids]\nADR = { kind = \"decision\", width = 4 }\n")
        .unwrap_or_else(|error| panic!("{}", error.at("specengine.toml")));
    assert_eq!(scheme, without);
    CheckConfig::from_toml(text).unwrap_or_else(|error| panic!("{}", error.at("specengine.toml")));
}

#[test]
fn an_invalid_link_base_fails_with_file_line_message() {
    for (case, value) in [
        ("absolute", "\"/docs\""),
        ("parent", "\"../x\""),
        ("dot component", "\"a/./b\""),
        ("empty", "\"\""),
        ("a number", "1"),
        ("a list", "[\"docs\"]"),
        ("a bool", "true"),
        ("an empty component", "\"docs//spec\""),
        ("a parent inside", "\"docs/../x\""),
        ("a lone dot", "\".\""),
        ("a lone slash", "\"/\""),
    ] {
        let text = format!("[paths]\nroots = [\"docs\"]\n\nlink_base = {value}\n");
        let error = Paths::from_toml(&text)
            .err()
            .unwrap_or_else(|| panic!("{case}: accepted"));
        let shown = error.at("specengine.toml");
        assert!(shown.starts_with("specengine.toml:4: "), "{case}: {shown}");
        assert!(
            shown.len() > "specengine.toml:4: ".len(),
            "{case}: a message: {shown}"
        );
    }
}

// ------------------------------------------------- index shards (ADR-0030)
// docs/features/index-shards.md AC-06: every registry error of its "Data"
// at its line, one case each; `shards = []` is no shard; the table and the
// inline forms load in config order with their lines.

/// The index entry with `writes` and the `shards` array's items, one per
/// line: the entry's header is line 4, `writes` line 6, `shards = [` line
/// 8, the first item line 9.
fn sharded(writes: &str, items: &[&str]) -> String {
    let mut text = format!(
        "[paths]\nindex = \"x/root.md\"\n\n[[generators]]\ncommand = \"gen\"\nwrites = [{writes}]\nindex = true\nshards = [\n"
    );
    for item in items {
        text.push_str(&format!("  {item},\n"));
    }
    text.push_str("]\n");
    text
}

const ROOT_AND_S: &str = "\"x/root.md\", \"x/s.md\"";
const ROOT_S_T: &str = "\"x/root.md\", \"x/s.md\", \"x/t.md\"";

/// `(case, text, line, fragment)`.
fn invalid_shards() -> Vec<(&'static str, String, usize, &'static str)> {
    vec![
        (
            "shards on an entry without index = true",
            "[[generators]]\ncommand = \"gen\"\nwrites = [\"a.md\"]\nshards = [{ path = \"a.md\", tier3 = true }]\n".to_owned(),
            4,
            "only for the entry with `index = true`",
        ),
        (
            "shards not an array of tables",
            "[paths]\nindex = \"x/root.md\"\n\n[[generators]]\ncommand = \"gen\"\nwrites = [\"x/root.md\"]\nindex = true\nshards = \"x/s.md\"\n".to_owned(),
            8,
            "",
        ),
        (
            "a shard that is a string",
            sharded(ROOT_AND_S, &["\"x/s.md\""]),
            9,
            "",
        ),
        (
            "a shard with an unknown key",
            sharded(ROOT_AND_S, &["{ path = \"x/s.md\", tier3 = true, label = \"x\" }"]),
            9,
            "label",
        ),
        (
            "a shard without path",
            sharded(ROOT_AND_S, &["{ tier3 = true }"]),
            9,
            "without `path`",
        ),
        (
            "both tier3 and claims",
            sharded(
                ROOT_AND_S,
                &["{ path = \"x/s.md\", tier3 = true, claims = [\"y/**\"] }"],
            ),
            9,
            "both",
        ),
        (
            "neither tier3 nor claims",
            sharded(ROOT_AND_S, &["{ path = \"x/s.md\" }"]),
            9,
            "without `tier3 = true` or `claims`",
        ),
        (
            "tier3 not true",
            sharded(ROOT_AND_S, &["{ path = \"x/s.md\", tier3 = false }"]),
            9,
            "shard `tier3` is only `true`: set it, or give `claims` instead",
        ),
        (
            "tier3 not a boolean",
            sharded(ROOT_AND_S, &["{ path = \"x/s.md\", tier3 = \"yes\" }"]),
            9,
            "",
        ),
        (
            "claims empty",
            sharded(ROOT_AND_S, &["{ path = \"x/s.md\", claims = [] }"]),
            9,
            "empty",
        ),
        (
            "claims not strings",
            sharded(ROOT_AND_S, &["{ path = \"x/s.md\", claims = [1] }"]),
            9,
            "",
        ),
        (
            "a path leaving the root",
            sharded(ROOT_AND_S, &["{ path = \"../s.md\", tier3 = true }"]),
            9,
            "leaves the root",
        ),
        (
            "an absolute path",
            sharded(ROOT_AND_S, &["{ path = \"/x/s.md\", tier3 = true }"]),
            9,
            "absolute",
        ),
        (
            "the path is [paths] index",
            sharded(ROOT_AND_S, &["{ path = \"x/root.md\", tier3 = true }"]),
            9,
            "is the `[paths] index`",
        ),
        (
            "a path repeated",
            sharded(
                ROOT_AND_S,
                &[
                    "{ path = \"x/s.md\", tier3 = true }",
                    "{ path = \"x/s.md\", claims = [\"y/**\"] }",
                ],
            ),
            10,
            "repeated",
        ),
        (
            "tier3 = true twice",
            sharded(
                ROOT_S_T,
                &[
                    "{ path = \"x/s.md\", tier3 = true }",
                    "{ path = \"x/t.md\", tier3 = true }",
                ],
            ),
            10,
            "twice",
        ),
        (
            "a claim leaving the root",
            sharded(ROOT_AND_S, &["{ path = \"x/s.md\", claims = [\"../y/**\"] }"]),
            9,
            "shard `claims`",
        ),
        (
            "a claim with an empty component",
            sharded(ROOT_AND_S, &["{ path = \"x/s.md\", claims = [\"y//**\"] }"]),
            9,
            "shard `claims`",
        ),
        (
            "a shard path missing from writes",
            sharded("\"x/root.md\"", &["{ path = \"x/s.md\", tier3 = true }"]),
            9,
            "not in the entry's `writes`",
        ),
        (
            "an index writes path that is neither the index nor a shard",
            sharded(ROOT_S_T, &["{ path = \"x/s.md\", tier3 = true }"]),
            6,
            "x/t.md is neither",
        ),
        (
            "an extra writes path without shards",
            "[paths]\nindex = \"x/root.md\"\n\n[[generators]]\ncommand = \"gen\"\nwrites = [\"x/root.md\", \"x/stale.md\"]\nindex = true\n".to_owned(),
            6,
            "x/stale.md is neither",
        ),
    ]
}

#[test]
fn invalid_shards_fail_with_file_line_message() {
    let mut failures = Vec::new();
    for (case, text, line, fragment) in invalid_shards() {
        match CheckConfig::from_toml(&text) {
            Ok(config) => failures.push(format!("{case}: accepted as {config:?}")),
            Err(error) => {
                let shown = error.at("specengine.toml");
                let prefix = format!("specengine.toml:{line}: ");
                if !shown.starts_with(&prefix) || shown.len() <= prefix.len() {
                    failures.push(format!("{case}: {shown:?} does not start with {prefix:?}"));
                } else if !shown[prefix.len()..].contains(fragment) {
                    failures.push(format!("{case}: {shown:?} does not name {fragment:?}"));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The index entry in table form with the shard `path` (TOML source, also
/// in `writes`) on line 10 and `claims` one per line from line 12: the
/// first claim `"y/**"` on line 12, `claim` (TOML source) on line 13.
fn shard_table(path: &str, claim: &str) -> String {
    format!(
        "[paths]\nindex = \"x/root.md\"\n\n[[generators]]\ncommand = \"gen\"\nwrites = [\"x/root.md\", {path}]\nindex = true\n\n[[generators.shards]]\npath = {path}\nclaims = [\n  \"y/**\",\n  {claim},\n]\n"
    )
}

/// AC-06, iteration 2 (the registry errors of "Data", reported): a claim or
/// a shard path the index renders into Markdown (a pointer's label and
/// link, a shard's H1) holding a newline or another control character, or
/// a backtick, fails at its own line, the value shown escaped; `tier3 =
/// false` names both ways out. Exact lines.
#[test]
fn shard_text_that_would_break_the_markdown_fails_at_its_line() {
    const CONTROL: &str =
        "contains a newline or a control character: the index renders it into Markdown";
    const BACKTICK: &str = "contains a backtick: the index renders it into Markdown";
    let good_path = "\"x/s.md\"";
    let good_claim = "\"z/**\"";
    let cases: Vec<(&str, String, String)> = vec![
        (
            "a claim with a newline",
            shard_table(good_path, r#""y/a\nb/**""#),
            format!(r#"specengine.toml:13: shard `claims`: "y/a\nb/**" {CONTROL}"#),
        ),
        (
            "a claim with a tab",
            shard_table(good_path, r#""y/a\tb/**""#),
            format!(r#"specengine.toml:13: shard `claims`: "y/a\tb/**" {CONTROL}"#),
        ),
        (
            "a claim with a carriage return",
            shard_table(good_path, r#""y/a\rb/**""#),
            format!(r#"specengine.toml:13: shard `claims`: "y/a\rb/**" {CONTROL}"#),
        ),
        (
            "a claim with BEL",
            shard_table(good_path, r#""y/a\u0007b/**""#),
            format!(r#"specengine.toml:13: shard `claims`: "y/a\u{{7}}b/**" {CONTROL}"#),
        ),
        (
            "a claim with DEL",
            shard_table(good_path, r#""y/a\u007Fb/**""#),
            format!(r#"specengine.toml:13: shard `claims`: "y/a\u{{7f}}b/**" {CONTROL}"#),
        ),
        (
            "a claim with a C1 control (NEL)",
            shard_table(good_path, r#""y/a\u0085b/**""#),
            format!(r#"specengine.toml:13: shard `claims`: "y/a\u{{85}}b/**" {CONTROL}"#),
        ),
        (
            "a claim with a backtick",
            shard_table(good_path, r#""y/a`b/**""#),
            format!(r#"specengine.toml:13: shard `claims`: "y/a`b/**" {BACKTICK}"#),
        ),
        (
            "a claim that is a backtick and a glob",
            shard_table(good_path, r#""`/**""#),
            format!(r#"specengine.toml:13: shard `claims`: "`/**" {BACKTICK}"#),
        ),
        (
            "a path with a newline",
            shard_table(r#""x/s\n.md""#, good_claim),
            format!(r#"specengine.toml:10: shard `path`: "x/s\n.md" {CONTROL}"#),
        ),
        (
            "a path with a tab",
            shard_table(r#""x/s\t.md""#, good_claim),
            format!(r#"specengine.toml:10: shard `path`: "x/s\t.md" {CONTROL}"#),
        ),
        (
            "a path with an escape character",
            shard_table(r#""x/s\u001B.md""#, good_claim),
            format!(r#"specengine.toml:10: shard `path`: "x/s\u{{1b}}.md" {CONTROL}"#),
        ),
        (
            "a path with a backtick",
            shard_table(r#""x/s`.md""#, good_claim),
            format!(r#"specengine.toml:10: shard `path`: "x/s`.md" {BACKTICK}"#),
        ),
        (
            "tier3 = false",
            sharded(ROOT_AND_S, &["{ path = \"x/s.md\", tier3 = false }"]),
            "specengine.toml:9: shard `tier3` is only `true`: set it, or give `claims` instead"
                .to_owned(),
        ),
    ];
    let mut failures = Vec::new();
    for (case, text, expected) in &cases {
        match CheckConfig::from_toml(text) {
            Ok(config) => failures.push(format!("{case}: accepted as {config:?}")),
            Err(error) => {
                let shown = error.at("specengine.toml");
                if shown != *expected {
                    failures.push(format!(
                        "{case}:\n  got      {shown:?}\n  expected {expected:?}"
                    ));
                }
            }
        }
    }
    // The same entries without the offending character load.
    for text in [
        shard_table(good_path, good_claim),
        shard_table("\"x/s-a.md\"", "\"y/a-b/**\""),
    ] {
        if let Err(error) = CheckConfig::from_toml(&text) {
            failures.push(format!(
                "a clean entry refused: {}",
                error.at("specengine.toml")
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn shards_load_in_config_order_with_their_lines() {
    use specengine_core::check::{Shard, ShardKind};
    // Inline, overlapping claims and a claim matching nothing: no error.
    let text = sharded(
        "\"x/root.md\", \"x/arch.md\", \"x/a.md\", \"x/b.md\"",
        &[
            "{ path = \"x/b.md\", claims = [\"y/**\", \"z/*.md\"] }",
            "{ path = \"x/arch.md\", tier3 = true }",
            "{ path = \"x/a.md\", claims = [\"y/sub/**\", \"nothing/**\"] }",
        ],
    );
    let config =
        CheckConfig::from_toml(&text).unwrap_or_else(|e| panic!("{}", e.at("specengine.toml")));
    let generator = config.index_generator().expect("the index entry");
    assert_eq!(
        generator.shards,
        [
            Shard {
                path: "x/b.md".to_owned(),
                kind: ShardKind::Claims(strings(&["y/**", "z/*.md"])),
                line: 9,
            },
            Shard {
                path: "x/arch.md".to_owned(),
                kind: ShardKind::Tier3,
                line: 10,
            },
            Shard {
                path: "x/a.md".to_owned(),
                kind: ShardKind::Claims(strings(&["y/sub/**", "nothing/**"])),
                line: 11,
            },
        ]
    );
    assert_eq!(
        generator
            .shards
            .iter()
            .map(Shard::is_archive)
            .collect::<Vec<_>>(),
        [false, true, false]
    );

    // `shards = []`: no shard, the single-file index.
    let empty = sharded("\"x/root.md\"", &[]);
    let config =
        CheckConfig::from_toml(&empty).unwrap_or_else(|e| panic!("{}", e.at("specengine.toml")));
    assert!(config.index_generator().unwrap().shards.is_empty());

    // The table form.
    let tables = "[paths]\nindex = \"x/root.md\"\n\n[[generators]]\ncommand = \"gen\"\nwrites = [\"x/root.md\", \"x/arch.md\", \"x/a.md\"]\nindex = true\n\n[[generators.shards]]\npath = \"x/arch.md\"\ntier3 = true\n\n[[generators.shards]]\npath = \"x/a.md\"\nclaims = [\"y/**\"]\n";
    let config =
        CheckConfig::from_toml(tables).unwrap_or_else(|e| panic!("{}", e.at("specengine.toml")));
    let shards = &config.index_generator().unwrap().shards;
    assert_eq!(
        shards
            .iter()
            .map(|s| (s.path.as_str(), s.is_archive()))
            .collect::<Vec<_>>(),
        [("x/arch.md", true), ("x/a.md", false)]
    );

    // No shard: the root alone in `writes` loads as before.
    let plain = "[paths]\nindex = \"x/root.md\"\n\n[[generators]]\ncommand = \"gen\"\nwrites = [\"x/root.md\"]\nindex = true\n";
    let config =
        CheckConfig::from_toml(plain).unwrap_or_else(|e| panic!("{}", e.at("specengine.toml")));
    assert!(config.index_generator().unwrap().shards.is_empty());
}
