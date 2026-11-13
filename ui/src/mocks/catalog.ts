import type { CatalogChange, CatalogDocument, CatalogRow, TreeNode } from "../api/types";
import { fakeHex, type MockCorpus } from "./build";
import { treeOf, usageAt, type MockUsage } from "./corpus";
import type { Scenario } from "./scenario";

// A kinds direction as the daemon's `GET .../catalog?kinds=K...` serves it
// (docs/features/direction-catalog.md "Acceptance criteria", DC-6, DC-9, DC-10): the tree's nodes of
// those kinds in its order, each with the state, counts and last change the core would read from the
// trace, the queue and git, here stored by hand per row. Every value is invented: no clock is read.
//
// harbor-sim's direction Mechanics, by the kind `mechanic`: the corpus's own mechanics first, as the
// tree lists them, so their titles open their documents; then invented rows up to 130 without the
// archive, which the tree does not list and the document modal answers as naming nothing. Every
// shape: each of the five states and one this build does not know (`parked`), a version none is
// written for, section rows whose change is their file's, rows with no change read, open items and
// tasks above 0, a title in Cyrillic and one in Japanese, a row with no title, a long title.
// `empty`: no document, so the direction is empty. `large`: the corpus's 500-odd mechanics, no
// invented row. ledger-api configures no direction: the tree's rows of the kinds asked, none invented.
// The spec's own scenario is held too, its JSON verbatim (`pinnedCatalog`).
//
// The usage (docs/features/usage-orphans.md "Acceptance criteria", UO5, UO11): a corpus row's as the
// corpus gives it (src/mocks/corpus.ts `usageAt`), so the catalog, a document's Links and Health's
// orphans agree; an invented row's by hand: every seventh used by none and mentioned by none, an
// orphan unless a section, every fifth mentioned only, the rest used by one to four.

/** harbor-sim's direction without the archive: the corpus's mechanics and invented rows up to this. */
export const HARBOR_CATALOG_ROWS = 130;

/** The states each row takes in turn: the five rollups and one word this build does not know. */
const STATES = ["in work", "no card", "confirmed", "suspect", "departed", "parked"] as const;

/** The people a change is by: invented, a Cyrillic and a Japanese name among them (escaped, `policy.test.ts`). */
const PEOPLE: readonly (readonly [string, string])[] = [
  ["Mira Holt", "mira@harbor.example.invalid"],
  ["Ansel Quay", "ansel@harbor.example.invalid"],
  ["\u041e\u043b\u044f \u041a\u0440\u0430\u0441\u043d\u043e\u0432\u0430", "olya@harbor.example.invalid"],
  ["Tove Brandt", "tove@harbor.example.invalid"],
  ["\u5c71\u7530 \u82b1\u5b50", "hanako@harbor.example.invalid"],
];

/** Invented mechanics' titles, each taken in turn with a number. */
const TITLES = [
  "Anchor chain",
  "Ballast exchange",
  "Bollard load",
  "Buoy lights",
  "Capstan",
  "Dredging",
  "Ferry slip",
  "Fog bell",
  "Gangway",
  "Harbour dues",
  "Ice breaking",
  "Lighterage",
  "Net mending",
  "Quay cranes",
  "Salvage",
  "Tug assist",
  "Warping",
  "Wharf rats",
];

/** A title in Cyrillic, one in Japanese, and one long enough to wrap: line breaking and collation over any script. */
const ODD_TITLES: ReadonlyMap<number, string> = new Map([
  [3, "\u0428\u0432\u0430\u0440\u0442\u043e\u0432\u043a\u0430 \u0443 \u0432\u043d\u0435\u0448\u043d\u0435\u0433\u043e \u043f\u0440\u0438\u0447\u0430\u043b\u0430"],
  [8, "\u6e2f\u306e\u6f6e\u4f4d\u8868"],
  [13, "Night-time berthing of deep-draught vessels at the outer quay when the spring tide runs against the wind"],
]);

/** An invented row's number among the invented ones whose title is none. */
const NO_TITLE = 21;

/** Every this many invented rows, one is a section of the document before it. */
const SECTION_EVERY = 10;

/** An invented row's usage by its place: unused and an orphan, mentioned only, or used by one to four. */
function inventedUsage(at: number, section: boolean): MockUsage {
  if (at % 7 === 0) {
    return { used_by: 0, mentioned_by: 0, orphan: section ? null : true };
  }
  if (at % 5 === 0) {
    return { used_by: 0, mentioned_by: 1 + (at % 3), orphan: section ? null : false };
  }
  return { used_by: 1 + (at % 4), mentioned_by: at % 3, orphan: section ? null : false };
}

/** A row's stored facts by its place in the direction: its state, counts, usage and last change. */
function shaped(node: TreeNode, at: number, section: boolean, usage: MockUsage): CatalogRow {
  const state = STATES[at % STATES.length] ?? "no card";
  const tasks = state === "no card" ? 0 : state === "in work" ? 1 + (at % 3) : state === "departed" ? 2 : 1;
  const open = state === "in work" || state === "suspect" || state === "departed" ? 1 : 0;
  return {
    id: node.id,
    kind: node.kind,
    title: node.title,
    path: node.path,
    line: node.line,
    rev: node.rev,
    status: node.status,
    archived: node.archived,
    state,
    open_items: at % 4 === 1 ? 1 + (at % 5) : 0,
    tasks,
    tasks_open: open,
    used_by: usage.used_by,
    mentioned_by: usage.mentioned_by,
    orphan: usage.orphan,
    last_change: at % 13 === 7 ? null : changeOf(at, section),
  };
}

/** A row's last change: a time over 2026, someone among PEOPLE, a hand commit every third row. */
function changeOf(at: number, ofFile: boolean): CatalogChange {
  const [name, email] = PEOPLE[at % PEOPLE.length] ?? ["Mira Holt", "mira@harbor.example.invalid"];
  // Spread over the year out of row order, so a press on Last change moves the rows.
  const hours = ((at * 7919) % 360) * 23;
  const time = new Date(Date.UTC(2026, 0, 2, 9) + hours * 3_600_000).toISOString().replace(/\.\d{3}Z$/, "Z");
  return {
    at: time,
    by: `${name} <${email}>`,
    by_name: name,
    commit: fakeHex(`catalog-${String(at)}`, 40),
    proposal: at % 3 === 0 ? null : `PR-${String(100 + at).padStart(4, "0")}`,
    of_file: ofFile,
  };
}

/** The invented rows after the corpus's: documents, every tenth a section of the one before. */
function invented(count: number, kind: string): TreeNode[] {
  const rows: TreeNode[] = [];
  let document: TreeNode | null = null;
  for (let at = 1; at <= count; at += 1) {
    const number = String(at).padStart(3, "0");
    if (at % SECTION_EVERY === 0 && document !== null) {
      rows.push({ ...document, id: `${document.id ?? ""}-NOTES`, title: `Notes on ${document.title ?? "it"}`, line: 14, depth: 1, parent: document.id, rev: null });
      continue;
    }
    const word = TITLES[at % TITLES.length] ?? "Mechanic";
    const title = at === NO_TITLE ? null : (ODD_TITLES.get(at) ?? `${word} ${String(Math.floor(at / TITLES.length) + 1)}`);
    document = {
      id: `MEC-CAT-${number}`,
      kind,
      title,
      path: `docs/spec/catalog/mec-cat-${number}.md`,
      line: 1,
      depth: 0,
      parent: null,
      mark: null,
      status: at % 5 === 0 ? "draft" : "accepted",
      rev: at % 9 === 4 ? null : 1 + (at % 4),
      tokens_est: 120,
      archived: false,
    };
    rows.push(document);
  }
  return rows;
}

/** The places of the corpus's documents: any other tree row is a section. */
function documentPlaces(corpus: MockCorpus): Set<string> {
  return new Set(corpus.documents.map((file) => `${file.node.path}:${String(file.node.line)}`));
}

/** A project's catalog for `kinds` in a scenario: the tree's rows, then harbor-sim's invented ones. */
export function catalogOf(corpus: MockCorpus, slug: string, scenario: Scenario, kinds: readonly string[], archive: boolean): CatalogDocument {
  const listed = treeOf(corpus, { kinds: [...kinds], archive }).nodes;
  const documents = documentPlaces(corpus);
  const unarchived = archive ? treeOf(corpus, { kinds: [...kinds] }).nodes.length : listed.length;
  const filling = slug === "harbor-sim" && scenario !== "empty" && kinds.length > 0 ? Math.max(0, HARBOR_CATALOG_ROWS - unarchived) : 0;
  const none: MockUsage = { used_by: 0, mentioned_by: 0, orphan: null };
  const rows = [
    ...listed.map((node, at) => shaped(node, at, !documents.has(`${node.path}:${String(node.line)}`), usageAt(corpus, node.path, node.line) ?? none)),
    ...invented(filling, kinds[0] ?? "").map((node, at) => shaped(node, listed.length + at, node.line !== 1, inventedUsage(listed.length + at, node.line !== 1))),
  ];
  return { kinds: [...kinds], archive, notes: [], rows, truncated: false, omitted: null };
}

/**
 * The spec's scenario, its JSON verbatim: a git copy of `fixtures/studio-lamp`, Ann Owner applying
 * PR-0001 to the arrival, then Bob Hand editing the storm by hand; C-0001 open on the arrival, PR-0040
 * an open question naming it. Its OIDs stand as 40 hex digits.
 */
export function pinnedCatalog(): CatalogDocument {
  return {
    kinds: ["scene"],
    archive: false,
    notes: [],
    rows: [
      {
        id: "SCN-ARRIVAL",
        kind: "scene",
        title: "Arrival",
        path: "docs/spec/scn-arrival.md",
        line: 1,
        rev: 1,
        status: null,
        archived: false,
        state: "in work",
        open_items: 1,
        tasks: 1,
        tasks_open: 1,
        used_by: 1,
        mentioned_by: 0,
        orphan: false,
        last_change: { at: "2026-01-02T10:00:00Z", by: "Ann Owner <ann@example.org>", by_name: "Ann Owner", commit: fakeHex("scn-arrival", 40), proposal: "PR-0001", of_file: false },
      },
      {
        id: "SCN-STORM",
        kind: "scene",
        title: "The storm",
        path: "docs/spec/scn-storm.md",
        line: 1,
        rev: 1,
        status: null,
        archived: false,
        state: "no card",
        open_items: 0,
        tasks: 0,
        tasks_open: 0,
        used_by: 0,
        mentioned_by: 0,
        orphan: false,
        last_change: { at: "2026-01-03T10:00:00Z", by: "Bob Hand <bob@example.org>", by_name: "Bob Hand", commit: fakeHex("scn-storm", 40), proposal: null, of_file: false },
      },
    ],
    truncated: false,
    omitted: null,
  };
}
