//! docs/features/symbol-reads.md "Description and interactions", Crates,
//! **For 6b-1** (Q35): `symbols` and `unmapped` are exported from the CLI
//! library with their request and outcome types as `refs` is, the form an
//! enum (QUERY terms and limit, node, file, `at` raw: the CLI's one
//! `PATH:LINE` parser), the limit bounds as constants (1, 200, 20), so MCP
//! calls them without clap. In process, each outcome serializes to the
//! document `spec --json` prints; a `--node` naming nothing is an outcome
//! with a `reason`, never an error.
//!
//! The library names, as built: the request types `SymbolsRequest { form }`,
//! `SymbolsForm::{Query { terms, limit }, Node, File, At}`,
//! `UnmappedRequest { path }`, the calls `symbols`, `unmapped`, the
//! constants `SYMBOLS_LIMIT_MIN`, `SYMBOLS_LIMIT_MAX`,
//! `SYMBOLS_LIMIT_DEFAULT`; they live in this one binary.
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;
mod symbol_reads_common;

use std::path::Path;

use serde_json::Value;
use specengine_cli::{
    Env, Globals, SYMBOLS_LIMIT_DEFAULT, SYMBOLS_LIMIT_MAX, SYMBOLS_LIMIT_MIN, SymbolsForm,
    SymbolsRequest, UnmappedRequest, symbols, unmapped,
};

use common::{Scratch, spec};
use symbol_reads_common::{FIXTURE, copy};

fn env(root: &Path, home: &Path) -> (Env, Globals) {
    (
        Env {
            cwd: root.to_path_buf(),
            home: Some(home.as_os_str().to_owned()),
            xdg_data_home: None,
        },
        Globals {
            root: Some(root.to_path_buf()),
            config: None,
        },
    )
}

/// `spec --json <args>`'s document.
fn binary(home: &Path, root: &Path, args: &[&str]) -> (i32, Value) {
    let mut json_args = vec!["--json"];
    json_args.extend(args);
    let run = spec(home, root, &json_args);
    (run.code, run.json())
}

/// The bounds `search`'s `SEARCH_LIMIT_*` sit beside, as numbers MCP's
/// descriptions assert at compile time (docs/canon/mcp-code.md "Texts").
#[test]
fn symbols_limit_bounds_are_exported_constants() {
    assert_eq!(
        (SYMBOLS_LIMIT_MIN, SYMBOLS_LIMIT_MAX, SYMBOLS_LIMIT_DEFAULT),
        (1, 200, 20)
    );
}

/// Each form in process serializes to the binary's document (QUERY with
/// and without a limit, a node, a file, `at` given raw), `unmapped` with
/// and without a PATH; a node naming nothing answers `Ok` with a `reason`.
#[test]
fn library_symbols_and_unmapped_equal_the_binary() {
    let scratch = Scratch::new("sr-library");
    let root = copy(&scratch, FIXTURE, "code-index");
    let home = scratch.home("h");
    let (env, globals) = env(&root, &home);
    let forms: [(SymbolsForm, &[&str]); 6] = [
        (
            SymbolsForm::Query {
                terms: vec!["impl".to_owned(), "Sheet".to_owned(), "#2".to_owned()],
                limit: None,
            },
            &["symbols", "impl", "Sheet", "#2"],
        ),
        (
            SymbolsForm::Query {
                terms: vec!["run".to_owned()],
                limit: Some(1),
            },
            &["symbols", "run", "--limit", "1"],
        ),
        (
            SymbolsForm::Node("TENET-WIDTH".to_owned()),
            &["symbols", "--node", "TENET-WIDTH"],
        ),
        (
            SymbolsForm::File("crates/inkwell/src/twins.rs".to_owned()),
            &["symbols", "--file", "crates/inkwell/src/twins.rs"],
        ),
        (
            SymbolsForm::At("crates/inkwell/src/lib.rs:19".to_owned()),
            &["symbols", "--at", "crates/inkwell/src/lib.rs:19"],
        ),
        (
            SymbolsForm::Node("QUILL-99".to_owned()),
            &["symbols", "--node", "QUILL-99"],
        ),
    ];
    for (form, args) in forms {
        let outcome = symbols(&env, &globals, &SymbolsRequest { form })
            .unwrap_or_else(|error| panic!("{args:?}: an outcome, never an error: {error:?}"));
        let library = serde_json::to_value(&outcome).expect("the outcome serializes");
        let (code, document) = binary(&home, &root, args);
        assert_eq!(library, document, "{args:?}");
        assert_eq!(
            code == 1,
            library["reason"].is_string(),
            "{args:?}: exit 1 exactly when a reason"
        );
    }
    for (path, args) in [
        (None, &["unmapped"][..]),
        (
            Some("crates/inkwell/src".to_owned()),
            &["unmapped", "crates/inkwell/src"][..],
        ),
    ] {
        let outcome = unmapped(&env, &globals, &UnmappedRequest { path })
            .unwrap_or_else(|error| panic!("{args:?}: {error:?}"));
        let library = serde_json::to_value(&outcome).expect("the outcome serializes");
        let (code, document) = binary(&home, &root, args);
        assert_eq!(code, 0);
        assert_eq!(library, document, "{args:?}");
    }
}

/// The library refuses what the binary refuses with exit 2: a bad
/// `PATH:LINE` given raw, a limit out of bounds, a PATH outside the walk.
#[test]
fn library_refuses_a_bad_line_limit_or_path() {
    let scratch = Scratch::new("sr-library-refused");
    let root = copy(&scratch, FIXTURE, "code-index");
    let home = scratch.home("h");
    let (env, globals) = env(&root, &home);
    for form in [
        SymbolsForm::At("crates/inkwell/src/lib.rs:0".to_owned()),
        SymbolsForm::At("crates/inkwell/src/lib.rs".to_owned()),
        SymbolsForm::File("docs/spec/ink.md".to_owned()),
        SymbolsForm::Query {
            terms: vec!["run".to_owned()],
            limit: Some(SYMBOLS_LIMIT_MAX + 1),
        },
        SymbolsForm::Query {
            terms: vec!["run".to_owned()],
            limit: Some(0),
        },
    ] {
        let shown = format!("{form:?}");
        assert!(
            symbols(&env, &globals, &SymbolsRequest { form }).is_err(),
            "{shown}: refused"
        );
    }
    assert!(
        unmapped(
            &env,
            &globals,
            &UnmappedRequest {
                path: Some("docs".to_owned()),
            },
        )
        .is_err(),
        "a PATH outside every root"
    );
}
