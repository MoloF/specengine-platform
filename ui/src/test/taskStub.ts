import type { TaskListEntry, TaskPackage } from "../api/types";
import { aTaskEntry, aTaskPackage } from "./builders";
import { stubClient } from "./stubClient";

/**
 * A stub client serving tasks: the list from `entries`, each package from `packages` (else one
 * built for that row), an unknown T as the exit-1 document. Every method counts its calls.
 */
export function taskClient(entries: TaskListEntry[], packages: TaskPackage[] = [], notes: string[] = []) {
  const client = stubClient();
  client.getTasks.mockImplementation(() => Promise.resolve(structuredClone({ tasks: entries, notes })));
  client.getTask.mockImplementation((_project, id) => {
    const given = packages.find((candidate) => candidate.id === id);
    if (given !== undefined) {
      return Promise.resolve(structuredClone(given));
    }
    const entry = entries.find((candidate) => candidate.id === id);
    if (entry !== undefined) {
      return Promise.resolve(aTaskPackage({ id, status: entry.status, title: entry.title, stale: entry.stale }));
    }
    return Promise.resolve({ id, reason: `no task ${id} in this repository` });
  });
  return client;
}

/** A list with one task per reachable state, a closed pair and an unknown state. */
export const SOME_TASKS: TaskListEntry[] = [
  aTaskEntry({ id: "T-0001", status: "done", title: "Finished" }),
  aTaskEntry({ id: "T-0002", status: "review", title: "Plan to read", targets: ["R-1", "R-2", "R-3", "R-4", "R-5"] }),
  aTaskEntry({ id: "T-0003", status: "ready", title: "Changed under it", stale: true, targets: ["R-1"] }),
  aTaskEntry({ id: "T-0004", status: "ready", title: "Calm", stale: false }),
  aTaskEntry({ id: "T-0005", status: "draft", title: null }),
  aTaskEntry({ id: "T-0006", status: "in_progress", title: "Under way", stale: true }),
  aTaskEntry({ id: "T-0007", status: "changes_requested", title: "Sent back" }),
  aTaskEntry({ id: "T-0008", status: "cancelled", title: "Dropped" }),
  aTaskEntry({ id: "T-0009", status: "triage", title: "Odd state" }),
];
