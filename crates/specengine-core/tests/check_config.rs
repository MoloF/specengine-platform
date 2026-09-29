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
