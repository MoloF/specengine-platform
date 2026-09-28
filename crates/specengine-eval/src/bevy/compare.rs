//! Detector against dump: which dumped systems of the corpus's own crates the
//! syntactic detector found, and why the others were missed.
//!
//! **Basis.** A dumped system is keyed by its schedule (the label's `Debug`
//! text) and its terminal name: the last path segment of its `type_name`
//! without generic arguments, or — for `…::f::{{closure}}` — the function `f`
//! the closure is written in; a `Pipe(a, b)` is keyed by `a`. A detected
//! registration (ordinary code only) is keyed the same way from its text:
//! the schedule label with path qualifiers dropped (`OnEnter(State::A)` →
//! `OnEnter(A)`, like `Debug`) and its name, or its enclosing function for a
//! closure (a factory call counts for both). Matching is one to one:
//!
//! 1. same schedule and name;
//! 2. same name where the detected schedule is not a literal label (a
//!    variable, a parameter, the one-argument `Schedule::add_systems`);
//! 3. every other dumped system is a miss with one category, in this order:
//!    `generic_instance` / `repeated_site` (the name's registrations are all
//!    taken: one generic or helper site reached several times),
//!    `other_schedule` (registered, but only under another literal label),
//!    `macro_rules` / `macro_call` (the name appears only in macro tokens),
//!    `closure` (no closure registered in that function), `indirect` (the
//!    name is in the sources, not in a readable registration),
//!    `not_in_source`.
//!
//! Candidates from test, example and bench targets (never part of the dumped
//! app) are taken after those of the app's own code. Only the sets of names
//! and counts matter, and the dumped systems are
//! processed in sorted order: one dump and one corpus give one result.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use serde::Serialize;
use specengine_code::bevy::{Form, Origin, Registration, Target};

use super::dump::Dump;
use crate::harness::percent;

/// Miss categories, in classification order.
pub const MISS_CATEGORIES: &[&str] = &[
    "generic_instance",
    "repeated_site",
    "other_schedule",
    "macro_rules",
    "macro_call",
    "closure",
    "indirect",
    "not_in_source",
];

/// Nested `Pipe(…)` unwrapped per dumped name, at most.
const MAX_PIPE_DEPTH: usize = 64;

/// The `dump` object of the result. Counts only: no schedule or system name.
#[derive(Serialize)]
pub struct DumpSummary {
    pub schema: Schema,
    pub schedules: usize,
    /// Every dumped system, all crates, sync points included.
    pub systems_total: usize,
    /// Engine sync points (`ApplyDeferred`), never compared.
    pub apply_deferred: usize,
    /// Dumped systems of the corpus's own crates: the comparison base.
    pub systems: usize,
    /// Not in the `schedule_data` schema: always `null`.
    pub observers: Option<usize>,
    /// Not in the `schedule_data` schema: always `null`.
    pub plugins: Option<usize>,
    pub matched: usize,
    pub matched_schedule_exact: usize,
    pub matched_schedule_unverified: usize,
    /// `matched` / `systems`.
    pub match_pct: f64,
    /// Dumped systems whose name a readable registration carries, however
    /// often or under whichever schedule.
    pub name_found_pct: f64,
    /// The same, counting names read from macro tokens too.
    pub name_found_incl_macros_pct: f64,
    pub misses: usize,
    /// Every category of [`MISS_CATEGORIES`], zeros included.
    pub miss_categories: BTreeMap<&'static str, usize>,
    /// Readable system registrations no dumped system took (other apps,
    /// `cfg`-gated code, dead registrations, or a name the dump spells otherwise).
    pub detected_unmatched: usize,
    pub match_basis: &'static str,
}

#[derive(Serialize)]
pub struct Schema {
    /// The schema the reader expects.
    pub expected: &'static str,
    /// Field names outside it (names in `--out`).
    pub unknown_fields: usize,
    /// Schema fields absent from some record (names in `--out`).
    pub missing_fields: usize,
}

/// One row of `dump_match.json` (under `--out` only).
#[derive(Serialize)]
pub struct MatchRow {
    pub schedule: String,
    pub name: String,
    /// `matched_exact`, `matched_schedule_unverified` or `miss`.
    pub status: &'static str,
    pub category: Option<&'static str>,
    /// `file:line` of the registration that took it.
    pub registration: Option<String>,
}

/// A detected registration with the file it came from.
pub struct Located<'a> {
    pub file: &'a str,
    pub registration: &'a Registration,
}

/// Name parts of one dumped system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DumpName {
    /// First path segment: the defining crate (`<T as X>::f` → `T`'s crate).
    pub krate: String,
    /// Last segment without generics; for a closure, the function around it.
    pub terminal: String,
    pub closure: bool,
    /// The terminal segment carries generic arguments.
    pub generic: bool,
}

/// Splits a `type_name` into its parts; `Pipe(a, b)` is read as `a`.
#[must_use]
pub fn dump_name(name: &str) -> DumpName {
    let mut name = name;
    for _ in 0..MAX_PIPE_DEPTH {
        match name
            .strip_prefix("Pipe(")
            .and_then(|inner| inner.strip_suffix(')'))
        {
            Some(inner) => name = first_argument(inner),
            None => break,
        }
    }
    let mut segments = segments(name);
    let mut closure = false;
    while segments.len() > 1 && segments.last().is_some_and(|s| s.starts_with("{{")) {
        segments.pop();
        closure = true;
    }
    let last = segments.last().copied().unwrap_or_default();
    let generic = last.contains('<') && !last.starts_with('<');
    let terminal = if last.starts_with('<') {
        last.to_owned()
    } else {
        last.split('<').next().unwrap_or_default().to_owned()
    };
    let first = segments.first().copied().unwrap_or_default();
    let krate = first
        .trim_start_matches('<')
        .split([':', ' ', '<', '>'])
        .next()
        .unwrap_or_default()
        .to_owned();
    DumpName {
        krate,
        terminal,
        closure,
        generic,
    }
}

/// `name` split at `::` outside `<…>`, `(…)` and `[…]`.
fn segments(name: &str) -> Vec<&str> {
    let bytes = name.as_bytes();
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'<' | b'(' | b'[' => depth += 1,
            b'>' if i > 0 && bytes[i - 1] == b'-' => {}
            b'>' | b')' | b']' => depth -= 1,
            b':' if depth == 0 && bytes.get(i + 1) == Some(&b':') => {
                out.push(&name[start..i]);
                i += 2;
                start = i;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    out.push(&name[start..]);
    out
}

/// The first comma-separated argument of `inner` at depth 0.
fn first_argument(inner: &str) -> &str {
    let bytes = inner.as_bytes();
    let mut depth = 0i32;
    for (i, b) in bytes.iter().enumerate() {
        match b {
            b'<' | b'(' | b'[' => depth += 1,
            b'>' if i > 0 && bytes[i - 1] == b'-' => {}
            b'>' | b')' | b']' => depth -= 1,
            b',' if depth == 0 => return inner[..i].trim(),
            _ => {}
        }
    }
    inner.trim()
}

/// A file of a test, example or bench target (a `tests`, `examples` or
/// `benches` directory anywhere on its corpus-relative path).
fn auxiliary(file: &str) -> bool {
    file.split('/')
        .any(|part| matches!(part, "tests" | "examples" | "benches"))
}

/// A schedule label as `Debug` prints it: whitespace and path qualifiers
/// dropped (`OnEnter(GameState::Menu)` → `OnEnter(Menu)`).
#[must_use]
pub fn normalize_label(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().filter(|c| !c.is_whitespace()).peekable();
    while let Some(c) = chars.next() {
        if c == ':' && chars.peek() == Some(&':') {
            chars.next();
            while out
                .chars()
                .last()
                .is_some_and(|c| c.is_alphanumeric() || c == '_')
            {
                out.pop();
            }
            continue;
        }
        out.push(c);
    }
    out
}

/// A label written as a type or variant (`Update`, `OnEnter(A)`), not a
/// variable, field or call.
fn is_literal_label(normalized: &str) -> bool {
    normalized.starts_with(|c: char| c.is_ascii_uppercase())
}

/// Compares the dump with the detected registrations.
///
/// `crates`: the corpus's own crate names (underscored); `words`: every
/// identifier-like word of the corpus sources.
pub fn compare(
    dump: &Dump,
    detected: &[Located],
    crates: &BTreeSet<String>,
    words: &HashSet<&str>,
) -> (DumpSummary, Vec<MatchRow>) {
    let systems_total = dump.systems.len();
    let apply_deferred = dump.systems.iter().filter(|s| s.apply_deferred).count();
    let mut dumped: Vec<(String, &str, DumpName)> = dump
        .systems
        .iter()
        .filter(|s| !s.apply_deferred)
        .map(|s| {
            (
                normalize_label(&s.schedule),
                s.name.as_str(),
                dump_name(&s.name),
            )
        })
        .filter(|(_, _, parts)| crates.contains(&parts.krate))
        .collect();
    dumped.sort_by(|a, b| (&a.0, a.1).cmp(&(&b.0, b.1)));

    // Readable system registrations and their keys.
    let code: Vec<&Located> = detected
        .iter()
        .filter(|l| {
            l.registration.target == Target::System && l.registration.origin == Origin::Code
        })
        .collect();
    let schedules: Vec<Option<String>> = code
        .iter()
        .map(|l| l.registration.schedule.as_deref().map(normalize_label))
        .collect();
    let mut by_name: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    let mut by_closure: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    // Candidates of the app's own code come before those of test, example
    // and bench targets, which never build the dumped app.
    let mut order: Vec<usize> = (0..code.len()).collect();
    order.sort_by_key(|&index| auxiliary(code[index].file));
    for index in order {
        let registration = code[index].registration;
        match registration.form {
            Form::Path => {
                if let Some(name) = &registration.name {
                    by_name.entry(name).or_default().push(index);
                }
            }
            Form::Factory => {
                if let Some(name) = &registration.name {
                    by_name.entry(name).or_default().push(index);
                    by_closure.entry(name).or_default().push(index);
                }
            }
            Form::Closure => {
                if let Some(function) = &registration.enclosing_fn {
                    by_closure.entry(function).or_default().push(index);
                }
            }
        }
    }
    let macro_names = |origin: Origin| -> BTreeSet<&str> {
        detected
            .iter()
            .filter(|l| l.registration.target == Target::System && l.registration.origin == origin)
            .filter_map(|l| l.registration.name.as_deref())
            .collect()
    };
    let macro_rules = macro_names(Origin::MacroRules);
    let macro_call = macro_names(Origin::MacroCall);

    let mut used = vec![false; code.len()];
    let mut taken: Vec<Option<(usize, &'static str)>> = vec![None; dumped.len()];
    let pool = |parts: &DumpName| -> &[usize] {
        let map = if parts.closure { &by_closure } else { &by_name };
        map.get(parts.terminal.as_str()).map_or(&[], Vec::as_slice)
    };
    // Pass 1: same schedule and name. Pass 2: name, detected schedule not literal.
    for exact in [true, false] {
        for (slot, (schedule, _, parts)) in taken.iter_mut().zip(&dumped) {
            if slot.is_some() {
                continue;
            }
            let found = pool(parts).iter().copied().find(|&index| {
                !used[index]
                    && match &schedules[index] {
                        Some(detected) if exact => detected == schedule,
                        Some(detected) => !is_literal_label(detected),
                        None => !exact,
                    }
            });
            if let Some(index) = found {
                used[index] = true;
                let status = if exact {
                    "matched_exact"
                } else {
                    "matched_schedule_unverified"
                };
                *slot = Some((index, status));
            }
        }
    }

    let mut categories: BTreeMap<&'static str, usize> =
        MISS_CATEGORIES.iter().map(|c| (*c, 0)).collect();
    let mut rows = Vec::with_capacity(dumped.len());
    let (mut exact, mut unverified, mut name_found, mut macro_found) = (0, 0, 0, 0);
    for ((schedule, name, parts), slot) in dumped.iter().zip(&taken) {
        let (status, category, registration) = match slot {
            Some((index, status)) => {
                if *status == "matched_exact" {
                    exact += 1;
                } else {
                    unverified += 1;
                }
                name_found += 1;
                let located = code[*index];
                let at = format!("{}:{}", located.file, located.registration.line);
                (*status, None, Some(at))
            }
            None => {
                let candidates = pool(parts);
                let category = if !candidates.is_empty() {
                    name_found += 1;
                    if candidates.iter().all(|&index| used[index]) {
                        if parts.generic {
                            "generic_instance"
                        } else {
                            "repeated_site"
                        }
                    } else {
                        "other_schedule"
                    }
                } else if !parts.closure && macro_rules.contains(parts.terminal.as_str()) {
                    macro_found += 1;
                    "macro_rules"
                } else if !parts.closure && macro_call.contains(parts.terminal.as_str()) {
                    macro_found += 1;
                    "macro_call"
                } else if parts.closure {
                    "closure"
                } else if words.contains(parts.terminal.as_str()) {
                    "indirect"
                } else {
                    "not_in_source"
                };
                *categories.entry(category).or_default() += 1;
                ("miss", Some(category), None)
            }
        };
        rows.push(MatchRow {
            schedule: schedule.clone(),
            name: (*name).to_owned(),
            status,
            category,
            registration,
        });
    }
    let systems = dumped.len();
    let matched = exact + unverified;
    let summary = DumpSummary {
        schema: Schema {
            expected: "bevy_dev_tools 0.19 schedule_data",
            unknown_fields: dump.unknown_fields.len(),
            missing_fields: dump.missing_fields.len(),
        },
        schedules: dump.schedules,
        systems_total,
        apply_deferred,
        systems,
        observers: None,
        plugins: None,
        matched,
        matched_schedule_exact: exact,
        matched_schedule_unverified: unverified,
        match_pct: percent(matched, systems),
        name_found_pct: percent(name_found, systems),
        name_found_incl_macros_pct: percent(name_found + macro_found, systems),
        misses: systems - matched,
        miss_categories: categories,
        detected_unmatched: used.iter().filter(|u| !**u).count(),
        match_basis: "schedule+name",
    };
    (summary, rows)
}
