//! `spec guide [TOPIC[#ID]]` (task spec `specengine-guide`, "The embedded
//! guide (CLI)", and its decision record): SpecEngine's own guide, one page
//! per topic, embedded in this build by the crate's build script as
//! [`GUIDE`] and served read-only beside a project, never inside it.
//!
//! No discovery, no `HOME`, no data directory, no index, no git, no clock:
//! [`guide`] reads only the embedded pages, so it answers in any directory,
//! with or without a project, and writes nothing. The pages are static data
//! of the binary: nothing is cached, held per call or grown, and no project
//! turn is taken (MCP's `get_guide`, the daemon's `/api/guide` call this one
//! function). One build gives one guide: every answer is a function of the
//! REF alone.
//!
//! ```text
//! <topic> | <title> | <summary> | rules <n>        no TOPIC: one line per topic, in GUIDE_ORDER
//! <the page's text>                                 TOPIC: its bytes after the front-matter
//! <the rule's section>                              TOPIC#ID: from its heading to the next of its level or above
//! ```
//!
//! JSON: `{topics: [{topic, title, summary, rules}]}`; `{topic, title,
//! summary, summarises, reviewed, rules: [{id, title, line}], text}`;
//! `{topic, rule: {id, title, line}, text}`. An unknown topic or rule is
//! exit 1 with its line on stderr and nothing on stdout; a REF holding a
//! space or a control character is exit 2. A page is at most a Tier 2
//! canon page (12 288 B), so [`crate::OUTPUT_CAP_CHARS`] never binds and
//! nothing is cut.

use serde::Serialize;

use crate::{CliError, Exit, escape_controls, one_line};

/// One rule of a guide page: a heading outside a fenced block whose `{#…}`
/// attribute is a `GD-` ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuideRule {
    /// The rule's ID, `GD-<TOPIC>-<NAME>`.
    pub id: &'static str,
    /// The heading's text before its `{#…}` block, as written (Markdown
    /// kept).
    pub title: &'static str,
    /// The heading's 1-based line in its page's file, front-matter counted.
    pub line: usize,
    /// The rule's section in its page's [`GuidePage::text`], byte offsets:
    /// its heading's line to the next heading of its level or above, or
    /// the end; its nested sections included.
    pub start: usize,
    pub end: usize,
}

/// One page of the guide, the file `<topic>.md` of the guide's directory,
/// as this build embedded it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuidePage {
    /// The file stem.
    pub topic: &'static str,
    /// The first level-1 heading's text, as written (Markdown kept).
    pub title: &'static str,
    /// The first sentence of the paragraph after the title, as written:
    /// up to its first `.` followed by whitespace, or the paragraph's end.
    pub summary: &'static str,
    /// `summarises:`, each entry as written (a root-relative `.md` path,
    /// an optional `#anchor`).
    pub summarises: &'static [&'static str],
    /// `reviewed:`, as written.
    pub reviewed: &'static str,
    /// Its rules, in page order.
    pub rules: &'static [GuideRule],
    /// The page's bytes after its front-matter's closing line.
    pub text: &'static str,
}

impl GuidePage {
    /// The rule's section, its bytes as the page holds them.
    pub fn section(&self, rule: &GuideRule) -> &'static str {
        self.text.get(rule.start..rule.end).unwrap_or_default()
    }

    /// The rule `id` of this page.
    pub fn rule(&self, id: &str) -> Option<&'static GuideRule> {
        self.rules.iter().find(|rule| rule.id == id)
    }
}

include!(concat!(env!("OUT_DIR"), "/guide.rs"));

// The output cap never binds on the guide (task spec `specengine-guide`,
// "The embedded guide (CLI)"): a page longer than it fails the build, as
// bytes are never fewer than characters.
const _: () = assert!(GUIDE_LONGEST <= crate::OUTPUT_CAP_CHARS);

/// The page of `topic`, if the guide holds it.
pub fn guide_page(topic: &str) -> Option<&'static GuidePage> {
    GUIDE.iter().find(|page| page.topic == topic)
}

/// `spec guide` options.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GuideRequest {
    /// `TOPIC[#ID]`; absent: the index.
    pub reference: Option<String>,
}

/// What `spec guide` answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuideOutcome {
    /// No TOPIC: every topic.
    Index,
    /// TOPIC: its page.
    Page(&'static GuidePage),
    /// TOPIC#ID: one rule's section.
    Rule(&'static GuidePage, &'static GuideRule),
}

/// `spec guide [TOPIC[#ID]]`: the index, a page or one rule. Exit 1 for a
/// topic or a rule the guide does not hold, naming the ones it does; exit 2
/// for a REF holding a space or a control character.
pub fn guide(request: &GuideRequest) -> Result<GuideOutcome, CliError> {
    let Some(reference) = request.reference.as_deref() else {
        return Ok(GuideOutcome::Index);
    };
    if reference
        .chars()
        .any(|c| c.is_whitespace() || c.is_control())
    {
        return Err(CliError::spec(format!(
            "guide REF `{}` holds a space or a control character; write TOPIC or TOPIC#ID",
            escape_controls(reference)
        )));
    }
    let (topic, rule) = match reference.split_once('#') {
        Some((topic, rule)) => (topic, Some(rule)),
        None => (reference, None),
    };
    let Some(page) = guide_page(topic) else {
        return Err(not_found(format!(
            "no guide topic `{topic}`; topics: {}",
            GUIDE_ORDER.join(", ")
        )));
    };
    let Some(id) = rule else {
        return Ok(GuideOutcome::Page(page));
    };
    match page.rule(id) {
        Some(rule) => Ok(GuideOutcome::Rule(page, rule)),
        None => {
            let rules: Vec<&str> = page.rules.iter().map(|rule| rule.id).collect();
            let rules = if rules.is_empty() {
                "none".to_owned()
            } else {
                rules.join(", ")
            };
            Err(not_found(format!(
                "no rule `{id}` in guide topic `{topic}`; rules: {rules}"
            )))
        }
    }
}

/// Exit 1, `spec: message`, nothing on stdout.
fn not_found(message: String) -> CliError {
    CliError {
        exit: Exit::NotFound,
        message: one_line(&format!("spec: {message}")),
    }
}

/// The outcome as stdout text.
pub(crate) fn render_text(outcome: &GuideOutcome) -> String {
    match outcome {
        GuideOutcome::Index => GUIDE
            .iter()
            .map(|page| {
                format!(
                    "{} | {} | {} | rules {}\n",
                    page.topic,
                    page.title,
                    page.summary,
                    page.rules.len()
                )
            })
            .collect(),
        GuideOutcome::Page(page) => page.text.to_owned(),
        GuideOutcome::Rule(page, rule) => page.section(rule).to_owned(),
    }
}

/// `{topics}`: the index.
#[derive(Serialize)]
struct IndexDocument {
    topics: Vec<TopicRow>,
}

/// One topic of the index: its rule count.
#[derive(Serialize)]
struct TopicRow {
    topic: &'static str,
    title: &'static str,
    summary: &'static str,
    rules: usize,
}

/// A page.
#[derive(Serialize)]
struct PageDocument {
    topic: &'static str,
    title: &'static str,
    summary: &'static str,
    summarises: &'static [&'static str],
    reviewed: &'static str,
    rules: Vec<RuleRow>,
    text: &'static str,
}

/// A rule's `{id, title, line}`.
#[derive(Serialize)]
struct RuleRow {
    id: &'static str,
    title: &'static str,
    line: usize,
}

impl RuleRow {
    fn of(rule: &GuideRule) -> Self {
        Self {
            id: rule.id,
            title: rule.title,
            line: rule.line,
        }
    }
}

/// One rule read.
#[derive(Serialize)]
struct RuleDocument {
    topic: &'static str,
    rule: RuleRow,
    text: &'static str,
}

impl Serialize for GuideOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match *self {
            Self::Index => IndexDocument {
                topics: GUIDE
                    .iter()
                    .map(|page| TopicRow {
                        topic: page.topic,
                        title: page.title,
                        summary: page.summary,
                        rules: page.rules.len(),
                    })
                    .collect(),
            }
            .serialize(serializer),
            Self::Page(page) => PageDocument {
                topic: page.topic,
                title: page.title,
                summary: page.summary,
                summarises: page.summarises,
                reviewed: page.reviewed,
                rules: page.rules.iter().map(RuleRow::of).collect(),
                text: page.text,
            }
            .serialize(serializer),
            Self::Rule(page, rule) => RuleDocument {
                topic: page.topic,
                rule: RuleRow::of(rule),
                text: page.section(rule),
            }
            .serialize(serializer),
        }
    }
}
