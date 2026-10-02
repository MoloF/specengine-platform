//! `CheckConfig::from_toml`: the `[budgets]`, `[classes]`, `[check]` and
//! `[[generators]]` tables of `specengine.toml`, and only those
//! (docs/canon/spec-check.md, "Configuration"). Pure and strict: an unknown
//! key or class, a wrong type, a cap below 1, an unknown mode or an invalid
//! generator entry is an error `file:line: message` through
//! [`ConfigError::at`]; other tables are ignored (`[paths] index` is read
//! only to cross-check the index generator), so editing these tables leaves
//! the `[ids]` fingerprint (and every stored parse) alone.

use std::fmt;
use std::ops::Range;

use serde::{Deserialize, Serialize};
use toml::Spanned;

/// Default document caps in bytes of the whole file, front-matter and BOM
/// included: the documentation convention's budgets (tier 0, tier 1, the
/// index, a decision record). Tier 2 canon has no default cap.
pub const DEFAULT_TIER0_BYTES: u64 = 16 * 1024;
pub const DEFAULT_TIER1_BYTES: u64 = 10 * 1024;
pub const DEFAULT_INDEX_BYTES: u64 = 10 * 1024;
pub const DEFAULT_DECISION_BYTES: u64 = 1536;

/// The `[budgets]` table: document caps in bytes (unit in the key name);
/// the bundle budgets are tokens, read and carried only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Budgets {
    /// Canon tier 0.
    pub tier0_bytes: u64,
    /// Every canon tier 1.
    pub tier1_bytes: u64,
    /// The file named by `[paths] index`, whatever its class.
    pub index_bytes: u64,
    /// Every decision.
    pub decision_bytes: u64,
    /// Any other canon; `None`: uncapped.
    pub canon_bytes: Option<u64>,
    /// Tokens of a node bundle (07 §5); not checked here.
    pub bundle_node: Option<u64>,
    /// Tokens of a task bundle (07 §5); not checked here.
    pub bundle_task: Option<u64>,
}

impl Default for Budgets {
    fn default() -> Self {
        Self {
            tier0_bytes: DEFAULT_TIER0_BYTES,
            tier1_bytes: DEFAULT_TIER1_BYTES,
            index_bytes: DEFAULT_INDEX_BYTES,
            decision_bytes: DEFAULT_DECISION_BYTES,
            canon_bytes: None,
            bundle_node: None,
            bundle_task: None,
        }
    }
}

/// The four document classes of the convention.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DocClass {
    Canon,
    Decision,
    Spec,
    Generated,
}

impl DocClass {
    pub const ALL: [DocClass; 4] = [Self::Canon, Self::Decision, Self::Spec, Self::Generated];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Canon => "canon",
            Self::Decision => "decision",
            Self::Spec => "spec",
            Self::Generated => "generated",
        }
    }

    /// The class named exactly `name`.
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|class| class.as_str() == name)
    }
}

impl fmt::Display for DocClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The front-matter keys a class requires and allows. `class` itself is
/// always allowed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassContract {
    /// Keys that must be present (`key-missing`).
    pub required: Vec<String>,
    /// Further keys a closed contract allows.
    pub optional: Vec<String>,
    /// No key outside `required` and `optional` (`key-extra`); an open
    /// contract leaves untyped keys to the parser's `unknown-key`.
    pub closed: bool,
}

impl ClassContract {
    /// The default contract: the keys the convention makes load-bearing
    /// (`scope` routes, `status` excludes; canon minimal), plus `class`;
    /// open.
    pub fn default_for(class: DocClass) -> Self {
        let required: &[&str] = match class {
            DocClass::Canon => &["class", "owner", "reviewed"],
            DocClass::Decision => &["class", "id", "status", "scope"],
            DocClass::Spec => &["class", "status", "scope"],
            DocClass::Generated => &["class"],
        };
        Self {
            required: required.iter().map(|key| (*key).to_owned()).collect(),
            optional: Vec::new(),
            closed: false,
        }
    }

    /// `key` may appear under this contract.
    pub fn allows(&self, key: &str) -> bool {
        !self.closed
            || key == "class"
            || self.required.iter().any(|allowed| allowed == key)
            || self.optional.iter().any(|allowed| allowed == key)
    }
}

/// One contract per class; a `[classes]` entry replaces that class's
/// default whole.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Classes {
    pub canon: ClassContract,
    pub decision: ClassContract,
    pub spec: ClassContract,
    pub generated: ClassContract,
}

impl Default for Classes {
    fn default() -> Self {
        Self {
            canon: ClassContract::default_for(DocClass::Canon),
            decision: ClassContract::default_for(DocClass::Decision),
            spec: ClassContract::default_for(DocClass::Spec),
            generated: ClassContract::default_for(DocClass::Generated),
        }
    }
}

impl Classes {
    pub fn get(&self, class: DocClass) -> &ClassContract {
        match class {
            DocClass::Canon => &self.canon,
            DocClass::Decision => &self.decision,
            DocClass::Spec => &self.spec,
            DocClass::Generated => &self.generated,
        }
    }
}

/// What blocks, a ladder in declared order (`Ord`): `observe` counts and
/// never blocks; `enforce-introduced` blocks on what a commit adds against
/// its base (introduced errors not in debt, expired debt, new debt);
/// `enforce` blocks on every error not in live debt (and, with a base, on
/// new debt). Without a base `enforce-introduced` judges as `enforce`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    Observe,
    EnforceIntroduced,
    #[default]
    Enforce,
}

impl Mode {
    /// Every mode, in ladder order.
    pub const ALL: [Self; 3] = [Self::Observe, Self::EnforceIntroduced, Self::Enforce];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Observe => "observe",
            Self::EnforceIntroduced => "enforce-introduced",
            Self::Enforce => "enforce",
        }
    }
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// `[budgets]`, `[classes]`, `[check]` and `[[generators]]`, defaults
/// applied.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckConfig {
    pub budgets: Budgets,
    pub classes: Classes,
    pub mode: Mode,
    /// The generator registry, in the order written; `None` without a
    /// `[[generators]]` table: the generator rules are off.
    pub generators: Option<Vec<Generator>>,
}

impl CheckConfig {
    /// Reads the four tables; absent tables give the defaults.
    pub fn from_toml(text: &str) -> Result<Self, ConfigError> {
        check_config_from_toml(text)
    }

    /// The entry with `index = true`: SpecEngine renders its output itself
    /// and compares it with the walked `[paths] index`.
    pub fn index_generator(&self) -> Option<&Generator> {
        self.generators
            .as_deref()
            .and_then(|generators| generators.iter().find(|generator| generator.index))
    }
}

/// The default gate named in the index header: the check itself.
pub const DEFAULT_GATE: &str = "spec check";

/// One `[[generators]]` entry: a command that writes generated documents.
/// Registered only: the check never runs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Generator {
    /// Compared byte for byte with a generated document's `generator:`.
    pub command: String,
    /// The root-relative files the command writes (the `[paths]` path
    /// rules), in the order written, repeats dropped.
    pub writes: Vec<String>,
    /// SpecEngine renders this output itself (the index).
    pub index: bool,
    /// The gate the index header names; only with `index = true`.
    pub gate: Option<String>,
    /// The shards of the index (ADR-0030), in config order; only with
    /// `index = true`. Empty: no shard (also `shards = []`), the index is
    /// the one file at `[paths] index`.
    pub shards: Vec<Shard>,
    /// The 1-based line of the entry.
    pub line: usize,
}

impl Generator {
    /// The gate the index header names: `gate`, else [`DEFAULT_GATE`].
    pub fn gate(&self) -> &str {
        self.gate.as_deref().unwrap_or(DEFAULT_GATE)
    }
}

/// One shard of the index (ADR-0030): a file the `index = true` entry
/// writes beside `[paths] index`, listed in its `writes` too, holding the
/// documents its kind places there; the root points to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shard {
    /// Root-relative, under the `[paths]` path rules.
    pub path: String,
    pub kind: ShardKind,
    /// The 1-based line of the shard.
    pub line: usize,
}

/// What a shard holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShardKind {
    /// `tier3 = true`: the archive shard, every Tier 3 document; uncapped
    /// and outside W (read by id only).
    Tier3,
    /// `claims`: globs over root-relative paths (the `exclude` grammar), as
    /// written; a document goes to the first shard, in config order, with a
    /// matching glob. Capped at `index_bytes`, in W.
    Claims(Vec<String>),
}

impl Shard {
    /// The archive shard (`tier3 = true`).
    pub fn is_archive(&self) -> bool {
        matches!(self.kind, ShardKind::Tier3)
    }
}

/// An error of the check tables: the 1-based line when known, and what is
/// wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError {
    pub line: Option<usize>,
    pub message: String,
}

impl ConfigError {
    /// `file:line: message` (`file: message` without a line).
    pub fn at(&self, file: &str) -> String {
        match self.line {
            Some(line) => format!("{file}:{line}: {}", self.message),
            None => format!("{file}: {}", self.message),
        }
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(line) => write!(f, "line {line}: {}", self.message),
            None => f.write_str(&self.message),
        }
    }
}

impl std::error::Error for ConfigError {}

/// [`CheckConfig::from_toml`] as a function.
pub fn check_config_from_toml(text: &str) -> Result<CheckConfig, ConfigError> {
    let error_at = |span: Option<Range<usize>>, message: String| ConfigError {
        line: span.map(|span| super::text::line_of_str(text, span.start)),
        message,
    };
    let raw: RawFile = toml::from_str(text)
        .map_err(|error| error_at(error.span(), error.message().trim().to_owned()))?;
    let mut config = CheckConfig::default();

    if let Some(budgets) = raw.budgets {
        // A value from 1 to `max`; `u64::MAX`: no upper bound of its own.
        let capped = |key: &str,
                      value: Option<Spanned<i64>>,
                      max: u64|
         -> Result<Option<u64>, ConfigError> {
            match value {
                None => Ok(None),
                Some(value) => {
                    let span = value.span();
                    match u64::try_from(*value.get_ref()) {
                        Ok(cap) if (1..=max).contains(&cap) => Ok(Some(cap)),
                        _ if max == u64::MAX => Err(error_at(
                            Some(span),
                            format!("`{key}` must be at least 1, not {}", value.get_ref()),
                        )),
                        _ => Err(error_at(
                            Some(span),
                            format!("`{key}` must be from 1 to {max}, not {}", value.get_ref()),
                        )),
                    }
                }
            }
        };
        let cap = |key: &str, value: Option<Spanned<i64>>| capped(key, value, u64::MAX);
        let target = &mut config.budgets;
        if let Some(value) = cap("tier0_bytes", budgets.tier0_bytes)? {
            target.tier0_bytes = value;
        }
        if let Some(value) = cap("tier1_bytes", budgets.tier1_bytes)? {
            target.tier1_bytes = value;
        }
        if let Some(value) = cap("index_bytes", budgets.index_bytes)? {
            target.index_bytes = value;
        }
        if let Some(value) = cap("decision_bytes", budgets.decision_bytes)? {
            target.decision_bytes = value;
        }
        target.canon_bytes = cap("canon_bytes", budgets.canon_bytes)?;
        // `spec bundle` reads it as a `u32` (`bundle_node_from_toml`): the
        // gate accepts no value that every bundle would refuse.
        target.bundle_node = capped("bundle_node", budgets.bundle_node, u64::from(u32::MAX))?;
        target.bundle_task = cap("bundle_task", budgets.bundle_task)?;
    }

    if let Some(classes) = raw.classes {
        let contract = |class: DocClass, raw: Option<Spanned<RawContract>>| {
            let Some(raw) = raw else {
                return Ok(ClassContract::default_for(class));
            };
            let span = raw.span();
            let raw = raw.into_inner();
            if raw
                .required
                .iter()
                .chain(&raw.optional)
                .any(|key| key.trim().is_empty())
            {
                return Err(error_at(
                    Some(span),
                    format!("`{class}`: an empty key name"),
                ));
            }
            Ok(ClassContract {
                required: raw.required,
                optional: raw.optional,
                closed: raw.closed,
            })
        };
        config.classes = Classes {
            canon: contract(DocClass::Canon, classes.canon)?,
            decision: contract(DocClass::Decision, classes.decision)?,
            spec: contract(DocClass::Spec, classes.spec)?,
            generated: contract(DocClass::Generated, classes.generated)?,
        };
    }

    if let Some(check) = raw.check
        && let Some(mode) = check.mode
    {
        let written = mode.get_ref().as_str();
        config.mode = match Mode::ALL
            .into_iter()
            .find(|known| known.as_str() == written)
        {
            Some(known) => known,
            None => {
                return Err(error_at(
                    Some(mode.span()),
                    format!(
                        "`mode` `{written}` is not \"observe\", \"enforce-introduced\" or \"enforce\""
                    ),
                ));
            }
        };
    }

    if let Some(generators) = raw.generators {
        config.generators = Some(generators_from(text, generators)?);
    }
    Ok(config)
}

/// `[budgets] bundle_node` as `spec bundle` reads it: the default budget of
/// a node bundle, in estimated tokens, and the line of its value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BundleNode {
    pub tokens: u32,
    pub line: usize,
}

/// `[budgets] bundle_node` alone (task spec `spec-cli-bundle`), read from
/// the whole `specengine.toml` text without judging any other table or key:
/// a broken `[classes]`, `[check]` or another `[budgets]` key never stops
/// `spec bundle` (`spec check` judges them). `None`: no such key, or
/// `budgets` is no table. A value that is no integer from 1 to `u32::MAX`
/// is an error at its line.
pub fn bundle_node_from_toml(text: &str) -> Result<Option<BundleNode>, ConfigError> {
    let error_at = |span: Option<Range<usize>>, message: String| ConfigError {
        line: span.map(|span| super::text::line_of_str(text, span.start)),
        message,
    };
    let loose: LooseFile = toml::from_str(text)
        .map_err(|error| error_at(error.span(), error.message().trim().to_owned()))?;
    if !loose.budgets.as_ref().is_some_and(toml::Value::is_table) {
        return Ok(None);
    }
    let narrow: NarrowFile = toml::from_str(text)
        .map_err(|error| error_at(error.span(), error.message().trim().to_owned()))?;
    let Some(value) = narrow.budgets.and_then(|budgets| budgets.bundle_node) else {
        return Ok(None);
    };
    let span = value.span();
    let tokens = value
        .get_ref()
        .as_integer()
        .and_then(|tokens| u32::try_from(tokens).ok())
        .filter(|&tokens| tokens >= 1);
    match tokens {
        Some(tokens) => Ok(Some(BundleNode {
            tokens,
            line: super::text::line_of_str(text, span.start),
        })),
        None => Err(error_at(
            Some(span.clone()),
            format!(
                "`[budgets] bundle_node` must be a whole number of tokens from 1 to {}, not {}",
                u32::MAX,
                text.get(span).unwrap_or_default().trim()
            ),
        )),
    }
}

/// The top level as [`bundle_node_from_toml`] first sees it: every key
/// but `budgets` ignored, whatever it holds.
#[derive(Deserialize)]
struct LooseFile {
    #[serde(default)]
    budgets: Option<toml::Value>,
}

/// The top level once `budgets` is known to be a table: only its
/// `bundle_node`, with the span of its value.
#[derive(Deserialize)]
struct NarrowFile {
    #[serde(default)]
    budgets: Option<NarrowBudgets>,
}

#[derive(Deserialize)]
struct NarrowBudgets {
    #[serde(default)]
    bundle_node: Option<Spanned<toml::Value>>,
}

/// The `[[generators]]` entries, checked: `command` present, not blank, a
/// plain YAML scalar ([`not_plain_scalar`]), not repeated; `writes` present,
/// not empty, root-relative paths no other entry writes; one `index = true`
/// at most, and only for an entry that writes the `[paths] index` file;
/// `gate` only beside `index = true`, not blank, a plain YAML scalar;
/// `shards` only beside `index = true` ([`shards_from`]), and that entry
/// writes nothing but the `[paths] index` file and its shards.
fn generators_from(
    text: &str,
    raw: Vec<Spanned<RawGenerator>>,
) -> Result<Vec<Generator>, ConfigError> {
    let error_at = |span: Range<usize>, message: String| ConfigError {
        line: Some(super::text::line_of_str(text, span.start)),
        message,
    };
    // `[paths] index` as `Paths` reads it; a `[paths]` error is reported by
    // `Paths::from_toml`, and the cross-check waits for it.
    let index_path = crate::paths_from_toml(text).ok().map(|paths| paths.index);
    let mut generators: Vec<Generator> = Vec::with_capacity(raw.len());
    for entry in raw {
        let span = entry.span();
        let line = super::text::line_of_str(text, span.start);
        let entry = entry.into_inner();
        let Some(command) = entry.command else {
            return Err(error_at(
                span,
                "generator entry without `command`".to_owned(),
            ));
        };
        if command.get_ref().trim().is_empty() {
            return Err(error_at(
                command.span(),
                "generator `command` is blank".to_owned(),
            ));
        }
        if let Some(problem) = not_plain_scalar(command.get_ref()) {
            return Err(error_at(
                command.span(),
                format!(
                    "generator `command` {:?} {problem}: the index header carries it as written (`generator:` and the build comment)",
                    command.get_ref()
                ),
            ));
        }
        if let Some(first) = generators
            .iter()
            .find(|known| known.command == *command.get_ref())
        {
            return Err(error_at(
                command.span(),
                format!(
                    "generator `command` `{}` is repeated (first on line {})",
                    command.get_ref(),
                    first.line
                ),
            ));
        }
        let Some(writes) = entry.writes else {
            return Err(error_at(
                span,
                "generator entry without `writes`".to_owned(),
            ));
        };
        let writes_span = writes.span();
        let mut paths: Vec<String> = Vec::new();
        // Each written path with the span of its first occurrence.
        let mut written: Vec<Range<usize>> = Vec::new();
        for path in writes.into_inner() {
            let checked = crate::paths_toml::checked_path(path.get_ref(), false)
                .map_err(|problem| error_at(path.span(), format!("`writes`: {problem}")))?;
            if let Some(other) = generators
                .iter()
                .find(|known| known.writes.contains(&checked))
            {
                return Err(error_at(
                    path.span(),
                    format!(
                        "`writes`: {checked} is also written by `{}` (line {})",
                        other.command, other.line
                    ),
                ));
            }
            if !paths.contains(&checked) {
                paths.push(checked);
                written.push(path.span());
            }
        }
        if paths.is_empty() {
            return Err(error_at(
                writes_span,
                "generator `writes` is empty".to_owned(),
            ));
        }
        let index = match entry.index {
            Some(index) if *index.get_ref() => {
                if let Some(first) = generators.iter().find(|known| known.index) {
                    return Err(error_at(
                        index.span(),
                        format!(
                            "`index = true` twice: `{}` (line {}) already renders the index",
                            first.command, first.line
                        ),
                    ));
                }
                match &index_path {
                    Some(None) => {
                        return Err(error_at(
                            index.span(),
                            "`index = true` without a `[paths] index`".to_owned(),
                        ));
                    }
                    Some(Some(path)) if !paths.contains(path) => {
                        return Err(error_at(
                            index.span(),
                            format!(
                                "`index = true`, but `writes` lacks the `[paths] index` {path}"
                            ),
                        ));
                    }
                    _ => {}
                }
                true
            }
            _ => false,
        };
        let gate = match entry.gate {
            Some(gate) if !index => {
                return Err(error_at(
                    gate.span(),
                    "`gate` is only for the entry with `index = true`".to_owned(),
                ));
            }
            Some(gate) if gate.get_ref().trim().is_empty() => {
                return Err(error_at(
                    gate.span(),
                    "generator `gate` is blank".to_owned(),
                ));
            }
            Some(gate) => {
                if let Some(problem) = not_plain_scalar(gate.get_ref()) {
                    return Err(error_at(
                        gate.span(),
                        format!(
                            "generator `gate` {:?} {problem}: the index header names it as written",
                            gate.get_ref()
                        ),
                    ));
                }
                Some(gate.into_inner())
            }
            None => None,
        };
        let shards = match entry.shards {
            None => Vec::new(),
            Some(shards) if !index => {
                return Err(error_at(
                    shards.span(),
                    "`shards` is only for the entry with `index = true`".to_owned(),
                ));
            }
            Some(shards) => shards_from(text, shards.into_inner(), &paths, &index_path)?,
        };
        // The index entry writes the root and its shards, nothing else: a
        // stale output cannot hide in `writes`, never compared with a render.
        if index && let Some(Some(root)) = &index_path {
            for (path, span) in paths.iter().zip(&written) {
                if path != root && !shards.iter().any(|shard| shard.path == *path) {
                    return Err(error_at(
                        span.clone(),
                        format!(
                            "`writes`: {path} is neither the `[paths] index` {root} nor a shard: the index entry writes only the index and its `shards`"
                        ),
                    ));
                }
            }
        }
        generators.push(Generator {
            command: command.into_inner(),
            writes: paths,
            index,
            gate,
            shards,
            line,
        });
    }
    Ok(generators)
}

/// The `shards` of the index entry, checked, in config order: each a table
/// with `path` and exactly one of `tier3 = true` and `claims` (a non-empty
/// list of globs under the `exclude` rules); a path under the `[paths]` path
/// rules, neither `[paths] index` nor another shard's, and in the entry's
/// `writes`; one `tier3 = true` at most. Claims and paths are rendered into
/// the index's Markdown (a pointer's label and link, a shard's H1): neither
/// holds a control character or a backtick. `index_path` is `[paths] index` as
/// read (`None`: a `[paths]` error, reported by `Paths::from_toml`, and the
/// comparison with it waits).
fn shards_from(
    text: &str,
    raw: Vec<Spanned<RawShard>>,
    writes: &[String],
    index_path: &Option<Option<String>>,
) -> Result<Vec<Shard>, ConfigError> {
    let error_at = |span: Range<usize>, message: String| ConfigError {
        line: Some(super::text::line_of_str(text, span.start)),
        message,
    };
    let mut shards: Vec<Shard> = Vec::with_capacity(raw.len());
    for shard in raw {
        let span = shard.span();
        let line = super::text::line_of_str(text, span.start);
        let shard = shard.into_inner();
        let Some(path) = shard.path else {
            return Err(error_at(span, "a shard without `path`".to_owned()));
        };
        let kind = match (shard.tier3, shard.claims) {
            (Some(_), Some(_)) => {
                return Err(error_at(
                    span,
                    "a shard with both `tier3` and `claims`: one of them".to_owned(),
                ));
            }
            (None, None) => {
                return Err(error_at(
                    span,
                    "a shard without `tier3 = true` or `claims`: one of them".to_owned(),
                ));
            }
            (Some(tier3), None) => {
                if !*tier3.get_ref() {
                    return Err(error_at(
                        tier3.span(),
                        "shard `tier3` is only `true`: set it, or give `claims` instead".to_owned(),
                    ));
                }
                if let Some(first) = shards.iter().find(|known| known.is_archive()) {
                    return Err(error_at(
                        tier3.span(),
                        format!(
                            "`tier3 = true` twice: the shard {} (line {}) already holds Tier 3",
                            first.path, first.line
                        ),
                    ));
                }
                ShardKind::Tier3
            }
            (None, Some(claims)) => {
                let claims_span = claims.span();
                let mut globs: Vec<String> = Vec::new();
                for claim in claims.into_inner() {
                    let checked = crate::paths_toml::checked_path(claim.get_ref(), false).map_err(
                        |problem| error_at(claim.span(), format!("shard `claims`: {problem}")),
                    )?;
                    if let Some(why) = not_markdown_text(&checked) {
                        return Err(error_at(
                            claim.span(),
                            format!(
                                "shard `claims`: {:?} {why}: the index renders it into Markdown",
                                claim.get_ref()
                            ),
                        ));
                    }
                    globs.push(checked);
                }
                if globs.is_empty() {
                    return Err(error_at(claims_span, "shard `claims` is empty".to_owned()));
                }
                ShardKind::Claims(globs)
            }
        };
        let checked = crate::paths_toml::checked_path(path.get_ref(), false)
            .map_err(|problem| error_at(path.span(), format!("shard `path`: {problem}")))?;
        if let Some(why) = not_markdown_text(&checked) {
            return Err(error_at(
                path.span(),
                format!(
                    "shard `path`: {:?} {why}: the index renders it into Markdown",
                    path.get_ref()
                ),
            ));
        }
        if let Some(Some(root)) = index_path
            && *root == checked
        {
            return Err(error_at(
                path.span(),
                format!("shard `path` {checked} is the `[paths] index`: the root is no shard"),
            ));
        }
        if let Some(other) = shards.iter().find(|known| known.path == checked) {
            return Err(error_at(
                path.span(),
                format!(
                    "shard `path` {checked} is repeated (first on line {})",
                    other.line
                ),
            ));
        }
        if !writes.contains(&checked) {
            return Err(error_at(
                path.span(),
                format!("shard `path` {checked} is not in the entry's `writes`: list it there too"),
            ));
        }
        shards.push(Shard {
            path: checked,
            kind,
            line,
        });
    }
    Ok(shards)
}

/// Why `value`, a shard's claim or path, would not render into the index's
/// Markdown as itself, if so: a newline or another control character (a
/// line break inside the pointer or the H1), a backtick (the end of the code
/// span a claim is shown in).
fn not_markdown_text(value: &str) -> Option<&'static str> {
    if value.chars().any(char::is_control) {
        return Some("contains a newline or a control character");
    }
    if value.contains('`') {
        return Some("contains a backtick");
    }
    None
}

/// Characters that cannot start a plain YAML scalar.
const YAML_INDICATORS: [char; 19] = [
    '-', '[', ']', '{', '}', ',', '&', '*', '!', '|', '>', '\'', '"', '%', '@', '`', '#', '?', ':',
];

/// Why `value` would not read back as itself, a string, from a plain YAML
/// scalar (`generator: <value>` in the rendered header), or would break the
/// header's HTML comment, if so: a newline or control character, leading or
/// trailing whitespace, a YAML indicator first, `: ` or ` #` inside, `:`
/// last; then the round trip through the front-matter reader
/// ([`read_back`]): a value it reads as null, a boolean or a number (or as
/// another string), or not as a scalar; then `-->` inside.
fn not_plain_scalar(value: &str) -> Option<String> {
    if value.chars().any(char::is_control) {
        return Some("contains a newline or a control character".to_owned());
    }
    if value.trim() != value {
        return Some("has leading or trailing whitespace".to_owned());
    }
    if let Some(first) = value.chars().next()
        && YAML_INDICATORS.contains(&first)
    {
        return Some(format!("starts with the YAML indicator `{first}`"));
    }
    if value.contains(": ") || value.contains(" #") {
        return Some("contains `: ` or ` #`".to_owned());
    }
    if value.ends_with(':') {
        return Some("ends with `:`".to_owned());
    }
    match read_back(value) {
        ReadBack::Same => {}
        ReadBack::NotAString => {
            return Some(
                "is read by YAML as a null, a boolean or a number, not a string".to_owned(),
            );
        }
        ReadBack::Broken => {
            return Some("is not read back by YAML as a plain scalar".to_owned());
        }
    }
    if value.contains("-->") {
        return Some("contains `-->`, which would close the index header's comment".to_owned());
    }
    None
}

/// The text the front-matter reader gives a non-finite float (any YAML
/// spelling of an infinity or a NaN): a string equal to it cannot be told
/// from that number by reading it back.
const NON_FINITE_TEXTS: [&str; 3] = [".inf", "-.inf", ".nan"];

/// How `value` reads back through the front-matter reader (`crate::yaml`,
/// the options every parse uses) as the plain scalar of `generator:`.
enum ReadBack {
    /// The same string.
    Same,
    /// Null, a boolean or a number — or a string that is not the value (a
    /// non-finite float normalised to `.inf`, `-.inf`, `.nan`), or one of
    /// those three texts themselves.
    NotAString,
    /// No scalar at all (a YAML error, a collection).
    Broken,
}

/// Round trip of `generator: <value>` through the one YAML reader. Pure: the
/// text is built here, nothing is read.
fn read_back(value: &str) -> ReadBack {
    use crate::yaml::{self, YValue};
    let Ok(root) = yaml::parse(&format!("generator: {value}\n")) else {
        return ReadBack::Broken;
    };
    let YValue::Map(entries) = root.value else {
        return ReadBack::Broken;
    };
    let [(_, read)] = entries.as_slice() else {
        return ReadBack::Broken;
    };
    match &read.value {
        YValue::Str(text) if text == value && !NON_FINITE_TEXTS.contains(&value) => ReadBack::Same,
        YValue::Str(_)
        | YValue::Null
        | YValue::Bool(_)
        | YValue::Int(_)
        | YValue::UInt(_)
        | YValue::Float(_) => ReadBack::NotAString,
        YValue::Seq(_) | YValue::Map(_) => ReadBack::Broken,
    }
}

/// The file: only the check tables are read; every other table is ignored.
#[derive(Deserialize)]
struct RawFile {
    #[serde(default)]
    budgets: Option<RawBudgets>,
    #[serde(default)]
    classes: Option<RawClasses>,
    #[serde(default)]
    check: Option<RawCheck>,
    #[serde(default)]
    generators: Option<Vec<Spanned<RawGenerator>>>,
}

/// One `[[generators]]` entry as written.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawGenerator {
    #[serde(default)]
    command: Option<Spanned<String>>,
    #[serde(default)]
    writes: Option<Spanned<Vec<Spanned<String>>>>,
    #[serde(default)]
    index: Option<Spanned<bool>>,
    #[serde(default)]
    gate: Option<Spanned<String>>,
    #[serde(default)]
    shards: Option<Spanned<Vec<Spanned<RawShard>>>>,
}

/// One `shards` table of the index entry as written.
#[derive(Deserialize)]
#[serde(
    deny_unknown_fields,
    expecting = "a shard table: `path`, and `tier3 = true` or `claims`"
)]
struct RawShard {
    #[serde(default)]
    path: Option<Spanned<String>>,
    #[serde(default)]
    tier3: Option<Spanned<bool>>,
    #[serde(default)]
    claims: Option<Spanned<Vec<Spanned<String>>>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBudgets {
    #[serde(default)]
    tier0_bytes: Option<Spanned<i64>>,
    #[serde(default)]
    tier1_bytes: Option<Spanned<i64>>,
    #[serde(default)]
    index_bytes: Option<Spanned<i64>>,
    #[serde(default)]
    decision_bytes: Option<Spanned<i64>>,
    #[serde(default)]
    canon_bytes: Option<Spanned<i64>>,
    #[serde(default)]
    bundle_node: Option<Spanned<i64>>,
    #[serde(default)]
    bundle_task: Option<Spanned<i64>>,
}

/// `[classes]`: only the four class names.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawClasses {
    #[serde(default)]
    canon: Option<Spanned<RawContract>>,
    #[serde(default)]
    decision: Option<Spanned<RawContract>>,
    #[serde(default)]
    spec: Option<Spanned<RawContract>>,
    #[serde(default)]
    generated: Option<Spanned<RawContract>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawContract {
    #[serde(default)]
    required: Vec<String>,
    #[serde(default)]
    optional: Vec<String>,
    #[serde(default)]
    closed: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCheck {
    #[serde(default)]
    mode: Option<Spanned<String>>,
}
