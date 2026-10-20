//! The plan of a project's items (canon `spec-roadmap`, "Plan"): [`plan`]
//! over the check's input and the checked `[roadmap]` table. Pure: no
//! clock, no I/O, no queue; one input in any order gives one [`Plan`].
//!
//! 1. **Items**: every parsed document, live or Tier 3, whose front-matter
//!    `id:` has the table's `prefix`; never a `{#ID}` section or a
//!    `class: generated` document. An ID several documents hold: the first
//!    by path is the item, the others are left out with a note.
//! 2. **Done**: `status` `shipped` or `abandoned`; any other item is open.
//! 3. **Groups**: each release in the table's order, then the pool (no
//!    `release`), then the unplaced items (a `release` the table does not
//!    list, or one that is no string); every group is listed, empty or
//!    not. A group counts its done items and lists its open ones.
//! 4. **Edges**: each `links.depends_on` entry resolving, as `spec check`
//!    resolves it (its section ignored), to an item; an item's `waits_on`
//!    are its edge targets that are open, in written order, once each.
//! 5. **Order in a release**: the strongly connected components of its
//!    open items over the edges between them; a component of two or more,
//!    or of one with an edge to itself, is a cycle. A component comes after
//!    every component it depends on; of those ready, the smallest key first.
//!    An item's key is (priority rank, ID): the rank its priority's index in
//!    the scale, the scale's length when absent, off it or no string; IDs
//!    compare as strings. A component's key is its smallest member's; its
//!    members follow one another in key order. Edges to other groups never
//!    reorder.
//! 6. **Pool and unplaced**: by key, no position.
//! 7. **Estimates**: `N` or `N-M` ([`parse_estimate`]); any other text or a
//!    mistyped value is unestimated with a note; absent, without one.
//! 8. **Sums** over a group's open items: the estimated ones' bounds added,
//!    the others counted apart, never as 0. The cut: the releases from the
//!    first through `mvp_cut`; the total: every release.
//! 9. **Current**: the first release holding an open item.
//! 10. **Notes**, of open items only, sorted by text, each once
//!     (canon `spec-roadmap`, "Notes"); an ID held twice is noted when the
//!     file read or the one left out is open.
//!
//! It names no prefix, release, priority or unit of any project: every one
//! comes from the table.
//!
//! `spec roadmap`'s lines are built here ([`Plan::summary`],
//! [`PlanGroup::header`], [`PlanItem::line`], [`Sum::text`]); the index
//! entry's roadmap view (canon `spec-roadmap`, "View"; the last output of
//! [`crate::check::render_index_set`]) is the same lines in Markdown,
//! `render_view`, from the walked files alone.
//!
//! [`plan_tasks`] joins a plan with a repository's task rows (canon
//! `spec-roadmap`, "Position"), pure as [`plan`]: each open task executes
//! the first open item among its targets in the listing's order, the
//! others form the pool; [`task_item`] is one task's item and group,
//! [`next_order`] tasks by their items' listing order, then number (`spec
//! task show --next`, advisory). [`plan`], its lines and the view never
//! read a task: the caller adds the join to its own output.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fmt::Write as _;

use petgraph::algo::tarjan_scc;
use petgraph::graph::{DiGraph, NodeIndex};
use specengine_model::{FmValue, Node, ParsedFile, TaskStatus};

use crate::check::config::DocClass;
use crate::check::resolve::written;
use crate::check::{
    CheckFile, CheckInput, FileText, Generator, Resolver, RoadmapView, front_matter_failure,
    generated_header, relative_link,
};
use crate::roadmap_toml::RoadmapConfig;
use crate::task::task_number;
use crate::walk_scope::is_under;

/// The link type of the chain.
const DEPENDS_ON: &str = "depends_on";
/// The statuses of a done item (the spec class's closed ones).
const DONE: [&str; 2] = ["shipped", "abandoned"];
/// The typed key of an item's release.
const RELEASE_KEY: &str = "release";
/// The typed key of an item's priority.
const PRIORITY_KEY: &str = "priority";
/// The typed key of an item's estimate.
const ESTIMATE_KEY: &str = "estimate";

/// The largest bound of an estimate.
pub const ESTIMATE_MAX: u64 = 1_000_000;

/// The plan of one input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// Open items of every group.
    pub open: usize,
    /// Done items of every group.
    pub done: usize,
    /// The first release holding an open item.
    pub current: Option<String>,
    /// The sum to `mvp_cut`; `None` without one.
    pub cut: Option<CutSum>,
    /// The sum over every release.
    pub total: Sum,
    /// The releases in order, the pool, then the unplaced items.
    pub groups: Vec<PlanGroup>,
    /// Sorted by text, each once.
    pub notes: Vec<String>,
}

/// The sum of the open items of some groups.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Sum {
    /// The lower bounds of the estimated items, added.
    pub min: u64,
    /// The upper bounds of the estimated items, added.
    pub max: u64,
    /// The open items without an estimate.
    pub unestimated: u64,
}

impl Sum {
    fn add(&mut self, other: Self) {
        self.min = self.min.saturating_add(other.min);
        self.max = self.max.saturating_add(other.max);
        self.unestimated = self.unestimated.saturating_add(other.unestimated);
    }

    fn count(&mut self, estimate: Option<Estimate>) {
        match estimate {
            Some(estimate) => {
                self.min = self.min.saturating_add(estimate.min);
                self.max = self.max.saturating_add(estimate.max);
            }
            None => self.unestimated = self.unestimated.saturating_add(1),
        }
    }
}

/// The sum from the first release through `mvp_cut`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CutSum {
    /// `mvp_cut`.
    pub release: String,
    pub sum: Sum,
}

/// What a group gathers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GroupKind {
    /// One release of the table.
    Release,
    /// Items without a release.
    Pool,
    /// Items whose release the table does not list, or is no string.
    Unplaced,
}

impl GroupKind {
    /// `release`, `pool`, `unplaced`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Release => "release",
            Self::Pool => "pool",
            Self::Unplaced => "unplaced",
        }
    }
}

/// One group of the plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanGroup {
    pub kind: GroupKind,
    /// The release; `None` for the pool and the unplaced items.
    pub name: Option<String>,
    /// The release is [`Plan::current`].
    pub current: bool,
    /// The release is `mvp_cut`.
    pub mvp_cut: bool,
    pub open: usize,
    pub done: usize,
    /// Over the open items.
    pub sum: Sum,
    /// The open items, in order.
    pub items: Vec<PlanItem>,
}

/// One open item of a group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanItem {
    /// 1-based in a release; `None` in the pool and the unplaced items.
    pub position: Option<usize>,
    pub id: String,
    /// The document's title.
    pub title: Option<String>,
    /// Root-relative.
    pub path: String,
    /// `status:` as written.
    pub status: Option<String>,
    /// `release:` as written; `None` when absent or no string.
    pub release: Option<String>,
    /// `release:` is written but no string.
    pub release_mistyped: bool,
    /// `priority:` as written.
    pub priority: Option<String>,
    /// `None`: unestimated.
    pub estimate: Option<Estimate>,
    /// The open items it depends on, in written order.
    pub waits_on: Vec<String>,
    /// It is a member of a cycle of its release.
    pub cycle: bool,
}

/// An item's estimate, in the table's unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Estimate {
    pub min: u64,
    pub max: u64,
}

/// `N` when both bounds are equal, else `N-M`.
impl fmt::Display for Estimate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.min == self.max {
            write!(f, "{}", self.min)
        } else {
            write!(f, "{}-{}", self.min, self.max)
        }
    }
}

/// `N` (N..N) or `N-M` (N..M): ASCII digits, each at most
/// [`ESTIMATE_MAX`], `N <= M`, no sign, no space; `None` otherwise.
pub fn parse_estimate(text: &str) -> Option<Estimate> {
    let (low, high) = text.split_once('-').unwrap_or((text, text));
    let min = estimate_bound(low)?;
    let max = estimate_bound(high)?;
    (min <= max).then_some(Estimate { min, max })
}

fn estimate_bound(text: &str) -> Option<u64> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let value: u64 = text.parse().ok()?;
    (value <= ESTIMATE_MAX).then_some(value)
}

impl Sum {
    /// `<min>-<max>` (`<min>` when equal), ` <unit>` when set, `, <u>
    /// unestimated` when some are.
    pub fn text(self, unit: Option<&str>) -> String {
        let mut text = if self.min == self.max {
            self.min.to_string()
        } else {
            format!("{}-{}", self.min, self.max)
        };
        if let Some(unit) = unit {
            text.push(' ');
            text.push_str(&one_line(unit));
        }
        if self.unestimated > 0 {
            let _ = write!(text, ", {} unestimated", self.unestimated);
        }
        text
    }
}

impl Plan {
    /// The whole plan's line, `spec roadmap`'s first and the view's
    /// paragraph: `roadmap: <o> open, <d> done; current <release or
    /// none>[; to <cut> <sum>]; total <sum>`.
    pub fn summary(&self, unit: Option<&str>) -> String {
        let mut line = format!(
            "roadmap: {} open, {} done; current {}",
            self.open,
            self.done,
            self.current
                .as_deref()
                .map_or_else(|| "none".to_owned(), one_line)
        );
        if let Some(cut) = &self.cut {
            let _ = write!(
                line,
                "; to {} {}",
                one_line(&cut.release),
                cut.sum.text(unit)
            );
        }
        let _ = write!(line, "; total {}", self.total.text(unit));
        line
    }
}

impl PlanGroup {
    /// The group's line, `spec roadmap`'s header and the view's heading:
    /// `release <name>[ | current][ | mvp cut] | <o> open, <d> done |
    /// <sum>`, `pool | …`, `unplaced | …`.
    pub fn header(&self, unit: Option<&str>) -> String {
        let mut line = match (self.kind, &self.name) {
            (GroupKind::Release, Some(name)) => format!("release {}", one_line(name)),
            (kind, _) => kind.as_str().to_owned(),
        };
        if self.current {
            line.push_str(" | current");
        }
        if self.mvp_cut {
            line.push_str(" | mvp cut");
        }
        let _ = write!(
            line,
            " | {} open, {} done | {}",
            self.open,
            self.done,
            self.sum.text(unit)
        );
        line
    }
}

impl PlanItem {
    /// `spec roadmap`'s line of the item in a group of `kind`, indented:
    /// `<position>.` in a release, else `-`; the ID, title, status,
    /// priority and estimate (`-` when absent)`[ | waits on <ID>, …][ |
    /// cycle] | <path>`, and in the unplaced group `| release <as written,
    /// or mistyped>`.
    pub fn line(&self, kind: GroupKind) -> String {
        format!(
            "  {} {}{} | {}{}",
            self.lead(kind),
            one_line(&self.id),
            self.cells(kind),
            one_line(&self.path),
            self.placement(kind)
        )
    }

    /// The view's entry: [`PlanItem::line`] unindented, the ID a Markdown
    /// link to `link`, the path left out.
    pub(crate) fn entry(&self, kind: GroupKind, link: &str) -> String {
        format!(
            "{} [{}]({link}){}{}",
            self.lead(kind),
            one_line(&self.id),
            self.cells(kind),
            self.placement(kind)
        )
    }

    /// `<position>.` in a release, else `-`.
    fn lead(&self, kind: GroupKind) -> String {
        match self.position {
            Some(position) if kind == GroupKind::Release => format!("{position}."),
            _ => "-".to_owned(),
        }
    }

    /// ` | <title> | <status> | <priority> | <estimate>[ | waits on <ID>,
    /// …][ | cycle]`.
    fn cells(&self, kind: GroupKind) -> String {
        let mut cells = format!(
            " | {} | {} | {} | {}",
            or_dash(self.title.as_deref()),
            or_dash(self.status.as_deref()),
            or_dash(self.priority.as_deref()),
            self.estimate
                .map_or_else(|| "-".to_owned(), |estimate| estimate.to_string())
        );
        if !self.waits_on.is_empty() {
            let names: Vec<String> = self.waits_on.iter().map(|id| one_line(id)).collect();
            let _ = write!(cells, " | waits on {}", names.join(", "));
        }
        if kind == GroupKind::Release && self.cycle {
            cells.push_str(" | cycle");
        }
        cells
    }

    /// ` | release <as written, or mistyped>` in the unplaced group, else
    /// nothing.
    fn placement(&self, kind: GroupKind) -> String {
        if kind != GroupKind::Unplaced {
            return String::new();
        }
        match &self.release {
            Some(release) => format!(" | {RELEASE_KEY} {}", one_line(release)),
            None => format!(" | {RELEASE_KEY} mistyped"),
        }
    }
}

/// A value on one line: line breaks become spaces.
fn one_line(text: &str) -> String {
    text.replace("\r\n", " ").replace(['\n', '\r'], " ")
}

/// `-` for a value not given.
fn or_dash(value: Option<&str>) -> String {
    value.map_or_else(|| "-".to_owned(), one_line)
}

/// The view's H1.
const VIEW_TITLE: &str = "Roadmap";
/// The view's `source:`.
const VIEW_SOURCE: &str = "the plan items of [roadmap]";
/// The heading of the view's notes.
const NOTES_TITLE: &str = "Notes";

/// The roadmap view of `input` (canon `spec-roadmap`, "View"): the index
/// header of `generator` (H1 `Roadmap`, `source:` the plan items), then
/// `spec roadmap`'s lines of the whole plan of `view.config` in Markdown,
/// uncut: the summary a paragraph; each group a `##` heading, its open
/// items after a blank line (none: the heading alone), each
/// [`PlanItem::entry`] linked from the view's directory; `## Notes` with
/// one `- <note>` each when there are notes. Pure: no clock, no queue; one
/// input in any order gives one text.
pub(crate) fn render_view(input: &CheckInput, generator: &Generator, view: &RoadmapView) -> String {
    let plan = plan(input, &view.config);
    let unit = view.config.unit.as_deref();
    let mut out = generated_header(generator, VIEW_TITLE, VIEW_SOURCE);
    out.push_str(&plan.summary(unit));
    out.push('\n');
    for group in &plan.groups {
        let _ = write!(out, "\n## {}\n", group.header(unit));
        if !group.items.is_empty() {
            out.push('\n');
        }
        for item in &group.items {
            let link = relative_link(&view.path, &item.path);
            out.push_str(&item.entry(group.kind, &link));
            out.push('\n');
        }
    }
    if !plan.notes.is_empty() {
        let _ = write!(out, "\n## {NOTES_TITLE}\n\n");
        for note in &plan.notes {
            let _ = writeln!(out, "- {}", one_line(note));
        }
    }
    out
}

/// An item's `release:`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Placement<'a> {
    Absent,
    Written(&'a str),
    Mistyped,
}

/// An item's `estimate:`.
#[derive(Debug, Clone, PartialEq, Eq)]
enum EstimateRead {
    Absent,
    Valid(Estimate),
    /// The text, or a mistyped value's JSON.
    Bad(String),
}

/// An item as read from its document.
struct Read<'a> {
    file: &'a CheckFile,
    document: &'a Node,
    id: &'a str,
    status: Option<&'a str>,
    done: bool,
    release: Placement<'a>,
    priority: Option<&'a str>,
    /// `priority:` is written but no string (kept in `extra`).
    priority_mistyped: bool,
    /// The priority's index in the scale; its length when absent, off it or
    /// no string.
    rank: usize,
    estimate: EstimateRead,
}

impl Read<'_> {
    fn key(&self) -> (usize, &str) {
        (self.rank, self.id)
    }

    fn estimate(&self) -> Option<Estimate> {
        match self.estimate {
            EstimateRead::Valid(estimate) => Some(estimate),
            EstimateRead::Absent | EstimateRead::Bad(_) => None,
        }
    }
}

/// Where an item is placed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Slot {
    Release(usize),
    Pool,
    Unplaced,
}

/// The plan of `input` under `config` (see the module documentation).
pub fn plan(input: &CheckInput, config: &RoadmapConfig) -> Plan {
    let mut files: Vec<&CheckFile> = input.files.iter().collect();
    files.sort_by(|a, b| a.path.cmp(&b.path));
    let mut items: Vec<Read<'_>> = Vec::new();
    let mut by_id: BTreeMap<&str, usize> = BTreeMap::new();
    // (the item, a path also defining its ID, that file is open)
    let mut also: Vec<(usize, &str, bool)> = Vec::new();
    for file in files {
        let Some(parsed) = &file.parsed else {
            continue;
        };
        let Some((document, id)) = item_document(parsed, config) else {
            continue;
        };
        match by_id.get(id) {
            Some(&first) => {
                let status = document
                    .fields
                    .as_ref()
                    .and_then(|fields| fields.status.as_deref());
                also.push((first, file.path.as_str(), !is_done(status)));
            }
            None => {
                by_id.insert(id, items.len());
                items.push(read_item(file, document, id, config));
            }
        }
    }

    let targets = edges(input, config, &items);
    let waits_on: Vec<Vec<String>> = targets
        .iter()
        .map(|targets| {
            targets
                .iter()
                .filter(|&&target| !items[target].done)
                .map(|&target| items[target].id.to_owned())
                .collect()
        })
        .collect();
    let releases: BTreeMap<&str, usize> = config
        .releases
        .iter()
        .enumerate()
        .map(|(at, release)| (release.as_str(), at))
        .collect();
    let slots: Vec<Slot> = items
        .iter()
        .map(|item| match item.release {
            Placement::Absent => Slot::Pool,
            Placement::Mistyped => Slot::Unplaced,
            Placement::Written(release) => releases
                .get(release)
                .map_or(Slot::Unplaced, |&at| Slot::Release(at)),
        })
        .collect();

    let mut notes = BTreeSet::new();
    let mut groups = Vec::with_capacity(config.releases.len() + 2);
    for (at, release) in config.releases.iter().enumerate() {
        let members: Vec<usize> = (0..items.len())
            .filter(|&index| slots[index] == Slot::Release(at))
            .collect();
        let open = open_by_key(&items, &members);
        let (order, cycles) = chain_order(&items, &open, &targets);
        for cycle in cycles {
            let mut ids: Vec<&str> = cycle.iter().map(|&index| items[index].id).collect();
            ids.sort_unstable();
            notes.insert(format!("{DEPENDS_ON} cycle: {}", ids.join(", ")));
        }
        let listed = order
            .into_iter()
            .enumerate()
            .map(|(position, (index, cycle))| {
                plan_item(&items[index], Some(position + 1), &waits_on[index], cycle)
            })
            .collect();
        groups.push(group(
            GroupKind::Release,
            Some(release.clone()),
            config.mvp_cut.as_deref() == Some(release.as_str()),
            &items,
            &members,
            listed,
        ));
    }
    for (kind, slot) in [
        (GroupKind::Pool, Slot::Pool),
        (GroupKind::Unplaced, Slot::Unplaced),
    ] {
        let members: Vec<usize> = (0..items.len())
            .filter(|&index| slots[index] == slot)
            .collect();
        let listed = open_by_key(&items, &members)
            .into_iter()
            .map(|index| plan_item(&items[index], None, &waits_on[index], false))
            .collect();
        groups.push(group(kind, None, false, &items, &members, listed));
    }

    let current = groups
        .iter()
        .find(|group| group.kind == GroupKind::Release && group.open > 0)
        .and_then(|group| group.name.clone());
    for group in &mut groups {
        group.current = group.kind == GroupKind::Release && group.name == current;
    }
    let mut total = Sum::default();
    for group in groups
        .iter()
        .filter(|group| group.kind == GroupKind::Release)
    {
        total.add(group.sum);
    }
    let cut = config.mvp_cut.as_ref().map(|cut| {
        let mut sum = Sum::default();
        for group in groups
            .iter()
            .filter(|group| group.kind == GroupKind::Release)
        {
            sum.add(group.sum);
            if group.name.as_ref() == Some(cut) {
                break;
            }
        }
        CutSum {
            release: cut.clone(),
            sum,
        }
    });

    for (index, item) in items.iter().enumerate() {
        if !item.done {
            item_notes(item, slots[index], config, &mut notes);
        }
    }
    for (first, path, left_out_open) in also {
        let item = &items[first];
        if !item.done || left_out_open {
            notes.insert(format!(
                "{}: also defined in {path}; {} is read",
                item.id, item.file.path
            ));
        }
    }

    Plan {
        open: groups.iter().map(|group| group.open).sum(),
        done: groups.iter().map(|group| group.done).sum(),
        current,
        cut,
        total,
        groups,
        notes: notes.into_iter().collect(),
    }
}

/// The document of `parsed` and its ID when it is an item: its front-matter
/// read, its `id:` of the table's prefix, not `class: generated`.
fn item_document<'a>(
    parsed: &'a ParsedFile,
    config: &RoadmapConfig,
) -> Option<(&'a Node, &'a str)> {
    if front_matter_failure(parsed).is_some() {
        return None;
    }
    let document = parsed.document()?;
    let id = document.id.as_deref()?;
    let (prefix, _) = id.split_once('-')?;
    if prefix != config.prefix {
        return None;
    }
    let generated = document
        .fields
        .as_ref()
        .and_then(|fields| fields.class.as_deref())
        == Some(DocClass::Generated.as_str());
    (!generated).then_some((document, id))
}

fn read_item<'a>(
    file: &'a CheckFile,
    document: &'a Node,
    id: &'a str,
    config: &RoadmapConfig,
) -> Read<'a> {
    let fields = document.fields.as_ref();
    let extra = |key: &str| {
        document
            .extra
            .iter()
            .flatten()
            .find(|entry| entry.key == key)
            .map(|entry| &entry.value)
    };
    let status = fields.and_then(|fields| fields.status.as_deref());
    let release = match fields.and_then(|fields| fields.release.as_deref()) {
        Some(release) => Placement::Written(release),
        None if extra(RELEASE_KEY).is_some() => Placement::Mistyped,
        None => Placement::Absent,
    };
    let priority = fields.and_then(|fields| fields.priority.as_deref());
    let rank = priority
        .and_then(|priority| config.priorities.iter().position(|known| known == priority))
        .unwrap_or(config.priorities.len());
    let estimate = match fields.and_then(|fields| fields.estimate.as_deref()) {
        Some(text) => parse_estimate(text)
            .map_or_else(|| EstimateRead::Bad(text.to_owned()), EstimateRead::Valid),
        None => extra(ESTIMATE_KEY).map_or(EstimateRead::Absent, |value| {
            EstimateRead::Bad(value_text(value))
        }),
    };
    Read {
        file,
        document,
        id,
        status,
        done: is_done(status),
        release,
        priority,
        priority_mistyped: priority.is_none() && extra(PRIORITY_KEY).is_some(),
        rank,
        estimate,
    }
}

/// `status` closes an item.
fn is_done(status: Option<&str>) -> bool {
    status.is_some_and(|status| DONE.contains(&status))
}

/// A mistyped value as compact JSON.
fn value_text(value: &FmValue) -> String {
    serde_json::to_string(value).unwrap_or_default()
}

/// Per item, the items its `depends_on` entries resolve to, in written
/// order, once each (itself included when an entry names it).
fn edges(input: &CheckInput, config: &RoadmapConfig, items: &[Read<'_>]) -> Vec<Vec<usize>> {
    let mut targets: Vec<Vec<usize>> = vec![Vec::new(); items.len()];
    if items.is_empty() {
        return targets;
    }
    let resolver = Resolver::new(input, &config.scheme, &config.paths);
    let item_at: BTreeMap<&str, usize> = items
        .iter()
        .enumerate()
        .map(|(index, item)| (item.file.path.as_str(), index))
        .collect();
    for (index, item) in items.iter().enumerate() {
        let Some(list) = item
            .document
            .fields
            .as_ref()
            .and_then(|fields| fields.links.as_ref())
            .and_then(|links| links.get(DEPENDS_ON))
        else {
            continue;
        };
        let text = FileText::new(&item.file.bytes);
        for reference in list {
            let written = written(&text, reference);
            let Some(holders) = resolver.holders_of(&item.file.path, reference, &written) else {
                continue;
            };
            for holder in holders {
                let Some(&target) = resolver
                    .paths()
                    .get(holder)
                    .and_then(|path| item_at.get(path))
                else {
                    continue;
                };
                if !targets[index].contains(&target) {
                    targets[index].push(target);
                }
            }
        }
    }
    targets
}

/// The open ones of `members`, by key.
fn open_by_key(items: &[Read<'_>], members: &[usize]) -> Vec<usize> {
    let mut open: Vec<usize> = members
        .iter()
        .copied()
        .filter(|&index| !items[index].done)
        .collect();
    open.sort_by(|&a, &b| items[a].key().cmp(&items[b].key()));
    open
}

/// A release's open items (`open`, by key) in chain order, each with its
/// cycle flag, and the cycles (see the module documentation, rule 5).
fn chain_order(
    items: &[Read<'_>],
    open: &[usize],
    targets: &[Vec<usize>],
) -> (Vec<(usize, bool)>, Vec<Vec<usize>>) {
    let mut graph: DiGraph<usize, ()> = DiGraph::with_capacity(open.len(), 0);
    let nodes: BTreeMap<usize, NodeIndex> = open
        .iter()
        .map(|&index| (index, graph.add_node(index)))
        .collect();
    for &index in open {
        for target in &targets[index] {
            if let Some(&to) = nodes.get(target) {
                graph.add_edge(nodes[&index], to, ());
            }
        }
    }
    let key = |index: usize| items[index].key();
    // Components, each with its members by key and its cycle flag.
    let mut components: Vec<(Vec<usize>, bool)> = Vec::new();
    let mut component_of: BTreeMap<usize, usize> = BTreeMap::new();
    for component in tarjan_scc(&graph) {
        let cycle = match component.as_slice() {
            [] => continue,
            [only] => graph.contains_edge(*only, *only),
            _ => true,
        };
        let mut members: Vec<usize> = component.iter().map(|&node| graph[node]).collect();
        members.sort_by(|&a, &b| key(a).cmp(&key(b)));
        for &member in &members {
            component_of.insert(member, components.len());
        }
        components.push((members, cycle));
    }
    // What each component waits for, and who waits for it.
    let mut waiting: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); components.len()];
    let mut dependents: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); components.len()];
    for &index in open {
        let from = component_of[&index];
        for target in &targets[index] {
            if let Some(&to) = component_of.get(target)
                && to != from
            {
                waiting[from].insert(to);
                dependents[to].insert(from);
            }
        }
    }
    let mut ready: BTreeSet<((usize, &str), usize)> = BTreeSet::new();
    for (at, (members, _)) in components.iter().enumerate() {
        if waiting[at].is_empty() {
            ready.insert((key(members[0]), at));
        }
    }
    let mut order = Vec::with_capacity(open.len());
    let mut cycles = Vec::new();
    while let Some((_, at)) = ready.pop_first() {
        let (members, cycle) = &components[at];
        order.extend(members.iter().map(|&member| (member, *cycle)));
        if *cycle {
            cycles.push(members.clone());
        }
        for &dependent in &dependents[at] {
            waiting[dependent].remove(&at);
            if waiting[dependent].is_empty() {
                ready.insert((key(components[dependent].0[0]), dependent));
            }
        }
    }
    (order, cycles)
}

fn plan_item(
    item: &Read<'_>,
    position: Option<usize>,
    waits_on: &[String],
    cycle: bool,
) -> PlanItem {
    PlanItem {
        position,
        id: item.id.to_owned(),
        title: item.document.title.clone(),
        path: item.file.path.clone(),
        status: item.status.map(str::to_owned),
        release: match item.release {
            Placement::Written(release) => Some(release.to_owned()),
            Placement::Absent | Placement::Mistyped => None,
        },
        release_mistyped: item.release == Placement::Mistyped,
        priority: item.priority.map(str::to_owned),
        estimate: item.estimate(),
        waits_on: waits_on.to_vec(),
        cycle,
    }
}

fn group(
    kind: GroupKind,
    name: Option<String>,
    mvp_cut: bool,
    items: &[Read<'_>],
    members: &[usize],
    listed: Vec<PlanItem>,
) -> PlanGroup {
    let mut sum = Sum::default();
    for &index in members {
        if !items[index].done {
            sum.count(items[index].estimate());
        }
    }
    let done = members.iter().filter(|&&index| items[index].done).count();
    PlanGroup {
        kind,
        name,
        current: false,
        mvp_cut,
        open: members.len() - done,
        done,
        sum,
        items: listed,
    }
}

/// An open item's notes: its release, priority, estimate and place.
fn item_notes(item: &Read<'_>, slot: Slot, config: &RoadmapConfig, notes: &mut BTreeSet<String>) {
    let id = item.id;
    match (item.release, slot) {
        (Placement::Written(release), Slot::Unplaced) => {
            notes.insert(format!(
                "{id}: {RELEASE_KEY} \"{release}\" is not in [roadmap] releases"
            ));
        }
        (Placement::Mistyped, _) => {
            notes.insert(format!("{id}: {RELEASE_KEY} is not a string"));
        }
        _ => {}
    }
    if let Some(priority) = item.priority
        && item.rank == config.priorities.len()
    {
        notes.insert(format!(
            "{id}: {PRIORITY_KEY} \"{priority}\" is not in [roadmap] priorities"
        ));
    }
    if item.priority_mistyped {
        notes.insert(format!("{id}: {PRIORITY_KEY} is not a string"));
    }
    if let EstimateRead::Bad(text) = &item.estimate {
        notes.insert(format!("{id}: {ESTIMATE_KEY} \"{text}\" is not N or N-M"));
    }
    if !is_under(&item.file.path, &config.dir) {
        notes.insert(format!("{id}: outside [roadmap] dir {}", config.dir));
    }
}

/// A task row as the join reads it (canon `spec-roadmap`, "Position"): one
/// of the queue's tasks of the reading repository, any state.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlanTask {
    /// As stored, `T-0001`.
    pub id: String,
    pub status: TaskStatus,
    pub title: Option<String>,
    /// As stored: canonical IDs, or an id-less document's path.
    pub targets: Vec<String>,
}

impl PlanTask {
    /// Not `done`, `accepted` or `cancelled`: the join places it.
    pub fn is_open(&self) -> bool {
        !matches!(
            self.status,
            TaskStatus::Done | TaskStatus::Accepted | TaskStatus::Cancelled
        )
    }
}

/// The open tasks of a plan ([`plan_tasks`]): the items they execute and
/// the pool of the others.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlanTasks {
    /// Per open item's ID, the open tasks executing it, by number; an item
    /// no task executes is absent.
    pub items: BTreeMap<String, Vec<PlanTask>>,
    /// The open tasks executing no open item (no target an item, or only
    /// done ones), by number.
    pub pool: Vec<PlanTask>,
}

impl PlanTasks {
    /// The open tasks executing the item `id`, by number.
    pub fn of(&self, id: &str) -> &[PlanTask] {
        self.items.get(id).map_or(&[], Vec::as_slice)
    }

    /// The item `id` is in progress: a task executing it is `in_progress`.
    /// Its own `status:` is the file's, never this.
    pub fn in_progress(&self, id: &str) -> bool {
        self.of(id)
            .iter()
            .any(|task| task.status == TaskStatus::InProgress)
    }

    /// No open task at all.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty() && self.pool.is_empty()
    }
}

/// The join of `plan` with the task rows `tasks` (canon `spec-roadmap`,
/// "Position"): pure, any row order giving one result. Of the open rows
/// ([`PlanTask::is_open`]), each executes the first of its targets that
/// is an open item of `plan` in `spec roadmap`'s listing order (the
/// groups in order, each one's items in order), targets compared with item
/// IDs as written; one with none, or with only done or unknown targets,
/// joins the pool. Tasks are ordered by number (an ID that is none after
/// every number), then by the whole row. Closed rows are left out.
pub fn plan_tasks(plan: &Plan, tasks: &[PlanTask]) -> PlanTasks {
    let listing = Listing::new(plan);
    let mut open: Vec<&PlanTask> = tasks.iter().filter(|task| task.is_open()).collect();
    open.sort_by(|a, b| task_key(a).cmp(&task_key(b)).then_with(|| a.cmp(b)));
    let mut joined = PlanTasks::default();
    for task in open {
        match listing.executed(task) {
            Some(at) => joined
                .items
                .entry(listing.items[at].1.id.clone())
                .or_default()
                .push(task.clone()),
            None => joined.pool.push(task.clone()),
        }
    }
    joined
}

/// The open item an open `task` executes in `plan`, with its group, as
/// [`plan_tasks`] joins it: the item whose line names the task in `spec
/// roadmap`. `None` for a closed task or one executing no open item.
pub fn task_item<'p>(plan: &'p Plan, task: &PlanTask) -> Option<(&'p PlanGroup, &'p PlanItem)> {
    if !task.is_open() {
        return None;
    }
    let listing = Listing::new(plan);
    listing.executed(task).map(|at| listing.items[at])
}

/// `tasks` in `next`'s order (canon `spec-roadmap`, "Position"): by the
/// listing index of the open item each executes ([`task_item`]), then by
/// number; those executing none after every one that does, by number.
/// Advisory: an order to read, never a hold on a task. Pure, any row order
/// one result.
pub fn next_order<'a>(plan: &Plan, tasks: &'a [PlanTask]) -> Vec<&'a PlanTask> {
    let listing = Listing::new(plan);
    let mut keyed: Vec<(usize, &PlanTask)> = tasks
        .iter()
        .map(|task| {
            let at = task
                .is_open()
                .then(|| listing.executed(task))
                .flatten()
                .unwrap_or(usize::MAX);
            (at, task)
        })
        .collect();
    keyed.sort_by(|(a_at, a), (b_at, b)| {
        a_at.cmp(b_at)
            .then_with(|| task_key(a).cmp(&task_key(b)))
            .then_with(|| a.cmp(b))
    });
    keyed.into_iter().map(|(_, task)| task).collect()
}

/// The open items of a plan in `spec roadmap`'s listing order (the groups
/// in order, each one's items in order), with their groups.
struct Listing<'p> {
    items: Vec<(&'p PlanGroup, &'p PlanItem)>,
    /// Each item's ID, its first index in `items`.
    index: BTreeMap<&'p str, usize>,
}

impl<'p> Listing<'p> {
    fn new(plan: &'p Plan) -> Self {
        let items: Vec<(&PlanGroup, &PlanItem)> = plan
            .groups
            .iter()
            .flat_map(|group| group.items.iter().map(move |item| (group, item)))
            .collect();
        let mut index = BTreeMap::new();
        for (at, (_, item)) in items.iter().enumerate() {
            index.entry(item.id.as_str()).or_insert(at);
        }
        Self { items, index }
    }

    /// The listing index of the first of `task`'s targets that is an open
    /// item, targets compared with item IDs as written; its state unread.
    fn executed(&self, task: &PlanTask) -> Option<usize> {
        task.targets
            .iter()
            .filter_map(|target| self.index.get(target.as_str()).copied())
            .min()
    }
}

/// A task's place in number order: its number, an ID that is none last.
fn task_key(task: &PlanTask) -> (u64, &str) {
    (task_number(&task.id).unwrap_or(u64::MAX), task.id.as_str())
}
