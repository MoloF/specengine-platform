//! The read tools (task spec `mcp-read`): `get_tree`, `get_node`, `search`,
//! `get_context_bundle`, each one call into the CLI library.
//!
//! A call runs on the blocking pool (`spawn_blocking`): `Env` from the
//! process, the server's `--root` / `--config` as the CLI globals, one
//! library function, its rendering. Nothing is kept after it. The answer is
//! the CLI's, byte for byte: `content` = its stderr lines then its stdout
//! text (`spec … 2>&1`), `structuredContent` = its `--json` document parsed.
//! Exit 1 is an error result with that document; exit 2 an error result with
//! the error line(s) alone; a panic an error result naming the tool. An
//! argument the input schema refuses (a wrong type, an unknown name, a value
//! outside an enum) is rmcp's `Parameters` error result before any call
//! (`isError: true`, no `structuredContent`; SEP-1303), never a JSON-RPC
//! error.

use std::borrow::Cow;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, JsonObject, MetaObject};
use rmcp::{ErrorData, schemars, tool, tool_router};
use serde::Deserialize;
use serde_json::Value;
use specengine_cli::{
    BundleRequest, CliError, DEFAULT_BUNDLE_BUDGET, Env, Exit, Globals, MIN_TERM_CHARS,
    OUTPUT_CAP_CHARS, Outcome, SEARCH_LIMIT_DEFAULT, SEARCH_LIMIT_MAX, SEARCH_LIMIT_MIN,
    SHOW_TAIL_NAMES, SearchRequest, ShowRequest, TreeRequest, render_json, render_text,
};

use crate::mirror::{
    BundleDocument, SearchDocument, ShowDocument, TreeDocument, input_schema, output_schema,
};
use crate::server::SpecEngineServer;

/// Tool `_meta` key: the result size Claude Code accepts from this tool
/// (A12 of the task spec; checked by the owner, AC-16).
pub(crate) const MAX_RESULT_SIZE_KEY: &str = "anthropic/maxResultSizeChars";

/// Characters a read tool's result may hold, text and structured part
/// together. The text is at most [`OUTPUT_CAP_CHARS`] (40 000) plus a
/// bounded tail (at most [`SHOW_TAIL_NAMES`] names per list) and the `note:`
/// lines; the structured part is the same document as JSON, escaping a
/// character into at most 6; the measured adversarial maximum is about
/// 268 000. Not bounded by it, the corpus-defect residue: many holders of
/// one ID (plain `show` JSON, without `--links`, keeps every holder's node:
/// about 1 700 holders of one ID pass 500 000) and the `warning:` lines on
/// stderr that list corpus defects.
pub const MAX_RESULT_CHARS: u64 = 500_000;

/// Claude Code truncates tool descriptions at this length
/// (`crates/specengine-mcp/README.md`, "Claude Code client").
pub(crate) const DESCRIPTION_LIMIT: usize = 2048;

/// Every read tool's description ends with this sentence (07 §1.1).
macro_rules! common_tail {
    () => {
        "Reads the files as they are now: refreshes SpecEngine's index in its data \
directory, writes nothing under the project root. Deterministic: one state, one \
result; no LLM inside."
    };
}

const TREE_DESCRIPTION: &str = concat!(
    "The containment tree of the project's spec (`parent:` links and sections \
inside documents), without node bodies: one line per node, indented by depth, with \
its ID (else its path), kind, title, path:line and status, then a count line. The \
same as `spec tree [ROOT] [--depth N] [--kind K]... [--archive]`: content is the \
command's output (its note and warning lines first), structuredContent its --json \
document.

root: start here; any REF get_node takes. Absent: every root document under \
`[paths] spec`.
depth: levels below the roots at most; 0 = the roots only. Absent: unbounded.
kinds: keep only the lines of these node kinds (free strings, as the project's \
config names them); depth and parent stay those of the whole tree.
archive: include Tier 3 (archived) documents.

An answer above 40000 characters is cut, with a tail saying how to narrow it (it \
names the command's flags: ROOT is root, --kind is kinds). A root naming nothing: \
an error result with the reason, and the document. A call that cannot run (no \
project, bad argument value): an error result with its one line. ",
    common_tail!()
);

const NODE_DESCRIPTION: &str = concat!(
    "A node's current text: a document's whole file or an ID section's span, read \
from disk now, with a header line (ID, kind, title, path:lines, status, rev, \
estimated tokens) per node. The same as `spec show REF [--links] [--archive]`: \
content is the command's output (its note and warning lines first), \
structuredContent its --json document.

id: a REF: an ID, an alias (`aliases:` or a configured legacy prefix), `slug/ID`, \
`ID#SECTION`, or a root-relative `.md` path. An ID several documents hold gives all \
of them, with a warning.
with: [\"links\"] lists each node's links, outgoing and incoming, before its text.
archive: with [\"links\"] only: links written in Tier 3 (archived) documents too.

An answer above 40000 characters is cut, with a tail naming the lines not shown and \
the first 20 sections and holders not shown, the rest counted (read a section by its \
ID). A REF naming nothing: an error result with the reason, and the document. A look-alike or mixed-script ID (refused with its \
Latin fix), a `project:` REF or another call that cannot run: an error result with \
its one line. ",
    common_tail!()
);

const SEARCH_DESCRIPTION: &str = concat!(
    "Full-text search over the spec's nodes: best first, one line per hit with its \
ID (else its path), kind, title, path:line and a snippet. Use it instead of listing \
everything. The same as `spec search QUERY [--kind K]... [--limit N] [--archive]`: \
content is the command's output (its note lines first), structuredContent its \
--json document.

query: the words to find; terms shorter than 3 characters are dropped with a note (a \
query of only such terms cannot run).
kinds: keep only nodes of these kinds (free strings, as the project's config names \
them).
limit: hits at most, 1 to 200; absent: 20.
archive: include Tier 3 (archived) documents; without it they are counted, not \
shown.

An answer above 40000 characters is cut, with a tail saying how to narrow it (it \
names the command's flags: --limit is limit). Zero hits is a result, not an error. \
A call that cannot run: an error result with its one line. ",
    common_tail!()
);

const BUNDLE_DESCRIPTION: &str = concat!(
    "The context of the named nodes within a budget of estimated tokens, as one \
Markdown body: the targets, their open questions, ancestors, criteria, decisions, \
neighbours and terms, in that order; what did not fit is named by ID with its cost. \
The body is named by its bundle_hash (BLAKE3 of the body): the same state and \
request give the same hash. The same as `spec bundle REF... [--budget N]`: content \
is the command's output (its note and warning lines first), structuredContent its \
--json document.

node_ids: one or more REFs, any form get_node takes; bundled together.
budget: estimated tokens at most. Absent: `[budgets] bundle_node` of the project's \
config, else 2000. Below the minimum the targets need: an error naming it.

A REF naming nothing: an error result with the reason, and the document. A call \
that cannot run: an error result with its one line. ",
    common_tail!()
);

/// The sentence every description holds (07 §1.1; AC-01).
pub(crate) const DETERMINISM: &str = "Deterministic: one state, one result; no LLM inside.";

/// `text` holds `part`.
pub(crate) const fn holds(text: &str, part: &str) -> bool {
    holds_bytes(text.as_bytes(), part.as_bytes(), part.len())
}

/// `text` holds the first `len` bytes of `part`.
const fn holds_bytes(text: &[u8], part: &[u8], len: usize) -> bool {
    let mut start = 0;
    while start + len <= text.len() {
        let mut index = 0;
        while index < len && text[start + index] == part[index] {
            index += 1;
        }
        if index == len {
            return true;
        }
        start += 1;
    }
    false
}

/// `text` holds `before`, `number` in decimal, then `after`: ties a number
/// written in a description to the CLI constant it states (a compile error
/// when they part). `before` and `after` together at most 100 bytes.
pub(crate) const fn holds_number(text: &str, before: &str, number: usize, after: &str) -> bool {
    let mut part = [0u8; 128];
    let mut len = 0;
    let before = before.as_bytes();
    let mut index = 0;
    while index < before.len() {
        part[len] = before[index];
        len += 1;
        index += 1;
    }
    let mut digits = [0u8; 20];
    let mut count = 0;
    let mut rest = number;
    loop {
        digits[count] = b'0' + (rest % 10) as u8;
        count += 1;
        rest /= 10;
        if rest == 0 {
            break;
        }
    }
    while count > 0 {
        count -= 1;
        part[len] = digits[count];
        len += 1;
    }
    let after = after.as_bytes();
    index = 0;
    while index < after.len() {
        part[len] = after[index];
        len += 1;
        index += 1;
    }
    holds_bytes(text.as_bytes(), &part, len)
}

// ASCII only, so bytes equal characters.
const _: () = assert!(TREE_DESCRIPTION.len() <= DESCRIPTION_LIMIT);
const _: () = assert!(NODE_DESCRIPTION.len() <= DESCRIPTION_LIMIT);
const _: () = assert!(SEARCH_DESCRIPTION.len() <= DESCRIPTION_LIMIT);
const _: () = assert!(BUNDLE_DESCRIPTION.len() <= DESCRIPTION_LIMIT);
const _: () = assert!(TREE_DESCRIPTION.is_ascii());
const _: () = assert!(NODE_DESCRIPTION.is_ascii());
const _: () = assert!(SEARCH_DESCRIPTION.is_ascii());
const _: () = assert!(BUNDLE_DESCRIPTION.is_ascii());
const _: () = assert!(holds(TREE_DESCRIPTION, DETERMINISM));
const _: () = assert!(holds(NODE_DESCRIPTION, DETERMINISM));
const _: () = assert!(holds(SEARCH_DESCRIPTION, DETERMINISM));
const _: () = assert!(holds(BUNDLE_DESCRIPTION, DETERMINISM));
// The numbers the descriptions state are the CLI's.
const _: () = assert!(holds_number(
    TREE_DESCRIPTION,
    "above ",
    OUTPUT_CAP_CHARS,
    " characters"
));
const _: () = assert!(holds_number(
    NODE_DESCRIPTION,
    "above ",
    OUTPUT_CAP_CHARS,
    " characters"
));
const _: () = assert!(holds_number(
    NODE_DESCRIPTION,
    "the first ",
    SHOW_TAIL_NAMES,
    " sections and holders"
));
const _: () = assert!(holds_number(
    SEARCH_DESCRIPTION,
    "above ",
    OUTPUT_CAP_CHARS,
    " characters"
));
const _: () = assert!(holds_number(
    SEARCH_DESCRIPTION,
    "shorter than ",
    MIN_TERM_CHARS,
    " characters"
));
const _: () = assert!(holds_number(
    SEARCH_DESCRIPTION,
    "hits at most, ",
    SEARCH_LIMIT_MIN,
    " to "
));
const _: () = assert!(holds_number(
    SEARCH_DESCRIPTION,
    " to ",
    SEARCH_LIMIT_MAX,
    "; absent: "
));
const _: () = assert!(holds_number(
    SEARCH_DESCRIPTION,
    "; absent: ",
    SEARCH_LIMIT_DEFAULT,
    ".\n"
));
const _: () = assert!(holds_number(
    BUNDLE_DESCRIPTION,
    "config, else ",
    DEFAULT_BUNDLE_BUDGET as usize,
    ". "
));
const _: () = assert!(holds_number(
    QUERY_DOC,
    "terms of ",
    MIN_TERM_CHARS,
    " or more"
));
const _: () = assert!(holds_number(
    LIMIT_DOC,
    "at most, ",
    SEARCH_LIMIT_MIN,
    " to "
));
const _: () = assert!(holds_number(LIMIT_DOC, " to ", SEARCH_LIMIT_MAX, ". "));
const _: () = assert!(holds_number(
    LIMIT_DOC,
    "Absent: ",
    SEARCH_LIMIT_DEFAULT,
    "."
));
const _: () = assert!(holds_number(
    BUDGET_DOC,
    "else ",
    DEFAULT_BUNDLE_BUDGET as usize,
    "."
));

/// A read tool's `_meta`: `{"anthropic/maxResultSizeChars": 500000}`.
pub(crate) fn result_size_meta() -> MetaObject {
    let mut meta = MetaObject::new();
    meta.insert(
        MAX_RESULT_SIZE_KEY.to_owned(),
        Value::from(MAX_RESULT_CHARS),
    );
    meta
}

/// `get_tree` arguments.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct TreeArgs {
    /// Start here: any REF get_node takes. Absent: the root documents.
    pub root: Option<String>,
    /// Levels below the roots at most; 0 = the roots only. Absent: unbounded.
    pub depth: Option<i64>,
    /// Keep only the lines of these node kinds.
    pub kinds: Option<Vec<String>>,
    /// Include Tier 3 (archived) documents.
    pub archive: Option<bool>,
}

/// `get_node` arguments.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct NodeArgs {
    /// An ID, an alias, `slug/ID`, `ID#SECTION` or a root-relative `.md` path.
    pub id: String,
    /// `["links"]`: each node's links, outgoing and incoming.
    pub with: Option<Vec<With>>,
    /// With `["links"]` only: links written in Tier 3 (archived) documents too.
    pub archive: Option<bool>,
}

/// What `get_node` adds to a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum With {
    /// `spec show --links`.
    Links,
}

/// `{"type": "string", "enum": ["links"]}`, inlined (the derive would write
/// a `oneOf` of `const`s).
impl schemars::JsonSchema for With {
    fn inline_schema() -> bool {
        true
    }

    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("With")
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        let mut object = JsonObject::new();
        object.insert("type".to_owned(), Value::from("string"));
        object.insert("enum".to_owned(), Value::from(vec![Value::from("links")]));
        schemars::Schema::from(object)
    }
}

/// The input schema's description of `search.query`.
const QUERY_DOC: &str = "The words to find (terms of 3 or more characters).";

/// The input schema's description of `search.limit`.
const LIMIT_DOC: &str = "Hits at most, 1 to 200. Absent: 20.";

/// The input schema's description of `get_context_bundle.budget`.
const BUDGET_DOC: &str = "Estimated tokens at most. Absent: `[budgets] bundle_node`, else 2000.";

/// `search` arguments.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct SearchArgs {
    #[schemars(description = QUERY_DOC)]
    pub query: String,
    /// Keep only nodes of these kinds.
    pub kinds: Option<Vec<String>>,
    #[schemars(description = LIMIT_DOC)]
    pub limit: Option<i64>,
    /// Include Tier 3 (archived) documents.
    pub archive: Option<bool>,
}

/// `get_context_bundle` arguments.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct BundleArgs {
    /// One or more REFs, any form get_node takes.
    pub node_ids: Vec<String>,
    #[schemars(description = BUDGET_DOC)]
    pub budget: Option<i64>,
}

#[tool_router(router = read_tools, vis = "pub(crate)")]
impl SpecEngineServer {
    /// `spec tree`.
    #[tool(
        description = TREE_DESCRIPTION,
        input_schema = input_schema::<TreeArgs>(),
        output_schema = output_schema::<TreeDocument>(),
        annotations(read_only_hint = true, destructive_hint = false, open_world_hint = false),
        meta = result_size_meta()
    )]
    async fn get_tree(
        &self,
        Parameters(args): Parameters<TreeArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let request = TreeRequest {
            root: args.root,
            depth: args.depth,
            kinds: args.kinds.unwrap_or_default(),
            archive: args.archive.unwrap_or(false),
        };
        Ok(self
            .call(move |env, globals| {
                specengine_cli::tree(env, globals, &request).map(Outcome::Tree)
            })
            .await
            .into_tool_result("get_tree"))
    }

    /// `spec show`.
    #[tool(
        description = NODE_DESCRIPTION,
        input_schema = input_schema::<NodeArgs>(),
        output_schema = output_schema::<ShowDocument>(),
        annotations(read_only_hint = true, destructive_hint = false, open_world_hint = false),
        meta = result_size_meta()
    )]
    async fn get_node(
        &self,
        Parameters(args): Parameters<NodeArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let request = ShowRequest {
            reference: args.id,
            links: args.with.is_some_and(|with| with.contains(&With::Links)),
            archive: args.archive.unwrap_or(false),
        };
        Ok(self
            .call(move |env, globals| {
                specengine_cli::show(env, globals, &request).map(Outcome::Show)
            })
            .await
            .into_tool_result("get_node"))
    }

    /// `spec search`.
    #[tool(
        description = SEARCH_DESCRIPTION,
        input_schema = input_schema::<SearchArgs>(),
        output_schema = output_schema::<SearchDocument>(),
        annotations(read_only_hint = true, destructive_hint = false, open_world_hint = false),
        meta = result_size_meta()
    )]
    async fn search(
        &self,
        Parameters(args): Parameters<SearchArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        // One shell word: `spec search "<query>"`.
        let request = SearchRequest {
            terms: vec![args.query],
            kinds: args.kinds.unwrap_or_default(),
            limit: args.limit,
            archive: args.archive.unwrap_or(false),
        };
        Ok(self
            .call(move |env, globals| {
                specengine_cli::search(env, globals, &request).map(Outcome::Search)
            })
            .await
            .into_tool_result("search"))
    }

    /// `spec bundle`.
    #[tool(
        description = BUNDLE_DESCRIPTION,
        input_schema = input_schema::<BundleArgs>(),
        output_schema = output_schema::<BundleDocument>(),
        annotations(read_only_hint = true, destructive_hint = false, open_world_hint = false),
        meta = result_size_meta()
    )]
    async fn get_context_bundle(
        &self,
        Parameters(args): Parameters<BundleArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let request = BundleRequest {
            references: args.node_ids,
            budget: args.budget,
        };
        Ok(self
            .call(move |env, globals| {
                specengine_cli::bundle(env, globals, &request).map(Outcome::Bundle)
            })
            .await
            .into_tool_result("get_context_bundle"))
    }
}

/// What one CLI library call gave, rendered on the blocking pool.
pub(crate) enum Answer {
    /// The command answered (exit 0) or found nothing (exit 1).
    Rendered {
        exit: Exit,
        /// Stderr lines, then stdout: `spec … 2>&1`.
        text: String,
        /// The `--json` document as `render_json` gives it, parsed.
        document: Value,
    },
    /// The command could not run (exit 2).
    CannotRun(CliError),
    /// The `--json` document did not parse (never expected): the parser's
    /// message.
    Unparsed(String),
    /// The call panicked.
    Panicked,
}

impl Answer {
    /// The tool result of the mapping table (task spec, "Data").
    pub(crate) fn into_tool_result(self, tool: &str) -> CallToolResult {
        match self {
            Self::Rendered {
                exit,
                text,
                document,
            } => {
                let content = vec![ContentBlock::text(text)];
                let mut result = if exit == Exit::Answered {
                    CallToolResult::success(content)
                } else {
                    CallToolResult::error(content)
                };
                result.structured_content = Some(document);
                result
            }
            Self::CannotRun(error) => {
                CallToolResult::error(vec![ContentBlock::text(error_text(&error))])
            }
            Self::Unparsed(error) => CallToolResult::error(vec![ContentBlock::text(format!(
                "internal error: {tool} failed: its --json document does not parse: {error}"
            ))]),
            Self::Panicked => CallToolResult::error(vec![ContentBlock::text(format!(
                "internal error: {tool} failed"
            ))]),
        }
    }
}

/// The text `spec … 2>&1` prints for an outcome: its stderr lines, then its
/// stdout.
pub(crate) fn outcome_text(outcome: &Outcome) -> String {
    let mut text = String::new();
    for line in outcome.stderr_lines() {
        text.push_str(&line);
        text.push('\n');
    }
    text.push_str(&render_text(outcome));
    text
}

/// The text `spec … 2>&1` prints when the command cannot run: its error
/// line(s).
pub(crate) fn error_text(error: &CliError) -> String {
    format!("{error}\n")
}

impl SpecEngineServer {
    /// Runs `command` once on the blocking pool with the process's `Env` and
    /// the server's globals, and renders its outcome there. A blocking call
    /// always runs to its end (its only write: the index); for a cancelled
    /// request rmcp drops the answer, and `ping` is served meanwhile.
    pub(crate) async fn call<F>(&self, command: F) -> Answer
    where
        F: FnOnce(&Env, &Globals) -> Result<Outcome, CliError> + Send + 'static,
    {
        let globals = self.globals().clone();
        let joined = tokio::task::spawn_blocking(move || {
            let outcome = Env::from_process().and_then(|env| command(&env, &globals));
            match outcome {
                Ok(outcome) => match serde_json::from_str(&render_json(&outcome)) {
                    Ok(document) => Answer::Rendered {
                        exit: outcome.exit(),
                        text: outcome_text(&outcome),
                        document,
                    },
                    Err(error) => Answer::Unparsed(error.to_string()),
                },
                Err(error) => Answer::CannotRun(error),
            }
        })
        .await;
        joined.unwrap_or(Answer::Panicked)
    }
}
