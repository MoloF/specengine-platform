//! `spec.lock`'s model (`docs/canon/spec-lock.md` "The file", "Entry";
//! format 2: task spec `lock-levels`, "Format 2", "Readers"): the reader,
//! the canonical render, an accept's merge into one node's entries
//! (`docs/canon/spec-lock-accept.md` "Steps", step 6), the editorial
//! refresh (its "`--editorial`"), a gone node's drop (its "A gone node"),
//! `spec lock rebase`'s plan (task spec `lock-rebase`: [`plan_rebase`],
//! [`DropReason`]) and `spec lock regen`'s merge of a conflict's stages
//! (`docs/canon/spec-lock.md` "`spec lock regen`"). Pure: the text in, the
//! values out; no hash is computed and no file or git object read here
//! (the CLI hashes, reads and writes); the code side is digest strings, the
//! four level names ([`LockLevel`]) and `body_bytes`. Writers: ADR-0041,
//! ADR-0050, `docs/canon/architecture-code.md#spec-lock`.
//!
//! - **Formats** ([`LockFormat`]): this build writes format 2
//!   ([`LOCK_FORMAT`]: each entry's `levels`, `sig_hash`, `body_hash`,
//!   `deps_hash`, an item's or a module's fingerprints) and never format 1
//!   (one v2 `ast_hash` per item). A [`Lock`] knows its format; no reader
//!   upgrades one in memory.
//! - **Bytes** ([`render_lock`]): the header comment, `format`,
//!   `norm_recipe`, `code_recipe`, then one `[[binding]]` block per entry
//!   in key order, an empty line before each; keys in the order of
//!   [`ENTRY_KEYS`] (`item_kind` only for an item, the fingerprints only
//!   for an item or a module); LF and a final LF; basic strings escaping
//!   only `"`, `\` and U+0000–U+001F, U+007F. Never `toml`'s serializer.
//! - **Key** ([`EntryKey`]): `node` bytes, `relation` as listed
//!   ([`LockRelation`]), then `file`, `kind`, `symbol`, `item_kind` bytes.
//! - **Reader** ([`parse_lock`]): any TOML layout; every refusal is a
//!   [`LockError`] at its line, `spec.lock:<line>: <message>`. Without the
//!   build's recipes formats 1 and 2 are read, each by its own rules (a
//!   reader of the spec side alone); with them, format 2 at those recipes
//!   only, format 1 and another `code_recipe` refused by a text naming
//!   `spec lock rebase` (every writer and code judge). A key's strings pass
//!   [`key_problem`], the writer's check too: whatever is written reads
//!   back.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::ops::Range;

use serde::Deserialize;
use toml::Spanned;

use super::check::is_calendar_date;
use super::walk_scope::is_clean_relative;

/// The file's name, at the project root.
pub const LOCK_FILE: &str = "spec.lock";

/// The format this build writes, and the only one a writer or a code judge
/// reads ([`parse_lock`] with the build's recipes).
pub const LOCK_FORMAT: i64 = 2;

/// The step every writer and code judge names for a lock of format 1 or of
/// another `code_recipe` (task spec `lock-levels` Q11): one text, every
/// caller alike.
pub const REBASE_STEP: &str = "run spec lock rebase on a terminal";

/// The comment opening every rendered lock.
pub const LOCK_HEADER: &str = "# spec.lock: written only by SpecEngine (spec lock, spec approve, spec verify --tests); do not edit";

/// The one `verified_by` of a binding entry: a test run's evidence is not
/// an entry (slice 8b's `[[tested]]`, a later format).
pub const VERIFIED_BY: &str = "owner";

/// The prefix of a `spec_hash` and of every code digest.
pub const HASH_PREFIX: &str = "b3:";

/// The largest revision a lock holds.
pub const REV_MAX: u32 = 999_999_999;

/// The top-level keys, in render order (`binding` the blocks).
const TOP_KEYS: [&str; 4] = ["format", "norm_recipe", "code_recipe", "binding"];

/// A format-2 binding block's keys, in render order.
pub const ENTRY_KEYS: [&str; 19] = [
    "node",
    "rev",
    "spec_hash",
    "relation",
    "file",
    "kind",
    "symbol",
    "item_kind",
    "levels",
    "sig_hash",
    "body_hash",
    "deps_hash",
    "skeleton_hash",
    "quote_hash",
    "body_bytes",
    "file_blob",
    "commit",
    "verified_by",
    "verified_at",
];

/// A format-1 binding block's keys, in its render order (read only).
pub const FORMAT_1_ENTRY_KEYS: [&str; 13] = [
    "node",
    "rev",
    "spec_hash",
    "relation",
    "file",
    "kind",
    "symbol",
    "item_kind",
    "ast_hash",
    "file_blob",
    "commit",
    "verified_by",
    "verified_at",
];

/// The keys a format-1 item's entry alone has.
const FORMAT_1_ITEM_ONLY_KEYS: [&str; 2] = ["item_kind", "ast_hash"];

/// The fingerprint keys of a format-2 entry, an item's or a module's only.
const FINGERPRINT_KEYS: [&str; 3] = ["skeleton_hash", "quote_hash", "body_bytes"];

/// A lock's format: what its entries record of the code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LockFormat {
    /// Format 1 (`docs/canon/spec-lock.md` "Entry"): an item's whole v2
    /// digest (`ast_hash`), a module or a RON value by `file_blob` alone.
    /// Read where only the spec side counts; never written.
    One,
    /// Format 2: every entry's levels and their digests, an item's or a
    /// module's fingerprints. This build's ([`LOCK_FORMAT`]).
    Two,
}

impl LockFormat {
    /// The number written as `format`.
    pub const fn number(self) -> i64 {
        match self {
            Self::One => 1,
            Self::Two => 2,
        }
    }

    /// The format numbered `number`.
    pub const fn of(number: i64) -> Option<Self> {
        match number {
            1 => Some(Self::One),
            2 => Some(Self::Two),
            _ => None,
        }
    }
}

/// A level an entry records, in canonical order: the lock's own copy of
/// the four names (core reads no code crate), as in
/// `docs/canon/code-identity.md` "Marker grammar".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LockLevel {
    Path,
    Sig,
    Body,
    Deps,
}

impl LockLevel {
    /// Every level, in canonical order.
    pub const ALL: [Self; 4] = [Self::Path, Self::Sig, Self::Body, Self::Deps];

    /// The name as a level list writes it.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Path => "path",
            Self::Sig => "sig",
            Self::Body => "body",
            Self::Deps => "deps",
        }
    }

    /// The level named `name`.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|level| level.as_str() == name)
    }
}

/// An item's or a module's fingerprints: stored, never compared, never
/// drift evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fingerprints {
    /// `b3:` + 64 lowercase hex.
    pub skeleton_hash: String,
    /// `b3:` + 64 lowercase hex.
    pub quote_hash: String,
    /// 0 to `u32::MAX`, written in decimal.
    pub body_bytes: u32,
}

/// A format-2 entry's code side: the levels its markers name and every
/// level's digest, in `levels` or not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LevelCode {
    /// Non-empty; the union of the effective levels of the markers giving
    /// the key (a marker without a list: `sig`, `body`).
    pub levels: BTreeSet<LockLevel>,
    /// `b3:` + 64 lowercase hex each; a RON value's `sig` and `body` its
    /// value hash, its `deps` the empty set's digest.
    pub sig_hash: String,
    pub body_hash: String,
    pub deps_hash: String,
    /// An item's and a module's; `None` for a RON value.
    pub fingerprints: Option<Fingerprints>,
}

/// What an entry records of its code, as its lock's format records it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryCode {
    /// Format 1: an item's whole v2 digest (`ast_hash`); `None` for a
    /// module or a RON value.
    Whole(Option<String>),
    /// Format 2.
    Levels(LevelCode),
}

/// The recipes a lock's hashes were made with: `norm_recipe` (the spec
/// side's) and `code_recipe` (the code side's), as written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recipes {
    pub norm: String,
    pub code: String,
}

/// A marker's relation, in the order the key sorts them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LockRelation {
    Implements,
    Verifies,
    Configures,
    Assumes,
}

impl LockRelation {
    /// Every relation, in key order.
    pub const ALL: [Self; 4] = [
        Self::Implements,
        Self::Verifies,
        Self::Configures,
        Self::Assumes,
    ];

    /// The name a marker's keyword and the lock write.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Implements => "implements",
            Self::Verifies => "verifies",
            Self::Configures => "configures",
            Self::Assumes => "assumes",
        }
    }

    /// The relation named `name`.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|relation| relation.as_str() == name)
    }
}

/// What an entry's binding is: an item (with its hash), a module or a RON
/// value (by `file_blob` alone). Declared in byte order of the names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LockKind {
    Item,
    Module,
    Ron,
}

impl LockKind {
    /// Every kind, in byte order of the names.
    pub const ALL: [Self; 3] = [Self::Item, Self::Module, Self::Ron];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Item => "item",
            Self::Module => "module",
            Self::Ron => "ron",
        }
    }

    /// The kind named `name`.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == name)
    }
}

/// An entry's key: one per lockable marker binding (markers giving one key
/// are one entry). Ordered by `node` bytes, `relation` as listed, then
/// `file`, `kind`, `symbol`, `item_kind` bytes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EntryKey {
    /// The canonical cited ID (`slug/ID` when feature-scoped).
    pub node: String,
    pub relation: LockRelation,
    /// Root-relative, `/`-separated.
    pub file: String,
    pub kind: LockKind,
    /// An item's or a module's `qpath`; a RON value's field path.
    pub symbol: String,
    /// An item's tree-sitter kind; `None` for a module and a RON value.
    pub item_kind: Option<String>,
}

impl Ord for EntryKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.node
            .as_bytes()
            .cmp(other.node.as_bytes())
            .then_with(|| self.relation.cmp(&other.relation))
            .then_with(|| self.file.as_bytes().cmp(other.file.as_bytes()))
            .then_with(|| self.kind.as_str().cmp(other.kind.as_str()))
            .then_with(|| self.symbol.as_bytes().cmp(other.symbol.as_bytes()))
            .then_with(|| item_kind_bytes(&self.item_kind).cmp(&item_kind_bytes(&other.item_kind)))
    }
}

/// An `item_kind`'s bytes for the key order: none first, then by bytes
/// (consistent with `==`: none and an empty kind differ).
fn item_kind_bytes(kind: &Option<String>) -> Option<&[u8]> {
    kind.as_deref().map(str::as_bytes)
}

impl PartialOrd for EntryKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// What an entry vouches for: the node's revision and meaning hash, the
/// code's digests and blob, at a commit, on a date. `verified_by` is
/// always [`VERIFIED_BY`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verified {
    pub rev: u32,
    /// `b3:` + 64 lowercase hex: the node's `norm_hash`.
    pub spec_hash: String,
    /// The code side, as the lock's format records it: [`EntryCode::Levels`]
    /// in a format-2 lock, [`EntryCode::Whole`] in a format-1 one.
    pub code: EntryCode,
    /// The file's blob OID in the commit's tree (40 or 64 lowercase hex).
    pub file_blob: String,
    /// The commit's full OID.
    pub commit: String,
    /// `YYYY-MM-DD`.
    pub verified_at: String,
}

/// A whole lock: its format, its recipes and its entries by key. Every
/// entry's code is of the lock's format ([`parse_lock`] reads it so; a
/// writer gives format-2 entries to a format-2 lock only).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lock {
    pub format: LockFormat,
    pub recipes: Recipes,
    pub entries: BTreeMap<EntryKey, Verified>,
}

impl Lock {
    /// A format-2 lock with no entry.
    pub fn empty(recipes: Recipes) -> Self {
        Self {
            format: LockFormat::Two,
            recipes,
            entries: BTreeMap::new(),
        }
    }

    /// The entries of `node` (canonical ID bytes), in key order.
    pub fn entries_of<'a>(
        &'a self,
        node: &'a str,
    ) -> impl Iterator<Item = (&'a EntryKey, &'a Verified)> + 'a {
        self.entries.iter().filter(move |(key, _)| key.node == node)
    }
}

/// Why a lock cannot be read: the 1-based line, when one is known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockError {
    pub line: Option<usize>,
    pub message: String,
}

impl fmt::Display for LockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(line) => write!(f, "{LOCK_FILE}:{line}: {}", self.message),
            None => write!(f, "{LOCK_FILE}: {}", self.message),
        }
    }
}

impl std::error::Error for LockError {}

// ------------------------------------------------------------------ read

type RawTable = BTreeMap<Spanned<String>, Spanned<toml::Value>>;

/// The blocks, read apart from the top-level keys.
#[derive(Deserialize)]
struct RawBlocks {
    #[serde(default)]
    binding: Option<Vec<Spanned<RawTable>>>,
}

/// Reads a lock in any TOML layout. Refused: a `format` neither 1 nor 2;
/// an unknown, missing or repeated key; a duplicate entry key; a value of
/// the wrong form.
///
/// - `build` `None` (a reader of the spec side alone: the rule
///   `rev-not-raised`, `spec bump`, a review's vouching count, an
///   editorial approve, regen per stage): formats 1 and 2, each by its own
///   rules, recipes never compared.
/// - `Some(build)` (every writer and code judge): format 2 at the build's
///   recipes only, checked in this order, the first failing one the error
///   (task spec `lock-levels` Q27): (1) unreadable (the TOML, `format`, a
///   recipe key) or a `format` neither 1 nor 2; (2) another
///   `norm_recipe`, no step (no rebase crosses it); (3) format 1; (4)
///   another `code_recipe`; (5) the entries. (3) and (4) name
///   [`REBASE_STEP`].
pub fn parse_lock(text: &str, build: Option<&Recipes>) -> Result<Lock, LockError> {
    let at = |span: Option<Range<usize>>, message: String| LockError {
        line: span.map(|span| line_of(text, span.start)),
        message,
    };
    let top: RawTable = toml::from_str(text)
        .map_err(|error| at(error.span(), error.message().trim().to_owned()))?;
    for key in top.keys() {
        if !TOP_KEYS.contains(&key.get_ref().as_str()) {
            return Err(at(
                Some(key.span()),
                format!("unknown key `{}`", key.get_ref()),
            ));
        }
    }
    let value = |name: &str| top.get(name);
    let missing = |name: &str| LockError {
        line: Some(1),
        message: format!("missing key `{name}`"),
    };

    let format_value = value("format").ok_or_else(|| missing("format"))?;
    let format_span = format_value.span();
    let format = match format_value.get_ref() {
        toml::Value::Integer(number) => match LockFormat::of(*number) {
            Some(format) => format,
            None if *number > LOCK_FORMAT => {
                return Err(at(
                    Some(format_span),
                    format!("format {number} is newer than this build's {LOCK_FORMAT}"),
                ));
            }
            None => {
                return Err(at(
                    Some(format_span),
                    format!("format {number} is not this build's: it reads formats 1 and 2"),
                ));
            }
        },
        other => {
            return Err(at(
                Some(format_span),
                format!("`format` is a {}, not an integer", other.type_str()),
            ));
        }
    };
    let recipe = |name: &str| -> Result<(String, Range<usize>), LockError> {
        let value = value(name).ok_or_else(|| missing(name))?;
        match value.get_ref() {
            toml::Value::String(text) if !text.is_empty() => Ok((text.clone(), value.span())),
            toml::Value::String(_) => Err(at(Some(value.span()), format!("`{name}` is empty"))),
            other => Err(at(
                Some(value.span()),
                format!("`{name}` is a {}, not a string", other.type_str()),
            )),
        }
    };
    let (norm, norm_span) = recipe("norm_recipe")?;
    let (code, code_span) = recipe("code_recipe")?;
    if let Some(build) = build {
        if norm != build.norm {
            return Err(at(
                Some(norm_span),
                format!(
                    "`norm_recipe` is \"{norm}\", this build's is \"{}\": the lock was written by \
                     another build (a grammar or recipe change re-hashes every entry)",
                    build.norm
                ),
            ));
        }
        if format != LockFormat::Two {
            return Err(at(
                Some(format_span),
                format!(
                    "format {} (\"{code}\"), this build's is {LOCK_FORMAT} (\"{}\"): \
                     {REBASE_STEP}",
                    format.number(),
                    build.code
                ),
            ));
        }
        if code != build.code {
            return Err(at(
                Some(code_span),
                format!(
                    "`code_recipe` is \"{code}\", this build's is \"{}\": {REBASE_STEP}",
                    build.code
                ),
            ));
        }
    }

    let blocks: RawBlocks = toml::from_str(text)
        .map_err(|error| at(error.span(), error.message().trim().to_owned()))?;
    let mut entries = BTreeMap::new();
    for block in blocks.binding.unwrap_or_default() {
        let span = block.span();
        let (key, verified) = entry(text, format, block.get_ref(), span.clone())?;
        if entries.contains_key(&key) {
            return Err(at(
                Some(span),
                format!(
                    "a second entry for `{}` {} `{}` {} `{}`{}",
                    key.node,
                    key.relation.as_str(),
                    key.file,
                    key.kind.as_str(),
                    key.symbol,
                    key.item_kind
                        .as_deref()
                        .map(|kind| format!(" {kind}"))
                        .unwrap_or_default()
                ),
            ));
        }
        entries.insert(key, verified);
    }
    Ok(Lock {
        format,
        recipes: Recipes { norm, code },
        entries,
    })
}

/// One block of a lock of `format` read: its key and what it vouches for.
fn entry(
    text: &str,
    format: LockFormat,
    table: &RawTable,
    span: Range<usize>,
) -> Result<(EntryKey, Verified), LockError> {
    let at = |span: Range<usize>, message: String| LockError {
        line: Some(line_of(text, span.start)),
        message,
    };
    let known: &[&str] = match format {
        LockFormat::One => &FORMAT_1_ENTRY_KEYS,
        LockFormat::Two => &ENTRY_KEYS,
    };
    for key in table.keys() {
        if !known.contains(&key.get_ref().as_str()) {
            return Err(at(
                key.span(),
                format!("unknown key `{}` in a [[binding]]", key.get_ref()),
            ));
        }
    }
    let string = |name: &str| -> Result<Option<(&str, Range<usize>)>, LockError> {
        match table.get(name) {
            None => Ok(None),
            Some(value) => match value.get_ref() {
                toml::Value::String(text) => Ok(Some((text.as_str(), value.span()))),
                other => Err(at(
                    value.span(),
                    format!("`{name}` is a {}, not a string", other.type_str()),
                )),
            },
        }
    };
    let required = |name: &str| -> Result<(&str, Range<usize>), LockError> {
        string(name)?.ok_or_else(|| at(span.clone(), format!("a [[binding]] without `{name}`")))
    };

    let refused = |problem: KeyProblem, value: &str, span: Range<usize>| {
        at(span, format!("{problem}: \"{value}\""))
    };
    let (node, node_span) = required("node")?;
    if let Some(problem) = node_problem(node) {
        return Err(refused(problem, node, node_span));
    }
    let rev = match table.get("rev") {
        None => return Err(at(span.clone(), "a [[binding]] without `rev`".to_owned())),
        Some(value) => match value.get_ref() {
            toml::Value::Integer(rev) => u32::try_from(*rev)
                .ok()
                .filter(|rev| (1..=REV_MAX).contains(rev))
                .ok_or_else(|| {
                    at(
                        value.span(),
                        format!("`rev` {rev} is not a revision (1 to {REV_MAX})"),
                    )
                })?,
            other => {
                return Err(at(
                    value.span(),
                    format!("`rev` is a {}, not an integer", other.type_str()),
                ));
            }
        },
    };
    let (spec_hash, spec_span) = required("spec_hash")?;
    hash_form("spec_hash", spec_hash).map_err(|message| at(spec_span, message))?;
    let (relation, relation_span) = required("relation")?;
    let relation = LockRelation::from_name(relation).ok_or_else(|| {
        at(
            relation_span,
            format!(
                "`relation` \"{relation}\" is none of {}",
                LockRelation::ALL.map(LockRelation::as_str).join(", ")
            ),
        )
    })?;
    let (file, file_span) = required("file")?;
    if let Some(problem) = file_problem(file) {
        return Err(refused(problem, file, file_span));
    }
    let (kind, kind_span) = required("kind")?;
    let kind = LockKind::from_name(kind).ok_or_else(|| {
        at(
            kind_span,
            format!(
                "`kind` \"{kind}\" is none of {}",
                LockKind::ALL.map(LockKind::as_str).join(", ")
            ),
        )
    })?;
    let (symbol, symbol_span) = required("symbol")?;
    if let Some(problem) = symbol_problem(symbol) {
        return Err(refused(problem, symbol, symbol_span));
    }
    let item_kind = string("item_kind")?;
    let (item_kind, code) = match format {
        LockFormat::One => {
            let ast_hash = string("ast_hash")?;
            match (kind, item_kind, ast_hash) {
                (LockKind::Item, Some((item_kind, item_span)), Some((ast_hash, ast_span))) => {
                    if let Some(problem) = item_kind_problem(item_kind) {
                        return Err(refused(problem, item_kind, item_span));
                    }
                    hash_form("ast_hash", ast_hash).map_err(|message| at(ast_span, message))?;
                    (
                        Some(item_kind.to_owned()),
                        EntryCode::Whole(Some(ast_hash.to_owned())),
                    )
                }
                (LockKind::Item, item_kind, _) => {
                    let absent = if item_kind.is_none() {
                        "item_kind"
                    } else {
                        "ast_hash"
                    };
                    return Err(at(
                        span.clone(),
                        format!("an item's [[binding]] without `{absent}`"),
                    ));
                }
                (other, item_kind, ast_hash) => {
                    if let Some((_, extra)) = item_kind.or(ast_hash) {
                        return Err(at(
                            extra,
                            format!(
                                "a {} [[binding]] has no `{}`: only an item's has",
                                other.as_str(),
                                FORMAT_1_ITEM_ONLY_KEYS.join("` or `")
                            ),
                        ));
                    }
                    (None, EntryCode::Whole(None))
                }
            }
        }
        LockFormat::Two => {
            let item_kind = match (kind, item_kind) {
                (LockKind::Item, Some((item_kind, item_span))) => {
                    if let Some(problem) = item_kind_problem(item_kind) {
                        return Err(refused(problem, item_kind, item_span));
                    }
                    Some(item_kind.to_owned())
                }
                (LockKind::Item, None) => {
                    return Err(at(
                        span.clone(),
                        "an item's [[binding]] without `item_kind`".to_owned(),
                    ));
                }
                (other, Some((_, extra))) => {
                    return Err(at(
                        extra,
                        format!(
                            "a {} [[binding]] has no `item_kind`: only an item's has",
                            other.as_str()
                        ),
                    ));
                }
                (_, None) => None,
            };
            let code = level_code(text, kind, table, span.clone())?;
            (item_kind, EntryCode::Levels(code))
        }
    };
    let (file_blob, blob_span) = required("file_blob")?;
    oid_form("file_blob", file_blob).map_err(|message| at(blob_span, message))?;
    let (commit, commit_span) = required("commit")?;
    oid_form("commit", commit).map_err(|message| at(commit_span, message))?;
    let (verified_by, by_span) = required("verified_by")?;
    if verified_by != VERIFIED_BY {
        return Err(at(
            by_span,
            format!(
                "`verified_by` is \"{verified_by}\": a binding entry is the owner's \
                 (\"{VERIFIED_BY}\"); a test run's evidence is no entry"
            ),
        ));
    }
    let (verified_at, at_span) = required("verified_at")?;
    if !is_calendar_date(verified_at) {
        return Err(at(
            at_span,
            format!("`verified_at` \"{verified_at}\" is not a date (YYYY-MM-DD)"),
        ));
    }
    Ok((
        EntryKey {
            node: node.to_owned(),
            relation,
            file: file.to_owned(),
            kind,
            symbol: symbol.to_owned(),
            item_kind,
        },
        Verified {
            rev,
            spec_hash: spec_hash.to_owned(),
            code,
            file_blob: file_blob.to_owned(),
            commit: commit.to_owned(),
            verified_at: verified_at.to_owned(),
        },
    ))
}

/// A format-2 block's code side, its keys in render order: `levels`
/// (`levels: <empty|unknown <name>|duplicate <name>|order>` at its line),
/// the three digests always, the fingerprints iff `kind` is an item or a
/// module (`body_bytes` an integer 0 to `u32::MAX`).
fn level_code(
    text: &str,
    kind: LockKind,
    table: &RawTable,
    span: Range<usize>,
) -> Result<LevelCode, LockError> {
    let at = |span: Range<usize>, message: String| LockError {
        line: Some(line_of(text, span.start)),
        message,
    };
    let owner = match kind {
        LockKind::Item => "an item's",
        LockKind::Module => "a module's",
        LockKind::Ron => "a ron",
    };
    let levels = match table.get("levels") {
        None => return Err(at(span, "a [[binding]] without `levels`".to_owned())),
        Some(value) => match value.get_ref() {
            toml::Value::Array(items) => level_list(items)
                .map_err(|problem| at(value.span(), format!("levels: {problem}")))?,
            other => {
                return Err(at(
                    value.span(),
                    format!("`levels` is a {}, not an array", other.type_str()),
                ));
            }
        },
    };
    let digest = |name: &str| -> Result<String, LockError> {
        match table.get(name) {
            None => Err(at(span.clone(), format!("a [[binding]] without `{name}`"))),
            Some(value) => match value.get_ref() {
                toml::Value::String(hash) => {
                    hash_form(name, hash).map_err(|message| at(value.span(), message))?;
                    Ok(hash.clone())
                }
                other => Err(at(
                    value.span(),
                    format!("`{name}` is a {}, not a string", other.type_str()),
                )),
            },
        }
    };
    let sig_hash = digest("sig_hash")?;
    let body_hash = digest("body_hash")?;
    let deps_hash = digest("deps_hash")?;
    let fingerprints = match kind {
        LockKind::Item | LockKind::Module => {
            let without = |name: &str| {
                at(
                    span.clone(),
                    format!("{owner} [[binding]] without `{name}`"),
                )
            };
            let fingerprint = |name: &str| match table.get(name) {
                None => Err(without(name)),
                Some(_) => digest(name),
            };
            let skeleton_hash = fingerprint("skeleton_hash")?;
            let quote_hash = fingerprint("quote_hash")?;
            let body_bytes = match table.get("body_bytes") {
                None => return Err(without("body_bytes")),
                Some(value) => match value.get_ref() {
                    toml::Value::Integer(bytes) => u32::try_from(*bytes).map_err(|_| {
                        at(
                            value.span(),
                            format!("`body_bytes` {bytes} is not 0 to {}", u32::MAX),
                        )
                    })?,
                    other => {
                        return Err(at(
                            value.span(),
                            format!("`body_bytes` is a {}, not an integer", other.type_str()),
                        ));
                    }
                },
            };
            Some(Fingerprints {
                skeleton_hash,
                quote_hash,
                body_bytes,
            })
        }
        LockKind::Ron => {
            if let Some((_, extra)) = FINGERPRINT_KEYS
                .iter()
                .find_map(|name| table.get_key_value(*name))
            {
                return Err(at(
                    extra.span(),
                    format!(
                        "{owner} [[binding]] has no `{}`: only an item's or a module's has",
                        FINGERPRINT_KEYS.join("`, `")
                    ),
                ));
            }
            None
        }
    };
    Ok(LevelCode {
        levels,
        sig_hash,
        body_hash,
        deps_hash,
        fingerprints,
    })
}

/// A level list read: non-empty, each item a level name, none twice, in
/// canonical order; else the first problem of `empty`, `unknown <name>`,
/// `duplicate <name>`, `order`.
fn level_list(items: &[toml::Value]) -> Result<BTreeSet<LockLevel>, String> {
    if items.is_empty() {
        return Err("empty".to_owned());
    }
    let mut levels = Vec::with_capacity(items.len());
    for item in items {
        let level = match item {
            toml::Value::String(name) => LockLevel::from_name(name),
            _ => None,
        };
        match (level, item) {
            (Some(level), _) => levels.push(level),
            (None, toml::Value::String(name)) => return Err(format!("unknown {name}")),
            (None, toml::Value::Integer(number)) => return Err(format!("unknown {number}")),
            (None, toml::Value::Float(number)) => return Err(format!("unknown {number}")),
            (None, toml::Value::Boolean(flag)) => return Err(format!("unknown {flag}")),
            (None, other) => return Err(format!("unknown {}", other.type_str())),
        }
    }
    if let Some(twice) = levels
        .iter()
        .enumerate()
        .find_map(|(at, level)| levels[..at].contains(level).then_some(*level))
    {
        return Err(format!("duplicate {}", twice.as_str()));
    }
    if levels.windows(2).any(|pair| pair[0] > pair[1]) {
        return Err("order".to_owned());
    }
    Ok(levels.into_iter().collect())
}

/// Why a key's strings cannot be written and read back: the field and the
/// reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyProblem {
    pub field: &'static str,
    pub reason: &'static str,
}

impl fmt::Display for KeyProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "`{}` {}", self.field, self.reason)
    }
}

/// The one check of a key's strings, the writer's and the reader's alike:
/// a key it refuses is never written (the binding is not locked), and the
/// reader refuses each such field at its line, so whatever is written reads
/// back. `node` and `item_kind`: not empty, no control character; `file`:
/// a clean root-relative `/` path, no `\`, no control character; `symbol`:
/// not empty, any other string (a `qpath` or a field path is opaque, the
/// render escapes what TOML needs); `item_kind` for an item and only for
/// one.
pub fn key_problem(key: &EntryKey) -> Option<KeyProblem> {
    node_problem(&key.node)
        .or_else(|| file_problem(&key.file))
        .or_else(|| symbol_problem(&key.symbol))
        .or_else(|| match (key.kind, &key.item_kind) {
            (LockKind::Item, Some(item_kind)) => item_kind_problem(item_kind),
            (LockKind::Item, None) => Some(KeyProblem {
                field: "item_kind",
                reason: "is missing: an item's key has one",
            }),
            (_, Some(_)) => Some(KeyProblem {
                field: "item_kind",
                reason: "is only an item's",
            }),
            (_, None) => None,
        })
}

fn node_problem(node: &str) -> Option<KeyProblem> {
    (node.is_empty() || node.chars().any(char::is_control)).then_some(KeyProblem {
        field: "node",
        reason: "is empty or holds a control character",
    })
}

fn file_problem(file: &str) -> Option<KeyProblem> {
    (!is_clean_relative(file) || file.contains('\\') || file.chars().any(char::is_control))
        .then_some(KeyProblem {
            field: "file",
            reason: "is not a clean root-relative `/` path (no `\\`, no control character)",
        })
}

fn symbol_problem(symbol: &str) -> Option<KeyProblem> {
    symbol.is_empty().then_some(KeyProblem {
        field: "symbol",
        reason: "is empty",
    })
}

fn item_kind_problem(item_kind: &str) -> Option<KeyProblem> {
    (item_kind.is_empty() || item_kind.chars().any(char::is_control)).then_some(KeyProblem {
        field: "item_kind",
        reason: "is empty or holds a control character",
    })
}

/// `b3:` and 64 lowercase hex digits.
fn hash_form(name: &str, value: &str) -> Result<(), String> {
    match value.strip_prefix(HASH_PREFIX) {
        Some(hex) if hex.len() == 64 && is_lower_hex(hex) => Ok(()),
        _ => Err(format!(
            "`{name}` \"{value}\" is not `{HASH_PREFIX}` and 64 lowercase hex digits"
        )),
    }
}

/// A full object ID: 40 (SHA-1) or 64 (SHA-256) lowercase hex digits.
fn oid_form(name: &str, value: &str) -> Result<(), String> {
    if matches!(value.len(), 40 | 64) && is_lower_hex(value) {
        Ok(())
    } else {
        Err(format!(
            "`{name}` \"{value}\" is not a full object ID (40 or 64 lowercase hex digits)"
        ))
    }
}

fn is_lower_hex(text: &str) -> bool {
    text.bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn line_of(text: &str, offset: usize) -> usize {
    let end = offset.min(text.len());
    text.as_bytes()[..end]
        .iter()
        .filter(|&&byte| byte == b'\n')
        .count()
        + 1
}

// ---------------------------------------------------------------- render

/// The lock's canonical bytes, of its own format. A writer renders a
/// format-2 lock only: one reading a format-1 lock refuses before it
/// renders (task spec `lock-levels` "Readers").
pub fn render_lock(lock: &Lock) -> String {
    let mut out = String::new();
    out.push_str(LOCK_HEADER);
    out.push('\n');
    out.push_str(&format!("format = {}\n", lock.format.number()));
    out.push_str(&format!("norm_recipe = {}\n", basic(&lock.recipes.norm)));
    out.push_str(&format!("code_recipe = {}\n", basic(&lock.recipes.code)));
    for (key, verified) in &lock.entries {
        debug_assert!(
            matches!(
                (lock.format, &verified.code),
                (LockFormat::One, EntryCode::Whole(_)) | (LockFormat::Two, EntryCode::Levels(_))
            ),
            "an entry of another format than its lock's"
        );
        out.push('\n');
        out.push_str(&render_block(key, verified));
    }
    out
}

/// One entry's `[[binding]]` block, its last line ended: the keys of
/// [`ENTRY_KEYS`] for a format-2 entry, of [`FORMAT_1_ENTRY_KEYS`] for a
/// format-1 one. What it writes reads back: its kind's keys as its format
/// holds them (format 1: `ast_hash` iff `item`; format 2: fingerprints iff
/// `item` or `module`), `levels` never empty.
pub fn render_block(key: &EntryKey, verified: &Verified) -> String {
    match &verified.code {
        EntryCode::Whole(ast_hash) => debug_assert_eq!(
            ast_hash.is_some(),
            key.kind == LockKind::Item,
            "a format-1 entry's `ast_hash` is an item's alone"
        ),
        EntryCode::Levels(code) => {
            debug_assert!(!code.levels.is_empty(), "a format-2 entry with no level");
            debug_assert_eq!(
                code.fingerprints.is_some(),
                key.kind != LockKind::Ron,
                "a format-2 entry's fingerprints are an item's or a module's alone"
            );
        }
    }
    let mut out = String::from("[[binding]]\n");
    let mut line = |name: &str, value: String| {
        out.push_str(name);
        out.push_str(" = ");
        out.push_str(&value);
        out.push('\n');
    };
    line("node", basic(&key.node));
    line("rev", verified.rev.to_string());
    line("spec_hash", basic(&verified.spec_hash));
    line("relation", basic(key.relation.as_str()));
    line("file", basic(&key.file));
    line("kind", basic(key.kind.as_str()));
    line("symbol", basic(&key.symbol));
    if let Some(item_kind) = &key.item_kind {
        line("item_kind", basic(item_kind));
    }
    match &verified.code {
        EntryCode::Whole(ast_hash) => {
            if let Some(ast_hash) = ast_hash {
                line("ast_hash", basic(ast_hash));
            }
        }
        EntryCode::Levels(code) => {
            for (name, value) in level_lines(code) {
                line(name, value);
            }
            if let Some(fingerprints) = &code.fingerprints {
                line("skeleton_hash", basic(&fingerprints.skeleton_hash));
                line("quote_hash", basic(&fingerprints.quote_hash));
                line("body_bytes", fingerprints.body_bytes.to_string());
            }
        }
    }
    line("file_blob", basic(&verified.file_blob));
    line("commit", basic(&verified.commit));
    line("verified_by", basic(VERIFIED_BY));
    line("verified_at", basic(&verified.verified_at));
    out
}

/// The `levels` and three digest lines of a format-2 entry, name and
/// rendered value, as [`render_block`] writes them.
fn level_lines(code: &LevelCode) -> [(&'static str, String); 4] {
    let levels: Vec<String> = code
        .levels
        .iter()
        .map(|level| basic(level.as_str()))
        .collect();
    [
        ("levels", format!("[{}]", levels.join(", "))),
        ("sig_hash", basic(&code.sig_hash)),
        ("body_hash", basic(&code.body_hash)),
        ("deps_hash", basic(&code.deps_hash)),
    ]
}

/// An entry's code as a `drift_hash` covers it (core
/// `drift::HashedEntry::code`; task spec `lock-levels` "Drift evidence";
/// `docs/canon/proposal-drift-item.md` "The item"): its `levels`,
/// `sig_hash`, `body_hash` and `deps_hash` lines as [`render_lock`] writes
/// them, LF-joined, no final LF, never a fingerprint or `body_bytes`
/// (tuning one re-keys no item). A format-1 entry, which no code judge
/// reads: its `ast_hash` line as rendered (empty for a module or a RON
/// value).
pub fn lock_code(verified: &Verified) -> String {
    match &verified.code {
        EntryCode::Whole(ast_hash) => ast_hash
            .as_deref()
            .map(|ast_hash| format!("ast_hash = {}", basic(ast_hash)))
            .unwrap_or_default(),
        EntryCode::Levels(code) => level_lines(code)
            .map(|(name, value)| format!("{name} = {value}"))
            .join("\n"),
    }
}

/// A TOML basic string: `"`, `\` and U+0000–U+001F, U+007F escaped
/// (`\b \t \n \f \r`, else `\u00XX`), everything else as it is.
fn basic(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\u{c}' => out.push_str("\\f"),
            '\r' => out.push_str("\\r"),
            c if c <= '\u{1f}' || c == '\u{7f}' => {
                out.push_str(&format!("\\u{:04X}", u32::from(c)));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

// ---------------------------------------------------------------- accept

/// What an accept did to one key of the node, or to a binding it could
/// not lock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Change {
    /// Lockable now, no old entry: added.
    New,
    /// Lockable now, the old entry differs on `rev`, `spec_hash`,
    /// `levels`, a level's digest or `file_blob`: replaced.
    Refreshed,
    /// Lockable now and equal on all of them: the old entry kept byte for
    /// byte (`commit`, `verified_at`, the fingerprints too).
    Unchanged,
    /// Bound now but not lockable: the old entry kept.
    Kept,
    /// Bound no more: the old entry dropped.
    Dropped,
    /// Bound now, not lockable, no old entry.
    NotLocked,
}

impl Change {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Refreshed => "refreshed",
            Self::Unchanged => "unchanged",
            Self::Kept => "kept",
            Self::Dropped => "dropped",
            Self::NotLocked => "not locked",
        }
    }

    /// The lock's bytes change.
    pub const fn writes(self) -> bool {
        matches!(self, Self::New | Self::Refreshed | Self::Dropped)
    }
}

/// A binding as a row shows it: its relation, file, kind and symbol (an
/// unlocked binding's own words: `unanchored in_body`), and its
/// `item_kind`. Ordered as an entry key of one node.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Place {
    pub relation: LockRelation,
    pub file: String,
    pub kind: String,
    pub symbol: String,
    pub item_kind: Option<String>,
}

impl Place {
    /// The place of an entry's key.
    pub fn of(key: &EntryKey) -> Self {
        Self {
            relation: key.relation,
            file: key.file.clone(),
            kind: key.kind.as_str().to_owned(),
            symbol: key.symbol.clone(),
            item_kind: key.item_kind.clone(),
        }
    }
}

impl Ord for Place {
    fn cmp(&self, other: &Self) -> Ordering {
        self.relation
            .cmp(&other.relation)
            .then_with(|| self.file.as_bytes().cmp(other.file.as_bytes()))
            .then_with(|| self.kind.as_bytes().cmp(other.kind.as_bytes()))
            .then_with(|| self.symbol.as_bytes().cmp(other.symbol.as_bytes()))
            .then_with(|| item_kind_bytes(&self.item_kind).cmp(&item_kind_bytes(&other.item_kind)))
    }
}

impl PartialOrd for Place {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// One row of an accept: what happened at a place, and why it is not
/// locked (`kept`, `not locked`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptRow {
    pub change: Change,
    pub place: Place,
    pub state: Option<String>,
}

/// What an accept found for one node: its revision and `norm_hash` at the
/// commit, and its marker bindings by what they allow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptInput {
    /// The canonical cited ID.
    pub node: String,
    pub rev: u32,
    pub spec_hash: String,
    /// Lockable now, one per key, each of `node`.
    pub fresh: Vec<(EntryKey, Verified)>,
    /// Bound now with a key, not lockable: the state of each. An old entry
    /// of such a key is kept (an item now with a parse error included: the
    /// caller names it by its key).
    pub unlockable: Vec<(EntryKey, String)>,
    /// Bound now with no key (unanchored, `cannot_verify`): its place (its
    /// own words as kind and symbol) and state; a `not locked` row each.
    pub keyless: Vec<(Place, String)>,
}

/// Why an accept merges nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcceptRefusal {
    /// An old entry at the node's revision vouches for another
    /// `spec_hash`: the text changed without a raise.
    TextChanged {
        key: Box<EntryKey>,
        locked: String,
        current: String,
    },
}

/// An accept's merge: the new lock, every row in key order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptMerge {
    pub lock: Lock,
    pub rows: Vec<AcceptRow>,
}

impl AcceptMerge {
    /// Rows with `change`.
    pub fn count(&self, change: Change) -> usize {
        self.rows.iter().filter(|row| row.change == change).count()
    }

    /// The lock's bytes change.
    pub fn writes(&self) -> bool {
        self.rows.iter().any(|row| row.change.writes())
    }
}

/// Merges an accept into `old`'s entries of the node (step 6): a lockable
/// binding fresh, or the old entry kept byte for byte when the same pair
/// (`rev`, `spec_hash`, `levels`, the three digests, `file_blob`; never a
/// fingerprint); one not lockable now keeps its old entry; one bound no
/// more is dropped. Refused: an old entry at the revision with another
/// `spec_hash`. Other nodes' entries are untouched; the lock keeps `old`'s
/// format, which a writer reads as format 2 (`parse_lock` with the build's
/// recipes), so the fresh entries are of it.
pub fn merge_accept(old: &Lock, input: &AcceptInput) -> Result<AcceptMerge, AcceptRefusal> {
    if let Some((key, verified)) = old
        .entries_of(&input.node)
        .find(|(_, verified)| verified.rev == input.rev && verified.spec_hash != input.spec_hash)
    {
        return Err(AcceptRefusal::TextChanged {
            key: Box::new(key.clone()),
            locked: verified.spec_hash.clone(),
            current: input.spec_hash.clone(),
        });
    }
    let mut lock = old.clone();
    let mut rows = Vec::new();
    let fresh_keys: BTreeSet<&EntryKey> = input.fresh.iter().map(|(key, _)| key).collect();
    for (key, verified) in &input.fresh {
        let change = match old.entries.get(key) {
            None => Change::New,
            Some(held) if same_pair(held, verified) => Change::Unchanged,
            Some(_) => Change::Refreshed,
        };
        if change != Change::Unchanged {
            lock.entries.insert(key.clone(), verified.clone());
        }
        rows.push(AcceptRow {
            change,
            place: Place::of(key),
            state: None,
        });
    }
    let unlockable: BTreeMap<&EntryKey, &String> = input
        .unlockable
        .iter()
        .map(|(key, state)| (key, state))
        .collect();
    for (key, _) in old.entries_of(&input.node) {
        if fresh_keys.contains(key) {
            continue;
        }
        let (change, state) = match unlockable.get(key) {
            Some(state) => (Change::Kept, Some((*state).clone())),
            None => (Change::Dropped, None),
        };
        if change == Change::Dropped {
            lock.entries.remove(key);
        }
        rows.push(AcceptRow {
            change,
            place: Place::of(key),
            state,
        });
    }
    for (key, state) in &input.unlockable {
        if !old.entries.contains_key(key) {
            rows.push(AcceptRow {
                change: Change::NotLocked,
                place: Place::of(key),
                state: Some(state.clone()),
            });
        }
    }
    for (place, state) in &input.keyless {
        rows.push(AcceptRow {
            change: Change::NotLocked,
            place: place.clone(),
            state: Some(state.clone()),
        });
    }
    rows.sort_by(|a, b| a.place.cmp(&b.place).then(a.change.cmp(&b.change)));
    Ok(AcceptMerge { lock, rows })
}

/// Equal on `rev`, `spec_hash`, the judged code and `file_blob`: format
/// 2's `levels`, `sig_hash`, `body_hash`, `deps_hash`, never a fingerprint
/// or `body_bytes` (task spec `lock-levels` Q12); format 1's `ast_hash`.
fn same_pair(held: &Verified, now: &Verified) -> bool {
    let same_code = match (&held.code, &now.code) {
        (EntryCode::Levels(held), EntryCode::Levels(now)) => {
            held.levels == now.levels
                && held.sig_hash == now.sig_hash
                && held.body_hash == now.body_hash
                && held.deps_hash == now.deps_hash
        }
        (EntryCode::Whole(held), EntryCode::Whole(now)) => held == now,
        _ => false,
    };
    held.rev == now.rev
        && held.spec_hash == now.spec_hash
        && same_code
        && held.file_blob == now.file_blob
}

/// The one vouching predicate (`docs/canon/node-revision.md` "Raising"):
/// the entry under `key` is one of `node` (canonical ID bytes) at `rev`
/// whose `spec_hash` is `norm_hash`, so it vouches for that text at that
/// revision. An entry at `rev` with another `spec_hash` (an earlier
/// unreviewed change) vouches for nothing.
pub fn vouches(key: &EntryKey, verified: &Verified, node: &str, rev: u32, norm_hash: &str) -> bool {
    key.node == node && verified.rev == rev && verified.spec_hash == norm_hash
}

/// An editorial refresh: the new lock and the node's entries at the
/// revision, each with whether its `spec_hash` changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Editorial {
    pub lock: Lock,
    /// The node's entries at the revision (with a filter, those vouching
    /// for the old text only), in key order: `true` when its `spec_hash`
    /// was set.
    pub at_rev: Vec<(EntryKey, bool)>,
}

impl Editorial {
    /// Entries whose `spec_hash` was set.
    pub fn refreshed(&self) -> usize {
        self.at_rev.iter().filter(|(_, changed)| *changed).count()
    }
}

/// `--editorial`: `spec_hash` set to `spec_hash` on the node's entries at
/// `rev`, nothing else (code hashes, `file_blob`, `commit`, `verified_*`
/// kept). `vouching`, the node's old `norm_hash`, filters them: only the
/// entries that vouched for the old text ([`vouches`]) are set, any other
/// at `rev` is left byte for byte and not listed (`spec approve
/// --editorial`, task spec `rev-discipline` Q10); `None`: every entry at
/// `rev` (`spec lock accept --editorial`).
pub fn refresh_editorial(
    old: &Lock,
    node: &str,
    rev: u32,
    spec_hash: &str,
    vouching: Option<&str>,
) -> Editorial {
    let mut lock = old.clone();
    let mut at_rev = Vec::new();
    for (key, verified) in lock.entries.iter_mut() {
        if key.node != node || verified.rev != rev {
            continue;
        }
        if let Some(old_hash) = vouching
            && !vouches(key, verified, node, rev, old_hash)
        {
            continue;
        }
        let changed = verified.spec_hash != spec_hash;
        if changed {
            verified.spec_hash = spec_hash.to_owned();
        }
        at_rev.push((key.clone(), changed));
    }
    Editorial { lock, at_rev }
}

/// A gone node's entries dropped: the new lock and the dropped keys.
pub fn drop_node(old: &Lock, node: &str) -> (Lock, Vec<EntryKey>) {
    let mut lock = old.clone();
    let dropped: Vec<EntryKey> = old.entries_of(node).map(|(key, _)| key.clone()).collect();
    for key in &dropped {
        lock.entries.remove(key);
    }
    (lock, dropped)
}

// ---------------------------------------------------------------- rebase

/// Why `spec lock rebase` drops an entry (task spec `lock-rebase`): the
/// first that applies, in this order. A dropped entry is named, never
/// guessed; the binding then reads `unlocked`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DropReason {
    /// No object of the entry's `file_blob` (gc, a partial or shallow
    /// clone), or an object of another type.
    BlobMissing,
    /// The blob is not UTF-8.
    NotUtf8,
    /// No unit of the blob is a candidate for the key.
    SymbolGone,
    /// The ties leave none or several candidates; an ambiguous RON anchor.
    Ambiguous,
    /// A format-1 entry whose key a marker with a malformed level list
    /// gives: never the default levels.
    BadLevels,
    /// The located unit gives no digests: its first category, if any.
    CannotVerify(Option<String>),
    /// A format-1 item's whole digest is not made again from its blob.
    HashMismatch,
}

impl DropReason {
    /// The reason as a row and `--json` name it: `blob_missing`,
    /// `not_utf8`, `symbol_gone`, `ambiguous`, `bad_levels`,
    /// `cannot_verify[ <category>]`, `hash_mismatch`.
    pub fn text(&self) -> String {
        match self {
            Self::BlobMissing => "blob_missing".to_owned(),
            Self::NotUtf8 => "not_utf8".to_owned(),
            Self::SymbolGone => "symbol_gone".to_owned(),
            Self::Ambiguous => "ambiguous".to_owned(),
            Self::BadLevels => "bad_levels".to_owned(),
            Self::CannotVerify(Some(category)) => format!("cannot_verify {category}"),
            Self::CannotVerify(None) => "cannot_verify".to_owned(),
            Self::HashMismatch => "hash_mismatch".to_owned(),
        }
    }
}

/// One entry re-derived from its own blob: its code at the build's
/// recipes, or why it is dropped. A kept entry takes a [`LevelCode`], so
/// no format-1 code reaches a rebased lock.
pub type Rederived = Result<LevelCode, DropReason>;

/// What a rebase did to one entry: the levels written, or why it is
/// dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RebaseRow {
    pub key: EntryKey,
    pub result: Result<BTreeSet<LockLevel>, DropReason>,
}

/// A rebase: the new lock and a row per old entry, in key order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RebasePlan {
    /// Format 2 at the build's recipes, every kept entry's key, `rev`,
    /// `spec_hash`, `file_blob`, `commit` and `verified_at` as they were.
    pub lock: Lock,
    pub rows: Vec<RebaseRow>,
}

impl RebasePlan {
    /// Entries kept, their code re-derived.
    pub fn rebased(&self) -> usize {
        self.rows.iter().filter(|row| row.result.is_ok()).count()
    }

    /// Entries dropped.
    pub fn dropped(&self) -> usize {
        self.rows.iter().filter(|row| row.result.is_err()).count()
    }
}

/// `spec lock rebase`'s plan (ADR-0050): `old` (any format and
/// `code_recipe`; the caller has refused another `norm_recipe`) carried
/// into format 2 at `build`, each entry in key order given its code by
/// `rederive` (from its own `file_blob`, never another tree) or dropped
/// with its reason. Kept byte for byte: the key, `rev`, `spec_hash`,
/// `file_blob`, `commit`, `verified_at`; written: the levels, the three
/// digests, the fingerprints. The first error of `rederive` ends the plan.
pub fn plan_rebase<E>(
    old: &Lock,
    build: &Recipes,
    mut rederive: impl FnMut(&EntryKey, &Verified) -> Result<Rederived, E>,
) -> Result<RebasePlan, E> {
    let mut lock = Lock::empty(build.clone());
    let mut rows = Vec::with_capacity(old.entries.len());
    for (key, verified) in &old.entries {
        let result = match rederive(key, verified)? {
            Ok(code) => {
                let levels = code.levels.clone();
                lock.entries.insert(
                    key.clone(),
                    Verified {
                        rev: verified.rev,
                        spec_hash: verified.spec_hash.clone(),
                        code: EntryCode::Levels(code),
                        file_blob: verified.file_blob.clone(),
                        commit: verified.commit.clone(),
                        verified_at: verified.verified_at.clone(),
                    },
                );
                Ok(levels)
            }
            Err(reason) => Err(reason),
        };
        rows.push(RebaseRow {
            key: key.clone(),
            result,
        });
    }
    Ok(RebasePlan { lock, rows })
}

// ----------------------------------------------------------------- regen

/// How `spec lock regen` chose an entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RegenChoice {
    /// The same on both sides.
    Both,
    /// Changed (or added) on our side only, or deleted on theirs and
    /// changed on ours.
    Ours,
    /// The same for their side.
    Theirs,
    /// Deleted on one side, unchanged on the other (or on both).
    Deleted,
    /// Changed differently: the later `verified_at`.
    Later,
    /// Changed differently on the same date: the block first by bytes.
    Bytes,
}

impl RegenChoice {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Both => "both",
            Self::Ours => "ours",
            Self::Theirs => "theirs",
            Self::Deleted => "deleted",
            Self::Later => "later",
            Self::Bytes => "bytes",
        }
    }
}

/// A merge of a conflict's stages: the lock and each key's choice, in key
/// order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Regen {
    pub lock: Lock,
    pub rows: Vec<(EntryKey, RegenChoice)>,
}

impl Regen {
    /// Keys chosen by `verified_at` or bytes.
    pub fn by_date_or_bytes(&self) -> usize {
        self.rows
            .iter()
            .filter(|(_, choice)| matches!(choice, RegenChoice::Later | RegenChoice::Bytes))
            .count()
    }
}

/// Why stages do not merge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegenRefusal {
    /// No stage of ours or theirs.
    NoSides,
    /// Two stages name different `norm_recipe`s: no rebase crosses it.
    RecipesDiffer { first: Recipes, second: Recipes },
    /// A stage is format 1, or two stages name different `code_recipe`s:
    /// the lagging branch is rebased before the merge (task spec
    /// `lock-levels` Q28).
    Unrebased,
}

/// Merges the stages of a conflict per entry key (`base` absent: an
/// add/add; a side absent: every entry deleted there). Side-symmetric:
/// swapping `ours` and `theirs` gives the same lock. Reads no code, no
/// spec, computes no hash: every entry kept is one an owner accepted.
/// Every stage present is format 2 of one pair of recipes (the build's not
/// required), the `norm_recipe` compared first; the merged lock is of
/// them.
pub fn merge_stages(
    base: Option<&Lock>,
    ours: Option<&Lock>,
    theirs: Option<&Lock>,
) -> Result<Regen, RegenRefusal> {
    let present: Vec<&Lock> = [base, ours, theirs].into_iter().flatten().collect();
    if ours.is_none() && theirs.is_none() {
        return Err(RegenRefusal::NoSides);
    }
    let recipes = present
        .first()
        .map(|lock| lock.recipes.clone())
        .ok_or(RegenRefusal::NoSides)?;
    if let Some(other) = present
        .iter()
        .find(|lock| lock.recipes.norm != recipes.norm)
    {
        return Err(RegenRefusal::RecipesDiffer {
            first: recipes,
            second: other.recipes.clone(),
        });
    }
    if present
        .iter()
        .any(|lock| lock.format != LockFormat::Two || lock.recipes.code != recipes.code)
    {
        return Err(RegenRefusal::Unrebased);
    }
    let none = BTreeMap::new();
    let base = base.map_or(&none, |lock| &lock.entries);
    let ours = ours.map_or(&none, |lock| &lock.entries);
    let theirs = theirs.map_or(&none, |lock| &lock.entries);
    let keys: BTreeSet<&EntryKey> = base
        .keys()
        .chain(ours.keys())
        .chain(theirs.keys())
        .collect();
    let mut lock = Lock::empty(recipes);
    let mut rows = Vec::new();
    for key in keys {
        let (b, o, t) = (base.get(key), ours.get(key), theirs.get(key));
        let (choice, kept) = match (o, t) {
            (Some(o), Some(t)) if o == t => (RegenChoice::Both, Some(o)),
            (None, None) => (RegenChoice::Deleted, None),
            (o, t) if o == b => match t {
                Some(t) => (RegenChoice::Theirs, Some(t)),
                None => (RegenChoice::Deleted, None),
            },
            (o, t) if t == b => match o {
                Some(o) => (RegenChoice::Ours, Some(o)),
                None => (RegenChoice::Deleted, None),
            },
            (Some(o), None) => (RegenChoice::Ours, Some(o)),
            (None, Some(t)) => (RegenChoice::Theirs, Some(t)),
            (Some(o), Some(t)) => match o.verified_at.cmp(&t.verified_at) {
                Ordering::Greater => (RegenChoice::Later, Some(o)),
                Ordering::Less => (RegenChoice::Later, Some(t)),
                Ordering::Equal => {
                    let first = if render_block(key, o) <= render_block(key, t) {
                        o
                    } else {
                        t
                    };
                    (RegenChoice::Bytes, Some(first))
                }
            },
        };
        if let Some(kept) = kept {
            lock.entries.insert(key.clone(), kept.clone());
        }
        rows.push((key.clone(), choice));
    }
    Ok(Regen { lock, rows })
}
