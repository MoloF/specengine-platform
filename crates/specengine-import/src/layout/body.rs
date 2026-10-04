//! A source document's body split between its residue and the record files
//! of the `{#ID}` sections moved out of it, with the reshaped `{#ID}`
//! blocks placed (`docs/canon/import-layout.md` "Reshaped sections", "Residue").
//! Line-based: every body line goes to exactly one place or is dropped
//! because a record file or a reshaped block carries its text.

use std::collections::BTreeMap;
use std::ops::RangeInclusive;

use crate::import::is_blank;
use crate::markdown::Scan;

use super::s_heading;

/// A `{#ID}` section moving to a record file: its source extent.
pub(super) struct MovedSection {
    pub extent: [usize; 2],
}

/// A row or list item becoming a `{#ID}` section of its document.
pub(super) struct Reshaped {
    pub extent: [usize; 2],
    /// The heading after its ATX marker: title else ID, then the attribute
    /// block.
    pub heading: String,
    /// The record's text, LF-joined.
    pub text: String,
}

/// What the body builder needs of one document.
pub(super) struct BodyPlan<'p, 's> {
    pub scan: &'p Scan<'s>,
    /// Lines that leave the body: a field table becoming the header, with
    /// the blank lines right after it.
    pub excluded: Option<RangeInclusive<usize>>,
    /// In source order.
    pub moved: Vec<MovedSection>,
    /// Extents of rows and items moved out (to a file or reshaped).
    pub dropped: Vec<[usize; 2]>,
    /// In source order.
    pub reshaped: Vec<Reshaped>,
    /// Heading lines of in-place sections whose `{#written}` becomes
    /// `{#id}` (rule S): line → (written, id).
    pub rule_s: BTreeMap<usize, (String, String)>,
    /// Heading lines of in-place `{#ID}` sections.
    pub in_place: Vec<usize>,
}

/// The residue body and each moved section's body (its heading excluded),
/// as lines; where each reshaped block and in-place section landed: `None`
/// the residue, `Some(k)` the body of moved section `k` (nested in it).
pub(super) struct BodyOut {
    pub residue: Vec<String>,
    pub sections: Vec<Vec<String>>,
    /// Aligned with [`BodyPlan::reshaped`].
    pub reshaped_in: Vec<Option<usize>>,
    /// Aligned with [`BodyPlan::in_place`].
    pub in_place_in: Vec<Option<usize>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Dest {
    Residue,
    /// A moved section's body.
    Section(usize),
    /// A moved section's heading: its title, no body line.
    Title(usize),
    Dropped,
    Excluded,
}

impl Dest {
    /// The output a line of this destination belongs to.
    fn container(self) -> Option<Container> {
        match self {
            Dest::Residue => Some(Container::Residue),
            Dest::Section(index) | Dest::Title(index) => Some(Container::Section(index)),
            Dest::Dropped | Dest::Excluded => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Container {
    Residue,
    Section(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Anchor {
    /// Before the residue's first body line.
    Start,
    /// After this source line, in the output its line goes to.
    After(usize),
}

/// Splits the body (`docs/canon/import-layout.md` "Reshaped sections"):
/// a reshaped block of level
/// L = min(n + 1, 6), n the level of the nearest import-scanner heading
/// above its extent (none: 0), follows the last non-blank line of that
/// heading's section kept in the same output (n = 0: the document's), as
/// `""`, heading, `""`, text.
/// Blocks sharing an anchor go deepest enclosing section first (n
/// descending), then in source order, so that no block lands inside
/// another's section (`docs/features/import-layout.md` AC-01).
pub(super) fn split(plan: &BodyPlan<'_, '_>) -> BodyOut {
    let lines = &plan.scan.lines;
    let Some(first) = lines.first().map(|line| line.number) else {
        return BodyOut {
            residue: Vec::new(),
            sections: vec![Vec::new(); plan.moved.len()],
            reshaped_in: vec![None; plan.reshaped.len()],
            in_place_in: vec![None; plan.in_place.len()],
        };
    };
    let last = first + lines.len() - 1;
    let position = |number: usize| number.checked_sub(first).filter(|at| *at < lines.len());
    let mut dest = vec![Dest::Residue; lines.len()];
    let mark = |range: RangeInclusive<usize>, value: Dest, dest: &mut Vec<Dest>| {
        for number in range {
            if let Some(at) = position(number) {
                dest[at] = value;
            }
        }
    };
    if let Some(excluded) = &plan.excluded {
        mark(excluded.clone(), Dest::Excluded, &mut dest);
    }
    // Source order: a nested section starts later and overrides its range.
    for (index, section) in plan.moved.iter().enumerate() {
        mark(
            section.extent[0]..=section.extent[1],
            Dest::Section(index),
            &mut dest,
        );
        mark(
            section.extent[0]..=section.extent[0],
            Dest::Title(index),
            &mut dest,
        );
    }
    for extent in &plan.dropped {
        mark(extent[0]..=extent[1], Dest::Dropped, &mut dest);
    }

    let section_of = |container: Option<Container>| match container {
        Some(Container::Section(index)) => Some(index),
        _ => None,
    };
    let in_place_in = plan
        .in_place
        .iter()
        .map(|&line| section_of(position(line).and_then(|at| dest[at].container())))
        .collect();

    let headings = &plan.scan.headings;
    // Per anchor: (n, source order, lines).
    let mut blocks: BTreeMap<Anchor, Vec<(usize, usize, Vec<String>)>> = BTreeMap::new();
    let mut reshaped_in = Vec::with_capacity(plan.reshaped.len());
    for (order, reshaped) in plan.reshaped.iter().enumerate() {
        let above = headings
            .iter()
            .rev()
            .find(|heading| heading.line < reshaped.extent[0]);
        let (level, range, container) = match above {
            Some(heading) => {
                let end = headings
                    .iter()
                    .find(|next| next.line > heading.line && next.level <= heading.level)
                    .map_or(last, |next| next.line.saturating_sub(1));
                let container = position(heading.line)
                    .and_then(|at| dest[at].container())
                    .unwrap_or(Container::Residue);
                (heading.level, heading.line..=end, container)
            }
            None => (0, first..=last, Container::Residue),
        };
        let anchor = range
            .rev()
            .find(|&number| {
                position(number).is_some_and(|at| {
                    dest[at].container() == Some(container) && !is_blank(lines[at].raw)
                })
            })
            .map_or(Anchor::Start, Anchor::After);
        reshaped_in.push(section_of(Some(container)));
        let marker = "#".repeat((level + 1).min(6));
        let mut block = vec![String::new(), format!("{marker} {}", reshaped.heading)];
        if !reshaped.text.is_empty() {
            block.push(String::new());
            block.extend(reshaped.text.split('\n').map(str::to_owned));
        }
        blocks
            .entry(anchor)
            .or_default()
            .push((level, order, block));
    }
    for shared in blocks.values_mut() {
        shared.sort_by_key(|&(level, order, _)| (std::cmp::Reverse(level), order));
    }

    let mut residue: Vec<String> = Vec::new();
    let mut sections: Vec<Vec<String>> = vec![Vec::new(); plan.moved.len()];
    let place = |anchor: Anchor, out: &mut Vec<String>| {
        for (_, _, block) in blocks.get(&anchor).into_iter().flatten() {
            out.extend(block.iter().cloned());
        }
    };
    place(Anchor::Start, &mut residue);
    for (at, line) in lines.iter().enumerate() {
        let text = match plan.rule_s.get(&line.number) {
            Some((written, id)) => {
                s_heading(line.raw, written, id).unwrap_or_else(|| line.raw.to_owned())
            }
            None => line.raw.to_owned(),
        };
        match dest[at] {
            Dest::Residue => {
                residue.push(text);
                place(Anchor::After(line.number), &mut residue);
            }
            Dest::Section(index) => {
                sections[index].push(text);
                place(Anchor::After(line.number), &mut sections[index]);
            }
            Dest::Title(index) => place(Anchor::After(line.number), &mut sections[index]),
            Dest::Dropped | Dest::Excluded => {}
        }
    }
    BodyOut {
        residue,
        sections,
        reshaped_in,
        in_place_in,
    }
}
