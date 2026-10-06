import { describe, expect, it } from "vitest";
import type { KnownRunOutcome, KnownTaskStatus } from "../api/types";
import { runOutcomeLook, stalenessLook, taskStatusLook } from "./labels";

// AC-03 of docs/features/ui-tasks.md: ten states and four outcomes, each a distinct label and an
// icon; an unknown value raw in a neutral badge. AC-06: the four staleness displays.

const STATES = {
  draft: true,
  analysis: true,
  review: true,
  changes_requested: true,
  ready: true,
  in_progress: true,
  in_review: true,
  done: true,
  accepted: true,
  cancelled: true,
} satisfies Record<KnownTaskStatus, true>;

const OUTCOMES = { completed: true, partial: true, failed: true, abandoned: true } satisfies Record<KnownRunOutcome, true>;

describe("task states (AC-03)", () => {
  const looks = Object.keys(STATES).map((status) => taskStatusLook(status));

  it("give each of the ten its own label, a colour role of its own and an icon", () => {
    expect(looks).toHaveLength(10);
    expect(new Set(looks.map((look) => look.label)).size).toBe(10);
    expect(new Set(looks.map((look) => look.tone)).size).toBe(10);
    for (const look of looks) {
      expect(look.label).not.toBe("");
      expect(look.icon).not.toBe("unknown");
      expect(look.tone).toMatch(/^task-/);
    }
  });

  it("show an unknown state raw in the neutral badge", () => {
    expect(taskStatusLook("triage")).toEqual({ label: "triage", tone: "task-unknown", icon: "unknown" });
    expect(taskStatusLook("hasOwnProperty").label).toBe("hasOwnProperty");
  });
});

describe("run outcomes (AC-03)", () => {
  it("give each of the four its own label and an icon; an unknown one raw", () => {
    const looks = Object.keys(OUTCOMES).map((outcome) => runOutcomeLook(outcome));
    expect(new Set(looks.map((look) => look.label)).size).toBe(4);
    for (const look of looks) {
      expect(look.icon).not.toBe("unknown");
      expect(look.tone).toMatch(/^run-/);
    }
    expect(runOutcomeLook("postponed")).toEqual({ label: "postponed", tone: "run-unknown", icon: "unknown" });
  });
});

describe("staleness (AC-06)", () => {
  it("has four displays: changed, unchanged, unknown with a snapshot, none without one", () => {
    expect(stalenessLook(true, true)?.label).toBe("Spec changed since approval");
    expect(stalenessLook(false, true)?.label).toBe("Unchanged since approval");
    expect(stalenessLook(null, true)?.label).toBe("Unknown");
    expect(stalenessLook(null, false)).toBeNull();
    expect(stalenessLook(null, true)?.tone).not.toBe(stalenessLook(false, true)?.tone);
  });
});
