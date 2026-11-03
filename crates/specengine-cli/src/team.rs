//! A hub's `[team]` (task spec `hub-team`): read per request from the root the
//! route's slug names, with no cache, by the acts past the plain writes alone
//! and by `GET .../team`, the asker's own row. No read, move, `by`, status,
//! flow, gate or check of a discrepancy reads it, so a bad `[team]` stops none
//! of them; a terminal is never judged, a shell on the host being the
//! operator's. Nothing here writes.
//!
//! `spec team` (task spec `hub-accounts`, "`spec team`") prints the table
//! for the deployment's account script to compare, read-only: one
//! `<login>\t<role>\t<email>` line per entry in byte order of the login, or
//! nothing with no `[team]`; `--json` `{"team": [...]}`, `null` with none.
//! It opens no index; a bad `[team]` exits 2, `spec: ` and [`team_read`]'s
//! line.

use serde::Serialize;
use specengine_core::{TeamConfig, TeamEntry};

use crate::project::{config_error, discover_with_text};
use crate::proposals::escaped_error;
use crate::{CliError, Env, Globals};

/// The project's `[team]` as its config reads now; `None` no such table.
/// Exit 2 a bad one, `<config>:<line>: <message>`, the daemon's 503.
pub fn team_read(env: &Env, globals: &Globals) -> Result<Option<TeamConfig>, CliError> {
    let (project, text) = discover_with_text(env, globals)?;
    TeamConfig::from_toml(&text)
        .map_err(|error| escaped_error(config_error(&project.config_label, &error)))
}

/// What `spec team` printed: the entries by login, `None` no `[team]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TeamOutcome {
    pub team: Option<Vec<TeamEntryRow>>,
}

/// One `[team.<login>]` as `spec team --json` prints it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TeamEntryRow {
    /// The key, byte for byte.
    pub login: String,
    pub name: Option<String>,
    pub role: &'static str,
    /// A `lead`'s zones in `zones` order; empty otherwise.
    pub zones: Vec<String>,
    pub email: String,
}

impl TeamEntryRow {
    fn of(entry: &TeamEntry) -> Self {
        Self {
            login: entry.login.clone(),
            name: entry.name.clone(),
            role: entry.role.as_str(),
            zones: entry.zones.iter().map(|zone| zone.name.clone()).collect(),
            email: entry.email.clone(),
        }
    }
}

/// `spec team` (see the module documentation).
pub fn team(env: &Env, globals: &Globals) -> Result<TeamOutcome, CliError> {
    let team = team_read(env, globals).map_err(spec_prefixed)?;
    Ok(TeamOutcome {
        team: team.map(|team| team.entries().map(TeamEntryRow::of).collect()),
    })
}

/// `spec team`'s refusal is one `spec: ` line: a config error's
/// `<config>:<line>: <message>` after the prefix (task spec `hub-accounts`,
/// "`spec team`"), as a missing project's already is.
fn spec_prefixed(error: CliError) -> CliError {
    if error.message.starts_with("spec: ") {
        return error;
    }
    CliError {
        exit: error.exit,
        message: format!("spec: {}", error.message),
    }
}

/// One `<login>\t<role>\t<email>` line per entry. Neither a login
/// (`hub_login_problem`) nor an address holds a control character, so a
/// tab or a line break never occurs inside a field.
pub(crate) fn render_text(outcome: &TeamOutcome) -> String {
    outcome
        .team
        .iter()
        .flatten()
        .map(|row| format!("{}\t{}\t{}\n", row.login, row.role, row.email))
        .collect()
}

/// `GET .../team`: what a page may offer `login`, which the route's refusal
/// still decides. Outside `[team]`, or with none: `name` null, `allowed`
/// empty.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TeamRow {
    pub login: String,
    pub name: Option<String>,
    pub allowed: &'static [&'static str],
}

impl TeamRow {
    /// `login`'s row in `team`.
    pub fn of(team: Option<&TeamConfig>, login: String) -> Self {
        let entry = team.and_then(|team| team.entry(&login));
        Self {
            name: entry.and_then(|entry| entry.name.clone()),
            allowed: entry.map_or(&[], |entry| entry.role.allowed()),
            login,
        }
    }
}
