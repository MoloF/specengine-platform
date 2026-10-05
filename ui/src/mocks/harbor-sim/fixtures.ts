import { node, proposal, stamp, type MockProject } from "../build";
import { nodeKinds } from "./kinds";

// harbor-sim: an invented harbour-simulation game. English text; non-Latin script only as
// escapes. The normal queue holds every severity and an unknown one, an unknown proposal kind
// and status, options with a recommendation, proposals without options, two proposals on one
// node, section diffs, a 300-character unbroken token and decomposed non-Latin text.

const SLUG = "harbor-sim";

/** The north basin's local name, decomposed (NFD): e + U+0308, i + U+0306. */
export const BASIN_NAME_DECOMPOSED =
  "\u0417\u0430\u043b\u0438\u0432 \u0417\u0435\u043b\u0435\u0308\u043d\u044b\u0438\u0306";

/** One unbroken 300-character token, as a loader error prints it. */
export const LONG_TOKEN = "tide_table_sample_v3".padEnd(300, "k7Qm2Xr9Lp4Zt8Wn");

const TIDE_FILE = "docs/spec/tides/tide-cycle.md";
const DRAFT_FILE = "docs/spec/berths/draft-limits.md";
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

export function harborSim(now: number): MockProject {
  const at = (minutesAgo: number) => stamp(now, minutesAgo);
  const agent = (role: string, run: string) => ({ type: "agent", role, model: "claude-opus-5-5", run });
  return {
    project: { slug: SLUG, name: "Harbor Sim" },
    nodeKinds,
    notes: [],
    nodes: [
      node({
        id: "DOM-BERTHS",
        kind: "domain",
        title: "Berths and moorings",
        path: "docs/spec/berths/README.md",
        line: 1,
        status: "accepted",
        rev: 2,
        text: [
          "---",
          "id: DOM-BERTHS",
          "kind: domain",
          "status: accepted",
          "rev: 2",
          "---",
          "",
          "# Berths and moorings",
          "",
          "Ships wait at anchor until a berth with enough depth and length is free.",
          "A berth holds one ship; mooring takes a crew of four and 20 simulated minutes.",
          `The north basin is called "${BASIN_NAME_DECOMPOSED}" in the owner's notes.`,
        ].join("\n"),
      }),
      node({
        id: "MEC-TIDES",
        kind: "mechanic",
        title: "Tide cycle",
        path: TIDE_FILE,
        line: 1,
        status: "accepted",
        rev: 3,
        sections: ["RULE-TIDE-WINDOW"],
        text: [
          "---",
          "id: MEC-TIDES",
          "kind: mechanic",
          "status: accepted",
          "rev: 3",
          "---",
          "",
          "# Tide cycle",
          "",
          "Water level follows a 12 h 25 min cycle read from the tide table of the scenario.",
          "",
          "## RULE-TIDE-WINDOW: Entry only inside the tide window",
          "",
          "A ship with more than 11 m draft enters only from 90 minutes before",
          "to 60 minutes after high water.",
        ].join("\n"),
      }),
      node({
        id: "RULE-TIDE-WINDOW",
        kind: "rule",
        title: "Entry only inside the tide window",
        path: TIDE_FILE,
        line: 12,
        rev: 3,
        text: [
          "## RULE-TIDE-WINDOW: Entry only inside the tide window",
          "",
          "A ship with more than 11 m draft enters only from 90 minutes before",
          "to 60 minutes after high water.",
        ].join("\n"),
      }),
      node({
        id: "RULE-BERTH-DRAFT",
        kind: "rule",
        title: "Draft limit at a berth",
        path: DRAFT_FILE,
        line: 8,
        rev: 4,
        text: DRAFT_TEXT,
      }),
      node({
        id: "MEC-PILOTAGE",
        kind: "mechanic",
        title: "Pilot boarding",
        path: PILOT_FILE,
        line: 1,
        status: "review",
        rev: 1,
        sections: ["RULE-PILOT-REQ"],
        text: [
          "---",
          "id: MEC-PILOTAGE",
          "kind: mechanic",
          "status: review",
          "rev: 1",
          "---",
          "",
          "# Pilot boarding",
          "",
          PILOT_TEXT,
        ].join("\n"),
      }),
      node({
        id: "RULE-PILOT-REQ",
        kind: "rule",
        title: "Pilot required above 120 m",
        path: PILOT_FILE,
        line: 10,
        rev: 1,
        text: PILOT_TEXT,
      }),
    ],
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
        target_ids: ["RULE-BERTH-DRAFT"],
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
    ],
  };
}
