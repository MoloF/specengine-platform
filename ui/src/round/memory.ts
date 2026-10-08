import { useSyncExternalStore } from "react";
import type { ApiError, RoundAnswerEntry } from "../api/types";
import type { Drafts } from "./drafts";

// The Questions screen's memory for this page's life, per project (`docs/features/ui-round.md`
// "Description and interactions": drafts in tab memory, surviving section switches, not a reload).
// One store the shell holds, read by every screen that mounts: a POST that settles after the owner
// left and came back clears its drafts and leaves its outcome where the screen now on view reads
// them, and no screen keeps a copy of its own to write back over it.

/** What the last POST said, shown under the sheet: its summary, the daemon's notes, how many of the answers sent it staged. */
export interface RoundResult {
  text: string;
  notes: readonly string[];
  staged: number;
  sent: number;
}

/** One project's entry. */
export interface RoundEntry {
  readonly drafts: Drafts;
  /** The `limit` read (null: the daemon's default), so "Show all" outlives a section switch. */
  readonly limit: number | null;
  /** The last POST's answers that were not staged, by item. */
  readonly refused: ReadonlyMap<string, RoundAnswerEntry>;
  readonly result: RoundResult | null;
  /** The last POST's failure, in the daemon's words. */
  readonly failure: ApiError | null;
  /** Items whose answers a re-read dropped: no longer on the sheet. */
  readonly dropped: readonly string[];
}

export const EMPTY_ROUND: RoundEntry = { drafts: new Map(), limit: null, refused: new Map(), result: null, failure: null, dropped: [] };

/**
 * The store: read a project's entry, change it, hear of any change; and which projects have a
 * Questions screen on view (`attach` on mount, its return on unmount), so a POST settling after its
 * screen left speaks only when no screen of that project shows the outcome itself.
 */
export interface RoundMemory {
  read: (project: string) => RoundEntry;
  update: (project: string, change: (entry: RoundEntry) => RoundEntry) => void;
  subscribe: (listener: () => void) => () => void;
  attach: (project: string) => () => void;
  onView: (project: string) => boolean;
}

/** A new, empty store: one per page (the shell's), one per test. */
export function createRoundMemory(): RoundMemory {
  const entries = new Map<string, RoundEntry>();
  const listeners = new Set<() => void>();
  /** Questions screens on view, by project. */
  const views = new Map<string, number>();
  return {
    read: (project) => entries.get(project) ?? EMPTY_ROUND,
    update: (project, change) => {
      const before = entries.get(project) ?? EMPTY_ROUND;
      const after = change(before);
      if (after === before) {
        return;
      }
      entries.set(project, after);
      for (const listener of [...listeners]) {
        listener();
      }
    },
    subscribe: (listener) => {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    attach: (project) => {
      views.set(project, (views.get(project) ?? 0) + 1);
      return () => {
        views.set(project, (views.get(project) ?? 1) - 1);
      };
    },
    onView: (project) => (views.get(project) ?? 0) > 0,
  };
}

/** A project's entry, the component rendering again whenever it changes. */
export function useRoundEntry(memory: RoundMemory, project: string): RoundEntry {
  return useSyncExternalStore(memory.subscribe, () => memory.read(project));
}
