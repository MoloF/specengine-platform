//! docs/features/impact.md (7b-1) over the real binary over stdio, the
//! default build: the MCP tool `get_impact {id?, symbol?, depth?}`, the
//! twin of `spec impact [REF] [--symbol QPATH] [--depth N]`, in both
//! protocol eras. The parity reference is the CLI library
//! (`common/read.rs` `library`): `content` = `spec impact … 2>&1`,
//! `structuredContent` = its `--json` document, kept at exit 1, absent at
//! exit 2 and on the tool's form error; every document is walked against
//! the `outputSchema` (exact key sets, every key required).
//!
//! - RD-2 (MCP half): `impact_takes_exactly_one_input` (none, both, both
//!   `null`: the form error before discovery, a server without a project);
//!   `get_impact_arguments_are_the_flag_names` (`{id}`, `{symbol}` answer;
//!   `{node_id}`, `{qpath}`, `{since}` refused as bad arguments).
//! - RD-8: `get_impact_is_listed_as_a_read` (annotations, `_meta`, the
//!   flat input with its field docs, the description verbatim, its
//!   figure); `get_impact_equals_the_cli` (per exit, both eras, over a git
//!   repository of `fixtures/impact` with a partial lock, code moved past
//!   it and tasks, over `fixtures/spec-c` and a project past the cut);
//!   `get_impact_mirror_is_impact_json` (the key sets named here, the
//!   `AlwaysNull` keys, the closed enums); `get_impact_documents_meet_every_key`
//!   (each nullable key met with a value and with `null`).
//! - TE-6: `mcp_door.rs` (the door over spec-a), `mcp_size.rs` (the
//!   maximum), `mcp_genre.rs`; INSTRUCTIONS: `mcp_code.rs`.
//!
//! docs/features/impact-since.md (7b-2) "MCP, INSTRUCTIONS, plugin 0.16.0":
//! `get_impact {id?, symbol?, since?, depth?}`, `since` the CLI's `--since
//! REV`. RD-1 (MCP half): `get_impact_refuses_an_option_like_since_before_git`
//! (the guard's line, a `git` shim on the server's `PATH` logging no
//! child); TE-5: the form over three inputs, `{since}` answering, parity
//! per exit for `since` (an answer, a cut one, a REV naming no commit, the
//! guard, no `[code]`, outside git), the mirror typing `input.since`,
//! `revision`, `delta` and `truncated.delta`, the description of 2 043
//! bytes and the four field docs verbatim.
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

#![cfg(all(unix, not(feature = "probes")))]

mod common;

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;

use std::collections::BTreeSet;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::PathBuf;

use common::read::{
    ERAS, Era, GET_IMPACT_ONE_FORM, Session, TOOLS, assert_bad_arguments, checked_call, cli_env,
    conforms, content_text, flat_input_problems, output_schemas,
};
use common::*;
use git::Sandbox;
use serde_json::{Value, json};
use specengine_cli::{
    Env, GitEnv, Globals, LockAcceptRequest, LockPrompt, OUTPUT_CAP_CHARS, TaskDecisionRequest,
    TaskNewRequest, TaskStatus, lock_accept, task_approve, task_cancel, task_new,
};

/// The fixture of the parity runs.
const FIXTURE: &str = "impact";

/// The sentence every description holds.
const DETERMINISM: &str = "Deterministic: one state, one result; no LLM inside.";

/// docs/features/impact-since.md "MCP, INSTRUCTIONS, plugin 0.16.0":
/// 7b-1's description (docs/features/impact.md "MCP `get_impact`") with the
/// seven edits in place, 2 043 bytes.
const IMPACT_DESCRIPTION: &str = "What a change to a node, a code symbol or the code since a revision reaches: the nodes the impact walk reaches from it, each with its distance; the code bound to them and to the sections within them, with each binding's sync; the tests bound to them; and this repository's open tasks naming them. The same as `spec impact REF | --symbol QPATH | --since REV [--depth N]`: content is the command's output, structuredContent its --json document.

id: a REF, any form get_node takes: its nodes start the walk (an ID several documents hold: all, with a warning).
symbol: a code symbol, looked up as find_symbols' query: the walk starts at the nodes whose markers bind an item of its best tier, an item enclosing one, or its file; a mention never starts it. Needs a `[code]` table.
since: a git revision: the walk starts at the nodes bound to the code items changed on disk since it, untracked ones too. Needs `[code]` and git.
depth: links from the start at most; 0 = the start only. Absent: unbounded.
Give exactly one of id, symbol and since: anything else is an error result naming them.

The walk is `spec graph --impact`'s: what depends on, derives from, verifies or uses the terms of a reached node, and what it constrains. Nothing here changes a status, a verdict or a gate. Without a `[code]` table, id answers with no code and no tests; a binding that cannot be judged (no git, an unreadable lock) is listed with an unknown sync and one note. An answer above 40000 characters is cut, the counts first, with a tail counting the rest (it names --depth N: depth). A REF, symbol or since naming nothing: an error result with the reason, and the document. A call that cannot run (a look-alike ID, refused with its Latin fix; symbol or since without `[code]` or with an incomplete walk; since outside git): an error result with its one line. Reads the files as they are now: refreshes SpecEngine's indexes of the spec and the code in its data directory, writes nothing under the project root. Deterministic: one state, one result; no LLM inside.";

/// docs/features/impact.md "MCP `get_impact`": the field docs, verbatim;
/// `since`'s (`IMPACT_SINCE_DOC`)
/// docs/features/impact-since.md "MCP, INSTRUCTIONS, plugin 0.16.0".
const FIELD_DOCS: [(&str, &str); 4] = [
    (
        "id",
        "An ID, an alias, `slug/ID`, `ID#SECTION` or a root-relative `.md` path.",
    ),
    (
        "symbol",
        "A code symbol, looked up as find_symbols' query: a qualified path, a suffix of one or a bare name.",
    ),
    (
        "since",
        "A git revision: the code changed since it, against the files on disk.",
    ),
    (
        "depth",
        "Links from the start at most; 0 = the start only. Absent: unbounded.",
    ),
];

/// docs/features/impact-since.md "Command and guard": the guard's line for
/// a REV printed as `shown`, LF-ended as the tool's content.
fn refused(shown: &str) -> String {
    format!(
        "spec: --since `{shown}` is refused: a revision never starts with `-` or holds a control character\n"
    )
}

/// The tail's narrowing under `since` (docs/features/impact-since.md "Cut").
const SINCE_TAIL: &str = " not shown; lower --depth N or name a later --since REV]";

/// The document's keys (docs/features/impact.md "JSON").
const TOP_KEYS: [&str; 14] = [
    "input",
    "reason",
    "revision",
    "depth",
    "seeds",
    "delta",
    "left_out",
    "nodes",
    "symbols",
    "tests",
    "tasks",
    "notes",
    "truncated",
    "code",
];

/// A binding row's 14 keys (`code-reads.md` "The binding row").
const ROW_KEYS: [&str; 14] = [
    "node",
    "relation",
    "file",
    "kind",
    "symbol",
    "item_kind",
    "line",
    "end_line",
    "signature",
    "layer",
    "sync",
    "category",
    "detail",
    "anchor",
];

/// The row keys a document meets with a value and with `null` across the
/// parity runs.
const ROW_NULLABLE: [&str; 9] = [
    "kind",
    "symbol",
    "item_kind",
    "line",
    "end_line",
    "signature",
    "layer",
    "category",
    "anchor",
];

/// The lock clock and the task clock.
const ACCEPT_CLOCK: &str = "2026-10-08T09:30:00Z";
const TASK_CLOCK: &str = "2026-10-21T09:00:00Z";

// ------------------------------------------------------------- harness

/// What a call must answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Want {
    /// Exit 0: success, its document, `reason` null.
    Answered,
    /// Exit 1: `isError`, the `spec:` line last, the document with its
    /// `reason`.
    NotFound,
    /// Exit 2: `isError`, the `CliError` line(s), no document.
    CannotRun,
    /// The tool's form error, exactly this text, no document.
    Form(&'static str),
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
                let reason = document["reason"]
                    .as_str()
                    .unwrap_or_else(|| panic!("{context}: the document's reason: {document}"));
                assert_eq!(
                    text.lines().last(),
                    Some(format!("spec: {reason}").as_str()),
                    "{context}: the reason line last: {text:?}"
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
        }
    }
}

/// One call of a parity run.
struct Call {
    label: &'static str,
    args: Value,
    want: Want,
}

fn call(label: &'static str, args: Value, want: Want) -> Call {
    Call { label, args, want }
}

/// The result of the call labelled `label`.
fn pick<'a>(calls: &[Call], results: &'a [Value], label: &str) -> &'a Value {
    let at = calls
        .iter()
        .position(|call| call.label == label)
        .unwrap_or_else(|| panic!("no call {label}"));
    &results[at]
}

/// A scratch git repository of a fixture with its own `HOME`.
struct Project {
    scratch: Scratch,
    home: PathBuf,
    root: PathBuf,
    sandbox: Sandbox,
}

impl Project {
    /// `fixtures/<fixture>` committed (`expected.json` left out).
    fn new(label: &str, fixture_name: &str) -> Self {
        let scratch = Scratch::new(label);
        let home = scratch.home("h");
        let root = scratch.copy(fixture_name, "repo");
        let _ = fs::remove_file(root.join("expected.json"));
        let sandbox = Sandbox::new(scratch.path());
        sandbox.init(&root);
        sandbox.add_all(&root);
        sandbox.commit(&root, &format!("{fixture_name} as committed"));
        Self {
            scratch,
            home,
            root,
            sandbox,
        }
    }

    /// `fixtures/<fixture>` in a directory no git repository holds.
    fn outside_git(label: &str, fixture_name: &str) -> Self {
        let scratch = Scratch::new(label);
        let home = scratch.home("h");
        let root = scratch.copy(fixture_name, "plain");
        let _ = fs::remove_file(root.join("expected.json"));
        let sandbox = Sandbox::new(scratch.path());
        Self {
            scratch,
            home,
            root,
            sandbox,
        }
    }

    fn env(&self) -> Env {
        cli_env(&self.root, Some(&self.home))
    }

    fn git_env(&self) -> GitEnv {
        GitEnv::new(&self.root, self.sandbox.vars())
    }

    /// `nodes` accepted through the CLI library (one lock commit each).
    fn accept(&self, nodes: &[&str]) {
        for node in nodes {
            let mut yes = |_: &LockPrompt<'_>| true;
            lock_accept(
                &self.env(),
                &Globals::default(),
                &LockAcceptRequest {
                    reference: (*node).to_owned(),
                    editorial: false,
                    now: ACCEPT_CLOCK.to_owned(),
                    git: self.git_env(),
                    proposal: None,
                    branch: None,
                },
                &mut yes,
                &[],
            )
            .unwrap_or_else(|error| panic!("lock accept {node}: {error}"));
        }
    }

    /// A task on `nodes` (`title` given or none), then the owner's
    /// `action` (`approve`, `cancel`) if any: its ID.
    fn task(&self, nodes: &[&str], title: Option<&str>, action: Option<&str>) -> String {
        let id = task_new(
            &self.env(),
            &Globals::default(),
            &TaskNewRequest {
                nodes: nodes.iter().map(|node| (*node).to_owned()).collect(),
                title: title.map(str::to_owned),
                goal: None,
                author_role: None,
                author_model: None,
                run: None,
                now: TASK_CLOCK.to_owned(),
                git: self.git_env(),
            },
        )
        .expect("task new")
        .id
        .expect("an ID");
        if let Some(action) = action {
            let mut yes = |_: &str| true;
            let request = TaskDecisionRequest {
                id: id.clone(),
                note: None,
                now: TASK_CLOCK.to_owned(),
                git: self.git_env(),
            };
            let outcome = match action {
                "approve" => task_approve(&self.env(), &Globals::default(), &request, &mut yes),
                _ => task_cancel(&self.env(), &Globals::default(), &request, &mut yes),
            };
            outcome.unwrap_or_else(|error| panic!("task {action}: {error}"));
        }
        id
    }

    /// Every call in both eras, each checked against the library (parity,
    /// the output schema) and its [`Want`]; the results by era. Stderr
    /// stays empty.
    fn answers(&self, calls: &[Call]) -> Vec<Vec<Value>> {
        let env = self.env();
        let globals = Globals::default();
        let mut by_era = Vec::new();
        for era in ERAS {
            let mut session = Session::open(era, &[], Some(&self.root), Home::At(&self.home));
            let schemas = output_schemas(&session.tools());
            let mut results = Vec::new();
            for call in calls {
                let result = checked_call(
                    &mut session,
                    &schemas,
                    "get_impact",
                    call.args.clone(),
                    &env,
                    &globals,
                );
                call.want
                    .check(&result, &format!("{era:?} {} {}", call.label, call.args));
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

/// `fixtures/impact` with TIDE-WINDOW and FARE-01 accepted, code moved
/// past the lock, a marker above a `use` (a keyless row of FARE-02), three
/// tasks (a `draft` without a title, a `ready`, a `cancelled`).
fn ferry(label: &str) -> Project {
    let project = Project::new(label, FIXTURE);
    project.accept(&["TIDE-WINDOW", "FARE-01"]);
    replace(
        &project.root,
        "crates/ferry/src/fares.rs",
        "//! Fares per crossing.\n",
        "//! Fares per crossing.\n\n// @implements FARE-02\nuse std::fmt as _;\n",
    );
    replace(
        &project.root,
        "crates/ferry/src/fares.rs",
        "    crossings\n}",
        "    crossings + 0\n}",
    );
    replace(
        &project.root,
        "crates/ferry/src/crossing.rs",
        "        self.open\n",
        "        self.open + 0\n",
    );
    project.task(&["ROUTE-CROSSING"], None, None);
    project.task(&["TIDE-WINDOW"], Some("Widen the window"), Some("approve"));
    project.task(&["ROUTE-CROSSING"], Some("Dropped"), Some("cancel"));
    project
}

/// A project past the cut: `HUB-CORE` with 1 200 spokes depending on it,
/// 300 bound items, 300 bound tests; 1 000 unbound `tick`s.
fn wide(label: &str) -> Project {
    let scratch = Scratch::new(label);
    let home = scratch.home("h");
    let root = scratch.dir("wide");
    write(
        &root,
        "specengine.toml",
        "[project]\nslug = \"wide-radius\"\n\n[ids]\nHUB = { kind = \"hub\", shape = \"name\" }\n\
         SPOKE = { kind = \"spoke\", width = 4 }\n\n[code]\nroots = [\"pkg\"]\n",
    );
    write(
        &root,
        "docs/spec/hub.md",
        "---\nclass: canon\nid: HUB-CORE\nowner: owner\nreviewed: 2026-10-21\n---\n\n# Core\n\nThe hub.\n",
    );
    for n in 0..1_200 {
        write(
            &root,
            &format!("docs/records/SPOKE-{n:04}.md"),
            format!(
                "---\nclass: canon\nid: SPOKE-{n:04}\nowner: owner\nreviewed: 2026-10-21\nlinks:\n  depends_on: [HUB-CORE]\n---\n\n\
                 # Spoke {n:04} of the wide radius, its title written long to fill a line\n\nIt leans on the hub.\n"
            ),
        );
    }
    write(
        &root,
        "pkg/w/Cargo.toml",
        "[package]\nname = \"w\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n",
    );
    write(
        &root,
        "pkg/w/src/lib.rs",
        "pub mod bound;\npub mod ticks;\n",
    );
    let mut bound = String::new();
    for n in 0..300 {
        bound.push_str(&format!(
            "// @implements HUB-CORE\npub fn bound_item_with_a_long_name_{n:04}() -> u32 {{\n    {n}\n}}\n\n\
             // @verifies HUB-CORE\n#[test]\nfn bound_test_with_a_long_name_{n:04}() {{}}\n\n"
        ));
    }
    write(&root, "pkg/w/src/bound.rs", bound);
    let mut ticks = String::new();
    for n in 0..1_000 {
        ticks.push_str(&format!(
            "pub mod m{n:04} {{\n    pub fn tick() -> u32 {{\n        {n}\n    }}\n}}\n"
        ));
    }
    write(&root, "pkg/w/src/ticks.rs", ticks);
    let sandbox = Sandbox::new(scratch.path());
    sandbox.init(&root);
    sandbox.add_all(&root);
    sandbox.commit(&root, "the wide corpus");
    Project {
        scratch,
        home,
        root,
        sandbox,
    }
}

/// The untracked file of [`untracked_many`].
const MANY: &str = "pkg/w/src/many.rs";

/// 900 untracked items in the wide project: a `since` delta past the cut.
fn untracked_many(project: &Project) {
    let mut many = String::new();
    for n in 0..900 {
        many.push_str(&format!(
            "pub fn an_untracked_function_with_a_long_name_{n:04}() -> u32 {{\n    {n}\n}}\n"
        ));
    }
    write(&project.root, MANY, many);
}

/// The `get_impact` tool of a fresh server's `tools/list`.
fn listed_tool(era: Era) -> Value {
    let mut session = Session::open(era, &[], None, Home::Fresh);
    let list = session.tools();
    assert_eq!(tool_names(&list), TOOLS, "{era:?}: twenty-one tools");
    let listed = tool(&list, "get_impact").clone();
    let done = session.finish();
    assert!(done.status.success(), "{era:?}: {}", done.stderr);
    listed
}

/// The subschemas `schema` stands for: itself and, recursively, the
/// members of its `anyOf`/`oneOf`.
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

/// The subschema at `path` (`.`-separated names, `[]` steps into items),
/// through nullable unions.
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

/// The property names an object schema declares (through unions).
fn keys_at(schema: &Value) -> BTreeSet<String> {
    members(schema)
        .into_iter()
        .filter_map(|member| member.get("properties").and_then(Value::as_object))
        .flat_map(|properties| properties.keys().cloned())
        .collect()
}

/// The values a schema's `enum`s and `const`s admit, through unions.
fn enum_values(schema: &Value) -> Option<BTreeSet<String>> {
    let mut found: Option<BTreeSet<String>> = None;
    for member in members(schema) {
        if let Some(values) = member.get("enum").and_then(Value::as_array) {
            found
                .get_or_insert_with(BTreeSet::new)
                .extend(values.iter().filter_map(Value::as_str).map(str::to_owned));
        }
    }
    found
}

/// The JSON types a schema admits, through unions.
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

fn set(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

/// The `get_impact` output schema of a fresh server.
fn impact_schema() -> Value {
    listed_tool(Era::Legacy)["outputSchema"].clone()
}

// ------------------------------------------------------------------ RD-8

/// RD-8 (docs/features/impact.md "MCP `get_impact`"), TE-5
/// (docs/features/impact-since.md "MCP, INSTRUCTIONS, plugin 0.16.0"):
/// `get_impact` is listed in both eras as a read (the reads' annotations,
/// `_meta`, an object `outputSchema` without `$ref`); its input is exactly
/// `{id?, symbol?, since?, depth?}`: `id`, `symbol`, `since` string or
/// null, `depth` integer or null (`format: int64`), nothing required,
/// `additionalProperties: false`, no root `oneOf`, the four field docs
/// verbatim (the reads' flat input rule too); its description is the
/// spec's, verbatim, 2 043 ASCII bytes, holding the determinism sentence
/// and its CLI twin, the cut it states the CLI's. M: a writer's hints;
/// `id` required; a root `oneOf`; the description reworded.
#[test]
fn get_impact_is_listed_as_a_read() {
    assert_eq!(IMPACT_DESCRIPTION.len(), 2043, "the spec's figure");
    for era in ERAS {
        let listed = listed_tool(era);
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
        assert!(
            input
                .get("required")
                .is_none_or(|required| required == &json!([])),
            "{era:?}: nothing required: {input}"
        );
        for key in ["oneOf", "anyOf", "allOf"] {
            assert!(input.get(key).is_none(), "{era:?}: a root {key}: {input}");
        }
        assert_eq!(
            keys_at(input),
            set(&["id", "symbol", "since", "depth"]),
            "{era:?}: {input}"
        );
        assert_eq!(
            types_of(&input["properties"]["since"]),
            set(&["null", "string"])
        );
        assert_eq!(
            types_of(&input["properties"]["id"]),
            set(&["null", "string"])
        );
        assert_eq!(
            types_of(&input["properties"]["symbol"]),
            set(&["null", "string"])
        );
        assert_eq!(
            types_of(&input["properties"]["depth"]),
            set(&["integer", "null"])
        );
        assert_eq!(input["properties"]["depth"]["format"], json!("int64"));
        for (key, doc) in FIELD_DOCS {
            assert_eq!(
                input["properties"][key]["description"],
                json!(doc),
                "{era:?}: {key}'s field doc"
            );
        }
        assert_eq!(
            flat_input_problems("get_impact", input),
            Vec::<String>::new(),
            "{era:?}: the reads' flat input"
        );
        let description = listed["description"].as_str().expect("a description");
        assert_eq!(description, IMPACT_DESCRIPTION, "{era:?}: the spec's text");
        assert!(description.is_ascii());
        assert!(description.contains(DETERMINISM));
        assert!(description.contains("spec impact REF | --symbol QPATH | --since REV [--depth N]"));
        assert!(
            description.contains(&format!("above {OUTPUT_CAP_CHARS} characters")),
            "the cut is the CLI's"
        );
    }
}

// ------------------------------------------------------------------ RD-2

/// RD-2 (the MCP half), TE-5
/// (docs/features/impact-since.md "MCP, INSTRUCTIONS, plugin 0.16.0"): none of
/// `id`, `symbol` and `since`, two or three of them, or all `null` (a `null` is
/// absent) is the tool's form error, exactly, no document, before discovery:
/// the same from a server whose directory holds no project; one non-null among
/// `null`s is that form. M2: two inputs accepted.
#[test]
fn impact_takes_exactly_one_input() {
    let project = ferry("im-mcp-one");
    let form = Want::Form(GET_IMPACT_ONE_FORM);
    project.answers(&[
        call("none", json!({}), form),
        call("both null", json!({"id": null, "symbol": null}), form),
        call("depth only", json!({"depth": 1}), form),
        call(
            "both",
            json!({"id": "ROUTE-CROSSING", "symbol": "opens"}),
            form,
        ),
        call(
            "both, depth",
            json!({"id": "TIDE-WINDOW", "symbol": "opens", "depth": 0}),
            form,
        ),
        call(
            "all null",
            json!({"id": null, "symbol": null, "since": null}),
            form,
        ),
        call(
            "id, since",
            json!({"id": "ROUTE-CROSSING", "since": "HEAD"}),
            form,
        ),
        call(
            "symbol, since",
            json!({"symbol": "opens", "since": "HEAD"}),
            form,
        ),
        call(
            "all three",
            json!({"id": "ROUTE-CROSSING", "symbol": "opens", "since": "HEAD"}),
            form,
        ),
        call(
            "a guarded since beside an id",
            json!({"id": "ROUTE-CROSSING", "since": "-x"}),
            form,
        ),
        call(
            "symbol, id null",
            json!({"id": null, "symbol": "opens"}),
            Want::Answered,
        ),
        call(
            "since, the others null",
            json!({"id": null, "symbol": null, "since": "HEAD"}),
            Want::Answered,
        ),
        call(
            "id, symbol null",
            json!({"id": "ROUTE-CROSSING", "symbol": null}),
            Want::Answered,
        ),
    ]);
    // Before discovery: a server without a project.
    let scratch = Scratch::new("im-mcp-nowhere");
    let home = scratch.home("h");
    let nowhere = scratch.dir("nowhere");
    for era in ERAS {
        let mut session = Session::open(era, &[], Some(&nowhere), Home::At(&home));
        for args in [
            json!({}),
            json!({"id": null, "symbol": null}),
            json!({"id": "ROUTE-CROSSING", "symbol": "opens"}),
            json!({"id": "ROUTE-CROSSING", "since": "HEAD"}),
            json!({"symbol": "opens", "since": "-x"}),
        ] {
            let reply = session.call("get_impact", args.clone());
            Want::Form(GET_IMPACT_ONE_FORM).check(result(&reply), &format!("{era:?} {args}"));
        }
        // One input: discovery's exit 2.
        for args in [json!({"id": "ROUTE-CROSSING"}), json!({"since": "HEAD"})] {
            let reply = session.call("get_impact", args.clone());
            let answered = result(&reply);
            Want::CannotRun.check(answered, &format!("{era:?} discovery {args}"));
            assert!(
                content_text(answered).contains("specengine.toml"),
                "{era:?}: discovery's line: {}",
                content_text(answered)
            );
        }
        // The guard comes before discovery too.
        let reply = session.call("get_impact", json!({"since": "-x"}));
        let answered = result(&reply);
        Want::CannotRun.check(answered, &format!("{era:?} the guard, no project"));
        assert_eq!(content_text(answered), refused("-x"), "{era:?}");
        let done = session.finish();
        assert!(done.status.success(), "{era:?}: {}", done.stderr);
    }
}

/// RD-2: the argument names are the CLI's flags (`id` the REF, as
/// `get_node`): `{id}`, `{symbol}` and (7b-2) `{since}` answer; 07's
/// `node_id` and `qpath`, a wrong type and an unknown key are rmcp's
/// `failed to deserialize parameters` results naming the field. M:
/// `node_id` or `qpath` names.
#[test]
fn get_impact_arguments_are_the_flag_names() {
    let project = ferry("im-mcp-names");
    project.answers(&[
        call("id", json!({"id": "ROUTE-CROSSING"}), Want::Answered),
        call("symbol", json!({"symbol": "opens"}), Want::Answered),
        call("since", json!({"since": "HEAD"}), Want::Answered),
        call(
            "depth",
            json!({"id": "ROUTE-CROSSING", "depth": 1}),
            Want::Answered,
        ),
        call(
            "since, depth",
            json!({"since": "HEAD", "depth": 0}),
            Want::Answered,
        ),
    ]);
    for era in ERAS {
        let mut session = Session::open(era, &[], Some(&project.root), Home::At(&project.home));
        for (args, needle) in [
            (json!({"node_id": "ROUTE-CROSSING"}), "node_id"),
            (json!({"qpath": "opens"}), "qpath"),
            (json!({"revision": "HEAD"}), "revision"),
            (json!({"since": 7}), ""),
            (json!({"since": ["HEAD"]}), ""),
            (json!({"id": "ROUTE-CROSSING", "with": ["links"]}), "with"),
            (json!({"id": 7}), ""),
            (json!({"id": ["ROUTE-CROSSING"]}), ""),
            (json!({"symbol": 3}), ""),
            (json!({"id": "ROUTE-CROSSING", "depth": "1"}), ""),
            (json!({"id": "ROUTE-CROSSING", "depth": 1.5}), ""),
        ] {
            let reply = session.call("get_impact", args.clone());
            assert_bad_arguments(&reply, needle, &format!("{era:?} {args}"));
        }
        let done = session.finish();
        assert!(done.status.success(), "{era:?}: {}", done.stderr);
    }
}

// ------------------------------------------------------------------ RD-8

/// RD-8, both eras, every exit as the CLI library answers it (content
/// `2>&1`, `structuredContent` its `--json` document walked against the
/// `outputSchema`):
///
/// - over `fixtures/impact` (a partial lock, code past it, tasks): exit 0
///   for a document, a section, a feature-scoped criterion, a path, a
///   depth (0, 1), a method, an inline mod's item, two matches, a mention
///   only, an exact test qpath; exit 1 for a REF and a symbol naming
///   nothing (the document kept, the reason line last); exit 2 for a
///   look-alike (its Latin fix), `project:`, a negative depth, a walk gap
///   under `symbol` (the file named), while the REF form answers it;
/// - over `fixtures/spec-c` (no `[code]`): a REF answers (code `null`),
///   `symbol` is the `[code]` line;
/// - over a project past the cut: the tail last, `truncated` counting.
///
/// M: an exit-1 document dropped; a CLI key without its mirror (the
/// walker's exact key sets).
#[test]
fn get_impact_equals_the_cli() {
    let project = ferry("im-mcp-parity");
    let calls = [
        call("document", json!({"id": "ROUTE-CROSSING"}), Want::Answered),
        call("section", json!({"id": "TIDE-WINDOW"}), Want::Answered),
        call(
            "criterion",
            json!({"id": "ferry-check/CHK-01"}),
            Want::Answered,
        ),
        call(
            "path",
            json!({"id": "docs/spec/crossing.md"}),
            Want::Answered,
        ),
        call(
            "depth 0",
            json!({"id": "ROUTE-DOCKS", "depth": 0}),
            Want::Answered,
        ),
        call(
            "depth 1",
            json!({"id": "ROUTE-DOCKS", "depth": 1}),
            Want::Answered,
        ),
        call("method", json!({"symbol": "opens"}), Want::Answered),
        call(
            "inline mod",
            json!({"symbol": "deep::reach"}),
            Want::Answered,
        ),
        call("two matches", json!({"symbol": "reach"}), Want::Answered),
        call(
            "mention only",
            json!({"symbol": "dock_time"}),
            Want::Answered,
        ),
        call(
            "exact test",
            json!({"symbol": "crates/ferry::test:crossing::window_closes"}),
            Want::Answered,
        ),
        call("no node", json!({"id": "ROUTE-NOWHERE"}), Want::NotFound),
        call(
            "no symbol",
            json!({"symbol": "nothing_here"}),
            Want::NotFound,
        ),
        call(
            "look-alike",
            json!({"id": "R\u{41E}UTE-CROSSING"}),
            Want::CannotRun,
        ),
        call(
            "project:",
            json!({"id": "project:ROUTE-CROSSING"}),
            Want::CannotRun,
        ),
        call(
            "negative depth",
            json!({"id": "ROUTE-CROSSING", "depth": -1}),
            Want::CannotRun,
        ),
        call("since HEAD", json!({"since": "HEAD"}), Want::Answered),
        call(
            "since, depth 0",
            json!({"since": "HEAD", "depth": 0}),
            Want::Answered,
        ),
        call("since a branch", json!({"since": "main"}), Want::Answered),
        call("since no commit", json!({"since": "nope"}), Want::NotFound),
        call(
            "since a tree",
            json!({"since": "HEAD^{tree}"}),
            Want::NotFound,
        ),
        call("since -x", json!({"since": "-x"}), Want::CannotRun),
        call(
            "since a control",
            json!({"since": "a\u{1}b"}),
            Want::CannotRun,
        ),
    ];
    let head = project
        .sandbox
        .git_text(&project.root, &["rev-parse", "HEAD"]);
    for results in project.answers(&calls) {
        let doc = |label: &str| &pick(&calls, &results, label)["structuredContent"];
        let text = |label: &str| content_text(pick(&calls, &results, label));
        assert!(
            doc("document")["tasks"]
                .as_array()
                .is_some_and(|tasks| tasks.len() == 2),
            "the draft and the ready task, not the cancelled: {}",
            doc("document")["tasks"]
        );
        assert!(
            doc("method")["seeds"]["nodes"] == json!(["TIDE-WINDOW"]),
            "the impl's marker seeds: {}",
            doc("method")["seeds"]
        );
        assert_eq!(
            doc("mention only")["nodes"],
            json!([]),
            "a mention seeds nothing"
        );
        assert_eq!(doc("no symbol")["seeds"]["symbols"], json!([]));
        assert!(
            text("look-alike").contains("ROUTE-CROSSING"),
            "the Latin fix: {:?}",
            text("look-alike")
        );
        assert!(
            text("document").starts_with("impact: nodes 6, "),
            "the summary first: {:?}",
            text("document")
        );
        // docs/features/impact-since.md "Output": `since`'s typed keys.
        let since = doc("since HEAD");
        assert_eq!(since["revision"], json!(head), "the full OID");
        assert_eq!(since["input"]["since"], json!("HEAD"));
        assert_eq!(since["seeds"]["symbols"], Value::Null);
        assert!(
            since["delta"]["changed"]
                .as_array()
                .is_some_and(|items| !items.is_empty()),
            "code moved past HEAD: {}",
            since["delta"]
        );
        assert_eq!(since["delta"]["renamed"], Value::Null);
        assert!(
            text("since HEAD").starts_with(&format!("since {}: ", &head[..7])),
            "the header first: {:?}",
            text("since HEAD")
        );
        assert_eq!(doc("since a branch")["delta"], since["delta"]);
        let missing = doc("since no commit");
        assert_eq!(missing["reason"], json!("--since `nope` is not a commit"));
        for key in ["revision", "delta"] {
            assert_eq!(missing[key], Value::Null, "exit 1: {key} null");
        }
        assert_eq!(text("since -x"), refused("-x"), "the guard's line");
        assert_eq!(text("since a control"), refused("a\\u{1}b"));
    }

    // A walk gap: `symbol` exit 2 naming the file; the REF form answers.
    let spare = project.root.join("crates/ferry/src/spare.rs");
    write(
        &project.root,
        "crates/ferry/src/spare.rs",
        "pub fn spare() {}\n",
    );
    fs::set_permissions(&spare, fs::Permissions::from_mode(0o000)).expect("chmod");
    let gap = [
        call("gap, REF", json!({"id": "ROUTE-CROSSING"}), Want::Answered),
        call("gap, symbol", json!({"symbol": "opens"}), Want::CannotRun),
    ];
    for results in project.answers(&gap) {
        let answered = &pick(&gap, &results, "gap, REF")["structuredContent"];
        assert_eq!(answered["symbols"], Value::Null, "no rows under a walk gap");
        assert!(
            answered["notes"]
                .as_array()
                .expect("notes")
                .iter()
                .any(|note| note
                    .as_str()
                    .is_some_and(|note| note.starts_with("bindings: "))),
            "one bindings note: {answered}"
        );
        assert!(
            content_text(pick(&gap, &results, "gap, symbol")).contains("crates/ferry/src/spare.rs"),
            "the gap named"
        );
    }
    fs::set_permissions(&spare, fs::Permissions::from_mode(0o644)).expect("chmod");

    // Another genre, no `[code]`.
    let genre = Project::new("im-mcp-genre", "spec-c");
    let calls = [
        call("REF", json!({"id": "docs/spec/fees.md"}), Want::Answered),
        call("symbol", json!({"symbol": "fee"}), Want::CannotRun),
        call("since", json!({"since": "HEAD"}), Want::CannotRun),
    ];
    for results in genre.answers(&calls) {
        let answered = &pick(&calls, &results, "REF")["structuredContent"];
        for key in ["symbols", "tests", "code"] {
            assert_eq!(answered[key], Value::Null, "spec-c: {key} null");
        }
        for label in ["symbol", "since"] {
            assert!(
                content_text(pick(&calls, &results, label)).contains("[code]"),
                "{label}: the [code] line"
            );
        }
    }

    // `since` outside a git repository: exit 2, one line.
    let outside = Project::outside_git("im-mcp-outside", FIXTURE);
    let calls = [call("since", json!({"since": "HEAD"}), Want::CannotRun)];
    for results in outside.answers(&calls) {
        let text = content_text(pick(&calls, &results, "since"));
        assert_eq!(text.lines().count(), 1, "one line: {text:?}");
        assert!(text.starts_with("spec: "), "{text:?}");
    }

    // Past the cut: a REF (its `truncated.seeds` null) and a `--symbol`
    // (its matches cut); then without `[code]` (`truncated.symbols`,
    // `.tests` null).
    let wide = wide("im-mcp-wide");
    let calls = [
        call("hub", json!({"id": "HUB-CORE"}), Want::Answered),
        call("ticks", json!({"symbol": "tick"}), Want::Answered),
    ];
    for results in wide.answers(&calls) {
        for label in ["hub", "ticks"] {
            let result = pick(&calls, &results, label);
            let text = content_text(result);
            assert!(
                text.lines()
                    .last()
                    .is_some_and(|line| line.starts_with("[truncated: ")),
                "{label}: the tail last"
            );
            assert!(
                result["structuredContent"]["truncated"].is_object(),
                "{label}: truncated counts"
            );
        }
        let hub = &pick(&calls, &results, "hub")["structuredContent"]["truncated"];
        assert_eq!(
            hub["seeds"],
            Value::Null,
            "a REF: truncated.seeds null: {hub}"
        );
        assert!(
            hub["nodes"].as_u64() > Some(0),
            "a REF cut in its nodes: {hub}"
        );
        let document = &pick(&calls, &results, "ticks")["structuredContent"];
        let ticks = &document["truncated"];
        assert!(
            ticks["seeds"].as_u64() > Some(0),
            "the matches cut: {ticks}"
        );
        let uncut = if document["symbols"].is_array() {
            json!(0)
        } else {
            Value::Null
        };
        assert_eq!(
            ticks["symbols"], uncut,
            "a list not cut 0, a null list null: {ticks}"
        );
    }
    // `since` past the cut: 900 untracked items, the delta cut last.
    untracked_many(&wide);
    let calls = [call("since, cut", json!({"since": "HEAD"}), Want::Answered)];
    for results in wide.answers(&calls) {
        let result = pick(&calls, &results, "since, cut");
        let tail = content_text(result)
            .lines()
            .last()
            .unwrap_or_default()
            .to_owned();
        assert!(
            tail.starts_with("[truncated: ") && tail.ends_with(SINCE_TAIL),
            "the tail names a later --since REV: {tail:?}"
        );
        let truncated = &result["structuredContent"]["truncated"];
        assert!(
            truncated["delta"].as_u64() > Some(0),
            "the delta cut: {truncated}"
        );
        assert_eq!(
            truncated["seeds"],
            Value::Null,
            "since: seeds null: {truncated}"
        );
    }
    fs::remove_file(wide.root.join(MANY)).expect("many.rs removed");
    let config = read_text(&wide.root, "specengine.toml");
    write(
        &wide.root,
        "specengine.toml",
        config.replace("\n[code]\nroots = [\"pkg\"]\n", "\n"),
    );
    let calls = [call(
        "hub, no [code]",
        json!({"id": "HUB-CORE"}),
        Want::Answered,
    )];
    for results in wide.answers(&calls) {
        let truncated = &pick(&calls, &results, "hub, no [code]")["structuredContent"]["truncated"];
        for key in ["symbols", "tests", "seeds"] {
            assert_eq!(
                truncated[key],
                Value::Null,
                "no [code]: truncated.{key} null"
            );
        }
        assert!(
            truncated["nodes"].as_u64() > Some(0),
            "cut in its nodes: {truncated}"
        );
    }
    write(&wide.root, "specengine.toml", &config);
}

/// TE-6 (docs/features/impact.md "MCP `get_impact`", **Mirror**), each
/// shape walked against the output schema once more by name: an uncut REF
/// answer (`truncated` null), a no-`[code]` answer (`symbols`, `tests`,
/// `code` null), a cut `--symbol` answer (`truncated` counting its
/// matches) and a cut REF answer (`truncated.seeds` null); TE-5
/// (docs/features/impact-since.md "Output"): an uncut `since` answer, an
/// exit-1 one (`delta` null) and a cut one (`truncated.delta` counting),
/// each as the server answers it. M: `truncated` or one of its counts not
/// nullable; `delta` not nullable.
#[test]
fn get_impact_schema_meets_every_answer_shape() {
    let schema = impact_schema();
    let ferry = ferry("im-mcp-shapes");
    let genre = Project::new("im-mcp-shapes-genre", "spec-c");
    let wide = wide("im-mcp-shapes-wide");
    let answer = |project: &Project, args: Value| -> Value {
        let mut session = Session::open(
            Era::Legacy,
            &[],
            Some(&project.root),
            Home::At(&project.home),
        );
        let reply = session.call("get_impact", args.clone());
        let document = result(&reply)["structuredContent"].clone();
        let done = session.finish();
        assert!(done.status.success(), "{args}: {}", done.stderr);
        document
    };
    let shapes = [
        (
            "an uncut REF",
            answer(&ferry, json!({"id": "ROUTE-CROSSING"})),
        ),
        (
            "a no-[code] REF",
            answer(&genre, json!({"id": "docs/spec/fees.md"})),
        ),
        ("a cut --symbol", answer(&wide, json!({"symbol": "tick"}))),
        ("a cut REF", answer(&wide, json!({"id": "HUB-CORE"}))),
        ("an uncut --since", answer(&ferry, json!({"since": "HEAD"}))),
        (
            "an exit-1 --since",
            answer(&ferry, json!({"since": "nope"})),
        ),
        ("a cut --since", {
            untracked_many(&wide);
            answer(&wide, json!({"since": "HEAD"}))
        }),
    ];
    for (shape, document) in &shapes {
        if let Err(problem) = conforms(&schema, document, "$") {
            panic!("{shape}: the document breaks the outputSchema: {problem}");
        }
    }
    assert_eq!(
        shapes[0].1["truncated"],
        Value::Null,
        "uncut: truncated null"
    );
    assert_eq!(
        shapes[1].1["symbols"],
        Value::Null,
        "no [code]: symbols null"
    );
    assert!(
        shapes[2].1["truncated"]["seeds"].as_u64() > Some(0),
        "the matches cut"
    );
    assert_eq!(
        shapes[3].1["truncated"]["seeds"],
        Value::Null,
        "a REF: seeds null"
    );
    assert!(shapes[4].1["delta"].is_object(), "since: a delta");
    assert_eq!(shapes[5].1["delta"], Value::Null, "exit 1: delta null");
    assert!(
        shapes[6].1["truncated"]["delta"].as_u64() > Some(0),
        "the delta cut"
    );
}

/// RD-8 (docs/features/impact.md "MCP `get_impact`", **Mirror**), TE-5
/// (docs/features/impact-since.md "MCP, INSTRUCTIONS, plugin 0.16.0"):
/// the output schema's key sets are `spec impact --json`'s, named here (a
/// key dropped from both the mirror and the CLI still fails): the 14 top
/// keys, `input`, `seeds`, a match, `left_out`, a node (graph's), a row
/// (`BindingRowShape`), a task, `truncated`, `code`, `delta` and a delta
/// item; `input.since` and `revision` a string or null, `delta` an object
/// or null whose three lists hold `{file, symbol, item_kind, line}` (three
/// strings, an integer, none null) and whose `renamed` is exactly `{"type":
/// "null"}` (`AlwaysNull`), `truncated.delta` an integer or null; a
/// match's `tier` the lookup's three tiers, a task's `status` every
/// `TaskStatus`; `seeds.symbols`, `symbols`, `tests`, `truncated`, `code`
/// nullable. M: a key's mirror missing; `since` left `AlwaysNull`.
#[test]
fn get_impact_mirror_is_impact_json() {
    let schema = impact_schema();
    assert_eq!(keys_at(&schema), set(&TOP_KEYS));
    assert_eq!(
        keys_at(schema_at(&schema, "input")),
        set(&["ref", "symbol", "since"])
    );
    assert_eq!(
        keys_at(schema_at(&schema, "seeds")),
        set(&["nodes", "symbols"])
    );
    assert_eq!(
        keys_at(schema_at(&schema, "seeds.symbols[]")),
        set(&[
            "qpath",
            "item_kind",
            "path",
            "start_line",
            "end_line",
            "tier"
        ])
    );
    assert_eq!(
        keys_at(schema_at(&schema, "left_out")),
        set(&["generated", "tier3"])
    );
    assert_eq!(
        keys_at(schema_at(&schema, "nodes[]")),
        set(&[
            "id", "kind", "title", "path", "line", "distance", "archived"
        ])
    );
    for list in ["symbols[]", "tests[]"] {
        assert_eq!(keys_at(schema_at(&schema, list)), set(&ROW_KEYS), "{list}");
    }
    assert_eq!(
        keys_at(schema_at(&schema, "tasks[]")),
        set(&["id", "status", "title", "nodes"])
    );
    assert_eq!(
        keys_at(schema_at(&schema, "truncated")),
        set(&["nodes", "symbols", "tests", "tasks", "seeds", "delta"])
    );
    assert_eq!(
        keys_at(schema_at(&schema, "code")),
        set(&["files", "rs", "ron", "skipped"])
    );
    for path in ["input.since", "revision"] {
        assert_eq!(
            types_of(schema_at(&schema, path)),
            set(&["null", "string"]),
            "{path}: a string or null"
        );
    }
    assert_eq!(
        types_of(schema_at(&schema, "delta")),
        set(&["null", "object"]),
        "delta: an object or null"
    );
    assert_eq!(
        keys_at(schema_at(&schema, "delta")),
        set(&["added", "removed", "changed", "renamed"])
    );
    for list in ["added", "removed", "changed"] {
        assert_eq!(
            types_of(schema_at(&schema, &format!("delta.{list}"))),
            set(&["array"]),
            "delta.{list}: a list"
        );
        let item = format!("delta.{list}[]");
        assert_eq!(
            keys_at(schema_at(&schema, &item)),
            set(&["file", "symbol", "item_kind", "line"]),
            "{item}"
        );
        for (key, kind) in [
            ("file", "string"),
            ("symbol", "string"),
            ("item_kind", "string"),
            ("line", "integer"),
        ] {
            assert_eq!(
                types_of(schema_at(&schema, &format!("{item}.{key}"))),
                set(&[kind]),
                "{item}.{key}"
            );
        }
    }
    let renamed = schema_at(&schema, "delta.renamed");
    let mut bare = renamed.clone();
    if let Some(object) = bare.as_object_mut() {
        object.remove("description");
    }
    assert_eq!(
        bare,
        json!({"type": "null"}),
        "delta.renamed: AlwaysNull: {renamed}"
    );
    assert_eq!(
        types_of(schema_at(&schema, "truncated.delta")),
        set(&["integer", "null"]),
        "truncated.delta: a count or null"
    );
    assert_eq!(
        enum_values(schema_at(&schema, "seeds.symbols[].tier")),
        Some(set(&["exact", "suffix", "name"])),
        "the lookup's tiers"
    );
    let statuses: BTreeSet<String> = TaskStatus::ALL
        .iter()
        .map(|status| status.as_str().to_owned())
        .collect();
    assert_eq!(
        enum_values(schema_at(&schema, "tasks[].status")),
        Some(statuses),
        "every TaskStatus"
    );
    for path in [
        "seeds.symbols",
        "symbols",
        "tests",
        "truncated",
        "code",
        "reason",
        "depth",
    ] {
        assert!(
            types_of(schema_at(&schema, path)).contains("null"),
            "{path} nullable"
        );
    }
    // docs/features/impact.md "JSON"
    // `truncated` is null uncut and each of its counts null for a null
    // list (`symbols`, `tests` without code, `seeds` for a REF).
    for path in ["truncated.symbols", "truncated.tests", "truncated.seeds"] {
        assert!(
            types_of(schema_at(&schema, path)).contains("null"),
            "{path} nullable"
        );
    }
    for path in ["seeds.nodes", "nodes", "tasks", "notes"] {
        assert_eq!(
            types_of(schema_at(&schema, path)),
            set(&["array"]),
            "{path}: a list, never null"
        );
    }
    for path in [
        "nodes[].id",
        "nodes[].kind",
        "nodes[].title",
        "tasks[].title",
    ] {
        assert!(
            types_of(schema_at(&schema, path)).contains("null"),
            "{path} nullable"
        );
    }
}

/// RD-8, the walker over documents (`checked_call` walks each against the
/// output schema, every key required): across the parity runs each
/// nullable key is met with a value and with `null`: `reason`, `depth`,
/// `seeds.symbols`, `symbols`, `tests`, `truncated`, `code`, a node's
/// `id` and `kind` (a feature document holds none), a task's `title`, the
/// row keys of [`ROW_NULLABLE`], and (TE-5, docs/features/impact-since.md
/// "Output") `revision`, `delta`, `input.since`, a cut answer's
/// `truncated.delta`. M: a mirror key's type narrower than the CLI's.
#[test]
fn get_impact_documents_meet_every_key() {
    let project = ferry("im-mcp-keys");
    let ferry_calls = [
        call("document", json!({"id": "ROUTE-CROSSING"}), Want::Answered),
        call(
            "depth",
            json!({"id": "ROUTE-DOCKS", "depth": 2}),
            Want::Answered,
        ),
        call("symbol", json!({"symbol": "reach"}), Want::Answered),
        call("no node", json!({"id": "ROUTE-NOWHERE"}), Want::NotFound),
        call("since", json!({"since": "HEAD"}), Want::Answered),
        call("since no commit", json!({"since": "nope"}), Want::NotFound),
    ];
    let wide = wide("im-mcp-keys-wide");
    untracked_many(&wide);
    let wide_calls = [
        call("hub", json!({"id": "HUB-CORE", "depth": 0}), Want::Answered),
        call("ticks", json!({"symbol": "tick"}), Want::Answered),
        call("since, cut", json!({"since": "HEAD"}), Want::Answered),
    ];
    let genre = Project::new("im-mcp-keys-genre", "spec-c");
    let genre_calls = [call(
        "REF",
        json!({"id": "docs/spec/fees.md"}),
        Want::Answered,
    )];
    let mut documents: Vec<Value> = Vec::new();
    for results in project
        .answers(&ferry_calls)
        .into_iter()
        .chain(wide.answers(&wide_calls))
        .chain(genre.answers(&genre_calls))
    {
        for result in results {
            documents.push(result["structuredContent"].clone());
        }
    }
    let met = |what: &str, values: Vec<&Value>| {
        let found: BTreeSet<String> = values
            .into_iter()
            .map(|value| if value.is_null() { "null" } else { "value" }.to_owned())
            .collect();
        assert_eq!(found, set(&["null", "value"]), "{what}: both members met");
    };
    for key in ["reason", "depth", "symbols", "tests", "truncated", "code"] {
        met(
            key,
            documents.iter().map(|document| &document[key]).collect(),
        );
    }
    met(
        "seeds.symbols",
        documents
            .iter()
            .map(|document| &document["seeds"]["symbols"])
            .collect(),
    );
    let nodes: Vec<&Value> = documents
        .iter()
        .flat_map(|document| document["nodes"].as_array().expect("nodes"))
        .collect();
    for key in ["id", "kind"] {
        met(
            &format!("nodes[].{key}"),
            nodes.iter().map(|node| &node[key]).collect(),
        );
    }
    met(
        "tasks[].title",
        documents
            .iter()
            .flat_map(|document| document["tasks"].as_array().expect("tasks"))
            .map(|task| &task["title"])
            .collect(),
    );
    let rows: Vec<&Value> = documents
        .iter()
        .flat_map(|document| {
            ["symbols", "tests"]
                .into_iter()
                .flat_map(move |key| document[key].as_array().into_iter().flatten())
        })
        .collect();
    for key in ROW_NULLABLE {
        met(
            &format!("row.{key}"),
            rows.iter().map(|row| &row[key]).collect(),
        );
    }
    // docs/features/impact-since.md "Output": `since`'s keys typed.
    for key in ["revision", "delta"] {
        met(
            key,
            documents.iter().map(|document| &document[key]).collect(),
        );
    }
    met(
        "input.since",
        documents
            .iter()
            .map(|document| &document["input"]["since"])
            .collect(),
    );
    met(
        "truncated.delta",
        documents
            .iter()
            .filter(|document| document["truncated"].is_object())
            .map(|document| &document["truncated"]["delta"])
            .collect(),
    );
    assert!(
        rows.iter()
            .any(|row| row["kind"].is_null() && row["node"] == "FARE-02"),
        "premise: the keyless row"
    );
    for project in [&project, &wide, &genre] {
        assert!(
            project.scratch.path().is_dir(),
            "the scratch kept to the end"
        );
    }
}

// ------------------------------------------------------------------ RD-1

/// A `git` at `program` appending its argv (one line per process) to
/// `log`, then running `real`.
fn logging_git(program: &std::path::Path, log: &std::path::Path, real: &std::path::Path) {
    let log = log.to_str().expect("a UTF-8 scratch path");
    let real = real.to_str().expect("a UTF-8 git path");
    assert!(
        !log.contains('\'') && !real.contains('\''),
        "a quote in a path"
    );
    fs::write(
        program,
        format!("#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{log}'\nexec '{real}' \"$@\"\n"),
    )
    .expect("the shim");
    fs::set_permissions(program, fs::Permissions::from_mode(0o755)).expect("chmod the shim");
}

/// One server in `root` (`HOME` `home`, `PATH` `path`, nothing else),
/// initialized in the legacy era, then each `get_impact` arguments in
/// turn: their results.
fn calls_with_path(
    root: &std::path::Path,
    home: &std::path::Path,
    path: &str,
    calls: &[Value],
) -> Vec<Value> {
    use std::io::{BufRead as _, BufReader, Write as _};
    use std::process::{Command, Stdio};
    use std::sync::mpsc;
    use std::time::Duration;
    let mut child = Command::new(BIN)
        .env_clear()
        .env("HOME", home)
        .env("PATH", path)
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn specengine-mcp");
    let mut stdin = child.stdin.take().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");
    let (sender, lines) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if sender.send(line).is_err() {
                break;
            }
        }
    });
    let send = |stdin: &mut std::process::ChildStdin, message: Value| {
        let mut line = serde_json::to_string(&message).expect("a message");
        line.push('\n');
        stdin.write_all(line.as_bytes()).expect("write");
        stdin.flush().expect("flush");
    };
    let reply = |id: u64| -> Value {
        loop {
            let line = lines
                .recv_timeout(Duration::from_secs(60))
                .unwrap_or_else(|_| panic!("no reply to request {id} within a minute"));
            let message = parse_message(&line);
            if message.get("id") == Some(&json!(id)) {
                return message;
            }
        }
    };
    send(
        &mut stdin,
        json!({"jsonrpc": "2.0", "id": 0, "method": "initialize", "params": {
            "protocolVersion": LEGACY_VERSION,
            "capabilities": {},
            "clientInfo": {"name": "specengine-mcp-tests", "version": "0.0.0"}
        }}),
    );
    result(&reply(0));
    send(
        &mut stdin,
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
    );
    let mut results = Vec::new();
    for (at, arguments) in calls.iter().enumerate() {
        let id = at as u64 + 1;
        send(
            &mut stdin,
            json!({"jsonrpc": "2.0", "id": id, "method": "tools/call", "params": {
                "name": "get_impact", "arguments": arguments
            }}),
        );
        results.push(result(&reply(id)).clone());
    }
    drop(stdin);
    for _ in 0..1_000 {
        if child.try_wait().expect("wait").is_some() {
            return results;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let _ = child.kill();
    let _ = child.wait();
    panic!("the server outlived its stdin by ten seconds");
}

/// RD-1, TE-6 (the MCP half;
/// docs/features/impact-since.md "Command and guard"): a server whose `PATH`
/// starts with a `git` shim logging argv answers `{since: <REV>}` with a
/// leading `-` or a control character (LF and DEL among them) with an error
/// result whose content is the guard's line (every control `\u{XX}`), no
/// `structuredContent`, and the shim logs no call; `{since: "HEAD"}` reaches
/// the shim (the premise). M1: the guard removed.
#[test]
fn get_impact_refuses_an_option_like_since_before_git() {
    let project = ferry("im-mcp-guard");
    let shim = project.scratch.dir("shim");
    let log = shim.join("argv.log");
    logging_git(&shim.join("git"), &log, project.sandbox.git_program());
    let path = format!("{}:/usr/bin:/bin", shim.display());
    for (since, shown) in [
        ("-x", "-x"),
        ("--all", "--all"),
        ("a\u{1}b", "a\\u{1}b"),
        ("HEAD\n", "HEAD\\u{a}"),
        ("\u{7f}HEAD", "\\u{7f}HEAD"),
    ] {
        let _ = fs::remove_file(&log);
        let results = calls_with_path(
            &project.root,
            &project.home,
            &path,
            &[json!({"since": since})],
        );
        let answered = &results[0];
        assert_eq!(answered["isError"], json!(true), "{since:?}: {answered}");
        assert_eq!(content_text(answered), refused(shown), "{since:?}");
        assert!(
            answered.get("structuredContent").is_none(),
            "{since:?}: no document"
        );
        let logged = fs::read_to_string(&log).unwrap_or_default();
        assert_eq!(logged, "", "{since:?}: no git child");
    }
    let _ = fs::remove_file(&log);
    let results = calls_with_path(
        &project.root,
        &project.home,
        &path,
        &[json!({"since": "HEAD"})],
    );
    assert_eq!(results[0]["isError"], json!(false), "{}", results[0]);
    assert!(
        fs::read_to_string(&log).is_ok_and(|logged| logged.contains("rev-parse")),
        "premise: the shim logs the server's git"
    );
}
