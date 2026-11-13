import type { HealthOrphan, HealthOrphans } from "../api/types";
import type { MockCorpus } from "./build";
import type { MockBoard } from "./cards";
import { judgedDocuments } from "./corpus";
import type { Scenario } from "./scenario";

// The mock's row 8 of `spec health --json` (docs/features/usage-orphans.md "Acceptance criteria",
// UO4): every live document of the corpus judged as the core judges it (src/mocks/corpus.ts
// `usagesOf`), each orphan with the tasks the board holds on it or a section of it, cancelled ones
// ignored, open those not accepted; by kind in code-point order, no kind last, then by path. No clock.
//
// normal: harbor-sim's five as its corpus has them, two mechanics with an open task each (C-0004 on
// the lock, C-0008 on the cranes) and three decisions with no kind and none; ledger-api none over its
// five. empty: none over none. large: harbor-sim's five among 3 000, the rest invented documents the
// corpus does not hold (a path opens the modal's "names nothing"), a task on every tenth.
// orphan-notes: as normal but the queue not read, so every row's tasks are null and the note says why.

/** How many orphans the large scenario lists. */
export const LARGE_ORPHANS = 3000;

/** Why the queue was not read in `orphan-notes`: the store's words for a queue of a newer build. */
export const ORPHANS_QUEUE_NOTE =
  "orphans: tasks not read: the proposal queue's tables are of a newer SpecEngine (schema version 15, this build knows 14): upgrade SpecEngine (nothing changed)";

/** Code-point order, a kind's and a path's: never a locale's. */
function byCodePoint(a: string, b: string): number {
  return a < b ? -1 : a > b ? 1 : 0;
}

/** The core's order: by kind, no kind last, then by path. */
function byKindThenPath(a: HealthOrphan, b: HealthOrphan): number {
  if (a.kind !== b.kind) {
    return a.kind === null ? 1 : b.kind === null ? -1 : byCodePoint(a.kind, b.kind);
  }
  return byCodePoint(a.path, b.path);
}

/** The cards on a document or one of its sections, cancelled ones ignored, and how many of them are open. */
function tasksOn(board: MockBoard | undefined, names: readonly string[]): { tasks: number; open: number } {
  const standing = (board?.cards ?? []).filter((card) => card.cancellation === null && card.targets.some((target) => names.includes(target)));
  return { tasks: standing.length, open: standing.filter((card) => card.acceptance === null).length };
}

const pad = (value: number) => String(value).padStart(4, "0");

/** Kinds the invented orphans take in turn: harbor-sim's direction's, two raw ones and none. */
const INVENTED_KINDS = ["mechanic", "rule", "requirement", null] as const;

/** The large scenario's invented orphans after the corpus's own: live documents nothing links. */
function invented(count: number): HealthOrphan[] {
  return Array.from({ length: count }, (_, index): HealthOrphan => {
    const number = index + 1;
    const kind = INVENTED_KINDS[index % INVENTED_KINDS.length] ?? null;
    const id = kind === null ? null : `GEN-ORPHAN-${pad(number)}`;
    const tasks = number % 10 === 0 ? 1 : 0;
    return {
      name: id ?? `docs/spec/gen/orphans/o-${pad(number)}.md`,
      id,
      kind,
      title: `Generated orphan ${String(number)}`,
      path: `docs/spec/gen/orphans/o-${pad(number)}.md`,
      line: 1,
      status: number % 3 === 0 ? "accepted" : "draft",
      tasks,
      tasks_open: tasks,
    };
  });
}

/** Row 8 for a project's corpus and board in a scenario; the large scenario's invented rows are harbor-sim's alone. */
export function orphansOf(slug: string, corpus: MockCorpus, board: MockBoard | undefined, scenario: Scenario): HealthOrphans {
  const judged = judgedDocuments(corpus);
  const read = scenario !== "orphan-notes";
  const rows = judged
    .filter(({ usage }) => usage.orphan === true)
    .map(({ node, sections }): HealthOrphan => {
      const name = node.id ?? node.path;
      const { tasks, open } = tasksOn(board, [name, ...sections]);
      return {
        name,
        id: node.id,
        kind: node.kind,
        title: node.title,
        path: node.path,
        line: node.line,
        status: node.status,
        tasks: read ? tasks : null,
        tasks_open: read ? open : null,
      };
    });
  const extra = scenario === "large" && slug === "harbor-sim" ? invented(Math.max(0, LARGE_ORPHANS - rows.length)) : [];
  return { live: judged.length + extra.length, rows: [...rows, ...extra].sort(byKindThenPath), notes: read ? [] : [ORPHANS_QUEUE_NOTE] };
}
