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
//! docs/features/check-binding.md (6b-2b), `check_binding` = `spec verify
//! --node REF [--symbol QPATH]` (`common/read.rs` `code_library`), the
//! fourth code tool (`TWINS`, `CODE_TOOLS`; the RD-1, RD-5, genre and TE-2
//! door tests above take it in):
//!
//! - RD-7: `check_binding_parity_per_exit` over a git repository of
//!   `fixtures/code-a` with the golden lock, both eras: exits 0, 1 under
//!   `enforce` (and `enforce-introduced`, no base), 2 with the report
//!   (a Cannot row, a walk gap, a format-1 lock) and 2 without (REF,
//!   discovery), schema refusals; `Want::Verdict` (the document kept, no
//!   `reason`). M8: a rendered exit-2 report answered as `CannotRun`.
//! - `check_binding_is_listed_as_a_read` (input, annotations, description).
//! - TE-2: `check_binding_mirror_is_verify_json` (the key sets, enums,
//!   `AlwaysNull`); `check_binding_documents_meet_every_key` (the walker
//!   over code-a drifted and `fixtures/anchor-states` renamed: every state,
//!   anchor state, nullable key and `delta.bindings` list met);
//!   `check_binding_is_byte_identical_across_roots_order_and_cwd`;
//!   `check_binding_leaves_stdout_json_rpc_only`;
//!   `check_binding_on_spec_c_is_the_code_table_report`; `mcp_size.rs`.
//! - TE-3: `te2_code_tools_write_nothing_under_the_root` (its calls) and
//!   `te3_check_binding_writes_nothing_under_the_root` (`enforce`
//!   committed, the git index stat-dirty); `mcp_door.rs` over spec-a.
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
    Env, GitEnv, Globals, InboxRequest, LockAcceptRequest, LockPrompt, OUTPUT_CAP_CHARS,
    SYMBOLS_LIMIT_DEFAULT, SYMBOLS_LIMIT_MAX, SYMBOLS_LIMIT_MIN, TaskListRequest, events_after,
    inbox, lock_accept, task_list,
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

/// Each code tool and the CLI command its description names
/// (`check_binding` since docs/features/check-binding.md).
const TWINS: [(&str, &str); 4] = [
    ("check_binding", "spec verify --node"),
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
    /// `check_binding` (docs/features/check-binding.md "Data", the exit
    /// table): a rendered report with this verdict, its document kept at
    /// every exit: `clean` and `observed` a success (exit 0), `blocked`
    /// (exit 1) and `cannot-verify` (exit 2, a report) an error result;
    /// the summary line `verify --node …` in the text.
    Verdict(&'static str),
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
            Want::Verdict(verdict) => {
                let document = &result["structuredContent"];
                assert!(
                    document.is_object(),
                    "{context}: the report's document is kept (M8): {result}"
                );
                assert_eq!(document["verdict"], json!(verdict), "{context}: {text}");
                assert!(
                    document.get("reason").is_none(),
                    "{context}: verify's document has no `reason`: {document}"
                );
                let finding = !matches!(verdict, "clean" | "observed");
                assert_eq!(result["isError"], json!(finding), "{context}: {text}");
                assert!(
                    text.lines().any(|line| line.starts_with("verify --node ")),
                    "{context}: the summary line: {text:?}"
                );
            }
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
        assert_eq!(tool_names(&list), TOOLS, "{era:?}: twenty-one tools");
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

/// docs/features/layer-c-reads.md TE-7: the snapshot written over
/// code-index ([`common::layer_c::write_snapshot`]): twins.rs' two `impl
/// Sheet` items share a moniker (a collision, shown `#1`, `#2`); lib.rs'
/// first three rows are `unresolved`, `local` and `none`, main.rs' first
/// `not_loaded`; split.rs' first row ends a byte later (its item
/// `unmatched`); every other row a moniker of its own.
fn layer_c_plan(row: &common::layer_c::Row) -> (String, Option<String>, i64) {
    use common::layer_c::{MONIKER_WORD, plain};
    let gap = |state: &str| (state.to_owned(), None, 0);
    match (row.path.as_str(), row.qpath.as_str()) {
        ("crates/inkwell/src/twins.rs", qpath) if qpath.contains("::impl Sheet #") => (
            "moniker".to_owned(),
            Some(format!("{MONIKER_WORD} impl#[Sheet]")),
            0,
        ),
        (path, _) => match (path, row.nth) {
            ("crates/inkwell/src/lib.rs", 1) => gap("unresolved"),
            ("crates/inkwell/src/lib.rs", 2) => gap("local"),
            ("crates/inkwell/src/lib.rs", 3) => gap("none"),
            ("crates/inkwell/src/main.rs", 1) => gap("not_loaded"),
            ("crates/inkwell/src/split.rs", 1) => {
                let (state, moniker, _) = plain(row);
                (state, moniker, 1)
            }
            _ => plain(row),
        },
    }
}

/// The shown collision of [`layer_c_plan`]'s twins and its raw form.
const SHEET_SHOWN: &str = "zzmoniker impl#[Sheet] #2";
const SHEET_RAW: &str = "zzmoniker impl#[Sheet]";

/// The moniker calls of TE-7: one item's own moniker, the shown collision
/// form (one item), the raw form (none, Q3); the files [`layer_c_plan`]
/// gives gaps.
fn layer_c_calls(page_moniker: &str) -> Vec<Call> {
    use Want::Answered;
    vec![
        call(
            "c moniker",
            "find_symbols",
            json!({"query": page_moniker}),
            Answered,
        ),
        call(
            "c collision",
            "find_symbols",
            json!({"query": SHEET_SHOWN}),
            Answered,
        ),
        call(
            "c raw",
            "find_symbols",
            json!({"query": SHEET_RAW}),
            Answered,
        ),
        call(
            "c file lib",
            "find_symbols",
            json!({"file": "crates/inkwell/src/lib.rs"}),
            Answered,
        ),
        call(
            "c file main",
            "find_symbols",
            json!({"file": "crates/inkwell/src/main.rs"}),
            Answered,
        ),
        call(
            "c file split",
            "find_symbols",
            json!({"file": "crates/inkwell/src/split.rs"}),
            Answered,
        ),
    ]
}

/// Every `find_symbols` document's layer C keys of one era's `results`:
/// `layer_c.state` is `state` where a document holds a target list, `null`
/// at exit 1; a module's and a RON path's keys `null`, an item's
/// `c_state` a word; the values met, by key.
fn layer_c_values(
    calls: &[Call],
    results: &[Value],
    state: &str,
) -> BTreeMap<&'static str, BTreeSet<String>> {
    let mut met: BTreeMap<&'static str, BTreeSet<String>> = BTreeMap::new();
    for (call, result) in calls.iter().zip(results) {
        if call.tool != "find_symbols" {
            continue;
        }
        let Some(document) = result.get("structuredContent") else {
            continue;
        };
        let layer_c = &document["layer_c"];
        if document["reason"].is_string() {
            assert_eq!(layer_c, &Value::Null, "{}: exit 1, as `code`", call.label);
            continue;
        }
        assert_eq!(layer_c["state"], json!(state), "{}: {layer_c}", call.label);
        for target in document["symbols"].as_array().into_iter().flatten() {
            if target["target"] != json!("item") {
                for key in ["c_state", "moniker", "agrees"] {
                    assert_eq!(target[key], Value::Null, "{}: {target}", call.label);
                }
                continue;
            }
            assert!(target["c_state"].is_string(), "{}: {target}", call.label);
            for key in ["c_state", "moniker", "agrees"] {
                let shown = match &target[key] {
                    Value::String(text) if key == "moniker" => {
                        if text.contains(" #") {
                            "collision".to_owned()
                        } else {
                            "string".to_owned()
                        }
                    }
                    other => other.to_string(),
                };
                met.entry(key).or_default().insert(shown);
            }
            if let Some(tier) = target["tier"].as_str() {
                met.entry("tier").or_default().insert(tier.to_owned());
            }
        }
    }
    met
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
///
/// docs/features/layer-c-reads.md TE-7. M: a layer C key the mirror drops
/// or types otherwise; a moniker tier the MCP side computes apart. Over a
/// code-index copy with a layer C snapshot (`common::layer_c`: every
/// row's state and moniker planned, a collision, a moved end) the same
/// calls and three moniker queries answer as the CLI does, fresh and then
/// stale (page.rs changed): every `c_state` of each step met, `moniker`
/// met as text, collision and `null`, `agrees` with a value and `null`,
/// the `moniker` tier; the shown collision finds one item, the raw form
/// none, an unseen file's moniker none.
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

    // docs/features/layer-c-reads.md TE-7: code-index with a layer C
    // snapshot, fresh, then stale with page.rs changed (its rows
    // `unseen`): every answer the CLI's and walked against the
    // `outputSchema`, the layer C keys met with each kind of value, the
    // moniker tier answering.
    let project = Project::reads();
    index_with_library(&project.env(), &Globals::default());
    let db = data_dir(&project.home).join("inkwell.db");
    let written = common::layer_c::write_snapshot(&project.root, &db, &layer_c_plan);
    assert!(written.items > 20, "{} rows", written.items);
    let page_moniker = written
        .monikers
        .iter()
        .find(|((path, _), _)| path == "crates/inkwell/src/page.rs")
        .and_then(|(_, (_, moniker))| moniker.clone())
        .expect("a moniker of page.rs");
    let page_qpath = written
        .monikers
        .values()
        .find(|(_, moniker)| moniker.as_deref() == Some(page_moniker.as_str()))
        .map(|(qpath, _)| qpath.clone())
        .expect("its item");
    let mut calls = reads();
    calls.extend(layer_c_calls(&page_moniker));
    for results in project.answers(&calls) {
        let met = layer_c_values(&calls, &results, "fresh");
        assert_eq!(
            met["c_state"],
            set(&[
                "\"local\"",
                "\"moniker\"",
                "\"none\"",
                "\"not_loaded\"",
                "\"unmatched\"",
                "\"unresolved\""
            ]),
            "every state of a fresh snapshot walked"
        );
        assert_eq!(
            met["moniker"],
            set(&["collision", "null", "string"]),
            "{:?}",
            met["moniker"]
        );
        assert!(
            met["agrees"].contains("null") && met["agrees"].len() > 1,
            "`agrees` met with a value and null: {:?}",
            met["agrees"]
        );
        assert!(met["tier"].contains("moniker"), "{:?}", met["tier"]);
        let found = document(&calls, &results, "c moniker");
        assert_eq!(symbol_count(found), 1, "{found}");
        assert_eq!(found["symbols"][0]["qpath"], json!(page_qpath), "{found}");
        assert_eq!(found["symbols"][0]["tier"], json!("moniker"), "{found}");
        let twin = document(&calls, &results, "c collision");
        assert_eq!(symbol_count(twin), 1, "{twin}");
        assert_eq!(
            twin["symbols"][0]["qpath"],
            json!("crates/inkwell::twins::impl Sheet #2"),
            "{twin}"
        );
        assert_eq!(twin["symbols"][0]["moniker"], json!(SHEET_SHOWN), "{twin}");
        assert_eq!(symbol_count(document(&calls, &results, "c raw")), 0);
    }
    let page = project.root.join("crates/inkwell/src/page.rs");
    let mut bytes = fs::read(&page).expect("page.rs");
    bytes.push(b'\n');
    fs::write(&page, bytes).expect("page.rs");
    for results in project.answers(&calls) {
        let met = layer_c_values(&calls, &results, "stale");
        assert!(
            met["c_state"].contains("\"unseen\""),
            "{:?}",
            met["c_state"]
        );
        assert_eq!(
            symbol_count(document(&calls, &results, "c moniker")),
            0,
            "an unseen file's moniker is found no more"
        );
        assert_eq!(symbol_count(document(&calls, &results, "c collision")), 1);
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
///
/// docs/features/layer-c-reads.md RD-4 (R1): `find_symbols`' `tier` the four
/// words, `get_impact`'s seeds' the three; `c_state` the seven words,
/// `layer_c.state` `fresh`, `stale`; `moniker` free. M: `CodeTier` gaining
/// `moniker` (it would reach `get_impact`).
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
    // docs/features/layer-c-reads.md RD-4: `find_symbols`' own tier enum
    // gains `moniker`; `get_impact`'s seeds keep the three (R1).
    let tier = schema_at(&symbols, "symbols[].tier");
    assert_eq!(
        enum_values(tier),
        Some(set(&["exact", "moniker", "name", "suffix"]))
    );
    assert!(
        types_of(tier).contains("null"),
        "tier null outside QUERY: {tier}"
    );
    let impact = {
        let mut session = Session::open(Era::Legacy, &[], None, Home::Fresh);
        let schemas = output_schemas(&session.tools());
        drop(session.finish());
        schemas["get_impact"].clone()
    };
    assert_eq!(
        enum_values(schema_at(&impact, "seeds.symbols[].tier")),
        Some(set(&["exact", "name", "suffix"])),
        "get_impact's seeds keep three tiers (R1, Q2)"
    );
    // The layer C keys: `c_state` and `layer_c.state` the CLI's closed
    // words, `moniker` a free string (any text the snapshot stored).
    assert_eq!(
        enum_values(schema_at(&symbols, "symbols[].c_state")),
        Some(set(&[
            "local",
            "moniker",
            "none",
            "not_loaded",
            "unmatched",
            "unresolved",
            "unseen"
        ]))
    );
    assert_eq!(
        enum_values(schema_at(&symbols, "layer_c.state")),
        Some(set(&["fresh", "stale"]))
    );
    free(&symbols, "symbols[].moniker");

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
/// docs/features/layer-c-reads.md "JSON": the document gains `layer_c`
/// (null or an object of nine keys), a target `c_state`, `moniker`,
/// `agrees` (nullable). M: a layer C key unmirrored.
#[test]
fn code_mirrors_pin_the_q35_types() {
    let (symbols, refs, unmapped) = code_schemas();
    assert_eq!(
        keys_at(&symbols),
        set(&[
            "form", "query", "node", "path", "line", "reason", "notes", "symbols", "total",
            "omitted", "code", "layer_c"
        ])
    );
    // docs/features/layer-c-reads.md "JSON": `layer_c` null or the state and
    // `code_resolve`'s columns; a target's three layer C keys nullable.
    assert_eq!(
        keys_at(schema_at(&symbols, "layer_c")),
        set(&[
            "state",
            "protocol",
            "ra_ap",
            "host",
            "toolchains",
            "degraded",
            "workspaces",
            "files",
            "items"
        ])
    );
    for (path, kinds) in [
        ("layer_c", &["null", "object"][..]),
        ("layer_c.protocol", &["integer"][..]),
        ("layer_c.ra_ap", &["string"][..]),
        ("layer_c.host", &["string"][..]),
        ("layer_c.toolchains", &["array"][..]),
        ("layer_c.degraded", &["boolean"][..]),
        ("layer_c.workspaces", &["integer"][..]),
        ("layer_c.files", &["integer"][..]),
        ("layer_c.items", &["integer"][..]),
        ("symbols[].moniker", &["null", "string"][..]),
        ("symbols[].agrees", &["boolean", "null"][..]),
    ] {
        assert_eq!(types_of(schema_at(&symbols, path)), set(kinds), "{path}");
    }
    assert!(
        types_of(schema_at(&symbols, "symbols[].c_state")).contains("null"),
        "c_state null without a snapshot"
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
            "roles",
            "c_state",
            "moniker",
            "agrees"
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
/// ["links"]`), and `Hints use flags:`; 1 892 bytes. 1 932 since
/// docs/features/roadmap-surfaces.md ("Data"): the reads' line ends with
/// [`ROADMAP_READ`] (40 bytes). 1 978 since docs/features/check-binding.md
/// ("INSTRUCTIONS", Q31, Q-C): the code line ends with [`CHECK_BINDING_CLAUSE`]
/// (46 bytes). 1 977 since docs/features/show-bindings.md (Q-F): the
/// hints gain [`BINDINGS_HINT`] after `--links is with ["links"]` (33
/// bytes) and the REF paragraph loses [`NO_PROJECT_GLOSS`] (34 bytes).
/// 2 017 since docs/features/impact.md ("INSTRUCTIONS", Q-B): the reads'
/// line ends with `; get_impact(id|symbol, depth?) = impact` (40 bytes),
/// the `probes` paragraph gone. 2 023 since docs/features/impact-since.md
/// ("MCP, INSTRUCTIONS, plugin 0.16.0"): the clause gains [`SINCE_INPUT`]
/// after `id|symbol`, [`IMPACT_READ`] (46 bytes). 2 040 since
/// docs/features/review-form.md ("INSTRUCTIONS" of the developer's text,
/// D6): [`REVIEW_LINE`] after the drift line (85 bytes), and
/// [`MARKDOWN_GLOSS`], [`CODE_GLOSS`], [`DRIFT_GLOSS`] and [`TAIL_GLOSS`]
/// cut (68 bytes); [`as_of_impact_since`] puts them back.
const PLANNED_INSTRUCTIONS: &str = concat!(
    "SpecEngine keeps a project's spec in git. Each tool is a ",
    "spec command: content its `2>&1` output, structuredContent its --json document.\n",
    "\n",
    "Reads:\n",
    "- get_tree(root?, depth?, kinds?, archive?) = tree; get_node(id, with?, archive?) = ",
    "show; search(query, kinds?, limit?, archive?) = search; get_context_bundle(node_ids, ",
    "budget?) = bundle; get_proposal(proposal_id) = review --brief; get_roadmap(release?, ",
    "pool?) = roadmap; get_impact(id|symbol|since, depth?) = impact.\n",
    "- Code: find_symbols(query|node|file|at, limit?) = symbols; refs(id) = refs; ",
    "unmapped(path?) = unmapped; check_binding(node, symbol?) = verify --node.\n",
    "Queue (writes only the queue; the owner decides on a terminal):\n",
    "- propose_change = spec propose update|create|interpretation|amendment: a node's new ",
    "text against its span_hash; create: new ID sections in it, or a new file (base null).\n",
    "- ask_question, report_discrepancy, report_decision = spec propose ",
    "question|discrepancy|decision: hits are what is decided or asked; stored only if ",
    "distinct_from names each; work on your working answer.\n",
    "- report_drift = spec propose drift: queues HEAD's lock drift; evidence computed, never ",
    "typed.\n",
    "- review_proposal = review --brief after the owner's form; only when the owner asks.\n",
    "- get_task, claim_task, submit_plan, report_run, complete_task = spec task ",
    "show|claim|plan|report|complete; the three above take task_id.\n",
    "\n",
    "REF: an ID, alias, slug/ID, ID#SECTION or root-relative .md path. Hints use flags: ",
    "--kind is kinds, ROOT is root, --links is with [\"links\"], --bindings is with ",
    "[\"bindings\"]. Answers are cut at 40000 characters. A REF ",
    "naming nothing or a call that cannot run is an error result with the reason. ",
    "Resources: spec://<slug>/tree, spec://<slug>/node/<REF, percent-encoded>.\n",
    "\n",
    "Rules:\n",
    "- Deterministic: one state, one result; no LLM inside.\n",
    "- Reads refresh SpecEngine's index outside the project root; no tool writes under it. ",
    "Spec files change only when the owner approves.\n",
    "- Nothing is blocked by a discrepancy.\n",
    "- IDs are Latin only; a look-alike is refused with its Latin fix.",
);

/// docs/features/review-form.md: the owner's form's line, its LF
/// included (85 bytes), right after the drift line.
const REVIEW_LINE: &str =
    "- review_proposal = review --brief after the owner's form; only when the owner asks.\n";

/// docs/features/review-form.md: the cuts that make room for
/// [`REVIEW_LINE`] (12 + 16 + 14 + 26 = 68 bytes).
const MARKDOWN_GLOSS: &str = " as Markdown";
const CODE_GLOSS: &str = " beside the code";
const DRIFT_GLOSS: &str = " for the owner";
const TAIL_GLOSS: &str = ", with a tail on narrowing";

/// impact-since's text (2 023 bytes) out of review-form's (2 040):
/// [`REVIEW_LINE`] taken out, the four glosses put back.
fn as_of_impact_since(instructions: &str) -> String {
    instructions
        .replacen(REVIEW_LINE, "", 1)
        .replacen(
            "a project's spec in git.",
            &format!("a project's spec{MARKDOWN_GLOSS} in git{CODE_GLOSS}."),
            1,
        )
        .replacen(
            "queues HEAD's lock drift;",
            &format!("queues HEAD's lock drift{DRIFT_GLOSS};"),
            1,
        )
        .replacen(
            "cut at 40000 characters.",
            &format!("cut at 40000 characters{TAIL_GLOSS}."),
            1,
        )
}

/// docs/features/roadmap-surfaces.md "Data": what the reads' line gains,
/// before its full stop (40 bytes).
const ROADMAP_READ: &str = "; get_roadmap(release?, pool?) = roadmap";

/// docs/features/impact.md "INSTRUCTIONS": what the reads' line gains after
/// [`ROADMAP_READ`], before its full stop (40 bytes); 46 since
/// docs/features/impact-since.md (with [`SINCE_INPUT`]).
const IMPACT_READ: &str = "; get_impact(id|symbol|since, depth?) = impact";

/// docs/features/impact-since.md "MCP, INSTRUCTIONS, plugin 0.16.0": what
/// the impact clause gains right after `id|symbol` (6 bytes).
const SINCE_INPUT: &str = "|since";

/// docs/features/impact.md "INSTRUCTIONS": the reads' line, its LF
/// included (312 bytes; 318 since docs/features/impact-since.md).
const READS_LINE: &str = "- get_tree(root?, depth?, kinds?, archive?) = tree; get_node(id, with?, archive?) = \
                          show; search(query, kinds?, limit?, archive?) = search; get_context_bundle(node_ids, \
                          budget?) = bundle; get_proposal(proposal_id) = review --brief; get_roadmap(release?, \
                          pool?) = roadmap; get_impact(id|symbol|since, depth?) = impact.\n";

/// docs/features/check-binding.md "INSTRUCTIONS": what the code line gains,
/// before its full stop (46 bytes).
const CHECK_BINDING_CLAUSE: &str = "; check_binding(node, symbol?) = verify --node";

/// docs/features/show-bindings.md "INSTRUCTIONS" (Q-F): what the hints
/// gain right after `--links is with ["links"]` (33 bytes).
const BINDINGS_HINT: &str = ", --bindings is with [\"bindings\"]";

/// docs/features/show-bindings.md "INSTRUCTIONS" (Q-F): what the REF
/// paragraph loses right after "a call that cannot run" (34 bytes; the
/// no-project error result names `spec init` itself).
const NO_PROJECT_GLOSS: &str = " (no specengine.toml: `spec init`)";

/// check-binding's text (1 978 bytes) out of show-bindings' (1 977):
/// [`BINDINGS_HINT`] taken out and [`NO_PROJECT_GLOSS`] put back.
fn as_of_check_binding(instructions: &str) -> String {
    instructions.replacen(BINDINGS_HINT, "", 1).replacen(
        "a call that cannot run is",
        &format!("a call that cannot run{NO_PROJECT_GLOSS} is"),
        1,
    )
}

/// impact's text (2 017 bytes) out of impact-since's (2 023):
/// [`SINCE_INPUT`] taken out of the impact clause.
fn as_of_impact(instructions: &str) -> String {
    instructions.replacen("get_impact(id|symbol|since, ", "get_impact(id|symbol, ", 1)
}

/// show-bindings' text (1 977 bytes) out of impact's (2 017): the impact
/// clause taken out.
fn as_of_show_bindings(instructions: &str) -> String {
    instructions.replacen("; get_impact(id|symbol, depth?) = impact", "", 1)
}

/// The code tools' line, its LF included (104 + 1 bytes; 151 since
/// docs/features/check-binding.md: [`CHECK_BINDING_CLAUSE`] before the stop).
const CODE_LINE: &str = "- Code: find_symbols(query|node|file|at, limit?) = symbols; refs(id) = refs; \
                         unmapped(path?) = unmapped; check_binding(node, symbol?) = verify --node.\n";

/// task-package's tasks line, its LF included.
const TASKS_LINE: &str = "- get_task, claim_task, submit_plan, report_run, complete_task = spec \
                          task show|claim|plan|report|complete; the three above take task_id.\n";

/// The `probes` build's paragraph until docs/features/impact.md (Q-B: it
/// goes; both builds send `INSTRUCTIONS` alone, `mcp_create.rs`).
const PROBES_PARAGRAPH: &str = "\n\nProbes build: review_proposal, probe_output, probe_sleep.";

/// `blake3_hex(PLANNED_INSTRUCTIONS)`: the hash of the spec's text built
/// here, never of the server's; `mcp_path.rs` and `mcp_decision.rs` pin
/// the same value against `INSTRUCTIONS` (re-pinned by
/// docs/features/roadmap-surfaces.md, docs/features/check-binding.md,
/// docs/features/show-bindings.md, docs/features/impact.md,
/// docs/features/impact-since.md and docs/features/review-form.md).
const PLANNED_BLAKE3: &str = "47411b2fd929cbb742ee813df864c584f3d39f7294c1c0348f9360ff0d0b05de";

/// impact-since's text (2 023 bytes), [`as_of_impact_since`] over
/// [`PLANNED_INSTRUCTIONS`]: its `PLANNED_BLAKE3` before
/// docs/features/review-form.md.
const IMPACT_SINCE_BLAKE3: &str =
    "9737b6644b04bf18e3c69b14c83d36ef6de5ca01fff650f6c3f9821072e45b1c";

/// impact's text (2 017 bytes), [`as_of_impact`] over
/// [`PLANNED_INSTRUCTIONS`]: its `PLANNED_BLAKE3` before
/// docs/features/impact-since.md.
const IMPACT_BLAKE3: &str = "34f0f0b4a5f3555f6d59c2fb2e9fc4e94b9a303daeb1342320652309b44482e7";

/// show-bindings' text (1 977 bytes), [`as_of_show_bindings`] over
/// [`PLANNED_INSTRUCTIONS`]: its `PLANNED_BLAKE3` before
/// docs/features/impact.md.
const SHOW_BINDINGS_BLAKE3: &str =
    "0be8933f33543d7aafd9ab2c9419f5b5ae6b1702e07ce755a83c15ed799061f4";

/// check-binding's text (1 978 bytes), [`as_of_check_binding`] over
/// show-bindings': its `PLANNED_BLAKE3` before
/// docs/features/show-bindings.md.
const CHECK_BINDING_BLAKE3: &str =
    "d4194b2b26224e59dfcda9fa3853af96a119fd5015cd524cb39bdbfad9bb17bf";

/// roadmap-surfaces' text (1 932 bytes), check-binding's less
/// [`CHECK_BINDING_CLAUSE`]: its `PLANNED_BLAKE3` before
/// docs/features/check-binding.md.
const ROADMAP_BLAKE3: &str = "415d6cadd8d7f19d6a00d520d16c41cff8a78ec83c0f9fb3399dc942201de37e";

/// code-tools' text (1 892 bytes), roadmap-surfaces' less [`ROADMAP_READ`]:
/// its `PLANNED_BLAKE3` before docs/features/roadmap-surfaces.md.
const CODE_TOOLS_BLAKE3: &str = "3d172b74534282e8ad7fa14eb29f5e2b51e09f96970babe3145173c51fd1ad9c";

/// The BLAKE3 of [`PLANNED_INSTRUCTIONS`] less [`TASKS_LINE`] (1 902
/// bytes since docs/features/review-form.md); impact-since's 1 885 was
/// `381344b2…`, impact's 1 879 `523f0715…`, show-bindings' 1 839
/// `54d160f1…`.
const PLANNED_LESS_TASKS_BLAKE3: &str =
    "f0924769b09ff777ad983063624e1721f26a407d448fe5900b311324786ce1c1";

/// RD-5 (Q31): `INSTRUCTIONS` are the planned text byte for byte: 1 892
/// bytes, 1 754 less the tasks line (the spec's 1 755 is off by one: D1),
/// 1 968 with the probes paragraph, 80 left of 2 048; the code line once,
/// under "Reads:", right after the reads' line; `Hints use flags:`; every
/// tool named. M: a tool unnamed; the text changed, a pin kept.
/// docs/features/roadmap-surfaces.md RD-8 ("Data"): the reads' line gains
/// [`ROADMAP_READ`] before its full stop: 1 932 bytes, 1 794 less the
/// tasks line, 2 008 with the probes paragraph, 40 left.
/// docs/features/check-binding.md ("INSTRUCTIONS", Q-C): the code line
/// gains [`CHECK_BINDING_CLAUSE`] before its stop and the probes paragraph loses
/// ` (owner's checks)`: 1 978 bytes, 1 840 less the tasks line, 2 037 with
/// the 59-byte paragraph, 11 left (70 in the default build).
/// docs/features/show-bindings.md (RD-7, Q-F): the hints gain
/// [`BINDINGS_HINT`] right after `--links is with ["links"]` and the REF
/// paragraph loses [`NO_PROJECT_GLOSS`]: 1 977 bytes, 1 839 less the tasks
/// line, 2 036 with the 59-byte paragraph, 12 left (71 in the default
/// build). M: the hint or the cut missing; the text changed, a pin kept.
/// docs/features/impact.md ("INSTRUCTIONS", Q-B): the reads' line ends
/// with [`IMPACT_READ`] after [`ROADMAP_READ`] (312 bytes with its LF), the
/// `probes` paragraph gone: 2 017 bytes in both builds, 1 879 less the
/// tasks line, 31 left. M: INSTRUCTIONS over 2 048 bytes (the build's
/// asserts); the clause elsewhere; the text changed, a pin kept.
/// docs/features/impact-since.md ("MCP, INSTRUCTIONS, plugin 0.16.0"): the
/// impact clause reads `get_impact(id|symbol|since, depth?) = impact`
/// ([`SINCE_INPUT`], 6 bytes): 2 023 bytes, 1 885 less the tasks line, 25
/// left. M: `since` unnamed; the text changed, a pin kept.
/// docs/features/review-form.md (D6): [`REVIEW_LINE`] after the drift line,
/// the four glosses cut: 2 040 bytes, 1 902 less the tasks line, 8 left;
/// the text before it is impact-since's byte for byte. M: the line
/// elsewhere or unnamed; the text changed, a pin kept.
#[test]
fn rd5_instructions_are_the_planned_text() {
    let instructions = specengine_mcp::INSTRUCTIONS;
    assert_eq!(PLANNED_INSTRUCTIONS.len(), 2040, "review-form's figure");
    assert_eq!(REVIEW_LINE.len(), 85);
    assert_eq!(
        MARKDOWN_GLOSS.len() + CODE_GLOSS.len() + DRIFT_GLOSS.len() + TAIL_GLOSS.len(),
        68
    );
    assert_eq!(ROADMAP_READ.len(), 40);
    assert_eq!(IMPACT_READ.len(), 46);
    assert_eq!(SINCE_INPUT.len(), 6);
    assert_eq!(READS_LINE.len(), 318, "the spec's reads line");
    assert_eq!(CHECK_BINDING_CLAUSE.len(), 46);
    assert_eq!(BINDINGS_HINT.len(), 33);
    assert_eq!(NO_PROJECT_GLOSS.len(), 34);
    assert_eq!(CODE_LINE.len(), 151);
    assert_eq!(TASKS_LINE.len(), 138);
    assert_eq!(PROBES_PARAGRAPH.len(), 59);
    assert_eq!(instructions, PLANNED_INSTRUCTIONS);
    assert_eq!(instructions.len(), 2040);
    assert_eq!(2048 - instructions.len(), 8, "the room left");
    assert!(
        instructions.contains(&format!("never typed.\n{REVIEW_LINE}{TASKS_LINE}")),
        "the form's line between the drift line and the tasks line"
    );
    assert!(instructions.is_ascii());
    assert!(
        !instructions.contains("Probes build") && !instructions.contains(PROBES_PARAGRAPH),
        "no probes paragraph (Q-B)"
    );
    assert_eq!(
        instructions.matches(READS_LINE).count(),
        1,
        "the reads' line"
    );
    assert!(
        instructions.contains(
            "get_roadmap(release?, pool?) = roadmap; get_impact(id|symbol|since, depth?) = impact.\n"
        ),
        "the reads' line ends with the impact clause, exactly"
    );
    assert_eq!(instructions.matches(CODE_LINE).count(), 1);
    assert_eq!(instructions.matches(ROADMAP_READ).count(), 1);
    assert_eq!(instructions.matches(IMPACT_READ).count(), 1);
    assert_eq!(instructions.matches(CHECK_BINDING_CLAUSE).count(), 1);
    assert_eq!(instructions.matches(BINDINGS_HINT).count(), 1);
    assert!(
        !instructions.contains(NO_PROJECT_GLOSS) && !instructions.contains("spec init"),
        "the no-project gloss is cut: its error result names `spec init`"
    );
    assert!(
        instructions.contains(&format!(
            "unmapped(path?) = unmapped{CHECK_BINDING_CLAUSE}.\nQueue ("
        )),
        "check_binding ends the code line"
    );
    assert!(
        instructions.contains(&format!(
            "review --brief{ROADMAP_READ}{IMPACT_READ}.\n{CODE_LINE}Queue ("
        )),
        "the roadmap and impact reads end the reads' line; the code line closes the reads"
    );
    assert!(instructions.contains("Hints use flags: --kind is kinds"));
    assert!(!instructions.contains("CLI flags"));
    assert!(
        instructions.contains(&format!(
            "--links is with [\"links\"]{BINDINGS_HINT}. Answers are cut"
        )),
        "the bindings hint follows the links hint and ends the hints"
    );
    assert!(
        instructions.contains("A REF naming nothing or a call that cannot run is an error result"),
        "the cut joins the sentence"
    );
    for name in TOOLS {
        assert!(has_word(instructions, name), "{name} unnamed");
    }
    for name in CODE_TOOLS {
        assert!(instructions.contains(&format!("{name}(")), "{name}(");
    }
    assert!(instructions.contains("get_impact("), "get_impact(");
    let less = instructions.replacen(TASKS_LINE, "", 1);
    assert_eq!(less.len(), 1902, "2 040 less 138");
    // The text before docs/features/review-form.md: impact-since's 2 023.
    let impact_since = as_of_impact_since(instructions);
    assert_eq!(impact_since.len(), 2023);
    assert_eq!(
        blake3_hex(impact_since.as_bytes()),
        IMPACT_SINCE_BLAKE3,
        "only the form's line was added and the four glosses cut"
    );
    // The text before docs/features/impact-since.md: impact's 2 017.
    let impact = as_of_impact(&impact_since);
    assert_eq!(impact.len(), 2017);
    assert_eq!(
        blake3_hex(impact.as_bytes()),
        IMPACT_BLAKE3,
        "only `|since` was added"
    );
    // The text before docs/features/impact.md: show-bindings' 1 977.
    let show_bindings = as_of_show_bindings(&impact);
    assert_eq!(show_bindings.len(), 1977);
    assert_eq!(
        blake3_hex(show_bindings.as_bytes()),
        SHOW_BINDINGS_BLAKE3,
        "only the impact clause was added"
    );
    // The text before docs/features/show-bindings.md: check-binding's
    // 1 978.
    let check_binding = as_of_check_binding(&show_bindings);
    assert_eq!(check_binding.len(), 1978);
    assert_eq!(
        blake3_hex(check_binding.as_bytes()),
        CHECK_BINDING_BLAKE3,
        "only the bindings hint was added and the no-project gloss cut"
    );
    // The text before docs/features/check-binding.md: roadmap-surfaces'
    // 1 932.
    let roadmap = check_binding.replacen(CHECK_BINDING_CLAUSE, "", 1);
    assert_eq!(roadmap.len(), 1932);
    assert_eq!(
        blake3_hex(roadmap.as_bytes()),
        ROADMAP_BLAKE3,
        "only the check_binding clause was added"
    );
    // The text before docs/features/roadmap-surfaces.md: code-tools' 1 892,
    // both clauses stripped.
    let before = roadmap.replacen(ROADMAP_READ, "", 1);
    assert_eq!(before.len(), 1892);
    assert_eq!(
        blake3_hex(before.as_bytes()),
        CODE_TOOLS_BLAKE3,
        "only the roadmap read and the check_binding clause were added"
    );
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
    let impact_since = as_of_impact_since(PLANNED_INSTRUCTIONS);
    assert_eq!(blake3_hex(impact_since.as_bytes()), IMPACT_SINCE_BLAKE3);
    let impact = as_of_impact(&impact_since);
    assert_eq!(blake3_hex(impact.as_bytes()), IMPACT_BLAKE3);
    let show_bindings = as_of_show_bindings(&impact);
    assert_eq!(blake3_hex(show_bindings.as_bytes()), SHOW_BINDINGS_BLAKE3);
    assert_eq!(
        blake3_hex(as_of_check_binding(&show_bindings).as_bytes()),
        CHECK_BINDING_BLAKE3
    );
    assert_eq!(
        blake3_hex(
            as_of_check_binding(&show_bindings)
                .replacen(CHECK_BINDING_CLAUSE, "", 1)
                .as_bytes()
        ),
        ROADMAP_BLAKE3
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
/// 1, 2, bad arguments; `check_binding` since docs/features/check-binding.md
/// TE-3; `get_node` with `with: ["bindings"]` since
/// docs/features/show-bindings.md TE-5; `get_context_bundle` with its
/// code rows and `get_task` on a draft task of GOAL-01 since
/// docs/features/bundle-bindings.md TE-5) in both eras from a subdirectory;
/// then `git
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
    // docs/features/bundle-bindings.md TE-5: a draft task whose package
    // lists rows, made before the snapshots.
    let task = specengine_cli::task_new(
        &env,
        &Globals::default(),
        &specengine_cli::TaskNewRequest {
            nodes: vec!["GOAL-01".to_owned()],
            title: Some("Check the cap".to_owned()),
            goal: Some("Read the code bound to the objective.".to_owned()),
            author_role: None,
            author_model: None,
            run: None,
            now: "2026-10-21T09:00:00Z".to_owned(),
            git: git.clone(),
        },
    )
    .expect("task new")
    .id
    .expect("an ID");
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
        // docs/features/check-binding.md TE-3: exits 0 (observed, clean), 2
        // with a report and without one.
        ("check_binding", json!({"node": "PIECE-COUNTER"})),
        (
            "check_binding",
            json!({"node": "LAW-STEP", "symbol": "crates/tally::counter::Tally::step"}),
        ),
        (
            "check_binding",
            json!({"node": "PIECE-LIMITS", "symbol": null}),
        ),
        ("check_binding", json!({"node": "LAW-BOUND"})),
        ("check_binding", json!({"node": "LAW-NOPE"})),
        ("check_binding", json!({"node": "L\u{0410}W-STEP"})),
        // docs/features/show-bindings.md TE-5: `get_node` with its rows
        // judged (a document, a section, links too, none, naming nothing).
        (
            "get_node",
            json!({"id": "PIECE-COUNTER", "with": ["bindings"]}),
        ),
        (
            "get_node",
            json!({"id": "LAW-BOUND", "with": ["links", "bindings"], "archive": true}),
        ),
        (
            "get_node",
            json!({"id": "PIECE-LIMITS", "with": ["bindings"]}),
        ),
        ("get_node", json!({"id": "LAW-NOPE", "with": ["bindings"]})),
        // docs/features/bundle-bindings.md TE-5: bundles with their rows
        // judged (a document, two REFs, rows left out, naming nothing) and
        // the task's package with its rows.
        ("get_context_bundle", json!({"node_ids": ["PIECE-COUNTER"]})),
        (
            "get_context_bundle",
            json!({"node_ids": ["GOAL-01", "LAW-STEP"], "budget": 10000}),
        ),
        (
            "get_context_bundle",
            json!({"node_ids": ["LAW-BOUND"], "budget": 300}),
        ),
        ("get_context_bundle", json!({"node_ids": ["LAW-NOPE"]})),
        ("get_task", json!({"task_id": task})),
        ("get_task", json!({"next": true})),
    ];
    let invalid = [
        ("find_symbols", json!({"query": 5})),
        ("find_symbols", json!({"query": "step", "bogus": 1})),
        ("refs", json!({})),
        ("refs", json!({"id": ["LAW-STEP"]})),
        ("unmapped", json!({"path": 3})),
        ("check_binding", json!({})),
        ("check_binding", json!({"node": 7})),
        ("check_binding", json!({"node": "LAW-STEP", "bogus": true})),
        ("get_node", json!({"id": "LAW-STEP", "with": ["history"]})),
        ("get_context_bundle", json!({"node_ids": "GOAL-01"})),
        (
            "get_context_bundle",
            json!({"node_ids": ["GOAL-01"], "budget": "all"}),
        ),
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

// ------------------------------------------------- check-binding (6b-2b)

/// The golden lock (`fixtures/spec-lock`, format 2) and its format-1 twin.
const GOLDEN_LOCK: &str = "code-a.lock";
const GOLDEN_LOCK_FORMAT_1: &str = "code-a.format-1.lock";

/// code-a's files the check-binding tests edit.
const TALLY_COUNTER: &str = "crates/tally/src/counter.rs";
const TALLY_LIB: &str = "crates/tally/src/lib.rs";
const TALLY_BOUNDS: &str = "crates/tally/tests/bounds.rs";
const TALLY_LIMITS: &str = "data/limits.ron";
const TALLY_COUNTER_MD: &str = "docs/spec/counter.md";

/// GOAL-01 with a Cyrillic O: refused with its Latin fix.
const GOAL_LOOK_ALIKE: &str = "G\u{41E}AL-01";

/// `spec verify`'s run cause without `[code]` (docs/canon/code-bindings.md
/// "The `[code]` table"), as its document's `cannot` holds it.
const NO_CODE_CAUSE: &str =
    "specengine.toml has no [code] table: the code layer is off (add [code] with roots)";

/// The clock of the anchor repository's accepts.
const ACCEPT_CLOCK: &str = "2026-10-08T09:30:00Z";

/// The nine row states (docs/canon/spec-verify.md "States").
const VERIFY_STATES: [&str; 9] = [
    "cannot_verify",
    "predated",
    "bad_marker",
    "broken_binding",
    "unlocked",
    "spec_ahead",
    "code_ahead",
    "conflict",
    "ok",
];

/// The top keys of verify's document (14; `spec-verify-output.md`;
/// `tested` after `delta` since docs/features/tested-evidence.md
/// "Plain verify", **Mirror**), the keys of a row (17), of a binding key
/// and of an anchor's place (docs/features/check-binding.md "Data",
/// **Mirror**).
const VERIFY_TOP_KEYS: [&str; 14] = [
    "mode", "base", "task", "node", "symbol", "verdict", "counts", "cascade", "rows", "delta",
    "tested", "cannot", "notes", "omitted",
];
/// A `tested` entry's keys (docs/features/tested-evidence.md **Mirror**).
const VERIFY_TESTED_KEYS: [&str; 7] = [
    "node",
    "state",
    "category",
    "rev",
    "node_rev",
    "commit",
    "verified_at",
];
const VERIFY_ROW_KEYS: [&str; 17] = [
    "node",
    "relation",
    "file",
    "kind",
    "symbol",
    "item_kind",
    "state",
    "category",
    "detail",
    "class",
    "pre_existing",
    "markers",
    "node_rev",
    "lock",
    "compared",
    "levels",
    "anchor",
];
const VERIFY_KEY_KEYS: [&str; 6] = ["node", "relation", "file", "kind", "symbol", "item_kind"];
const VERIFY_PLACE_KEYS: [&str; 4] = ["file", "kind", "symbol", "item_kind"];

/// The row keys `null` on some row and set on another over the corpora of
/// `check_binding_documents_meet_every_key` (the walker meets both members
/// of each union).
const VERIFY_NULLABLE: [&str; 11] = [
    "kind",
    "symbol",
    "item_kind",
    "category",
    "detail",
    "class",
    "node_rev",
    "lock",
    "compared",
    "levels",
    "anchor",
];

/// A git repository of `fixtures/<fixture>` at `<scratch>/<dir>` (copied
/// in reverse name order when `reversed`), `edit` applied, committed
/// clean by the sandbox's git (its own `HOME`, fixed identity); the root,
/// canonical.
fn git_copy(
    scratch: &Scratch,
    fixture_name: &str,
    dir: &str,
    reversed: bool,
    edit: impl Fn(&Path),
) -> PathBuf {
    let root = scratch.join(dir);
    copy_dir(&fixture(fixture_name), &root, reversed);
    let root = root.canonicalize().expect("a root");
    edit(&root);
    let sandbox = Sandbox::new(scratch.path());
    sandbox.init(&root);
    sandbox.add_all(&root);
    sandbox.commit(&root, &format!("{fixture_name} as committed"));
    root
}

/// `fixtures/spec-lock/<lock>` written as `root`'s `spec.lock`.
fn with_lock(lock: &str) -> impl Fn(&Path) {
    let bytes = fs::read(fixture("spec-lock").join(lock)).expect("the golden lock");
    move |root: &Path| fs::write(root.join("spec.lock"), &bytes).expect("spec.lock")
}

/// `[verify] mode = "<mode>"` appended to the root's config (written,
/// never committed).
fn set_verify_mode(root: &Path, mode: &str) {
    let config = read_text(root, "specengine.toml");
    write(
        root,
        "specengine.toml",
        format!("{config}\n[verify]\nmode = \"{mode}\"\n"),
    );
}

/// verify_node.rs' `drift()` over the golden: `CAP` and the RON cap
/// edited, LAW-BOUND's text edited, PIECE-COUNTER's `//!` marker in the
/// test file removed: every state of "States" met (the golden holds the
/// others).
fn drift_golden(root: &Path) {
    replace(root, TALLY_COUNTER, "CAP: u32 = 10;", "CAP: u32 = 12;");
    replace(root, TALLY_LIMITS, "cap: 10,", "cap: 12,");
    replace(
        root,
        TALLY_COUNTER_MD,
        "The count never exceeds the cap.",
        "The count never exceeds the cap, ever.",
    );
    replace(root, TALLY_BOUNDS, "//! @verifies PIECE-COUNTER\n", "");
}

impl Project {
    /// `fixtures/code-a` with `fixtures/spec-lock/<lock>` committed as
    /// `spec.lock`, a git repository, its own `HOME`.
    fn golden(lock: &str) -> Self {
        let scratch = Scratch::new("cb-golden");
        let home = scratch.home("h");
        let root = git_copy(&scratch, "code-a", "repo", false, with_lock(lock));
        Self {
            _scratch: scratch,
            home,
            root,
        }
    }

    /// `fixtures/anchor-states` committed, `nodes` accepted through the
    /// CLI library (one lock commit each, format 2, consent yes), then the
    /// renames of TE-1's scenarios "rename within the file" (KITE-RENAME:
    /// a moved anchor) and "twins, equal quotes, both renamed"
    /// (KITE-CLONE: ambiguous anchors) written, uncommitted.
    fn anchors(nodes: &[&str]) -> Self {
        let scratch = Scratch::new("cb-anchors");
        let home = scratch.home("h");
        let root = git_copy(&scratch, "anchor-states", "repo", false, |_| {});
        let sandbox = Sandbox::new(scratch.path());
        let env = cli_env(&root, Some(&home));
        for node in nodes {
            let mut yes = |_: &LockPrompt<'_>| true;
            lock_accept(
                &env,
                &Globals::default(),
                &LockAcceptRequest {
                    reference: (*node).to_owned(),
                    editorial: false,
                    now: ACCEPT_CLOCK.to_owned(),
                    git: GitEnv::new(&root, sandbox.vars()),
                    proposal: None,
                    branch: None,
                },
                &mut yes,
                &[],
            )
            .unwrap_or_else(|error| panic!("lock accept {node}: {error}"));
        }
        assert_eq!(porcelain(&sandbox, &root), b"", "the accepts committed");
        replace(
            &root,
            "crates/sky/src/rename.rs",
            "pub fn lift_force(",
            "pub fn lift_power(",
        );
        replace(
            &root,
            "crates/sky/src/clones.rs",
            "pub fn gust_left(",
            "pub fn gust_west(",
        );
        replace(
            &root,
            "crates/sky/src/clones.rs",
            "pub fn gust_right(",
            "pub fn gust_east(",
        );
        Self {
            _scratch: scratch,
            home,
            root,
        }
    }
}

/// docs/features/check-binding.md "Data", **Description**: what the agent
/// mends and what the owner settles, exactly (437 bytes; no state or
/// class word).
const CHECK_BINDING_MEND: &str = "Mend what your change broke: a binding that no longer reaches its code, spec \
and code both changed, a bad marker, a marker naming a revision its node has \
not reached, or a binding it cannot verify; else raise it with \
report_discrepancy. The owner settles code ahead of its spec (as after \
implementing), a spec ahead of its code and a binding not yet locked: name \
them in your report, never hide them. Zero rows is a result, not an error.";

/// docs/features/check-binding.md "Data", **Mirror**: `node`'s doc
/// comment, its `outputSchema` description, exactly.
const CHECK_BINDING_NODE_DOC: &str =
    "The IDs (a holder without one: its citation) of the nodes the REF names, in holder order.";

/// `check_binding` with `args`, its verdict wanted.
fn bound(label: &'static str, args: Value, verdict: &'static str) -> Call {
    call(label, "check_binding", args, Want::Verdict(verdict))
}

/// `check_binding` with `args`, exit 2 without a document wanted.
fn unbound(label: &'static str, args: Value) -> Call {
    call(label, "check_binding", args, Want::CannotRun)
}

/// The `check_binding` output schema of a fresh server.
fn check_binding_schema() -> Value {
    let mut session = Session::open(Era::Legacy, &[], None, Home::Fresh);
    let list = session.tools();
    let done = session.finish();
    assert!(done.status.success(), "{}", done.stderr);
    output_schemas(&list)
        .remove("check_binding")
        .expect("check_binding has an outputSchema")
}

/// docs/features/check-binding.md RD-1 and "Data": `check_binding` is
/// listed in both eras as a read (the reads' annotations, `_meta`, an
/// object `outputSchema` without `$ref`); its input is exactly `{"node":
/// string, "symbol": string|null}`, `node` required, nothing else; its
/// description is at most 2 048 ASCII bytes holding the determinism
/// sentence, its CLI twin, `[code]`, "this session's worktree", that it
/// writes nothing under the root, the base it lacks (`enforce-introduced`
/// as `enforce`), a finding as no failure, and states the cut as the
/// CLI's. M: a writer's hints; `symbol` required or `task` offered; the
/// description without the worktree or the base.
#[test]
fn check_binding_is_listed_as_a_read() {
    for era in ERAS {
        let mut session = Session::open(era, &[], None, Home::Fresh);
        let list = session.tools();
        assert_eq!(tool_names(&list), TOOLS, "{era:?}");
        let listed = tool(&list, "check_binding");
        assert_eq!(
            listed["annotations"],
            json!({"readOnlyHint": true, "destructiveHint": false, "openWorldHint": false}),
            "{era:?}: the reads' annotations"
        );
        assert_eq!(
            listed["_meta"],
            json!({"anthropic/maxResultSizeChars": specengine_mcp::MAX_RESULT_CHARS}),
            "{era:?}: _meta"
        );
        assert_eq!(listed["outputSchema"]["type"], json!("object"));
        assert!(!listed.to_string().contains("\"$ref\""), "{era:?}: a $ref");
        let input = &listed["inputSchema"];
        assert_eq!(input["type"], json!("object"), "{era:?}");
        assert_eq!(input["additionalProperties"], json!(false), "{era:?}");
        assert_eq!(input["required"], json!(["node"]), "{era:?}");
        assert_eq!(keys_at(input), set(&["node", "symbol"]), "{era:?}: {input}");
        assert_eq!(types_of(&input["properties"]["node"]), set(&["string"]));
        assert_eq!(
            types_of(&input["properties"]["symbol"]),
            set(&["null", "string"])
        );
        let description = listed["description"].as_str().expect("a description");
        assert!(description.len() <= 2048, "{} bytes", description.len());
        assert!(description.is_ascii(), "{era:?}: not ASCII");
        for part in [
            DETERMINISM,
            "spec verify --node",
            "[code]",
            "this session's worktree",
            "writes nothing under the project root",
            "enforce-introduced",
            "finding",
        ] {
            assert!(
                description.contains(part),
                "{era:?}: no {part:?} in {description:?}"
            );
        }
        assert_eq!(CHECK_BINDING_MEND.len(), 437, "the spec's figure");
        assert_eq!(
            description.matches(CHECK_BINDING_MEND).count(),
            1,
            "{era:?}: the mend paragraph exactly: {description:?}"
        );
        let mut texts = vec![description.to_owned()];
        descriptions(input, &mut texts);
        assert!(
            !cut_numbers(description).is_empty(),
            "{era:?}: no cut stated: {description:?}"
        );
        for text in &texts {
            assert!(text.is_ascii(), "{era:?}: {text:?}");
            for number in cut_numbers(text) {
                assert_eq!(number, OUTPUT_CAP_CHARS, "{era:?}: {text:?}");
            }
        }
        let done = session.finish();
        assert!(done.status.success(), "{era:?}: {}", done.stderr);
    }
}

/// RD-7 (M8), docs/features/check-binding.md "Data", the exit table, both
/// eras, a scratch git repository of `fixtures/code-a` with the golden
/// lock: every answer is the CLI library's `spec verify --node REF
/// [--symbol QPATH]` (content `2>&1`, `structuredContent` its `--json`
/// document, walked against the `outputSchema`) and its exit's shape:
///
/// - exit 0: `observed` (a node, its document path, `slug/ID`, a module
///   QPATH, `symbol: null`) and `clean` (no binding: the note; a QPATH
///   matching no row);
/// - exit 1 under `enforce`: `blocked`, an error result with its document
///   (a finding, not a failure); `enforce-introduced` judged as `enforce`
///   (no base);
/// - exit 2 with a report, its document kept: a Cannot row (an ID, an
///   alias, `ID#SECTION`, a RON `<file>#<path>` QPATH), a walk gap (the
///   CLI's `cannot:` line naming the file; readable again, the next call
///   answers), a format-1 lock;
/// - exit 2 without one: a REF naming nothing, a look-alike (its Latin
///   fix), `project:`, an empty REF, discovery (no `specengine.toml`);
///   schema refusals (`node` absent or no string, `symbol` no string, an
///   unknown `task`): rmcp's `Parameters` error results.
///
/// M8: a rendered exit-2 report answered as `CannotRun`.
#[test]
fn check_binding_parity_per_exit() {
    let project = Project::golden(GOLDEN_LOCK);
    let observe = [
        bound("observed", json!({"node": "PIECE-COUNTER"}), "observed"),
        bound(
            "its document",
            json!({"node": TALLY_COUNTER_MD}),
            "observed",
        ),
        bound(
            "symbol null",
            json!({"node": "PIECE-COUNTER", "symbol": null}),
            "observed",
        ),
        bound(
            "a module",
            json!({"node": "PIECE-COUNTER", "symbol": "crates/tally::counter"}),
            "observed",
        ),
        bound(
            "no row",
            json!({"node": "PIECE-COUNTER", "symbol": "crates/tally::nowhere"}),
            "clean",
        ),
        bound("no binding", json!({"node": "PIECE-LIMITS"}), "clean"),
        bound("slug/ID", json!({"node": "cap-check/PASS-01"}), "observed"),
        bound(
            "a cannot row",
            json!({"node": "LAW-BOUND"}),
            "cannot-verify",
        ),
        bound("an alias", json!({"node": "AIM-01"}), "cannot-verify"),
        bound(
            "a section",
            json!({"node": "PIECE-COUNTER#LAW-BOUND"}),
            "cannot-verify",
        ),
        bound(
            "a RON symbol",
            json!({"node": "LAW-STEP", "symbol": "data/limits.ron#root.step"}),
            "cannot-verify",
        ),
        bound(
            "a bound qpath",
            json!({"node": "LAW-STEP", "symbol": "crates/tally::counter::Tally::step"}),
            "observed",
        ),
        bound(
            "nope",
            json!({"node": "LAW-STEP", "symbol": "nope"}),
            "clean",
        ),
        bound(
            "several holders",
            json!({"node": "PREM-001"}),
            "cannot-verify",
        ),
        unbound("unknown", json!({"node": "LAW-NOPE"})),
        unbound("look-alike", json!({"node": GOAL_LOOK_ALIKE})),
        unbound("a law look-alike", json!({"node": "L\u{0410}W-STEP"})),
        unbound("project:", json!({"node": "project:PIECE-COUNTER"})),
        unbound("empty", json!({"node": ""})),
    ];
    // A second holder of PREM-001, uncommitted: verify judges the
    // worktree.
    let premise = read_text(&project.root, "docs/records/PREM-001.md");
    write(&project.root, "docs/records/PREM-001-twin.md", &premise);
    for results in project.answers(&observe) {
        let doc = |label: &str| document(&observe, &results, label);
        let text = |label: &str| content_text(pick(&observe, &results, label));
        for (at, call) in observe.iter().enumerate() {
            if let Want::Verdict(_) = call.want {
                let document = &results[at]["structuredContent"];
                assert_eq!(document["mode"], json!("observe"), "{}", call.label);
                assert!(document["node"].is_array(), "{}: {document}", call.label);
                assert_eq!(document["symbol"], call.args["symbol"], "{}", call.label);
                assert!(document["omitted"]["rows"].is_u64(), "{}", call.label);
            }
        }
        assert_eq!(doc("observed")["node"], json!(["PIECE-COUNTER"]));
        assert_eq!(doc("an alias")["node"], json!(["GOAL-01"]));
        assert!(
            text("no binding").contains("note: verify: PIECE-LIMITS has no binding\n"),
            "{:?}",
            text("no binding")
        );
        assert!(
            doc("a cannot row")["counts"]["cannot"].as_u64() >= Some(1),
            "{}",
            doc("a cannot row")
        );
        assert!(
            text("look-alike").contains("write `GOAL-01`"),
            "the Latin fix: {:?}",
            text("look-alike")
        );
        assert!(
            text("unknown").starts_with("spec: `LAW-NOPE`"),
            "{:?}",
            text("unknown")
        );
        assert!(
            text("a law look-alike").contains("write `LAW-STEP`"),
            "{:?}",
            text("a law look-alike")
        );
        assert_eq!(doc("nope")["rows"], json!([]));
        assert!(
            text("nope").contains("note: verify: LAW-STEP has no binding on nope\n"),
            "zero rows, the note: {:?}",
            text("nope")
        );
        assert_eq!(doc("a bound qpath")["counts"]["rows"], json!(1));
        // Several holders: the CLI's one `warning:` line first, `node`
        // every holder.
        assert!(
            text("several holders").starts_with("warning: `PREM-001` has 2 holders"),
            "{:?}",
            text("several holders")
        );
        assert_eq!(text("several holders").matches("warning: ").count(), 1);
        assert_eq!(
            doc("several holders")["node"],
            json!(["PREM-001", "PREM-001"])
        );
    }
    fs::remove_file(project.root.join("docs/records/PREM-001-twin.md")).expect("the twin");

    // Exit 1: a Fix row under `enforce`, and under `enforce-introduced`
    // with no base.
    let config = read_text(&project.root, "specengine.toml");
    for mode in ["enforce", "enforce-introduced"] {
        set_verify_mode(&project.root, mode);
        let calls = [
            bound("blocked", json!({"node": "PIECE-COUNTER"}), "blocked"),
            bound(
                "blocked slug/ID",
                json!({"node": "cap-check/PASS-01"}),
                "blocked",
            ),
            bound(
                "its module only",
                json!({"node": "PIECE-COUNTER", "symbol": "crates/tally::counter"}),
                "observed",
            ),
            bound("clean", json!({"node": "PIECE-LIMITS"}), "clean"),
            bound(
                "a cannot row",
                json!({"node": "LAW-BOUND"}),
                "cannot-verify",
            ),
            unbound("unknown", json!({"node": "LAW-NOPE"})),
        ];
        for results in project.answers(&calls) {
            let blocked = document(&calls, &results, "blocked");
            // No base: `enforce-introduced` judged as `enforce`, with the
            // CLI's note.
            assert_eq!(blocked["mode"], json!("enforce"), "{blocked}");
            assert_eq!(
                blocked["notes"]
                    .as_array()
                    .expect("notes")
                    .iter()
                    .any(|note| note.as_str().is_some_and(|note| {
                        note.contains("enforce-introduced") && note.contains("judged as `enforce`")
                    })),
                mode == "enforce-introduced",
                "{mode}: {blocked}"
            );
            assert!(blocked["counts"]["fix"].as_u64() >= Some(1), "{blocked}");
        }
        write(&project.root, "specengine.toml", &config);
    }
    // Exit 1: a locked binding gone (PIECE-COUNTER's `//!` marker in the
    // test file removed: `marker_gone`, a Fix row), under `enforce`.
    set_verify_mode(&project.root, "enforce");
    let bounds = read_text(&project.root, TALLY_BOUNDS);
    replace(
        &project.root,
        TALLY_BOUNDS,
        "//! @verifies PIECE-COUNTER\n",
        "",
    );
    let calls = [bound(
        "a binding gone",
        json!({"node": "PIECE-COUNTER", "symbol": "crates/tally::test:bounds"}),
        "blocked",
    )];
    for results in project.answers(&calls) {
        let rows = &document(&calls, &results, "a binding gone")["rows"];
        assert_eq!(rows[0]["state"], json!("broken_binding"), "{rows}");
        assert_eq!(rows[0]["class"], json!("fix"), "{rows}");
    }
    write(&project.root, TALLY_BOUNDS, &bounds);
    write(&project.root, "specengine.toml", &config);

    // Exit 2, a report: a walk gap, then healed.
    let gap = project.root.join(TALLY_LIB);
    fs::set_permissions(&gap, fs::Permissions::from_mode(0o000)).expect("chmod");
    let walk = [
        bound(
            "walk gap",
            json!({"node": "PIECE-COUNTER"}),
            "cannot-verify",
        ),
        bound(
            "walk gap, a symbol",
            json!({"node": "LAW-BOUND", "symbol": "crates/tally::counter::CAP"}),
            "cannot-verify",
        ),
        unbound("walk gap, unknown", json!({"node": "LAW-NOPE"})),
    ];
    for results in project.answers(&walk) {
        for label in ["walk gap", "walk gap, a symbol"] {
            let text = content_text(pick(&walk, &results, label));
            assert!(
                text.contains("cannot: the code walk is incomplete: `crates/tally/src/lib.rs`"),
                "{label}: {text:?}"
            );
            let cannot = &document(&walk, &results, label)["cannot"];
            assert!(
                cannot[0]
                    .as_str()
                    .is_some_and(|line| line.starts_with("the code walk is incomplete: ")),
                "{label}: {cannot}"
            );
            assert_eq!(
                document(&walk, &results, label)["rows"],
                json!([]),
                "{label}: no row judged under a run cause"
            );
        }
    }
    fs::set_permissions(&gap, fs::Permissions::from_mode(0o644)).expect("chmod");
    project.answers(&[bound(
        "healed",
        json!({"node": "PIECE-COUNTER"}),
        "observed",
    )]);

    // Exit 2, a report: a format-1 lock.
    let old = Project::golden(GOLDEN_LOCK_FORMAT_1);
    let calls = [
        bound(
            "format 1",
            json!({"node": "PIECE-COUNTER"}),
            "cannot-verify",
        ),
        unbound("format 1, unknown", json!({"node": "LAW-NOPE"})),
    ];
    for results in old.answers(&calls) {
        let text = content_text(pick(&calls, &results, "format 1"));
        assert!(
            text.contains("cannot: spec.lock:2: format 1"),
            "the lock's run cause: {text:?}"
        );
        assert_eq!(
            document(&calls, &results, "format 1")["rows"],
            json!([]),
            "no row judged under a run cause"
        );
    }

    // Exit 2 without a document: discovery; and the schema's refusals.
    let scratch = Scratch::new("cb-nowhere");
    let home = scratch.home("h");
    let nowhere = scratch.dir("nowhere");
    let env = cli_env(&nowhere, Some(&home));
    let globals = Globals::default();
    for era in ERAS {
        let mut session = Session::open(era, &[], Some(&nowhere), Home::At(&home));
        let schemas = output_schemas(&session.tools());
        for args in [
            json!({"node": "PIECE-COUNTER"}),
            json!({"node": "PIECE-COUNTER", "symbol": "crates/tally::counter"}),
        ] {
            let result = checked_call(
                &mut session,
                &schemas,
                "check_binding",
                args.clone(),
                &env,
                &globals,
            );
            let context = format!("{era:?} discovery {args}");
            Want::CannotRun.check(&result, &context);
            assert!(
                content_text(&result).contains("no specengine.toml"),
                "{context}: {}",
                content_text(&result)
            );
        }
        for (args, needle) in [
            (json!({}), "node"),
            (json!({"symbol": "crates/tally::counter"}), "node"),
            (json!({"node": 7}), ""),
            (json!({"node": ["PIECE-COUNTER"]}), ""),
            (json!({"node": "PIECE-COUNTER", "symbol": 3}), ""),
            (json!({"node": "PIECE-COUNTER", "task": "T-0001"}), "task"),
        ] {
            let reply = session.call("check_binding", args.clone());
            assert_bad_arguments(&reply, needle, &format!("{era:?} {args}"));
        }
        let done = session.finish();
        assert!(done.status.success(), "{era:?}: {}", done.stderr);
        assert_eq!(done.stderr, "", "{era:?}");
    }
}

/// TE-2, the mirror (docs/features/check-binding.md "Data", **Mirror**):
/// the output schema's key sets are verify's document, named here (a key
/// dropped from both the mirror and the CLI still fails): 13 top keys, 17
/// row keys, `counts`, `cascade`, `delta` with its `bindings`, a binding
/// key and an anchor's place, `levels`, `anchor`, `markers[]`, `lock`,
/// `omitted {rows}`; `base`, `task`, `delta.symbols` and
/// `delta.tests_reaching` exactly `{"type": "null"}` (`AlwaysNull`); the
/// CLI's closed words enums (`mode`, `verdict`, `relation`, `kind` with
/// `null`, the nine states, `class`, `compared`, an anchor's `state`);
/// `item_kind`, `category`, `detail` and `levels` items free strings.
/// M: `kind` free or without `null`; a state missing; `task` typed; a
/// row key dropped.
#[test]
fn check_binding_mirror_is_verify_json() {
    let schema = check_binding_schema();
    assert_eq!(keys_at(&schema), set(&VERIFY_TOP_KEYS));
    for path in ["base", "task", "delta.symbols", "delta.tests_reaching"] {
        let at = schema_at(&schema, path);
        let mut bare = at.clone();
        if let Some(object) = bare.as_object_mut() {
            object.remove("description");
        }
        assert_eq!(bare, json!({"type": "null"}), "{path}: AlwaysNull: {at}");
    }
    let types = |path: &str| types_of(schema_at(&schema, path));
    let words = |path: &str| enum_values(schema_at(&schema, path));
    assert_eq!(types("node"), set(&["array"]), "node: the holders' IDs");
    assert_eq!(
        schema_at(&schema, "node")["description"],
        json!(CHECK_BINDING_NODE_DOC),
        "node's doc"
    );
    assert_eq!(types("node[]"), set(&["string"]));
    assert_eq!(types("symbol"), set(&["null", "string"]));
    assert_eq!(
        words("mode"),
        Some(set(&["observe", "enforce-introduced", "enforce"]))
    );
    assert!(types("mode").contains("null"), "mode before the config");
    assert_eq!(
        words("verdict"),
        Some(set(&["clean", "observed", "blocked", "cannot-verify"]))
    );
    assert_eq!(
        keys_at(schema_at(&schema, "counts")),
        set(&[
            "rows",
            "ok",
            "fix",
            "check",
            "cannot",
            "pre_existing",
            "skipped"
        ])
    );
    assert_eq!(
        keys_at(schema_at(&schema, "cascade")),
        set(&["by_blob", "by_ast"])
    );
    for path in ["counts.rows", "counts.skipped", "cascade.by_ast"] {
        assert_eq!(types(path), set(&["integer"]), "{path}");
    }
    for path in ["cannot", "notes"] {
        assert_eq!(types(path), set(&["array"]), "{path}");
        assert_eq!(types(&format!("{path}[]")), set(&["string"]), "{path}");
    }
    assert_eq!(keys_at(schema_at(&schema, "omitted")), set(&["rows"]));
    assert_eq!(types("omitted.rows"), set(&["integer"]));

    assert_eq!(keys_at(schema_at(&schema, "rows[]")), set(&VERIFY_ROW_KEYS));
    assert_eq!(
        words("rows[].relation"),
        Some(set(&["implements", "verifies", "configures", "assumes"]))
    );
    assert_eq!(words("rows[].kind"), Some(set(&["item", "module", "ron"])));
    assert!(types("rows[].kind").contains("null"), "a keyless row");
    assert_eq!(words("rows[].state"), Some(set(&VERIFY_STATES)));
    assert_eq!(
        words("rows[].class"),
        Some(set(&["fix", "check", "cannot"]))
    );
    assert!(types("rows[].class").contains("null"), "an ok row");
    assert_eq!(words("rows[].compared"), Some(set(&["blob", "ast"])));
    assert!(types("rows[].compared").contains("null"));
    for path in [
        "rows[].item_kind",
        "rows[].category",
        "rows[].detail",
        "rows[].levels.lock[]",
        "rows[].levels.markers[]",
        "rows[].levels.changed[]",
    ] {
        assert_eq!(words(path), None, "{path}: an enum");
        assert!(types(path).contains("string"), "{path}: not a string");
    }
    for path in [
        "rows[].symbol",
        "rows[].item_kind",
        "rows[].category",
        "rows[].detail",
        "rows[].node_rev",
        "rows[].lock",
        "rows[].levels",
        "rows[].levels.changed",
        "rows[].anchor",
        "rows[].anchor.from",
        "rows[].anchor.candidates",
        "rows[].markers[].rev",
    ] {
        assert!(types(path).contains("null"), "{path}: not nullable");
    }
    assert_eq!(types("rows[].pre_existing"), set(&["boolean"]));
    assert!(types("rows[].node_rev").contains("integer"));
    assert_eq!(
        keys_at(schema_at(&schema, "rows[].markers[]")),
        set(&["line", "rev"])
    );
    assert_eq!(
        keys_at(schema_at(&schema, "rows[].lock")),
        set(&["rev", "commit", "verified_at"])
    );
    assert_eq!(
        keys_at(schema_at(&schema, "rows[].levels")),
        set(&["lock", "markers", "changed"])
    );
    assert_eq!(
        keys_at(schema_at(&schema, "rows[].anchor")),
        set(&["state", "from", "candidates"])
    );
    assert_eq!(
        words("rows[].anchor.state"),
        Some(set(&["exact", "moved", "ambiguous", "lost"]))
    );
    for path in ["rows[].anchor.from", "rows[].anchor.candidates[]"] {
        assert_eq!(
            keys_at(schema_at(&schema, path)),
            set(&VERIFY_PLACE_KEYS),
            "{path}"
        );
        assert_eq!(
            words(&format!("{path}.kind")),
            Some(set(&["item", "module", "ron"])),
            "{path}.kind"
        );
    }

    assert_eq!(
        keys_at(schema_at(&schema, "delta")),
        set(&["symbols", "bindings", "affected_nodes", "tests_reaching"])
    );
    assert_eq!(
        keys_at(schema_at(&schema, "delta.bindings")),
        set(&["moved", "ambiguous", "lost", "stale"])
    );
    assert_eq!(
        keys_at(schema_at(&schema, "delta.bindings.moved[]")),
        set(&["from", "to"])
    );
    for path in [
        "delta.bindings.moved[].from",
        "delta.bindings.moved[].to",
        "delta.bindings.ambiguous[]",
        "delta.bindings.lost[]",
        "delta.bindings.stale[]",
    ] {
        assert_eq!(
            keys_at(schema_at(&schema, path)),
            set(&VERIFY_KEY_KEYS),
            "{path}"
        );
    }
    assert_eq!(types("delta.affected_nodes[]"), set(&["string"]));

    // docs/features/tested-evidence.md **Mirror**: `tested` a nullable
    // array of `{node, state, category, rev, node_rev, commit,
    // verified_at}`, every key required; `state` the closed enum;
    // `category` a nullable free string; `node_rev` nullable.
    assert_eq!(types("tested"), set(&["array", "null"]), "tested");
    assert_eq!(
        keys_at(schema_at(&schema, "tested[]")),
        set(&VERIFY_TESTED_KEYS)
    );
    let item = schema_at(&schema, "tested[]");
    let required: BTreeSet<String> = members(item)
        .iter()
        .filter_map(|member| member.get("required").and_then(Value::as_array))
        .flatten()
        .filter_map(|key| key.as_str().map(str::to_owned))
        .collect();
    assert_eq!(required, set(&VERIFY_TESTED_KEYS), "every key required");
    assert_eq!(
        words("tested[].state"),
        Some(set(&["verified", "stale", "unverifiable", "node_gone"]))
    );
    assert_eq!(words("tested[].category"), None, "category: a free string");
    assert_eq!(types("tested[].category"), set(&["null", "string"]));
    assert_eq!(types("tested[].node_rev"), set(&["integer", "null"]));
    assert_eq!(types("tested[].rev"), set(&["integer"]));
    for path in ["tested[].node", "tested[].commit", "tested[].verified_at"] {
        assert_eq!(types(path), set(&["string"]), "{path}");
    }
}

/// The golden with a format-3 lock: its entries, then blocks for
/// PIECE-COUNTER (at its revision 3, a made-up text: `stale
/// spec_changed`), LAW-BOUND (at another revision) and a gone node.
fn golden_with_blocks() -> Project {
    let golden = fs::read_to_string(fixture("spec-lock").join(GOLDEN_LOCK)).expect("golden");
    let block = |node: &str, rev: u32, digit: char| {
        let hash = format!("b3:{}", digit.to_string().repeat(64));
        format!(
            "\n[[tested]]\nnode = \"{node}\"\nrev = {rev}\nspec_hash = \"{hash}\"\nbound_hash = \"{hash}\"\ntests = [\n  {{ file = \"crates/tally/tests/bounds.rs\", symbol = \"crates/tally::test:bounds::stops_at_cap\" }},\n]\ncommit = \"{}\"\nverified_at = \"2026-10-21\"\n",
            digit.to_string().repeat(40)
        )
    };
    let mut text = golden.replacen("\nformat = 2\n", "\nformat = 3\n", 1);
    text.push_str(&block("GONE-77", 1, '7'));
    text.push_str(&block("LAW-BOUND", 9, '8'));
    text.push_str(&block("PIECE-COUNTER", 3, '9'));
    let scratch = Scratch::new("cb-tested");
    let home = scratch.home("h");
    let root = git_copy(&scratch, "code-a", "repo", false, move |root: &Path| {
        fs::write(root.join("spec.lock"), &text).expect("spec.lock")
    });
    Project {
        _scratch: scratch,
        home,
        root,
    }
}

/// docs/features/tested-evidence.md RD-8 and TE-5 (M: `VerifyDocument`
/// without `tested`, or `check_binding` and `spec verify --node --json`
/// disagreeing): over a format-3 lock, every `check_binding` answer is the
/// CLI library's `verify --node` document (parity, both eras) and conforms
/// to the schema's exact key sets; `tested` lists the holders' blocks
/// (PIECE-COUNTER's, LAW-BOUND's), each entry the seven keys, its `state`
/// in the enum; a node without a block `[]`; the gone node's block never
/// named by a holder's call.
#[test]
fn check_binding_tested_is_verify_nodes() {
    let project = golden_with_blocks();
    let calls: Vec<Call> = ["PIECE-COUNTER", "LAW-BOUND", "GOAL-01"]
        .iter()
        .map(|node| {
            call(
                "tested",
                "check_binding",
                json!({"node": node}),
                Want::Parity,
            )
        })
        .collect();
    for results in project.answers(&calls) {
        let documents: Vec<&Value> = results
            .iter()
            .map(|result| &result["structuredContent"])
            .collect();
        let nodes = |document: &Value| -> Vec<String> {
            document["tested"]
                .as_array()
                .unwrap_or_else(|| panic!("`tested` an array: {document}"))
                .iter()
                .map(|entry| {
                    let keys: BTreeSet<String> = entry
                        .as_object()
                        .expect("an entry")
                        .keys()
                        .cloned()
                        .collect();
                    assert_eq!(keys, set(&VERIFY_TESTED_KEYS), "{entry}");
                    assert!(
                        ["verified", "stale", "unverifiable", "node_gone"]
                            .contains(&entry["state"].as_str().unwrap_or_default()),
                        "{entry}"
                    );
                    entry["node"].as_str().expect("a node").to_owned()
                })
                .collect()
        };
        assert_eq!(nodes(documents[0]), ["PIECE-COUNTER"]);
        assert_eq!(nodes(documents[1]), ["LAW-BOUND"]);
        assert_eq!(nodes(documents[2]), Vec::<String>::new(), "no block: []");
        assert_eq!(documents[0]["tested"][0]["rev"], json!(3));
        assert_eq!(documents[1]["tested"][0]["rev"], json!(9));
    }
}

/// docs/features/tested-evidence.md, 8b-1 iteration 2 (owner-approved): a
/// failure while hashing the evidence's bound code never changes
/// `check_binding`'s answer. Over a scratch copy of `fixtures/verify-tests`
/// whose lock holds a block and no entry, the server run with a `git` first
/// on `PATH` that refuses `hash-object` answers `isError` false, `tested`
/// `null`, one `tested: <cause>` note (in `notes` and the text). M: the
/// failure a run cause (`cannot-verify`, an error result).
#[test]
fn check_binding_tested_hash_failure_is_a_note() {
    let scratch = Scratch::new("cb-tested-hash");
    let home = scratch.home("h");
    let lock = format!(
        "# spec.lock: written only by SpecEngine (spec lock, spec approve, spec verify --tests); do not edit\nformat = 3\nnorm_recipe = \"specengine-norm/v1\"\ncode_recipe = \"specengine-hash/v3 tree-sitter-rust/0.24.2 abi/15\"\n\n[[tested]]\nnode = \"KILN-RAMP\"\nrev = 1\nspec_hash = \"b3:{0}\"\nbound_hash = \"b3:{0}\"\ntests = [\n  {{ file = \"crates/kiln/src/ramp.rs\", symbol = \"crates/kiln::ramp::tests::ramp_stops_at_peak\" }},\n]\ncommit = \"{1}\"\nverified_at = \"2026-10-21\"\n",
        "7".repeat(64),
        "7".repeat(40)
    );
    let root = git_copy(
        &scratch,
        "verify-tests",
        "repo",
        false,
        move |root: &Path| fs::write(root.join("spec.lock"), &lock).expect("spec.lock"),
    );
    let real = Sandbox::new(scratch.path()).git_program().to_path_buf();
    let bin = scratch.dir("failing-git");
    let wrapper = bin.join("git");
    fs::write(
        &wrapper,
        format!(
            "#!/bin/sh\nfor arg in \"$@\"; do\n  if [ \"$arg\" = hash-object ]; then\n    echo 'fatal: hash-object refused by the test' >&2\n    exit 128\n  fi\ndone\nexec '{}' \"$@\"\n",
            real.display()
        ),
    )
    .expect("the wrapper");
    fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o755)).expect("executable");

    let mut command = isolated_command(BIN, &[], &root, Some(&home));
    command
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = command.spawn().expect("spawn specengine-mcp");
    {
        use std::io::Write as _;
        let mut stdin = child.stdin.take().expect("stdin");
        for message in [
            json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
                "protocolVersion": LEGACY_VERSION,
                "capabilities": {},
                "clientInfo": {"name": "specengine-mcp-tests", "version": "0.0.0"}}}),
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {
                "name": "check_binding", "arguments": {"node": "KILN-RAMP"}}}),
        ] {
            writeln!(stdin, "{message}").expect("a request");
        }
    }
    let started = std::time::Instant::now();
    while child.try_wait().expect("the server").is_none() {
        if started.elapsed() > std::time::Duration::from_secs(60) {
            let _ = child.kill();
            panic!("the server did not exit within 60 s");
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let output = child.wait_with_output().expect("the server's output");
    let stdout = String::from_utf8(output.stdout).expect("UTF-8");
    let reply: Value = stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .find(|message| message["id"] == json!(2))
        .unwrap_or_else(|| panic!("no reply to the call: {stdout}"));
    let result = &reply["result"];
    assert_eq!(result["isError"], json!(false), "{result}");
    let document = &result["structuredContent"];
    assert_eq!(document["tested"], Value::Null, "{document}");
    let notes: Vec<&str> = document["notes"]
        .as_array()
        .expect("notes")
        .iter()
        .filter_map(Value::as_str)
        .filter(|note| note.starts_with("tested: "))
        .collect();
    assert_eq!(notes.len(), 1, "one note: {document}");
    assert_eq!(document["cannot"], json!([]), "no run cause");
    let text = content_text(result);
    assert_eq!(
        text.lines()
            .filter(|line| *line == format!("note: {}", notes[0]))
            .count(),
        1,
        "{text}"
    );
}

/// TE-2, the walker (docs/features/check-binding.md "Data", **Mirror**):
/// every answer over two corpora of different nature is the CLI
/// library's and conforms to the `outputSchema` (exact key sets, every
/// key required): `fixtures/code-a` with the golden lock and
/// verify_node.rs' drift edits (a library: every node, the nine states)
/// and `fixtures/anchor-states` (a kite game crate: KITE-RENAME renamed,
/// KITE-CLONE's equal twins renamed, KITE-STEADY untouched). Across them
/// each state is met, each nullable row key with a value and with `null`,
/// each anchor state, an anchor's `from` and `candidates`, and each of
/// `delta.bindings`' lists non-empty. M: a mirror key's type narrower than
/// the CLI's (a `null` or a value refused).
#[test]
fn check_binding_documents_meet_every_key() {
    let golden = Project::golden(GOLDEN_LOCK);
    drift_golden(&golden.root);
    let nodes = [
        "PIECE-COUNTER",
        "LAW-BOUND",
        "LAW-STEP",
        "LAW-ODD",
        "GOAL-01",
        "PREM-001",
        "cap-check/PASS-01",
        "PIECE-LIMITS",
    ];
    let golden_calls: Vec<Call> = nodes
        .iter()
        .map(|node| {
            call(
                "golden",
                "check_binding",
                json!({"node": node}),
                Want::Parity,
            )
        })
        .collect();
    let anchors = Project::anchors(&["KITE-STEADY", "KITE-RENAME", "KITE-CLONE"]);
    let anchor_calls: Vec<Call> = ["KITE-STEADY", "KITE-RENAME", "KITE-CLONE"]
        .iter()
        .map(|node| {
            call(
                "anchors",
                "check_binding",
                json!({"node": node}),
                Want::Parity,
            )
        })
        .collect();
    let mut documents: Vec<Value> = Vec::new();
    for results in golden
        .answers(&golden_calls)
        .into_iter()
        .chain(anchors.answers(&anchor_calls))
    {
        for result in results {
            documents.push(result["structuredContent"].clone());
        }
    }
    let rows: Vec<&Value> = documents
        .iter()
        .flat_map(|document| document["rows"].as_array().expect("rows"))
        .collect();
    let met = |what: &str, values: Vec<&Value>| {
        let found: BTreeSet<String> = values
            .into_iter()
            .map(|value| if value.is_null() { "null" } else { "value" }.to_owned())
            .collect();
        assert_eq!(found, set(&["null", "value"]), "{what}: both members met");
    };
    for key in VERIFY_NULLABLE {
        met(key, rows.iter().map(|row| &row[key]).collect());
    }
    let levels: Vec<&Value> = rows.iter().map(|row| &row["levels"]).collect();
    met(
        "levels.changed",
        levels
            .iter()
            .filter(|levels| levels.is_object())
            .map(|levels| &levels["changed"])
            .collect(),
    );
    let anchored: Vec<&Value> = rows
        .iter()
        .map(|row| &row["anchor"])
        .filter(|anchor| anchor.is_object())
        .collect();
    met(
        "anchor.from",
        anchored.iter().map(|anchor| &anchor["from"]).collect(),
    );
    met(
        "anchor.candidates",
        anchored
            .iter()
            .map(|anchor| &anchor["candidates"])
            .collect(),
    );
    met(
        "markers[].rev",
        rows.iter()
            .flat_map(|row| row["markers"].as_array().expect("markers"))
            .map(|marker| &marker["rev"])
            .collect(),
    );
    let states: BTreeSet<String> = rows
        .iter()
        .map(|row| row["state"].as_str().expect("a state").to_owned())
        .collect();
    assert_eq!(states, set(&VERIFY_STATES), "every state met");
    let anchor_states: BTreeSet<String> = anchored
        .iter()
        .map(|anchor| anchor["state"].as_str().expect("a state").to_owned())
        .collect();
    assert_eq!(
        anchor_states,
        set(&["exact", "moved", "ambiguous", "lost"]),
        "every anchor state met"
    );
    for list in ["moved", "ambiguous", "lost", "stale"] {
        assert!(
            documents.iter().any(|document| {
                document["delta"]["bindings"][list]
                    .as_array()
                    .is_some_and(|items| !items.is_empty())
            }),
            "delta.bindings.{list} met non-empty"
        );
    }
    for document in &documents {
        for key in ["base", "task"] {
            assert_eq!(document[key], Value::Null, "{key}: always null");
        }
        for key in ["symbols", "tests_reaching"] {
            assert_eq!(document["delta"][key], Value::Null, "delta.{key}");
        }
    }
}

/// The requests of one check_binding determinism session: `tools/list`,
/// then every exit and REF form over the golden.
fn check_binding_requests() -> Vec<(String, Value)> {
    let mut list = vec![("tools/list".to_owned(), json!({}))];
    for arguments in [
        json!({"node": "PIECE-COUNTER"}),
        json!({"node": "PIECE-COUNTER", "symbol": "crates/tally::counter"}),
        json!({"node": TALLY_COUNTER_MD}),
        json!({"node": "PIECE-LIMITS"}),
        json!({"node": "cap-check/PASS-01"}),
        json!({"node": "LAW-BOUND"}),
        json!({"node": "AIM-01"}),
        json!({"node": "GOAL-01"}),
        json!({"node": "LAW-STEP", "symbol": "data/limits.ron#root.step"}),
        json!({"node": "LAW-NOPE"}),
        json!({"node": GOAL_LOOK_ALIKE}),
        json!({"node": 7}),
    ] {
        list.push((
            "tools/call".to_owned(),
            json!({"name": "check_binding", "arguments": arguments}),
        ));
    }
    list
}

/// TE-2, determinism (docs/features/check-binding.md, its rules): a
/// forward git copy of the golden, a copy created in reverse name order
/// at another, deeper root, and a copy read with `--root` from another
/// working directory, each with its own `HOME`, give byte-identical reply
/// lines (the same request ids) for every exit and REF form in both eras;
/// a repeated session the same bytes again; no line holds a root, a
/// `HOME`, a scratch name or today's date. CRLF copies are not covered
/// (`spec-verify-scope.md` "Known limits"). M: the absolute root in a
/// document; a row order from a hash map.
#[test]
fn check_binding_is_byte_identical_across_roots_order_and_cwd() {
    let one = Scratch::new("cbdet-one");
    let two = Scratch::new("cbdet-two-with-a-longer-name");
    let forward = git_copy(
        &one,
        "code-a",
        "cbdet-first-root",
        false,
        with_lock(GOLDEN_LOCK),
    );
    let reverse = git_copy(
        &two,
        "code-a",
        "nested/cbdet-second-root",
        true,
        with_lock(GOLDEN_LOCK),
    );
    let fourth = git_copy(
        &two,
        "code-a",
        "cbdet-fourth-root",
        true,
        with_lock(GOLDEN_LOCK),
    );
    let elsewhere = two.dir("cbdet-elsewhere");
    let homes = [
        one.home("cbdet-home-one"),
        two.home("cbdet-home-two"),
        two.home("cbdet-home-four"),
    ];
    let fourth_text = fourth.to_str().expect("UTF-8").to_owned();
    let dates = dates_around_now();
    for era in ERAS {
        let lines = |args: &[&str], cwd: &Path, home: &Path| {
            lines_of_requests(era, args, cwd, home, check_binding_requests())
        };
        let first = lines(&[], &forward, &homes[0]);
        let variants = [
            ("reverse", lines(&[], &reverse, &homes[1])),
            (
                "--root from elsewhere",
                lines(&["--root", fourth_text.as_str()], &elsewhere, &homes[2]),
            ),
            ("again", lines(&[], &forward, &homes[0])),
        ];
        for (label, lines) in &variants {
            assert_eq!(first.len(), lines.len(), "{era:?} {label}");
            for (index, (left, right)) in first.iter().zip(lines).enumerate() {
                assert_eq!(left, right, "{era:?} {label}: reply {index} differs");
            }
        }
        let forbidden: Vec<String> = [&forward, &reverse, &fourth, &elsewhere]
            .iter()
            .map(|path| path.to_str().expect("UTF-8").to_owned())
            .chain(
                homes
                    .iter()
                    .map(|home| home.to_str().expect("UTF-8").to_owned()),
            )
            .chain(["cbdet-".to_owned()])
            .chain(dates.iter().cloned())
            .collect();
        for line in &first {
            for text in &forbidden {
                assert!(!line.contains(text.as_str()), "{era:?}: {text:?} in {line}");
            }
        }
        // The replies are the answers asked for: observed, a report, none.
        let replies: Vec<Value> = first.iter().map(|line| parse_message(line)).collect();
        assert_eq!(
            result(&replies[1])["structuredContent"]["verdict"],
            json!("observed"),
            "{era:?}"
        );
        assert_eq!(
            result(&replies[6])["structuredContent"]["verdict"],
            json!("cannot-verify"),
            "{era:?}"
        );
        assert!(
            result(&replies[10]).get("structuredContent").is_none(),
            "{era:?}: LAW-NOPE"
        );
    }
}

/// TE-2, quiet (AC-19's rule for `check_binding`): over broken, empty,
/// BOM-led, look-alike-marked and non-UTF-8 code files in the golden
/// repository every call answers the CLI's answer (its `note:` and
/// `cannot:` lines inside `content`), stdout carries JSON-RPC only (the
/// harness parses every line), stderr stays empty, the process exits 0.
/// M: a `println!` or the default panic hook on the path.
#[test]
fn check_binding_leaves_stdout_json_rpc_only() {
    let project = Project::golden(GOLDEN_LOCK);
    let root = &project.root;
    write(
        root,
        "crates/tally/src/raw.rs",
        b"// @implements LAW-STEP\npub fn raw() -> u32 {\n    1\n}\n// \xff\xfe not text\n"
            .as_slice(),
    );
    write(
        root,
        "crates/tally/src/half.rs",
        "// @implements LAW-STEP\npub fn half( {\n",
    );
    write(root, "crates/tally/src/empty.rs", "");
    write(
        root,
        "crates/tally/src/bom.rs",
        "\u{feff}// @implements PIECE-COUNTER\npub fn bom() -> u32 {\n    2\n}\n",
    );
    write(
        root,
        "crates/tally/src/mixed.rs",
        "// @implements L\u{0410}W-STEP@1\npub fn mixed() -> u32 {\n    3\n}\n",
    );
    write(
        root,
        "data/odd.ron",
        "Odd(\n    width: , // @configures LAW-STEP\n",
    );
    let calls = [
        call(
            "quiet",
            "check_binding",
            json!({"node": "LAW-STEP"}),
            Want::Parity,
        ),
        call(
            "quiet",
            "check_binding",
            json!({"node": "PIECE-COUNTER"}),
            Want::Parity,
        ),
        call(
            "quiet",
            "check_binding",
            json!({"node": "GOAL-01"}),
            Want::Parity,
        ),
        call(
            "quiet",
            "check_binding",
            json!({"node": "LAW-STEP", "symbol": "crates/tally::raw::raw"}),
            Want::Parity,
        ),
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

/// TE-2, genre (docs/features/check-binding.md "Data", the exit table:
/// "2, a report"): over `fixtures/spec-c` (a Python service, no `[code]`)
/// committed in git, `check_binding` is the CLI's report with its
/// document: `cannot-verify`, the one run cause `[code]`'s line (`cannot:`
/// in the text, `cannot` in the document), no row; a REF naming nothing
/// still has none. The tool's texts and sources: the genre test above (it
/// walks every code tool). M: the `[code]` cause answered as `CannotRun`.
#[test]
fn check_binding_on_spec_c_is_the_code_table_report() {
    let scratch = Scratch::new("cb-genre");
    let home = scratch.home("h");
    let root = git_copy(&scratch, "spec-c", "repo", false, |_| {});
    let project = Project {
        _scratch: scratch,
        home,
        root,
    };
    let calls = [
        bound("a node", json!({"node": "AREA-HIRE"}), "cannot-verify"),
        bound(
            "a node, a symbol",
            json!({"node": "FEE-RIDE", "symbol": "src/dock_hire/fees.py"}),
            "cannot-verify",
        ),
        unbound("unknown", json!({"node": "FEE-NOPE"})),
    ];
    for results in project.answers(&calls) {
        for label in ["a node", "a node, a symbol"] {
            let document = document(&calls, &results, label);
            assert_eq!(document["cannot"], json!([NO_CODE_CAUSE]), "{label}");
            assert_eq!(document["rows"], json!([]), "{label}");
            let text = content_text(pick(&calls, &results, label));
            assert!(
                text.contains(&format!("cannot: {NO_CODE_CAUSE}\n")),
                "{label}: {text:?}"
            );
        }
    }
}

/// TE-3, the MCP half (08 AC-3, the single door): a git repository of
/// `fixtures/code-a` with the golden lock and `[verify] mode = "enforce"`
/// committed, a code file's mtime moved after the commit (the git index
/// stat-dirty: a `git status` that refreshes it would write
/// `.git/index`); `check_binding` called valid at every exit (0, 1, 2 with
/// a report and without) and invalid, in both eras, from a
/// subdirectory; then `git status --porcelain=v1 -z
/// --untracked-files=all` is as before (empty), `.git/index`'s mtime and
/// `spec.lock`'s bytes unchanged, the tree (`.git` included)
/// byte-identical, the queue's proposals, tasks and events unchanged,
/// every new path under `HOME`. M: a lock write, a queue row, an index
/// refresh.
#[test]
fn te3_check_binding_writes_nothing_under_the_root() {
    let scratch = Scratch::new("cb-door");
    let home = scratch.home("h");
    let root = git_copy(&scratch, "code-a", "repo", false, |root| {
        with_lock(GOLDEN_LOCK)(root);
        set_verify_mode(root, "enforce");
    });
    let lock = fs::read(root.join("spec.lock")).expect("spec.lock");
    let sandbox = Sandbox::new(scratch.path());
    let env = cli_env(&root, Some(&home));
    let git = GitEnv::new(&root, sandbox.vars());
    index_with_library(&env, &Globals::default());
    let queue = queue_rows(&env, &git);
    // Stat-dirty, content clean: the index records the old mtime.
    let touched = root.join(TALLY_COUNTER);
    fs::File::options()
        .write(true)
        .open(&touched)
        .and_then(|file| file.set_modified(UNIX_EPOCH + std::time::Duration::from_secs(86_400)))
        .expect("an older mtime");
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
        (json!({"node": "PIECE-LIMITS"}), Some("clean")),
        (
            json!({"node": "PIECE-COUNTER", "symbol": "crates/tally::counter"}),
            Some("observed"),
        ),
        (json!({"node": "PIECE-COUNTER"}), Some("blocked")),
        (json!({"node": "LAW-BOUND"}), Some("cannot-verify")),
        (json!({"node": "AIM-01"}), Some("cannot-verify")),
        (
            json!({"node": "LAW-STEP", "symbol": null}),
            Some("cannot-verify"),
        ),
        (json!({"node": "LAW-NOPE"}), None),
        (json!({"node": GOAL_LOOK_ALIKE}), None),
    ];
    let invalid = [
        json!({}),
        json!({"node": 5}),
        json!({"node": "PIECE-COUNTER", "symbol": ["x"]}),
        json!({"node": "PIECE-COUNTER", "task": "T-0001"}),
        json!({"node": "PIECE-COUNTER", "accept": true}),
    ];
    for era in ERAS {
        let mut session = Session::open(era, &[], Some(&cwd), Home::At(&home));
        assert_eq!(tool_names(&session.tools()), TOOLS, "{era:?}");
        for (args, verdict) in &valid {
            let reply = session.call("check_binding", args.clone());
            let result = result(&reply);
            let context = format!("{era:?} {args}");
            match verdict {
                Some(verdict) => Want::Verdict(verdict).check(result, &context),
                None => Want::CannotRun.check(result, &context),
            }
        }
        for args in &invalid {
            let reply = session.call("check_binding", args.clone());
            assert_bad_arguments(&reply, "", &format!("{era:?} {args}"));
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
}
