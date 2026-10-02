//! The resources (07 §1.3; task spec `mcp-read`, "Data"), for `@`-mentions:
//!
//! - `spec://<slug>/tree`, `text/plain`: the text of `get_tree {}`;
//! - `spec://<slug>/node/<id>`, `text/markdown`: the text of
//!   `get_node {id}`, `<id>` any REF, percent-encoded (the template
//!   `spec://{project}/node/{id}`).
//!
//! `resources/list` holds the tree, then every indexed live document
//! (neither `class: generated` nor Tier 3) by path (CLI `documents`),
//! [`PAGE_SIZE`] per page; `nextCursor` is the page's last path, the next
//! page starts after it. Without a project (or a usable `HOME`) the list is
//! empty; any other failure is -32603. Like the tools, each request runs on
//! the blocking pool and keeps nothing.
//!
//! Error codes: not found -32002 with `data: {"uri"}` (rmcp sends it as
//! -32602 to 2026-07-28 peers, SEP-2164; `data` still tells it apart); a
//! bad percent sequence or a non-UTF-8 `{id}` -32602 without `data`; a
//! read that cannot run (CLI exit 2) -32603 with its line(s).

use rmcp::ErrorData;
use rmcp::model::{Resource, ResourceContents, ResourceTemplate};
use specengine_cli::{
    CliError, DocumentEntry, Env, Exit, Globals, Outcome, ShowRequest, TreeRequest, data_dir,
    discover, documents, locate,
};

use crate::read::outcome_text;

/// The URI scheme and its separator.
const SCHEME: &str = "spec://";

/// The node template (RFC 6570).
pub(crate) const NODE_TEMPLATE: &str = "spec://{project}/node/{id}";

/// Entries per `resources/list` page.
pub(crate) const PAGE_SIZE: usize = 200;

const TEXT_PLAIN: &str = "text/plain";
const TEXT_MARKDOWN: &str = "text/markdown";

/// The resource templates: the node template alone.
pub(crate) fn templates() -> Vec<ResourceTemplate> {
    vec![ResourceTemplate::new(NODE_TEMPLATE, "node").with_mime_type(TEXT_MARKDOWN)]
}

/// One `resources/list` page: its resources and the cursor of the next.
#[derive(Debug, Default)]
pub(crate) struct Page {
    pub resources: Vec<Resource>,
    pub next_cursor: Option<String>,
}

/// The page after `cursor` (the first page without one) of the project
/// found from `env` and `globals`. Empty when no project is found (CLI
/// [`locate`] fails: an unusable current directory or `--root`, no
/// `specengine.toml`) or `HOME` is unusable, checked in the order the CLI
/// meets them; any other failure once a project is found (its config
/// unreadable, not UTF-8 or invalid, no slug; the database: busy, corrupt,
/// not writable) is -32603 with the CLI's line(s).
pub(crate) fn list(env: &Env, globals: &Globals, cursor: Option<&str>) -> Result<Page, ErrorData> {
    if locate(env, globals).is_err() {
        return Ok(Page::default());
    }
    let project = discover(env, globals).map_err(cannot_run)?;
    let slug = project.slug().map_err(cannot_run)?.to_owned();
    if data_dir(env).is_err() {
        return Ok(Page::default());
    }
    let entries = documents(env, globals).map_err(cannot_run)?;
    Ok(page(&slug, &entries, cursor))
}

/// The tree (first page only), then the documents after `cursor`, at most
/// [`PAGE_SIZE`] in all; the next cursor when documents remain.
fn page(slug: &str, entries: &[DocumentEntry], cursor: Option<&str>) -> Page {
    let mut resources = Vec::new();
    if cursor.is_none() {
        resources
            .push(Resource::new(format!("{SCHEME}{slug}/tree"), "tree").with_mime_type(TEXT_PLAIN));
    }
    let after: Vec<&DocumentEntry> = entries
        .iter()
        .filter(|entry| cursor.is_none_or(|cursor| entry.path.as_str() > cursor))
        .collect();
    let shown = &after[..after.len().min(PAGE_SIZE - resources.len())];
    resources.extend(shown.iter().map(|entry| document_resource(slug, entry)));
    let next_cursor = if after.len() > shown.len() {
        shown.last().map(|entry| entry.path.clone())
    } else {
        None
    };
    Page {
        resources,
        next_cursor,
    }
}

/// `{uri, name, title?, mimeType}` of one document: `name` its ID, else its
/// path.
fn document_resource(slug: &str, entry: &DocumentEntry) -> Resource {
    let name = entry.id.clone().unwrap_or_else(|| entry.path.clone());
    let resource = Resource::new(
        format!("{SCHEME}{slug}/node/{}", percent_encode(&entry.path)),
        name,
    )
    .with_mime_type(TEXT_MARKDOWN);
    match &entry.title {
        Some(title) => resource.with_title(title.clone()),
        None => resource,
    }
}

/// What a URI names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Target {
    Tree,
    /// The REF, percent-decoded.
    Node(String),
}

/// A parsed resource URI: its project segment as written and its target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Parsed {
    pub project: String,
    pub target: Target,
}

/// Parses `uri`: `spec://<project>/tree` or `spec://<project>/node/<id>`,
/// `<id>` all after `node/`, percent-decoded once as UTF-8 (a raw `#` kept).
/// Neither form → not found; a bad percent sequence or non-UTF-8 → -32602
/// without `data`.
pub(crate) fn parse(uri: &str) -> Result<Parsed, ErrorData> {
    let neither = || {
        not_found(
            uri,
            format!(
                "no SpecEngine resource `{uri}`: the forms are {SCHEME}<slug>/tree and \
                 {NODE_TEMPLATE}"
            ),
        )
    };
    let rest = uri.strip_prefix(SCHEME).ok_or_else(neither)?;
    let (project, path) = rest.split_once('/').ok_or_else(neither)?;
    if project.is_empty() {
        return Err(neither());
    }
    let target = if path == "tree" {
        Target::Tree
    } else if let Some(raw) = path.strip_prefix("node/").filter(|raw| !raw.is_empty()) {
        Target::Node(percent_decode(raw).map_err(|problem| {
            ErrorData::invalid_params(format!("resource `{uri}`: {problem}"), None)
        })?)
    } else {
        return Err(neither());
    };
    Ok(Parsed {
        project: project.to_owned(),
        target,
    })
}

/// Reads a parsed URI: the project's slug must be `project`; then one CLI
/// call, its text.
pub(crate) fn read(
    env: &Env,
    globals: &Globals,
    uri: &str,
    parsed: &Parsed,
) -> Result<ResourceContents, ErrorData> {
    let project = discover(env, globals).map_err(cannot_run)?;
    let slug = project.slug().map_err(cannot_run)?;
    if slug != parsed.project {
        return Err(not_found(
            uri,
            format!(
                "resource `{uri}`: this server reads project `{slug}`, not `{}`",
                parsed.project
            ),
        ));
    }
    let (outcome, mime_type) = match &parsed.target {
        Target::Tree => (
            specengine_cli::tree(env, globals, &TreeRequest::default()).map(Outcome::Tree),
            TEXT_PLAIN,
        ),
        Target::Node(reference) => (
            specengine_cli::show(
                env,
                globals,
                &ShowRequest {
                    reference: reference.clone(),
                    links: false,
                    archive: false,
                },
            )
            .map(Outcome::Show),
            TEXT_MARKDOWN,
        ),
    };
    let outcome = outcome.map_err(cannot_run)?;
    if outcome.exit() != Exit::Answered {
        // Exit 1: the reason line, `spec: …`, is the last stderr line.
        let reason = outcome
            .stderr_lines()
            .pop()
            .unwrap_or_else(|| format!("spec: `{uri}` names nothing"));
        return Err(not_found(uri, reason));
    }
    Ok(ResourceContents::text(outcome_text(&outcome), uri).with_mime_type(mime_type))
}

/// Resource not found: -32002, `data: {"uri": …}` (rmcp sends the code as
/// -32602 to 2026-07-28 peers, SEP-2164; `data` still tells it from a bad
/// percent sequence).
fn not_found(uri: &str, message: String) -> ErrorData {
    ErrorData::resource_not_found(message, Some(serde_json::json!({ "uri": uri })))
}

/// Exit 2: -32603 with the error line(s).
fn cannot_run(error: CliError) -> ErrorData {
    ErrorData::internal_error(error.message, None)
}

/// Percent-encodes every byte but RFC 3986's unreserved characters
/// (`A-Z a-z 0-9 - . _ ~`), upper-case hex: `docs/a.md` → `docs%2Fa.md`.
pub(crate) fn percent_encode(text: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut encoded = String::with_capacity(text.len());
    for &byte in text.as_bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push('%');
            encoded.push(char::from(HEX[usize::from(byte >> 4)]));
            encoded.push(char::from(HEX[usize::from(byte & 0x0F)]));
        }
    }
    encoded
}

/// Decodes each `%XX` once (either case); every other byte as is. A `%` not
/// followed by two hex digits, or bytes that are not UTF-8, are refused.
pub(crate) fn percent_decode(raw: &str) -> Result<String, String> {
    let bytes = raw.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte != b'%' {
            decoded.push(byte);
            index += 1;
            continue;
        }
        let high = bytes.get(index + 1).copied().and_then(hex_value);
        let low = bytes.get(index + 2).copied().and_then(hex_value);
        match (high, low) {
            (Some(high), Some(low)) => {
                decoded.push(high << 4 | low);
                index += 3;
            }
            _ => {
                let sequence: String = raw[index..].chars().take(3).collect();
                return Err(format!(
                    "`{sequence}` at byte {index} is no percent-encoded byte (`%` and two hex digits)"
                ));
            }
        }
    }
    String::from_utf8(decoded).map_err(|error| {
        format!(
            "the decoded REF is not UTF-8 (byte {})",
            error.utf8_error().valid_up_to()
        )
    })
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}
