//! A hub's `[team]` (task spec `hub-team`): read per request from the root the
//! route's slug names, with no cache, by the acts past the plain writes alone
//! and by `GET .../team`, the asker's own row. No read, move, `by`, status,
//! flow, gate or check of a discrepancy reads it, so a bad `[team]` stops none
//! of them; a terminal is never judged, a shell on the host being the
//! operator's. Nothing here writes.

use serde::Serialize;
use specengine_core::TeamConfig;

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
