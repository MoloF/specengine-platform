//! AC-05 of docs/features/spec-check.md, the re-parse half: editing only the
//! check tables (`[budgets]`, `[classes]`, `[check]`) or the new `[paths]`
//! keys (`tier0`, `tier1_name`, `index`) of `specengine.toml` re-parses
//! nothing: `update` reports `parsed: 0`, and the edited file still loads.

mod common;

use common::{Corpus, Scratch};
use specengine_core::Paths;
use specengine_core::check::{CheckConfig, Mode};

const CHECK_TABLES: &str = "
[budgets]
tier0_bytes    = 16384
tier1_bytes    = 10240
index_bytes    = 10240
decision_bytes = 1536
canon_bytes    = 12288
bundle_node    = 4000

[classes]
decision = { required = [\"class\", \"id\", \"status\", \"scope\"], optional = [\"canon\"], closed = true }
spec = { required = [\"class\", \"status\", \"scope\"] }

[check]
mode = \"observe\"
";

#[test]
fn editing_only_the_check_tables_and_new_paths_keys_parses_nothing() {
    for name in ["spec-a", "spec-b"] {
        let scratch = Scratch::new("check-config");
        let mut corpus = Corpus::copy_of(name, &scratch, "wt");
        let mut index = corpus.open(&scratch.db("index"));
        let first = corpus.update(&mut index);
        assert_eq!(first.parsed, first.walked, "{name}: first update");

        // New tables.
        let toml = corpus.read_text("specengine.toml");
        corpus.write("specengine.toml", format!("{toml}{CHECK_TABLES}"));
        corpus.reload();
        let report = corpus.update(&mut index);
        assert_eq!(report.parsed, 0, "{name}: check tables: {report:?}");
        assert!(!report.reparsed_all, "{name}: check tables: {report:?}");

        // New `[paths]` keys (a `[paths]` table is added where there is none).
        let toml = corpus.read_text("specengine.toml");
        let keys = "tier0 = \"docs/spec/game.md\"\ntier1_name = \"README.md\"\nindex = \"docs/index.md\"\n";
        let edited = if toml.contains("[paths]\n") {
            toml.replacen("[paths]\n", &format!("[paths]\n{keys}"), 1)
        } else {
            format!("[paths]\n{keys}\n{toml}")
        };
        corpus.write("specengine.toml", &edited);
        corpus.reload();
        let report = corpus.update(&mut index);
        assert_eq!(report.parsed, 0, "{name}: new [paths] keys: {report:?}");
        assert!(!report.reparsed_all, "{name}: new [paths] keys: {report:?}");

        // The edited file still loads, every table read.
        let config = CheckConfig::from_toml(&edited)
            .unwrap_or_else(|error| panic!("{}", error.at("specengine.toml")));
        assert_eq!(config.mode, Mode::Observe);
        assert_eq!(config.budgets.canon_bytes, Some(12288));
        let paths = Paths::from_toml(&edited)
            .unwrap_or_else(|error| panic!("{}", error.at("specengine.toml")));
        assert_eq!(paths.index.as_deref(), Some("docs/index.md"));
        assert_eq!(paths, corpus.paths, "{name}: the corpus re-read the same");
    }
}
