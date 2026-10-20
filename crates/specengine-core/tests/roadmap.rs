//! RD-3 and RD-4 of docs/features/roadmap-items.md ("The plan"): core's
//! pure `plan` over real parses (the real parser on real Markdown, the
//! real `[ids]` and `[roadmap]` readers) — the order in a release (the
//! chain, priority ties, cycles), the groups, `waits_on`, the sums, the
//! cut, `current` and the notes — and the two fixtures' plans against their
//! `expected.json` (the hand-made oracle the CLI test also reads).
//!
//! Named mutations: M3 (ties broken by ID alone) turns
//! `ties_break_by_priority_then_id` red; M4 (a cycle's members dropped or
//! listed twice) `cycle_members_stay_listed_with_a_note`; M5 (an
//! unestimated item counted as 0) `unestimated_items_counted_apart`.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

mod common;

use common::check::fixture_input;
use common::fixture;
use serde_json::Value;
use specengine_core::IdSchemeToml;
use specengine_core::check::{CheckFile, CheckInput};
use specengine_core::roadmap::{GroupKind, Plan, PlanGroup, PlanItem, plan};
use specengine_core::roadmap_toml::RoadmapConfig;
use specengine_model::IdScheme;

// ------------------------------------------------------------------ inputs

/// `[ids]` of every case: the item prefix (with a legacy alias), a
/// name-shaped prefix for non-item documents and sections.
const IDS: &str = "[ids]\n\
RM  = { kind = \"plan-item\",   width = 4, aliases_from = [\"PLAN\"] }\n\
BAY = { kind = \"harbor-zone\", shape = \"name\" }\n";

/// The default `[roadmap]`: three releases, the cut at the second.
const ROADMAP: &str = "[roadmap]\n\
prefix     = \"RM\"\n\
dir        = \"plan\"\n\
releases   = [\"a\", \"b\", \"c\"]\n\
mvp_cut    = \"b\"\n\
priorities = [\"must\", \"should\", \"could\"]\n\
unit       = \"days\"\n";

fn config(roadmap: &str) -> String {
    format!("[project]\nslug = \"probe\"\n\n{IDS}\n{roadmap}")
}

/// One document of the input, front-matter written key by key.
#[derive(Clone)]
struct Doc {
    id: String,
    path: String,
    class: String,
    status: Option<String>,
    release: Option<String>,
    priority: Option<String>,
    estimate: Option<String>,
    depends_on: Vec<String>,
    title: Option<String>,
    extra: String,
}

/// An item `id` at `plan/<id>.md`: `draft`, titled, nothing else set.
fn item(id: &str) -> Doc {
    Doc {
        id: id.to_owned(),
        path: format!("plan/{id}.md"),
        class: "spec".to_owned(),
        status: Some("draft".to_owned()),
        release: None,
        priority: None,
        estimate: None,
        depends_on: Vec::new(),
        title: Some(format!("Item {id}")),
        extra: String::new(),
    }
}

impl Doc {
    /// `release:` as written in YAML.
    fn rel(mut self, value: &str) -> Self {
        self.release = Some(value.to_owned());
        self
    }
    /// `priority:` as written in YAML.
    fn pri(mut self, value: &str) -> Self {
        self.priority = Some(value.to_owned());
        self
    }
    /// `estimate:` as written in YAML.
    fn est(mut self, value: &str) -> Self {
        self.estimate = Some(value.to_owned());
        self
    }
    fn deps(mut self, ids: &[&str]) -> Self {
        self.depends_on = ids.iter().map(|id| (*id).to_owned()).collect();
        self
    }
    fn status(mut self, status: &str) -> Self {
        self.status = Some(status.to_owned());
        self
    }
    fn no_status(mut self) -> Self {
        self.status = None;
        self
    }
    fn at(mut self, path: &str) -> Self {
        self.path = path.to_owned();
        self
    }
    fn class(mut self, class: &str) -> Self {
        self.class = class.to_owned();
        self
    }
    fn body(mut self, extra: &str) -> Self {
        self.extra = extra.to_owned();
        self
    }

    fn text(&self) -> String {
        let mut out = format!("---\nclass: {}\nid: {}\n", self.class, self.id);
        if let Some(status) = &self.status {
            out.push_str(&format!("status: {status}\n"));
            if status == "shipped" {
                out.push_str("shipped: 2026-10-01\n");
            }
        }
        out.push_str("scope: [probe]\n");
        for (key, value) in [
            ("release", &self.release),
            ("priority", &self.priority),
            ("estimate", &self.estimate),
        ] {
            if let Some(value) = value {
                out.push_str(&format!("{key}: {value}\n"));
            }
        }
        if !self.depends_on.is_empty() {
            out.push_str(&format!(
                "links:\n  depends_on: [{}]\n",
                self.depends_on.join(", ")
            ));
        }
        out.push_str("---\n\n");
        if let Some(title) = &self.title {
            out.push_str(&format!("# {title}\n\n"));
        }
        out.push_str("The goal of the item.\n");
        if !self.extra.is_empty() {
            out.push('\n');
            out.push_str(&self.extra);
            out.push('\n');
        }
        out
    }
}

/// The scheme and the checked `[roadmap]` of `toml`.
fn read_config(toml: &str) -> (IdScheme, RoadmapConfig) {
    let scheme =
        IdScheme::from_toml(toml).unwrap_or_else(|error| panic!("{}", error.at("specengine.toml")));
    let roadmap = RoadmapConfig::from_toml(toml, &scheme)
        .unwrap_or_else(|error| panic!("{}", error.at("specengine.toml")))
        .expect("a [roadmap] table");
    (scheme, roadmap)
}

/// `docs` and `others` (path, text) parsed into one input, in that order.
fn input_of(scheme: &IdScheme, docs: &[Doc], others: &[(&str, &str)]) -> CheckInput {
    let mut files: Vec<CheckFile> = docs
        .iter()
        .map(|doc| CheckFile::parse(doc.path.clone(), doc.text().into_bytes(), scheme))
        .collect();
    files.extend(
        others
            .iter()
            .map(|(path, text)| CheckFile::parse(*path, text.as_bytes().to_vec(), scheme)),
    );
    CheckInput {
        files,
        problems: Vec::new(),
    }
}

fn plan_with(toml: &str, docs: &[Doc], others: &[(&str, &str)]) -> Plan {
    let (scheme, roadmap) = read_config(toml);
    plan(&input_of(&scheme, docs, others), &roadmap)
}

fn plan_of(docs: &[Doc]) -> Plan {
    plan_with(&config(ROADMAP), docs, &[])
}

/// A non-item document, `BAY-HARBOR`, holding a section with the item
/// prefix, `{#RM-0090}`: neither is an item.
const HARBOR: (&str, &str) = (
    "docs/spec/harbor.md",
    "---\nclass: spec\nid: BAY-HARBOR\nstatus: draft\nscope: [probe]\n---\n\n# Harbor\n\nText.\n\n## Deferred {#RM-0090}\n\nA section with the item prefix.\n",
);

// ------------------------------------------------------------------- views

/// The group of a release `name`, or `pool` / `unplaced`.
#[track_caller]
fn group<'p>(plan: &'p Plan, name: &str) -> &'p PlanGroup {
    plan.groups
        .iter()
        .find(|group| match group.kind {
            GroupKind::Release => group.name.as_deref() == Some(name),
            GroupKind::Pool => name == "pool",
            GroupKind::Unplaced => name == "unplaced",
        })
        .unwrap_or_else(|| panic!("no group {name}: {}", shape(plan)))
}

fn ids(group: &PlanGroup) -> Vec<&str> {
    group.items.iter().map(|item| item.id.as_str()).collect()
}

#[track_caller]
fn find<'p>(plan: &'p Plan, id: &str) -> &'p PlanItem {
    let found: Vec<&PlanItem> = plan
        .groups
        .iter()
        .flat_map(|group| group.items.iter())
        .filter(|item| item.id == id)
        .collect();
    assert_eq!(found.len(), 1, "{id} listed once: {}", shape(plan));
    found[0]
}

fn sum(group: &PlanGroup) -> (u64, u64, u64) {
    (group.sum.min, group.sum.max, group.sum.unestimated)
}

/// A one-line picture of the plan for failure messages.
fn shape(plan: &Plan) -> String {
    plan.groups
        .iter()
        .map(|group| {
            format!(
                "{}{} [{}]",
                group.kind.as_str(),
                group
                    .name
                    .as_deref()
                    .map(|name| format!(" {name}"))
                    .unwrap_or_default(),
                group
                    .items
                    .iter()
                    .map(|item| {
                        format!(
                            "{}{}{}",
                            item.position.map(|p| format!("{p}.")).unwrap_or_default(),
                            item.id,
                            if item.cycle { "*" } else { "" }
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

/// Positions 1..=n in a release group, in order.
#[track_caller]
fn assert_positions(group: &PlanGroup) {
    let positions: Vec<Option<usize>> = group.items.iter().map(|item| item.position).collect();
    let want: Vec<Option<usize>> = (1..=group.items.len()).map(Some).collect();
    assert_eq!(positions, want, "1-based positions in the group");
}

// ------------------------------------------------------------------- RD-3

/// The groups: every release in the table's order (not sorted: `c`, `a`,
/// `b`), an empty one included, then the pool, then the unplaced items.
#[test]
fn releases_follow_config_order() {
    let toml = config(&ROADMAP.replace("[\"a\", \"b\", \"c\"]", "[\"c\", \"a\", \"b\"]"));
    let plan = plan_with(
        &toml,
        &[
            item("RM-0001").rel("a"),
            item("RM-0002").rel("c"),
            item("RM-0003"),
            item("RM-0004").rel("zz"),
        ],
        &[],
    );
    let kinds: Vec<(GroupKind, Option<&str>)> = plan
        .groups
        .iter()
        .map(|group| (group.kind, group.name.as_deref()))
        .collect();
    assert_eq!(
        kinds,
        [
            (GroupKind::Release, Some("c")),
            (GroupKind::Release, Some("a")),
            (GroupKind::Release, Some("b")),
            (GroupKind::Pool, None),
            (GroupKind::Unplaced, None),
        ],
        "{}",
        shape(&plan)
    );
    assert_eq!(ids(group(&plan, "c")), ["RM-0002"]);
    assert_eq!(ids(group(&plan, "a")), ["RM-0001"]);
    assert!(group(&plan, "b").items.is_empty());
    assert_eq!((group(&plan, "b").open, group(&plan, "b").done), (0, 0));
    assert_eq!(plan.current.as_deref(), Some("c"), "the first in order");
}

/// The text grammar prints `pool | …` and `unplaced | …` unbracketed: both
/// groups are part of the whole plan even when empty.
#[test]
fn empty_pool_and_unplaced_groups_are_listed() {
    let plan = plan_of(&[item("RM-0001").rel("a")]);
    let kinds: Vec<GroupKind> = plan.groups.iter().map(|group| group.kind).collect();
    assert_eq!(
        kinds,
        [
            GroupKind::Release,
            GroupKind::Release,
            GroupKind::Release,
            GroupKind::Pool,
            GroupKind::Unplaced
        ],
        "{}",
        shape(&plan)
    );
}

/// M3: ties broken by ID alone. Key (priority rank, ID): `must` before
/// `should` before `could`, absent and off-scale after every priority, in
/// ID order; IDs compare as strings (`RM-10` before `RM-9`).
#[test]
fn ties_break_by_priority_then_id() {
    let plan = plan_of(&[
        item("RM-0001").rel("a").pri("could"),
        item("RM-0002").rel("a").pri("should"),
        item("RM-0003").rel("a").pri("should"),
        item("RM-0004").rel("a"),
        item("RM-0005").rel("a").pri("must"),
        item("RM-0006").rel("a").pri("urgent"),
        item("RM-9").rel("b").pri("should"),
        item("RM-10").rel("b").pri("should"),
        item("RM-0007").rel("b").pri("could"),
        item("RM-0042").pri("could"),
        item("RM-0041").pri("must"),
        item("RM-0040"),
    ]);
    let a = group(&plan, "a");
    assert_eq!(
        ids(a),
        [
            "RM-0005", "RM-0002", "RM-0003", "RM-0001", "RM-0004", "RM-0006"
        ],
        "{}",
        shape(&plan)
    );
    assert_positions(a);
    assert_eq!(
        ids(group(&plan, "b")),
        ["RM-10", "RM-9", "RM-0007"],
        "IDs compare as strings"
    );
    let pool = group(&plan, "pool");
    assert_eq!(
        ids(pool),
        ["RM-0041", "RM-0042", "RM-0040"],
        "the pool by key"
    );
    assert!(pool.items.iter().all(|item| item.position.is_none()));
}

/// A chain of three against its priorities: the chain wins, then the key
/// orders what is ready.
#[test]
fn chain_beats_priority() {
    let plan = plan_of(&[
        item("RM-0001").rel("a").pri("could"),
        item("RM-0002").rel("a").pri("should").deps(&["RM-0001"]),
        item("RM-0003").rel("a").pri("must").deps(&["RM-0002"]),
        item("RM-0004").rel("a").pri("should"),
        item("RM-0005").rel("a").pri("could"),
    ]);
    // Ready: 0001 (could), 0004 (should), 0005 (could) -> 0004, 0001; then
    // 0002 (should) beats 0005 (could); then 0003 (must); then 0005.
    let a = group(&plan, "a");
    assert_eq!(
        ids(a),
        ["RM-0004", "RM-0001", "RM-0002", "RM-0003", "RM-0005"],
        "{}",
        shape(&plan)
    );
    assert_positions(a);
    assert!(a.items.iter().all(|item| !item.cycle));
    assert_eq!(find(&plan, "RM-0003").waits_on, ["RM-0002"]);
    assert_eq!(find(&plan, "RM-0002").waits_on, ["RM-0001"]);
    assert!(plan.notes.is_empty(), "{:?}", plan.notes);
}

/// M4: a cycle's members dropped or listed twice. A two-cycle, a
/// three-cycle and a self-loop in one release: every member listed once,
/// flagged, its members in key order one after another, one note per
/// cycle (members sorted), the component placed by its smallest key.
#[test]
fn cycle_members_stay_listed_with_a_note() {
    let plan = plan_of(&[
        item("RM-0001").rel("a").pri("could").deps(&["RM-0002"]),
        item("RM-0002").rel("a").pri("must").deps(&["RM-0001"]),
        item("RM-0003").rel("a").pri("should").deps(&["RM-0004"]),
        item("RM-0004").rel("a").deps(&["RM-0005"]),
        item("RM-0005").rel("a").pri("must").deps(&["RM-0003"]),
        item("RM-0006").rel("a").pri("should").deps(&["RM-0006"]),
        item("RM-0007").rel("a").pri("must"),
    ]);
    let a = group(&plan, "a");
    assert_eq!(
        ids(a),
        [
            "RM-0002", "RM-0001", "RM-0005", "RM-0003", "RM-0004", "RM-0007", "RM-0006"
        ],
        "{}",
        shape(&plan)
    );
    assert_positions(a);
    assert_eq!(a.open, 7);
    let cycles: Vec<(&str, bool)> = a
        .items
        .iter()
        .map(|item| (item.id.as_str(), item.cycle))
        .collect();
    assert_eq!(
        cycles,
        [
            ("RM-0002", true),
            ("RM-0001", true),
            ("RM-0005", true),
            ("RM-0003", true),
            ("RM-0004", true),
            ("RM-0007", false),
            ("RM-0006", true),
        ]
    );
    assert_eq!(
        plan.notes,
        [
            "depends_on cycle: RM-0001, RM-0002",
            "depends_on cycle: RM-0003, RM-0004, RM-0005",
            "depends_on cycle: RM-0006",
        ]
    );
    assert_eq!(find(&plan, "RM-0001").waits_on, ["RM-0002"]);
    assert_eq!(find(&plan, "RM-0005").waits_on, ["RM-0003"]);
}

/// An item depending on a cycle comes after every member, whatever its
/// priority.
#[test]
fn cycle_dependent_follows_the_cycle() {
    let plan = plan_of(&[
        item("RM-0001").rel("a").pri("could").deps(&["RM-0002"]),
        item("RM-0002").rel("a").pri("could").deps(&["RM-0001"]),
        item("RM-0003").rel("a").pri("must").deps(&["RM-0002"]),
        item("RM-0004").rel("a").pri("should"),
        item("RM-0005").rel("a").pri("could"),
    ]);
    // Ready: the cycle (could, RM-0001), 0004 (should), 0005 (could, RM-0005):
    // 0004, the cycle, then 0003 (must) before 0005.
    let a = group(&plan, "a");
    assert_eq!(
        ids(a),
        ["RM-0004", "RM-0001", "RM-0002", "RM-0003", "RM-0005"],
        "{}",
        shape(&plan)
    );
    assert!(!find(&plan, "RM-0003").cycle);
    assert_eq!(find(&plan, "RM-0003").waits_on, ["RM-0002"]);
    assert_eq!(plan.notes, ["depends_on cycle: RM-0001, RM-0002"]);
}

/// Edges to items of other groups never reorder a group (they show in
/// `waits_on`); the pool and unplaced go by key even over their own edges.
#[test]
fn edges_across_groups_never_reorder() {
    let plan = plan_of(&[
        item("RM-0001").rel("a").pri("must").deps(&["RM-0010"]),
        item("RM-0002").rel("a").pri("should"),
        item("RM-0003").rel("a").pri("could").deps(&["RM-0040"]),
        item("RM-0010").rel("b").pri("could").deps(&["RM-0001"]),
        item("RM-0011").rel("b").pri("must"),
        item("RM-0040").pri("must").deps(&["RM-0041"]),
        item("RM-0041").pri("could"),
        item("RM-0050").rel("zz").pri("must").deps(&["RM-0051"]),
        item("RM-0051").rel("zz").pri("could"),
    ]);
    assert_eq!(ids(group(&plan, "a")), ["RM-0001", "RM-0002", "RM-0003"]);
    assert_eq!(ids(group(&plan, "b")), ["RM-0011", "RM-0010"]);
    assert_eq!(ids(group(&plan, "pool")), ["RM-0040", "RM-0041"]);
    assert_eq!(ids(group(&plan, "unplaced")), ["RM-0050", "RM-0051"]);
    assert_eq!(find(&plan, "RM-0001").waits_on, ["RM-0010"]);
    assert_eq!(find(&plan, "RM-0010").waits_on, ["RM-0001"]);
    assert_eq!(find(&plan, "RM-0003").waits_on, ["RM-0040"]);
    assert_eq!(find(&plan, "RM-0040").waits_on, ["RM-0041"]);
    assert_eq!(find(&plan, "RM-0050").waits_on, ["RM-0051"]);
    assert!(
        plan.groups
            .iter()
            .all(|group| group.items.iter().all(|item| !item.cycle))
    );
    assert!(
        !plan
            .notes
            .iter()
            .any(|note| note.starts_with("depends_on cycle")),
        "a cross-group loop is no cycle of a group: {:?}",
        plan.notes
    );
}

/// `waits_on`: the open items an item's `depends_on` resolves to, in
/// written order, once each — not a done item, a non-item document, a
/// section with the prefix, a dangling ID; a `#section` resolves to its
/// document.
#[test]
fn waits_on_lists_open_predecessors_only() {
    let plan = plan_with(
        &config(ROADMAP),
        &[
            item("RM-0001").rel("a").status("shipped"),
            item("RM-0003").rel("a"),
            item("RM-0004")
                .rel("a")
                .body("## Part {#BAY-PART}\n\nA part of the item."),
            item("RM-0005").rel("a").status("abandoned"),
            item("RM-0006").rel("a"),
            item("RM-0009").rel("a"),
            item("RM-0010").rel("a").deps(&[
                "RM-0001",
                "RM-0009",
                "BAY-HARBOR",
                "RM-0099",
                "RM-0090",
                "RM-0003",
                "RM-0009",
                "RM-0004#BAY-PART",
                "RM-0005",
            ]),
        ],
        &[HARBOR],
    );
    assert_eq!(
        find(&plan, "RM-0010").waits_on,
        ["RM-0009", "RM-0003", "RM-0004"],
        "{}",
        shape(&plan)
    );
    // The edges order the release: RM-0010 after the three.
    assert_eq!(
        ids(group(&plan, "a")),
        ["RM-0003", "RM-0004", "RM-0006", "RM-0009", "RM-0010"]
    );
    // The section `{#RM-0090}` is no item.
    assert!(
        !plan
            .groups
            .iter()
            .any(|group| group.items.iter().any(|item| item.id == "RM-0090")),
        "{}",
        shape(&plan)
    );
}

/// A legacy ID (`aliases_from`) resolves as the check resolves it: to
/// its item, which `waits_on` names by its ID and the order follows.
#[test]
fn a_legacy_alias_resolves_to_its_item() {
    let plan = plan_of(&[
        item("RM-0001").rel("a").pri("could"),
        item("RM-0002").rel("a").pri("must").deps(&["PLAN-0001"]),
    ]);
    assert_eq!(
        ids(group(&plan, "a")),
        ["RM-0001", "RM-0002"],
        "{}",
        shape(&plan)
    );
    assert_eq!(find(&plan, "RM-0002").waits_on, ["RM-0001"]);
}

/// An off-scale priority ranks with an absent one, after every priority,
/// shown as written, with a note; an absent one has none. Priorities are
/// case-sensitive.
#[test]
fn off_scale_priority_ranks_last_with_a_note() {
    let plan = plan_of(&[
        item("RM-0001").rel("a").pri("urgent"),
        item("RM-0002").rel("a"),
        item("RM-0003").rel("a").pri("could"),
        item("RM-0004").rel("a").pri("MUST"),
        item("RM-0005").pri("later"),
    ]);
    assert_eq!(
        ids(group(&plan, "a")),
        ["RM-0003", "RM-0001", "RM-0002", "RM-0004"],
        "{}",
        shape(&plan)
    );
    assert_eq!(find(&plan, "RM-0001").priority.as_deref(), Some("urgent"));
    assert_eq!(find(&plan, "RM-0002").priority, None);
    assert_eq!(
        plan.notes,
        [
            "RM-0001: priority \"urgent\" is not in [roadmap] priorities",
            "RM-0004: priority \"MUST\" is not in [roadmap] priorities",
            "RM-0005: priority \"later\" is not in [roadmap] priorities",
        ]
    );
}

/// An ID two files hold: the first by path is the item (whatever the
/// input order), the other left out with a note.
#[test]
fn duplicate_id_reads_the_first_by_path() {
    let docs = [
        item("RM-0001").rel("b").at("plan/z.md"),
        item("RM-0001").rel("a").at("plan/a.md"),
    ];
    for docs in [docs.to_vec(), docs.iter().rev().cloned().collect()] {
        let plan = plan_of(&docs);
        let read = find(&plan, "RM-0001");
        assert_eq!(read.path, "plan/a.md");
        assert_eq!(read.release.as_deref(), Some("a"));
        assert!(group(&plan, "b").items.is_empty());
        assert_eq!((plan.open, plan.done), (1, 0));
        assert_eq!(
            plan.notes,
            ["RM-0001: also defined in plan/z.md; plan/a.md is read"]
        );
    }
}

/// Canon `spec-roadmap` "Notes" (1a iteration 2, a): a priority that is no
/// string (`frontmatter-type`) ranks with an absent one, after every
/// priority, `priority` null, with the note `<ID>: priority is not a
/// string`; an absent one has none.
#[test]
fn mistyped_priority_ranks_last_with_a_note() {
    let plan = plan_of(&[
        item("RM-0001").rel("a").pri("[must]"),
        item("RM-0002").rel("a").pri("1"),
        item("RM-0003").rel("a").pri("{p: must}"),
        item("RM-0004").rel("a"),
        item("RM-0005").rel("a").pri("could"),
        item("RM-0006").rel("a").pri("[must]").status("shipped"),
    ]);
    assert_eq!(
        ids(group(&plan, "a")),
        ["RM-0005", "RM-0001", "RM-0002", "RM-0003", "RM-0004"],
        "{}",
        shape(&plan)
    );
    for id in ["RM-0001", "RM-0002", "RM-0003"] {
        assert_eq!(find(&plan, id).priority, None, "{id}");
    }
    assert_eq!(
        plan.notes,
        [
            "RM-0001: priority is not a string",
            "RM-0002: priority is not a string",
            "RM-0003: priority is not a string",
        ],
        "open items only; none for an absent priority"
    );
}

/// Canon `spec-roadmap` "Notes" (1a iteration 2, c): the duplicate note
/// when the file read or the one left out is open — a shipped file read
/// (the first by path) with an open duplicate left out has it; both done,
/// none.
#[test]
fn a_done_item_read_with_an_open_duplicate_left_out_has_a_note() {
    let plan = plan_of(&[
        item("RM-0001").rel("b").at("plan/z.md"),
        item("RM-0001").rel("a").status("shipped").at("plan/a.md"),
        item("RM-0002").rel("a").status("shipped").at("plan/b.md"),
        item("RM-0002").rel("a").status("abandoned").at("plan/y.md"),
    ]);
    assert!(
        plan.groups.iter().all(|group| group.items.is_empty()),
        "the done file is read, the open duplicate left out: {}",
        shape(&plan)
    );
    assert_eq!((group(&plan, "a").done, group(&plan, "b").open), (2, 0));
    assert_eq!((plan.open, plan.done), (0, 2));
    assert_eq!(
        plan.notes,
        ["RM-0001: also defined in plan/z.md; plan/a.md is read"]
    );
}

// ------------------------------------------------------------------- RD-4

/// M5: an unestimated item counted as 0. An absent and a bad estimate
/// are counted in `unestimated`, never in the bounds; a group of only
/// unestimated items sums 0-0 with its count.
#[test]
fn unestimated_items_counted_apart() {
    let plan = plan_of(&[
        item("RM-0001").rel("a").est("2-4"),
        item("RM-0002").rel("a"),
        item("RM-0003").rel("a").est("x"),
        item("RM-0010").rel("b"),
        item("RM-0020").rel("c").est("3"),
        item("RM-0040"),
        item("RM-0041").est("1"),
    ]);
    assert_eq!(sum(group(&plan, "a")), (2, 4, 2));
    assert_eq!(sum(group(&plan, "b")), (0, 0, 1));
    assert_eq!(sum(group(&plan, "c")), (3, 3, 0));
    assert_eq!(sum(group(&plan, "pool")), (1, 1, 1));
    assert_eq!(
        (plan.total.min, plan.total.max, plan.total.unestimated),
        (5, 7, 3)
    );
    let cut = plan.cut.as_ref().expect("mvp_cut b");
    assert_eq!(
        (
            cut.release.as_str(),
            cut.sum.min,
            cut.sum.max,
            cut.sum.unestimated
        ),
        ("b", 2, 4, 3)
    );
    assert_eq!(find(&plan, "RM-0002").estimate, None);
}

/// Done items are counted, never summed or listed; open ones (any status
/// but `shipped`, `abandoned`: absent, invalid, `in-progress`) are both.
#[test]
fn done_items_leave_the_sums() {
    let plan = plan_of(&[
        item("RM-0001").rel("a").est("5").status("shipped"),
        item("RM-0002").rel("a").est("3-9").status("abandoned"),
        item("RM-0003").rel("a").est("1"),
        item("RM-0004").rel("a").est("1").status("in-progress"),
        item("RM-0005").rel("a").est("1").no_status(),
        item("RM-0006").rel("a").est("1").status("Shipped"),
        item("RM-0007").rel("a").est("1").status("wip"),
        item("RM-0040").est("7").status("shipped"),
        item("RM-0050").rel("zz").est("8").status("abandoned"),
    ]);
    let a = group(&plan, "a");
    assert_eq!(
        ids(a),
        ["RM-0003", "RM-0004", "RM-0005", "RM-0006", "RM-0007"]
    );
    assert_eq!((a.open, a.done), (5, 2));
    assert_eq!(sum(a), (5, 5, 0));
    let pool = group(&plan, "pool");
    assert_eq!((pool.open, pool.done, sum(pool)), (0, 1, (0, 0, 0)));
    let unplaced = group(&plan, "unplaced");
    assert_eq!((unplaced.open, unplaced.done), (0, 1));
    assert!(unplaced.items.is_empty());
    assert_eq!((plan.open, plan.done), (5, 4));
    assert_eq!(find(&plan, "RM-0005").status, None);
    assert_eq!(
        find(&plan, "RM-0004").status.as_deref(),
        Some("in-progress")
    );
}

/// The cut sums the releases from the first through `mvp_cut`; the total
/// every release; the pool and the unplaced neither.
#[test]
fn cut_sums_releases_through_mvp_cut() {
    let docs = [
        item("RM-0001").rel("a").est("1"),
        item("RM-0002").rel("b").est("2-3"),
        item("RM-0003").rel("c").est("10"),
        item("RM-0040").est("100"),
        item("RM-0050").rel("zz").est("1000"),
    ];
    let plan = plan_of(&docs);
    let cut = plan.cut.as_ref().expect("a cut");
    assert_eq!(
        (
            cut.release.as_str(),
            cut.sum.min,
            cut.sum.max,
            cut.sum.unestimated
        ),
        ("b", 3, 4, 0)
    );
    assert_eq!((plan.total.min, plan.total.max), (13, 14));
    assert!(group(&plan, "b").mvp_cut);
    assert!(!group(&plan, "a").mvp_cut && !group(&plan, "c").mvp_cut);
    for (at, want) in [("a", (1, 1)), ("c", (13, 14))] {
        let toml = config(&ROADMAP.replace("mvp_cut    = \"b\"", &format!("mvp_cut = \"{at}\"")));
        let plan = plan_with(&toml, &docs, &[]);
        let cut = plan.cut.as_ref().expect("a cut");
        assert_eq!(
            (cut.release.as_str(), cut.sum.min, cut.sum.max),
            (at, want.0, want.1)
        );
    }
}

#[test]
fn no_cut_without_mvp_cut() {
    let toml = config(&ROADMAP.replace("mvp_cut    = \"b\"\n", ""));
    let plan = plan_with(&toml, &[item("RM-0001").rel("a").est("1")], &[]);
    assert_eq!(plan.cut, None);
    assert!(plan.groups.iter().all(|group| !group.mvp_cut));
    assert_eq!((plan.total.min, plan.total.max), (1, 1));
}

/// `current`: the first release in order holding an open item; a release
/// of done items is skipped; open pool or unplaced work never makes one.
#[test]
fn current_is_the_first_release_with_open_work() {
    let plan = plan_of(&[
        item("RM-0001").rel("a").status("shipped"),
        item("RM-0002").rel("a").status("abandoned"),
        item("RM-0003").rel("b").status("in-progress"),
        item("RM-0004").rel("c"),
    ]);
    assert_eq!(plan.current.as_deref(), Some("b"));
    let flags: Vec<bool> = plan
        .groups
        .iter()
        .filter(|group| group.kind == GroupKind::Release)
        .map(|group| group.current)
        .collect();
    assert_eq!(flags, [false, true, false]);
    assert!(
        plan.groups
            .iter()
            .filter(|group| group.kind != GroupKind::Release)
            .all(|group| !group.current)
    );

    let plan = plan_of(&[
        item("RM-0001").rel("a").status("shipped"),
        item("RM-0040"),
        item("RM-0050").rel("zz"),
    ]);
    assert_eq!(plan.current, None);
    assert!(plan.groups.iter().all(|group| !group.current));
    assert_eq!(plan_of(&[]).current, None);
}

/// Any other estimate text, or a mistyped value, is unestimated with a
/// note; an absent one has none; a done item's has none.
#[test]
fn bad_estimate_is_unestimated_with_a_note() {
    let bad = [
        ("RM-0001", "4-2", "4-2"),
        ("RM-0002", "\"-3\"", "-3"),
        ("RM-0003", "\"+3\"", "+3"),
        ("RM-0004", "3 - 4", "3 - 4"),
        ("RM-0005", "\"3-\"", "3-"),
        ("RM-0006", "\"-\"", "-"),
        ("RM-0007", "soon", "soon"),
        ("RM-0008", "1-2-3", "1-2-3"),
        ("RM-0009", "\"3.5\"", "3.5"),
        ("RM-0010", "\"1e3\"", "1e3"),
        ("RM-0011", "\"\u{ff13}\"", "\u{ff13}"),
        ("RM-0012", "\"\u{663}\"", "\u{663}"),
        ("RM-0013", "\"0x10\"", "0x10"),
        ("RM-0014", "-3", "-3"),
    ];
    let mut docs: Vec<Doc> = bad
        .iter()
        .map(|(id, yaml, _)| item(id).rel("a").est(yaml))
        .collect();
    docs.push(item("RM-0020").rel("a"));
    docs.push(item("RM-0021").rel("a").est("4-2").status("shipped"));
    let plan = plan_of(&docs);
    for (id, _, text) in bad {
        assert_eq!(find(&plan, id).estimate, None, "{id}");
        let note = format!("{id}: estimate \"{text}\" is not N or N-M");
        assert!(plan.notes.contains(&note), "{note} in {:?}", plan.notes);
    }
    assert_eq!(sum(group(&plan, "a")), (0, 0, 15));
    assert!(
        !plan.notes.iter().any(|note| note.starts_with("RM-0020")),
        "an absent estimate has no note: {:?}",
        plan.notes
    );
    assert!(
        !plan.notes.iter().any(|note| note.starts_with("RM-0021")),
        "a done item has no note: {:?}",
        plan.notes
    );

    // Mistyped values (each also the parser's `frontmatter-type`), and
    // padded or empty strings: unestimated, with a note on the estimate.
    let odd = [
        ("RM-0001", "[1]"),
        ("RM-0002", "{a: 1}"),
        ("RM-0003", "true"),
        ("RM-0004", "2.5"),
        ("RM-0005", "\" 3\""),
        ("RM-0006", "\"3 \""),
        ("RM-0007", "\"\""),
    ];
    let docs: Vec<Doc> = odd
        .iter()
        .map(|(id, yaml)| item(id).rel("a").est(yaml))
        .collect();
    let plan = plan_of(&docs);
    for (id, yaml) in odd {
        assert_eq!(find(&plan, id).estimate, None, "{id}: {yaml}");
        assert!(
            plan.notes
                .iter()
                .any(|note| note.starts_with(&format!("{id}: estimate"))),
            "{id} ({yaml}): a note in {:?}",
            plan.notes
        );
    }
    assert_eq!(sum(group(&plan, "a")), (0, 0, 7));
}

/// Each bound at most 1 000 000: `1000000` reads, `1000001` does not;
/// an integer reads as N-N; digits past `u64` are text, never a panic.
#[test]
fn estimate_bounds() {
    let plan = plan_of(&[
        item("RM-0001").rel("a").est("1000000"),
        item("RM-0002").rel("a").est("\"1000000\""),
        item("RM-0003").rel("a").est("0-1000000"),
        item("RM-0004").rel("a").est("0"),
        item("RM-0005").rel("a").est("3"),
        item("RM-0006").rel("a").est("2-4"),
        item("RM-0007").rel("a").est("5-5"),
        item("RM-0011").rel("b").est("1000001"),
        item("RM-0012").rel("b").est("\"1000001\""),
        item("RM-0013").rel("b").est("999999-1000001"),
        item("RM-0014").rel("b").est("\"99999999999999999999999\""),
        item("RM-0015").rel("b").est("1-99999999999999999999999"),
    ]);
    let estimate = |id: &str| find(&plan, id).estimate.map(|e| (e.min, e.max));
    assert_eq!(estimate("RM-0001"), Some((1_000_000, 1_000_000)));
    assert_eq!(estimate("RM-0002"), Some((1_000_000, 1_000_000)));
    assert_eq!(estimate("RM-0003"), Some((0, 1_000_000)));
    assert_eq!(estimate("RM-0004"), Some((0, 0)), "0 is an estimate");
    assert_eq!(estimate("RM-0005"), Some((3, 3)));
    assert_eq!(estimate("RM-0006"), Some((2, 4)));
    assert_eq!(estimate("RM-0007"), Some((5, 5)));
    for id in ["RM-0011", "RM-0012", "RM-0013", "RM-0014", "RM-0015"] {
        assert_eq!(estimate(id), None, "{id}");
        assert!(
            plan.notes
                .iter()
                .any(|note| note.starts_with(&format!("{id}: estimate \""))),
            "{id}: {:?}",
            plan.notes
        );
    }
    assert_eq!(sum(group(&plan, "a")), (2_000_010, 3_000_012, 0));
    assert_eq!(sum(group(&plan, "b")), (0, 0, 5));
}

// ----------------------------------------------------------------- the rest

/// Items are documents whose front-matter `id:` has the prefix: not a
/// `{#ID}` section, not a `class: generated` document, not another
/// prefix's; a Tier 3 document (an archive path, a shipped status) is
/// read; a broken front-matter or an unread file is skipped, never a panic.
#[test]
fn items_are_front_matter_documents_with_the_prefix() {
    let (scheme, roadmap) = read_config(&config(ROADMAP));
    let docs = [
        item("RM-0001").rel("a").status("shipped"),
        item("RM-0002").rel("a").at("docs/archive/RM-0002.md"),
        item("RM-0008")
            .rel("a")
            .class("generated")
            .at("docs/generated/RM-0008.md"),
    ];
    let mut input = input_of(
        &scheme,
        &docs,
        &[
            HARBOR,
            (
                "plan/broken.md",
                "---\nid: RM-0003\ntitle: a: b\n---\n# Broken\n",
            ),
            (
                "plan/plain.md",
                "# No front-matter\n\nRM-0004 is mentioned.\n",
            ),
        ],
    );
    input.files.push(CheckFile {
        path: "plan/unread.md".to_owned(),
        size: 0,
        parsed: None,
        read_error: Some("permission denied".to_owned()),
        bytes: Vec::new(),
    });
    let plan = plan(&input, &roadmap);
    let all: Vec<&str> = plan
        .groups
        .iter()
        .flat_map(|group| group.items.iter().map(|item| item.id.as_str()))
        .collect();
    assert_eq!(all, ["RM-0002"], "{}", shape(&plan));
    assert_eq!((plan.open, plan.done), (1, 1));
    assert_eq!(
        group(&plan, "a").done,
        1,
        "the shipped Tier 3 item is counted"
    );
    assert_eq!(
        plan.notes,
        ["RM-0002: outside [roadmap] dir plan"],
        "an archived item outside dir"
    );
}

/// `release` mistyped: unplaced, `release` null, a note; an unknown one:
/// unplaced as written, a note; releases are case-sensitive.
#[test]
fn mistyped_or_unknown_release_is_unplaced_with_a_note() {
    let plan = plan_of(&[
        item("RM-0001").rel("[a]"),
        item("RM-0002").rel("3"),
        item("RM-0003").rel("B"),
        item("RM-0004").rel("someday").pri("must"),
        item("RM-0005").rel("a"),
    ]);
    let unplaced = group(&plan, "unplaced");
    assert_eq!(
        ids(unplaced),
        ["RM-0004", "RM-0001", "RM-0002", "RM-0003"],
        "by key: {}",
        shape(&plan)
    );
    assert!(unplaced.items.iter().all(|item| item.position.is_none()));
    assert_eq!(find(&plan, "RM-0001").release, None);
    assert_eq!(find(&plan, "RM-0003").release.as_deref(), Some("B"));
    assert_eq!(find(&plan, "RM-0004").release.as_deref(), Some("someday"));
    assert!(group(&plan, "pool").items.is_empty(), "never the pool");
    assert_eq!(
        plan.notes,
        [
            "RM-0001: release is not a string",
            "RM-0002: release is not a string",
            "RM-0003: release \"B\" is not in [roadmap] releases",
            "RM-0004: release \"someday\" is not in [roadmap] releases",
        ]
    );
}

/// `dir` is a directory: `plan/sub/` is inside, `planner/` and the root
/// are outside.
#[test]
fn an_item_outside_dir_has_a_note() {
    let plan = plan_of(&[
        item("RM-0001").rel("a").at("plan/sub/RM-0001.md"),
        item("RM-0002").rel("a").at("planner/RM-0002.md"),
        item("RM-0003").rel("a").at("RM-0003.md"),
        item("RM-0004")
            .rel("a")
            .at("docs/RM-0004.md")
            .status("shipped"),
    ]);
    assert_eq!(
        plan.notes,
        [
            "RM-0002: outside [roadmap] dir plan",
            "RM-0003: outside [roadmap] dir plan",
        ]
    );
    assert_eq!(find(&plan, "RM-0002").path, "planner/RM-0002.md");
}

/// One input in any order gives one plan.
#[test]
fn plan_ignores_input_order() {
    for name in ["roadmap-a", "roadmap-b"] {
        let corpus = fixture(name);
        let toml = std::fs::read_to_string(corpus.join("specengine.toml")).unwrap();
        let (_, roadmap) = read_config(&toml);
        let (_, input) = fixture_input(&corpus);
        let first = plan(&input, &roadmap);
        let mut reversed = input.clone();
        reversed.files.reverse();
        assert_eq!(plan(&reversed, &roadmap), first, "{name}: reversed");
        let mut rotated = input.clone();
        rotated.files.rotate_left(input.files.len() / 3);
        assert_eq!(plan(&rotated, &roadmap), first, "{name}: rotated");
    }
}

// ------------------------------------------------- the fixtures (TE-1, core)

fn sum_json(sum: &specengine_core::roadmap::Sum) -> Value {
    serde_json::json!({"min": sum.min, "max": sum.max, "unestimated": sum.unestimated})
}

/// The plan of a fixture in the shape of `expected.json` (the CLI's
/// document), the request keys left at a whole-plan read and the task
/// keys of the join (docs/features/roadmap-position.md TE-1) empty.
fn plan_json(plan: &Plan, roadmap: &RoadmapConfig) -> Value {
    let groups: Vec<Value> = plan
        .groups
        .iter()
        .map(|group| {
            let items: Vec<Value> = group
                .items
                .iter()
                .map(|item| {
                    serde_json::json!({
                        "position": item.position,
                        "id": item.id,
                        "title": item.title,
                        "path": item.path,
                        "status": item.status,
                        "release": item.release,
                        "priority": item.priority,
                        "estimate": item.estimate.map(|e| serde_json::json!({"min": e.min, "max": e.max})),
                        "waits_on": item.waits_on,
                        "cycle": item.cycle,
                        // The plan reads no queue: the join's keys
                        // (roadmap-position) are empty.
                        "tasks": [],
                    })
                })
                .collect();
            serde_json::json!({
                "group": group.kind.as_str(),
                "name": group.name,
                "current": group.current,
                "mvp_cut": group.mvp_cut,
                "open": group.open,
                "done": group.done,
                "sum": sum_json(&group.sum),
                "items": items,
            })
        })
        .collect();
    serde_json::json!({
        "release": null,
        "pool": false,
        "reason": null,
        "prefix": roadmap.prefix,
        "dir": roadmap.dir,
        "unit": roadmap.unit,
        "current": plan.current,
        "counts": {"open": plan.open, "done": plan.done},
        "cut": plan.cut.as_ref().map(|cut| serde_json::json!({
            "release": cut.release,
            "min": cut.sum.min,
            "max": cut.sum.max,
            "unestimated": cut.sum.unestimated,
        })),
        "total": sum_json(&plan.total),
        "groups": groups,
        "pool_tasks": [],
        "notes": plan.notes,
        "truncated": false,
        "omitted": 0,
    })
}

/// TE-1's oracle at the core: each fixture's plan is its `expected.json`.
#[test]
fn fixture_plans_match_expected_json() {
    for name in ["roadmap-a", "roadmap-b"] {
        let corpus = fixture(name);
        let toml = std::fs::read_to_string(corpus.join("specengine.toml")).unwrap();
        let (_, roadmap) = read_config(&toml);
        let (_, input) = fixture_input(&corpus);
        let got = plan_json(&plan(&input, &roadmap), &roadmap);
        let want: Value = serde_json::from_str(
            &std::fs::read_to_string(corpus.join("expected.json")).expect("expected.json"),
        )
        .expect("expected.json is JSON");
        if got != want {
            let groups = |value: &Value| -> Vec<String> {
                value["groups"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|group| {
                        format!(
                            "{} {}: {}",
                            group["group"],
                            group["name"],
                            group["items"]
                                .as_array()
                                .into_iter()
                                .flatten()
                                .map(|item| item["id"].as_str().unwrap_or("?").to_owned())
                                .collect::<Vec<_>>()
                                .join(" ")
                        )
                    })
                    .collect()
            };
            panic!(
                "{name}: the plan differs from expected.json\n  got groups:  {:#?}\n  want groups: {:#?}\n  got notes:  {}\n  want notes: {}\n  got:  {got}\n  want: {want}",
                groups(&got),
                groups(&want),
                got["notes"],
                want["notes"]
            );
        }
    }
}
