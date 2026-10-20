//! The code tools (`docs/features/code-tools.md`, 6b-1): `find_symbols`,
//! `refs` and `unmapped`, the twins of `spec symbols`, `spec refs` and
//! `spec unmapped` (`docs/canon/symbol-reads.md`,
//! `docs/canon/spec-cli-refs.md`), and `check_binding`
//! (`docs/features/check-binding.md`, 6b-2b), the twin of `spec verify
//! --node REF [--symbol QPATH]` (`docs/canon/spec-verify-scope.md`
//! "`--node`"), each one call into the CLI library through `read.rs`'
//! [`SpecEngineServer::call`] and [`Answer`], answering what the CLI
//! answers: `content` its text, `structuredContent` its `--json` document
//! (exit 1 kept; exit 2 kept when the CLI prints a report, `check_binding`'s
//! run causes and rows it cannot verify, else none).
//!
//! - **Forms**: `find_symbols` takes exactly one of `query`, `node`, `file`
//!   and `at` (a `null` is absent), told at run time as `get_task`'s (no
//!   root `oneOf`), and `limit` with `query` only: else the tool's own
//!   error result, before any discovery, no document. A limit below zero
//!   is the CLI's own `--limit` usage error.
//! - **Passed as given**: `query` is one positional, `at` goes raw to the
//!   CLI's one `PATH:LINE` reader: nothing here splits a name or a line.
//! - **Reads**: each call refreshes the spec index, then the code index,
//!   inside the CLI read; nothing is kept between calls, nothing under the
//!   project root is written.
//! - **`check_binding`**: `node` and `symbol` passed as written (MCP never
//!   resolves a REF or matches a path), the working tree judged, no task
//!   and no base: the server's root, this session's worktree. Nothing reads
//!   its result (ADR-0012, ADR-0042): no `spec.lock`, queue row or event is
//!   written.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock};
use rmcp::{ErrorData, schemars, tool, tool_router};
use serde::Deserialize;
use specengine_cli::{
    OUTPUT_CAP_CHARS, Outcome, RefsRequest, SYMBOLS_LIMIT_DEFAULT, SYMBOLS_LIMIT_MAX,
    SYMBOLS_LIMIT_MIN, SymbolsForm, SymbolsRequest, UnmappedRequest, VerifiedTree, VerifyRequest,
    process_git, symbols_limit_error,
};

use crate::mirror::{
    RefsDocument, SymbolsDocument, UnmappedDocument, VerifyDocument, input_schema, output_schema,
};
use crate::read::{Answer, DESCRIPTION_LIMIT, DETERMINISM, holds, holds_number, result_size_meta};
use crate::server::SpecEngineServer;

/// Every code tool's description ends with this sentence (07 §1.1).
macro_rules! code_tail {
    () => {
        "Reads the files as they are now: refreshes SpecEngine's indexes of the spec and the code \
in its data directory, writes nothing under the project root. Deterministic: one state, one \
result; no LLM inside."
    };
}

const FIND_SYMBOLS_DESCRIPTION: &str = concat!(
    "The code items a name, a node, a file or a line points at, each with the spec markers \
bound to it (relation, position, ID and revision states, levels, the nodes it is listed \
under). The same as `spec symbols QUERY [--limit N] | --node REF | --file PATH | --at \
PATH:LINE`: content is the command's output (its note and warning lines first), \
structuredContent its --json document. Needs a `[code]` table in the project's config: \
without one, an error result naming it.

query: a qualified path, a suffix of one or a bare name, as one value: exact paths first, \
then suffixes, then names (tier); an in-file twin's ` #N` is kept.
node: a REF, any form get_node takes: the items, modules and data paths its markers bind, \
by path and line.
file: a root-relative code file the walk reads: its items in source order.
at: PATH:LINE, a root-relative code file and a line from 1, passed as written: the items \
holding that line, innermost first.
limit: with query only: items at most, 1 to 200; absent: 20.
Give exactly one of query, node, file and at: anything else is an error result naming them.

A target: its qpath, kind, path and lines, signature, layer, mentions and ambiguity (shown, \
not resolved). Zero items is a result, not an error. An answer above 40000 characters is \
cut, with a tail counting the items not shown. A node REF naming nothing: an error result \
with the reason, and the document. A call that cannot run (no `[code]`, a path the walk does \
not read, a bad line, a limit out of bounds, an incomplete walk): an error result with its \
one line. ",
    code_tail!()
);

const REFS_DESCRIPTION: &str = concat!(
    "What in the code names a node: for each node a REF names, its spec markers in code \
(relation, file, line and column, the token with its ID and revision states, levels, what \
it binds: an item, a module, a data path, or why nothing) and the comment mentions of its \
ID. The same as `spec refs REF`: content is the command's output (its note and warning \
lines first), structuredContent its --json document. Needs a `[code]` table in the \
project's config: without one, an error result naming it.

id: a REF: an ID, an alias, `slug/ID`, `ID#SECTION` or a root-relative `.md` path. An ID \
several documents hold gives all of them, with a warning.

An answer above 40000 characters is cut, with a tail counting the markers and mentions not \
shown. A REF naming nothing: an error result with the reason, and the document. A \
look-alike or mixed-script ID (refused with its Latin fix) or another call that cannot run \
(no `[code]`, an incomplete walk): an error result with its one line. ",
    code_tail!()
);

const UNMAPPED_DESCRIPTION: &str = concat!(
    "How much of the code the spec maps: per directory and code file, its units (items; a \
data file is one), those a marker maps (bound to the item, to an item or module enclosing \
it, or to its file, its ID naming a node), their percent and those a mention reaches; a \
code file also lists its unmapped items in source order. The same as `spec unmapped \
[PATH]`: content is the command's output (its note and warning lines first), \
structuredContent its --json document (percent a number, null without a unit). Needs a \
`[code]` table in the project's config: without one, an error result naming it.

path: a `[code]` root, or a directory or code file the walk reads under one, \
root-relative. Absent: a row per root, then the total.

An answer above 40000 characters is cut, with a tail counting the rows and items not \
shown. A call that cannot run (no `[code]`, a path the walk does not read, an incomplete \
walk): an error result with its one line. ",
    code_tail!()
);

const CHECK_BINDING_DESCRIPTION: &str = concat!(
    "Whether a node's code bindings still hold, judged in this session's worktree: per \
binding of the nodes a REF names (a spec marker, a spec.lock entry), a row with its file, \
symbol, relation, state, class, markers, lock entry, levels and anchor, then the counts, the \
delta and the verdict under `[verify] mode`. The same as `spec verify --node REF [--symbol \
QPATH]`: content is the command's output (note and warning lines first), structuredContent \
its --json document. Needs a `[code]` table in the project's config: without one, an error \
result with the report.

node: a REF, any form get_node takes: its own bindings, not its sections' or its parent's. An \
ID several documents hold judges all of them, with a warning.
symbol: only the rows whose symbol is this qualified path, byte for byte (a moved row's old \
one too). Absent: every row.

Mend what your change broke: a binding that no longer reaches its code, spec and code both \
changed, a bad marker, a marker naming a revision its node has not reached, or a binding it \
cannot verify; else raise it with report_discrepancy. The owner settles code ahead of its \
spec (as after implementing), a spec ahead of its code and a binding not yet locked: name \
them in your report, never hide them. Zero rows is a result, not an error.

No base is read: `enforce-introduced` is judged as `enforce`, so under either a binding the \
change broke, or one broken before it, is an error result with the report: a finding, not a \
failed call. So is a row that cannot be verified or a run cause (no `[code]`, an incomplete \
walk, an unreadable lock). An answer above 40000 characters is cut, with a tail counting the \
rest (it names --symbol QPATH: symbol). A REF naming nothing, a look-alike ID (refused with \
its Latin fix) or another call that cannot run: an error result with its one line, no \
document. ",
    code_tail!()
);

/// The input schema's description of `find_symbols.query`.
const QUERY_DOC: &str = "A qualified path, a suffix of one or a bare name, as one value.";

/// The input schema's description of `find_symbols.node`.
const NODE_DOC: &str = "A REF, any form get_node takes: the items its markers bind.";

/// The input schema's description of `find_symbols.file`.
const FILE_DOC: &str = "A root-relative code file the walk reads: its items in source order.";

/// The input schema's description of `find_symbols.at`.
const AT_DOC: &str = "PATH:LINE: a root-relative code file and a line from 1, passed as written.";

/// The input schema's description of `find_symbols.limit`.
const LIMIT_DOC: &str = "With query only: items at most, 1 to 200. Absent: 20.";

/// The input schema's description of `refs.id`.
const REF_DOC: &str = "An ID, an alias, `slug/ID`, `ID#SECTION` or a root-relative `.md` path.";

/// The input schema's description of `unmapped.path`.
const PATH_DOC: &str =
    "A `[code]` root, or a directory or code file under one. Absent: a row per root.";

/// The input schema's description of `check_binding.symbol`.
const SYMBOL_DOC: &str =
    "Only the rows whose symbol is this qualified path, byte for byte. Absent: every row.";

/// `find_symbols` given not exactly one form
/// (`docs/canon/mcp-code.md` "Tools", Forms).
const ONE_FORM: &str = "spec: find_symbols takes exactly one of `query`, `node`, `file` and `at`\n";

/// `find_symbols` given `limit` beside a form other than `query`.
const LIMIT_WITH_QUERY: &str = "spec: find_symbols takes `limit` with `query` only\n";

// ASCII only, so bytes equal characters.
const _: () = assert!(FIND_SYMBOLS_DESCRIPTION.len() <= DESCRIPTION_LIMIT);
const _: () = assert!(REFS_DESCRIPTION.len() <= DESCRIPTION_LIMIT);
const _: () = assert!(UNMAPPED_DESCRIPTION.len() <= DESCRIPTION_LIMIT);
const _: () = assert!(CHECK_BINDING_DESCRIPTION.len() <= DESCRIPTION_LIMIT);
const _: () = assert!(FIND_SYMBOLS_DESCRIPTION.is_ascii());
const _: () = assert!(REFS_DESCRIPTION.is_ascii());
const _: () = assert!(UNMAPPED_DESCRIPTION.is_ascii());
const _: () = assert!(CHECK_BINDING_DESCRIPTION.is_ascii());
const _: () = assert!(holds(FIND_SYMBOLS_DESCRIPTION, DETERMINISM));
const _: () = assert!(holds(REFS_DESCRIPTION, DETERMINISM));
const _: () = assert!(holds(UNMAPPED_DESCRIPTION, DETERMINISM));
const _: () = assert!(holds(CHECK_BINDING_DESCRIPTION, DETERMINISM));
// Each names its CLI twin and the table it needs.
const _: () = assert!(holds(FIND_SYMBOLS_DESCRIPTION, "`spec symbols "));
const _: () = assert!(holds(REFS_DESCRIPTION, "`spec refs "));
const _: () = assert!(holds(UNMAPPED_DESCRIPTION, "`spec unmapped "));
const _: () = assert!(holds(CHECK_BINDING_DESCRIPTION, "`spec verify --node "));
const _: () = assert!(holds(FIND_SYMBOLS_DESCRIPTION, "`[code]`"));
const _: () = assert!(holds(REFS_DESCRIPTION, "`[code]`"));
const _: () = assert!(holds(UNMAPPED_DESCRIPTION, "`[code]`"));
const _: () = assert!(holds(CHECK_BINDING_DESCRIPTION, "`[code]`"));
// `check_binding` judges this session's worktree with no base
// (`docs/features/check-binding.md` "Data").
const _: () = assert!(holds(CHECK_BINDING_DESCRIPTION, "this session's worktree"));
const _: () = assert!(holds(
    CHECK_BINDING_DESCRIPTION,
    "`enforce-introduced` is judged as `enforce`"
));
// The numbers the texts state are the CLI's.
const _: () = assert!(holds_number(
    FIND_SYMBOLS_DESCRIPTION,
    "above ",
    OUTPUT_CAP_CHARS,
    " characters"
));
const _: () = assert!(holds_number(
    REFS_DESCRIPTION,
    "above ",
    OUTPUT_CAP_CHARS,
    " characters"
));
const _: () = assert!(holds_number(
    UNMAPPED_DESCRIPTION,
    "above ",
    OUTPUT_CAP_CHARS,
    " characters"
));
const _: () = assert!(holds_number(
    CHECK_BINDING_DESCRIPTION,
    "above ",
    OUTPUT_CAP_CHARS,
    " characters"
));
const _: () = assert!(holds_number(
    FIND_SYMBOLS_DESCRIPTION,
    "items at most, ",
    SYMBOLS_LIMIT_MIN,
    " to "
));
const _: () = assert!(holds_number(
    FIND_SYMBOLS_DESCRIPTION,
    " to ",
    SYMBOLS_LIMIT_MAX,
    "; absent: "
));
const _: () = assert!(holds_number(
    FIND_SYMBOLS_DESCRIPTION,
    "; absent: ",
    SYMBOLS_LIMIT_DEFAULT,
    ".\n"
));
const _: () = assert!(holds_number(
    LIMIT_DOC,
    "items at most, ",
    SYMBOLS_LIMIT_MIN,
    " to "
));
const _: () = assert!(holds_number(LIMIT_DOC, " to ", SYMBOLS_LIMIT_MAX, ". "));
const _: () = assert!(holds_number(
    LIMIT_DOC,
    "Absent: ",
    SYMBOLS_LIMIT_DEFAULT,
    "."
));

/// `find_symbols` arguments: exactly one of the four forms, told at run
/// time (no root `oneOf`: `docs/canon/mcp-read.md` "Tools").
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct SymbolsArgs {
    #[schemars(description = QUERY_DOC)]
    pub query: Option<String>,
    #[schemars(description = NODE_DOC)]
    pub node: Option<String>,
    #[schemars(description = FILE_DOC)]
    pub file: Option<String>,
    #[schemars(description = AT_DOC)]
    pub at: Option<String>,
    #[schemars(description = LIMIT_DOC)]
    pub limit: Option<i64>,
}

/// `refs` arguments.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct RefsArgs {
    #[schemars(description = REF_DOC)]
    pub id: String,
}

/// `unmapped` arguments.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct UnmappedArgs {
    #[schemars(description = PATH_DOC)]
    pub path: Option<String>,
}

/// `check_binding` arguments: `--node REF [--symbol QPATH]`, each as
/// written.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct CheckBindingArgs {
    #[schemars(description = REF_DOC)]
    pub node: String,
    #[schemars(description = SYMBOL_DOC)]
    pub symbol: Option<String>,
}

/// The tool's own error result: `text`, no document.
fn form_error(text: &str) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(text)])
}

/// The one form of `args` as the CLI takes it, or the result refusing
/// them: the form count first, then `limit`, then a limit below zero (the
/// CLI's own `--limit` error). `query` is one positional, `at` passed raw.
fn symbols_form(args: SymbolsArgs) -> Result<SymbolsForm, CallToolResult> {
    let given = [
        args.query.is_some(),
        args.node.is_some(),
        args.file.is_some(),
        args.at.is_some(),
    ];
    if given.into_iter().filter(|&given| given).count() != 1 {
        return Err(form_error(ONE_FORM));
    }
    if args.limit.is_some() && args.query.is_none() {
        return Err(form_error(LIMIT_WITH_QUERY));
    }
    match (args.query, args.node, args.file, args.at) {
        (Some(query), ..) => {
            let limit = match args.limit {
                None => None,
                Some(limit) => match usize::try_from(limit) {
                    Ok(limit) => Some(limit),
                    Err(_) => {
                        return Err(Answer::CannotRun(symbols_limit_error(limit))
                            .into_tool_result("find_symbols"));
                    }
                },
            };
            // One shell word: `spec symbols "<query>"`.
            Ok(SymbolsForm::Query {
                terms: vec![query],
                limit,
            })
        }
        (None, Some(node), ..) => Ok(SymbolsForm::Node(node)),
        (None, None, Some(file), _) => Ok(SymbolsForm::File(file)),
        (None, None, None, Some(at)) => Ok(SymbolsForm::At(at)),
        (None, None, None, None) => Err(form_error(ONE_FORM)),
    }
}

#[tool_router(router = code_tools, vis = "pub(crate)")]
impl SpecEngineServer {
    /// `spec symbols`.
    #[tool(
        description = FIND_SYMBOLS_DESCRIPTION,
        input_schema = input_schema::<SymbolsArgs>(),
        output_schema = output_schema::<SymbolsDocument>(),
        annotations(read_only_hint = true, destructive_hint = false, open_world_hint = false),
        meta = result_size_meta()
    )]
    async fn find_symbols(
        &self,
        Parameters(args): Parameters<SymbolsArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let form = match symbols_form(args) {
            Ok(form) => form,
            Err(refused) => return Ok(refused),
        };
        let request = SymbolsRequest { form };
        Ok(self
            .call(move |env, globals| {
                specengine_cli::symbols(env, globals, &request)
                    .map(|outcome| Outcome::Symbols(Box::new(outcome)))
            })
            .await
            .into_tool_result("find_symbols"))
    }

    /// `spec refs`.
    #[tool(
        description = REFS_DESCRIPTION,
        input_schema = input_schema::<RefsArgs>(),
        output_schema = output_schema::<RefsDocument>(),
        annotations(read_only_hint = true, destructive_hint = false, open_world_hint = false),
        meta = result_size_meta()
    )]
    async fn refs(
        &self,
        Parameters(args): Parameters<RefsArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let request = RefsRequest { reference: args.id };
        Ok(self
            .call(move |env, globals| {
                specengine_cli::refs(env, globals, &request)
                    .map(|outcome| Outcome::Refs(Box::new(outcome)))
            })
            .await
            .into_tool_result("refs"))
    }

    /// `spec unmapped`.
    #[tool(
        description = UNMAPPED_DESCRIPTION,
        input_schema = input_schema::<UnmappedArgs>(),
        output_schema = output_schema::<UnmappedDocument>(),
        annotations(read_only_hint = true, destructive_hint = false, open_world_hint = false),
        meta = result_size_meta()
    )]
    async fn unmapped(
        &self,
        Parameters(args): Parameters<UnmappedArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let request = UnmappedRequest { path: args.path };
        Ok(self
            .call(move |env, globals| {
                specengine_cli::unmapped(env, globals, &request)
                    .map(|outcome| Outcome::Unmapped(Box::new(outcome)))
            })
            .await
            .into_tool_result("unmapped"))
    }

    /// `spec verify --node`: the working tree, no task. A rendered exit 2
    /// (a run cause, a row it cannot verify) keeps its document.
    #[tool(
        description = CHECK_BINDING_DESCRIPTION,
        input_schema = input_schema::<CheckBindingArgs>(),
        output_schema = output_schema::<VerifyDocument>(),
        annotations(read_only_hint = true, destructive_hint = false, open_world_hint = false),
        meta = result_size_meta()
    )]
    async fn check_binding(
        &self,
        Parameters(args): Parameters<CheckBindingArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let CheckBindingArgs { node, symbol } = args;
        Ok(self
            .call(move |env, globals| {
                let request = VerifyRequest {
                    tree: VerifiedTree::WorkingTree,
                    task: None,
                    node: Some(node),
                    symbol,
                    git: process_git(env),
                };
                specengine_cli::verify(env, globals, &request)
                    .map(|outcome| Outcome::Verify(Box::new(outcome)))
            })
            .await
            .into_tool_result("check_binding"))
    }
}
