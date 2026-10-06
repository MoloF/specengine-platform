import { frontMatter, link, proposal, specFile, stamp, type MockCorpus, type MockProject } from "../build";
import { nodeKinds, specStatuses } from "./kinds";

// harbor-sim: an invented harbour-simulation game. English text; non-Latin script only as
// escapes. The normal queue holds every severity and an unknown one, an unknown proposal kind
// and status, options with a recommendation, proposals without options, two proposals on one
// node, section diffs, a 300-character unbroken token and decomposed non-Latin text; targets by
// ID, by an ID-less document's path and by a second `target_ids` entry.
//
// The spec (docs/features/ui-tree-node.md "Data", Mocks): a tree five levels deep with nested
// sections; an ID-less document; an ID with two holders; an archived document; a generated one; a
// dangling parent; a two-document parent cycle; a document over 40 000 characters, whole; links
// in four states and mentions, both ways, with reasons; hostile markup in a title, a text and a
// link's written form (escaped here so no source line spells a dialog call).
//
// The graph (docs/features/ui-graph.md "Mock"): a link of a type outside the twelve shared ones
// (MEC-PILOTAGE `precedes` MEC-MOORING), a typed link in a nested section of MEC-TIDES
// (RULE-TIDE-WINDOW `adopts` RULE-HIGH-WATER), an Impact link landing on a nested section
// (MEC-NIGHT-PASSAGE `depends_on` MEC-TIDES#RULE-TIDE-WINDOW), the archived document's
// `depends_on` MEC-TIDES.

const SLUG = "harbor-sim";

/** The north basin's local name, decomposed (NFD): e + U+0308, i + U+0306. */
export const BASIN_NAME_DECOMPOSED =
  "\u0417\u0430\u043b\u0438\u0432 \u0417\u0435\u043b\u0435\u0308\u043d\u044b\u0438\u0306";

/** One unbroken 300-character token, as a loader error prints it. */
export const LONG_TOKEN = "tide_table_sample_v3".padEnd(300, "k7Qm2Xr9Lp4Zt8Wn");

export const TIDE_FILE = "docs/spec/tides/tide-cycle.md";
/** The ID-less document: the tree names it by its path. */
export const DRAFT_FILE = "docs/spec/berths/draft-limits.md";
const PILOT_FILE = "docs/spec/pilotage/boarding.md";

const DRAFT_TEXT = [
  "## RULE-BERTH-DRAFT: Draft limit at a berth",
  "",
  "A ship may moor only where the berth depth exceeds its draft by at least 0.8 m.",
  "Berth 4 is limited to 12.0 m draft.",
  "Berths 1 to 3 keep the 10.5 m limit.",
  "",
  "Ships over the limit wait at the outer anchorage.",
].join("\n");

const DRAFT_DIFF = [
  `--- base ${DRAFT_FILE}`,
  `+++ proposed ${DRAFT_FILE}`,
  "@@ -1,7 +1,8 @@",
  " ## RULE-BERTH-DRAFT: Draft limit at a berth",
  " ",
  " A ship may moor only where the berth depth exceeds its draft by at least 0.8 m.",
  "-Berth 4 is limited to 12.0 m draft.",
  "+Berth 4 is limited to 14.5 m draft since the 2026 dredging.",
  "+The limit drops back to 12.0 m when the last dredging survey is older than two years.",
  " Berths 1 to 3 keep the 10.5 m limit.",
  " ",
  " Ships over the limit wait at the outer anchorage.",
].join("\n");

const PILOT_TEXT = [
  "## RULE-PILOT-REQ: Pilot required above 120 m",
  "",
  "Every ship longer than 120 m takes a pilot at the fairway buoy.",
  "Without a free pilot the ship waits outside the fairway.",
].join("\n");

const PILOT_DIFF = [
  `--- base ${PILOT_FILE}`,
  `+++ proposed ${PILOT_FILE}`,
  "@@ -1,4 +1,5 @@",
  " ## RULE-PILOT-REQ: Pilot required above 120 m",
  " ",
  " Every ship longer than 120 m takes a pilot at the fairway buoy.",
  "+A pilot boat carries one pilot and needs 15 simulated minutes to reach the buoy.",
  " Without a free pilot the ship waits outside the fairway.",
].join("\n");

const PILOT_CONFLICT = [
  "<<<<<<< current",
  "Every ship longer than 110 m takes a pilot at the fairway buoy.",
  "||||||| base",
  "Every ship longer than 120 m takes a pilot at the fairway buoy.",
  "=======",
  "Every ship longer than 120 m takes a pilot at the fairway buoy.",
  "A pilot boat carries one pilot and needs 15 simulated minutes to reach the buoy.",
  ">>>>>>> proposed",
].join("\n");


/** Markup an author pasted into a title; shown as text, never parsed. */
export const HOSTILE_TITLE = "<img src=x onerror=\u0061lert(1)>";

/** A link's written form that must never become an href. */
export const HOSTILE_LINK = "javascript:\u0061lert(1)";

/** Over 40 000 characters: the browser reads it whole (daemon-read: uncut). */
const TIDE_TABLE_ROWS = Array.from({ length: 520 }, (_, day) => {
  const date = new Date(Date.UTC(2026, 0, 1) + day * 86_400_000).toISOString().slice(0, 10);
  const minutes = (base: number) => {
    const at = (base + day * 50) % 1440;
    return `${String(Math.floor(at / 60)).padStart(2, "0")}:${String(at % 60).padStart(2, "0")}`;
  };
  const height = (base: number, swing: number) => (base + ((day * 37) % 41) * swing).toFixed(2);
  return `| ${date} | HW ${minutes(192)} ${height(3.9, 0.012)} m | LW ${minutes(565)} ${height(0.4, 0.008)} m | HW ${minutes(937)} ${height(3.8, 0.011)} m | LW ${minutes(1310)} ${height(0.5, 0.007)} m |`;
});

function harborCorpus(): MockCorpus {
  const documents = [
    specFile({
      path: "docs/spec/cranes.md",
      id: "MEC-CARGO-CRANES",
      kind: "mechanic",
      title: "Cargo cranes",
      status: "draft",
      mark: "dangling-parent",
      lines: [
        ...frontMatter({ id: "MEC-CARGO-CRANES", kind: "mechanic", status: "draft", parent: "DOM-CARGO", rev: 1 }),
        "",
        "# Cargo cranes",
        "",
        "Two gantry cranes serve the container quay; each lifts one box per 90 simulated seconds.",
      ],
    }),
    specFile({
      path: "docs/spec/generated/index.md",
      id: null,
      kind: null,
      title: "Spec index",
      generated: true,
      lines: [...frontMatter({ class: "generated" }), "", "# Spec index", "", "Written by the index export; never edited."],
    }),
    specFile({
      path: "docs/spec/harbor.md",
      id: "DOM-HARBOR",
      kind: "domain",
      title: "Harbor simulation",
      status: "accepted",
      rev: 4,
      summary: "The harbour as one simulation: ships, water, berths and pilots on one clock.",
      lines: [
        ...frontMatter({
          id: "DOM-HARBOR",
          kind: "domain",
          status: "accepted",
          rev: 4,
          summary: "The harbour as one simulation: ships, water, berths and pilots on one clock.",
        }),
        "",
        "# Harbor simulation",
        "",
        "The simulation models one harbour: ships arrive, wait for water and a berth, moor, unload and leave.",
        "Every system reads the same clock; nothing advances on its own.",
        "",
        // A section named by a heading attribute (docs/features/ui-markdown.md "Data").
        "## One simulated clock {#RULE-HARBOR-CLOCK}",
        "",
        "One tick is one simulated minute. Systems run in a fixed order each tick:",
        "tides, arrivals, pilotage, berthing, cargo.",
      ],
      sections: [{ id: "RULE-HARBOR-CLOCK", kind: "rule", title: "One simulated clock" }],
    }),
    specFile({
      path: "docs/spec/archive/old-quays.md",
      id: "MEC-OLD-QUAYS",
      kind: "mechanic",
      title: "Old quay layout",
      status: "accepted",
      parent: "DOM-BERTHS",
      archived: true,
      lines: [
        ...frontMatter({ id: "MEC-OLD-QUAYS", kind: "mechanic", status: "accepted", parent: "DOM-BERTHS", depends_on: "MEC-TIDES", rev: 1 }),
        "",
        "# Old quay layout",
        "",
        "Before the 2026 dredging the quays followed MEC-TIDES alone; kept for the record.",
        "",
        "## RULE-OLD-QUAY-DEPTH: Quay depth before dredging",
        "",
        "Every quay had 10.5 m of water at low tide.",
      ],
      sections: [{ id: "RULE-OLD-QUAY-DEPTH", kind: "rule", title: "Quay depth before dredging" }],
    }),
    specFile({
      path: "docs/spec/berths/README.md",
      id: "DOM-BERTHS",
      kind: "domain",
      title: "Berths and moorings",
      status: "accepted",
      rev: 2,
      parent: "DOM-HARBOR",
      summary: "Where ships moor and what limits them.",
      lines: [
        ...frontMatter({
          id: "DOM-BERTHS",
          kind: "domain",
          status: "accepted",
          parent: "DOM-HARBOR",
          rev: 2,
          summary: "Where ships moor and what limits them.",
        }),
        "",
        "# Berths and moorings",
        "",
        "Ships wait at anchor until a berth with enough depth and length is free.",
        "A berth holds one ship; mooring takes a crew of four and 20 simulated minutes.",
        "Deep ships also wait for the tide window (MEC-TIDES#RULE-TIDE-WINDOW).",
        // Markdown links the rendered text anchors as the links read says (docs/features/ui-markdown.md "Data").
        "See [the tide cycle](../tides/tide-cycle.md) and the [quay plans](quays.md).",
        "",
        `## RULE-BASIN-NAME: ${BASIN_NAME_DECOMPOSED}`,
        "",
        `The north basin is called "${BASIN_NAME_DECOMPOSED}" in the owner's notes.`,
        "Berth plans show the name as written.",
      ],
      sections: [{ id: "RULE-BASIN-NAME", kind: "rule", title: BASIN_NAME_DECOMPOSED }],
    }),
    specFile({
      path: DRAFT_FILE,
      id: null,
      kind: null,
      title: "Berth limits",
      rev: 4,
      parent: "DOM-BERTHS",
      lines: [
        ...frontMatter({ parent: "DOM-BERTHS", rev: 4 }),
        "",
        "# Berth limits",
        "",
        "Limits every berth checks when a ship asks to moor.",
        "",
        ...DRAFT_TEXT.split("\n"),
      ],
      sections: [{ id: "RULE-BERTH-DRAFT", kind: "rule", title: "Draft limit at a berth" }],
    }),
    specFile({
      path: "docs/spec/berths/mooring.md",
      id: "MEC-MOORING",
      kind: "mechanic",
      title: "Mooring",
      status: "proposed",
      rev: 2,
      parent: "DOM-BERTHS",
      lines: [
        ...frontMatter({ id: "MEC-MOORING", kind: "mechanic", status: "proposed", parent: "DOM-BERTHS", rev: 2 }),
        "",
        "# Mooring",
        "",
        "A free berth takes the first ship in the anchorage queue whose draft and length fit.",
        "",
        "## RULE-MOOR-CREW: Mooring crew",
        "",
        "Mooring takes a crew of four and 20 simulated minutes; a crew serves one berth at a time.",
        "",
        `### RULE-MOOR-NIGHT: ${HOSTILE_TITLE}`,
        "",
        "At night mooring takes 30 minutes. A note pasted from a web page kept its markup:",
        "<script>window.location = 'https://example.org/'</script><b>bold?</b> **not bold either**",
        `The old planning tool lives at ${HOSTILE_LINK}.`,
      ],
      sections: [
        { id: "RULE-MOOR-CREW", kind: "rule", title: "Mooring crew" },
        { id: "RULE-MOOR-NIGHT", kind: "rule", title: HOSTILE_TITLE },
      ],
    }),
    specFile({
      path: "docs/spec/fairway/README.md",
      id: "MEC-FAIRWAY",
      kind: "mechanic",
      title: "Fairway",
      status: "accepted",
      parent: "DOM-HARBOR",
      lines: [
        ...frontMatter({ id: "MEC-FAIRWAY", kind: "mechanic", status: "accepted", parent: "DOM-HARBOR", rev: 1 }),
        "",
        "# Fairway",
        "",
        "The fairway runs from the outer buoy to the inner basin; one ship passes at a time.",
        "",
        "## RULE-FAIRWAY-SPEED: Speed in the fairway",
        "",
        "Ships keep to 8 knots in the fairway.",
      ],
      sections: [{ id: "RULE-FAIRWAY-SPEED", kind: "rule", title: "Speed in the fairway" }],
    }),
    specFile({
      path: "docs/spec/fairway/night.md",
      id: "MEC-NIGHT-PASSAGE",
      kind: "mechanic",
      title: "Night passage",
      status: "draft",
      parent: "MEC-FAIRWAY",
      lines: [
        ...frontMatter({
          id: "MEC-NIGHT-PASSAGE",
          kind: "mechanic",
          status: "draft",
          parent: "MEC-FAIRWAY",
          depends_on: "MEC-TIDES#RULE-TIDE-WINDOW",
          rev: 1,
        }),
        "",
        "# Night passage",
        "",
        "At night the fairway is lit buoy to buoy.",
        "",
        "## RULE-FAIRWAY-SPEED: Speed in the fairway at night",
        "",
        "Ships keep to 6 knots in the fairway at night. (The same ID as in the fairway's README: two holders.)",
      ],
      sections: [{ id: "RULE-FAIRWAY-SPEED", kind: "rule", title: "Speed in the fairway at night" }],
    }),
    specFile({
      path: "docs/spec/locks/lock-a.md",
      id: "MEC-LOCK-A",
      kind: "mechanic",
      title: "Outer lock gate",
      status: "draft",
      mark: "parent-cycle",
      lines: [
        ...frontMatter({ id: "MEC-LOCK-A", kind: "mechanic", status: "draft", parent: "MEC-LOCK-B", rev: 1 }),
        "",
        "# Outer lock gate",
        "",
        "The outer gate opens when the lock level matches the sea.",
      ],
    }),
    specFile({
      path: "docs/spec/locks/lock-b.md",
      id: "MEC-LOCK-B",
      kind: "mechanic",
      title: "Inner lock gate",
      status: "draft",
      parent: "MEC-LOCK-A",
      lines: [
        ...frontMatter({ id: "MEC-LOCK-B", kind: "mechanic", status: "draft", parent: "MEC-LOCK-A", rev: 1 }),
        "",
        "# Inner lock gate",
        "",
        "The inner gate opens when the lock level matches the basin.",
      ],
    }),
    specFile({
      path: PILOT_FILE,
      id: "MEC-PILOTAGE",
      kind: "mechanic",
      title: "Pilot boarding",
      status: "proposed",
      parent: "DOM-HARBOR",
      lines: [
        ...frontMatter({
          id: "MEC-PILOTAGE",
          kind: "mechanic",
          status: "proposed",
          parent: "DOM-HARBOR",
          depends_on: "[[MEC-TIDES]]",
          precedes: "[MEC-MOORING]",
          rev: 1,
        }),
        "",
        "# Pilot boarding",
        "",
        ...PILOT_TEXT.split("\n"),
      ],
      sections: [{ id: "RULE-PILOT-REQ", kind: "rule", title: "Pilot required above 120 m" }],
    }),
    specFile({
      path: "docs/spec/tides/spring-window.md",
      id: "RULE-SPRING-WINDOW",
      kind: "rule",
      title: "Spring tides: a wider window?",
      status: "draft",
      parent: "MEC-TIDES",
      lines: [
        ...frontMatter({
          id: "RULE-SPRING-WINDOW",
          kind: "rule",
          status: "draft",
          parent: "MEC-TIDES",
          working_answer: "RULE-TIDE-WINDOW",
        }),
        "",
        "# Spring tides: a wider window?",
        "",
        "At spring tides high water is higher; MEC-TIDES may allow a wider entry window then.",
      ],
    }),
    specFile({
      path: TIDE_FILE,
      id: "MEC-TIDES",
      kind: "mechanic",
      title: "Tide cycle",
      status: "accepted",
      rev: 3,
      parent: "DOM-WATER",
      lines: [
        ...frontMatter({
          id: "MEC-TIDES",
          kind: "mechanic",
          status: "accepted",
          parent: "DOM-WATER",
          depends_on: "[DOM-BERTHS, harbor-ops:MEC-SHIFTS]",
          constrains: "[RULE-BERTH-DRAFT]",
          uses_term: "[RULE-HIGH-WATER, TERM-SLACK-WATER]",
          canon: "docs/canon/tides.md",
          rev: 3,
        }),
        "",
        "# Tide cycle",
        "",
        "Water level follows a 12 h 25 min cycle read from the tide table of the scenario (MEC-TIDE-TABLES).",
        "",
        "## RULE-TIDE-WINDOW: Entry only inside the tide window",
        "",
        "A ship with more than 11 m draft enters only from 90 minutes before",
        "to 60 minutes after high water.",
        "The window also bounds pilot boarding (MEC-PILOTAGE).",
        "It adopts the high-water mark of RULE-HIGH-WATER.",
      ],
      sections: [{ id: "RULE-TIDE-WINDOW", kind: "rule", title: "Entry only inside the tide window" }],
    }),
    specFile({
      path: "docs/spec/tides/tide-tables.md",
      id: "MEC-TIDE-TABLES",
      kind: "mechanic",
      title: "Tide tables",
      status: "accepted",
      parent: "MEC-TIDES",
      lines: [
        ...frontMatter({ id: "MEC-TIDE-TABLES", kind: "mechanic", status: "accepted", parent: "MEC-TIDES", depends_on: "MEC-TIDES" }),
        "",
        "# Tide tables",
        "",
        "Each scenario carries a year and a half of tide events, high water (HW) and low water (LW).",
        `The loader also accepts samples such as ${LONG_TOKEN} without a schema.`,
        "",
        "| Date | First | Second | Third | Fourth |",
        "|---|---|---|---|---|",
        ...TIDE_TABLE_ROWS,
      ],
    }),
    specFile({
      path: "docs/spec/water/README.md",
      id: "DOM-WATER",
      kind: "domain",
      title: "Water and weather",
      status: "accepted",
      parent: "DOM-HARBOR",
      summary: "Tides, wind and visibility: what the water lets ships do.",
      lines: [
        ...frontMatter({
          id: "DOM-WATER",
          kind: "domain",
          status: "accepted",
          parent: "DOM-HARBOR",
          depends_on: "docs/spec/tides/tide-cycle.md#slack",
          summary: "Tides, wind and visibility: what the water lets ships do.",
        }),
        "",
        "# Water and weather",
        "",
        "Water and weather decide when ships may move.",
      ],
    }),
    specFile({
      path: "docs/spec/water/high-water.md",
      id: "RULE-HIGH-WATER",
      kind: "rule",
      title: "High water",
      status: "accepted",
      parent: "DOM-WATER",
      lines: [
        ...frontMatter({ id: "RULE-HIGH-WATER", kind: "rule", status: "accepted", parent: "DOM-WATER" }),
        "",
        "# High water",
        "",
        "High water is the highest level of one tide cycle, read from the tide table.",
      ],
    }),
  ];
  const linkAt = (path: string, needle: string): number => {
    const file = documents.find((candidate) => candidate.path === path);
    const at = file?.lines.findIndex((line) => line.includes(needle)) ?? -1;
    if (at < 0) {
      throw new Error(`${path}: no line holds ${needle}`);
    }
    return at + 1;
  };
  const frontmatter = "frontmatter";
  const inline = "inline";
  return {
    treeNotes: [],
    documents,
    links: [
      link({ type: "depends_on", origin: frontmatter, written: "DOM-BERTHS", path: TIDE_FILE, line: linkAt(TIDE_FILE, "depends_on:"), to: "DOM-BERTHS" }),
      link({
        type: "depends_on",
        origin: frontmatter,
        written: "harbor-ops:MEC-SHIFTS",
        path: TIDE_FILE,
        line: linkAt(TIDE_FILE, "depends_on:"),
        state: "skipped",
      }),
      link({ type: "constrains", origin: frontmatter, written: "RULE-BERTH-DRAFT", path: TIDE_FILE, line: linkAt(TIDE_FILE, "constrains:"), to: "RULE-BERTH-DRAFT" }),
      link({ type: "uses_term", origin: frontmatter, written: "RULE-HIGH-WATER", path: TIDE_FILE, line: linkAt(TIDE_FILE, "uses_term:"), to: "RULE-HIGH-WATER" }),
      link({
        type: "uses_term",
        origin: frontmatter,
        written: "TERM-SLACK-WATER",
        path: TIDE_FILE,
        line: linkAt(TIDE_FILE, "uses_term:"),
        reason: "`TERM-SLACK-WATER` resolves to no ID and no alias",
      }),
      link({
        type: "canon",
        origin: frontmatter,
        written: "docs/canon/tides.md",
        path: TIDE_FILE,
        line: linkAt(TIDE_FILE, "canon:"),
        state: "unchecked",
        reason: "`canon:` names docs/canon/tides.md, no walked document; not checked",
      }),
      link({ type: "mentions", origin: inline, written: "MEC-TIDE-TABLES", path: TIDE_FILE, line: linkAt(TIDE_FILE, "(MEC-TIDE-TABLES)"), to: "MEC-TIDE-TABLES" }),
      link({ type: "mentions", origin: inline, written: "MEC-PILOTAGE", path: TIDE_FILE, line: linkAt(TIDE_FILE, "(MEC-PILOTAGE)"), to: "MEC-PILOTAGE" }),
      link({ type: "depends_on", origin: frontmatter, written: "[[MEC-TIDES]]", path: PILOT_FILE, line: linkAt(PILOT_FILE, "depends_on:"), to: "MEC-TIDES" }),
      link({ type: "precedes", origin: frontmatter, written: "MEC-MOORING", path: PILOT_FILE, line: linkAt(PILOT_FILE, "precedes:"), to: "MEC-MOORING" }),
      link({ type: "adopts", origin: inline, written: "RULE-HIGH-WATER", path: TIDE_FILE, line: linkAt(TIDE_FILE, "high-water mark"), to: "RULE-HIGH-WATER" }),
      link({
        type: "depends_on",
        origin: frontmatter,
        written: "MEC-TIDES#RULE-TIDE-WINDOW",
        path: "docs/spec/fairway/night.md",
        line: linkAt("docs/spec/fairway/night.md", "depends_on:"),
        to: "RULE-TIDE-WINDOW",
      }),
      link({
        type: "depends_on",
        origin: frontmatter,
        written: "MEC-TIDES",
        path: "docs/spec/archive/old-quays.md",
        line: linkAt("docs/spec/archive/old-quays.md", "depends_on:"),
        to: "MEC-TIDES",
      }),
      link({
        type: "mentions",
        origin: inline,
        written: "MEC-TIDES#RULE-TIDE-WINDOW",
        path: "docs/spec/berths/README.md",
        line: linkAt("docs/spec/berths/README.md", "MEC-TIDES#RULE-TIDE-WINDOW"),
        to: "RULE-TIDE-WINDOW",
      }),
      link({
        type: "mentions",
        origin: inline,
        written: "../tides/tide-cycle.md",
        path: "docs/spec/berths/README.md",
        line: linkAt("docs/spec/berths/README.md", "[the tide cycle]"),
        to: "MEC-TIDES",
      }),
      link({
        type: "mentions",
        origin: inline,
        written: "quays.md",
        path: "docs/spec/berths/README.md",
        line: linkAt("docs/spec/berths/README.md", "[quay plans]"),
        reason: "`quays.md` names no file under the spec roots",
      }),
      link({
        type: "depends_on",
        origin: frontmatter,
        written: "MEC-TIDES",
        path: "docs/spec/tides/tide-tables.md",
        line: linkAt("docs/spec/tides/tide-tables.md", "depends_on:"),
        to: "MEC-TIDES",
      }),
      link({
        type: "working_answer",
        origin: frontmatter,
        written: "RULE-TIDE-WINDOW",
        path: "docs/spec/tides/spring-window.md",
        line: linkAt("docs/spec/tides/spring-window.md", "working_answer:"),
        to: "RULE-TIDE-WINDOW",
      }),
      link({
        type: "mentions",
        origin: inline,
        written: "MEC-TIDES",
        path: "docs/spec/tides/spring-window.md",
        line: linkAt("docs/spec/tides/spring-window.md", "MEC-TIDES may"),
        to: "MEC-TIDES",
      }),
      link({
        type: "mentions",
        origin: inline,
        written: "MEC-TIDES",
        path: "docs/spec/archive/old-quays.md",
        line: linkAt("docs/spec/archive/old-quays.md", "MEC-TIDES alone"),
        to: "MEC-TIDES",
      }),
      link({
        type: "depends_on",
        origin: frontmatter,
        written: "docs/spec/tides/tide-cycle.md#slack",
        path: "docs/spec/water/README.md",
        line: linkAt("docs/spec/water/README.md", "depends_on:"),
        to: "MEC-TIDES",
        reason: "`#slack` names no anchor in docs/spec/tides/tide-cycle.md; lands on the document",
      }),
      link({
        type: "depends_on",
        origin: inline,
        written: HOSTILE_LINK,
        path: "docs/spec/berths/mooring.md",
        line: linkAt("docs/spec/berths/mooring.md", "old planning tool"),
        reason: `\`${HOSTILE_LINK}\` resolves to no ID and no alias`,
      }),
    ],
  };
}

export function harborSim(now: number): MockProject {
  const at = (minutesAgo: number) => stamp(now, minutesAgo);
  const agent = (role: string, run: string) => ({ type: "agent", role, model: "claude-opus-5-5", run });
  return {
    project: { slug: SLUG, name: "Harbor Sim", root: "/work/harbor-sim", branch: "main" },
    nodeKinds,
    specStatuses,
    notes: [],
    corpus: harborCorpus(),
    proposals: [
      proposal({
        id: "PR-0041",
        project: SLUG,
        kind: "discrepancy",
        severity: "high",
        gap_type: "contradicts",
        task_id: "T-0107",
        target_id: "RULE-TIDE-WINDOW",
        target_path: TIDE_FILE,
        target_ids: ["RULE-TIDE-WINDOW"],
        branch: "task/T-0107",
        worktree: "/work/harbor-sim/T-0107",
        base_commit: "5d0c1e2a9b7f4c3d8e6a1b0f2c4d6e8a0b2c4d6e",
        summary: "Tide window is not enforced and contradicts RULE-TIDE-WINDOW",
        rationale: "Found while implementing T-0107 (queue at the outer anchorage).",
        evidence: [
          {
            file: "src/sim/tide.rs",
            qpath: "sim::tide::entry_window_system",
            lines: "41-58",
            observed: "Ships enter the harbour at any water level; the system never reads the tide table.",
            documented: "A ship with more than 11 m draft enters only from 90 minutes before to 60 minutes after high water.",
          },
        ],
        options: [
          {
            label: "Code to spec",
            effect: "Add the tide-window check to entry_window_system.",
            price: "One more system and a probe test bound to RULE-TIDE-WINDOW.",
          },
          {
            label: "Spec to code",
            effect: "Drop the window from RULE-TIDE-WINDOW; deep ships wait by draft only.",
            price: "The harbour gets busier at low water; balance note B-12 changes.",
          },
        ],
        recommendation: 0,
        working_answer: "Until you decide, entry stays unrestricted; the code carries // @assumes PR-0041.",
        author: agent("rust-developer", "R-2211"),
        preview: "applies",
        created_at: at(12),
      }),
      proposal({
        id: "PR-0042",
        project: SLUG,
        kind: "update",
        severity: "normal",
        task_id: "T-0107",
        target_id: "RULE-BERTH-DRAFT",
        target_path: DRAFT_FILE,
        target_ids: ["RULE-BERTH-DRAFT"],
        branch: "task/T-0107",
        worktree: "/work/harbor-sim/T-0107",
        base_commit: "5d0c1e2a9b7f4c3d8e6a1b0f2c4d6e8a0b2c4d6e",
        base_hash: "b3:0f4e1d2c3b4a59687766554433221100ffeeddccbbaa99887766554433221100",
        base_text: DRAFT_TEXT,
        summary: "Raise the draft limit at berth 4 to 14.5 m after dredging",
        rationale: "The 2026 dredging deepened berth 4; the scenario data already uses 14.5 m.",
        diff: DRAFT_DIFF,
        preview: "rebases",
        notes: ["rebases: RULE-BERTH-DRAFT changed after this proposal was raised (PR-0039 applied)"],
        author: agent("spec-writer", "R-2190"),
        created_at: at(185),
      }),
      proposal({
        id: "PR-0043",
        project: SLUG,
        kind: "question",
        severity: "low",
        target_id: "DOM-BERTHS",
        target_ids: ["DOM-BERTHS"],
        summary: `Keep the local name "${BASIN_NAME_DECOMPOSED}" for the north basin in berth plans?`,
        rationale: "Berth plans are player-facing; the owner's notes use the local name.",
        options: [
          {
            label: "Keep the local name",
            effect: "Berth plans show the name as written in the owner's notes.",
            price: "Players whose font lacks the glyphs see fallback boxes.",
          },
          {
            label: "Use North basin",
            effect: "Berth plans show an English name; the local one stays in the lore notes.",
            price: "The setting loses some of its flavour.",
          },
        ],
        recommendation: 0,
        working_answer: "Plans use the local name and fall back to North basin when a font lacks the glyphs.",
        author: agent("requirement-analyst", "R-2154"),
        created_at: at(26 * 60),
      }),
      proposal({
        id: "PR-0044",
        project: SLUG,
        kind: "discrepancy",
        severity: "urgent",
        gap_type: "partial",
        task_id: "T-0112",
        target_id: "RULE-BERTH-DRAFT",
        target_path: DRAFT_FILE,
        target_ids: ["RULE-BERTH-DRAFT", "DOM-BERTHS"],
        branch: "task/T-0112",
        summary: "berths.ron gives berth 4 a 13.0 m draft limit; the spec says 12.0 m",
        evidence: [
          {
            file: "data/berths.ron",
            qpath: "root.berths[3].max_draft",
            lines: "18",
            observed: "max_draft: 13.0",
            documented: "Berth 4 is limited to 12.0 m draft.",
          },
        ],
        options: [
          {
            label: "Data to spec",
            effect: "Set max_draft of berth 4 to 12.0 in berths.ron.",
            price: "Two scenario saves fail their replay check and need a re-record.",
          },
          {
            label: "Spec to data",
            effect: "Write 13.0 m into RULE-BERTH-DRAFT (PR-0042 proposes 14.5 m).",
            price: "Conflicts with PR-0042; whichever is applied second rebases.",
          },
        ],
        recommendation: 1,
        working_answer: "The simulation keeps 13.0 m until you decide.",
        author: agent("test-engineer", "R-2230"),
        created_at: at(40),
      }),
      proposal({
        id: "PR-0045",
        project: SLUG,
        kind: "reconcile",
        status: "escalated",
        gap_type: "unrequested",
        task_id: "T-0109",
        target_id: "MEC-TIDES",
        target_ids: ["MEC-TIDES"],
        summary: "The tide-table loader accepts an undocumented sample format",
        evidence: [
          {
            file: "src/sim/tide_table.rs",
            qpath: "sim::tide_table::parse_sample",
            lines: "102-131",
            observed: `The loader accepts samples such as ${LONG_TOKEN} without a schema.`,
            documented: "The tide table is read from the scenario's RON file only.",
          },
        ],
        working_answer: "The loader keeps accepting both formats.",
        author: agent("code-reviewer", "R-2201"),
        created_at: at(2 * 24 * 60),
      }),
      proposal({
        id: "PR-0046",
        project: SLUG,
        kind: "discrepancy",
        status: "deferred",
        severity: "low",
        gap_type: "missing",
        task_id: "T-0107",
        target_id: "RULE-PILOT-REQ",
        target_path: PILOT_FILE,
        target_ids: ["RULE-PILOT-REQ"],
        summary: "Pilot boat travel time is in the code but not in the spec",
        evidence: [
          {
            file: "src/sim/pilotage.rs",
            qpath: "sim::pilotage::dispatch_pilot",
            lines: "64-77",
            observed: "A pilot boat needs 15 simulated minutes to reach the fairway buoy.",
            documented: "RULE-PILOT-REQ says nothing about pilot travel time.",
          },
        ],
        options: [
          {
            label: "Spec to code",
            effect: "Add the travel time to RULE-PILOT-REQ (the attached diff).",
            price: "None beyond the edit.",
          },
          {
            label: "Code to spec",
            effect: "Make boarding instant.",
            price: "Queues at the fairway disappear; T-0107's probe changes.",
          },
        ],
        recommendation: 0,
        working_answer: "The code keeps the 15-minute travel time.",
        diff: PILOT_DIFF,
        preview: "conflicts",
        conflict: PILOT_CONFLICT,
        decision_note: "Wait for the pilotage rework.",
        author: agent("rust-developer", "R-2105"),
        created_at: at(5 * 24 * 60),
      }),
      proposal({
        id: "PR-0047",
        project: SLUG,
        kind: "question",
        severity: "low",
        target_id: DRAFT_FILE,
        target_path: DRAFT_FILE,
        target_ids: [DRAFT_FILE],
        summary: "Give the berth limits page an ID so tasks can cite it?",
        options: [
          { label: "Add an ID", effect: "The page gets an ID of its own.", price: "Every citation of its path is rewritten once." },
          { label: "Keep the path", effect: "Tasks cite the page by its path.", price: "A move of the file breaks those citations." },
        ],
        recommendation: 0,
        working_answer: "Tasks cite the page by its path for now.",
        author: agent("spec-writer", "R-2240"),
        created_at: at(90),
      }),
    ],
  };
}
