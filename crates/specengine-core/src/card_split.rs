//! A split's plan (`docs/canon/card-split.md` "The rule";
//! `docs/features/department-split.md` "Acceptance criteria", S2): which
//! subtasks one split makes, where each lands and what it targets. Pure:
//! the caller reads the scope (each parent target and the nodes under it to
//! depth 2, in the tree read's order) and hands it in; nothing is read or
//! written here.
//!
//! A `[[card.split]]` row matches when its `kind` equals a scope node's
//! kind byte for byte. The matched rows' disciplines are `suggested`, row
//! order then list order. A subtask targets the scope nodes of the kinds its
//! matched rows name, at most [`NODES_MAX`] with a note past them, else the
//! parent's targets. A row only pre-selects: no plan is refused for a row,
//! and no kind, discipline or flow means anything here (ADR-0008, ADR-0012).

use serde::Serialize;

use crate::card_toml::{CardConfig, CardSplit, name_problem};
use crate::task::NODES_MAX;

/// The most subtasks one split makes.
pub const MAX_SPLIT_CHILDREN: usize = 32;

/// One node of a split's scope: its canonical REF, as a card stores a
/// target, and its kind (`None`: none written).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ScopeNode {
    pub target: String,
    pub kind: Option<String>,
}

/// Where a subtask's targets come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TargetsFrom {
    /// The scope nodes of the kinds its matched rows name.
    Kinds,
    /// The parent's targets: no matched row lists its discipline, or no
    /// scope node is of their kinds.
    Parent,
}

/// One subtask a split makes: its discipline, the flow and first column
/// the new-card rule gives it, its targets and whether a row suggested it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct PlannedChild {
    pub discipline: String,
    pub flow: String,
    /// The flow's `title`, else its name.
    pub flow_title: String,
    pub column: String,
    pub targets: Vec<String>,
    pub from: TargetsFrom,
    pub suggested: bool,
}

/// What a split makes: the suggested disciplines, the subtasks in the order
/// asked, and what a reader should know (a target cap taken).
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct SplitPlan {
    pub suggested: Vec<String>,
    pub children: Vec<PlannedChild>,
    pub notes: Vec<String>,
}

/// The plan of a split over `scope` (see the module documentation).
/// `chosen`: the subtasks' disciplines in the order asked; `None`, every
/// discipline the flows list, flow order then list order, each once. The
/// `Err` is a refusal's reason: the chosen list's first defect, else the
/// first subtask, in order, no flow takes.
pub fn split_plan(
    config: &CardConfig,
    scope: &[ScopeNode],
    parent_targets: &[String],
    chosen: Option<&[String]>,
) -> Result<SplitPlan, String> {
    let disciplines = match chosen {
        Some(chosen) => chosen_problem(chosen).map_or_else(|| Ok(chosen.to_vec()), Err)?,
        None => every_discipline(config),
    };
    let matched: Vec<&CardSplit> = config
        .splits
        .iter()
        .filter(|row| {
            scope
                .iter()
                .any(|node| node.kind.as_deref() == Some(row.kind.as_str()))
        })
        .collect();
    let mut suggested: Vec<String> = Vec::new();
    for discipline in matched.iter().flat_map(|row| &row.disciplines) {
        if !suggested.contains(discipline) {
            suggested.push(discipline.clone());
        }
    }
    let mut children = Vec::with_capacity(disciplines.len());
    let mut notes = Vec::new();
    for discipline in disciplines {
        let flow = config
            .flow_for(Some(&discipline))
            .map_err(|reason| format!("child: {reason}"))?;
        let kinds: Vec<&str> = matched
            .iter()
            .filter(|row| row.disciplines.contains(&discipline))
            .map(|row| row.kind.as_str())
            .collect();
        let found: Vec<&str> = scope
            .iter()
            .filter(|node| {
                node.kind
                    .as_deref()
                    .is_some_and(|kind| kinds.contains(&kind))
            })
            .map(|node| node.target.as_str())
            .collect();
        let (targets, from) = if found.is_empty() {
            (parent_targets.to_vec(), TargetsFrom::Parent)
        } else {
            if found.len() > NODES_MAX {
                notes.push(format!(
                    "subtask `{discipline}`: {} nodes of its kinds; the first {NODES_MAX} taken",
                    found.len()
                ));
            }
            let taken = found
                .iter()
                .take(NODES_MAX)
                .map(|&target| target.to_owned());
            (taken.collect(), TargetsFrom::Kinds)
        };
        children.push(PlannedChild {
            suggested: suggested.contains(&discipline),
            flow: flow.name.clone(),
            flow_title: flow.title.clone(),
            column: flow.first_column().to_owned(),
            discipline,
            targets,
            from,
        });
    }
    Ok(SplitPlan {
        suggested,
        children,
        notes,
    })
}

/// The chosen list's first defect: none, more than
/// [`MAX_SPLIT_CHILDREN`], then in order a name off the grammar or a
/// repeat.
fn chosen_problem(chosen: &[String]) -> Option<String> {
    if chosen.is_empty() {
        return Some(format!("child: none named; name 1 to {MAX_SPLIT_CHILDREN}"));
    }
    if chosen.len() > MAX_SPLIT_CHILDREN {
        return Some(format!(
            "child: {} named; a split makes 1 to {MAX_SPLIT_CHILDREN}",
            chosen.len()
        ));
    }
    chosen.iter().enumerate().find_map(|(at, discipline)| {
        name_problem(discipline)
            .map(|problem| format!("child: {problem}"))
            .or_else(|| {
                chosen[..at]
                    .contains(discipline)
                    .then(|| format!("child: `{discipline}` is named twice"))
            })
    })
}

/// Every discipline the flows list, flow order then list order, each once.
fn every_discipline(config: &CardConfig) -> Vec<String> {
    let mut every: Vec<String> = Vec::new();
    for discipline in config.flows.iter().flat_map(|flow| &flow.disciplines) {
        if !every.contains(discipline) {
            every.push(discipline.clone());
        }
    }
    every
}
