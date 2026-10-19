//! The pure half of `spec verify`
//! (`docs/canon/spec-verify.md` "The `[verify]` table", "States",
//! "Verdict and exit"): the `[verify]` table read apart from the reads,
//! the row key, the facts one row is judged by, the judge, the classes and
//! the verdict (ADR-0042,
//! `docs/canon/architecture-code.md#verify`). Pure: facts in, a state out;
//! nothing here reads a file, a git object or the index, and the judge
//! never knows where its facts came from (a fresh walk and cascade, an
//! index's OIDs, an indexed hash), so every caller gets the same state for
//! the same facts.
//!
//! - **Table** ([`VerifyConfig::from_toml`]): closed, `mode` (`observe`,
//!   the default; `enforce-introduced`; `enforce`), `tests` (the project's
//!   own test command as an argv template, [`TestTemplate`]) and `timeout`
//!   (seconds per invocation, 1 to [`TIMEOUT_MAX`], default
//!   [`TIMEOUT_DEFAULT`]); a wrong type or value refused at its line.
//!   `ProjectConfig` only name-checks `verify`.
//! - **Bound tests** (task spec `verify-tests`): a template expanded over a
//!   test's placeholder values ([`TestTemplate::expand`]: each value one
//!   element's bytes, never split or joined, never a shell's), the probe's
//!   test name ([`PROBE_TEST`]), and the verdicts of a test, a node and a
//!   run ([`node_verdict`], [`tests_verdict`]). Which runner a project
//!   uses is its own: nothing here names one.
//! - **Key** ([`RowKey`]): a lock key ([`EntryKey`]) or a keyless
//!   binding's place, ordered as `EntryKey`, a keyless binding (no kind,
//!   symbol or item kind) first.
//! - **Judge** ([`judge_row`]): the first state of "States" that applies,
//!   with its category, detail and how the code was compared.
//! - **Verdict** ([`verdict`]): `cannot-verify` on any `cannot_verify`
//!   row or run cause, in every mode; `blocked` on a Fix row (under
//!   `enforce-introduced` one not Pre-existing); Check rows never block.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt;
use std::ops::Range;

use serde::Deserialize;
use toml::Spanned;

use crate::check::Mode;
use crate::lock::{EntryKey, LockKind, LockRelation, Verified, key_problem};
use crate::project_toml::ProjectError;

/// The keys `[verify]` takes.
const KEYS: [&str; 3] = ["mode", "tests", "timeout"];

/// `[verify] timeout` when not written, seconds.
pub const TIMEOUT_DEFAULT: u32 = 600;

/// The longest `[verify] timeout`, seconds (one day).
pub const TIMEOUT_MAX: u32 = 86_400;

/// `{test}` of every probe: a test no project has, which the project's
/// command must fail to find (task spec `verify-tests`, "Order" step 8).
pub const PROBE_TEST: &str = "specengine_verify_probe_no_such_test";

/// What `[verify] tests` must be, as its errors say.
const ARGV_LIST: &str =
    "[verify] tests must be an argv list, e.g. [\"scripts/run-test\", \"{test}\"]";

/// The `[verify]` table, checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyConfig {
    /// `observe` when not written.
    pub mode: Mode,
    /// The project's test command; `None` when not written.
    pub tests: Option<TestTemplate>,
    /// Seconds per invocation; [`TIMEOUT_DEFAULT`] when not written.
    pub timeout: u32,
}

impl Default for VerifyConfig {
    fn default() -> Self {
        Self {
            mode: Mode::Observe,
            tests: None,
            timeout: TIMEOUT_DEFAULT,
        }
    }
}

impl VerifyConfig {
    /// Reads the `[verify]` table of `specengine.toml`; none: the default.
    pub fn from_toml(text: &str) -> Result<Self, ProjectError> {
        verify_from_toml(text)
    }
}

/// [`VerifyConfig::from_toml`] as a function.
pub fn verify_from_toml(text: &str) -> Result<VerifyConfig, ProjectError> {
    let error_at = |span: Option<Range<usize>>, message: String| ProjectError {
        line: span.map(|span| line_of(text, span.start)),
        message,
    };
    let raw: RawVerifyFile = toml::from_str(text)
        .map_err(|error| error_at(error.span(), error.message().trim().to_owned()))?;
    let Some(table) = raw.verify else {
        return Ok(VerifyConfig::default());
    };
    let mut config = VerifyConfig::default();
    for (key, value) in table.get_ref() {
        let span = Some(value.span());
        match key.as_str() {
            "mode" => {
                let toml::Value::String(written) = value.get_ref() else {
                    return Err(error_at(
                        span,
                        format!(
                            "`[verify] mode` is a {}, not \"observe\", \"enforce-introduced\" or \
                             \"enforce\"",
                            value.get_ref().type_str()
                        ),
                    ));
                };
                config.mode = Mode::ALL
                    .into_iter()
                    .find(|known| known.as_str() == written)
                    .ok_or_else(|| {
                        error_at(
                            span.clone(),
                            format!(
                                "`[verify] mode` `{written}` is not \"observe\", \
                                 \"enforce-introduced\" or \"enforce\""
                            ),
                        )
                    })?;
            }
            "tests" => {
                config.tests =
                    Some(TestTemplate::from_value(value.get_ref()).map_err(|m| error_at(span, m))?);
            }
            "timeout" => {
                config.timeout = timeout_of(value.get_ref()).map_err(|m| error_at(span, m))?;
            }
            other => {
                return Err(error_at(
                    span,
                    format!(
                        "`[verify]` has no key `{other}`: it takes `{}` only",
                        KEYS.join("`, `")
                    ),
                ));
            }
        }
    }
    Ok(config)
}

/// Only `[verify]`, its keys read here.
#[derive(Deserialize)]
struct RawVerifyFile {
    #[serde(default)]
    verify: Option<Spanned<BTreeMap<String, Spanned<toml::Value>>>>,
}

fn line_of(text: &str, offset: usize) -> usize {
    let end = offset.min(text.len());
    text.as_bytes()[..end]
        .iter()
        .filter(|&&byte| byte == b'\n')
        .count()
        + 1
}

/// `[verify] timeout`: an integer from 1 to [`TIMEOUT_MAX`].
fn timeout_of(value: &toml::Value) -> Result<u32, String> {
    let toml::Value::Integer(seconds) = value else {
        return Err(format!(
            "[verify] timeout is a {}, not whole seconds from 1 to {TIMEOUT_MAX}",
            value.type_str()
        ));
    };
    u32::try_from(*seconds)
        .ok()
        .filter(|seconds| (1..=TIMEOUT_MAX).contains(seconds))
        .ok_or_else(|| format!("[verify] timeout {seconds} is not from 1 to {TIMEOUT_MAX} seconds"))
}

// ------------------------------------------------------------ bound tests

/// A value `[verify] tests` names: one of a bound test (task spec
/// `verify-tests`, "Placeholder values").
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Placeholder {
    /// The test's file, root-relative.
    File,
    /// Its `qpath` without a twin's discriminator.
    Qpath,
    /// Its manifest's `[package] name`.
    Package,
    /// Its unit's Cargo kind.
    TargetKind,
    /// Its unit's Cargo target name.
    Target,
    /// Its module path and name, joined by `::`.
    Test,
}

impl Placeholder {
    /// Every placeholder, in the order the spec lists them.
    pub const ALL: [Self; 6] = [
        Self::File,
        Self::Qpath,
        Self::Package,
        Self::TargetKind,
        Self::Target,
        Self::Test,
    ];

    /// Its name between the braces.
    pub const fn name(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Qpath => "qpath",
            Self::Package => "package",
            Self::TargetKind => "target_kind",
            Self::Target => "target",
            Self::Test => "test",
        }
    }
}

/// One part of a template element.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Part {
    Text(String),
    Value(Placeholder),
}

/// `[verify] tests`, checked: the elements as written and each one's
/// parts. Element 0 is the program; each element stays one argument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestTemplate {
    written: Vec<String>,
    elements: Vec<Vec<Part>>,
}

/// The values of one bound test's placeholders; `None`: it has none (an
/// empty one counts as none).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlaceholderValues {
    pub file: Option<String>,
    pub qpath: Option<String>,
    pub package: Option<String>,
    pub target_kind: Option<String>,
    pub target: Option<String>,
    pub test: Option<String>,
}

impl PlaceholderValues {
    /// The value of `placeholder`, if it has a non-empty one.
    pub fn get(&self, placeholder: Placeholder) -> Option<&str> {
        let value = match placeholder {
            Placeholder::File => &self.file,
            Placeholder::Qpath => &self.qpath,
            Placeholder::Package => &self.package,
            Placeholder::TargetKind => &self.target_kind,
            Placeholder::Target => &self.target,
            Placeholder::Test => &self.test,
        };
        value.as_deref().filter(|value| !value.is_empty())
    }
}

/// Why a bound test's argv cannot be made (task spec `verify-tests`,
/// "Runnable"), the first that applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unexpandable {
    /// A used placeholder's value holds U+0000-U+001F or U+007F.
    ControlCharacter,
    /// A used placeholder has no value.
    NoValue(Placeholder),
}

impl fmt::Display for Unexpandable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ControlCharacter => f.write_str("control character"),
            Self::NoValue(placeholder) => write!(f, "no value {{{}}}", placeholder.name()),
        }
    }
}

impl TestTemplate {
    /// The template of `elements` (as `[verify] tests` writes them): at
    /// least one, element 0 not empty; placeholders closed, `{{` and `}}`
    /// literal braces, any other brace refused; `{test}` or `{qpath}`
    /// used. `Err`: the message.
    pub fn parse(elements: &[String]) -> Result<Self, String> {
        if elements.is_empty() {
            return Err(format!("[verify] tests is empty: {ARGV_LIST}"));
        }
        if elements[0].is_empty() {
            return Err("[verify] tests[0], the program, is empty".to_owned());
        }
        let parsed = elements
            .iter()
            .enumerate()
            .map(|(index, element)| {
                parts(element)
                    .map_err(|why| format!("[verify] tests[{index}] {}: {why}", quoted(element)))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let template = Self {
            written: elements.to_vec(),
            elements: parsed,
        };
        if !template.uses(Placeholder::Test) && !template.uses(Placeholder::Qpath) {
            return Err(
                "[verify] tests uses neither {test} nor {qpath}: the command cannot tell which \
                 test to run"
                    .to_owned(),
            );
        }
        Ok(template)
    }

    /// The template of a TOML value: an array of strings.
    fn from_value(value: &toml::Value) -> Result<Self, String> {
        let toml::Value::Array(items) = value else {
            return Err(ARGV_LIST.to_owned());
        };
        let elements = items
            .iter()
            .enumerate()
            .map(|(index, item)| match item {
                toml::Value::String(text) => Ok(text.clone()),
                other => Err(format!(
                    "[verify] tests[{index}] is a {}, not a string: {ARGV_LIST}",
                    other.type_str()
                )),
            })
            .collect::<Result<Vec<_>, _>>()?;
        Self::parse(&elements)
    }

    /// The elements as written.
    pub fn written(&self) -> &[String] {
        &self.written
    }

    /// Some element uses `placeholder`.
    pub fn uses(&self, placeholder: Placeholder) -> bool {
        self.elements
            .iter()
            .flatten()
            .any(|part| *part == Part::Value(placeholder))
    }

    /// The argv of a test with `values`: each element its text with each
    /// placeholder's value put in as bytes, one argument whatever it holds;
    /// never split, joined or quoted. `Err`: the first reason it cannot be
    /// made, a control character in any used value before a missing one
    /// (in the order the template uses them).
    pub fn expand(&self, values: &PlaceholderValues) -> Result<Vec<String>, Unexpandable> {
        let used: Vec<Placeholder> = self
            .elements
            .iter()
            .flatten()
            .filter_map(|part| match part {
                Part::Value(placeholder) => Some(*placeholder),
                Part::Text(_) => None,
            })
            .collect();
        if used
            .iter()
            .filter_map(|placeholder| values.get(*placeholder))
            .any(has_control)
        {
            return Err(Unexpandable::ControlCharacter);
        }
        if let Some(missing) = used
            .iter()
            .find(|placeholder| values.get(**placeholder).is_none())
        {
            return Err(Unexpandable::NoValue(*missing));
        }
        Ok(self
            .elements
            .iter()
            .map(|element| {
                element
                    .iter()
                    .map(|part| match part {
                        Part::Text(text) => text.as_str(),
                        Part::Value(placeholder) => values.get(*placeholder).unwrap_or_default(),
                    })
                    .collect()
            })
            .collect())
    }
}

/// A value holds U+0000-U+001F or U+007F.
fn has_control(value: &str) -> bool {
    value.chars().any(|c| c <= '\u{1f}' || c == '\u{7f}')
}

/// `element` as quoted in an error: backticks, control characters escaped.
fn quoted(element: &str) -> String {
    format!("`{}`", element.escape_debug())
}

/// The parts of one element: text, `{{` and `}}` as braces, `{<name>}` a
/// placeholder; any other brace refused.
fn parts(element: &str) -> Result<Vec<Part>, String> {
    let mut out = Vec::new();
    let mut text = String::new();
    let mut rest = element;
    while let Some(at) = rest.find(['{', '}']) {
        text.push_str(&rest[..at]);
        let tail = &rest[at..];
        if let Some(after) = tail.strip_prefix("{{") {
            text.push('{');
            rest = after;
        } else if let Some(after) = tail.strip_prefix("}}") {
            text.push('}');
            rest = after;
        } else if let Some(after) = tail.strip_prefix('{') {
            let Some(close) = after
                .find(['{', '}'])
                .filter(|&end| after[end..].starts_with('}'))
            else {
                return Err("a lone `{`: {{ writes a brace".to_owned());
            };
            let name = &after[..close];
            let Some(placeholder) = Placeholder::ALL
                .into_iter()
                .find(|placeholder| placeholder.name() == name)
            else {
                let names: Vec<String> = Placeholder::ALL
                    .iter()
                    .map(|placeholder| format!("{{{}}}", placeholder.name()))
                    .collect();
                return Err(format!(
                    "`{{{}}}` is no placeholder: {}; {{{{ and }}}} write a brace",
                    name.escape_debug(),
                    names.join(", ")
                ));
            };
            if !text.is_empty() {
                out.push(Part::Text(std::mem::take(&mut text)));
            }
            out.push(Part::Value(placeholder));
            rest = &after[close + 1..];
        } else {
            return Err("a lone `}`: }} writes a brace".to_owned());
        }
    }
    text.push_str(rest);
    if !text.is_empty() || out.is_empty() {
        out.push(Part::Text(text));
    }
    Ok(out)
}

/// How one bound test ended (task spec `verify-tests`, "Verdicts").
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TestOutcome {
    /// Exit 0 within the timeout.
    Passed,
    /// A non-zero exit or a signal.
    Failed,
    /// Still running at the timeout.
    TimedOut,
    /// Never started, or stopped by the run.
    NotRun,
}

impl TestOutcome {
    /// The JSON word.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::TimedOut => "timed_out",
            Self::NotRun => "not_run",
        }
    }

    /// The text word.
    pub const fn words(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::TimedOut => "timed out",
            Self::NotRun => "not run",
        }
    }
}

/// A node's verdict of one run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NodeVerdict {
    /// One runnable test or more, all passed.
    Verified,
    /// A runnable test not passed.
    Failed,
    /// Accept's predicate refuses a binding of it.
    NotVerifiable,
    /// No test of it can run.
    NoRunnableTest,
}

impl NodeVerdict {
    /// The JSON word.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Verified => "verified",
            Self::Failed => "failed",
            Self::NotVerifiable => "not_verifiable",
            Self::NoRunnableTest => "no_runnable_test",
        }
    }

    /// The text words.
    pub const fn words(self) -> &'static str {
        match self {
            Self::Verified => "verified",
            Self::Failed => "failed",
            Self::NotVerifiable => "not verifiable",
            Self::NoRunnableTest => "no runnable test",
        }
    }
}

/// The verdict of a node: not verifiable first; then no runnable test;
/// verified only when every one of its runnable tests passed.
pub fn node_verdict(verifiable: bool, runnable: &[TestOutcome]) -> NodeVerdict {
    if !verifiable {
        NodeVerdict::NotVerifiable
    } else if runnable.is_empty() {
        NodeVerdict::NoRunnableTest
    } else if runnable
        .iter()
        .all(|outcome| *outcome == TestOutcome::Passed)
    {
        NodeVerdict::Verified
    } else {
        NodeVerdict::Failed
    }
}

/// The verdict of a whole `spec verify --tests` run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RunVerdict {
    /// Every reported node verified.
    Verified,
    /// A reported node not verified.
    Unverified,
    /// The tree was not clean before the run.
    Dirty,
    /// Nothing to run.
    Empty,
    /// The owner did not consent.
    Declined,
    /// The tree or `HEAD` changed during the run.
    Changed,
    /// A probe passed or timed out, or a program could not start.
    Refused,
    /// A signal to `spec` stopped the run.
    Interrupted,
    /// The run could not be made.
    Cannot,
}

impl RunVerdict {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Verified => "verified",
            Self::Unverified => "unverified",
            Self::Dirty => "dirty",
            Self::Empty => "empty",
            Self::Declined => "declined",
            Self::Changed => "changed",
            Self::Refused => "refused",
            Self::Interrupted => "interrupted",
            Self::Cannot => "cannot",
        }
    }

    /// The exit code: 0 verified; 1 not verified (ADR-0044's command, never
    /// a Fix finding of ADR-0042's judge); 2 refused, interrupted, cannot.
    pub const fn exit_code(self) -> u8 {
        match self {
            Self::Verified => 0,
            Self::Unverified | Self::Dirty | Self::Empty | Self::Declined | Self::Changed => 1,
            Self::Refused | Self::Interrupted | Self::Cannot => 2,
        }
    }
}

/// The verdict of a completed run over its reported nodes' verdicts.
pub fn tests_verdict(nodes: impl IntoIterator<Item = NodeVerdict>) -> RunVerdict {
    let mut any = false;
    for node in nodes {
        any = true;
        if node != NodeVerdict::Verified {
            return RunVerdict::Unverified;
        }
    }
    if any {
        RunVerdict::Verified
    } else {
        RunVerdict::Unverified
    }
}

// ------------------------------------------------------------------- key

/// What a row's key binds: a lock key's binding, or none (a keyless
/// marker: `unanchored` or `cannot_verify`, with its category).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RowBinding {
    Keyed {
        kind: LockKind,
        symbol: String,
        item_kind: Option<String>,
    },
    Keyless {
        /// `unanchored` or `cannot_verify`.
        state: String,
        category: String,
    },
}

/// One row's key: a lock key, or a keyless binding's node, relation, file,
/// state and category.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RowKey {
    /// The canonical cited ID; an unresolved marker's token.
    pub node: String,
    pub relation: LockRelation,
    pub file: String,
    pub binding: RowBinding,
}

impl RowKey {
    /// The row of a lock key.
    pub fn of_entry(key: &EntryKey) -> Self {
        Self {
            node: key.node.clone(),
            relation: key.relation,
            file: key.file.clone(),
            binding: RowBinding::Keyed {
                kind: key.kind,
                symbol: key.symbol.clone(),
                item_kind: key.item_kind.clone(),
            },
        }
    }

    /// The lock key; `None` for a keyless binding.
    pub fn entry_key(&self) -> Option<EntryKey> {
        match &self.binding {
            RowBinding::Keyed {
                kind,
                symbol,
                item_kind,
            } => Some(EntryKey {
                node: self.node.clone(),
                relation: self.relation,
                file: self.file.clone(),
                kind: *kind,
                symbol: symbol.clone(),
                item_kind: item_kind.clone(),
            }),
            RowBinding::Keyless { .. } => None,
        }
    }
}

impl Ord for RowKey {
    /// [`EntryKey`]'s order; a keyless binding (no kind) before every kind,
    /// keyless ones by state and category bytes.
    fn cmp(&self, other: &Self) -> Ordering {
        self.node
            .as_bytes()
            .cmp(other.node.as_bytes())
            .then_with(|| self.relation.cmp(&other.relation))
            .then_with(|| self.file.as_bytes().cmp(other.file.as_bytes()))
            .then_with(|| match (&self.binding, &other.binding) {
                (
                    RowBinding::Keyless { state, category },
                    RowBinding::Keyless {
                        state: other_state,
                        category: other_category,
                    },
                ) => state
                    .as_bytes()
                    .cmp(other_state.as_bytes())
                    .then_with(|| category.as_bytes().cmp(other_category.as_bytes())),
                (RowBinding::Keyless { .. }, RowBinding::Keyed { .. }) => Ordering::Less,
                (RowBinding::Keyed { .. }, RowBinding::Keyless { .. }) => Ordering::Greater,
                (
                    RowBinding::Keyed {
                        kind,
                        symbol,
                        item_kind,
                    },
                    RowBinding::Keyed {
                        kind: other_kind,
                        symbol: other_symbol,
                        item_kind: other_item_kind,
                    },
                ) => kind
                    .as_str()
                    .cmp(other_kind.as_str())
                    .then_with(|| symbol.as_bytes().cmp(other_symbol.as_bytes()))
                    .then_with(|| {
                        item_kind
                            .as_deref()
                            .map(str::as_bytes)
                            .cmp(&other_item_kind.as_deref().map(str::as_bytes))
                    }),
            })
    }
}

impl PartialOrd for RowKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

// ----------------------------------------------------------------- facts

/// The node a row names, as the spec reads now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeFact {
    /// One node: its revision R (`None`: unreadable) and its `norm_hash` H
    /// (`None`: null).
    Found {
        rev: Option<u32>,
        hash: Option<String>,
    },
    /// The ID names several nodes.
    Ambiguous,
    /// A marker's ID is a look-alike or of mixed script.
    Homoglyph,
    /// A marker's ID names no node.
    Dangling,
    /// A lock entry's `node` is no node's canonical ID (deleted, renamed,
    /// an alias now).
    Gone,
}

/// A marker's `@N`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkerRev {
    /// No `@` after the ID.
    Unversioned,
    Written(u32),
    /// `@` not followed by 1-9 digits ending the token.
    Bad,
}

/// One marker giving the row's key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarkerFact {
    /// 1-based, the line of its `@`.
    pub line: usize,
    pub rev: MarkerRev,
    /// Its level list is malformed.
    pub bad_levels: bool,
}

/// How a bound binding's code compares with its lock entry, as its
/// producer found it (the cascade, an index's OIDs, an indexed hash): the
/// judge never compares hashes itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Comparison {
    /// Equal to the entry, by the file's blob or by the item's hash.
    Equal(Compared),
    /// Changed since the entry, by the blob (`module`, `ron`) or by the
    /// item's hash after the blob differed.
    Changed(Compared),
    /// The item's hash refused: its first error category, snake case.
    CannotVerify(String),
    /// Not compared: no entry to compare with, or no hash made.
    NotHashed,
}

/// Why no marker gives a lock entry's key now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unbound {
    /// The file is absent.
    FileGone,
    /// The file is there but cannot be judged: `unreadable`, `not_utf8`,
    /// `outside_code`.
    Unreadable(String),
    /// The symbol is absent, no orphan parse error in the file.
    SymbolGone,
    /// The symbol is absent from a file with an orphan parse error.
    HiddenByError,
    /// The symbol names an item with a parse error, and a keyless
    /// `cannot_verify` marker of the entry's node and relation lies in the
    /// file: the item's first error category.
    ItemError(String),
    /// The symbol is there; no marker of the node binds it.
    MarkerGone,
}

/// The code a row names, now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodeFact {
    /// Bound by the row's markers, compared with the row's entry.
    Bound(Comparison),
    /// Bound, but the binding is ambiguous: `path_attribute`, `unrooted`,
    /// `duplicate`.
    Ambiguous(String),
    /// A keyless marker binding nothing: its category.
    Unanchored(String),
    /// A keyless marker on an item with a parse error (its first error
    /// category), or in a RON container nested too deep.
    ItemError(String),
    /// No marker gives the key: why.
    Unbound(Unbound),
}

/// Everything one row is judged by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowFacts {
    pub key: RowKey,
    pub node: NodeFact,
    /// The row's lock entry E.
    pub entry: Option<Verified>,
    /// The markers giving the key, in file order; none for an entry alone.
    pub markers: Vec<MarkerFact>,
    pub code: CodeFact,
}

// ----------------------------------------------------------------- judge

/// A row's state, in the order "States" tries them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum State {
    CannotVerify,
    Predated,
    BadMarker,
    BrokenBinding,
    Unlocked,
    SpecAhead,
    CodeAhead,
    Conflict,
    Ok,
}

impl State {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CannotVerify => "cannot_verify",
            Self::Predated => "predated",
            Self::BadMarker => "bad_marker",
            Self::BrokenBinding => "broken_binding",
            Self::Unlocked => "unlocked",
            Self::SpecAhead => "spec_ahead",
            Self::CodeAhead => "code_ahead",
            Self::Conflict => "conflict",
            Self::Ok => "ok",
        }
    }

    /// The state's class; `None` for `ok`.
    pub const fn class(self) -> Option<Class> {
        match self {
            Self::CannotVerify => Some(Class::Cannot),
            Self::Predated | Self::BadMarker | Self::BrokenBinding | Self::Conflict => {
                Some(Class::Fix)
            }
            Self::Unlocked | Self::SpecAhead | Self::CodeAhead => Some(Class::Check),
            Self::Ok => None,
        }
    }
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A finding's class: Fix (broken by the change), Check (a human
/// re-reads), Cannot (could not be verified).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Class {
    Fix,
    Check,
    Cannot,
}

impl Class {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fix => "fix",
            Self::Check => "check",
            Self::Cannot => "cannot",
        }
    }
}

/// How the code side was compared with the entry: by the file's blob
/// alone, or by the item's hash after the blob differed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Compared {
    Blob,
    Ast,
}

impl Compared {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Blob => "blob",
            Self::Ast => "ast",
        }
    }
}

/// A row's judgement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Judgement {
    pub state: State,
    pub category: Option<String>,
    pub detail: Option<String>,
    /// `None`: no entry, or the state was decided before the code was
    /// compared.
    pub compared: Option<Compared>,
}

impl Judgement {
    fn new(state: State, category: Option<&str>, detail: Option<&str>) -> Self {
        Self {
            state,
            category: category.map(str::to_owned),
            detail: detail.map(str::to_owned),
            compared: None,
        }
    }
}

/// The categories and details the judge gives.
pub const ITEM_ERROR: &str = "item_error";
pub const AMBIGUOUS_SYMBOL: &str = "ambiguous_symbol";
pub const AMBIGUOUS_ID: &str = "ambiguous_id";
pub const REV_UNKNOWN: &str = "rev_unknown";
pub const HASH_NULL: &str = "hash_null";
pub const UNREADABLE: &str = "unreadable";
pub const HIDDEN_BY_ERROR: &str = "hidden_by_error";
pub const DANGLING: &str = "dangling";
pub const HOMOGLYPH: &str = "homoglyph";
pub const BAD_REV: &str = "bad_rev";
pub const UNANCHORED: &str = "unanchored";
pub const BAD_LEVELS: &str = "bad_levels";
pub const NODE_GONE: &str = "node_gone";
pub const FILE_GONE: &str = "file_gone";
pub const SYMBOL_GONE: &str = "symbol_gone";
pub const MARKER_GONE: &str = "marker_gone";
pub const KEY_PROBLEM: &str = "key_problem";
pub const MARKER_BEHIND: &str = "marker_behind";
pub const SPEC_CHANGED: &str = "spec_changed";
pub const SAME_REVISION: &str = "same_revision";
pub const CODE_CHANGED: &str = "code_changed";
pub const RAISED: &str = "raised";

/// The row's state: the first of "States" that applies.
pub fn judge_row(facts: &RowFacts) -> Judgement {
    let found = match &facts.node {
        NodeFact::Found { rev, hash } => Some((*rev, hash.as_deref())),
        _ => None,
    };

    // cannot_verify
    if let CodeFact::ItemError(category) = &facts.code {
        return Judgement::new(State::CannotVerify, Some(ITEM_ERROR), Some(category));
    }
    if facts.entry.is_some()
        && let CodeFact::Unbound(Unbound::ItemError(category)) = &facts.code
    {
        return Judgement::new(State::CannotVerify, Some(ITEM_ERROR), Some(category));
    }
    if let CodeFact::Ambiguous(reason) = &facts.code {
        return Judgement::new(State::CannotVerify, Some(AMBIGUOUS_SYMBOL), Some(reason));
    }
    if facts.node == NodeFact::Ambiguous {
        return Judgement::new(State::CannotVerify, Some(AMBIGUOUS_ID), None);
    }
    if let Some((rev, hash)) = found {
        if rev.is_none() {
            return Judgement::new(State::CannotVerify, Some(REV_UNKNOWN), None);
        }
        if hash.is_none() {
            return Judgement::new(State::CannotVerify, Some(HASH_NULL), None);
        }
    }
    if let CodeFact::Unbound(Unbound::Unreadable(detail)) = &facts.code {
        return Judgement::new(State::CannotVerify, Some(UNREADABLE), Some(detail));
    }
    if facts.entry.is_some() && facts.code == CodeFact::Unbound(Unbound::HiddenByError) {
        return Judgement::new(State::CannotVerify, Some(HIDDEN_BY_ERROR), None);
    }
    if let CodeFact::Bound(Comparison::CannotVerify(category)) = &facts.code {
        return Judgement::new(State::CannotVerify, Some(ITEM_ERROR), Some(category));
    }

    // predated
    let rev = found.and_then(|(rev, _)| rev);
    let written = || {
        facts.markers.iter().filter_map(|marker| match marker.rev {
            MarkerRev::Written(n) => Some(n),
            MarkerRev::Unversioned | MarkerRev::Bad => None,
        })
    };
    if let Some(rev) = rev
        && written().any(|n| n > rev)
    {
        return Judgement::new(State::Predated, None, None);
    }

    // bad_marker
    match facts.node {
        NodeFact::Dangling => return Judgement::new(State::BadMarker, Some(DANGLING), None),
        NodeFact::Homoglyph => return Judgement::new(State::BadMarker, Some(HOMOGLYPH), None),
        _ => {}
    }
    if facts
        .markers
        .iter()
        .any(|marker| marker.rev == MarkerRev::Bad)
    {
        return Judgement::new(State::BadMarker, Some(BAD_REV), None);
    }
    if let CodeFact::Unanchored(category) = &facts.code {
        return Judgement::new(State::BadMarker, Some(UNANCHORED), Some(category));
    }
    if facts.markers.iter().any(|marker| marker.bad_levels) {
        return Judgement::new(State::BadMarker, Some(BAD_LEVELS), None);
    }

    // broken_binding
    if facts.entry.is_some() {
        if facts.node == NodeFact::Gone {
            return Judgement::new(State::BrokenBinding, Some(NODE_GONE), None);
        }
        let gone = match &facts.code {
            CodeFact::Unbound(Unbound::FileGone) => Some(FILE_GONE),
            CodeFact::Unbound(Unbound::SymbolGone) => Some(SYMBOL_GONE),
            CodeFact::Unbound(Unbound::MarkerGone) => Some(MARKER_GONE),
            _ => None,
        };
        if let Some(category) = gone {
            return Judgement::new(State::BrokenBinding, Some(category), None);
        }
    }

    // unlocked
    if facts
        .key
        .entry_key()
        .as_ref()
        .and_then(key_problem)
        .is_some()
    {
        return Judgement::new(State::Unlocked, None, Some(KEY_PROBLEM));
    }
    let lagging = rev.is_some_and(|rev| written().any(|n| n < rev));
    let Some(entry) = &facts.entry else {
        return if lagging {
            Judgement::new(State::SpecAhead, Some(MARKER_BEHIND), None)
        } else {
            Judgement::new(State::Unlocked, None, None)
        };
    };

    // The comparison: an entry, its node found, its binding bound.
    let (Some(rev), Some((_, Some(hash)))) = (rev, found) else {
        // Not reached: every other node or code fact is decided above.
        return Judgement::new(State::CannotVerify, None, None);
    };
    let (code_changed, compared) = match &facts.code {
        CodeFact::Bound(Comparison::Equal(by)) => (false, *by),
        CodeFact::Bound(Comparison::Changed(by)) => (true, *by),
        // An entry whose code was never compared is never `ok`.
        _ => return Judgement::new(State::CannotVerify, Some(ITEM_ERROR), None),
    };
    let spec_changed = (rev, hash) != (entry.rev, entry.spec_hash.as_str());
    let raised = rev > entry.rev && written().any(|n| n == rev);
    let mut judged = if lagging && !code_changed {
        Judgement::new(State::SpecAhead, Some(MARKER_BEHIND), None)
    } else if spec_changed && !raised && !code_changed {
        let category = if rev == entry.rev {
            SAME_REVISION
        } else {
            SPEC_CHANGED
        };
        Judgement::new(State::SpecAhead, Some(category), None)
    } else if code_changed && !spec_changed {
        Judgement::new(State::CodeAhead, Some(CODE_CHANGED), None)
    } else if spec_changed && raised {
        Judgement::new(State::CodeAhead, Some(RAISED), None)
    } else if spec_changed && code_changed {
        Judgement::new(State::Conflict, None, None)
    } else {
        Judgement::new(State::Ok, None, None)
    };
    judged.compared = Some(compared);
    judged
}

// --------------------------------------------------------------- verdict

/// What a run says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Verdict {
    /// Every row `ok`.
    Clean,
    /// Findings, none blocking.
    Observed,
    /// A Fix row the mode stops on.
    Blocked,
    /// A `cannot_verify` row or a run cause.
    CannotVerify,
}

impl Verdict {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Clean => "clean",
            Self::Observed => "observed",
            Self::Blocked => "blocked",
            Self::CannotVerify => "cannot-verify",
        }
    }
}

/// One judged row as the verdict reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Finding {
    pub state: State,
    /// `HEAD` gives the same key, state and category.
    pub pre_existing: bool,
}

/// The verdict of a run in `mode` (the mode judged: without a base,
/// `enforce-introduced` is judged as `enforce` by the caller): a run cause
/// or a `cannot_verify` row wins in every mode; `enforce` blocks on any
/// Fix row, `enforce-introduced` on one not Pre-existing, `observe` never.
pub fn verdict(mode: Mode, run_causes: bool, rows: impl IntoIterator<Item = Finding>) -> Verdict {
    if run_causes {
        return Verdict::CannotVerify;
    }
    let mut verdict = Verdict::Clean;
    for row in rows {
        let blocks = match row.state.class() {
            Some(Class::Cannot) => return Verdict::CannotVerify,
            Some(Class::Fix) => match mode {
                Mode::Observe => false,
                Mode::EnforceIntroduced => !row.pre_existing,
                Mode::Enforce => true,
            },
            Some(Class::Check) => false,
            None => continue,
        };
        verdict = verdict.max(if blocks {
            Verdict::Blocked
        } else {
            Verdict::Observed
        });
    }
    verdict
}

/// Whether a row judged now is Pre-existing: not `ok`, and `head` (the
/// base's judgements by key) gives the same state and category.
pub fn is_pre_existing(
    key: &RowKey,
    judged: &Judgement,
    head: &BTreeMap<RowKey, Judgement>,
) -> bool {
    judged.state != State::Ok
        && head.get(key).is_some_and(|at_head| {
            at_head.state == judged.state && at_head.category == judged.category
        })
}
