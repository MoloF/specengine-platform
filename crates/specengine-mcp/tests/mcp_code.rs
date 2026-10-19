//! docs/features/code-tools.md (6b-1) over the real binary over stdio, the
//! default build: the MCP tools `find_symbols`, `refs` and `unmapped`,
//! twins of `spec symbols`, `spec refs` and `spec unmapped`
//! (docs/features/symbol-reads.md, docs/canon/spec-cli-refs.md), in both
//! protocol eras. Parity, forms and exits over scratch copies of
//! `fixtures/code-index` (twins, the tiers with a quoted `#2`, a RON
//! anchor; an item-less file written here for an `unmapped` row of `-`),
//! the single door over a git repository of `fixtures/code-a` with
//! `fixtures/spec-lock/code-a.lock` as it stands, genre over
//! `fixtures/spec-c` (no `[code]`). The parity reference is the CLI
//! library (`common/read.rs` `code_library`): `content` = `spec … 2>&1`,
//! `structuredContent` = the `--json` document, kept at exit 1, absent at
//! exit 2; every document is walked against its `outputSchema` (exact key
//! sets, every key required).
//!
//! - RD-1: `tools/list` is `TOOLS` (eighteen) in both eras; each code
//!   tool has the reads' annotations and `_meta`, an object
//!   `outputSchema`, a description of at most 2 048 ASCII bytes holding
//!   the determinism sentence, its CLI twin and `[code]`, and states only
//!   the CLI's numbers (`find_symbols`' limit bounds, any cut).
//! - RD-2: `find_symbols_takes_exactly_one_form`,
//!   `find_symbols_bad_at_is_the_clis_error`.
//! - RD-3: `code_tools_equal_the_cli`, every tool and exit.
//! - RD-4: `code_mirrors_keep_kinds_free`, `code_mirrors_pin_the_q35_types`
//!   (the flat inputs: `mcp_read.rs` AC-13).
//! - RD-5: `rd5_instructions_are_the_planned_text` (the re-pins:
//!   `mcp_tasks.rs`, `mcp_path.rs`, `mcp_decision.rs`, `mcp_create.rs`).
//! - TE-1: forms, exits 0, 1, 2 (above and
//!   `te1_a_walk_gap_and_a_bad_config_are_the_clis_exit_2`); determinism
//!   (`te1_code_tools_are_byte_identical_across_roots_order_cwd_and_crlf`);
//!   session (`te1_an_edited_rs_shows_in_the_next_call_without_spec_index`);
//!   quiet (`te1_broken_and_non_utf8_code_leave_stdout_json_rpc_only`);
//!   genre (`te1_spec_c_answers_each_code_tool_with_the_code_table_line`,
//!   `genre_code_sources_and_texts_hold_no_kind_or_rust_word`);
//!   `mcp_index.rs`; `mcp_size.rs`.
//! - TE-2: `te2_code_tools_write_nothing_under_the_root`.
//!
//! The 6a-3 library entries are `common/read.rs`' (`symbols`,
//! `unmapped`, `symbols_limit_error`, `SYMBOLS_LIMIT_{MIN,MAX,DEFAULT}`:
//! symbol-reads.md Q35); the two `PLANNED_*_BLAKE3` literals are computed
//! over [`PLANNED_INSTRUCTIONS`], the spec's text, and `mcp_path.rs` and
//! `mcp_decision.rs` pin the same values. A negative `limit` answers the
//! CLI's `--limit` line before any discovery (C1, the developer's
//! iteration 1): `find_symbols_negative_limit_is_the_clis_limit_line`.
//!
//! docs/features/bevy-roles-reads.md (10a-2), `find_symbols`' `roles`
//! (`SymbolsTarget.roles`, docs/canon/mcp-code.md "Mirrors"):
//!
//! - RD-10: `code_tools_equal_the_cli` walks every answer over a scratch
//!   `fixtures/code-bevy` (the opt-in on; a module and a RON marker of
//!   `CHORE-02` written in) against the `outputSchema`, its `roles` keys
//!   met with a value and with `null`, and over code-index (no opt-in)
//!   finds `roles: null` on every target; `code_mirrors_keep_kinds_free`
//!   (each role key a free string or integer, `roles` a nullable array);
//!   `code_mirrors_pin_the_q35_types` (the key sets);
//!   `find_symbols_roles_schema_holds_no_stack_role_or_source_word` (the
//!   doc comments become `outputSchema` text; the stack words of every
//!   text: `mcp_genre.rs`).
//! - TE-3, the `find_symbols` half:
//!   `te3_find_symbols_roles_are_byte_identical_across_roots_order_cwd_and_crlf`.
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

#![cfg(all(unix, not(feature = "probes")))]

mod common;

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use common::blake3::blake3_hex;
use common::read::{
    CODE_TOOLS, ERAS, Era, FIND_SYMBOLS_LIMIT, FIND_SYMBOLS_ONE_FORM, STACK_WORDS, Session, TOOLS,
    assert_bad_arguments, checked_call, cli_env, content_text, has_word, index_with_library,
    output_schemas,
};
use common::*;
use git::Sandbox;
use serde_json::{Value, json};
use specengine_cli::{
    Env, GitEnv, Globals, InboxRequest, OUTPUT_CAP_CHARS, SYMBOLS_LIMIT_DEFAULT, SYMBOLS_LIMIT_MAX,
    SYMBOLS_LIMIT_MIN, TaskListRequest, events_after, inbox, task_list,
};

/// The reads' fixture (Q33).
const FIXTURE: &str = "code-index";

/// An item-less code file written into the scratch copies: its `unmapped`
/// row, and its directory's total, are `-` (`percent` null; Q35).
const BLANK: &str = "crates/inkwell/src/blank/none.rs";

/// `spec refs`' exit-2 line without `[code]`, `<config>` as given
/// (docs/canon/code-bindings.md "The `[code]` table"); the other two
/// commands answer it alike (symbol-reads.md "spec symbols", exit 2).
const NO_CODE: &str =
    "spec: specengine.toml has no [code] table: the code layer is off (add [code] with roots)\n";

/// The determinism sentence of every description.
const DETERMINISM: &str = "Deterministic: one state, one result; no LLM inside.";

/// Each code tool and the CLI command its description names.
const TWINS: [(&str, &str); 3] = [
    ("find_symbols", "spec symbols"),
    ("refs", "spec refs"),
    ("unmapped", "spec unmapped"),
];

/// Language words no MCP source literal or code tool text holds
/// (architecture.md#universal; "Rules and edge cases", Genre), beside
/// `STACK_WORDS`.
const RUST_WORDS: [&str; 11] = [
    "rust",
    "rustc",
    "rust-analyzer",
    "crate",
    "crates",
    "impl",
    "trait",
    "struct",
    "fn",
    "cfg",
    "tree-sitter",
];

/// The fixtures' slugs and package names: never in an MCP literal or text.
const FIXTURE_WORDS: [&str; 7] = [
    "inkwell",
    "tally",
    "code-a",
    "lantern-keep",
    "zerkalo",
    "dock-hire",
    "inkwell-core",
];

/// docs/features/bevy-roles-reads.md's corpus: 10a-1's farm game, its
/// `[code] frameworks = ["bevy"]` on.
const BEVY: &str = "code-bevy";

/// One role of `symbols[].roles[]` (docs/features/bevy-roles-reads.md
/// "find_symbols"): every key a free string or integer.
const ROLE_KEYS: [&str; 11] = [
    "role",
    "layer",
    "path",
    "line",
    "source",
    "form",
    "origin",
    "group",
    "state",
    "detail",
    "candidates",
];

/// The role keys holding an integer; the others hold a string.
const ROLE_INTEGERS: [&str; 2] = ["line", "candidates"];

/// The role keys `null` on some row of code-bevy and set on another.
const ROLE_NULLABLE: [&str; 4] = ["form", "group", "detail", "candidates"];

/// A module and a RON marker of `CHORE-02` written into a code-bevy copy
/// (path, the line as committed, the line with the marker): `{node:
/// "CHORE-02"}` lists a module and a RON target. `clock.rs` holds no role
/// row.
const BEVY_MARKS: [(&str, &str, &str); 2] = [
    (
        "crates/farm/src/clock.rs",
        "//! The clock: what turns over every tick.\n",
        "//! The clock: what turns over every tick.\n//! @implements CHORE-02\n",
    ),
    (
        "assets/farm.ron",
        "    growth_per_tick: 1,\n",
        "    growth_per_tick: 1, // @configures CHORE-02\n",
    ),
];

// ------------------------------------------------------------- harness

/// What a call must answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Want {
    /// Exit 0: success, its document, `reason` null.
    Answered,
    /// Exit 1: `isError`, the `spec:` line, the document with its `reason`.
    NotFound,
    /// Exit 2: `isError`, the `CliError` line(s), no document.
    CannotRun,
    /// The tool's own form error, exactly this text, no document.
    Form(&'static str),
    /// Parity only: the spec pins no exit (a reading names it).
    Parity,
}

impl Want {
    fn check(self, result: &Value, context: &str) {
        let text = content_text(result);
        match self {
            Want::Answered => {
                assert_eq!(result["isError"], json!(false), "{context}: {text}");
                let document = &result["structuredContent"];
                assert!(document.is_object(), "{context}: a document");
                assert_eq!(document["reason"], Value::Null, "{context}: reason");
            }
            Want::NotFound => {
                assert_eq!(result["isError"], json!(true), "{context}: {text}");
                let document = &result["structuredContent"];
                assert!(
                    document["reason"].is_string(),
                    "{context}: the exit-1 document keeps its reason: {document}"
                );
                assert!(
                    text.lines().any(|line| line.starts_with("spec: ")),
                    "{context}: {text:?}"
                );
            }
            Want::CannotRun => {
                assert_eq!(result["isError"], json!(true), "{context}: {text}");
                assert!(
                    result.get("structuredContent").is_none(),
                    "{context}: a document at exit 2: {result}"
                );
                assert!(
                    !text.is_empty() && text.ends_with('\n'),
                    "{context}: the CliError line(s): {text:?}"
                );
            }
            Want::Form(message) => {
                assert_eq!(text, message, "{context}");
                assert_eq!(result["isError"], json!(true), "{context}");
                assert!(
                    result.get("structuredContent").is_none(),
                    "{context}: a document on a form error: {result}"
                );
            }
            Want::Parity => {}
        }
    }
}

/// One call of a parity run.
struct Call {
    label: &'static str,
    tool: &'static str,
    args: Value,
    want: Want,
}

fn call(label: &'static str, tool: &'static str, args: Value, want: Want) -> Call {
    Call {
        label,
        tool,
        args,
        want,
    }
}

/// The result of the call labelled `label`.
fn pick<'a>(calls: &[Call], results: &'a [Value], label: &str) -> &'a Value {
    let at = calls
        .iter()
        .position(|call| call.label == label)
        .unwrap_or_else(|| panic!("no call {label}"));
    &results[at]
}

/// The document of the call labelled `label`.
fn document<'a>(calls: &[Call], results: &'a [Value], label: &str) -> &'a Value {
    &pick(calls, results, label)["structuredContent"]
}

/// A scratch copy of a fixture with its own `HOME`.
struct Project {
    _scratch: Scratch,
    home: PathBuf,
    root: PathBuf,
}

impl Project {
    fn new(fixture: &str) -> Self {
        let scratch = Scratch::new(&format!("code-{fixture}"));
        let home = scratch.home("h");
        let root = scratch.copy(fixture, "copy");
        Self {
            _scratch: scratch,
            home,
            root,
        }
    }

    /// A code-index copy with [`BLANK`] written.
    fn reads() -> Self {
        let project = Self::new(FIXTURE);
        write(&project.root, BLANK, "// Nothing is declared here.\n");
        project
    }

    /// A code-bevy copy (the opt-in on) with [`BEVY_MARKS`] written.
    fn bevy() -> Self {
        let project = Self::new(BEVY);
        mark_bevy(&project.root);
        project
    }

    fn env(&self) -> Env {
        cli_env(&self.root, Some(&self.home))
    }

    fn open(&self, era: Era) -> (Session, std::collections::BTreeMap<String, Value>) {
        let mut session = Session::open(era, &[], Some(&self.root), Home::At(&self.home));
        let schemas = output_schemas(&session.tools());
        (session, schemas)
    }

    /// Every call in both eras, each checked against the library (parity,
    /// its output schema) and its [`Want`]; the results by era, in call
    /// order. Stderr stays empty.
    fn answers(&self, calls: &[Call]) -> Vec<Vec<Value>> {
        let env = self.env();
        let globals = Globals::default();
        let mut by_era = Vec::new();
        for era in ERAS {
            let (mut session, schemas) = self.open(era);
            let mut results = Vec::new();
            for call in calls {
                let result = checked_call(
                    &mut session,
                    &schemas,
                    call.tool,
                    call.args.clone(),
                    &env,
                    &globals,
                );
                call.want
                    .check(&result, &format!("{era:?} {} {}", call.tool, call.args));
                results.push(result);
            }
            let done = session.finish();
            assert!(done.status.success(), "{era:?}: {}", done.stderr);
            assert_eq!(done.stderr, "", "{era:?}: stderr");
            by_era.push(results);
        }
        by_era
    }
}

/// The subschemas `schema` stands for: itself and, recursively, the
/// members of its `anyOf`/`oneOf` (a nullable object or enum).
fn members(schema: &Value) -> Vec<&Value> {
    let mut all = vec![schema];
    for key in ["anyOf", "oneOf"] {
        if let Some(options) = schema.get(key).and_then(Value::as_array) {
            for option in options {
                all.extend(members(option));
            }
        }
    }
    all
}

/// The subschema at `path`: `.`-separated property names, `[]` after a
/// name steps into its items; through nullable unions.
fn schema_at<'a>(schema: &'a Value, path: &str) -> &'a Value {
    let mut at = schema;
    for step in path.split('.').filter(|step| !step.is_empty()) {
        let (key, items) = match step.strip_suffix("[]") {
            Some(key) => (key, true),
            None => (step, false),
        };
        at = members(at)
            .into_iter()
            .find_map(|member| member.get("properties").and_then(|map| map.get(key)))
            .unwrap_or_else(|| panic!("no property {key:?} on the way to {path}"));
        if items {
            at = members(at)
                .into_iter()
                .find_map(|member| member.get("items"))
                .unwrap_or_else(|| panic!("no items at {key:?} on the way to {path}"));
        }
    }
    at
}

/// The values a schema's `enum`s and `const`s admit, through unions;
/// `None` when it has neither (a free value).
fn enum_values(schema: &Value) -> Option<BTreeSet<String>> {
    let mut found: Option<BTreeSet<String>> = None;
    for member in members(schema) {
        if let Some(values) = member.get("enum").and_then(Value::as_array) {
            found
                .get_or_insert_with(BTreeSet::new)
                .extend(values.iter().filter_map(Value::as_str).map(str::to_owned));
        }
        if let Some(value) = member.get("const").and_then(Value::as_str) {
            found
                .get_or_insert_with(BTreeSet::new)
                .insert(value.to_owned());
        }
    }
    found
}

/// The JSON types a schema admits, through unions (`null` from a nullable
/// enum's `null` member too).
fn types_of(schema: &Value) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for member in members(schema) {
        match member.get("type") {
            Some(Value::String(name)) => {
                found.insert(name.clone());
            }
            Some(Value::Array(names)) => {
                found.extend(names.iter().filter_map(Value::as_str).map(str::to_owned));
            }
            _ => {}
        }
        if member
            .get("enum")
            .and_then(Value::as_array)
            .is_some_and(|values| values.iter().any(Value::is_null))
        {
            found.insert("null".to_owned());
        }
    }
    found
}

/// The property names an object schema declares (through unions).
fn keys_at(schema: &Value) -> BTreeSet<String> {
    members(schema)
        .into_iter()
        .filter_map(|member| member.get("properties").and_then(Value::as_object))
        .flat_map(|properties| properties.keys().cloned())
        .collect()
}

fn set(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

/// Every `description` string under `schema`.
fn descriptions(schema: &Value, out: &mut Vec<String>) {
    match schema {
        Value::Object(object) => {
            for (key, value) in object {
                if key == "description" {
                    if let Some(text) = value.as_str() {
                        out.push(text.to_owned());
                    }
                } else {
                    descriptions(value, out);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                descriptions(item, out);
            }
        }
        _ => {}
    }
}

/// The numbers a text states as its cut: right after `cut at ` or
/// `above ` when `characters` follows (as built: "An answer above 40000
/// characters is cut").
fn cut_numbers(text: &str) -> Vec<usize> {
    ["cut at ", "above "]
        .iter()
        .flat_map(|phrase| text.match_indices(phrase))
        .filter_map(|(at, phrase)| {
            let rest = &text[at + phrase.len()..];
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            rest[digits.len()..]
                .starts_with(" characters")
                .then(|| digits.parse().ok())
                .flatten()
        })
        .collect()
}

/// The genre offences of one text: a stack, language or fixture word, a
/// code file suffix, a `::` (MCP never reads a qpath).
fn genre_problems(text: &str) -> Vec<String> {
    let mut problems = Vec::new();
    for word in STACK_WORDS.iter().chain(&RUST_WORDS).chain(&FIXTURE_WORDS) {
        if has_word(text, word) {
            problems.push(format!("the word {word:?}"));
        }
    }
    for part in [".rs", ".ron", "::"] {
        if text.contains(part) {
            problems.push(format!("{part:?}"));
        }
    }
    problems
}

/// [`BEVY_MARKS`] written into a code-bevy copy at `root`.
fn mark_bevy(root: &Path) {
    for (path, from, to) in BEVY_MARKS {
        replace(root, path, from, to);
    }
}

/// The `roles` lists of every target of a `find_symbols` result's
/// document (none for an error without one).
fn role_lists(result: &Value) -> Vec<Value> {
    result["structuredContent"]["symbols"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|target| target["roles"].clone())
        .collect()
}

// ------------------------------------------------------------------ RD-1

/// RD-1: the eighteen tools in both eras; each code tool listed as a read
/// (annotations, `_meta`, an object `outputSchema` without `$ref`), its
/// description within 2 048 ASCII bytes, holding the determinism sentence,
/// its CLI twin and `[code]`; `find_symbols` states its limit bounds as
/// the CLI's constants (reading L6); any `cut at N` is
/// `OUTPUT_CAP_CHARS`. M: a description over 2 048 bytes, non-ASCII or
/// without the sentence; a stated limit not the CLI's; a code tool with a
/// writer's hints.
#[test]
fn rd1_code_tools_are_listed_as_reads() {
    for era in ERAS {
        let mut session = Session::open(era, &[], None, Home::Fresh);
        let list = session.tools();
        assert_eq!(tool_names(&list), TOOLS, "{era:?}: eighteen tools");
        for (name, twin) in TWINS {
            assert!(CODE_TOOLS.contains(&name));
            let listed = tool(&list, name);
            assert_eq!(
                listed["annotations"],
                json!({"readOnlyHint": true, "destructiveHint": false, "openWorldHint": false}),
                "{era:?} {name}: the reads' annotations"
            );
            assert_eq!(
                listed["_meta"],
                json!({"anthropic/maxResultSizeChars": specengine_mcp::MAX_RESULT_CHARS}),
                "{era:?} {name}: _meta"
            );
            assert_eq!(listed["outputSchema"]["type"], json!("object"), "{name}");
            assert!(
                !listed.to_string().contains("\"$ref\""),
                "{era:?} {name}: a $ref"
            );
            let description = listed["description"].as_str().expect("a description");
            assert!(
                description.len() <= 2048,
                "{era:?} {name}: {} bytes",
                description.len()
            );
            assert!(description.is_ascii(), "{era:?} {name}: not ASCII");
            for part in [DETERMINISM, twin, "[code]"] {
                assert!(
                    description.contains(part),
                    "{era:?} {name}: no {part:?} in {description:?}"
                );
            }
            // As built each description states the cut (Phase B reading
            // L25): the number is the CLI's.
            assert!(
                !cut_numbers(description).is_empty(),
                "{era:?} {name}: no cut stated: {description:?}"
            );
            let mut texts = vec![description.to_owned()];
            descriptions(&listed["inputSchema"], &mut texts);
            for text in &texts {
                assert!(text.is_ascii(), "{era:?} {name}: {text:?}");
                for number in cut_numbers(text) {
                    assert_eq!(number, OUTPUT_CAP_CHARS, "{era:?} {name}: {text:?}");
                }
            }
        }
        let symbols = tool(&list, "find_symbols");
        let mut stated = vec![
            symbols["description"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
        ];
        descriptions(&symbols["inputSchema"], &mut stated);
        let stated = stated.join(" ");
        assert!(
            stated.contains(&format!("{SYMBOLS_LIMIT_MIN} to {SYMBOLS_LIMIT_MAX}")),
            "{era:?}: find_symbols states its bounds as the CLI's: {stated:?}"
        );
        assert!(
            has_word(&stated, &SYMBOLS_LIMIT_DEFAULT.to_string()),
            "{era:?}: find_symbols states its default as the CLI's: {stated:?}"
        );
        let done = session.finish();
        assert!(done.status.success(), "{era:?}: {}", done.stderr);
    }
}

// ------------------------------------------------------------------ RD-2

/// RD-2 ("Form errors"): not exactly one of `query`, `node`, `file`, `at`
/// (a `null` is absent) and `limit` beside another form are the tool's own
/// error results, as `get_task`'s: exactly the spec's text, no document,
/// the same before any discovery (a directory without a project: no `spec
/// init` line; reading L3: the form count is judged before `limit`). A
/// form with `null`s beside it is one form. M: zero or two forms reaching
/// the CLI; `limit` with `node`.
#[test]
fn find_symbols_takes_exactly_one_form() {
    let twins = "crates/inkwell/src/twins.rs";
    let at = "crates/inkwell/src/lib.rs:19";
    let refused = [
        (json!({}), FIND_SYMBOLS_ONE_FORM),
        (json!({"limit": 5}), FIND_SYMBOLS_ONE_FORM),
        (
            json!({"query": null, "node": null, "file": null, "at": null}),
            FIND_SYMBOLS_ONE_FORM,
        ),
        (
            json!({"query": "run", "node": "TENET-WIDTH"}),
            FIND_SYMBOLS_ONE_FORM,
        ),
        (
            json!({"query": "run", "file": twins}),
            FIND_SYMBOLS_ONE_FORM,
        ),
        (json!({"query": "run", "at": at}), FIND_SYMBOLS_ONE_FORM),
        (
            json!({"node": "TENET-WIDTH", "file": twins}),
            FIND_SYMBOLS_ONE_FORM,
        ),
        (
            json!({"node": "TENET-WIDTH", "at": at}),
            FIND_SYMBOLS_ONE_FORM,
        ),
        (json!({"file": twins, "at": at}), FIND_SYMBOLS_ONE_FORM),
        (
            json!({"query": "run", "node": "TENET-WIDTH", "file": twins, "at": at, "limit": 2}),
            FIND_SYMBOLS_ONE_FORM,
        ),
        (
            json!({"node": "TENET-WIDTH", "limit": 3}),
            FIND_SYMBOLS_LIMIT,
        ),
        (json!({"file": twins, "limit": 3}), FIND_SYMBOLS_LIMIT),
        (json!({"at": at, "limit": 3}), FIND_SYMBOLS_LIMIT),
        (json!({"node": "QUILL-99", "limit": 1}), FIND_SYMBOLS_LIMIT),
    ];
    let project = Project::new(FIXTURE);
    let mut calls: Vec<Call> = refused
        .iter()
        .map(|(args, message)| call("refused", "find_symbols", args.clone(), Want::Form(message)))
        .collect();
    calls.push(call(
        "query beside nulls",
        "find_symbols",
        json!({"query": "run", "node": null, "file": null, "at": null, "limit": 1}),
        Want::Answered,
    ));
    calls.push(call(
        "node, a null limit",
        "find_symbols",
        json!({"node": "TENET-WIDTH", "limit": null}),
        Want::Answered,
    ));
    for results in project.answers(&calls) {
        let one = document(&calls, &results, "query beside nulls");
        assert_eq!(one["form"], json!("query"), "{one}");
        assert_eq!(one["symbols"].as_array().map(Vec::len), Some(1), "{one}");
        let node = document(&calls, &results, "node, a null limit");
        assert_eq!(node["form"], json!("node"), "{node}");
    }

    // Before any discovery: no project here, yet the form errors are the
    // tool's own; one form reaches the CLI (its `spec init` line).
    let scratch = Scratch::new("code-forms-noproject");
    let home = scratch.home("h");
    let empty = scratch.dir("empty");
    for era in ERAS {
        let mut session = Session::open(era, &[], Some(&empty), Home::At(&home));
        for (args, message) in &refused {
            let reply = session.call("find_symbols", args.clone());
            Want::Form(message).check(result(&reply), &format!("{era:?} no project {args}"));
        }
        let reply = session.call("find_symbols", json!({"query": "run"}));
        let text = content_text(result(&reply));
        assert!(
            text.starts_with("spec: ") && text.contains("spec init"),
            "{era:?}: {text:?}"
        );
        let done = session.finish();
        assert!(done.status.success(), "{era:?}: {}", done.stderr);
        assert_eq!(done.stderr, "", "{era:?}");
    }
}

/// RD-2: `at` goes raw to the CLI's one `PATH:LINE` parser (Q35): every
/// malformed or outside `at` is the CLI's exit-2 line(s) exactly (LINE 0,
/// not a number, absent, empty, negative, spaced, overflowing; an empty
/// PATH; a PATH that is no walked code file; the whole value empty); the
/// values the spec does not pin (a `+`, a second colon, a leading zero, a
/// LINE past the end, a file whose name holds a colon) answer what the
/// CLI answers. M: `at` split in MCP.
#[test]
fn find_symbols_bad_at_is_the_clis_error() {
    let project = Project::new(FIXTURE);
    write(
        &project.root,
        "crates/inkwell/src/odd:name.rs",
        "pub fn odd() -> u32 {\n    1\n}\n",
    );
    let lib = "crates/inkwell/src/lib.rs";
    let bad = [
        format!("{lib}:0"),
        format!("{lib}:x"),
        lib.to_owned(),
        format!("{lib}:"),
        format!("{lib}:-3"),
        format!("{lib}: 3"),
        format!("{lib}:99999999999999999999"),
        ":7".to_owned(),
        String::new(),
        "docs/spec/ink.md:3".to_owned(),
        "crates/inkwell/src/nope.rs:3".to_owned(),
        "crates/inkwell/src:3".to_owned(),
    ];
    let unpinned = [
        format!("{lib}:+3"),
        format!("{lib}:3:4"),
        format!("{lib}:019"),
        format!("{lib}:999"),
        "crates/inkwell/src/odd:name.rs:2".to_owned(),
    ];
    let mut calls: Vec<Call> = bad
        .iter()
        .map(|at| call("bad", "find_symbols", json!({"at": at}), Want::CannotRun))
        .collect();
    calls.extend(
        unpinned
            .iter()
            .map(|at| call("unpinned", "find_symbols", json!({"at": at}), Want::Parity)),
    );
    calls.push(call(
        "valid",
        "find_symbols",
        json!({"at": format!("{lib}:19")}),
        Want::Answered,
    ));
    for results in project.answers(&calls) {
        let valid = document(&calls, &results, "valid");
        assert_eq!(valid["form"], json!("at"), "{valid}");
        assert!(
            !valid["symbols"].as_array().expect("symbols").is_empty(),
            "line 19 lies in an item: {valid}"
        );
    }
}

/// C1 (the developer's iteration 1, `symbols_limit_error`): a `limit`
/// outside 1 to 200 is the CLI's own `--limit` line, exit 2, no document:
/// 0 and 201 from the library, a negative one before any discovery (the
/// library's `usize` cannot hold it), the same words with the value as
/// given. In a directory without a project a negative limit still answers
/// that line. M: MCP wording its own error for a negative limit, or
/// passing it on as `None` (the default 20).
#[test]
fn find_symbols_negative_limit_is_the_clis_limit_line() {
    let line = |limit: i64| format!("spec: --limit {limit}: the limit is 1 to 200\n");
    let project = Project::new(FIXTURE);
    let limits: [i64; 4] = [-3, -1, 0, 201];
    let calls: Vec<Call> = limits
        .iter()
        .map(|limit| {
            call(
                "limit",
                "find_symbols",
                json!({"query": "run", "limit": limit}),
                Want::CannotRun,
            )
        })
        .collect();
    for results in project.answers(&calls) {
        for (limit, result) in limits.iter().zip(&results) {
            assert_eq!(content_text(result), line(*limit), "limit {limit}");
        }
    }
    let scratch = Scratch::new("code-limit-noproject");
    let home = scratch.home("h");
    let empty = scratch.dir("empty");
    for era in ERAS {
        let mut session = Session::open(era, &[], Some(&empty), Home::At(&home));
        let reply = session.call("find_symbols", json!({"query": "run", "limit": -3}));
        Want::CannotRun.check(result(&reply), &format!("{era:?} no project"));
        assert_eq!(content_text(result(&reply)), line(-3), "{era:?}");
        let done = session.finish();
        assert!(done.status.success(), "{era:?}: {}", done.stderr);
        assert_eq!(done.stderr, "", "{era:?}");
    }
}

// ------------------------------------------------------------------ RD-3

/// The calls of the parity run: every form and tool, exits 0, 1, 2 and
/// the form errors.
fn reads() -> Vec<Call> {
    use Want::{Answered, CannotRun, Form, NotFound};
    let twins = "crates/inkwell/src/twins.rs";
    vec![
        call(
            "twins",
            "find_symbols",
            json!({"query": "separator"}),
            Answered,
        ),
        call(
            "quoted #2",
            "find_symbols",
            json!({"query": "impl Sheet #2"}),
            Answered,
        ),
        call(
            "limit",
            "find_symbols",
            json!({"query": "run", "limit": 1}),
            Answered,
        ),
        call(
            "cross-file",
            "find_symbols",
            json!({"query": "joined"}),
            Answered,
        ),
        call(
            "no rows",
            "find_symbols",
            json!({"query": "no_such_item_anywhere"}),
            Answered,
        ),
        call(
            "nulls",
            "find_symbols",
            json!({"query": "probe", "node": null, "file": null, "at": null}),
            Answered,
        ),
        call(
            "node",
            "find_symbols",
            json!({"node": "TENET-WIDTH"}),
            Answered,
        ),
        call(
            "node ron",
            "find_symbols",
            json!({"node": "SLAB-INK"}),
            Answered,
        ),
        call("file", "find_symbols", json!({"file": twins}), Answered),
        call(
            "file ron",
            "find_symbols",
            json!({"file": "data/inks.ron"}),
            Answered,
        ),
        call(
            "at",
            "find_symbols",
            json!({"at": "crates/inkwell/src/lib.rs:19"}),
            Answered,
        ),
        call(
            "node nothing",
            "find_symbols",
            json!({"node": "QUILL-99"}),
            NotFound,
        ),
        call(
            "file md",
            "find_symbols",
            json!({"file": "docs/spec/ink.md"}),
            CannotRun,
        ),
        call(
            "line 0",
            "find_symbols",
            json!({"at": "crates/inkwell/src/lib.rs:0"}),
            CannotRun,
        ),
        call(
            "limit low",
            "find_symbols",
            json!({"query": "run", "limit": SYMBOLS_LIMIT_MIN - 1}),
            CannotRun,
        ),
        call(
            "limit high",
            "find_symbols",
            json!({"query": "run", "limit": SYMBOLS_LIMIT_MAX + 1}),
            CannotRun,
        ),
        call(
            "no form",
            "find_symbols",
            json!({}),
            Form(FIND_SYMBOLS_ONE_FORM),
        ),
        call(
            "two forms",
            "find_symbols",
            json!({"query": "run", "file": twins}),
            Form(FIND_SYMBOLS_ONE_FORM),
        ),
        call(
            "node limit",
            "find_symbols",
            json!({"node": "TENET-WIDTH", "limit": 3}),
            Form(FIND_SYMBOLS_LIMIT),
        ),
        call("refs", "refs", json!({"id": "TENET-WIDTH"}), Answered),
        call("refs ron", "refs", json!({"id": "SLAB-INK"}), Answered),
        call("refs broken", "refs", json!({"id": "QUILL-02"}), Answered),
        call("refs nothing", "refs", json!({"id": "QUILL-99"}), NotFound),
        call(
            "refs look-alike",
            "refs",
            json!({"id": "QU\u{0406}LL-01"}),
            CannotRun,
        ),
        call("unmapped", "unmapped", json!({}), Answered),
        call("unmapped null", "unmapped", json!({"path": null}), Answered),
        call(
            "unmapped dir",
            "unmapped",
            json!({"path": "crates/inkwell/src"}),
            Answered,
        ),
        call(
            "unmapped file",
            "unmapped",
            json!({"path": twins}),
            Answered,
        ),
        call(
            "unmapped blank",
            "unmapped",
            json!({"path": "crates/inkwell/src/blank"}),
            Answered,
        ),
        call(
            "unmapped root",
            "unmapped",
            json!({"path": "data"}),
            Answered,
        ),
        call(
            "unmapped outside",
            "unmapped",
            json!({"path": "docs"}),
            CannotRun,
        ),
    ]
}

/// The symbol count of a `find_symbols` document.
fn symbol_count(document: &Value) -> usize {
    document["symbols"].as_array().map_or(0, Vec::len)
}

/// Every code file of code-bevy and its call label.
const BEVY_FILES: [(&str, &str); 9] = [
    ("file ron", "assets/farm.ron"),
    ("file main", "crates/farm-app/src/main.rs"),
    ("file clock", "crates/farm/src/clock.rs"),
    ("file crops", "crates/farm/src/crops.rs"),
    ("file fields", "crates/farm/src/fields.rs"),
    ("file herd", "crates/farm/src/herd.rs"),
    ("file lib", "crates/farm/src/lib.rs"),
    ("file news", "crates/farm/src/news.rs"),
    ("file weather", "crates/farm/src/weather.rs"),
];

/// The calls of the code-bevy parity run (docs/features/bevy-roles-reads.md
/// RD-10): every `find_symbols` form over every code file, a module and a
/// RON target (`CHORE-02`), exits 1 and 2, `refs` and `unmapped` beside.
fn bevy_reads() -> Vec<Call> {
    use Want::{Answered, CannotRun, NotFound};
    let mut calls = vec![
        call("tick", "find_symbols", json!({"query": "tick"}), Answered),
        call("run", "find_symbols", json!({"query": "run"}), Answered),
        call(
            "run limit",
            "find_symbols",
            json!({"query": "run", "limit": 1}),
            Answered,
        ),
        call("graze", "find_symbols", json!({"query": "graze"}), Answered),
        call("Quake", "find_symbols", json!({"query": "Quake"}), Answered),
        call(
            "node ALM-GROWTH",
            "find_symbols",
            json!({"node": "ALM-GROWTH"}),
            Answered,
        ),
        call(
            "node CHORE-01",
            "find_symbols",
            json!({"node": "CHORE-01"}),
            Answered,
        ),
        call(
            "node CHORE-02",
            "find_symbols",
            json!({"node": "CHORE-02"}),
            Answered,
        ),
        call(
            "node nothing",
            "find_symbols",
            json!({"node": "CHORE-99"}),
            NotFound,
        ),
        call(
            "at lib",
            "find_symbols",
            json!({"at": "crates/farm/src/lib.rs:30"}),
            Answered,
        ),
        call(
            "at herd",
            "find_symbols",
            json!({"at": "crates/farm/src/herd.rs:32"}),
            Answered,
        ),
        call(
            "file md",
            "find_symbols",
            json!({"file": "docs/spec/almanac.md"}),
            CannotRun,
        ),
        call("refs", "refs", json!({"id": "ALM-GROWTH"}), Answered),
        call("unmapped", "unmapped", json!({}), Answered),
    ];
    for (label, path) in BEVY_FILES {
        calls.push(call(label, "find_symbols", json!({"file": path}), Answered));
    }
    calls
}

/// RD-3 (Q33): every code tool and exit over a code-index copy, both
/// eras: `content` the CLI's text, `structuredContent` its document (kept
/// at exit 1 with its `reason`, absent at exit 2), each walked against the
/// `outputSchema`; and what the fixture fixes (6a-2's committed goldens:
/// `TENET-WIDTH` has six markers; two `separator` and two `run` twins, two
/// `joined` across files; reading L19: one target per binding of a node).
/// The forms echo their `form`; `tier` is set in the QUERY form only
/// (Q35). M: an MCP-side sort or cut; exit 1 as success.
///
/// docs/features/bevy-roles-reads.md RD-10 ("find_symbols"; "Rules and
/// edge cases": `content` and `structuredContent` the CLI's, roles
/// included). M: `roles` unmirrored (the walker meets a key the
/// `outputSchema` does not declare). Code-index lists no `[code]
/// frameworks`: every target holds `roles: null`. Over a scratch
/// code-bevy (the opt-in on, a module and a RON target written in) every
/// answer is the CLI's and walked; every target holds a `roles` list,
/// `[]` on the module and the RON path; the walker meets each role key
/// that can be `null` both with a value and `null`, every state, more
/// than one origin.
#[test]
fn code_tools_equal_the_cli() {
    let project = Project::reads();
    let calls = reads();
    for results in project.answers(&calls) {
        for (call, result) in calls.iter().zip(&results) {
            if call.tool == "find_symbols" {
                for roles in role_lists(result) {
                    assert_eq!(
                        roles,
                        Value::Null,
                        "{}: `roles` null without the opt-in",
                        call.label
                    );
                }
            }
        }
        let doc = |label: &str| document(&calls, &results, label).clone();
        assert_eq!(symbol_count(&doc("twins")), 2, "{}", doc("twins"));
        assert_eq!(doc("twins")["total"], json!(2), "{}", doc("twins"));
        // One positional, `#2` kept: its suffix first, then `#1` by name
        // (6a-3 phase A's `suffix_then_name`).
        let quoted = doc("quoted #2");
        assert_eq!(symbol_count(&quoted), 2, "{quoted}");
        assert_eq!(
            quoted["symbols"][0]["qpath"],
            json!("crates/inkwell::twins::impl Sheet #2"),
            "{quoted}"
        );
        assert_eq!(
            quoted["symbols"][1]["qpath"],
            json!("crates/inkwell::twins::impl Sheet #1"),
            "{quoted}"
        );
        assert_eq!(symbol_count(&doc("limit")), 1, "{}", doc("limit"));
        assert_eq!(doc("limit")["total"], json!(2), "{}", doc("limit"));
        let joined: BTreeSet<&str> = doc("cross-file")["symbols"]
            .as_array()
            .expect("symbols")
            .iter()
            .filter_map(|symbol| symbol["path"].as_str())
            .map(|path| match path {
                "crates/inkwell/src/split.rs" => "split.rs",
                "crates/inkwell/src/split/mod.rs" => "split/mod.rs",
                _ => "elsewhere",
            })
            .collect();
        assert_eq!(joined, BTreeSet::from(["split.rs", "split/mod.rs"]));
        assert_eq!(symbol_count(&doc("no rows")), 0);
        assert_eq!(doc("no rows")["total"], json!(0));
        for (label, form) in [
            ("twins", "query"),
            ("nulls", "query"),
            ("node", "node"),
            ("file", "file"),
            ("at", "at"),
        ] {
            let found = doc(label);
            assert_eq!(found["form"], json!(form), "{label}: {found}");
            for symbol in found["symbols"].as_array().expect("symbols") {
                assert_eq!(
                    symbol["tier"].is_string(),
                    form == "query",
                    "{label}: tier set in the QUERY form only (Q35): {symbol}"
                );
            }
        }
        assert_eq!(symbol_count(&doc("node")), 6, "{}", doc("node"));
        let nothing = doc("node nothing");
        assert_eq!(nothing["symbols"], json!([]), "{nothing}");
        let refs = doc("refs");
        assert_eq!(refs["nodes"][0]["id"], json!("TENET-WIDTH"), "{refs}");
        assert_eq!(
            refs["nodes"][0]["markers"].as_array().map(Vec::len),
            Some(6),
            "{refs}"
        );
        let gone = doc("refs nothing");
        assert_eq!(gone["nodes"], json!([]), "{gone}");
        assert_eq!(gone["omitted"], Value::Null, "{gone}");
        let blank = doc("unmapped blank");
        let rows = blank["rows"].as_array().expect("rows");
        assert_eq!(rows.len(), 1, "{blank}");
        assert_eq!(rows[0]["units"], json!(0), "{blank}");
        assert_eq!(rows[0]["percent"], Value::Null, "`-` is null: {blank}");
        assert_eq!(blank["total"]["percent"], Value::Null, "{blank}");
        let dir = doc("unmapped dir");
        let rows = dir["rows"].as_array().expect("rows");
        assert!(
            rows.iter().any(|row| row["percent"].is_number()),
            "a number beside the `-`: {dir}"
        );
        assert!(
            rows.iter().any(|row| row["percent"].is_null()),
            "the blank directory's `-`: {dir}"
        );
    }

    let project = Project::bevy();
    let calls = bevy_reads();
    for results in project.answers(&calls) {
        let mut values: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
        let mut kinds = BTreeSet::new();
        for (call, result) in calls.iter().zip(&results) {
            if call.tool != "find_symbols" {
                continue;
            }
            let document = &result["structuredContent"];
            for target in document["symbols"].as_array().into_iter().flatten() {
                let kind = target["target"].as_str().unwrap_or_default();
                kinds.insert(kind.to_owned());
                let roles = target["roles"].as_array().unwrap_or_else(|| {
                    panic!("{}: `roles` a list with the opt-in: {target}", call.label)
                });
                if kind != "item" {
                    assert!(roles.is_empty(), "{}: {target}", call.label);
                }
                for role in roles {
                    for key in ROLE_KEYS {
                        values.entry(key).or_default().insert(role[key].to_string());
                    }
                }
            }
        }
        assert_eq!(kinds, set(&["item", "module", "ron"]), "every target kind");
        for key in ROLE_NULLABLE {
            let seen = &values[key];
            assert!(
                seen.contains("null") && seen.len() > 1,
                "`{key}`: a value and `null` both walked: {seen:?}"
            );
        }
        assert_eq!(
            values["state"],
            set(&[
                "\"ambiguous\"",
                "\"certain\"",
                "\"linked\"",
                "\"uncertain\""
            ]),
            "every state walked"
        );
        assert!(values["origin"].len() > 1, "{:?}", values["origin"]);
        let chore = document(&calls, &results, "node CHORE-02");
        assert_eq!(symbol_count(chore), 2, "{chore}");
    }
}

// ------------------------------------------------------------------ RD-4

/// The output schemas of the code tools, from a fresh server.
fn code_schemas() -> (Value, Value, Value) {
    let mut session = Session::open(Era::Legacy, &[], None, Home::Fresh);
    let list = session.tools();
    let schemas = output_schemas(&list);
    drop(session.finish());
    (
        schemas["find_symbols"].clone(),
        schemas["refs"].clone(),
        schemas["unmapped"].clone(),
    )
}

/// RD-4 ("Output mirrors"): the CLI's closed words are enums (`form`,
/// `target`, `tier`, `relation`, `id_state`, `rev_state`, `levels_state`,
/// a binding's `kind`, `ambiguity`); `item_kind`, `layer`, `category`,
/// `levels` items and node kinds are free strings, never an `enum` (10a's
/// role keys join free). `form` is exactly the four forms, `target` Q35's
/// `item`, `module`, `ron`, `tier` the three tiers or `null`. M:
/// `item_kind` an enum.
///
/// docs/features/bevy-roles-reads.md RD-10 ("find_symbols"): `roles` is
/// a nullable array; each of its keys a free string or integer, never an
/// enum of words (as `item_kind`, `layer`): `line` and `candidates`
/// integers, the nine others strings. M: a role word (`role`, `source`,
/// `form`, `origin`, `state` …) an enum.
#[test]
fn code_mirrors_keep_kinds_free() {
    let (symbols, refs, unmapped) = code_schemas();
    let free = |schema: &Value, path: &str| {
        let at = schema_at(schema, path);
        assert_eq!(enum_values(at), None, "{path}: an enum: {at}");
        assert!(
            types_of(at).contains("string"),
            "{path}: not a string: {at}"
        );
    };
    let closed = |schema: &Value, path: &str| {
        let at = schema_at(schema, path);
        assert!(
            enum_values(at).is_some_and(|values| !values.is_empty()),
            "{path}: not the CLI's closed set: {at}"
        );
    };
    for path in [
        "symbols[].item_kind",
        "symbols[].layer",
        "symbols[].bindings[].levels[]",
    ] {
        free(&symbols, path);
    }
    for path in [
        "form",
        "symbols[].target",
        "symbols[].tier",
        "symbols[].ambiguity",
        "symbols[].bindings[].relation",
        "symbols[].bindings[].id_state",
        "symbols[].bindings[].rev_state",
        "symbols[].bindings[].levels_state",
    ] {
        closed(&symbols, path);
    }
    for path in [
        "nodes[].kind",
        "nodes[].markers[].levels[]",
        "nodes[].markers[].binding.item_kind",
        "nodes[].markers[].binding.category",
        "nodes[].mentions[].binding.item_kind",
    ] {
        free(&refs, path);
    }
    for path in [
        "nodes[].markers[].relation",
        "nodes[].markers[].id_state",
        "nodes[].markers[].rev_state",
        "nodes[].markers[].levels_state",
        "nodes[].markers[].binding.kind",
        "nodes[].markers[].binding.ambiguity",
        "nodes[].mentions[].id_state",
        "nodes[].mentions[].binding.kind",
    ] {
        closed(&refs, path);
    }
    free(&unmapped, "unmapped[].item_kind");
    assert_eq!(
        enum_values(schema_at(&symbols, "form")),
        Some(set(&["at", "file", "node", "query"]))
    );
    assert_eq!(
        enum_values(schema_at(&symbols, "symbols[].target")),
        Some(set(&["item", "module", "ron"]))
    );
    let tier = schema_at(&symbols, "symbols[].tier");
    assert_eq!(enum_values(tier), Some(set(&["exact", "name", "suffix"])));
    assert!(
        types_of(tier).contains("null"),
        "tier null outside QUERY: {tier}"
    );

    let roles = schema_at(&symbols, "symbols[].roles");
    assert_eq!(enum_values(roles), None, "{roles}");
    assert!(
        types_of(roles).is_superset(&set(&["array", "null"])),
        "`roles` a nullable array: {roles}"
    );
    for key in ROLE_KEYS {
        let path = format!("symbols[].roles[].{key}");
        let at = schema_at(&symbols, &path);
        assert_eq!(enum_values(at), None, "{path}: an enum: {at}");
        let want = if ROLE_INTEGERS.contains(&key) {
            "integer"
        } else {
            "string"
        };
        assert!(
            types_of(at).contains(want),
            "{path}: not a free {want}: {at}"
        );
    }
}

/// RD-4 (Q35): the documents' key sets are the CLI's, named here (a key
/// dropped from both the mirror and the CLI still fails); `percent` a
/// number or `null`, never a string; `total` a row object; `omitted`
/// `{symbols}`, `{rows, unmapped}`, refs' `{markers, mentions}`; `reason`
/// a string or `null`. M: `percent` a string (the walker over a `-` row
/// in `code_tools_equal_the_cli` and the types here); a key dropped.
/// docs/features/bevy-roles-reads.md "find_symbols": a target gains
/// `roles`, each item exactly [`ROLE_KEYS`]; `form`, `group`, `detail`,
/// `candidates` admit `null`. M: a role key dropped.
#[test]
fn code_mirrors_pin_the_q35_types() {
    let (symbols, refs, unmapped) = code_schemas();
    assert_eq!(
        keys_at(&symbols),
        set(&[
            "form", "query", "node", "path", "line", "reason", "notes", "symbols", "total",
            "omitted", "code"
        ])
    );
    assert_eq!(
        keys_at(schema_at(&symbols, "symbols[]")),
        set(&[
            "target",
            "qpath",
            "name",
            "discriminator",
            "item_kind",
            "path",
            "start_line",
            "end_line",
            "signature",
            "ambiguity",
            "layer",
            "tier",
            "mentions",
            "bindings",
            "roles"
        ])
    );
    assert_eq!(
        keys_at(schema_at(&symbols, "symbols[].roles[]")),
        set(&ROLE_KEYS)
    );
    for key in ROLE_NULLABLE {
        let path = format!("symbols[].roles[].{key}");
        let at = schema_at(&symbols, &path);
        assert!(
            types_of(at).contains("null"),
            "{path}: `null` on most rows: {at}"
        );
    }
    assert_eq!(
        keys_at(schema_at(&symbols, "symbols[].bindings[]")),
        set(&[
            "relation",
            "line",
            "column",
            "token",
            "id",
            "rev",
            "id_state",
            "fix",
            "rev_state",
            "levels",
            "levels_state",
            "note",
            "node"
        ])
    );
    assert_eq!(keys_at(schema_at(&symbols, "omitted")), set(&["symbols"]));
    assert_eq!(
        types_of(schema_at(&symbols, "reason")),
        set(&["null", "string"])
    );
    assert_eq!(
        keys_at(&unmapped),
        set(&[
            "path", "rows", "total", "unmapped", "notes", "omitted", "code"
        ])
    );
    let row = set(&["path", "dir", "units", "mapped", "percent", "mentioned"]);
    assert_eq!(keys_at(schema_at(&unmapped, "rows[]")), row);
    assert_eq!(keys_at(schema_at(&unmapped, "total")), row, "a row object");
    assert!(types_of(schema_at(&unmapped, "total")).contains("object"));
    for path in ["rows[].percent", "total.percent"] {
        assert_eq!(
            types_of(schema_at(&unmapped, path)),
            set(&["null", "number"]),
            "{path}: a number, null for `-` (Q35)"
        );
    }
    assert_eq!(
        keys_at(schema_at(&unmapped, "unmapped[]")),
        set(&["qpath", "item_kind", "path", "line"])
    );
    assert_eq!(
        keys_at(schema_at(&unmapped, "omitted")),
        set(&["rows", "unmapped"])
    );
    assert_eq!(
        keys_at(&refs),
        set(&["ref", "reason", "notes", "nodes", "code", "omitted"])
    );
    assert_eq!(
        keys_at(schema_at(&refs, "nodes[]")),
        set(&[
            "id",
            "kind",
            "title",
            "path",
            "line",
            "rev",
            "rev_written",
            "markers",
            "mentions"
        ])
    );
    assert_eq!(
        keys_at(schema_at(&refs, "nodes[].markers[]")),
        set(&[
            "relation",
            "path",
            "line",
            "column",
            "token",
            "id",
            "rev",
            "id_state",
            "fix",
            "rev_state",
            "levels",
            "levels_state",
            "note",
            "binding"
        ])
    );
    assert_eq!(
        keys_at(schema_at(&refs, "nodes[].mentions[]")),
        set(&[
            "path", "line", "column", "written", "id_state", "fix", "binding"
        ])
    );
    let binding = set(&[
        "kind",
        "qpath",
        "item_kind",
        "line",
        "ambiguity",
        "category",
    ]);
    assert_eq!(
        keys_at(schema_at(&refs, "nodes[].markers[].binding")),
        binding
    );
    assert_eq!(
        keys_at(schema_at(&refs, "nodes[].mentions[].binding")),
        binding
    );
    assert_eq!(
        keys_at(schema_at(&refs, "code")),
        set(&["files", "rs", "ron", "skipped"])
    );
    assert_eq!(
        keys_at(schema_at(&refs, "omitted")),
        set(&["markers", "mentions"])
    );
    assert_eq!(
        types_of(schema_at(&refs, "reason")),
        set(&["null", "string"])
    );
}

// ------------------------------------------------------------------ RD-5

/// `INSTRUCTIONS` as docs/features/code-tools.md "INSTRUCTIONS" plans them
/// (Q31): 4c-1's text with the reads in two lines, the second the code
/// tools (`spec ` and two glosses dropped; the REF paragraph keeps `with
/// ["links"]`), and `Hints use flags:`; 1 892 bytes.
const PLANNED_INSTRUCTIONS: &str = concat!(
    "SpecEngine keeps a project's spec as Markdown in git beside the code. Each tool is a ",
    "spec command: content its `2>&1` output, structuredContent its --json document.\n",
    "\n",
    "Reads:\n",
    "- get_tree(root?, depth?, kinds?, archive?) = tree; get_node(id, with?, archive?) = ",
    "show; search(query, kinds?, limit?, archive?) = search; get_context_bundle(node_ids, ",
    "budget?) = bundle; get_proposal(proposal_id) = review --brief.\n",
    "- Code: find_symbols(query|node|file|at, limit?) = symbols; refs(id) = refs; ",
    "unmapped(path?) = unmapped.\n",
    "Queue (writes only the queue; the owner decides on a terminal):\n",
    "- propose_change = spec propose update|create|interpretation|amendment: a node's new ",
    "text against its span_hash; create: new ID sections in it, or a new file (base null).\n",
    "- ask_question, report_discrepancy, report_decision = spec propose ",
    "question|discrepancy|decision: hits are what is decided or asked; stored only if ",
    "distinct_from names each; work on your working answer.\n",
    "- report_drift = spec propose drift: queues HEAD's lock drift for the owner; evidence ",
    "computed, never typed.\n",
    "- get_task, claim_task, submit_plan, report_run, complete_task = spec task ",
    "show|claim|plan|report|complete; the three above take task_id.\n",
    "\n",
    "REF: an ID, alias, slug/ID, ID#SECTION or root-relative .md path. Hints use flags: ",
    "--kind is kinds, ROOT is root, --links is with [\"links\"]. Answers are cut at 40000 ",
    "characters, with a tail on narrowing. A REF naming nothing or a call that cannot run ",
    "(no specengine.toml: `spec init`) is an error result with the reason. Resources: ",
    "spec://<slug>/tree, spec://<slug>/node/<REF, percent-encoded>.\n",
    "\n",
    "Rules:\n",
    "- Deterministic: one state, one result; no LLM inside.\n",
    "- Reads refresh SpecEngine's index outside the project root; no tool writes under it. ",
    "Spec files change only when the owner approves.\n",
    "- Nothing is blocked by a discrepancy.\n",
    "- IDs are Latin only; a look-alike is refused with its Latin fix.",
);

/// The code tools' line, its LF included (104 + 1 bytes).
const CODE_LINE: &str = "- Code: find_symbols(query|node|file|at, limit?) = symbols; refs(id) = refs; \
                         unmapped(path?) = unmapped.\n";

/// task-package's tasks line, its LF included.
const TASKS_LINE: &str = "- get_task, claim_task, submit_plan, report_run, complete_task = spec \
                          task show|claim|plan|report|complete; the three above take task_id.\n";

/// The `probes` build's paragraph after code-tools (76 bytes; its pin:
/// `mcp_create.rs`).
const PROBES_PARAGRAPH: &str =
    "\n\nProbes build: review_proposal, probe_output, probe_sleep (owner's checks).";

/// `blake3_hex(PLANNED_INSTRUCTIONS)`: the hash of the spec's text built
/// here, never of the server's; `mcp_path.rs` and `mcp_decision.rs` pin
/// the same value against `INSTRUCTIONS`.
const PLANNED_BLAKE3: &str = "3d172b74534282e8ad7fa14eb29f5e2b51e09f96970babe3145173c51fd1ad9c";

/// The BLAKE3 of [`PLANNED_INSTRUCTIONS`] less [`TASKS_LINE`].
const PLANNED_LESS_TASKS_BLAKE3: &str =
    "42f9ecfda69898a7615608e5a09789da3b2c2ac25d178b6461b70ddfd33f1133";

/// RD-5 (Q31): `INSTRUCTIONS` are the planned text byte for byte: 1 892
/// bytes, 1 754 less the tasks line (the spec's 1 755 is off by one: D1),
/// 1 968 with the probes paragraph, 80 left of 2 048; the code line once,
/// under "Reads:", right after the reads' line; `Hints use flags:`; every
/// tool named. M: a tool unnamed; the text changed, a pin kept.
#[test]
fn rd5_instructions_are_the_planned_text() {
    let instructions = specengine_mcp::INSTRUCTIONS;
    assert_eq!(PLANNED_INSTRUCTIONS.len(), 1892, "the spec's figure");
    assert_eq!(CODE_LINE.len(), 105);
    assert_eq!(TASKS_LINE.len(), 138);
    assert_eq!(PROBES_PARAGRAPH.len(), 76);
    assert_eq!(instructions, PLANNED_INSTRUCTIONS);
    assert_eq!(instructions.len(), 1892);
    assert!(instructions.is_ascii());
    assert_eq!(instructions.matches(CODE_LINE).count(), 1);
    assert!(
        instructions.contains(&format!("review --brief.\n{CODE_LINE}Queue (")),
        "the code line closes the reads"
    );
    assert!(instructions.contains("Hints use flags: --kind is kinds"));
    assert!(!instructions.contains("CLI flags"));
    assert!(instructions.contains("--links is with [\"links\"]"));
    for name in TOOLS {
        assert!(has_word(instructions, name), "{name} unnamed");
    }
    for name in CODE_TOOLS {
        assert!(instructions.contains(&format!("{name}(")), "{name}(");
    }
    let less = instructions.replacen(TASKS_LINE, "", 1);
    assert_eq!(less.len(), 1754, "1 892 less 138 (the spec says 1 755)");
    let with_probes = instructions.len() + PROBES_PARAGRAPH.len();
    assert_eq!(with_probes, 1968);
    assert_eq!(2048 - with_probes, 80, "the room Q31 states");
}

/// RD-5: the BLAKE3 pins of the planned text, computed over the spec's
/// text built here; `mcp_path.rs` and `mcp_decision.rs` pin the same
/// values against the server's text.
#[test]
fn rd5_planned_instructions_hashes() {
    assert_eq!(blake3_hex(PLANNED_INSTRUCTIONS.as_bytes()), PLANNED_BLAKE3);
    assert_eq!(
        blake3_hex(PLANNED_INSTRUCTIONS.replacen(TASKS_LINE, "", 1).as_bytes()),
        PLANNED_LESS_TASKS_BLAKE3
    );
}

// ------------------------------------------------------------------ TE-1

/// TE-1, exits 2 beyond `code_tools_equal_the_cli`: an unreadable code file
/// is a walk gap, every tool and form the CLI's `spec: the code walk is
/// incomplete: <gap>` naming it, no document; readable again, the next call
/// answers (no state kept); a refused `[code]` key (`languages`, "is not
/// built") is the CLI's config line for every tool.
#[test]
fn te1_a_walk_gap_and_a_bad_config_are_the_clis_exit_2() {
    let gap = "crates/inkwell/src/page.rs";
    let forms = [
        ("find_symbols", json!({"query": "run"})),
        ("find_symbols", json!({"node": "TENET-WIDTH"})),
        (
            "find_symbols",
            json!({"file": "crates/inkwell/src/twins.rs"}),
        ),
        (
            "find_symbols",
            json!({"at": "crates/inkwell/src/lib.rs:19"}),
        ),
        ("refs", json!({"id": "TENET-WIDTH"})),
        ("unmapped", json!({})),
        ("unmapped", json!({"path": "crates/inkwell/src"})),
    ];
    for era in ERAS {
        let project = Project::new(FIXTURE);
        let env = project.env();
        let globals = Globals::default();
        let (mut session, schemas) = project.open(era);
        let path = project.root.join(gap);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).expect("chmod");
        for (tool, args) in &forms {
            let result = checked_call(&mut session, &schemas, tool, args.clone(), &env, &globals);
            let context = format!("{era:?} {tool} {args}");
            Want::CannotRun.check(&result, &context);
            let text = content_text(&result);
            assert!(
                text.starts_with("spec: the code walk is incomplete: ") && text.contains(gap),
                "{context}: {text:?}"
            );
        }
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).expect("chmod");
        let result = checked_call(
            &mut session,
            &schemas,
            "find_symbols",
            json!({"query": "wide"}),
            &env,
            &globals,
        );
        Want::Answered.check(&result, &format!("{era:?}: healed"));
        let done = session.finish();
        assert!(done.status.success(), "{era:?}: {}", done.stderr);
        assert_eq!(done.stderr, "", "{era:?}");
    }
    let project = Project::new(FIXTURE);
    replace(
        &project.root,
        "specengine.toml",
        "roots = [\"crates\", \"data\"]",
        "roots = [\"crates\", \"data\"]\nlanguages = [\"rust\"]",
    );
    let calls: Vec<Call> = forms
        .iter()
        .map(|(tool, args)| call("config", tool, args.clone(), Want::CannotRun))
        .collect();
    for results in project.answers(&calls) {
        for result in &results {
            let text = content_text(result);
            assert!(
                text.contains("specengine.toml") && text.contains("languages"),
                "the key's line: {text:?}"
            );
        }
    }
}

/// `relative` files under `root` (recursively) whose extension is one of
/// `extensions`.
fn files_with(root: &Path, extensions: &[&str]) -> Vec<String> {
    fn walk(root: &Path, dir: &Path, extensions: &[&str], out: &mut Vec<String>) {
        let mut entries: Vec<_> = fs::read_dir(dir)
            .expect("a directory")
            .map(|entry| entry.expect("an entry").path())
            .collect();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                walk(root, &path, extensions, out);
            } else if path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| extensions.contains(&ext))
            {
                let relative = path.strip_prefix(root).expect("under root");
                out.push(relative.to_str().expect("UTF-8").to_owned());
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, extensions, &mut out);
    out
}

/// The requests of one determinism session: `tools/list`, then every
/// form, exit and tool.
fn determinism_requests() -> Vec<(String, Value)> {
    let call = |name: &str, arguments: Value| {
        (
            "tools/call".to_owned(),
            json!({"name": name, "arguments": arguments}),
        )
    };
    let mut list = vec![("tools/list".to_owned(), json!({}))];
    for args in [
        json!({"query": "separator"}),
        json!({"query": "impl Sheet #2"}),
        json!({"query": "run", "limit": 1}),
        json!({"query": "joined"}),
        json!({"node": "TENET-WIDTH"}),
        json!({"node": "SLAB-SHEET"}),
        json!({"node": "SLAB-INK"}),
        json!({"file": "crates/inkwell/src/twins.rs"}),
        json!({"file": "data/inks.ron"}),
        json!({"at": "crates/inkwell/src/lib.rs:19"}),
        json!({"node": "QUILL-99"}),
        json!({"file": "docs/spec/ink.md"}),
        json!({}),
    ] {
        list.push(call("find_symbols", args));
    }
    for id in [
        "TENET-WIDTH",
        "SLAB-INK",
        "SLAB-SHEET",
        "QUILL-02",
        "QUILL-99",
    ] {
        list.push(call("refs", json!({"id": id})));
    }
    for args in [
        json!({}),
        json!({"path": "crates/inkwell/src"}),
        json!({"path": "crates/inkwell/src/twins.rs"}),
        json!({"path": "data"}),
    ] {
        list.push(call("unmapped", args));
    }
    list
}

/// Every reply line of one session (`args`, `cwd`, `home`).
fn session_lines(era: Era, args: &[&str], cwd: &Path, home: &Path) -> Vec<String> {
    lines_of_requests(era, args, cwd, home, determinism_requests())
}

/// Every reply line of one session (`args`, `cwd`, `home`) sending
/// `requests` in order.
fn lines_of_requests(
    era: Era,
    args: &[&str],
    cwd: &Path,
    home: &Path,
    requests: Vec<(String, Value)>,
) -> Vec<String> {
    let mut session = Session::open(era, args, Some(cwd), Home::At(home));
    let lines = requests
        .into_iter()
        .map(|(method, params)| session.request_line(&method, params))
        .collect();
    let done = session.finish();
    assert!(done.status.success(), "{era:?}: {}", done.stderr);
    assert_eq!(done.stderr, "", "{era:?}");
    lines
}

/// The requests of one code-bevy determinism session
/// (docs/features/bevy-roles-reads.md TE-3): `tools/list`, then
/// `find_symbols` in every form over every code file, a module and a RON
/// target, exits 1 and 2.
fn role_requests() -> Vec<(String, Value)> {
    let mut list = vec![("tools/list".to_owned(), json!({}))];
    let mut forms = vec![
        json!({"query": "tick"}),
        json!({"query": "run"}),
        json!({"query": "run", "limit": 1}),
        json!({"query": "graze"}),
        json!({"query": "Harvest"}),
        json!({"query": "Quake"}),
        json!({"node": "ALM-GROWTH"}),
        json!({"node": "CHORE-01"}),
        json!({"node": "CHORE-02"}),
        json!({"node": "CHORE-99"}),
        json!({"at": "crates/farm/src/lib.rs:30"}),
        json!({"at": "crates/farm/src/herd.rs:32"}),
        json!({"file": "docs/spec/almanac.md"}),
    ];
    forms.extend(BEVY_FILES.iter().map(|(_, path)| json!({"file": path})));
    for arguments in forms {
        list.push((
            "tools/call".to_owned(),
            json!({"name": "find_symbols", "arguments": arguments}),
        ));
    }
    list
}

/// Today's date and its neighbours, `YYYY-MM-DD` (any time zone).
fn dates_around_now() -> Vec<String> {
    let days = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after 1970")
        .as_secs() as i64
        / 86_400;
    (days - 1..=days + 1)
        .map(|days| {
            let z = days + 719_468;
            let era = z.div_euclid(146_097);
            let doe = z.rem_euclid(146_097);
            let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
            let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
            let mp = (5 * doy + 2) / 153;
            let day = doy - (153 * mp + 2) / 5 + 1;
            let month = if mp < 10 { mp + 3 } else { mp - 9 };
            let year = yoe + era * 400 + i64::from(month <= 2);
            format!("{year:04}-{month:02}-{day:02}")
        })
        .collect()
}

/// TE-1, determinism ("Rules and edge cases"): a forward copy, a copy
/// created in reverse order at another root, a CRLF copy (every `.rs`,
/// `.ron`, `.md`) and a copy read with `--root` from another working
/// directory, each with its own `HOME`, give byte-identical reply lines
/// (the same request ids) for every form, exit and tool in both eras; a
/// repeated session the same bytes again; no line holds a root, a `HOME`,
/// a scratch name or today's date. M: the absolute root in a document; a
/// rowid order.
#[test]
fn te1_code_tools_are_byte_identical_across_roots_order_cwd_and_crlf() {
    let one = Scratch::new("cdet-one");
    let two = Scratch::new("cdet-two-with-a-longer-name");
    let forward = one.join("cdet-first-root");
    copy_dir(&fixture(FIXTURE), &forward, false);
    let reverse = two.join("nested/cdet-second-root");
    copy_dir(&fixture(FIXTURE), &reverse, true);
    let crlf = one.join("cdet-crlf-root");
    copy_dir(&fixture(FIXTURE), &crlf, false);
    for relative in files_with(&crlf, &["rs", "ron", "md"]) {
        let text = read_text(&crlf, &relative);
        write(
            &crlf,
            &relative,
            text.replace("\r\n", "\n").replace('\n', "\r\n"),
        );
    }
    let fourth = two.join("cdet-fourth-root");
    copy_dir(&fixture(FIXTURE), &fourth, true);
    let elsewhere = two.dir("cdet-elsewhere");
    let [forward, reverse, crlf, fourth] =
        [forward, reverse, crlf, fourth].map(|root| root.canonicalize().expect("a root"));
    let homes = [
        one.home("cdet-home-one"),
        two.home("cdet-home-two"),
        one.home("cdet-home-crlf"),
        two.home("cdet-home-four"),
    ];
    let fourth_text = fourth.to_str().expect("UTF-8").to_owned();
    let dates = dates_around_now();
    for era in ERAS {
        let first = session_lines(era, &[], &forward, &homes[0]);
        let variants = [
            ("reverse", session_lines(era, &[], &reverse, &homes[1])),
            ("crlf", session_lines(era, &[], &crlf, &homes[2])),
            (
                "--root from elsewhere",
                session_lines(
                    era,
                    &["--root", fourth_text.as_str()],
                    &elsewhere,
                    &homes[3],
                ),
            ),
            ("again", session_lines(era, &[], &forward, &homes[0])),
        ];
        for (label, lines) in &variants {
            assert_eq!(first.len(), lines.len(), "{era:?} {label}");
            for (index, (left, right)) in first.iter().zip(lines).enumerate() {
                assert_eq!(left, right, "{era:?} {label}: reply {index} differs");
            }
        }
        let forbidden: Vec<String> = [&forward, &reverse, &crlf, &fourth, &elsewhere]
            .iter()
            .map(|path| path.to_str().expect("UTF-8").to_owned())
            .chain(
                homes
                    .iter()
                    .map(|home| home.to_str().expect("UTF-8").to_owned()),
            )
            .chain(["cdet-".to_owned()])
            .chain(dates.iter().cloned())
            .collect();
        for line in &first {
            for text in &forbidden {
                assert!(
                    !line.contains(text.as_str()),
                    "{era:?}: a reply holds {text}: {}",
                    clip(line)
                );
            }
        }
    }
}

/// docs/features/bevy-roles-reads.md TE-3, the `find_symbols` half
/// ("Rules and edge cases", Determinism; the `symbols` half:
/// `bevy_roles_reads_determinism.rs`). M: a rowid order of the rows or of
/// their candidates; the absolute root in a document. code-bevy (the
/// opt-in on, `CHORE-02`'s module and RON markers written in): a forward
/// copy, a copy created in reverse order at another root, a CRLF copy
/// (every `.rs`, `.ron`, `.md`) and a copy read with `--root` from another
/// working directory, each with its own `HOME`, give byte-identical reply
/// lines for every request of [`role_requests`] in both eras, role lines
/// and `roles` included; a repeated session the same bytes again; no line
/// holds a root, a `HOME`, a scratch name or today's date.
#[test]
fn te3_find_symbols_roles_are_byte_identical_across_roots_order_cwd_and_crlf() {
    let one = Scratch::new("rdet-one");
    let two = Scratch::new("rdet-two-with-a-longer-name");
    let forward = one.join("rdet-first-root");
    copy_dir(&fixture(BEVY), &forward, false);
    let reverse = two.join("nested/rdet-second-root");
    copy_dir(&fixture(BEVY), &reverse, true);
    let crlf = one.join("rdet-crlf-root");
    copy_dir(&fixture(BEVY), &crlf, false);
    let fourth = two.join("rdet-fourth-root");
    copy_dir(&fixture(BEVY), &fourth, true);
    for root in [&forward, &reverse, &crlf, &fourth] {
        mark_bevy(root);
    }
    for relative in files_with(&crlf, &["rs", "ron", "md"]) {
        let text = read_text(&crlf, &relative);
        write(
            &crlf,
            &relative,
            text.replace("\r\n", "\n").replace('\n', "\r\n"),
        );
    }
    let elsewhere = two.dir("rdet-elsewhere");
    let [forward, reverse, crlf, fourth] =
        [forward, reverse, crlf, fourth].map(|root| root.canonicalize().expect("a root"));
    let homes = [
        one.home("rdet-home-one"),
        two.home("rdet-home-two"),
        one.home("rdet-home-crlf"),
        two.home("rdet-home-four"),
    ];
    let fourth_text = fourth.to_str().expect("UTF-8").to_owned();
    let dates = dates_around_now();
    for era in ERAS {
        let first = lines_of_requests(era, &[], &forward, &homes[0], role_requests());
        assert!(
            first.iter().any(|line| line.contains("\\n  role ")),
            "{era:?}: role lines in the content"
        );
        assert!(
            first.iter().any(|line| line.contains("\"roles\":[{")),
            "{era:?}: roles in the documents"
        );
        let variants = [
            (
                "reverse",
                lines_of_requests(era, &[], &reverse, &homes[1], role_requests()),
            ),
            (
                "crlf",
                lines_of_requests(era, &[], &crlf, &homes[2], role_requests()),
            ),
            (
                "--root from elsewhere",
                lines_of_requests(
                    era,
                    &["--root", fourth_text.as_str()],
                    &elsewhere,
                    &homes[3],
                    role_requests(),
                ),
            ),
            (
                "again",
                lines_of_requests(era, &[], &forward, &homes[0], role_requests()),
            ),
        ];
        for (label, lines) in &variants {
            assert_eq!(first.len(), lines.len(), "{era:?} {label}");
            for (index, (left, right)) in first.iter().zip(lines).enumerate() {
                assert_eq!(left, right, "{era:?} {label}: reply {index} differs");
            }
        }
        let forbidden: Vec<String> = [&forward, &reverse, &crlf, &fourth, &elsewhere]
            .iter()
            .map(|path| path.to_str().expect("UTF-8").to_owned())
            .chain(
                homes
                    .iter()
                    .map(|home| home.to_str().expect("UTF-8").to_owned()),
            )
            .chain(["rdet-".to_owned()])
            .chain(dates.iter().cloned())
            .collect();
        for line in &first {
            for text in &forbidden {
                assert!(
                    !line.contains(text.as_str()),
                    "{era:?}: a reply holds {text}: {}",
                    clip(line)
                );
            }
        }
    }
}

/// TE-1, session (AC-07's rule for the code tools): one session sees an
/// edited `.rs` in its next call without `spec index`: a new item and its
/// marker appear in `find_symbols`, `refs` and `unmapped`; the database
/// deleted, the next call answers alike (nothing kept between calls).
#[test]
fn te1_an_edited_rs_shows_in_the_next_call_without_spec_index() {
    for era in ERAS {
        let project = Project::new(FIXTURE);
        let env = project.env();
        let globals = Globals::default();
        let (mut session, schemas) = project.open(era);
        let mut ask = |tool: &str, args: Value| {
            let result = checked_call(&mut session, &schemas, tool, args, &env, &globals);
            Want::Answered.check(&result, &format!("{era:?} {tool}"));
            result["structuredContent"].clone()
        };
        let fresh = json!({"query": "fresh_mark"});
        let split = json!({"path": "crates/inkwell/src/split.rs"});
        assert_eq!(symbol_count(&ask("find_symbols", fresh.clone())), 0);
        let markers = |refs: &Value| refs["nodes"][0]["markers"].as_array().map_or(0, Vec::len);
        let before = markers(&ask("refs", json!({"id": "TENET-FLOW"})));
        let units_before = ask("unmapped", split.clone())["total"]["units"]
            .as_u64()
            .expect("units");

        let path = project.root.join("crates/inkwell/src/split.rs");
        let mut text = fs::read_to_string(&path).expect("split.rs");
        text.push_str("\n// @implements TENET-FLOW@1\npub fn fresh_mark() -> u32 {\n    3\n}\n");
        fs::write(&path, text).expect("an edit");

        let found = ask("find_symbols", fresh.clone());
        assert_eq!(symbol_count(&found), 1, "{era:?}: {found}");
        assert_eq!(
            found["symbols"][0]["path"],
            json!("crates/inkwell/src/split.rs")
        );
        assert_eq!(
            found["symbols"][0]["bindings"][0]["id"],
            json!("TENET-FLOW"),
            "{found}"
        );
        assert_eq!(
            markers(&ask("refs", json!({"id": "TENET-FLOW"}))),
            before + 1
        );
        assert_eq!(
            ask("unmapped", split.clone())["total"]["units"],
            json!(units_before + 1)
        );

        for entry in fs::read_dir(data_dir(&project.home)).expect("the data directory") {
            let path = entry.expect("an entry").path();
            if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("inkwell.db"))
            {
                fs::remove_file(&path).expect("the database removed");
            }
        }
        assert_eq!(symbol_count(&ask("find_symbols", fresh)), 1, "{era:?}");
        let done = session.finish();
        assert!(done.status.success(), "{era:?}: {}", done.stderr);
        assert_eq!(done.stderr, "", "{era:?}");
    }
}

/// TE-1, quiet (AC-19's rule for the code tools): over broken, empty,
/// BOM-led, look-alike-marked and non-UTF-8 code files every tool answers
/// the CLI's answer (its `note:` lines inside `content`), stdout carries
/// JSON-RPC only (the harness parses every line), stderr stays empty, the
/// process exits 0. M: a `println!` or the default panic hook on the path.
#[test]
fn te1_broken_and_non_utf8_code_leave_stdout_json_rpc_only() {
    let project = Project::new(FIXTURE);
    let root = &project.root;
    write(
        root,
        "crates/inkwell/src/raw.rs",
        b"pub fn raw() -> u32 {\n    1\n}\n// \xff\xfe not text\n".as_slice(),
    );
    write(root, "crates/inkwell/src/half.rs", "pub fn half( {\n");
    write(root, "crates/inkwell/src/empty.rs", "");
    write(
        root,
        "crates/inkwell/src/bom.rs",
        "\u{feff}pub fn bom() -> u32 {\n    2\n}\n",
    );
    write(
        root,
        "crates/inkwell/src/mixed.rs",
        "// @implements QU\u{0406}LL-01@1\npub fn mixed() -> u32 {\n    3\n}\n",
    );
    write(root, "data/odd.ron", "Odd(\n    width: ,\n");
    let parity = |tool: &'static str, args: Value| call("quiet", tool, args, Want::Parity);
    let calls = [
        parity("find_symbols", json!({"query": "raw"})),
        parity("find_symbols", json!({"file": "crates/inkwell/src/raw.rs"})),
        parity(
            "find_symbols",
            json!({"file": "crates/inkwell/src/half.rs"}),
        ),
        parity(
            "find_symbols",
            json!({"at": "crates/inkwell/src/half.rs:1"}),
        ),
        parity(
            "find_symbols",
            json!({"file": "crates/inkwell/src/empty.rs"}),
        ),
        parity("find_symbols", json!({"query": "bom"})),
        parity("find_symbols", json!({"query": "mixed"})),
        parity("find_symbols", json!({"file": "data/odd.ron"})),
        parity("refs", json!({"id": "QUILL-01"})),
        parity("refs", json!({"id": "QUILL-02"})),
        parity("unmapped", json!({})),
        parity("unmapped", json!({"path": "crates/inkwell/src"})),
    ];
    let env = project.env();
    let globals = Globals::default();
    for era in ERAS {
        let (mut session, schemas) = project.open(era);
        for call in &calls {
            checked_call(
                &mut session,
                &schemas,
                call.tool,
                call.args.clone(),
                &env,
                &globals,
            );
        }
        let done = session.finish();
        assert_eq!(done.status.code(), Some(0), "{era:?}: {}", done.stderr);
        assert!(done.messages.is_empty(), "{era:?}: {:?}", done.messages);
        assert_eq!(done.stderr, "", "{era:?}: stderr");
    }
}

/// TE-1, genre (`spec-c`, a Python service without `[code]`): every code
/// tool and form is an error result naming `[code]`: the CLI's no-table
/// line exactly for a query, a node, `refs` and `unmapped`; parity for a
/// file and a line too; no document.
#[test]
fn te1_spec_c_answers_each_code_tool_with_the_code_table_line() {
    let project = Project::new("spec-c");
    let exact = |tool: &'static str, args: Value| call("exact", tool, args, Want::CannotRun);
    let calls = [
        exact("find_symbols", json!({"query": "hire"})),
        exact("find_symbols", json!({"node": "AREA-HIRE"})),
        exact("refs", json!({"id": "AREA-HIRE"})),
        exact("refs", json!({"id": "FEE-RIDE"})),
        exact("unmapped", json!({})),
        call(
            "named",
            "find_symbols",
            json!({"file": "src/dock_hire/fees.py"}),
            Want::CannotRun,
        ),
        call(
            "named",
            "find_symbols",
            json!({"at": "src/dock_hire/fees.py:1"}),
            Want::CannotRun,
        ),
        call("named", "unmapped", json!({"path": "src"}), Want::CannotRun),
    ];
    for results in project.answers(&calls) {
        for (call, result) in calls.iter().zip(&results) {
            let text = content_text(result);
            if call.label == "exact" {
                assert_eq!(text, NO_CODE, "{} {}", call.tool, call.args);
            } else {
                assert!(
                    text.contains("[code]"),
                    "{} {}: {text:?}",
                    call.tool,
                    call.args
                );
            }
        }
    }
}

/// Every `kind = "…"` of these fixtures' configs.
fn fixture_kinds(fixtures: &[&str]) -> BTreeSet<String> {
    let mut kinds = BTreeSet::new();
    for name in fixtures {
        let text = read_text(&fixture(name), "specengine.toml");
        for part in text.split("kind").skip(1) {
            let Some(rest) = part.trim_start().strip_prefix('=') else {
                continue;
            };
            let Some(rest) = rest.trim_start().strip_prefix('"') else {
                continue;
            };
            if let Some((kind, _)) = rest.split_once('"') {
                kinds.insert(kind.to_owned());
            }
        }
    }
    kinds
}

/// Every `item_kind` the code fixtures' goldens hold, and the parser's
/// other item words.
fn item_kinds() -> BTreeSet<String> {
    let mut kinds: BTreeSet<String> = [
        "function_item",
        "function_signature_item",
        "impl_item",
        "struct_item",
        "enum_item",
        "trait_item",
        "mod_item",
        "const_item",
        "static_item",
        "type_item",
        "union_item",
        "macro_definition",
    ]
    .iter()
    .map(|kind| (*kind).to_owned())
    .collect();
    for name in ["code-index", "code-a"] {
        let text = read_text(&fixture(name), "expected.json");
        for part in text.split("\"item_kind\": \"").skip(1) {
            if let Some((kind, _)) = part.split_once('"') {
                kinds.insert(kind.to_owned());
            }
        }
    }
    kinds
}

/// The string literals of a Rust source's lines outside `//` comments and
/// outside attribute lines (`#[…]`: `vis = "pub(crate)"`, a type path),
/// as `mcp_genre.rs` reads them (naive pairs of `"`).
fn literals(source: &str) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    for (number, line) in source.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("//") || trimmed.starts_with("#[") {
            continue;
        }
        for literal in line.split('"').skip(1).step_by(2) {
            found.push((number + 1, literal.to_owned()));
        }
    }
    found
}

/// Genre ("Rules and edge cases"; architecture.md#universal): in the code
/// tools' sources (`code.rs`, `mirror.rs`) no literal is a fixture's kind
/// (spec-a, -b, -c, code-index, code-a) or an item kind, holds `::`, a
/// stack, language or fixture word or a code file suffix; the code tools'
/// texts on the wire (descriptions, input schema descriptions) and the
/// `INSTRUCTIONS` code line hold none of those words. M: an `"impl"` or
/// `"function_item"` literal; a `::` split; "crate" in a description.
#[test]
fn genre_code_sources_and_texts_hold_no_kind_or_rust_word() {
    let kinds = fixture_kinds(&["spec-a", "spec-b", "spec-c", "code-index", "code-a"]);
    assert!(
        kinds.contains("tenet") && kinds.contains("piece") && kinds.contains("tariff"),
        "{kinds:?}"
    );
    let items = item_kinds();
    assert!(items.contains("function_item") && items.contains("impl_item"));
    let src = repository_root().join("crates/specengine-mcp/src");
    assert!(
        src.join("code.rs").is_file(),
        "the code tools live in code.rs (\"Description and interactions\")"
    );
    let mut offenders = Vec::new();
    for name in ["code.rs", "mirror.rs"] {
        let source = fs::read_to_string(src.join(name)).expect("a source");
        for (line, literal) in literals(&source) {
            if kinds.contains(&literal) {
                offenders.push(format!("{name}:{line}: the kind literal {literal:?}"));
            }
            if items.contains(&literal) || literal.contains("_item") {
                offenders.push(format!("{name}:{line}: the item kind {literal:?}"));
            }
            for problem in genre_problems(&literal) {
                offenders.push(format!("{name}:{line}: {problem} in {literal:?}"));
            }
        }
    }
    let mut session = Session::open(Era::Legacy, &[], None, Home::Fresh);
    let list = session.tools();
    drop(session.finish());
    for name in CODE_TOOLS {
        let listed = tool(&list, name);
        let mut texts = vec![
            listed["description"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
        ];
        descriptions(&listed["inputSchema"], &mut texts);
        for text in texts {
            for problem in genre_problems(&text) {
                offenders.push(format!("{name}'s text: {problem} in {text:?}"));
            }
        }
    }
    for problem in genre_problems(CODE_LINE) {
        offenders.push(format!("INSTRUCTIONS' code line: {problem}"));
    }
    assert!(offenders.is_empty(), "{offenders:#?}");
}

/// docs/features/bevy-roles-reads.md "find_symbols", "Rules and edge cases",
/// Genre: the doc comments of the role mirror become
/// `outputSchema` text, so every description under `find_symbols`'
/// `symbols[].roles` holds no stack word, role word, source word or
/// framework name, nor a language or fixture word, a code file suffix or
/// `::` (the role words and sources: code-bevy's `expected.json`, 10a-1's
/// copy of the code crate's data; every tool text's stack words:
/// `mcp_genre.rs`). M: "Bevy" or "system" in a doc comment of the role
/// mirror.
#[test]
fn find_symbols_roles_schema_holds_no_stack_role_or_source_word() {
    let expected: Value =
        serde_json::from_str(&read_text(&fixture(BEVY), "expected.json")).expect("JSON");
    let mut banned: Vec<String> = ["words", "sources"]
        .iter()
        .flat_map(|key| {
            expected[*key]
                .as_array()
                .unwrap_or_else(|| panic!("expected.json `{key}`"))
                .iter()
                .map(|word| word.as_str().expect("a word").to_owned())
                .collect::<Vec<_>>()
        })
        .collect();
    banned.push("bevy".to_owned());
    for word in ["system", "component", "derive", "call"] {
        assert!(banned.iter().any(|banned| banned == word), "{banned:?}");
    }
    let (symbols, _, _) = code_schemas();
    let mut texts = Vec::new();
    descriptions(schema_at(&symbols, "symbols[].roles"), &mut texts);
    let mut offenders = Vec::new();
    for text in &texts {
        for word in &banned {
            if has_word(text, word) {
                offenders.push(format!("the word {word:?} in {text:?}"));
            }
        }
        for problem in genre_problems(text) {
            offenders.push(format!("{problem} in {text:?}"));
        }
    }
    assert!(offenders.is_empty(), "{offenders:#?}");
}

// ------------------------------------------------------------------ TE-2

/// The `git status` of TE-2: `--porcelain=v1 -z --untracked-files=all`,
/// without optional locks (the check itself writes nothing).
fn porcelain(sandbox: &Sandbox, root: &Path) -> Vec<u8> {
    sandbox.git(
        root,
        &[
            "--no-optional-locks",
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
        ],
    )
}

/// The queue as the CLI library reads it: every proposal, every task,
/// every event.
fn queue_rows(env: &Env, git: &GitEnv) -> (Vec<String>, String, String) {
    let globals = Globals::default();
    let proposals = inbox(
        env,
        &globals,
        &InboxRequest {
            all: true,
            git: git.clone(),
        },
    )
    .unwrap_or_else(|error| panic!("inbox: {error}"))
    .proposals
    .into_iter()
    .map(|entry| entry.id)
    .collect();
    let tasks = task_list(
        env,
        &globals,
        &TaskListRequest {
            statuses: Vec::new(),
            git: git.clone(),
        },
    )
    .unwrap_or_else(|error| panic!("task list: {error}"));
    let events =
        events_after(env, &globals, None).unwrap_or_else(|error| panic!("events: {error}"));
    (
        proposals,
        format!("{:?}", tasks.tasks),
        format!("{events:?}"),
    )
}

/// TE-2 (08 AC-3, S14, S15): a temp git repository of `fixtures/code-a`
/// with `fixtures/spec-lock/code-a.lock` as it stands committed as
/// `spec.lock`; every code tool called valid and invalid (forms, exits 0,
/// 1, 2, bad arguments) in both eras from a subdirectory; then `git
/// status --porcelain=v1 -z --untracked-files=all` is as before (empty),
/// `.git/index`'s mtime and `spec.lock`'s bytes unchanged, the tree
/// (`.git` included) byte-identical, the queue's proposals, tasks and
/// events unchanged, every new path under `HOME`; no listed tool exposes
/// `index`, `lock accept` or `propose drift` but `report_drift`. M: a
/// write under the root or a queue row.
#[test]
fn te2_code_tools_write_nothing_under_the_root() {
    let scratch = Scratch::new("code-door");
    let home = scratch.home("h");
    let root = scratch.copy("code-a", "repo");
    let lock = fs::read(fixture("spec-lock").join("code-a.lock")).expect("the golden lock");
    fs::write(root.join("spec.lock"), &lock).expect("spec.lock");
    let sandbox = Sandbox::new(scratch.path());
    sandbox.init(&root);
    sandbox.add_all(&root);
    sandbox.commit(&root, "code-a and its lock");
    let env = cli_env(&root, Some(&home));
    let git = GitEnv::new(&root, sandbox.vars());
    // The index and queue exist before the snapshots (the library's
    // `spec index`: it writes the data directory only).
    index_with_library(&env, &Globals::default());
    let queue = queue_rows(&env, &git);
    let status = porcelain(&sandbox, &root);
    assert_eq!(status, b"", "committed clean");
    let index_mtime = || {
        fs::metadata(root.join(".git").join("index"))
            .and_then(|meta| meta.modified())
            .expect(".git/index")
    };
    let mtime = index_mtime();
    let tree = snapshot(&root);
    let paths: BTreeSet<String> = snapshot(scratch.path()).into_keys().collect();
    let cwd = root.join("crates").join("tally");

    let valid = [
        ("find_symbols", json!({"query": "step"})),
        ("find_symbols", json!({"query": "Tally", "limit": 3})),
        ("find_symbols", json!({"node": "LAW-STEP"})),
        (
            "find_symbols",
            json!({"file": "crates/tally/src/counter.rs"}),
        ),
        (
            "find_symbols",
            json!({"at": "crates/tally/src/counter.rs:23"}),
        ),
        ("find_symbols", json!({"node": "LAW-NOPE"})),
        ("find_symbols", json!({"file": "docs/spec/counter.md"})),
        ("find_symbols", json!({})),
        ("find_symbols", json!({"node": "LAW-STEP", "limit": 1})),
        ("refs", json!({"id": "LAW-STEP"})),
        ("refs", json!({"id": "PIECE-COUNTER"})),
        ("refs", json!({"id": "AIM-01"})),
        ("refs", json!({"id": "LAW-NOPE"})),
        ("refs", json!({"id": "L\u{0410}W-STEP"})),
        ("unmapped", json!({})),
        ("unmapped", json!({"path": "crates/tally/src"})),
        ("unmapped", json!({"path": "crates/tally/src/counter.rs"})),
        ("unmapped", json!({"path": "tools/loose.rs"})),
        ("unmapped", json!({"path": "docs"})),
    ];
    let invalid = [
        ("find_symbols", json!({"query": 5})),
        ("find_symbols", json!({"query": "step", "bogus": 1})),
        ("refs", json!({})),
        ("refs", json!({"id": ["LAW-STEP"]})),
        ("unmapped", json!({"path": 3})),
    ];
    for era in ERAS {
        let mut session = Session::open(era, &[], Some(&cwd), Home::At(&home));
        let names = tool_names(&session.tools());
        assert_eq!(names, TOOLS, "{era:?}");
        for name in &names {
            for word in ["index", "lock", "accept"] {
                assert!(!name.contains(word), "{era:?}: {name} exposes {word}");
            }
            assert!(
                !name.contains("drift") || name == "report_drift",
                "{era:?}: {name}"
            );
        }
        for (name, args) in &valid {
            let reply = session.call(name, args.clone());
            assert!(
                reply.get("result").is_some(),
                "{era:?} {name} {args}: {reply}"
            );
        }
        for (name, args) in &invalid {
            let reply = session.call(name, args.clone());
            assert_bad_arguments(&reply, "", &format!("{era:?} {name} {args}"));
        }
        let done = session.finish();
        assert!(done.status.success(), "{era:?}: {}", done.stderr);
        assert_eq!(done.stderr, "", "{era:?}");
    }

    assert_eq!(porcelain(&sandbox, &root), status, "git status");
    assert_eq!(index_mtime(), mtime, ".git/index touched");
    assert_eq!(
        fs::read(root.join("spec.lock")).expect("spec.lock"),
        lock,
        "spec.lock"
    );
    assert!(snapshot(&root) == tree, "the repository changed");
    assert_eq!(queue_rows(&env, &git), queue, "the queue changed");
    let home_prefix = format!(
        "{}/",
        home.strip_prefix(scratch.path())
            .expect("HOME in the scratch")
            .display()
    );
    for path in snapshot(scratch.path()).into_keys() {
        if !paths.contains(&path) {
            assert!(
                path.starts_with(&home_prefix),
                "a new path outside HOME: {path}"
            );
        }
    }
    assert!(
        data_dir(&home).join("code-a.db").is_file(),
        "the index lives under HOME"
    );
}
