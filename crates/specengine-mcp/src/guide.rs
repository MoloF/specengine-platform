//! `get_guide` (task spec `specengine-guide`, "MCP"; ADR-0085): `spec guide
//! [TOPIC[#ID]]`, SpecEngine's own guide as this build embedded it, on stdio
//! and as a hub's shared tool, and the resource template `guide://{topic}`.
//!
//! It needs no project and takes no turn: the CLI library's
//! [`specengine_cli::guide`] reads only the binary's static pages, so the
//! call runs in place, never on the blocking pool, never through a hub's
//! seam (no login line, no turn, no index trust), like `tools/list`. It is
//! not in [`crate::HUB_READ_TOOLS`]: it reads no index. Its answer is the
//! CLI's, byte for byte: `content` its text (no stderr line: an answer has
//! none), `structuredContent` its `--json` document; an unknown topic or
//! rule (exit 1) and a REF holding a space or a control character (exit 2)
//! an error result with the CLI's line alone. `rule` without `topic` is the
//! tool's own error result.

use rmcp::ErrorData;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, ResourceContents};
use rmcp::{schemars, tool, tool_router};
use serde::Deserialize;
use specengine_cli::{Exit, GuideRequest, Outcome};

use crate::mirror::{guide_output_schema, input_schema};
use crate::read::{DESCRIPTION_LIMIT, DETERMINISM, answer_of, holds, result_size_meta};
use crate::server::SpecEngineServer;

/// `get_guide`'s description: stdio's and a hub's alike.
pub(crate) const GUIDE_DESCRIPTION: &str = "SpecEngine's own guide, built into this server: \
where does X go in a project (a decision, a requirement, a department's rule, a card, a \
code marker), what each feature solves and how to use it, and the rules to keep. The same as \
`spec guide [TOPIC[#ID]]`: content is the command's output, structuredContent its --json \
document.

No topic: the index, each topic with its title, its one-line summary and its rule count. \
topic: that page in Markdown, with its rules (each an ID, a title and the line of its \
heading). topic and rule: that one rule's section. rule without topic: an error result.

A topic or a rule the guide does not hold: an error result with the CLI's line, naming the \
topics or the topic's rules. Cite a rule as guide:<topic>#<ID>. The guide is this build's, \
the same in every project and with none: it reads no project file and no index, writes \
nothing and waits on nothing. Deterministic: one state, one result; no LLM inside.";

const _: () = assert!(GUIDE_DESCRIPTION.is_ascii());
const _: () = assert!(GUIDE_DESCRIPTION.len() <= DESCRIPTION_LIMIT);
const _: () = assert!(holds(GUIDE_DESCRIPTION, DETERMINISM));
const _: () = assert!(holds(GUIDE_DESCRIPTION, "where does X go"));

/// The URI scheme of the guide's resources.
pub(crate) const GUIDE_SCHEME: &str = "guide://";

/// The guide's template (RFC 6570).
pub(crate) const GUIDE_TEMPLATE: &str = "guide://{topic}";

/// The input schema's description of `get_guide.topic`.
const TOPIC_DOC: &str = "A topic of the index, as the index names it. Absent: the index.";

/// The input schema's description of `get_guide.rule`.
const RULE_DOC: &str = "A rule ID of the topic, as the page names it; needs topic.";

/// `get_guide` arguments.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct GuideArgs {
    #[schemars(description = TOPIC_DOC)]
    pub topic: Option<String>,
    #[schemars(description = RULE_DOC)]
    pub rule: Option<String>,
}

/// The error result of `rule` without `topic`.
const RULE_NEEDS_TOPIC: &str = "spec: get_guide's rule needs its topic: give topic and rule";

/// The REF `spec guide` takes for `topic` and `rule`: `TOPIC` or
/// `TOPIC#ID`.
fn reference(topic: Option<String>, rule: Option<String>) -> Result<Option<String>, ()> {
    match (topic, rule) {
        (None, None) => Ok(None),
        (None, Some(_)) => Err(()),
        (Some(topic), None) => Ok(Some(topic)),
        (Some(topic), Some(rule)) => Ok(Some(format!("{topic}#{rule}"))),
    }
}

/// `spec guide [REF]` as a tool result: the CLI's answer, in place.
pub(crate) fn guide_result(topic: Option<String>, rule: Option<String>) -> CallToolResult {
    let Ok(reference) = reference(topic, rule) else {
        return CallToolResult::error(vec![ContentBlock::text(format!("{RULE_NEEDS_TOPIC}\n"))]);
    };
    answer_of(specengine_cli::guide(&GuideRequest { reference }).map(Outcome::Guide))
        .into_tool_result("get_guide")
}

#[tool_router(router = guide_tools, vis = "pub(crate)")]
impl SpecEngineServer {
    /// `spec guide`.
    #[tool(
        description = GUIDE_DESCRIPTION,
        input_schema = input_schema::<GuideArgs>(),
        output_schema = guide_output_schema(),
        annotations(read_only_hint = true, destructive_hint = false, open_world_hint = false),
        meta = result_size_meta()
    )]
    async fn get_guide(
        &self,
        Parameters(args): Parameters<GuideArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(guide_result(args.topic, args.rule))
    }
}

/// `uri` is the guide's (`guide://…`).
pub(crate) fn is_guide_uri(uri: &str) -> bool {
    uri.starts_with(GUIDE_SCHEME)
}

/// Reads `guide://<topic>` (the rest percent-decoded once, `<topic>#<ID>`
/// allowed): the text of `spec guide <topic>`, `text/markdown`, with or
/// without a project, in place. A topic or rule the guide does not hold:
/// not found (-32002, `data: {"uri"}`) with the CLI's line; a bad percent
/// sequence, an empty topic or a REF the CLI refuses (a space, a control
/// character): -32602.
pub(crate) fn read_guide(uri: &str) -> Result<ResourceContents, ErrorData> {
    let raw = uri.strip_prefix(GUIDE_SCHEME).unwrap_or_default();
    let invalid =
        |problem: String| ErrorData::invalid_params(format!("resource `{uri}`: {problem}"), None);
    let topic = crate::resources::percent_decode(raw).map_err(invalid)?;
    if topic.is_empty() {
        return Err(invalid(format!(
            "names no topic: the form is {GUIDE_TEMPLATE}"
        )));
    }
    let outcome = specengine_cli::guide(&GuideRequest {
        reference: Some(topic),
    });
    match outcome {
        Ok(outcome) => Ok(ResourceContents::text(
            specengine_cli::render_text(&Outcome::Guide(outcome)),
            uri,
        )
        .with_mime_type("text/markdown")),
        Err(error) if error.exit == Exit::NotFound => Err(ErrorData::resource_not_found(
            error.message,
            Some(serde_json::json!({ "uri": uri })),
        )),
        Err(error) => Err(ErrorData::invalid_params(error.message, None)),
    }
}
