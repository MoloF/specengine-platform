import type { Proposal, TaskListEntry } from "../api/types";
import { taskClient } from "./taskStub";

/**
 * A stub client for the home and the palette: the tasks from `entries` (their notes `taskNotes`),
 * the queue from `proposals` (its notes `inboxNotes`). Every method counts its calls.
 */
export function homeClient(
  entries: TaskListEntry[],
  proposals: Proposal[] = [],
  { taskNotes = [], inboxNotes = [] }: { taskNotes?: string[]; inboxNotes?: string[] } = {},
) {
  const client = taskClient(entries, [], taskNotes);
  client.state.proposals = [...proposals];
  client.state.notes = [...inboxNotes];
  return client;
}

/** Every call the client's methods took, by method; a test compares it whole. */
export function callsOf(client: object): Record<string, number> {
  const counted: Record<string, number> = {};
  for (const [name, value] of Object.entries(client)) {
    if (typeof value === "function" && "mock" in value) {
      const calls = (value as { mock: { calls: unknown[] } }).mock.calls.length;
      if (calls > 0) {
        counted[name] = calls;
      }
    }
  }
  return counted;
}
