//! The read tools (task spec `mcp-read`): `get_tree`, `get_node`, `search`,
//! `get_context_bundle`, `get_roadmap` (task spec `roadmap-surfaces`:
//! `spec roadmap`, the queue read as it is, no schema step) and
//! `get_impact` (task specs `impact`, `impact-since`: `spec impact REF |
//! --symbol QPATH | --since REV [--depth N]`, exactly one of `id`, `symbol`
//! and `since` told at run time before any discovery, as `find_symbols`'
//! forms), each one call into the CLI library.
//!
//! A call runs on the blocking pool (`spawn_blocking`): `Env` from the
//! process, the server's `--root` / `--config` as the CLI globals, one
//! library function, its rendering. Nothing is kept after it. The answer is
//! the CLI's, byte for byte: `content` = its stderr lines then its stdout
//! text (`spec … 2>&1`), `structuredContent` = its `--json` document parsed.
//! Exit 1 is an error result with that document, and so is an exit 2 the
//! command renders (a report, as `spec verify`'s run causes); an exit 2 that
//! renders nothing (the `CliError`) an error result with the error line(s)
//! alone; a panic an error result naming the tool. An
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
    BundleRequest, CliError, DEFAULT_BUNDLE_BUDGET, Env, Exit, Globals, ImpactRequest,
    MIN_TERM_CHARS, OUTPUT_CAP_CHARS, Outcome, PACKAGE_BUDGET, RoadmapRequest,
    SEARCH_LIMIT_DEFAULT, SEARCH_LIMIT_MAX, SEARCH_LIMIT_MIN, SHOW_TAIL_NAMES, SearchRequest,
    ShowRequest, TreeRequest, View, process_git, render_json, render_text,
};

use crate::mirror::{
    BundleDocument, ImpactDocument, RoadmapDocument, SearchDocument, ShowDocument, TreeDocument,
    input_schema, output_schema,
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
/// 268 000. `get_task`'s package and its notes keep within
/// [`PACKAGE_BUDGET`] (460 000) by leaving out diff texts, then criteria
/// references' texts, each such text cut at 8 192 B beforehand, then its
/// code binding rows (at most 64 before it); its brief
/// adds at most 40 000. Not bounded by it, the corpus-defect residue: many
/// holders of one ID (plain `show` JSON, without `--links`, keeps every
/// holder's node: about 1 700 holders of one ID pass 500 000) and the
/// `warning:` lines on stderr that list corpus defects; a package whose
/// other fields alone pass the budget (agent text escaped six-fold, many
/// owner notes or open proposals, long corpus titles and paths).
pub const MAX_RESULT_CHARS: u64 = 500_000;

// A package at its budget and a brief at its cap fit the result.
const _: () = assert!((PACKAGE_BUDGET + OUTPUT_CAP_CHARS) as u64 <= MAX_RESULT_CHARS);

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
estimated tokens) per node. The same as `spec show REF [--links] [--bindings] \
[--archive]`: content is the command's output (its note and warning lines first), \
structuredContent its --json document.

id: a REF: an ID, an alias (`aliases:` or a configured legacy prefix), `slug/ID`, \
`ID#SECTION`, or a root-relative `.md` path. An ID several documents hold gives all \
of them, with a warning.
with: [\"links\"] lists each node's links, outgoing and incoming, before its text.
with: [\"bindings\"] lists each node's code bindings, its sections' too, each with its sync \
(the state spec verify --node gives), after its links, with tested: the owner's test evidence \
for the node and its sections, as plain spec verify judges it now ([] when none, null when not \
judged); without a [code] table they are null, \
and a code layer that cannot be read gives a note, never an error.
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
Markdown body: the targets, their open questions, ancestors, criteria, code bindings \
(each with its sync: the state spec verify --node gives), decisions, neighbours, terms \
and the tests bound to them or their criteria, in that order; what did not fit is \
named by ID (a binding by its symbol, else its file) with its cost. \
The body is named by its bundle_hash (BLAKE3 of the body): the same state and \
request give the same hash. The same as `spec bundle REF... [--budget N]`: content \
is the command's output (its note and warning lines first), structuredContent its \
--json document.

node_ids: one or more REFs, any form get_node takes; bundled together.
budget: estimated tokens at most. Absent: `[budgets] bundle_node` of the project's \
config, else 2000. Below the minimum the targets need: an error naming it.

Code bindings, tests and the tested lines under a placed target's or criterion's header (the \
owner's test evidence, as plain spec verify judges it now) need a [code] table; a code layer \
that cannot be read gives a note, never an error.

A REF naming nothing: an error result with the reason, and the document. A call \
that cannot run: an error result with its one line. ",
    common_tail!()
);

const ROADMAP_DESCRIPTION: &str = concat!(
    "The plan of the project's roadmap items (the item documents its config's `[roadmap]` \
names): each release in the config's order with its open items by their depends_on chain, \
then priority, each with its position, status, priority, estimate and the open items it \
waits on; range sums per release, to the cut and in total; the current release; then the \
items in no release (the pool) and those in a release the config does not list. An item an \
open task of this repository executes names the task and its state; the open tasks \
executing no open item follow the pool. The same as `spec roadmap [--release R | --pool]`: \
content is the command's output (its note lines first), structuredContent its --json \
document.

release: only that release's items, as the config names it. pool: true: only the pool and \
the unlisted ones. Absent: every group. release with pool: true cannot run.

The order is advisory: get_task with next: true reads it, and nothing waits on it. Reads \
only: the items are spec files, changed only through propose_change and the owner's decision \
on a terminal.

An answer above 40000 characters is cut, with a tail saying how to narrow it (it names the \
command's flags: --release R is release, --pool is pool). A release the config does not \
name: an error result with the reason, and the document. A call that cannot run (no \
project, no `[roadmap]` in the config, release with pool: true): an error result with its \
one line. ",
    common_tail!()
);

const IMPACT_DESCRIPTION: &str = "What a change to a node, a code symbol or the code since a \
revision reaches: the nodes the impact walk reaches from it, each with its distance; the code \
bound to them and to the sections within them, with each binding's sync; the tests bound to them; \
and this repository's open tasks naming them. The same as `spec impact REF | --symbol QPATH | \
--since REV [--depth N]`: content is the command's output, structuredContent its --json document.

id: a REF, any form get_node takes: its nodes start the walk (an ID several documents hold: all, \
with a warning).
symbol: a code symbol, looked up as find_symbols' query: the walk starts at the nodes whose \
markers bind an item of its best tier, an item enclosing one, or its file; a mention never starts \
it. Needs a `[code]` table.
since: a git revision: the walk starts at the nodes bound to the code items changed on disk since \
it, untracked ones too. Needs `[code]` and git.
depth: links from the start at most; 0 = the start only. Absent: unbounded.
Give exactly one of id, symbol and since: anything else is an error result naming them.

The walk is `spec graph --impact`'s: what depends on, derives from, verifies or uses the terms of \
a reached node, and what it constrains. Nothing here changes a status, a verdict or a gate. \
Without a `[code]` table, id answers with no code and no tests; a binding that cannot be judged \
(no git, an unreadable lock) is listed with an unknown sync and one note. An answer above 40000 \
characters is cut, the counts first, with a tail counting the rest (it names --depth N: depth). A \
REF, symbol or since naming nothing: an error result with the reason, and the document. A call \
that cannot run (a look-alike ID, refused with its Latin fix; symbol or since without `[code]` or \
with an incomplete walk; since outside git): an error result with its one line. Reads the files \
as they are now: refreshes SpecEngine's indexes of the spec and the code in its data directory, \
writes nothing under the project root. Deterministic: one state, one result; no LLM inside.";

/// `get_impact` given not exactly one of `id`, `symbol` and `since`.
const ONE_INPUT: &str = "spec: get_impact takes exactly one of `id`, `symbol` and `since`\n";

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
const _: () = assert!(ROADMAP_DESCRIPTION.len() <= DESCRIPTION_LIMIT);
const _: () = assert!(IMPACT_DESCRIPTION.len() <= DESCRIPTION_LIMIT);
const _: () = assert!(TREE_DESCRIPTION.is_ascii());
const _: () = assert!(NODE_DESCRIPTION.is_ascii());
const _: () = assert!(SEARCH_DESCRIPTION.is_ascii());
const _: () = assert!(BUNDLE_DESCRIPTION.is_ascii());
const _: () = assert!(ROADMAP_DESCRIPTION.is_ascii());
const _: () = assert!(IMPACT_DESCRIPTION.is_ascii());
const _: () = assert!(holds(TREE_DESCRIPTION, DETERMINISM));
const _: () = assert!(holds(NODE_DESCRIPTION, DETERMINISM));
const _: () = assert!(holds(SEARCH_DESCRIPTION, DETERMINISM));
const _: () = assert!(holds(BUNDLE_DESCRIPTION, DETERMINISM));
const _: () = assert!(holds(ROADMAP_DESCRIPTION, DETERMINISM));
const _: () = assert!(holds(IMPACT_DESCRIPTION, DETERMINISM));
// It names its CLI twin and the table `symbol` needs.
const _: () = assert!(holds(IMPACT_DESCRIPTION, "`spec impact "));
const _: () = assert!(holds(IMPACT_DESCRIPTION, "`[code]`"));
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
    ROADMAP_DESCRIPTION,
    "above ",
    OUTPUT_CAP_CHARS,
    " characters"
));
const _: () = assert!(holds_number(
    IMPACT_DESCRIPTION,
    "above ",
    OUTPUT_CAP_CHARS,
    " characters"
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
    #[schemars(description = WITH_DOC)]
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
    /// `spec show --bindings`.
    Bindings,
}

/// `{"type": "string", "enum": ["links", "bindings"]}`, inlined (the
/// derive would write a `oneOf` of `const`s).
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
        object.insert(
            "enum".to_owned(),
            Value::from(vec![Value::from("links"), Value::from("bindings")]),
        );
        schemars::Schema::from(object)
    }
}

/// The input schema's description of `get_node.with`.
const WITH_DOC: &str = "`[\"links\"]`: each node's links, outgoing and incoming; `[\"bindings\"]`: \
each node's code bindings with their sync.";

/// The input schema's description of `search.query`.
const QUERY_DOC: &str = "The words to find (terms of 3 or more characters).";

/// The input schema's description of `search.limit`.
const LIMIT_DOC: &str = "Hits at most, 1 to 200. Absent: 20.";

/// The input schema's description of `get_context_bundle.budget`.
const BUDGET_DOC: &str = "Estimated tokens at most. Absent: `[budgets] bundle_node`, else 2000.";

/// The input schema's description of `get_impact.id`.
const IMPACT_ID_DOC: &str =
    "An ID, an alias, `slug/ID`, `ID#SECTION` or a root-relative `.md` path.";

/// The input schema's description of `get_impact.symbol`.
const IMPACT_SYMBOL_DOC: &str = "A code symbol, looked up as find_symbols' query: a qualified \
path, a suffix of one or a bare name.";

/// The input schema's description of `get_impact.since`.
const IMPACT_SINCE_DOC: &str =
    "A git revision: the code changed since it, against the files on disk.";

/// The input schema's description of `get_impact.depth`.
const IMPACT_DEPTH_DOC: &str =
    "Links from the start at most; 0 = the start only. Absent: unbounded.";

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

/// `get_roadmap` arguments; `release` with `pool: true` is the CLI's
/// refusal (exit 2), told at run time.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct RoadmapArgs {
    /// `--release R`: only this release's items, as `[roadmap] releases`
    /// names it.
    pub release: Option<String>,
    /// `--pool`: `true` lists only the pool and the items in a release not
    /// listed.
    pub pool: Option<bool>,
}

/// `get_impact` arguments: exactly one of `id`, `symbol` and `since`, told
/// at run time (no root `oneOf`: `docs/canon/mcp-read.md` "Tools").
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ImpactArgs {
    #[schemars(description = IMPACT_ID_DOC)]
    pub id: Option<String>,
    #[schemars(description = IMPACT_SYMBOL_DOC)]
    pub symbol: Option<String>,
    #[schemars(description = IMPACT_SINCE_DOC)]
    pub since: Option<String>,
    #[schemars(description = IMPACT_DEPTH_DOC)]
    pub depth: Option<i64>,
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
        let with = args.with.unwrap_or_default();
        let (reference, archive) = (args.id, args.archive.unwrap_or(false));
        Ok(self
            .call(move |env, globals| {
                let request = ShowRequest {
                    reference,
                    links: with.contains(&With::Links),
                    archive,
                    bindings: with.contains(&With::Bindings),
                    git: process_git(env),
                };
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
        let (references, budget) = (args.node_ids, args.budget);
        Ok(self
            .call(move |env, globals| {
                let request = BundleRequest {
                    references,
                    budget,
                    git: process_git(env),
                };
                specengine_cli::bundle(env, globals, &request).map(Outcome::Bundle)
            })
            .await
            .into_tool_result("get_context_bundle"))
    }

    /// `spec roadmap`.
    #[tool(
        description = ROADMAP_DESCRIPTION,
        input_schema = input_schema::<RoadmapArgs>(),
        output_schema = output_schema::<RoadmapDocument>(),
        annotations(read_only_hint = true, destructive_hint = false, open_world_hint = false),
        meta = result_size_meta()
    )]
    async fn get_roadmap(
        &self,
        Parameters(args): Parameters<RoadmapArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let release = args.release;
        let pool = args.pool.unwrap_or(false);
        Ok(self
            .call(move |env, globals| {
                let request = RoadmapRequest {
                    release,
                    pool,
                    git: process_git(env),
                };
                specengine_cli::roadmap_with_view(env, globals, &request, View::Capped)
                    .map(|outcome| Outcome::Roadmap(Box::new(outcome)))
            })
            .await
            .into_tool_result("get_roadmap"))
    }

    /// `spec impact`: exactly one of `id`, `symbol` and `since`, else the
    /// tool's own error result before any discovery, no document.
    #[tool(
        description = IMPACT_DESCRIPTION,
        input_schema = input_schema::<ImpactArgs>(),
        output_schema = output_schema::<ImpactDocument>(),
        annotations(read_only_hint = true, destructive_hint = false, open_world_hint = false),
        meta = result_size_meta()
    )]
    async fn get_impact(
        &self,
        Parameters(args): Parameters<ImpactArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let ImpactArgs {
            id,
            symbol,
            since,
            depth,
        } = args;
        let given = [id.is_some(), symbol.is_some(), since.is_some()];
        if given.into_iter().filter(|&given| given).count() != 1 {
            return Ok(CallToolResult::error(vec![ContentBlock::text(ONE_INPUT)]));
        }
        Ok(self
            .call(move |env, globals| {
                let request = ImpactRequest {
                    reference: id,
                    symbol,
                    since,
                    depth,
                    git: process_git(env),
                };
                specengine_cli::impact(env, globals, &request)
                    .map(|outcome| Outcome::Impact(Box::new(outcome)))
            })
            .await
            .into_tool_result("get_impact"))
    }
}

/// What one CLI library call gave, rendered on the blocking pool.
pub(crate) enum Answer {
    /// The command rendered its answer: exit 0; exit 1 (found nothing, a
    /// refusal, a finding); exit 2 with a report (as `spec verify`'s run
    /// causes and rows it cannot verify).
    Rendered {
        exit: Exit,
        /// Stderr lines, then stdout: `spec … 2>&1`.
        text: String,
        /// The `--json` document as `render_json` gives it, parsed.
        document: Value,
    },
    /// The command could not run (exit 2, nothing rendered).
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
            answer_of(Env::from_process().and_then(|env| command(&env, &globals)))
        })
        .await;
        joined.unwrap_or(Answer::Panicked)
    }
}

/// A command's outcome rendered as [`SpecEngineServer::call`] renders it.
pub(crate) fn answer_of(outcome: Result<Outcome, CliError>) -> Answer {
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
}
