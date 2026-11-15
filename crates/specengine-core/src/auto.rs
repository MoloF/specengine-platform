//! The one predicate every delegated act takes (`docs/canon/auto-mode.md`
//! "Switches", "Judging an item"; task spec `auto-rule` "The predicate"):
//! pure, no I/O, every input a fact the caller read in one snapshot, so one
//! state gives one verdict.
//!
//! - **Switches** first, in this order: `off` (no rule, a mode not taking
//!   the surface, a pack's branch changed since its start), `team` (two or
//!   more `[team]` logins; on a hub also none, or a sole login that is no
//!   `owner`), `stopped` (the stop file). The first that holds is every
//!   item's hold.
//! - **An item**: the checks its act takes, in the canon's table order, the
//!   first failing one its [`Reason`] with its exact detail; `not-in-pack`
//!   only locally, `author` and `cap` only on a hub.
//! - **Under a pack** the rule is [`effective`]: a path is covered when both
//!   the frozen rule and `HEAD`'s cover it, excluded when either excludes it;
//!   an ID excluded when either rule's excluded kinds hold its prefix. A
//!   check failing in one of the two only names which.
//!
//! Nothing here names a zone, a kind or a login: each is the config's
//! (ADR-0008).

use std::fmt;

use specengine_model::{RunOutcome, TaskStatus};

use crate::auto_toml::{AutoMode, AutoRule, NamedGlobs};
use crate::intake::with_article;
use crate::lock::LOCK_FILE;
use crate::proposal::AuthorType;
use crate::team_toml::TeamRole;

/// The trailer a delegated commit ends with: `Delegated-by: auto <hash>`.
pub const DELEGATED_BY_TRAILER: &str = "Delegated-by";

/// The stop file's name in the data directory.
pub const STOP_FILE: &str = "auto-stop";

/// The project config's file name, any ASCII case, at any depth.
const CONFIG_FILE: &str = "specengine.toml";

/// `Delegated-by: auto <hash>` and LF: a delegated commit's last line.
pub fn delegated_by_line(hash: &str) -> String {
    format!("{DELEGATED_BY_TRAILER}: auto {hash}\n")
}

/// Where an act runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Surface {
    /// A terminal: `spec auto *`.
    Local,
    /// A hub's daemon.
    Hub,
}

impl Surface {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Hub => "hub",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        [Self::Local, Self::Hub]
            .into_iter()
            .find(|surface| surface.as_str() == text)
    }

    /// The rule's mode takes this surface.
    pub const fn taken_by(self, mode: AutoMode) -> bool {
        matches!(
            (self, mode),
            (Self::Local, AutoMode::Local | AutoMode::Both)
                | (Self::Hub, AutoMode::Hub | AutoMode::Both)
        )
    }
}

impl fmt::Display for Surface {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Why an item holds: a closed set, the land's and the runner's last
/// (`docs/canon/auto-mode.md` "Judging an item").
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Reason {
    Off,
    Team,
    Stopped,
    NotInPack,
    Author,
    Until,
    Status,
    RunOutcome,
    ProposalKind,
    Staged,
    ConfigTarget,
    ExcludedZone,
    ExcludedKind,
    Uncovered,
    Findings,
    Rebases,
    Cap,
    // A land's (`docs/features/auto-land.md`).
    Place,
    MergeConflict,
    Check,
    BranchCheckedOut,
    BranchMoved,
    // The runner's (`spec auto hold`, `docs/features/auto-runner.md`).
    AgentError,
    DeniedTool,
    NoMcp,
    MaxTurns,
    MaxBudget,
    NotPlanned,
    NotDone,
}

impl Reason {
    pub const ALL: [Self; 29] = [
        Self::Off,
        Self::Team,
        Self::Stopped,
        Self::NotInPack,
        Self::Author,
        Self::Until,
        Self::Status,
        Self::RunOutcome,
        Self::ProposalKind,
        Self::Staged,
        Self::ConfigTarget,
        Self::ExcludedZone,
        Self::ExcludedKind,
        Self::Uncovered,
        Self::Findings,
        Self::Rebases,
        Self::Cap,
        Self::Place,
        Self::MergeConflict,
        Self::Check,
        Self::BranchCheckedOut,
        Self::BranchMoved,
        Self::AgentError,
        Self::DeniedTool,
        Self::NoMcp,
        Self::MaxTurns,
        Self::MaxBudget,
        Self::NotPlanned,
        Self::NotDone,
    ];

    /// The reasons `spec auto hold --reason` takes: the runner's.
    pub const RUNNER: [Self; 7] = [
        Self::AgentError,
        Self::DeniedTool,
        Self::NoMcp,
        Self::MaxTurns,
        Self::MaxBudget,
        Self::NotPlanned,
        Self::NotDone,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Team => "team",
            Self::Stopped => "stopped",
            Self::NotInPack => "not-in-pack",
            Self::Author => "author",
            Self::Until => "until",
            Self::Status => "status",
            Self::RunOutcome => "run-outcome",
            Self::ProposalKind => "proposal-kind",
            Self::Staged => "staged",
            Self::ConfigTarget => "config-target",
            Self::ExcludedZone => "excluded-zone",
            Self::ExcludedKind => "excluded-kind",
            Self::Uncovered => "uncovered",
            Self::Findings => "findings",
            Self::Rebases => "rebases",
            Self::Cap => "cap",
            Self::Place => "place",
            Self::MergeConflict => "merge-conflict",
            Self::Check => "check",
            Self::BranchCheckedOut => "branch-checked-out",
            Self::BranchMoved => "branch-moved",
            Self::AgentError => "agent-error",
            Self::DeniedTool => "denied-tool",
            Self::NoMcp => "no-mcp",
            Self::MaxTurns => "max-turns",
            Self::MaxBudget => "max-budget",
            Self::NotPlanned => "not-planned",
            Self::NotDone => "not-done",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|reason| reason.as_str() == text)
    }

    /// A switch: `off`, `team` or `stopped`.
    pub const fn is_switch(self) -> bool {
        matches!(self, Self::Off | Self::Team | Self::Stopped)
    }
}

impl fmt::Display for Reason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The act an item's hold names (`auto.held`'s `act`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Act {
    Approve,
    Apply,
    Land,
    Run,
}

impl Act {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Approve => "approve",
            Self::Apply => "apply",
            Self::Land => "land",
            Self::Run => "run",
        }
    }
}

impl fmt::Display for Act {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One item's verdict.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Verdict {
    Apply,
    Hold { reason: Reason, detail: String },
}

impl Verdict {
    fn hold(reason: Reason, detail: String) -> Self {
        Self::Hold { reason, detail }
    }

    /// `Some((reason, detail))` for a hold.
    pub fn held(&self) -> Option<(Reason, &str)> {
        match self {
            Self::Apply => None,
            Self::Hold { reason, detail } => Some((*reason, detail)),
        }
    }
}

/// A row's staged choice, as the predicate reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StagedAs {
    Approve,
    Reject,
}

/// What the review's preview says of a row's text now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextState {
    Applies,
    Rebases,
    Conflict,
    Gone,
    /// A file-form create whose file is on disk at the recorded place.
    FileExists,
}

/// The switches' facts.
#[derive(Debug, Clone, Copy)]
pub struct Switches<'a> {
    pub surface: Surface,
    /// The rule at `HEAD`; `Err` the switch `off`'s why.
    pub rule: Result<&'a AutoRule, &'a str>,
    /// `[team]`'s logins and roles (none without a table); `Err` the switch
    /// `team`'s why (`` `[team]` does not read: … ``).
    pub logins: Result<&'a [(String, TeamRole)], &'a str>,
    /// The stop file's path, when it exists.
    pub stopped: Option<&'a str>,
}

/// A live pack's facts: its `auto.started` `seq`, `--until` and frozen rule.
#[derive(Debug, Clone, Copy)]
pub struct PackFacts<'a> {
    pub seq: i64,
    pub until: Option<&'a str>,
    pub frozen: &'a AutoRule,
}

/// A proposal's facts (task spec `auto-rule` "Rules and edge cases").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposalFacts {
    pub id: String,
    /// Its kind's name (`update`, `create`, …).
    pub kind: String,
    pub staged: Option<(StagedAs, String)>,
    pub author_type: AuthorType,
    pub author_login: Option<String>,
    /// The pack of its task, when the caller found one.
    pub pack: Option<i64>,
    /// Root-relative: its target path.
    pub paths: Vec<String>,
    /// Canonical: the target ID, its document's `id`, a create's new IDs.
    pub ids: Vec<String>,
    /// Its stored findings: code, path, line.
    pub findings: Vec<(String, String, u32)>,
    pub text: TextState,
}

/// A task's facts (task spec `auto-rule` "Rules and edge cases").
#[derive(Debug, Clone)]
pub struct TaskFacts<'a> {
    pub id: String,
    pub status: TaskStatus,
    pub pack: Option<PackFacts<'a>>,
    /// Its targets' and affected nodes' holder paths, root-relative, each
    /// once, in that order.
    pub paths: Vec<String>,
    /// Its targets and affected nodes, canonical.
    pub ids: Vec<String>,
    /// The IDs that resolve to no file: judged `uncovered`.
    pub unresolved: Vec<String>,
    /// Its last run's number and outcome; `None` with no closed run.
    pub last_run: Option<(u32, RunOutcome)>,
}

/// The rule an item is judged by: `HEAD`'s, and under a pack the one its
/// start froze too (see the module documentation).
#[derive(Debug, Clone, Copy)]
pub struct Effective<'a> {
    pub now: &'a AutoRule,
    /// The frozen rule and its pack's `seq`.
    pub frozen: Option<(&'a AutoRule, i64)>,
}

/// The effective rule of `now` and, under a pack, its frozen one.
pub fn effective<'a>(frozen: Option<(&'a AutoRule, i64)>, now: &'a AutoRule) -> Effective<'a> {
    Effective { now, frozen }
}

/// Which of the two rules alone fails a check.
enum Alone {
    Head,
    Start(i64),
}

impl Alone {
    fn suffix(&self) -> String {
        match self {
            Self::Head => " (the rule at HEAD)".to_owned(),
            Self::Start(pack) => format!(" (the rule at pack {pack}'s start)"),
        }
    }
}

impl Effective<'_> {
    /// The smaller `max_tasks`.
    pub fn max_tasks(&self) -> u32 {
        self.frozen.map_or(self.now.max_tasks, |(frozen, _)| {
            frozen.max_tasks.min(self.now.max_tasks)
        })
    }

    /// The smaller `max_applies`.
    pub fn max_applies(&self) -> u32 {
        self.frozen.map_or(self.now.max_applies, |(frozen, _)| {
            frozen.max_applies.min(self.now.max_applies)
        })
    }

    /// The excluded zone of `rule` holding `path`.
    fn excluding<'r>(rule: &'r AutoRule, path: &str) -> Option<&'r NamedGlobs> {
        rule.exclude_zones.iter().find(|zone| zone.covers(path))
    }

    /// `` `<path>` is in the excluded zone `<z>` ``, when either rule
    /// excludes it.
    fn excluded_zone(&self, path: &str) -> Option<String> {
        if is_outside(path) {
            return None;
        }
        let now = Self::excluding(self.now, path);
        let frozen = self
            .frozen
            .map(|(frozen, pack)| (Self::excluding(frozen, path), pack));
        let (zone, alone) = match (now, frozen) {
            (Some(zone), None | Some((Some(_), _))) => (zone, None),
            (Some(zone), Some((None, _))) => (zone, Some(Alone::Head)),
            (None, Some((Some(zone), pack))) => (zone, Some(Alone::Start(pack))),
            (None, None | Some((None, _))) => return None,
        };
        Some(format!(
            "`{path}` is in the excluded zone `{}`{}",
            zone.name,
            alone.map(|alone| alone.suffix()).unwrap_or_default()
        ))
    }

    /// `` `<ID>` is a <kind>, an excluded kind ``, when either rule's
    /// excluded kinds hold its prefix.
    fn excluded_kind(&self, id: &str) -> Option<String> {
        self.excluded_kind_as(id, &format!("`{id}`"))
    }

    /// [`Self::excluded_kind`], the ID named as `named`.
    fn excluded_kind_as(&self, id: &str, named: &str) -> Option<String> {
        let kind_in = |rule: &AutoRule| {
            rule.exclude_kinds
                .iter()
                .find(|kind| kind.holds(id))
                .map(|kind| kind.kind.clone())
        };
        let now = kind_in(self.now);
        let frozen = self.frozen.map(|(frozen, pack)| (kind_in(frozen), pack));
        let (kind, alone) = match (now, frozen) {
            (Some(kind), None | Some((Some(_), _))) => (kind, None),
            (Some(kind), Some((None, _))) => (kind, Some(Alone::Head)),
            (None, Some((Some(kind), pack))) => (kind, Some(Alone::Start(pack))),
            (None, None | Some((None, _))) => return None,
        };
        Some(format!(
            "{named} is {}, an excluded kind{}",
            with_article_any(&kind),
            alone.map(|alone| alone.suffix()).unwrap_or_default()
        ))
    }

    /// `` `<path>` is in no zone the rule covers (<z1>, <z2>) ``, unless both
    /// rules cover it.
    fn uncovered(&self, path: &str) -> Option<String> {
        // A path outside the project root lies in no zone.
        let outside = is_outside(path);
        let covers = |rule: &AutoRule| !outside && rule.zones.iter().any(|zone| zone.covers(path));
        let listed = |rule: &AutoRule| rule.zone_names().join(", ");
        let now = covers(self.now);
        let (rule, alone) = match self.frozen {
            None if now => return None,
            None => (self.now, None),
            Some((frozen, pack)) => match (now, covers(frozen)) {
                (true, true) => return None,
                (true, false) => (frozen, Some(Alone::Start(pack))),
                (false, true) => (self.now, Some(Alone::Head)),
                (false, false) => (self.now, None),
            },
        };
        Some(format!(
            "`{path}` is in no zone the rule covers ({}){}",
            listed(rule),
            alone.map(|alone| alone.suffix()).unwrap_or_default()
        ))
    }
}

/// A root-relative path that leaves the project root (`../…`): a land's
/// change outside it, which no zone covers.
pub fn is_outside(path: &str) -> bool {
    path == ".." || path.starts_with("../")
}

/// The separator of a pack's line `<pack>/<n>`, of a feature-scoped
/// `slug/ID` and of a root-relative path's components.
const SLASH: char = '/';

/// What opens a comment line of a `spec auto start --lines` file.
const COMMENT: char = '#';

/// The longest kept line of a `spec auto start --lines` file, in bytes.
pub const PACK_LINE_MAX: usize = 4096;

/// `<pack>/<n>` as two decimal numbers: a pack's `seq` and a line number.
pub fn parse_pack_line(written: &str) -> Option<(i64, u32)> {
    let (pack, line) = written.split_once(SLASH)?;
    let digits = |text: &str| !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit());
    if !digits(pack) || !digits(line) {
        return None;
    }
    Some((pack.parse().ok()?, line.parse().ok()?))
}

/// The lines a `spec auto start --lines` file keeps: each trimmed, an empty
/// one and one opening with `#` skipped, each at most [`PACK_LINE_MAX`]
/// bytes with no control character; else `line <n>: <why>`, `n` the file's
/// line number.
pub fn pack_lines(text: &str) -> Result<Vec<String>, String> {
    let mut kept = Vec::new();
    for (at, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(COMMENT) {
            continue;
        }
        let number = at + 1;
        if line.len() > PACK_LINE_MAX {
            return Err(format!(
                "line {number}: {} bytes; at most {PACK_LINE_MAX}",
                line.len()
            ));
        }
        if line.chars().any(char::is_control) {
            return Err(format!("line {number}: it holds a control character"));
        }
        kept.push(line.to_owned());
    }
    Ok(kept)
}

/// The ID of a stored, canonical name: the text after the last `/` (a
/// feature-scoped `slug/ID`'s ID), else the whole name.
pub fn bare_id(name: &str) -> &str {
    name.rsplit_once(SLASH).map_or(name, |(_, bare)| bare)
}

/// An event type's family: the text before its first `.` (`task` of
/// `task.run_reported`, `auto` of `auto.held`), else the whole type.
pub fn event_family(kind: &str) -> &str {
    kind.split_once('.').map_or(kind, |(family, _)| family)
}

/// `parts` joined with `/`: a root-relative path.
pub fn slash_joined<I, S>(parts: I) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut out = String::new();
    for part in parts {
        if !out.is_empty() {
            out.push(SLASH);
        }
        out.push_str(part.as_ref());
    }
    out
}

/// The worktree-top path `top` relative to the project root `root_rel`
/// (both `/`-joined): below it, stripped; else `../` once per component of
/// `root_rel`, then `top`, a path outside the root ([`is_outside`]).
pub fn root_relative(root_rel: &str, top: &str) -> String {
    if root_rel.is_empty() {
        return top.to_owned();
    }
    if let Some(rest) = top
        .strip_prefix(root_rel)
        .and_then(|rest| rest.strip_prefix(SLASH))
    {
        return rest.to_owned();
    }
    let up = "../".repeat(root_rel.split(SLASH).count());
    format!("{up}{top}")
}

/// `kind` with its article, as a detail names it.
fn with_article_any(kind: &str) -> String {
    with_article(kind)
}

/// `path` names a project config (any depth, any ASCII case) or the root's
/// lock: never delegated, the gate's rule
/// (`docs/canon/gate.md` "Controlled paths").
pub fn is_config_target(path: &str) -> bool {
    path == LOCK_FILE
        || path
            .rsplit('/')
            .next()
            .is_some_and(|last| last.eq_ignore_ascii_case(CONFIG_FILE))
}

/// The switches, in order: `Err((reason, why))` for the first that holds.
pub fn judge_switches(switches: &Switches<'_>) -> Result<(), (Reason, String)> {
    switched(switches, None).map(|_| ())
}

/// The switches with a pack's own, its branch changed since its start
/// among `off`'s: `Err((reason, why))` for the first that holds
/// (`spec auto next` skips such a pack).
pub fn judge_pack_switches(
    switches: &Switches<'_>,
    pack: &PackFacts<'_>,
) -> Result<(), (Reason, String)> {
    switched(switches, Some(pack)).map(|_| ())
}

/// The switches, a pack's branch change among `off`'s: the rule at `HEAD`
/// when none holds.
fn switched<'a>(
    switches: &Switches<'a>,
    pack: Option<&PackFacts<'_>>,
) -> Result<&'a AutoRule, (Reason, String)> {
    let rule = match switches.rule {
        Ok(rule) => rule,
        Err(why) => return Err((Reason::Off, why.to_owned())),
    };
    if rule.mode == AutoMode::Off {
        return Err((Reason::Off, "[auto] mode is off".to_owned()));
    }
    if !switches.surface.taken_by(rule.mode) {
        let why = match switches.surface {
            Surface::Local => format!(
                "[auto] mode is {}: a terminal acts only under local or both",
                rule.mode
            ),
            Surface::Hub => format!(
                "[auto] mode is {}: a hub acts only under hub or both",
                rule.mode
            ),
        };
        return Err((Reason::Off, why));
    }
    if let Some(pack) = pack
        && pack.frozen.branch != rule.branch
    {
        return Err((
            Reason::Off,
            format!(
                "the rule's branch changed since pack {} started (`{}` -> `{}`)",
                pack.seq, pack.frozen.branch, rule.branch
            ),
        ));
    }
    let logins = switches
        .logins
        .map_err(|why| (Reason::Team, why.to_owned()))?;
    if logins.len() >= 2 {
        return Err((
            Reason::Team,
            format!("[team] names {} logins: auto stops at two", logins.len()),
        ));
    }
    if switches.surface == Surface::Hub {
        match logins {
            [] => {
                return Err((
                    Reason::Team,
                    "[team] names no login: a hub delegates only to its sole owner".to_owned(),
                ));
            }
            [(login, role)] if *role != TeamRole::Owner => {
                return Err((
                    Reason::Team,
                    format!(
                        "the sole [team] login `{login}` is a {}: a hub delegates only to the owner",
                        role.as_str()
                    ),
                ));
            }
            _ => {}
        }
    }
    if let Some(path) = switches.stopped {
        return Err((
            Reason::Stopped,
            format!("the stop file {path} exists: spec auto resume removes it"),
        ));
    }
    Ok(rule)
}

/// The path and ID checks of `act`'s rows, in order: `config-target`,
/// `excluded-zone`, `excluded-kind` (the IDs given, then each changed
/// document's declared IDs, `declared`), `uncovered` (the paths, then the
/// IDs resolving to no file).
fn judge_paths(
    rule: &Effective<'_>,
    paths: &[String],
    ids: Option<&[String]>,
    unresolved: &[String],
    declared: &[(String, Vec<String>)],
) -> Verdict {
    if let Some(path) = paths.iter().find(|path| is_config_target(path)) {
        return Verdict::hold(
            Reason::ConfigTarget,
            format!("`{path}` is a project config or the lock: never delegated"),
        );
    }
    if let Some(detail) = paths.iter().find_map(|path| rule.excluded_zone(path)) {
        return Verdict::hold(Reason::ExcludedZone, detail);
    }
    if let Some(detail) = ids
        .unwrap_or_default()
        .iter()
        .find_map(|id| rule.excluded_kind(id))
    {
        return Verdict::hold(Reason::ExcludedKind, detail);
    }
    if let Some(detail) = declared.iter().find_map(|(path, ids)| {
        ids.iter()
            .find_map(|id| rule.excluded_kind_as(id, &format!("`{id}` in `{path}`")))
    }) {
        return Verdict::hold(Reason::ExcludedKind, detail);
    }
    if let Some(detail) = paths.iter().find_map(|path| rule.uncovered(path)) {
        return Verdict::hold(Reason::Uncovered, detail);
    }
    if let Some(id) = unresolved.first() {
        return Verdict::hold(
            Reason::Uncovered,
            format!("`{id}` resolves to no file: uncovered"),
        );
    }
    Verdict::Apply
}

/// The rule of an item: the switches, then (locally) its pack.
fn item_rule<'a>(
    switches: &Switches<'a>,
    pack: Option<&PackFacts<'a>>,
) -> Result<Effective<'a>, Verdict> {
    let now = switched(switches, pack).map_err(|(reason, why)| Verdict::hold(reason, why))?;
    Ok(effective(pack.map(|pack| (pack.frozen, pack.seq)), now))
}

/// A proposal's apply (see the module documentation): the switches,
/// `not-in-pack` (local), `author` (hub), `proposal-kind`, `staged`, the
/// paths and IDs, `findings`, `rebases`, then `cap` (hub, `applies_today`
/// the count of today's delegated applies). `now` is unused by the rows an
/// apply takes (`until` is an approve's and an implement's: a land still
/// runs); kept for one signature.
pub fn judge_proposal(
    switches: &Switches<'_>,
    pack: Option<&PackFacts<'_>>,
    facts: &ProposalFacts,
    applies_today: Option<u32>,
    now: &str,
) -> Verdict {
    let _ = now;
    let rule = match item_rule(switches, pack) {
        Ok(rule) => rule,
        Err(hold) => return hold,
    };
    let id = &facts.id;
    match switches.surface {
        Surface::Local if pack.is_none() => {
            return Verdict::hold(
                Reason::NotInPack,
                format!("`{id}` is bound to no live pack's task"),
            );
        }
        Surface::Hub => {
            let login = switches
                .logins
                .ok()
                .and_then(|logins| logins.first())
                .map(|(login, _)| login.as_str())
                .unwrap_or_default();
            let by_owner = facts.author_type == AuthorType::Agent
                && facts.author_login.as_deref() == Some(login);
            if !by_owner {
                return Verdict::hold(
                    Reason::Author,
                    format!("`{id}` was not raised over /mcp by `{login}`"),
                );
            }
        }
        Surface::Local => {}
    }
    if facts.kind != "update" && facts.kind != "create" {
        return Verdict::hold(
            Reason::ProposalKind,
            format!("{} waits for a person", with_article(&facts.kind)),
        );
    }
    if let Some((staged, at)) = &facts.staged {
        let which = match staged {
            StagedAs::Approve => "an approve",
            StagedAs::Reject => "a reject",
        };
        return Verdict::hold(
            Reason::Staged,
            format!("`{id}` has {which} staged at {at}: a person is deciding"),
        );
    }
    let paths = judge_paths(&rule, &facts.paths, Some(&facts.ids), &[], &[]);
    if paths != Verdict::Apply {
        return paths;
    }
    if let Some((code, path, line)) = facts.findings.first() {
        return Verdict::hold(
            Reason::Findings,
            format!(
                "{} finding(s) stored at creation, the first {code} at {path}:{line}",
                facts.findings.len()
            ),
        );
    }
    let rebases = match facts.text {
        TextState::Applies => None,
        TextState::Rebases => Some("its span changed since its base: it would rebase".to_owned()),
        TextState::Conflict => Some("it conflicts with the span now".to_owned()),
        TextState::Gone => Some("its target is gone".to_owned()),
        TextState::FileExists => Some(format!(
            "`{}` exists: a new file's create needs none",
            facts.paths.first().map(String::as_str).unwrap_or_default()
        )),
    };
    if let Some(detail) = rebases {
        return Verdict::hold(Reason::Rebases, detail);
    }
    if switches.surface == Surface::Hub {
        let count = applies_today.unwrap_or(0);
        let cap = rule.max_applies();
        if count >= cap {
            return Verdict::hold(
                Reason::Cap,
                format!("{count} delegated applies today (UTC), the cap max_applies {cap}"),
            );
        }
    }
    Verdict::Apply
}

/// The pack rows every task act takes after the switches: `not-in-pack`
/// (local), then `until` when `until_counts`.
fn task_pack_rows(
    switches: &Switches<'_>,
    facts: &TaskFacts<'_>,
    until_counts: bool,
    now: &str,
) -> Option<Verdict> {
    let id = &facts.id;
    if switches.surface == Surface::Local && facts.pack.is_none() {
        return Some(Verdict::hold(
            Reason::NotInPack,
            format!("`{id}` is in no live pack"),
        ));
    }
    if until_counts
        && let Some(pack) = &facts.pack
        && let Some(until) = pack.until
        && until <= now
    {
        return Some(Verdict::hold(
            Reason::Until,
            format!("pack {} ended at {until}", pack.seq),
        ));
    }
    None
}

/// `` `<T>` is <status>: `<act>` needs <state> ``.
fn status_hold(facts: &TaskFacts<'_>, act: &str, state: TaskStatus) -> Verdict {
    Verdict::hold(
        Reason::Status,
        format!(
            "`{}` is {}: `{act}` needs {}",
            facts.id,
            facts.status.as_str(),
            state.as_str()
        ),
    )
}

/// A task's approve: the switches, `not-in-pack`, `until`, `status` (it
/// needs `review`), then the paths and IDs of its targets and affected
/// nodes.
pub fn judge_approve(switches: &Switches<'_>, facts: &TaskFacts<'_>, now: &str) -> Verdict {
    let rule = match item_rule(switches, facts.pack.as_ref()) {
        Ok(rule) => rule,
        Err(hold) => return hold,
    };
    if let Some(hold) = task_pack_rows(switches, facts, true, now) {
        return hold;
    }
    if facts.status != TaskStatus::Review {
        return status_hold(facts, "approve", TaskStatus::Review);
    }
    judge_paths(
        &rule,
        &facts.paths,
        Some(&facts.ids),
        &facts.unresolved,
        &[],
    )
}

/// An act judged by its pack alone (a plan, an implement, a line's plan):
/// the switches, `not-in-pack` and, when `until_counts`, `until`.
pub fn judge_pack_act(
    switches: &Switches<'_>,
    facts: &TaskFacts<'_>,
    now: &str,
    until_counts: bool,
) -> Verdict {
    if let Err(hold) = item_rule(switches, facts.pack.as_ref()) {
        return hold;
    }
    task_pack_rows(switches, facts, until_counts, now).unwrap_or(Verdict::Apply)
}

/// The path rows alone of a task's approve (no state, no pack's `until`):
/// `spec auto start`'s hint on each task.
pub fn judge_task_paths(switches: &Switches<'_>, facts: &TaskFacts<'_>) -> Verdict {
    let rule = match item_rule(switches, facts.pack.as_ref()) {
        Ok(rule) => rule,
        Err(hold) => return hold,
    };
    judge_paths(
        &rule,
        &facts.paths,
        Some(&facts.ids),
        &facts.unresolved,
        &[],
    )
}

/// A task's land: the switches, `not-in-pack`, `status` (it needs `done`),
/// `run-outcome` (its last run `completed`), then the paths `changed`:
/// `config-target`, `excluded-zone`, `uncovered`. A pack's `until` holds no
/// land.
pub fn judge_land_paths(
    switches: &Switches<'_>,
    facts: &TaskFacts<'_>,
    changed: &[String],
) -> Verdict {
    judge_land(switches, facts, changed, &[])
}

/// [`judge_land_paths`], and between `excluded-zone` and `uncovered` the
/// IDs each changed spec document declares at the merged tree (`declared`:
/// its path and IDs, in the paths' order): one of an excluded kind holds
/// `excluded-kind`, naming the ID and the file. File-level on purpose: a
/// document of an excluded kind stays the owner's.
pub fn judge_land(
    switches: &Switches<'_>,
    facts: &TaskFacts<'_>,
    changed: &[String],
    declared: &[(String, Vec<String>)],
) -> Verdict {
    let rule = match item_rule(switches, facts.pack.as_ref()) {
        Ok(rule) => rule,
        Err(hold) => return hold,
    };
    if let Some(hold) = task_pack_rows(switches, facts, false, "") {
        return hold;
    }
    if facts.status != TaskStatus::Done {
        return status_hold(facts, "land", TaskStatus::Done);
    }
    match facts.last_run {
        None => {
            return Verdict::hold(Reason::RunOutcome, format!("`{}` has no run", facts.id));
        }
        Some((run, outcome)) if outcome != RunOutcome::Completed => {
            return Verdict::hold(
                Reason::RunOutcome,
                format!(
                    "`{}`'s run {run} ended {}, not completed",
                    facts.id,
                    outcome.as_str()
                ),
            );
        }
        Some(_) => {}
    }
    judge_paths(&rule, changed, None, &[], declared)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auto_toml::AutoRule;

    const BASE: &str = "[project]\nslug = \"auto-mini\"\n\n[ids]\nXR = { kind = \"requirement\", width = 2 }\nXS = { kind = \"scenario\", width = 3 }\nXD = { kind = \"decision\", width = 4 }\n\n[zones]\ncode = [\"src/**\"]\ntests = [\"tests/**\"]\ndocs = [\"docs/**\"]\nnarrative = [\"docs/narrative/**\"]\n";

    fn rule(table: &str) -> AutoRule {
        let text = format!("{BASE}\n[auto]\n{table}");
        AutoRule::from_toml(text.as_bytes(), "specengine.toml")
            .rule
            .expect("a rule")
    }

    fn on() -> AutoRule {
        rule(
            "mode = \"both\"\nzones = [\"code\", \"tests\", \"docs\"]\nexclude_zones = [\"narrative\"]\nexclude_kinds = [\"scenario\"]\n",
        )
    }

    const OWNER: [(&str, TeamRole); 1] = [("molo", TeamRole::Owner)];

    fn logins(entries: &[(&str, TeamRole)]) -> Vec<(String, TeamRole)> {
        entries
            .iter()
            .map(|(login, role)| ((*login).to_owned(), *role))
            .collect()
    }

    fn update(path: &str, id: &str) -> ProposalFacts {
        ProposalFacts {
            id: "PR-0001".to_owned(),
            kind: "update".to_owned(),
            staged: None,
            author_type: AuthorType::Agent,
            author_login: Some("molo".to_owned()),
            pack: Some(7),
            paths: vec![path.to_owned()],
            ids: vec![id.to_owned()],
            findings: Vec::new(),
            text: TextState::Applies,
        }
    }

    fn task<'a>(status: TaskStatus, pack: Option<PackFacts<'a>>) -> TaskFacts<'a> {
        TaskFacts {
            id: "T-0001".to_owned(),
            status,
            pack,
            paths: vec!["src/lib.rs".to_owned()],
            ids: vec!["XR-01".to_owned()],
            unresolved: Vec::new(),
            last_run: Some((1, RunOutcome::Completed)),
        }
    }

    fn held(verdict: Verdict) -> (Reason, String) {
        match verdict {
            Verdict::Hold { reason, detail } => (reason, detail),
            Verdict::Apply => panic!("applied"),
        }
    }

    #[test]
    fn every_proposal_reason() {
        let rule = on();
        let owner = logins(&OWNER);
        let local = Switches {
            surface: Surface::Local,
            rule: Ok(&rule),
            logins: Ok(&owner),
            stopped: None,
        };
        let pack = PackFacts {
            seq: 7,
            until: None,
            frozen: &rule,
        };
        let now = "2026-11-15T00:00:00Z";
        let base = update("src/lib.rs", "XR-01");
        assert_eq!(
            judge_proposal(&local, Some(&pack), &base, None, now),
            Verdict::Apply
        );
        assert_eq!(
            held(judge_proposal(
                &Switches {
                    rule: Err("no commit"),
                    ..local
                },
                Some(&pack),
                &base,
                None,
                now
            )),
            (Reason::Off, "no commit".to_owned())
        );
        let two = logins(&[("a", TeamRole::Owner), ("b", TeamRole::Member)]);
        assert_eq!(
            held(judge_proposal(
                &Switches {
                    logins: Ok(&two),
                    ..local
                },
                Some(&pack),
                &base,
                None,
                now
            )),
            (
                Reason::Team,
                "[team] names 2 logins: auto stops at two".to_owned()
            )
        );
        assert_eq!(
            held(judge_proposal(
                &Switches {
                    stopped: Some("/d/auto-stop"),
                    ..local
                },
                Some(&pack),
                &base,
                None,
                now
            )),
            (
                Reason::Stopped,
                "the stop file /d/auto-stop exists: spec auto resume removes it".to_owned()
            )
        );
        assert_eq!(
            held(judge_proposal(&local, None, &base, None, now)),
            (
                Reason::NotInPack,
                "`PR-0001` is bound to no live pack's task".to_owned()
            )
        );
        let hub = Switches {
            surface: Surface::Hub,
            ..local
        };
        let mut human = base.clone();
        human.author_type = AuthorType::Human;
        assert_eq!(
            held(judge_proposal(&hub, None, &human, Some(0), now)),
            (
                Reason::Author,
                "`PR-0001` was not raised over /mcp by `molo`".to_owned()
            )
        );
        let mut question = base.clone();
        question.kind = "question".to_owned();
        assert_eq!(
            held(judge_proposal(&local, Some(&pack), &question, None, now)),
            (
                Reason::ProposalKind,
                "a question waits for a person".to_owned()
            )
        );
        let mut staged = base.clone();
        staged.staged = Some((StagedAs::Reject, "2026-11-14T10:00:00Z".to_owned()));
        assert_eq!(
            held(judge_proposal(&local, Some(&pack), &staged, None, now)),
            (
                Reason::Staged,
                "`PR-0001` has a reject staged at 2026-11-14T10:00:00Z: a person is deciding"
                    .to_owned()
            )
        );
        assert_eq!(
            held(judge_proposal(
                &local,
                Some(&pack),
                &update("sub/SpecEngine.TOML", "XR-01"),
                None,
                now
            )),
            (
                Reason::ConfigTarget,
                "`sub/SpecEngine.TOML` is a project config or the lock: never delegated".to_owned()
            )
        );
        assert_eq!(
            held(judge_proposal(
                &local,
                Some(&pack),
                &update("docs/narrative/intro.md", "XR-01"),
                None,
                now
            )),
            (
                Reason::ExcludedZone,
                "`docs/narrative/intro.md` is in the excluded zone `narrative`".to_owned()
            )
        );
        assert_eq!(
            held(judge_proposal(
                &local,
                Some(&pack),
                &update("docs/spec/core.md", "XS-001"),
                None,
                now
            )),
            (
                Reason::ExcludedKind,
                "`XS-001` is a scenario, an excluded kind".to_owned()
            )
        );
        assert_eq!(
            held(judge_proposal(
                &local,
                Some(&pack),
                &update("other/x.md", "XR-01"),
                None,
                now
            )),
            (
                Reason::Uncovered,
                "`other/x.md` is in no zone the rule covers (code, docs, tests)".to_owned()
            )
        );
        let mut findings = base.clone();
        findings.findings = vec![("dangling-link".to_owned(), "src/lib.rs".to_owned(), 3)];
        assert_eq!(
            held(judge_proposal(&local, Some(&pack), &findings, None, now)),
            (
                Reason::Findings,
                "1 finding(s) stored at creation, the first dangling-link at src/lib.rs:3"
                    .to_owned()
            )
        );
        let mut rebases = base.clone();
        rebases.text = TextState::Rebases;
        assert_eq!(
            held(judge_proposal(&local, Some(&pack), &rebases, None, now)),
            (
                Reason::Rebases,
                "its span changed since its base: it would rebase".to_owned()
            )
        );
        assert_eq!(
            held(judge_proposal(&hub, None, &base, Some(20), now)),
            (
                Reason::Cap,
                "20 delegated applies today (UTC), the cap max_applies 20".to_owned()
            )
        );
        assert_eq!(
            judge_proposal(&hub, None, &base, Some(19), now),
            Verdict::Apply
        );
    }

    #[test]
    fn task_reasons() {
        let rule = on();
        let owner = logins(&OWNER);
        let local = Switches {
            surface: Surface::Local,
            rule: Ok(&rule),
            logins: Ok(&owner),
            stopped: None,
        };
        let now = "2026-11-15T00:00:00Z";
        let pack = PackFacts {
            seq: 3,
            until: Some("2026-11-14T00:00:00Z"),
            frozen: &rule,
        };
        assert_eq!(
            held(judge_approve(&local, &task(TaskStatus::Review, None), now)),
            (Reason::NotInPack, "`T-0001` is in no live pack".to_owned())
        );
        assert_eq!(
            held(judge_approve(
                &local,
                &task(TaskStatus::Review, Some(pack)),
                now
            )),
            (
                Reason::Until,
                "pack 3 ended at 2026-11-14T00:00:00Z".to_owned()
            )
        );
        let open = PackFacts {
            until: None,
            ..pack
        };
        assert_eq!(
            held(judge_approve(
                &local,
                &task(TaskStatus::Draft, Some(open)),
                now
            )),
            (
                Reason::Status,
                "`T-0001` is draft: `approve` needs review".to_owned()
            )
        );
        assert_eq!(
            judge_approve(&local, &task(TaskStatus::Review, Some(open)), now),
            Verdict::Apply
        );
        let mut failed = task(TaskStatus::Done, Some(open));
        failed.last_run = Some((2, RunOutcome::Failed));
        assert_eq!(
            held(judge_land_paths(
                &local,
                &failed,
                &["src/lib.rs".to_owned()]
            )),
            (
                Reason::RunOutcome,
                "`T-0001`'s run 2 ended failed, not completed".to_owned()
            )
        );
        let done = task(TaskStatus::Done, Some(pack));
        assert_eq!(
            judge_land_paths(&local, &done, &["src/lib.rs".to_owned()]),
            Verdict::Apply
        );
        let mut unresolved = task(TaskStatus::Review, Some(open));
        unresolved.unresolved = vec!["XR-09".to_owned()];
        assert_eq!(
            held(judge_approve(&local, &unresolved, now)),
            (
                Reason::Uncovered,
                "`XR-09` resolves to no file: uncovered".to_owned()
            )
        );
    }

    #[test]
    fn frozen_and_now() {
        let wide = rule("mode = \"local\"\nzones = [\"code\", \"docs\"]\n");
        let narrow = rule("mode = \"local\"\nzones = [\"code\"]\n");
        let none: Vec<(String, TeamRole)> = Vec::new();
        let at = |now: &'static AutoRule| Switches {
            surface: Surface::Local,
            rule: Ok(now),
            logins: Ok(&[]),
            stopped: None,
        };
        let _ = &none;
        let wide: &'static AutoRule = Box::leak(Box::new(wide));
        let narrow: &'static AutoRule = Box::leak(Box::new(narrow));
        let docs = update("docs/spec/core.md", "XR-01");
        let pack = |frozen| PackFacts {
            seq: 812,
            until: None,
            frozen,
        };
        assert_eq!(
            held(judge_proposal(
                &at(narrow),
                Some(&pack(wide)),
                &docs,
                None,
                ""
            )),
            (
                Reason::Uncovered,
                "`docs/spec/core.md` is in no zone the rule covers (code) (the rule at HEAD)"
                    .to_owned()
            )
        );
        assert_eq!(
            held(judge_proposal(
                &at(wide),
                Some(&pack(narrow)),
                &docs,
                None,
                ""
            )),
            (
                Reason::Uncovered,
                "`docs/spec/core.md` is in no zone the rule covers (code) (the rule at pack \
                 812's start)"
                    .to_owned()
            )
        );
        let moved: &'static AutoRule = Box::leak(Box::new(rule(
            "mode = \"local\"\nzones = [\"code\", \"docs\"]\nbranch = \"night\"\n",
        )));
        assert_eq!(
            held(judge_proposal(
                &at(moved),
                Some(&pack(wide)),
                &docs,
                None,
                ""
            )),
            (
                Reason::Off,
                "the rule's branch changed since pack 812 started (`auto` -> `night`)".to_owned()
            )
        );
    }

    #[test]
    fn switches_on_a_hub() {
        let rule = on();
        let lead = logins(&[("mira", TeamRole::Lead)]);
        let hub = Switches {
            surface: Surface::Hub,
            rule: Ok(&rule),
            logins: Ok(&lead),
            stopped: None,
        };
        assert_eq!(
            judge_switches(&hub),
            Err((
                Reason::Team,
                "the sole [team] login `mira` is a lead: a hub delegates only to the owner"
                    .to_owned()
            ))
        );
        assert_eq!(
            judge_switches(&Switches {
                logins: Ok(&[]),
                ..hub
            }),
            Err((
                Reason::Team,
                "[team] names no login: a hub delegates only to its sole owner".to_owned()
            ))
        );
        let local_rule = rule.clone();
        let mut local_only = local_rule;
        local_only.mode = AutoMode::Local;
        assert_eq!(
            judge_switches(&Switches {
                rule: Ok(&local_only),
                ..hub
            }),
            Err((
                Reason::Off,
                "[auto] mode is local: a hub acts only under hub or both".to_owned()
            ))
        );
    }

    #[test]
    fn outside_paths_and_declared_ids() {
        let rule = on();
        let owner = logins(&OWNER);
        let local = Switches {
            surface: Surface::Local,
            rule: Ok(&rule),
            logins: Ok(&owner),
            stopped: None,
        };
        let pack = PackFacts {
            seq: 7,
            until: None,
            frozen: &rule,
        };
        let done = task(TaskStatus::Done, Some(pack));
        assert_eq!(
            held(judge_land_paths(&local, &done, &["../src/x.rs".to_owned()])),
            (
                Reason::Uncovered,
                "`../src/x.rs` is in no zone the rule covers (code, docs, tests)".to_owned()
            )
        );
        assert_eq!(
            held(judge_land_paths(
                &local,
                &done,
                &["../specengine.toml".to_owned()]
            ))
            .0,
            Reason::ConfigTarget
        );
        let declared = vec![(
            "docs/spec/core.md".to_owned(),
            vec!["XR-01".to_owned(), "XS-002".to_owned()],
        )];
        assert_eq!(
            held(judge_land(
                &local,
                &done,
                &["docs/spec/core.md".to_owned()],
                &declared
            )),
            (
                Reason::ExcludedKind,
                "`XS-002` in `docs/spec/core.md` is a scenario, an excluded kind".to_owned()
            )
        );
        assert!(judge_pack_switches(&local, &pack).is_ok());
    }

    #[test]
    fn lines_paths_and_names() {
        assert_eq!(parse_pack_line("812/1"), Some((812, 1)));
        assert_eq!(parse_pack_line("812/"), None);
        assert_eq!(parse_pack_line("a/1"), None);
        assert_eq!(
            pack_lines("  Sort it \n# skip\n\nShip it\n"),
            Ok(vec!["Sort it".to_owned(), "Ship it".to_owned()])
        );
        assert_eq!(
            pack_lines("ok\nbad\u{7}line\n"),
            Err("line 2: it holds a control character".to_owned())
        );
        assert_eq!(bare_id("slug/XS-001"), "XS-001");
        assert_eq!(bare_id("XS-001"), "XS-001");
        assert_eq!(event_family("task.run_reported"), "task");
        assert_eq!(event_family("auto.held"), "auto");
        assert_eq!(event_family("bogus"), "bogus");
        assert_eq!(event_family(""), "");
        assert_eq!(slash_joined(["a", "b"]), "a/b");
        assert_eq!(slash_joined(Vec::<String>::new()), "");
        assert_eq!(root_relative("", "src/a.rs"), "src/a.rs");
        assert_eq!(root_relative("sub", "sub/src/a.rs"), "src/a.rs");
        assert_eq!(root_relative("sub", "subway/a.rs"), "../subway/a.rs");
        assert_eq!(
            root_relative("a/b", "x/specengine.toml"),
            "../../x/specengine.toml"
        );
    }

    #[test]
    fn config_targets() {
        assert!(is_config_target("specengine.toml"));
        assert!(is_config_target("a/b/SPECENGINE.toml"));
        assert!(is_config_target("spec.lock"));
        assert!(!is_config_target("a/spec.lock"));
        assert!(!is_config_target("src/lib.rs"));
    }

    #[test]
    fn reasons_round_trip() {
        for reason in Reason::ALL {
            assert_eq!(Reason::parse(reason.as_str()), Some(reason));
        }
    }
}
