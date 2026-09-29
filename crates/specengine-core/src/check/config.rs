//! `CheckConfig::from_toml`: the `[budgets]`, `[classes]` and `[check]`
//! tables of `specengine.toml`, and only those (docs/canon/spec-check.md,
//! "Configuration"). Pure and strict: an unknown key or class, a wrong type, a cap
//! below 1 or an unknown mode is an error `file:line: message` through
//! [`ConfigError::at`]; other tables are ignored, so editing these tables
//! leaves the `[ids]` fingerprint (and every stored parse) alone.

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

/// What blocks: `observe` counts and never blocks; `enforce` blocks on
/// errors not in debt.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Observe,
    #[default]
    Enforce,
}

impl Mode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Observe => "observe",
            Self::Enforce => "enforce",
        }
    }
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// `[budgets]`, `[classes]` and `[check]`, defaults applied.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckConfig {
    pub budgets: Budgets,
    pub classes: Classes,
    pub mode: Mode,
}

impl CheckConfig {
    /// Reads the three tables; absent tables give the defaults.
    pub fn from_toml(text: &str) -> Result<Self, ConfigError> {
        check_config_from_toml(text)
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
        let cap = |key: &str, value: Option<Spanned<i64>>| -> Result<Option<u64>, ConfigError> {
            match value {
                None => Ok(None),
                Some(value) => {
                    let span = value.span();
                    match u64::try_from(*value.get_ref()) {
                        Ok(cap) if cap >= 1 => Ok(Some(cap)),
                        _ => Err(error_at(
                            Some(span),
                            format!("`{key}` must be at least 1, not {}", value.get_ref()),
                        )),
                    }
                }
            }
        };
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
        target.bundle_node = cap("bundle_node", budgets.bundle_node)?;
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
        config.mode = match mode.get_ref().as_str() {
            "observe" => Mode::Observe,
            "enforce" => Mode::Enforce,
            other => {
                return Err(error_at(
                    Some(mode.span()),
                    format!("`mode` `{other}` is neither \"observe\" nor \"enforce\""),
                ));
            }
        };
    }
    Ok(config)
}

/// The file: only the three tables are read; every other table is ignored.
#[derive(Deserialize)]
struct RawFile {
    #[serde(default)]
    budgets: Option<RawBudgets>,
    #[serde(default)]
    classes: Option<RawClasses>,
    #[serde(default)]
    check: Option<RawCheck>,
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
