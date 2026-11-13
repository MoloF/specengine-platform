import type { ShownLink, ShownLinks } from "../api/types";
import { aLink, aUsage } from "./builders";

// Who uses a document, on the stub client (docs/features/usage-orphans.md "Acceptance criteria",
// UO10): `fixtures/spec-a`'s MEC-STAMINA as `spec show MEC-STAMINA --links --json` serves its In
// rows today, their order and fields verbatim, with the counts of Data's table, `{2, 6}`: DEC-0023
// and MEC-SPRINT use it, the two through RULE-STAM-REGEN; Q-031 mentions it twice, counted once.
// Kinds, link types and paths are the fixture's data, never app code's.

export const STAMINA = "MEC-STAMINA";
export const STAMINA_PATH = "docs/spec/movement/stamina.md";

const SPRINT = "docs/spec/movement/sprint.md";

const row = (fields: Partial<ShownLink> & Pick<ShownLink, "type" | "written" | "name" | "path" | "line">): ShownLink => aLink({ origin: "frontmatter", ...fields });

/** MEC-STAMINA's In rows, served order: strong by type, then `mentions`, each by path and line. */
export const STAMINA_IN: readonly ShownLink[] = [
  row({ type: "canon", at: "RULE-STAM-REGEN", name: "DEC-0023", title: "Regeneration waits for rest", end_kind: "decision", written: "docs/spec/movement/stamina.md#regeneration", path: "docs/records/DEC/DEC-0023.md", line: 6 }),
  row({ type: "constrains", at: "RULE-STAM-REGEN", name: "MEC-SPRINT", title: "Sprint", end_kind: "mechanic", written: "RULE-STAM-REGEN", path: SPRINT, line: 10 }),
  row({ type: "depends_on", name: "MEC-SPRINT", title: "Sprint", end_kind: "mechanic", written: "MEC-STAMINA", path: SPRINT, line: 9 }),
  row({ type: "mentions", origin: "inline", at: "RULE-STAM-REGEN", name: "docs/features/stamina-tuning.md", title: "Stamina tuning", written: "MEC-STAMINA#RULE-STAM-REGEN@3", path: "docs/features/stamina-tuning.md", line: 18 }),
  row({ type: "mentions", name: "Q-031", end_kind: "question", written: "MEC-STAMINA", path: "docs/records/Q/Q-031.md", line: 9 }),
  row({ type: "mentions", origin: "inline", at: "RULE-STAM-REGEN", name: "Q-031", end_kind: "question", written: "RULE-STAM-REGEN", path: "docs/records/Q/Q-031.md", line: 16 }),
  row({ type: "mentions", origin: "inline", name: "DOM-MOVEMENT", title: "Movement", end_kind: "domain", written: "stamina.md", path: "docs/spec/movement/README.md", line: 13 }),
  row({ type: "mentions", origin: "inline", name: "RULE-MOVE-SPEEDS", title: "Speeds", end_kind: "rule", written: "MEC-STAMINA", path: "docs/spec/movement/README.md", line: 18 }),
  row({ type: "mentions", origin: "inline", at: "RULE-STAM-REGEN", name: "MEC-SPRINT", title: "Sprint", end_kind: "mechanic", written: "[[MEC-STAMINA#RULE-STAM-REGEN|regeneration]]", path: SPRINT, line: 16 }),
  row({ type: "mentions", origin: "inline", at: "EDGE-STAM-ZERO", name: "EDGE-SPRINT-EMPTY", title: "Empty tank", end_kind: "edge-case", written: "EDGE-STAM-ZERO", path: SPRINT, line: 24 }),
];

/** MEC-STAMINA's links: its five out rows left aside (Out is unchanged), its ten In rows, Data's counts. */
export function staminaLinks(): ShownLinks {
  return { outgoing: [], incoming: STAMINA_IN.map((link) => ({ ...link })), usage: aUsage({ used_by: 2, mentioned_by: 6 }), left_out: { generated: 0, tier3: 0 }, omitted: 0 };
}

/**
 * A rule its own document's other section uses, besides `others` documents of kind `mechanic` that
 * use it from their own files, one row each (`MEC-USER-01` on): the served count is those documents.
 */
export function usedRule(others: number, path = "docs/spec/rules.md"): ShownLinks {
  const outside = Array.from({ length: others }, (_, at) => {
    const name = `MEC-USER-${String(at + 1).padStart(2, "0")}`;
    return aLink({ type: "uses", written: "RULE-A", name, title: `User ${String(at + 1)}`, end_kind: "mechanic", path: `docs/spec/users/${name.toLowerCase()}.md`, line: 7 });
  });
  const own = aLink({ type: "uses", written: "RULE-A", name: "RULE-B", title: "The neighbour rule", end_kind: "rule", path, line: 30 });
  return { outgoing: [], incoming: [...outside, own], usage: aUsage({ used_by: others }), left_out: { generated: 0, tier3: 0 }, omitted: 0 };
}
