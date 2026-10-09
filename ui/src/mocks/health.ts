import type { HealthReport, ShownLink, UnappliedAmendment } from "../api/types";
import { DEEP_BERTH_FILE, QUAY_DECISION_FILE } from "./harbor-sim/fixtures";
import type { Scenario } from "./scenario";

// The mock's `spec health --json` (docs/features/health-left.md "Data", Mocks): one builder, no
// clock, the core's order (amendments by path, byte order). normal: harbor-sim two, the spec's
// decision with one dangling link and a draft spec amending RULE-BERTH-DRAFT, resolved, both
// documents of its corpus (src/mocks/harbor-sim/fixtures.ts), their links as its `show --links`
// lists them (src/mocks/health.test.ts holds the two to that); ledger-api none. empty: none. large: harbor-sim 3 000 by path. cannot-check: a 503, the walk incomplete
// (both reads walk one tree, so the first gap of the check's causes). `documents` is the check's
// `counts.documents` for the same project and scenario (src/mocks/check.ts). `error` and `slow`
// act in the client, as for every read.

const HARBOR = "harbor-sim";

/** The link type both mock amendments carry: the mocks' data, never compared by app code. */
const AMENDS = "amends";

/** An `amends` link as `spec show --links` lists it in the document's out links: written in its front matter. */
function amendsLink(fields: Pick<ShownLink, "written" | "path" | "line"> & Partial<ShownLink>): ShownLink {
  return {
    type: AMENDS,
    origin: "frontmatter",
    at: null,
    name: null,
    state: "dangling",
    reason: null,
    ...fields,
  };
}

/** The spec's decision ("Data"): accepted, amending an ID the corpus no longer holds. */
const QUAY_DECISION = QUAY_DECISION_FILE;

/** A feature spec with no `id:`, a draft amending the berth's draft limit. */
const DEEP_BERTH = DEEP_BERTH_FILE;

function harborAmendments(): UnappliedAmendment[] {
  return [
    {
      path: QUAY_DECISION,
      id: "DEC-0014",
      title: "Quay lights stay amber",
      class: "decision",
      status: "accepted",
      links: [
        amendsLink({
          written: "RULE-QUAY-LIGHTS",
          path: QUAY_DECISION,
          line: 8,
          reason: "`RULE-QUAY-LIGHTS` resolves to no ID and no alias",
        }),
      ],
    },
    {
      path: DEEP_BERTH,
      id: null,
      title: "Berth 4 takes 13.0 m draft after the dredging",
      class: "spec",
      status: "draft",
      links: [amendsLink({ written: "RULE-BERTH-DRAFT", name: "RULE-BERTH-DRAFT", state: "resolved", path: DEEP_BERTH, line: 6 })],
    },
  ];
}

/** How many amendments the large scenario lists. */
export const LARGE_AMENDMENTS = 3000;

const pad = (value: number, width = 2) => String(value).padStart(width, "0");

/**
 * The large scenario's amendments, by path: each amends one rule of the generated corpus
 * (src/mocks/harbor-sim/large.ts), every seventh written as an ID that names nothing; every third
 * a decision with an ID, the rest draft specs without one. Their own files are not in the corpus:
 * a path opens the tree's "names nothing"; a resolved link's rule is there.
 */
function largeAmendments(): UnappliedAmendment[] {
  return Array.from({ length: LARGE_AMENDMENTS }, (_, index): UnappliedAmendment => {
    const number = index + 1;
    const path = `docs/features/gen/amend-${pad(number, 4)}.md`;
    const decision = number % 3 === 0;
    const rule = `RULE-GEN-${pad((index % 12) + 1)}-${pad((index % 6) + 1)}-${pad((Math.floor(index / 6) % 6) + 1)}-${String((index % 2) + 1)}`;
    const dangling = number % 7 === 0;
    const written = dangling ? `RULE-GEN-GONE-${pad(number, 4)}` : rule;
    return {
      path,
      id: decision ? `DEC-${pad(number + 1000, 4)}` : null,
      title: `Generated amendment ${String(number)}`,
      class: decision ? "decision" : "spec",
      status: decision ? "accepted" : "draft",
      links: [
        dangling
          ? amendsLink({ written, path, line: 6, reason: `\`${written}\` resolves to no ID and no alias` })
          : amendsLink({ written, name: rule, state: "resolved", path, line: 6 }),
      ],
    };
  });
}

/** The check's document count for the project and scenario (src/mocks/check.ts). */
function documentsOf(project: string, scenario: Scenario): number {
  if (scenario === "empty") {
    return 4;
  }
  if (project !== HARBOR) {
    return 12;
  }
  return scenario === "large" ? 3149 : 41;
}

/**
 * Why the read cannot run in the `cannot-check` scenario: the CLI's line for the first gap of the
 * walk by path, as `spec export index` words it (`docs/features/health-left.md` "Data"); the
 * check's causes for the same project hold it.
 */
export function walkGapMessage(project: string): string {
  const gap = project === HARBOR ? "`docs/spec/cranes.md` cannot be read (Permission denied (os error 13))" : "the directory `docs/spec/api` cannot be listed";
  return `spec: the walk is incomplete: ${gap}: unapplied amendments not measured`;
}

/** The report `spec health --json` gives for `project` in `scenario` (`cannot-check`, `error` and `slow` act in the client). */
export function healthReportOf(project: string, scenario: Scenario): HealthReport {
  let amendments: UnappliedAmendment[] = [];
  if (project === HARBOR && scenario !== "empty") {
    amendments = scenario === "large" ? largeAmendments() : harborAmendments();
  }
  return { documents: documentsOf(project, scenario), amendments, notes: [] };
}
