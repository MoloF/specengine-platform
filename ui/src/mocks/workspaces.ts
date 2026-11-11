import type { WorkspaceDocument } from "../api/types";
import type { Scenario } from "./scenario";

// The mock projects' workspaces and modules (docs/features/workspaces.md "The UI", Mocks): `spec
// workspace --json` as the daemon serves it, read from each project's invented config.
// harbor-sim configures two trades, `galley` and `foundry`, two rooted modules, `berths` and
// `waters`, and one direction, `mechanics`, by the kind `mechanic` (docs/features/departments.md
// "Acceptance criteria", DP-11); their names are no repository's and no preset's (the spec's
// "Disjoint names"), and no app code spells one (policy.test.ts). Each department serves the
// disciplines a hand-off to it may carry, as the daemon's rule gives them over cards.ts' flows. ledger-api configures none: the implicit workspace, every section but
// Tasks. `config-notes`: harbor-sim's tables with two wrong values, each left out with its note, and
// ledger-api's one malformed table, which serves the implicit workspace with one note. `error`
// refuses the read as every read (MockClient).

/** Where the notes say the config is, as given: the project's own file. */
const CONFIG = "specengine.toml";

/** The implicit workspace's sections: `sections.ts`'s nine but `tasks`, in order (D3). */
const IMPLICIT_SECTIONS = ["inbox", "board", "roadmap", "flow", "tree", "graph", "health", "questions"];

/** What a project with no `[[workspace]]` table is served, notes aside. */
function implicit(notes: string[]): WorkspaceDocument {
  return {
    source: "implicit",
    notes,
    workspaces: [{ name: "all", title: "All sections", landing: "overview", sections: [...IMPLICIT_SECTIONS], flows: null, disciplines: [], module: null }],
    modules: [],
  };
}

/** harbor-sim's two trades, two rooted modules and a direction, the notes as the scenario leaves them. */
function harbor(notes: string[]): WorkspaceDocument {
  return {
    source: "config",
    notes,
    workspaces: [
      { name: "galley", title: "Galley", landing: "flow", sections: ["board", "flow", "tree", "inbox"], flows: ["copy"], disciplines: ["copy", "captions"], module: "berths" },
      { name: "foundry", title: "Foundry", landing: "overview", sections: ["board", "health", "graph", "tree"], flows: ["plates"], disciplines: ["plates"], module: null },
    ],
    modules: [
      { name: "berths", title: "Berths", root: "DOM-BERTHS", kinds: null },
      { name: "waters", title: "Harbour waters", root: "DOM-WATER", kinds: null },
      { name: "mechanics", title: "Mechanics", root: null, kinds: ["mechanic"] },
    ],
  };
}

/** `config-notes`: harbor-sim's two wrong values, each left out, in line order, as the reader words them. */
export const HARBOR_NOTES: readonly string[] = [
  `${CONFIG}:41: workspace \`galley\`: \`stage\` is no section (inbox, tasks, board, roadmap, flow, tree, graph, health, questions): left out`,
  `${CONFIG}:49: workspace \`foundry\`: no \`[[card.flow]]\` is named \`inks\`: left out`,
];

/** `config-notes`: ledger-api's table whose name is off the grammar, so the implicit workspace is served. */
export const LEDGER_NOTE = `${CONFIG}:24: \`Back office\` is not a name: lower-case ASCII letters, digits, \`_\` and \`-\`; the implicit workspace is served`;

/** A project's workspaces document in a scenario; a project the mock does not hold: the implicit one. */
export function workspacesOf(slug: string, scenario: Scenario): WorkspaceDocument {
  const notes = scenario === "config-notes";
  if (slug === "harbor-sim") {
    // `aside` holds the pottery workshop in harbor-sim's place: no `[[module]]` roots in it (flow-scenario, RM-0094).
    return scenario === "aside" || scenario === "aside40" ? { ...harbor([]), modules: [] } : harbor(notes ? [...HARBOR_NOTES] : []);
  }
  return implicit(notes && slug === "ledger-api" ? [LEDGER_NOTE] : []);
}
