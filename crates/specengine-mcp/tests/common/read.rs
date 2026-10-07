//! Helpers of the read-tool tests (task spec `mcp-read`): a session in
//! either protocol era, the same request through the CLI library (the
//! parity reference: text = `stderr_lines` + `render_text`, exit 2 = the
//! `CliError` line(s); `structuredContent` = the parsed `render_json`), a
//! test-local JSON Schema walker for the output schemas (AC-14), and the
//! flat-input rule of "Data" (AC-13).

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{Value, json};
use specengine_cli::{
    BundleRequest, CliError, DiscrepancyInput, DiscrepancyRequest, Env, Exit, GitEnv, Globals,
    IntakeSeverity, Outcome, ProposeRequest, ProposedText, QuestionRequest, ReviewRequest,
    SearchRequest, ShowRequest, TaskClaimRequest, TaskCompleteRequest, TaskPlanRequest,
    TaskReportRequest, TaskShowRequest, TreeRequest, render_json, render_text,
};

use super::{Finished, Home, Server, result, stateless_meta, with_meta};

/// The four read tools, in `tools/list` order.
pub const READ_TOOLS: [&str; 4] = ["get_context_bundle", "get_node", "get_tree", "search"];

/// The queue tools of task spec `agent-intake`, in `tools/list` order.
pub const QUEUE_TOOLS: [&str; 4] = [
    "ask_question",
    "get_proposal",
    "propose_change",
    "report_discrepancy",
];

/// The three queue tools that write the queue.
pub const WRITE_TOOLS: [&str; 3] = ["ask_question", "propose_change", "report_discrepancy"];

/// The task tools of docs/features/task-package.md ("Description and
/// interactions"), in `tools/list` order: one reads, four move a task as
/// an agent may (the owner's three have none).
pub const TASK_TOOLS: [&str; 5] = [
    "claim_task",
    "complete_task",
    "get_task",
    "report_run",
    "submit_plan",
];

/// Every tool of the default build, in `tools/list` order (by name):
/// thirteen since docs/features/task-package.md.
pub const TOOLS: [&str; 13] = [
    "ask_question",
    "claim_task",
    "complete_task",
    "get_context_bundle",
    "get_node",
    "get_proposal",
    "get_task",
    "get_tree",
    "propose_change",
    "report_discrepancy",
    "report_run",
    "search",
    "submit_plan",
];

/// The words of P2-3 (07 §1.2): stack words and this repository's role
/// names.
pub const STACK_WORDS: [&str; 15] = [
    "cargo",
    "nextest",
    "clippy",
    "bevy",
    "pnpm",
    "npm",
    "nest",
    "react",
    "jira",
    "requirement-analyst",
    "spec-writer",
    "rust-developer",
    "ui-developer",
    "test-engineer",
    "code-reviewer",
];

/// The protocol era of a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Era {
    /// `initialize`, then plain requests.
    Legacy,
    /// 2026-07-28: every request carries `_meta`.
    Stateless,
}

pub const ERAS: [Era; 2] = [Era::Legacy, Era::Stateless];

/// One client session over a spawned server.
pub struct Session {
    pub server: Server,
    pub era: Era,
    next: u64,
}

impl Session {
    /// Spawns the server (cleared env; `home`; `cwd`, else an empty scratch
    /// directory) and opens the era: legacy sends `initialize`.
    pub fn open(era: Era, args: &[&str], cwd: Option<&Path>, home: Home) -> Self {
        let mut server = Server::spawn_with(args, cwd, home);
        if era == Era::Legacy {
            server.initialize(json!({}));
        }
        Self {
            server,
            era,
            next: 1,
        }
    }

    /// The next request id.
    pub fn next_id(&mut self) -> u64 {
        let id = self.next;
        self.next += 1;
        id
    }

    /// `params` as the era sends them (stateless: with `_meta`).
    pub fn params(&self, params: Value) -> Value {
        match self.era {
            Era::Legacy => params,
            Era::Stateless => with_meta(params, &stateless_meta(json!({}))),
        }
    }

    /// One request, its reply.
    pub fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id();
        let params = self.params(params);
        self.server.request(id, method, Some(params))
    }

    /// One request, its reply line as written.
    pub fn request_line(&mut self, method: &str, params: Value) -> String {
        let id = self.next_id();
        let params = self.params(params);
        self.server.request_line(id, method, Some(params))
    }

    /// `tools/call`, the whole reply.
    pub fn call(&mut self, name: &str, arguments: Value) -> Value {
        self.request("tools/call", json!({"name": name, "arguments": arguments}))
    }

    /// The `tools/list` result.
    pub fn tools(&mut self) -> Value {
        result(&self.request("tools/list", json!({}))).clone()
    }

    /// `resources/list` (after `cursor`), the whole reply.
    pub fn resources(&mut self, cursor: Option<&str>) -> Value {
        let params = match cursor {
            Some(cursor) => json!({"cursor": cursor}),
            None => json!({}),
        };
        self.request("resources/list", params)
    }

    /// Every `resources/list` page, following `nextCursor`.
    pub fn all_resources(&mut self) -> Vec<Value> {
        let mut pages = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let reply = self.resources(cursor.as_deref());
            let page = result(&reply).clone();
            cursor = page["nextCursor"].as_str().map(str::to_owned);
            pages.push(page);
            if cursor.is_none() {
                return pages;
            }
            assert!(pages.len() < 1_000, "resources/list never ends");
        }
    }

    /// `resources/read`, the whole reply.
    pub fn read(&mut self, uri: &str) -> Value {
        self.request("resources/read", json!({"uri": uri}))
    }

    pub fn finish(self) -> Finished {
        self.server.finish()
    }
}

/// The `Env` of a CLI library call: `cwd`, `HOME`, no `XDG_DATA_HOME`.
pub fn cli_env(cwd: &Path, home: Option<&Path>) -> Env {
    Env {
        cwd: cwd.to_path_buf(),
        home: home.map(|home| home.as_os_str().to_owned()),
        xdg_data_home: None,
    }
}

/// What the CLI answers for one request.
#[derive(Debug, Clone, PartialEq)]
pub struct Expected {
    /// `spec … 2>&1`.
    pub text: String,
    /// `spec --json …`, parsed; `None` on exit 2.
    pub document: Option<Value>,
    /// Exit 1 or 2.
    pub is_error: bool,
}

/// The parity reference of one library outcome.
pub fn expected(outcome: Result<Outcome, CliError>) -> Expected {
    match outcome {
        Ok(outcome) => {
            let mut text = String::new();
            for line in outcome.stderr_lines() {
                text.push_str(&line);
                text.push('\n');
            }
            text.push_str(&render_text(&outcome));
            let json = render_json(&outcome);
            Expected {
                text,
                document: Some(serde_json::from_str(&json).expect("render_json is JSON")),
                is_error: outcome.exit() != Exit::Answered,
            }
        }
        Err(error) => Expected {
            text: format!("{}\n", error.message),
            document: None,
            is_error: true,
        },
    }
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .map(|item| item.as_str().expect("a string item").to_owned())
                .collect()
        })
        .unwrap_or_default()
}

/// The caller's git environment of a server spawned by [`Server`]: its
/// working directory and its only variable, `HOME`.
pub fn server_git(env: &Env) -> GitEnv {
    let vars: Vec<(String, std::ffi::OsString)> = env
        .home
        .iter()
        .map(|home| ("HOME".to_owned(), home.clone()))
        .collect();
    GitEnv::new(env.cwd.clone(), vars)
}

/// An optional string argument.
fn optional_text(args: &Value, key: &str) -> Option<String> {
    args[key].as_str().map(str::to_owned)
}

/// The queue tools' twins (task spec `agent-intake`, the tool table), with
/// the clock `now` and the caller's git environment `git`:
/// `propose_change` = `propose update … --brief`, `ask_question` =
/// `propose question`, `report_discrepancy` = `propose discrepancy` (the
/// arguments but the author's and `task_id` as its `--input`),
/// `get_proposal` = `review --brief`; the author's role always passed, an
/// optional `task_id` as `--task` (docs/features/task-package.md).
pub fn queue_library(
    tool: &str,
    args: &Value,
    env: &Env,
    globals: &Globals,
    now: &str,
    git: &GitEnv,
) -> Expected {
    let text = |key: &str| args[key].as_str().unwrap_or_default().to_owned();
    let task = args["task_id"].as_str();
    let outcome = match tool {
        "propose_change" => specengine_cli::propose_brief_with_task(
            env,
            globals,
            &ProposeRequest {
                target: text("target"),
                base: text("base"),
                text: ProposedText::Given(text("text").into_bytes()),
                rationale: text("rationale"),
                author_role: optional_text(args, "author_role"),
                author_model: optional_text(args, "author_model"),
                run: optional_text(args, "run"),
                now: now.to_owned(),
                git: git.clone(),
            },
            task,
        )
        .map(|outcome| Outcome::Proposal(Box::new(outcome))),
        "ask_question" => specengine_cli::propose_question_with_task(
            env,
            globals,
            &QuestionRequest {
                node_ids: strings(&args["node_ids"]),
                text: text("text"),
                working_answer: text("working_answer"),
                price_of_other: text("price_of_other"),
                severity: args["severity"]
                    .as_str()
                    .map(|name| IntakeSeverity::parse(name).expect("a severity")),
                distinct_from: strings(&args["distinct_from"]),
                author_role: optional_text(args, "author_role"),
                author_model: optional_text(args, "author_model"),
                run: optional_text(args, "run"),
                now: now.to_owned(),
                git: git.clone(),
            },
            task,
        )
        .map(|outcome| Outcome::Intake(Box::new(outcome))),
        "report_discrepancy" => {
            let mut input = args.clone();
            let object = input.as_object_mut().expect("arguments object");
            for key in ["author_role", "author_model", "run", "task_id"] {
                object.remove(key);
            }
            let input: DiscrepancyInput =
                serde_json::from_value(input).expect("a discrepancy's --input document");
            specengine_cli::propose_discrepancy_with_task(
                env,
                globals,
                &DiscrepancyRequest {
                    input,
                    author_role: optional_text(args, "author_role"),
                    author_model: optional_text(args, "author_model"),
                    run: optional_text(args, "run"),
                    now: now.to_owned(),
                    git: git.clone(),
                },
                task,
            )
            .map(|outcome| Outcome::Intake(Box::new(outcome)))
        }
        "get_proposal" => specengine_cli::review_brief(
            env,
            globals,
            &ReviewRequest {
                id: text("proposal_id"),
                git: git.clone(),
            },
        )
        .map(|outcome| Outcome::Proposal(Box::new(outcome))),
        other => panic!("no queue twin for tool {other}"),
    };
    expected(outcome)
}

/// The task tools' twins (docs/features/task-package.md "Description and
/// interactions"), with the clock `now` and the caller's git environment
/// `git`: `get_task` = `task show T | --next` (an answer naming no task is
/// an error result without `structuredContent`: the output schema is the
/// package's; not exactly one of `task_id`, `next: true`: the tool's own
/// error, no CLI twin), `claim_task` = `task claim`, `submit_plan` =
/// `task plan --plan-file -`, `report_run` = `task report`,
/// `complete_task` = `task complete`.
pub fn task_library(
    tool: &str,
    args: &Value,
    env: &Env,
    globals: &Globals,
    now: &str,
    git: &GitEnv,
) -> Expected {
    let text = |key: &str| args[key].as_str().unwrap_or_default().to_owned();
    let id = text("task_id");
    let outcome = match tool {
        "get_task" => {
            let (id, next) = match (args["task_id"].as_str(), &args["next"]) {
                (Some(id), Value::Null) => (Some(id.to_owned()), false),
                (None, Value::Bool(true)) => (None, true),
                _ => {
                    return Expected {
                        text: "spec: get_task takes exactly one of `task_id` and `next: true`\n"
                            .to_owned(),
                        document: None,
                        is_error: true,
                    };
                }
            };
            let mut want = expected(
                specengine_cli::task_show(
                    env,
                    globals,
                    &TaskShowRequest {
                        id,
                        next,
                        git: git.clone(),
                    },
                )
                .map(|outcome| Outcome::TaskShow(Box::new(outcome))),
            );
            if want.is_error {
                want.document = None;
            }
            return want;
        }
        "claim_task" => specengine_cli::task_claim(
            env,
            globals,
            &TaskClaimRequest {
                id,
                role: text("role"),
                worktree: text("worktree").into(),
                now: now.to_owned(),
                git: git.clone(),
            },
        ),
        "submit_plan" => specengine_cli::task_plan(
            env,
            globals,
            &TaskPlanRequest {
                id,
                plan: ProposedText::Given(text("plan_md").into_bytes()),
                criteria: strings(&args["criteria"]),
                affected: strings(&args["affected_nodes"]),
                now: now.to_owned(),
                git: git.clone(),
            },
        ),
        "report_run" => specengine_cli::task_report(
            env,
            globals,
            &TaskReportRequest {
                id,
                outcome: text("outcome"),
                summary: text("summary"),
                changed_files: strings(&args["changed_files"]),
                now: now.to_owned(),
                git: git.clone(),
            },
        ),
        "complete_task" => specengine_cli::task_complete(
            env,
            globals,
            &TaskCompleteRequest {
                id,
                now: now.to_owned(),
                git: git.clone(),
            },
        ),
        other => panic!("no task twin for tool {other}"),
    };
    expected(outcome.map(|outcome| Outcome::Task(Box::new(outcome))))
}

/// The same request through the CLI library (task spec, the tool table):
/// `get_tree` = `tree`, `get_node` = `show`, `search` = `search` with the
/// query as one word, `get_context_bundle` = `bundle`; a queue tool as
/// [`queue_library`] with the wall clock and the server's git environment.
pub fn library(tool: &str, args: &Value, env: &Env, globals: &Globals) -> Expected {
    if QUEUE_TOOLS.contains(&tool) {
        return queue_library(
            tool,
            args,
            env,
            globals,
            &specengine_cli::utc_now(),
            &server_git(env),
        );
    }
    if TASK_TOOLS.contains(&tool) {
        return task_library(
            tool,
            args,
            env,
            globals,
            &specengine_cli::utc_now(),
            &server_git(env),
        );
    }
    let flag = |key: &str| args[key].as_bool().unwrap_or(false);
    let number = |key: &str| args[key].as_i64();
    let text = |key: &str| args[key].as_str().map(str::to_owned);
    let outcome = match tool {
        "get_tree" => specengine_cli::tree(
            env,
            globals,
            &TreeRequest {
                root: text("root"),
                depth: number("depth"),
                kinds: strings(&args["kinds"]),
                archive: flag("archive"),
            },
        )
        .map(Outcome::Tree),
        "get_node" => specengine_cli::show(
            env,
            globals,
            &ShowRequest {
                reference: text("id").expect("get_node id"),
                links: strings(&args["with"]).iter().any(|with| with == "links"),
                archive: flag("archive"),
            },
        )
        .map(Outcome::Show),
        "search" => specengine_cli::search(
            env,
            globals,
            &SearchRequest {
                terms: vec![text("query").expect("search query")],
                kinds: strings(&args["kinds"]),
                limit: number("limit"),
                archive: flag("archive"),
            },
        )
        .map(Outcome::Search),
        "get_context_bundle" => specengine_cli::bundle(
            env,
            globals,
            &BundleRequest {
                references: strings(&args["node_ids"]),
                budget: number("budget"),
            },
        )
        .map(Outcome::Bundle),
        other => panic!("no CLI counterpart for tool {other}"),
    };
    expected(outcome)
}

/// The text of a tool result's one content block.
pub fn content_text(result: &Value) -> &str {
    let content = result["content"]
        .as_array()
        .unwrap_or_else(|| panic!("no content: {}", super::clip(&result.to_string())));
    assert_eq!(content.len(), 1, "one content block: {result}");
    assert_eq!(content[0]["type"], json!("text"), "a text block: {result}");
    content[0]["text"].as_str().expect("text")
}

/// The output schema of every tool of a `tools/list` result.
pub fn output_schemas(list: &Value) -> BTreeMap<String, Value> {
    list["tools"]
        .as_array()
        .expect("tools")
        .iter()
        .filter_map(|tool| {
            Some((
                tool["name"].as_str()?.to_owned(),
                tool.get("outputSchema")?.clone(),
            ))
        })
        .collect()
}

/// Asserts a tool result equals the CLI's answer and its
/// `structuredContent` conforms to `schema`.
pub fn assert_parity(reply: &Value, expected: &Expected, schema: &Value, context: &str) {
    let result = result(reply);
    assert_eq!(
        content_text(result),
        expected.text,
        "{context}: content differs from `spec … 2>&1`"
    );
    assert_eq!(
        result["isError"],
        json!(expected.is_error),
        "{context}: isError"
    );
    match &expected.document {
        Some(document) => {
            assert_eq!(
                &result["structuredContent"], document,
                "{context}: structuredContent differs from `spec --json …`"
            );
            if let Err(problem) = conforms(schema, &result["structuredContent"], "$") {
                panic!("{context}: structuredContent breaks its outputSchema: {problem}");
            }
        }
        None => assert!(
            result.get("structuredContent").is_none(),
            "{context}: structuredContent on exit 2: {result}"
        ),
    }
}

/// One tool call checked against the library: parity and schema; returns
/// the tool result.
pub fn checked_call(
    session: &mut Session,
    schemas: &BTreeMap<String, Value>,
    tool: &str,
    args: Value,
    env: &Env,
    globals: &Globals,
) -> Value {
    let reply = session.call(tool, args.clone());
    let want = library(tool, &args, env, globals);
    let schema = schemas
        .get(tool)
        .unwrap_or_else(|| panic!("{tool} has no outputSchema"));
    assert_parity(
        &reply,
        &want,
        schema,
        &format!("{:?} {tool} {args}", session.era),
    );
    result(&reply).clone()
}

fn type_matches(name: &str, value: &Value) -> bool {
    match name {
        "string" => value.is_string(),
        "integer" => value.is_i64() || value.is_u64(),
        "number" => value.is_number(),
        "boolean" => value.is_boolean(),
        "null" => value.is_null(),
        "array" => value.is_array(),
        "object" => value.is_object(),
        _ => false,
    }
}

/// Whether `value` conforms to `schema` (the subset the output schemas
/// use: `type`, `enum`, `const`, `properties`, `required`, `items`,
/// `minimum`, `anyOf`/`oneOf`/`allOf`), with the stricter rule of AC-14: an
/// object holds exactly its schema's properties and every property is
/// `required`. A `$ref` is refused (every subschema is inlined).
pub fn conforms(schema: &Value, value: &Value, at: &str) -> Result<(), String> {
    let object = match schema {
        Value::Bool(true) => return Ok(()),
        Value::Bool(false) => return Err(format!("{at}: the schema `false`")),
        Value::Object(object) => object,
        other => return Err(format!("{at}: not a schema: {other}")),
    };
    if object.contains_key("$ref") {
        return Err(format!("{at}: a $ref"));
    }
    if let Some(allowed) = object.get("enum") {
        let allowed = allowed.as_array().ok_or(format!("{at}: enum not a list"))?;
        if !allowed.contains(value) {
            return Err(format!("{at}: {value} not in {allowed:?}"));
        }
    }
    if let Some(constant) = object.get("const")
        && constant != value
    {
        return Err(format!("{at}: {value} is not the const {constant}"));
    }
    if let Some(kind) = object.get("type") {
        let names: Vec<&str> = match kind {
            Value::String(name) => vec![name.as_str()],
            Value::Array(names) => names.iter().filter_map(Value::as_str).collect(),
            other => return Err(format!("{at}: bad type {other}")),
        };
        if !names.iter().any(|name| type_matches(name, value)) {
            return Err(format!(
                "{at}: {} is not {names:?}",
                super::clip(&value.to_string())
            ));
        }
    }
    if let (Some(minimum), Some(number)) = (object.get("minimum"), value.as_f64())
        && number < minimum.as_f64().unwrap_or(f64::MIN)
    {
        return Err(format!("{at}: {number} below the minimum {minimum}"));
    }
    if let Some(all) = object.get("allOf").and_then(Value::as_array) {
        for (index, part) in all.iter().enumerate() {
            conforms(part, value, &format!("{at}/allOf{index}"))?;
        }
    }
    for key in ["anyOf", "oneOf"] {
        if let Some(options) = object.get(key).and_then(Value::as_array) {
            let matching = options
                .iter()
                .filter(|option| conforms(option, value, at).is_ok())
                .count();
            let ok = if key == "oneOf" {
                matching == 1
            } else {
                matching >= 1
            };
            if !ok {
                return Err(format!("{at}: {matching} of {key} match {value}"));
            }
        }
    }
    if let (Some(properties), Some(fields)) = (
        object.get("properties").and_then(Value::as_object),
        value.as_object(),
    ) {
        let declared: Vec<&String> = properties.keys().collect();
        let present: Vec<&String> = fields.keys().collect();
        if declared != present {
            return Err(format!(
                "{at}: keys {present:?}, the schema declares {declared:?}"
            ));
        }
        let required: Vec<&str> = object
            .get("required")
            .and_then(Value::as_array)
            .map(|names| names.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        for name in &declared {
            if !required.contains(&name.as_str()) {
                return Err(format!("{at}: property {name} is not required"));
            }
        }
        for (name, subschema) in properties {
            conforms(subschema, &fields[name], &format!("{at}.{name}"))?;
        }
    }
    if let (Some(items), Some(values)) = (object.get("items"), value.as_array()) {
        for (index, item) in values.iter().enumerate() {
            conforms(items, item, &format!("{at}[{index}]"))?;
        }
    }
    Ok(())
}

/// Data's flat rule for an input schema: an object of strings, integers,
/// booleans and string arrays (each optionally `[T, "null"]`), no root
/// `anyOf`/`oneOf`/`allOf`, no `$ref` anywhere, `additionalProperties:
/// false`; `kinds` items free strings, never an `enum`.
pub fn flat_input_problems(tool: &str, schema: &Value) -> Vec<String> {
    let mut problems = Vec::new();
    if schema.to_string().contains("\"$ref\"") {
        problems.push(format!("{tool}: a $ref"));
    }
    for key in ["anyOf", "oneOf", "allOf", "$defs", "definitions"] {
        if schema.get(key).is_some() {
            problems.push(format!("{tool}: root {key}"));
        }
    }
    if schema["type"] != json!("object") {
        problems.push(format!("{tool}: root type {}", schema["type"]));
    }
    if schema["additionalProperties"] != json!(false) {
        problems.push(format!("{tool}: additionalProperties is not false"));
    }
    let Some(properties) = schema["properties"].as_object() else {
        problems.push(format!("{tool}: no properties"));
        return problems;
    };
    let base_type = |property: &Value| -> Option<String> {
        match &property["type"] {
            Value::String(name) => Some(name.clone()),
            Value::Array(names) => {
                let names: Vec<&str> = names.iter().filter_map(Value::as_str).collect();
                match names.as_slice() {
                    [name] => Some((*name).to_owned()),
                    [name, "null"] | ["null", name] => Some((*name).to_owned()),
                    _ => None,
                }
            }
            _ => None,
        }
    };
    for (name, property) in properties {
        for key in ["anyOf", "oneOf", "allOf", "properties"] {
            if property.get(key).is_some() {
                problems.push(format!("{tool}.{name}: nested {key}"));
            }
        }
        match base_type(property).as_deref() {
            Some("string" | "integer" | "boolean") => {}
            Some("array") => {
                let items = &property["items"];
                if items["type"] != json!("string") {
                    problems.push(format!("{tool}.{name}: items are not strings: {items}"));
                }
                for key in ["anyOf", "oneOf", "allOf", "properties", "items"] {
                    if items.get(key).is_some() {
                        problems.push(format!("{tool}.{name}: items with {key}"));
                    }
                }
                if name == "kinds" && items.get("enum").is_some() {
                    problems.push(format!("{tool}.kinds: an enum"));
                }
            }
            other => problems.push(format!("{tool}.{name}: type {other:?}")),
        }
    }
    problems
}

/// `text` has `word` as a whole word (ASCII-case-insensitive; a word
/// character is an ASCII letter, digit, `_` or `-`), as the CLI tests.
pub fn has_word(text: &str, word: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    let word = word.to_ascii_lowercase();
    let is_word = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '-';
    lower.match_indices(&word).any(|(at, _)| {
        let before = lower[..at].chars().next_back();
        let after = lower[at + word.len()..].chars().next();
        !before.is_some_and(is_word) && !after.is_some_and(is_word)
    })
}

/// The library's `index` on a project, so its database exists.
pub fn index_with_library(env: &Env, globals: &Globals) {
    specengine_cli::index(env, globals, &specengine_cli::IndexRequest::default())
        .unwrap_or_else(|error| panic!("spec index: {error}"));
}

/// The prefix of rmcp's `Parameters` message for arguments that break the
/// input schema (a wrong type, an unknown key, a value outside an enum).
pub const BAD_ARGUMENTS: &str = "failed to deserialize parameters: ";

/// AC-05 as amended: a schema-invalid argument is a tool result, never a
/// JSON-RPC error: `isError: true`, one text block holding rmcp's
/// `Parameters` message (naming `needle`), no `structuredContent`.
pub fn assert_bad_arguments(reply: &Value, needle: &str, context: &str) {
    assert!(
        reply.get("error").is_none(),
        "{context}: a JSON-RPC error for a bad argument: {}",
        super::clip(&reply.to_string())
    );
    let result = result(reply);
    assert_eq!(result["isError"], json!(true), "{context}: {result}");
    assert!(
        result.get("structuredContent").is_none(),
        "{context}: structuredContent on a bad argument: {result}"
    );
    let text = content_text(result);
    assert!(
        text.starts_with(BAD_ARGUMENTS) && text.contains(needle),
        "{context}: want {BAD_ARGUMENTS:?} naming {needle:?}, got {text:?}"
    );
}
