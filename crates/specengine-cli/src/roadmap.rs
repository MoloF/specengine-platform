//! `spec roadmap [--release R | --pool] [--json]` (canon `spec-roadmap`,
//! "Command"): the plan of the project's `[roadmap]`
//! items, computed per read from the repository's files alone (core's
//! [`specengine_core::plan`]) over the index the call updates, as `spec
//! tree` reads it; never the queue. It writes nothing under the project
//! root.
//!
//! `--release R` lists that release's group, `--pool` the pool and the
//! unplaced items, neither every group; the first line and the JSON's
//! top-level keys are always the whole plan's.
//!
//! ```text
//! roadmap: <o> open, <d> done; current <release or none>[; to <cut> <sum>]; total <sum>
//! release <name>[ | current][ | mvp cut] | <o> open, <d> done | <sum>
//!   <position>. <ID> | <title or -> | <status or -> | <priority or -> | <estimate or ->[ | waits on <ID>, …][ | cycle] | <path>
//! pool | <o> open, <d> done | <sum>
//!   - <ID> | <title or -> | <status or -> | <priority or -> | <estimate or ->[ | waits on <ID>, …] | <path>
//! unplaced | <o> open, <d> done | <sum>
//!   - <ID> | … as pool … | release <as written, or mistyped>
//! [truncated: <k> of <n> items not shown; add --release R or --pool]
//! ```
//!
//! `<sum>`: `<min>-<max>` (`<min>` when equal), ` <unit>` when set, `, <u>
//! unestimated` when some are. The lines are core's ([`Plan::summary`],
//! `PlanGroup::header`, `PlanItem::line`), shared with the index entry's
//! roadmap view. Everything before the tail is at most
//! [`OUTPUT_CAP_CHARS`] characters: lines print whole, in order, until the
//! next would pass it; the tail only when an item is not shown
//! (canon `spec-roadmap`, "Cut"); the JSON holds what the text prints. The plan's
//! notes go to stderr as `note:` lines and into the JSON's `notes`; none
//! changes the exit. Exit 1: `--release R` names no release (stderr, and
//! the JSON with `reason` and no group); exit 2: no project, a config
//! error, no or a bad `[roadmap]`, an index that cannot be updated.

use serde::Serialize;
use specengine_core::{GroupKind, Plan, RoadmapConfig, Sum, plan};

use crate::cap::OUTPUT_CAP_CHARS;
use crate::corpus::indexed;
use crate::project::{config_error, discover_with_text};
use crate::{CliError, Env, Globals, Message};

/// `spec roadmap` options.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RoadmapRequest {
    /// `--release R`: that release's group only.
    pub release: Option<String>,
    /// `--pool`: the pool and the unplaced items only.
    pub pool: bool,
}

/// What `spec roadmap` computed and how much of it prints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoadmapOutcome {
    /// `--release R` as given.
    pub release: Option<String>,
    /// `--pool`.
    pub pool: bool,
    /// Why `--release R` lists nothing (exit 1).
    pub reason: Option<String>,
    /// The index update's notes and warnings, then the plan's notes.
    pub messages: Vec<Message>,
    /// `[roadmap] prefix`.
    pub prefix: String,
    /// `[roadmap] dir`.
    pub dir: String,
    /// `[roadmap] unit`.
    pub unit: Option<String>,
    /// The whole plan.
    pub plan: Plan,
    /// The groups the request lists, as indexes into `plan.groups`.
    pub listed: Vec<usize>,
    /// The groups whose header prints, each with how many of its items
    /// print, in order.
    pub shown: Vec<(usize, usize)>,
    /// A listed item does not print (`k` > 0): the tail and the JSON's
    /// `truncated`.
    pub truncated: bool,
}

impl RoadmapOutcome {
    /// The items the request lists (`n` of the tail).
    pub fn requested(&self) -> usize {
        self.listed
            .iter()
            .map(|&group| self.plan.groups[group].items.len())
            .sum()
    }

    /// The listed items that do not print (`k` of the tail).
    pub fn omitted(&self) -> usize {
        let shown: usize = self.shown.iter().map(|&(_, items)| items).sum();
        self.requested().saturating_sub(shown)
    }
}

/// `spec roadmap`: reads `[roadmap]`, updates the index, computes the plan.
pub fn roadmap(
    env: &Env,
    globals: &Globals,
    request: &RoadmapRequest,
) -> Result<RoadmapOutcome, CliError> {
    if request.release.is_some() && request.pool {
        return Err(CliError::spec(
            "--release and --pool cannot be used together",
        ));
    }
    let (project, text) = discover_with_text(env, globals)?;
    let config = RoadmapConfig::from_toml(&text, &project.config.scheme)
        .map_err(|error| config_error(&project.config_label, &error))?
        .ok_or_else(|| CliError::spec(format!("{} has no [roadmap]", project.config_label)))?;
    project.slug()?;
    let mut messages = Vec::new();
    let input = indexed(env, &project, &mut messages, true)?;
    let plan = plan(&input, &config);
    messages.extend(plan.notes.iter().cloned().map(Message::Note));

    let mut outcome = RoadmapOutcome {
        release: request.release.clone(),
        pool: request.pool,
        reason: None,
        messages,
        prefix: config.prefix.clone(),
        dir: config.dir.clone(),
        unit: config.unit.clone(),
        listed: Vec::new(),
        shown: Vec::new(),
        truncated: false,
        plan,
    };
    let groups = &outcome.plan.groups;
    outcome.listed = match &request.release {
        Some(release) => {
            let found = groups.iter().position(|group| {
                group.kind == GroupKind::Release && group.name.as_ref() == Some(release)
            });
            match found {
                Some(group) => vec![group],
                None => {
                    outcome.reason = Some(format!(
                        "no release \"{release}\" in [roadmap] releases ({})",
                        config.releases.join(", ")
                    ));
                    return Ok(outcome);
                }
            }
        }
        None if request.pool => (0..groups.len())
            .filter(|&group| groups[group].kind != GroupKind::Release)
            .collect(),
        None => (0..groups.len()).collect(),
    };
    cut(&mut outcome);
    Ok(outcome)
}

/// The lines that print within [`OUTPUT_CAP_CHARS`] after the first;
/// truncated only when a listed item does not print (an empty group's
/// header past the cap alone is no truncation).
fn cut(outcome: &mut RoadmapOutcome) {
    let unit = outcome.unit.as_deref();
    let mut used = summary(outcome).chars().count() + 1;
    let mut shown = Vec::new();
    'groups: for &at in &outcome.listed {
        let group = &outcome.plan.groups[at];
        let cost = group.header(unit).chars().count() + 1;
        if used + cost > OUTPUT_CAP_CHARS {
            break;
        }
        used += cost;
        let mut items = 0;
        for item in &group.items {
            let cost = item.line(group.kind).chars().count() + 1;
            if used + cost > OUTPUT_CAP_CHARS {
                shown.push((at, items));
                break 'groups;
            }
            used += cost;
            items += 1;
        }
        shown.push((at, items));
    }
    outcome.shown = shown;
    outcome.truncated = outcome.omitted() > 0;
}

/// The first line: the whole plan's (core [`Plan::summary`]).
fn summary(outcome: &RoadmapOutcome) -> String {
    outcome.plan.summary(outcome.unit.as_deref())
}

pub(crate) fn render_text(outcome: &RoadmapOutcome) -> String {
    if outcome.reason.is_some() {
        return String::new();
    }
    let unit = outcome.unit.as_deref();
    let mut out = summary(outcome);
    out.push('\n');
    for &(at, items) in &outcome.shown {
        let group = &outcome.plan.groups[at];
        out.push_str(&group.header(unit));
        out.push('\n');
        for item in &group.items[..items.min(group.items.len())] {
            out.push_str(&item.line(group.kind));
            out.push('\n');
        }
    }
    if outcome.truncated {
        out.push_str(&format!(
            "[truncated: {} of {} items not shown; add --release R or --pool]\n",
            outcome.omitted(),
            outcome.requested()
        ));
    }
    out
}

#[derive(Serialize)]
struct RoadmapJson<'a> {
    release: Option<&'a str>,
    pool: bool,
    reason: Option<&'a str>,
    prefix: &'a str,
    dir: &'a str,
    unit: Option<&'a str>,
    current: Option<&'a str>,
    counts: CountsJson,
    cut: Option<CutJson<'a>>,
    total: SumJson,
    groups: Vec<GroupJson<'a>>,
    notes: &'a [String],
    truncated: bool,
    omitted: usize,
}

#[derive(Serialize)]
struct CountsJson {
    open: usize,
    done: usize,
}

#[derive(Serialize)]
struct CutJson<'a> {
    release: &'a str,
    min: u64,
    max: u64,
    unestimated: u64,
}

#[derive(Serialize)]
struct SumJson {
    min: u64,
    max: u64,
    unestimated: u64,
}

impl From<Sum> for SumJson {
    fn from(sum: Sum) -> Self {
        Self {
            min: sum.min,
            max: sum.max,
            unestimated: sum.unestimated,
        }
    }
}

#[derive(Serialize)]
struct GroupJson<'a> {
    group: &'static str,
    name: Option<&'a str>,
    current: bool,
    mvp_cut: bool,
    open: usize,
    done: usize,
    sum: SumJson,
    items: Vec<ItemJson<'a>>,
}

#[derive(Serialize)]
struct ItemJson<'a> {
    position: Option<usize>,
    id: &'a str,
    title: Option<&'a str>,
    path: &'a str,
    status: Option<&'a str>,
    release: Option<&'a str>,
    priority: Option<&'a str>,
    estimate: Option<EstimateJson>,
    waits_on: &'a [String],
    cycle: bool,
}

#[derive(Serialize)]
struct EstimateJson {
    min: u64,
    max: u64,
}

fn view(outcome: &RoadmapOutcome) -> RoadmapJson<'_> {
    let plan = &outcome.plan;
    let groups = if outcome.reason.is_some() {
        Vec::new()
    } else {
        outcome
            .shown
            .iter()
            .map(|&(at, items)| {
                let group = &plan.groups[at];
                GroupJson {
                    group: group.kind.as_str(),
                    name: group.name.as_deref(),
                    current: group.current,
                    mvp_cut: group.mvp_cut,
                    open: group.open,
                    done: group.done,
                    sum: group.sum.into(),
                    items: group.items[..items.min(group.items.len())]
                        .iter()
                        .map(|item| ItemJson {
                            position: item.position,
                            id: &item.id,
                            title: item.title.as_deref(),
                            path: &item.path,
                            status: item.status.as_deref(),
                            release: item.release.as_deref(),
                            priority: item.priority.as_deref(),
                            estimate: item.estimate.map(|estimate| EstimateJson {
                                min: estimate.min,
                                max: estimate.max,
                            }),
                            waits_on: &item.waits_on,
                            cycle: item.cycle,
                        })
                        .collect(),
                }
            })
            .collect()
    };
    RoadmapJson {
        release: outcome.release.as_deref(),
        pool: outcome.pool,
        reason: outcome.reason.as_deref(),
        prefix: &outcome.prefix,
        dir: &outcome.dir,
        unit: outcome.unit.as_deref(),
        current: plan.current.as_deref(),
        counts: CountsJson {
            open: plan.open,
            done: plan.done,
        },
        cut: plan.cut.as_ref().map(|cut| CutJson {
            release: &cut.release,
            min: cut.sum.min,
            max: cut.sum.max,
            unestimated: cut.sum.unestimated,
        }),
        total: plan.total.into(),
        groups,
        notes: &plan.notes,
        truncated: outcome.truncated,
        omitted: outcome.omitted(),
    }
}

/// The same document as [`crate::render_json`]: every key present, absent =
/// `null`.
impl Serialize for RoadmapOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        view(self).serialize(serializer)
    }
}
