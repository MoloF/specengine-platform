//! `spec show REF`: the text of one node, found the way the check resolves
//! references.
//!
//! `REF` ending in `.md` is a root-relative path (a non-clean one → exit 2),
//! shown whole when indexed (else exit 1). Anything else is one reference of
//! the grammar (`ID`, an `aliases:` entry, a legacy `aliases_from` ID,
//! `slug/ID`, `ID#SECTION`, `@rev` ignored with a note): none → exit 1
//! listing the prefixes; look-alike letters or mixed scripts → exit 2
//! naming the Latin form; `project:` → exit 2. It resolves through core's
//! [`Resolver::resolve_detached`] over the index-fed input (a bare
//! feature-scoped ID resolves wherever defined); dangling → exit 1 with the
//! resolver's reason.
//!
//! Per holder file, the nodes whose ID is the reference's, else its
//! `aliases_from` target's, else (an `aliases:` entry) the document; with
//! `#SECTION`, that section. Several → all, by (path, position), and one
//! warning citing each. Spans come from a parse of the very bytes printed,
//! read once after the update, never from the index; a file that is not
//! UTF-8 is printed with U+FFFD and marked.

use std::panic::{self, AssertUnwindSafe};

use specengine_core::check::{Resolution, Resolver, is_tier3_file};
use specengine_core::{DOCUMENT_EXTENSION, is_clean_relative, tokens_est};
use specengine_model::script::normalize_char;
use specengine_model::{IdScheme, IdScript, ParsedFile, Reference, grammar};
use specengine_store::{Source as _, SpecIndex as _, WorkingTree};

use crate::location::open_index;
use crate::project::discover;
use crate::refresh::refresh;
use crate::{CliError, Env, Globals, Message, one_line, store_error};

/// `spec show` options.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ShowRequest {
    /// `REF` as given.
    pub reference: String,
}

/// What `spec show` found: nodes, or the reason there are none (exit 1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShowOutcome {
    /// `REF` as given.
    pub reference: String,
    /// Why nothing was found; `None` when `nodes` is not empty.
    pub reason: Option<String>,
    pub messages: Vec<Message>,
    /// By (path, position), each with its whole text; the output cap is
    /// applied when rendering.
    pub nodes: Vec<ShownNode>,
}

/// One node as read from its file just now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShownNode {
    pub id: Option<String>,
    pub kind: Option<String>,
    pub title: Option<String>,
    /// Root-relative.
    pub path: String,
    /// The 1-based lines of the first and the last byte of its span.
    pub line: usize,
    pub end_line: usize,
    /// The document's `status:`; `None` for a section.
    pub status: Option<String>,
    pub rev: Option<u32>,
    pub tokens_est: u32,
    /// The file is Tier 3.
    pub archived: bool,
    /// The file is UTF-8 (else `text` holds U+FFFD).
    pub utf8: bool,
    /// The ID sections nested in it, in source order.
    pub sections: Vec<NestedSection>,
    /// Its bytes: a section's span, a document's whole file.
    pub text: String,
}

/// An ID section inside a shown node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NestedSection {
    pub id: String,
    /// Where its heading line ends, in bytes from the start of the node's
    /// text.
    pub heading_end: usize,
}

/// What `REF` names.
enum Target {
    /// A root-relative `.md` path.
    Path(String),
    /// A reference of the grammar.
    Reference(Reference),
}

/// `spec show`: updates the index, resolves `REF`, reads its holders.
pub fn show(env: &Env, globals: &Globals, request: &ShowRequest) -> Result<ShowOutcome, CliError> {
    let project = discover(env, globals)?;
    project.slug()?;
    let written = request.reference.trim();
    let scheme = &project.config.scheme;
    let mut messages = Vec::new();
    let target = match classify(written, scheme, &mut messages)? {
        Ok(target) => target,
        Err(reason) => return Ok(not_found(request, reason, messages)),
    };

    let mut open = open_index(env, &project)?;
    let (_, warnings) = refresh(&mut open.index, &project, false)?;
    messages.extend(warnings);
    let input = open.index.indexed_input().map_err(store_error)?;
    let resolver = Resolver::new(&input, scheme, &project.config.paths);
    let holders: Vec<String> = match &target {
        Target::Path(path) => {
            if input.files.iter().any(|file| file.path == *path) {
                vec![path.clone()]
            } else {
                let reason = format!(
                    "`{path}` is no indexed document: not under the `[paths]` roots, \
                     excluded, or missing"
                );
                return Ok(not_found(request, reason, messages));
            }
        }
        Target::Reference(reference) => match resolver.resolve_detached(reference, written) {
            Resolution::Resolved(files) => files
                .iter()
                .map(|&file| resolver.paths()[file].to_owned())
                .collect(),
            Resolution::Dangling(reason) => {
                return Ok(not_found(
                    request,
                    format!("`{written}` {reason}"),
                    messages,
                ));
            }
            Resolution::Skipped => return Err(project_qualified(written)),
        },
    };

    let tree = WorkingTree::new(&project.root, &project.config.paths).map_err(store_error)?;
    let mut nodes = Vec::new();
    let mut citations = Vec::new();
    // Holders left out because they could not be read or parsed.
    let mut unread = 0;
    for path in &holders {
        let bytes = match tree.read(path) {
            Ok(bytes) => bytes,
            Err(error) => {
                if let Target::Path(_) = target {
                    let reason = format!("`{path}` cannot be read: {error}");
                    return Ok(not_found(request, reason, messages));
                }
                messages.push(Message::Warning(format!(
                    "cannot read `{path}`: {error}; left out"
                )));
                unread += 1;
                continue;
            }
        };
        let parsed = panic::catch_unwind(AssertUnwindSafe(|| {
            specengine_core::parse(path, &bytes, scheme)
        }));
        let Ok(parsed) = parsed else {
            messages.push(Message::Warning(format!(
                "the spec parser failed on `{path}`; left out"
            )));
            unread += 1;
            continue;
        };
        let archived = is_tier3_file(&parsed);
        let utf8 = std::str::from_utf8(&bytes).is_ok();
        if parsed.nodes.is_empty() {
            // Not UTF-8: no node, only the whole file by its path.
            if let Target::Path(_) = target {
                nodes.push(whole_file(path, &bytes, archived, utf8));
                citations.push(path.clone());
            }
            continue;
        }
        for ord in pick(&parsed, &target, written) {
            let node = shown(path, &bytes, &parsed, ord, archived, utf8);
            citations.push(match (resolver.feature_slug(path), &node.id) {
                (Some(slug), Some(id)) => format!("{slug}/{id}"),
                _ => path.clone(),
            });
            nodes.push(node);
        }
    }
    if nodes.is_empty() {
        let reason = if unread == holders.len() {
            format!("`{written}`: none of its files could be read")
        } else {
            format!("`{written}`: its files changed while being read; run it again")
        };
        return Ok(not_found(request, reason, messages));
    }
    if nodes.len() > 1 {
        messages.push(Message::Warning(format!(
            "`{written}` has {} holders, all shown: {}",
            nodes.len(),
            citations.join(", ")
        )));
    }
    Ok(ShowOutcome {
        reference: request.reference.clone(),
        reason: None,
        messages,
        nodes,
    })
}

/// Exit 1. The reason is one line (it may quote a path or `REF` as
/// written), the same in JSON and on stderr.
fn not_found(request: &ShowRequest, reason: String, messages: Vec<Message>) -> ShowOutcome {
    ShowOutcome {
        reference: request.reference.clone(),
        reason: Some(one_line(&reason)),
        messages,
        nodes: Vec::new(),
    }
}

/// What `written` names; `Ok(Err(reason))`: no reference at all (exit 1).
fn classify(
    written: &str,
    scheme: &IdScheme,
    messages: &mut Vec<Message>,
) -> Result<Result<Target, String>, CliError> {
    if written.ends_with(DOCUMENT_EXTENSION) {
        if !is_clean_relative(written) {
            return Err(CliError::spec(format!(
                "`{written}` is no clean root-relative path: no leading `/`, \
                 no `.`, `..` or empty component"
            )));
        }
        return Ok(Ok(Target::Path(written.to_owned())));
    }
    let Some(found) = grammar::parse_reference(written, 0, scheme) else {
        return Ok(Err(no_reference(written, scheme)));
    };
    let reference = found.reference;
    if !found.homoglyphs.is_empty() || reference.script == IdScript::Mixed {
        return Err(CliError::spec(latin_fix(written, &found.homoglyphs)));
    }
    if reference.project.is_some() {
        return Err(project_qualified(written));
    }
    if let Some(rev) = reference.rev {
        messages.push(Message::Note(format!(
            "`@{rev}` is ignored: `spec show` prints the current text"
        )));
    }
    Ok(Ok(Target::Reference(reference)))
}

/// Exit 1: `written` is no reference; the configured prefixes listed.
fn no_reference(written: &str, scheme: &IdScheme) -> String {
    let prefixes: Vec<&str> = scheme
        .prefixes()
        .iter()
        .map(|spec| spec.prefix.as_str())
        .collect();
    let mut legacy: Vec<&str> = scheme
        .prefixes()
        .iter()
        .flat_map(|spec| spec.aliases_from.iter().map(String::as_str))
        .collect();
    legacy.sort_unstable();
    if prefixes.is_empty() {
        return format!("`{written}` is no reference: the project's `[ids]` configures no prefix");
    }
    let mut reason = format!(
        "`{written}` is no reference to an ID of this project; prefixes: {}",
        prefixes.join(", ")
    );
    if !legacy.is_empty() {
        reason.push_str(&format!("; legacy prefixes: {}", legacy.join(", ")));
    }
    reason
}

/// Exit 2: look-alike letters or mixed scripts, with the Latin form.
fn latin_fix(written: &str, homoglyphs: &[specengine_model::Homoglyph]) -> String {
    let fix: String = if homoglyphs.is_empty() {
        written.chars().map(normalize_char).collect()
    } else {
        // Spans are offsets into `written` (parsed with base 0).
        let mut sorted: Vec<&specengine_model::Homoglyph> = homoglyphs.iter().collect();
        sorted.sort_by_key(|homoglyph| homoglyph.span.start);
        let mut fixed = String::new();
        let mut cursor = 0;
        for homoglyph in sorted {
            let (start, end) = (homoglyph.span.start, homoglyph.span.end);
            if start < cursor || end > written.len() {
                continue;
            }
            fixed.push_str(written.get(cursor..start).unwrap_or_default());
            fixed.push_str(&homoglyph.fix);
            cursor = end;
        }
        fixed.push_str(written.get(cursor..).unwrap_or_default());
        fixed
    };
    if fix.is_ascii() && fix != written {
        format!(
            "`{written}` mixes scripts or uses look-alike letters; IDs are Latin only: \
             write `{fix}`"
        )
    } else {
        format!("`{written}` mixes scripts; IDs are Latin only: write it in Latin letters")
    }
}

fn project_qualified(written: &str) -> CliError {
    CliError::spec(format!(
        "`{written}`: `project:` references are not supported yet; \
         drop the qualifier to read this project's node"
    ))
}

/// The nodes of one holder file that `target` names, in position order:
/// the ID's, else its `aliases_from` target's, else the document when its
/// `aliases:` still declare the reference; none when the fresh parse no
/// longer holds it.
fn pick(parsed: &ParsedFile, target: &Target, written: &str) -> Vec<usize> {
    let Target::Reference(reference) = target else {
        return vec![0];
    };
    let with_id = |id: &str, from: usize| -> Vec<usize> {
        parsed
            .nodes
            .iter()
            .enumerate()
            .skip(from)
            .filter(|(_, node)| node.id.as_deref() == Some(id))
            .map(|(ord, _)| ord)
            .collect()
    };
    if let Some(section) = &reference.section {
        return with_id(section, 1);
    }
    let mut found = match &reference.alias_of {
        None => with_id(&reference.id, 0),
        Some(_) => Vec::new(),
    };
    if found.is_empty()
        && let Some(prefix) = &reference.alias_of
        && let Some((_, body)) = reference.id.split_once('-')
    {
        found = with_id(&format!("{prefix}-{body}"), 0);
    }
    // An `aliases:` entry names the document; the fresh document must still
    // declare it (the ID or the bare text, as the resolver looks it up),
    // else the file changed since the update and holds nothing.
    if found.is_empty() {
        let bare = bare_text(reference, written);
        let declared = parsed
            .document()
            .and_then(|document| document.fields.as_ref())
            .and_then(|fields| fields.aliases.as_ref())
            .is_some_and(|aliases| {
                aliases
                    .iter()
                    .any(|alias| *alias == reference.id || alias == bare)
            });
        if declared {
            found.push(0);
        }
    }
    found
}

/// The written text before its section or revision, its `slug/` qualifier
/// dropped: the resolver's lookup key of an `aliases:` entry.
fn bare_text<'w>(reference: &Reference, written: &'w str) -> &'w str {
    let text = written.split(['#', '@']).next().unwrap_or(written);
    reference
        .scope
        .as_deref()
        .and_then(|slug| text.strip_prefix(slug)?.strip_prefix('/'))
        .unwrap_or(text)
}

/// Node `ord` of `parsed`, its text from `bytes`.
fn shown(
    path: &str,
    bytes: &[u8],
    parsed: &ParsedFile,
    ord: usize,
    archived: bool,
    utf8: bool,
) -> ShownNode {
    let node = &parsed.nodes[ord];
    let span = node.span;
    let text = String::from_utf8_lossy(bytes.get(span.range()).unwrap_or_default()).into_owned();
    let sections = parsed
        .nodes
        .iter()
        .enumerate()
        .skip(1)
        .filter(|&(other, section)| other != ord && span.contains(section.span))
        .filter_map(|(_, section)| {
            let id = section.id.clone()?;
            let heading_end = section
                .heading
                .map_or(section.span.start, |heading| heading.end);
            Some(NestedSection {
                id,
                heading_end: heading_end.saturating_sub(span.start),
            })
        })
        .collect();
    ShownNode {
        id: node.id.clone(),
        kind: node.kind.clone(),
        title: node.title.clone(),
        path: path.to_owned(),
        line: line_of(bytes, span.start),
        end_line: line_of(bytes, span.end.saturating_sub(1).max(span.start)),
        status: if ord == 0 {
            node.fields
                .as_ref()
                .and_then(|fields| fields.status.clone())
        } else {
            None
        },
        rev: node.rev,
        tokens_est: node.tokens_est,
        archived,
        utf8,
        sections,
        text,
    }
}

/// A file with no node (not UTF-8), shown whole.
fn whole_file(path: &str, bytes: &[u8], archived: bool, utf8: bool) -> ShownNode {
    let text = String::from_utf8_lossy(bytes).into_owned();
    ShownNode {
        id: None,
        kind: None,
        title: None,
        path: path.to_owned(),
        line: 1,
        end_line: line_of(bytes, bytes.len().saturating_sub(1)),
        status: None,
        rev: None,
        tokens_est: tokens_est(&text),
        archived,
        utf8,
        sections: Vec::new(),
        text,
    }
}

/// The 1-based line holding the byte at `offset` (`\n` counted; a CRLF
/// ending counts once).
fn line_of(bytes: &[u8], offset: usize) -> usize {
    let end = offset.min(bytes.len());
    bytes[..end].iter().filter(|&&byte| byte == b'\n').count() + 1
}

/// Iteration 2, item (2) of the spec-cli verification: which nodes of a
/// freshly parsed holder `pick` shows. The file may change between the
/// update that resolved it and the read that prints it; the document is
/// shown for a reference only while the fresh parse still holds the ID, its
/// `aliases_from` target, the section, or declares the alias. (Neutral
/// prefixes: this crate's sources hold no project's words, AC-17.)
#[cfg(test)]
mod pick_tests {
    use specengine_core::IdSchemeToml as _;

    use super::*;

    const SCHEME: &str = "[ids]\n\
        NODE = { kind = \"node\", shape = \"name\" }\n\
        PART = { kind = \"part\", shape = \"name\" }\n\
        WORD = { kind = \"word\", shape = \"name\" }\n\
        ASK  = { kind = \"ask\", width = 3, aliases_from = [\"OLDASK\"] }\n";

    fn picked(text: &str, written: &str) -> Vec<usize> {
        let scheme = IdScheme::from_toml(SCHEME).expect("scheme");
        let parsed = specengine_core::parse("notes/x.md", text.as_bytes(), &scheme);
        let reference = grammar::parse_reference(written, 0, &scheme)
            .unwrap_or_else(|| panic!("{written} is a reference"))
            .reference;
        pick(&parsed, &Target::Reference(reference), written)
    }

    const WORD: &str = "---\nid: WORD-tired\nclass: canon\naliases: [WORD-weary]\n---\n\n# Tired\n";
    const WORD_NO_ALIAS: &str = "---\nid: WORD-tired\nclass: canon\n---\n\n# Tired\n";
    const NODE: &str =
        "---\nid: NODE-LAMP\nclass: canon\n---\n\n# Lamp\n\n## Glow {#PART-LAMP-GLOW}\n\nText.\n";

    #[test]
    fn an_alias_still_declared_shows_the_document() {
        assert_eq!(picked(WORD, "WORD-weary"), [0]);
        assert_eq!(picked(WORD, "WORD-tired"), [0]);
    }

    #[test]
    fn an_alias_no_longer_declared_shows_nothing() {
        assert_eq!(picked(WORD_NO_ALIAS, "WORD-weary"), Vec::<usize>::new());
        // Another document that never held it: nothing either.
        assert_eq!(picked(NODE, "WORD-weary"), Vec::<usize>::new());
    }

    #[test]
    fn an_id_or_section_gone_from_the_fresh_parse_shows_nothing() {
        assert_eq!(picked(NODE, "NODE-LAMP#PART-LAMP-GLOW"), [1]);
        assert_eq!(picked(NODE, "PART-LAMP-GLOW"), [1]);
        let renamed = NODE.replace("PART-LAMP-GLOW", "PART-LAMP-DIM");
        assert_eq!(
            picked(&renamed, "NODE-LAMP#PART-LAMP-GLOW"),
            Vec::<usize>::new()
        );
        assert_eq!(picked(&renamed, "PART-LAMP-GLOW"), Vec::<usize>::new());
        assert_eq!(picked(NODE, "NODE-OTHER"), Vec::<usize>::new());
    }

    #[test]
    fn a_legacy_id_shows_its_target_only_while_it_is_there() {
        let ask = "---\nid: ASK-031\nclass: canon\n---\n\n# Ask\n";
        assert_eq!(picked(ask, "OLDASK-031"), [0]);
        assert_eq!(picked(ask, "ASK-031"), [0]);
        let moved = ask.replace("ASK-031", "ASK-032");
        assert_eq!(picked(&moved, "OLDASK-031"), Vec::<usize>::new());
    }
}
