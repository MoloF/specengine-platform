import type { LibraryChange, LibraryDecision, LibraryDocument, LibraryRow, WorkspaceModule } from "../api/types";
import { fakeHex, type MockCorpus, type MockDocument } from "./build";
import { nameOf, usageAt } from "./corpus";
import type { Scenario } from "./scenario";
import { workspacesOf } from "./workspaces";

// Every document as the daemon's `GET .../library[?archive=true]` serves it
// (docs/features/doc-library.md "The UI seam"): one row per spec file holding a document, in path
// order, the uses and the orphan flag as the corpus gives them (src/mocks/corpus.ts `usageAt`), the
// modules computed over the project's served `[[module]]`s as the core holds them, `behind` and
// `newer` from the stored last changes as the core judges them. Everything else is stored by hand:
// the mock corpus writes no class, tier, owner, review date, tag or `canon:` for most documents,
// so harbor-sim's are set here per document, and its decisions per canon page. Every value is
// invented; no clock is read.
//
// harbor-sim: six canon pages, tags on most rows (one in Cyrillic, one in Japanese), an invalid
// review date as written, an owner in each script; the harbour's canon page holds an accepted
// decision committed after it (behind), a superseded and a rejected one; the waters' page an
// accepted decision committed before it and one of another status; the cranes' page an accepted
// decision and no commit of its own (behind not known). DEC-0009, DEC-0010 and DEC-0011 are no
// documents of the corpus: the document modal answers them as naming nothing. The corpus's orphans
// are orphans here. ledger-api: its documents as the corpus writes them, git not read, with the
// note. `empty`: no document. `large`: 5 000 rows from a fixed generator, never `Math.random`.

/** What harbor-sim's files do not write, set per document by name. */
interface Shelf {
  class?: string;
  tier?: number;
  owner?: string;
  reviewed?: string;
  scope?: string[];
  tags?: string[];
  /** Its last change: a stored time and who, or null for a file never committed. Absent: one made up by its place. */
  change?: { at: string; by: number } | null;
}

/** The people a change is by and the owners: invented, a Cyrillic and a Japanese name among them (escaped, `policy.test.ts`). */
const PEOPLE: readonly (readonly [string, string])[] = [
  ["Mira Holt", "mira@harbor.example.invalid"],
  ["Ansel Quay", "ansel@harbor.example.invalid"],
  ["\u041e\u043b\u044f \u041a\u0440\u0430\u0441\u043d\u043e\u0432\u0430", "olya@harbor.example.invalid"],
  ["Tove Brandt", "tove@harbor.example.invalid"],
  ["\u5c71\u7530 \u82b1\u5b50", "hanako@harbor.example.invalid"],
];

function person(at: number): readonly [string, string] {
  return PEOPLE[at % PEOPLE.length] ?? ["Mira Holt", "mira@harbor.example.invalid"];
}

/** harbor-sim's shelf: canon pages, owners, review dates, scopes and tags, and the stored times `behind` turns on. */
const HARBOR_SHELF: Readonly<Record<string, Shelf>> = {
  "DOM-HARBOR": { class: "canon", tier: 1, owner: "Mira Holt", reviewed: "2026-09-20", tags: ["clock", "simulation"], change: { at: "2026-09-20T08:30:00Z", by: 0 } },
  "DOM-BERTHS": { class: "canon", tier: 2, owner: "Ansel Quay", reviewed: "2026-10-11", tags: ["berths", "\u043f\u0440\u0438\u0447\u0430\u043b"] },
  "DOM-WATER": { class: "canon", tier: 2, owner: "\u041e\u043b\u044f \u041a\u0440\u0430\u0441\u043d\u043e\u0432\u0430", reviewed: "2026-10-20", tags: ["tides", "\u6f6e"], change: { at: "2026-10-20T15:00:00Z", by: 2 } },
  "DOM-PILOTS": { class: "canon", tier: 2, owner: "Tove Brandt", reviewed: "2026-13-40", tags: ["pilots"] },
  "MEC-TIDES": { class: "canon", tier: 2, owner: "\u041e\u043b\u044f \u041a\u0440\u0430\u0441\u043d\u043e\u0432\u0430", reviewed: "2026-10-01", tags: ["tides", "tuning"] },
  "MEC-CARGO-CRANES": { class: "canon", tier: 2, owner: "\u5c71\u7530 \u82b1\u5b50", tags: ["cranes", "tuning"], change: null },
  "MEC-MOORING": { class: "spec", tier: 2, owner: "Ansel Quay", scope: ["berths"], tags: ["berths", "crew"] },
  "MEC-FAIRWAY": { class: "spec", tier: 2, tags: ["navigation"] },
  "MEC-NIGHT-PASSAGE": { class: "spec", tier: 2, tags: ["navigation", "night"] },
  "MEC-PILOTAGE": { class: "spec", tier: 2, owner: "Tove Brandt", tags: ["pilots"] },
  "MEC-LOCK-A": { class: "spec", tags: ["locks"] },
  "MEC-LOCK-B": { class: "spec", tags: ["locks"] },
  "RULE-SPRING-WINDOW": { tags: ["tides"] },
  "DEC-0012": { tier: 2, change: { at: "2026-10-01T09:00:00Z", by: 0 } },
  "DEC-0013": { tier: 2, change: { at: "2026-11-11T10:15:00Z", by: 4 } },
  "DEC-0014": { tier: 2, change: { at: "2026-10-02T12:00:00Z", by: 1 } },
  "docs/features/deep-berth.md": { tier: 3, scope: ["berths"] },
  "MEC-OLD-QUAYS": { tier: 3, tags: ["berths"] },
};

/** A decision on a canon page: a corpus document by name, or an invented one with what its file would hold. */
type Placed =
  | { name: string; section: string | null }
  | { name: string; title: string; status: string | null; group: string; date: string | null; section: string | null; change: { at: string; by: number } | null };

/** harbor-sim's decisions per canon page, in the core's order: accepted, superseded, rejected, other; `date` descending. */
const HARBOR_DECISIONS: Readonly<Record<string, readonly Placed[]>> = {
  "DOM-HARBOR": [
    { name: "DEC-0012", section: "RULE-HARBOR-CLOCK" },
    { name: "DEC-0009", title: "Weather from a fixed table", status: "superseded-by DEC-0012", group: "superseded", date: "2026-06-02", section: null, change: { at: "2026-06-02T11:00:00Z", by: 3 } },
    { name: "DEC-0010", title: "Two clocks, one per basin", status: "rejected", group: "rejected", date: "2026-06-15", section: "RULE-HARBOR-CLOCK", change: { at: "2026-06-15T14:20:00Z", by: 1 } },
  ],
  "DOM-WATER": [
    { name: "DEC-0014", section: null },
    { name: "DEC-0011", title: "Tide tables from the port authority", status: "proposed", group: "other", date: null, section: null, change: null },
  ],
  "MEC-CARGO-CRANES": [{ name: "DEC-0013", section: null }],
};

/** A front-matter key's value as a file writes it on one line; null when it writes none. */
function written(file: MockDocument, key: string): string | null {
  if (file.lines[0] !== "---") {
    return null;
  }
  for (const line of file.lines.slice(1)) {
    if (line === "---") {
      return null;
    }
    if (line.startsWith(`${key}:`)) {
      const value = line.slice(key.length + 1).trim();
      return value === "" ? null : value;
    }
  }
  return null;
}

/** A change at a stored time by one of PEOPLE. */
function changeAt(at: string, by: number, seed: string): LibraryChange {
  const [name, email] = person(by);
  return { at, by: `${name} <${email}>`, by_name: name, commit: fakeHex(`library-${seed}`, 40), proposal: by % 2 === 0 ? null : `PR-${String(60 + by).padStart(4, "0")}` };
}

/** A made-up change by a row's place: spread over 2026 out of path order, none every ninth. */
function placeChange(at: number, seed: string): LibraryChange | null {
  if (at % 9 === 8) {
    return null;
  }
  const hours = ((at * 7919) % 300) * 23;
  const time = new Date(Date.UTC(2026, 0, 5, 9) + hours * 3_600_000).toISOString().replace(/\.\d{3}Z$/, "Z");
  return changeAt(time, at, seed);
}

/** Whether a decision's file was committed after the row's: null when either is not read; to the second, as the core compares. */
function newerThan(decision: LibraryChange | null, row: LibraryChange | null): boolean | null {
  return decision === null || row === null ? null : decision.at > row.at;
}

/** `behind` as the core judges it: an accepted decision newer, else not known when one cannot be told, else false. */
function behindOf(decisions: readonly LibraryDecision[]): boolean | null {
  const accepted = decisions.filter((decision) => decision.group === "accepted");
  if (accepted.some((decision) => decision.newer === true)) {
    return true;
  }
  return accepted.some((decision) => decision.newer === null) ? null : false;
}

/** The served modules holding a row: a direction by its kind, a rooted module by its root among the row and its ancestors. */
function modulesOf(modules: readonly WorkspaceModule[], kind: string | null, chain: readonly string[]): string[] {
  return modules
    .filter((module) => (module.root === null ? kind !== null && (module.kinds ?? []).includes(kind) : chain.includes(module.root)))
    .map((module) => module.name);
}

/** A row's name and its ancestors' names, first the row's, cycle-guarded. */
function chainOf(name: string, parents: ReadonlyMap<string, string>): string[] {
  const chain = [name];
  let next = parents.get(name);
  while (next !== undefined && !chain.includes(next)) {
    chain.push(next);
    next = parents.get(next);
  }
  return chain;
}

/** The counts over the rows served. */
function countsOf(rows: readonly LibraryRow[]): LibraryDocument["counts"] {
  return {
    documents: rows.length,
    archived: rows.filter((row) => row.archived).length,
    decisions: rows.reduce((sum, row) => sum + row.decisions.length, 0),
    behind: rows.filter((row) => row.behind === true).length,
    orphans: rows.filter((row) => row.orphan === true).length,
  };
}

/** Code-point order, as the core sorts paths. */
function byPath(a: { path: string }, b: { path: string }): number {
  return a.path < b.path ? -1 : a.path > b.path ? 1 : 0;
}

/** A project's library over its corpus: harbor-sim's shelf and decisions set by hand, every other project's as its files write them. */
function corpusLibrary(corpus: MockCorpus, slug: string, scenario: Scenario, archive: boolean): LibraryDocument {
  const harbor = slug === "harbor-sim";
  const shelf = harbor ? HARBOR_SHELF : {};
  const placed = harbor ? HARBOR_DECISIONS : {};
  const gitRead = harbor;
  const modules = workspacesOf(slug, scenario).modules;
  const files = [...corpus.documents].sort(byPath);
  const names = new Set(files.map((file) => nameOf(file.node)));
  const parents = new Map(files.flatMap((file) => (file.node.parent !== null && names.has(file.node.parent) ? [[nameOf(file.node), file.node.parent] as const] : [])));
  const changes = new Map<string, LibraryChange | null>();
  files.forEach((file, at) => {
    const name = nameOf(file.node);
    const stored = shelf[name]?.change;
    changes.set(name, !gitRead ? null : stored === undefined ? placeChange(at, name) : stored === null ? null : changeAt(stored.at, stored.by, name));
  });
  const byName = new Map(files.map((file) => [nameOf(file.node), file]));
  const rows = files
    .filter((file) => archive || !file.node.archived)
    .map((file): LibraryRow => {
      const node = file.node;
      const name = nameOf(node);
      const own = shelf[name] ?? {};
      const change = changes.get(name) ?? null;
      const decisions = (placed[name] ?? []).map((entry): LibraryDecision => {
        if ("group" in entry) {
          const decided = entry.change === null || !gitRead ? null : changeAt(entry.change.at, entry.change.by, entry.name);
          return {
            name: entry.name,
            id: entry.name,
            title: entry.title,
            path: `docs/spec/archive/${entry.name}.md`,
            status: entry.status,
            group: entry.group,
            date: entry.date,
            section: entry.section,
            last_change: decided,
            newer: newerThan(decided, change),
          };
        }
        const record = byName.get(entry.name);
        const decided = changes.get(entry.name) ?? null;
        return {
          name: entry.name,
          id: record?.node.id ?? entry.name,
          title: record?.node.title ?? null,
          path: record?.path ?? `docs/spec/records/${entry.name}.md`,
          status: record?.node.status ?? null,
          group: "accepted",
          date: record === undefined ? null : written(record, "date"),
          section: entry.section,
          last_change: decided,
          newer: newerThan(decided, change),
        };
      });
      const usage = usageAt(corpus, node.path, node.line);
      return {
        name,
        id: node.id,
        path: node.path,
        title: node.title,
        class: own.class ?? written(file, "class"),
        kind: node.kind,
        status: node.status,
        archived: node.archived,
        tier: node.archived ? 3 : (own.tier ?? null),
        owner: own.owner ?? null,
        reviewed: own.reviewed ?? null,
        scope: own.scope ?? null,
        tags: own.tags ?? [],
        rev: node.rev,
        parent: parents.get(name) ?? null,
        modules: modulesOf(modules, node.kind, chainOf(name, parents)),
        used_by: usage?.used_by ?? 0,
        mentioned_by: usage?.mentioned_by ?? 0,
        orphan: usage?.orphan ?? null,
        last_change: change,
        decisions,
        behind: behindOf(decisions),
      };
    });
  const notes = gitRead || rows.length === 0 ? [] : ["library: last change not read: not a repository"];
  return { archive, notes, counts: countsOf(rows), rows, truncated: false, omitted: null };
}

/** The large library's row count. */
export const LARGE_LIBRARY_ROWS = 5000;

/** Numerical Recipes' LCG: a fixed sequence in [0, 1), the same on every run. */
function generator(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state = (Math.imul(state, 1664525) + 1013904223) >>> 0;
    return state / 4294967296;
  };
}

/** The large library's words, dealt in turn: classes (none among them), kinds, statuses, owners, tags in three scripts. */
const LARGE_CLASSES: readonly (string | null)[] = ["canon", "spec", "canon", "decision", "spec", null, "spec", "generated"];
const LARGE_KINDS: readonly (string | null)[] = ["mechanic", "rule", "domain", "quest", null, "item", "mechanic", "dialogue"];
const LARGE_STATUSES: readonly (string | null)[] = ["accepted", "draft", "proposed", null, "accepted"];
const LARGE_TAGS: readonly string[] = [
  "tides",
  "berths",
  "pilots",
  "cranes",
  "night",
  "tuning",
  "crew",
  "locks",
  "weather",
  "cargo",
  "economy",
  "ui",
  "audio",
  "\u043f\u0440\u0438\u0447\u0430\u043b",
  "\u0432\u0435\u0442\u0435\u0440",
  "\u6f6e",
  "\u8239",
];

/** `large`: 5 000 rows in 50 folders, every tenth under the berths' root, every fourth canon page holding an accepted and a superseded decision, every hundredth behind. */
function largeLibrary(slug: string, scenario: Scenario, archive: boolean): LibraryDocument {
  const random = generator(20261119);
  const modules = workspacesOf(slug, scenario).modules;
  const rows: LibraryRow[] = [];
  for (let at = 0; at < LARGE_LIBRARY_ROWS; at += 1) {
    const folder = String(Math.floor(at / 100)).padStart(2, "0");
    const number = String(at).padStart(4, "0");
    const name = `LIB-${number}`;
    const klass = LARGE_CLASSES[at % LARGE_CLASSES.length] ?? null;
    const kind = LARGE_KINDS[at % LARGE_KINDS.length] ?? null;
    const tags = LARGE_TAGS.filter(() => random() < 0.12);
    const change = placeChange(at, name);
    const canonAt = klass === "canon" && at % 4 === 0;
    const decided = (offset: number) => (change === null ? null : { ...change, at: new Date(Date.parse(change.at) + offset * 60_000).toISOString().replace(/\.\d{3}Z$/, "Z"), commit: fakeHex(`large-${name}-${String(offset)}`, 40) });
    const decisions: LibraryDecision[] = canonAt
      ? [
          { name: `DEC-L${number}`, id: `DEC-L${number}`, title: `Decision on document ${String(at)}`, path: `docs/records/dec-l${number}.md`, status: "accepted", group: "accepted", date: "2026-10-01", section: null, last_change: decided(at % 100 === 0 ? 60 : -60), newer: null },
          { name: `DEC-S${number}`, id: `DEC-S${number}`, title: `Earlier decision on document ${String(at)}`, path: `docs/records/archive/dec-s${number}.md`, status: `superseded-by DEC-L${number}`, group: "superseded", date: "2026-03-01", section: null, last_change: decided(-600), newer: null },
        ].map((decision) => ({ ...decision, newer: newerThan(decision.last_change, change) }))
      : [];
    rows.push({
      name,
      id: name,
      path: `docs/spec/l${folder}/d${number}.md`,
      title: at % 97 === 0 ? null : at % 50 === 3 ? `\u0414\u043e\u043a\u0443\u043c\u0435\u043d\u0442 ${String(at)}` : `Library document ${String(at)}`,
      class: klass,
      kind,
      status: LARGE_STATUSES[at % LARGE_STATUSES.length] ?? null,
      archived: false,
      tier: at % 6 === 5 ? null : 1 + (at % 3),
      owner: at % 7 === 6 ? null : person(at)[0],
      reviewed: at % 5 === 0 ? null : `2026-${String(1 + (at % 12)).padStart(2, "0")}-${String(1 + (at % 28)).padStart(2, "0")}`,
      scope: at % 11 === 0 ? ["sprint"] : null,
      tags,
      rev: 1 + (at % 4),
      parent: at % 10 === 0 ? "DOM-BERTHS" : null,
      modules: modulesOf(modules, kind, at % 10 === 0 ? [name, "DOM-BERTHS"] : [name]),
      used_by: Math.floor(random() ** 3 * 40),
      mentioned_by: Math.floor(random() * 4),
      orphan: at % 23 === 0,
      last_change: change,
      decisions,
      behind: behindOf(decisions),
    });
  }
  return { archive, notes: [], counts: countsOf(rows), rows, truncated: false, omitted: null };
}

/** A project's library in a scenario; `large` harbor-sim's 5 000 rows. */
export function libraryOf(corpus: MockCorpus, slug: string, scenario: Scenario, archive: boolean): LibraryDocument {
  return scenario === "large" && slug === "harbor-sim" ? largeLibrary(slug, scenario, archive) : corpusLibrary(corpus, slug, scenario, archive);
}
