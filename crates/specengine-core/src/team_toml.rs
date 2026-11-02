//! `[team]` of `specengine.toml` (task spec `hub-team`): which logins of a hub
//! act on a project's queue, in what role. Pure: the text in, the checked
//! values out. Read apart from the reads ([`TeamConfig::from_toml`]):
//! [`crate::ProjectConfig::from_toml`] only name-checks the table, so a bad
//! `[team]` fails no read, no plain write, no `spec card` and no gate. Only a
//! hub's acts past the plain writes and the check's `team-config` read it; a
//! terminal is never judged.
//!
//! - **Entries**: `[team.<login>]`, the key the login byte for byte, closed
//!   keys, at most [`TEAM_ENTRIES_MAX`]. `name` a card person's grammar, shown
//!   only, optional; `role` one of [`TeamRole::ALL`]; `zones` required of a
//!   `lead`, each a `[zones]` key once, refused of the other two; `email`
//!   required.
//! - **A login key** passes [`hub_login_problem`], holds no `<` or `>` and no
//!   character git trims at an end: either would change a trailer's name. An
//!   `email` is one `@` between two non-empty parts, printable ASCII with no
//!   space, no `<` or `>`, under the same end rule.
//! - **`[zones]`** is the gate's table ([`zones_from_toml`]), read here whether
//!   or not `[gate]` is on, and only when a `[team]` exists.
//! - **Roles** name authority over the queue, no trade: `member` every act
//!   but a group clear, `lead` that too inside its zones, `owner` all of it,
//!   an unzoned path too. Nothing here reads a discrepancy, a status or a
//!   flow, and a role is no second control point: with no lead free, items
//!   stay open.
//!
//! Every error is a [`ProjectError`] at its line.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

use serde::Deserialize;
use toml::Spanned;

use crate::card::{card_person, card_person_problem, hub_login_problem};
use crate::gate::{Zone, zones_from_toml};
use crate::project_toml::ProjectError;

/// The most entries of a `[team]`.
pub const TEAM_ENTRIES_MAX: usize = 256;

/// What git trims from either end of a name or an address, which a trailer's
/// reader would then take for another person (spaces and control characters
/// are refused on their own).
const GIT_TRIMS: [char; 7] = ['.', ',', ':', ';', '"', '\'', '\\'];

/// What an entry may do: its authority over the queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TeamRole {
    Owner,
    Lead,
    Member,
}

impl TeamRole {
    pub const ALL: [Self; 3] = [Self::Owner, Self::Lead, Self::Member];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Lead => "lead",
            Self::Member => "member",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|role| role.as_str() == text)
    }

    /// The acts the role opens, as `GET .../team`'s `allowed` names them.
    pub const fn allowed(self) -> &'static [&'static str] {
        match self {
            Self::Member => &["stage", "accept", "clear"],
            Self::Lead | Self::Owner => &["stage", "accept", "clear", "group_clear", "confirm"],
        }
    }

    /// A group clear is open to the role, inside whatever it covers.
    pub const fn clears_groups(self) -> bool {
        matches!(self, Self::Lead | Self::Owner)
    }

    /// A confirm is open to the role, inside whatever it covers.
    pub const fn confirms(self) -> bool {
        matches!(self, Self::Lead | Self::Owner)
    }
}

/// One `[team.<login>]`, checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamEntry {
    /// The key, byte for byte.
    pub login: String,
    /// Shown only, trimmed; never compared.
    pub name: Option<String>,
    pub role: TeamRole,
    /// The zones a `lead` leads, in `zones` order; empty otherwise.
    pub zones: Vec<Zone>,
    pub email: String,
}

impl TeamEntry {
    /// The entry may re-freeze a node or confirm a change at the root-relative
    /// `path`: `owner` anywhere, an unzoned path too; else inside one of its
    /// zones, so a `member`, leading none, nowhere.
    pub fn covers(&self, path: &str) -> bool {
        self.role == TeamRole::Owner || self.zones.iter().any(|zone| zone.covers(path))
    }

    /// Its zones' names, in `zones` order, joined for a refusal.
    pub fn zones_text(&self) -> String {
        let names: Vec<&str> = self.zones.iter().map(|zone| zone.name.as_str()).collect();
        names.join(", ")
    }
}

/// A whole `[team]`, checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamConfig {
    entries: BTreeMap<String, TeamEntry>,
}

impl TeamConfig {
    /// Reads `[team]` and, with it, `[zones]` (see the module documentation).
    /// `None`: no `[team]` table.
    pub fn from_toml(text: &str) -> Result<Option<Self>, ProjectError> {
        team_from_toml(text)
    }

    /// The entry of `login`, compared byte for byte; an absent login is no
    /// entry and no default role.
    pub fn entry(&self, login: &str) -> Option<&TeamEntry> {
        self.entries.get(login)
    }

    /// The entries, by login.
    pub fn entries(&self) -> impl Iterator<Item = &TeamEntry> {
        self.entries.values()
    }
}

/// [`TeamConfig::from_toml`] as a function.
pub fn team_from_toml(text: &str) -> Result<Option<TeamConfig>, ProjectError> {
    let raw: RawTeamFile = toml::from_str(text).map_err(|error| ProjectError {
        line: Some(error.span().map_or(1, |span| line_of(text, span.start))),
        message: error.message().trim().to_owned(),
    })?;
    let Some(table) = raw.team else {
        return Ok(None);
    };
    let reader = Reader {
        text,
        declared: zones_from_toml(text)?,
    };
    if table.get_ref().len() > TEAM_ENTRIES_MAX {
        return Err(reader.at(
            table.span(),
            format!(
                "`[team]` has {} entries; at most {TEAM_ENTRIES_MAX}",
                table.get_ref().len()
            ),
        ));
    }
    let mut entries = BTreeMap::new();
    for (login, entry) in table.into_inner() {
        let entry = reader.entry(login.clone(), entry)?;
        entries.insert(login, entry);
    }
    Ok(Some(TeamConfig { entries }))
}

/// What one reading of the file holds on to.
struct Reader<'t> {
    text: &'t str,
    declared: BTreeMap<String, Zone>,
}

impl Reader<'_> {
    /// `message` at `span`'s first line, a [`ProjectError`] always with one.
    fn at(&self, span: Range<usize>, message: String) -> ProjectError {
        ProjectError {
            line: Some(line_of(self.text, span.start)),
            message,
        }
    }

    fn entry(&self, login: String, entry: Spanned<RawEntry>) -> Result<TeamEntry, ProjectError> {
        let span = entry.span();
        let raw = entry.into_inner();
        if let Some(problem) = login_problem(&login) {
            return Err(self.at(span, format!("`[team]` login {login:?}: {problem}")));
        }
        let role_span = raw.role.span();
        let Some(role) = TeamRole::parse(raw.role.get_ref()) else {
            return Err(self.at(
                role_span,
                format!(
                    "`[team] {login}` role {:?}: `owner`, `lead` or `member`",
                    raw.role.get_ref()
                ),
            ));
        };
        let name = match raw.name {
            None => None,
            Some(name) => {
                let at = name.span();
                let written = name.into_inner();
                if let Some(problem) = card_person_problem(&written) {
                    return Err(self.at(at, format!("`[team] {login}` name: {problem}")));
                }
                Some(card_person(&written).to_owned())
            }
        };
        let zones = self.zones(&login, role, span, raw.zones)?;
        let email_span = raw.email.span();
        let email = raw.email.into_inner();
        if let Some(problem) = email_problem(&email) {
            return Err(self.at(
                email_span,
                format!("`[team] {login}` email {email:?}: {problem}"),
            ));
        }
        Ok(TeamEntry {
            login,
            name,
            role,
            zones,
            email,
        })
    }

    /// An entry's `zones`: required of a `lead`, each declared and named
    /// once; any other role refuses the key, an empty list too.
    fn zones(
        &self,
        login: &str,
        role: TeamRole,
        entry: Range<usize>,
        written: Option<Spanned<Vec<Spanned<String>>>>,
    ) -> Result<Vec<Zone>, ProjectError> {
        if role != TeamRole::Lead {
            return match written {
                Some(list) => Err(self.at(
                    list.span(),
                    format!(
                        "`[team] {login}` is `{}`: only a `lead` takes `zones`",
                        role.as_str()
                    ),
                )),
                None => Ok(Vec::new()),
            };
        }
        let empty = format!("`[team] {login}` is a `lead`: `zones` names what it leads");
        let Some(list) = written else {
            return Err(self.at(entry, empty));
        };
        if list.get_ref().is_empty() {
            return Err(self.at(list.span(), empty));
        }
        let mut seen = BTreeSet::new();
        let mut zones = Vec::with_capacity(list.get_ref().len());
        for name in list.into_inner() {
            let at = name.span();
            let name = name.into_inner();
            let Some(zone) = self.declared.get(&name) else {
                return Err(self.at(
                    at,
                    format!("`[team] {login}` lists `{name}`, which `[zones]` does not declare"),
                ));
            };
            if !seen.insert(name.clone()) {
                return Err(self.at(at, format!("`[team] {login}` lists `{name}` twice")));
            }
            zones.push(zone.clone());
        }
        Ok(zones)
    }
}

/// Why `login` can head no `[team]` entry: [`hub_login_problem`], or what a
/// trailer would change.
fn login_problem(login: &str) -> Option<String> {
    hub_login_problem(login)
        .or_else(|| {
            login
                .contains(['<', '>'])
                .then(|| "`<` or `>`, which close an address in a trailer".to_owned())
        })
        .or_else(|| end_problem(login))
}

/// Why `email` is no address (see the module documentation).
fn email_problem(email: &str) -> Option<String> {
    if !email.bytes().all(|byte| byte.is_ascii_graphic()) {
        return Some("printable ASCII, no space".to_owned());
    }
    if email.contains(['<', '>']) {
        return Some("`<` or `>`, which close an address in a trailer".to_owned());
    }
    match email.split_once('@') {
        Some((local, domain))
            if !local.is_empty() && !domain.is_empty() && !domain.contains('@') =>
        {
            end_problem(email)
        }
        _ => Some("one `@` between two parts".to_owned()),
    }
}

/// `text` opens or ends in a character [`GIT_TRIMS`] names.
fn end_problem(text: &str) -> Option<String> {
    [text.chars().next(), text.chars().next_back()]
        .into_iter()
        .flatten()
        .find(|c| GIT_TRIMS.contains(c))
        .map(|c| format!("`{c}` at an end, which git trims"))
}

/// Only `[team]`, by login.
#[derive(Deserialize)]
struct RawTeamFile {
    #[serde(default)]
    team: Option<Spanned<BTreeMap<String, Spanned<RawEntry>>>>,
}

/// One `[team.<login>]`; any other key is an error at its line. The name is
/// what a value of another type is told it should have been.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename = "team table")]
struct RawEntry {
    #[serde(default)]
    name: Option<Spanned<String>>,
    role: Spanned<String>,
    #[serde(default)]
    zones: Option<Spanned<Vec<Spanned<String>>>>,
    email: Spanned<String>,
}

fn line_of(text: &str, offset: usize) -> usize {
    let end = offset.min(text.len());
    text.as_bytes()[..end]
        .iter()
        .filter(|&&byte| byte == b'\n')
        .count()
        + 1
}
