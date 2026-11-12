import { describe, expect, it } from "vitest";
import type { KnownRunOutcome, KnownTaskStatus } from "../api/types";
import { runOutcomeLook, stalenessLook, taskStatusLook, taskStatusLozenge } from "./labels";
import { enT } from "../test/text";
import { ru } from "../i18n/ru";
import { ruT } from "../test/text";
import * as tasks from "./labels";
import { groupsOf } from "./groups";
import { aTaskEntry } from "../test/builders";

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
  const looks = Object.keys(STATES).map((status) => taskStatusLook(status, enT));

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
    expect(taskStatusLook("triage", enT)).toEqual({ label: "triage", tone: "task-unknown", icon: "unknown", served: true });
    expect(taskStatusLook("hasOwnProperty", enT).label).toBe("hasOwnProperty");
  });
});

describe("run outcomes (AC-03)", () => {
  it("give each of the four its own label and an icon; an unknown one raw", () => {
    const looks = Object.keys(OUTCOMES).map((outcome) => runOutcomeLook(outcome, enT));
    expect(new Set(looks.map((look) => look.label)).size).toBe(4);
    for (const look of looks) {
      expect(look.icon).not.toBe("unknown");
      expect(look.tone).toMatch(/^run-/);
    }
    expect(runOutcomeLook("postponed", enT)).toEqual({ label: "postponed", tone: "run-unknown", icon: "unknown", served: true });
  });
});

describe("staleness (AC-06)", () => {
  it("has four displays: changed, unchanged, unknown with a snapshot, none without one", () => {
    expect(stalenessLook(true, true, enT)?.label).toBe("Spec changed since approval");
    expect(stalenessLook(false, true, enT)?.label).toBe("Unchanged since approval");
    expect(stalenessLook(null, true, enT)?.label).toBe("Unknown");
    expect(stalenessLook(null, false, enT)).toBeNull();
    expect(stalenessLook(null, true, enT)?.tone).not.toBe(stalenessLook(false, true, enT)?.tone);
  });
});

// LG-3 of docs/features/ui-languages.md: under `ruT` every known value's label is ru.ts's value at
// its key, so no label of the table is left a literal (M9); an unknown value stays raw.
describe("the Tasks tables in Russian (LG-3)", () => {
  it("labels each of the ten states and four outcomes from ru.ts, an unknown one raw", () => {
    expect(tasks.KNOWN_TASK_STATUSES).toHaveLength(10);
    for (const status of tasks.KNOWN_TASK_STATUSES) {
      expect([status, tasks.taskStatusLook(status, ruT).label]).toEqual([status, ru.tasks.status[status]]);
    }
    for (const outcome of tasks.KNOWN_RUN_OUTCOMES) {
      expect([outcome, tasks.runOutcomeLook(outcome, ruT).label]).toEqual([outcome, ru.tasks.outcome[outcome]]);
    }
    expect(tasks.taskStatusLook("paused", ruT).label).toBe("paused");
    expect(tasks.runOutcomeLook("timed_out", ruT).label).toBe("timed_out");
  });

  it("words staleness, the list's mark, the frozen note and the groups from ru.ts", () => {
    expect([true, false, null].map((stale) => tasks.stalenessLook(stale, true, ruT)?.label)).toEqual([ru.tasks.staleness.changed, ru.tasks.staleness.unchanged, ru.tasks.staleness.unknown]);
    expect(tasks.specChanged(ruT).label).toBe(ru.tasks.specChanged);
    expect(tasks.frozenAtApproval(ruT)).toBe(ru.tasks.frozenAtApproval);
    const titles = groupsOf(
      [aTaskEntry({ id: "T-1", status: "review" }), aTaskEntry({ id: "T-2", status: "done" }), aTaskEntry({ id: "T-3", status: "paused" }), aTaskEntry({ id: "T-4", status: "draft" })],
      ruT,
    ).map((group) => group.title);
    expect(titles).toEqual([ru.tasks.group.waiting, ru.tasks.status.draft, ru.tasks.group.closed, ru.tasks.group.other]);
  });
});

// docs/features/design-sweep-b.md "Acceptance criteria", DB5: a task's state in its head as a lozenge,
// the badge's label and icon, each state its appearance; an unknown one neutral, raw, English.
describe("the state's lozenge (DB5 of design-sweep-b)", () => {
  it("maps the ten states onto their appearances, with the badge's labels and icons", () => {
    expect(Object.keys(STATES).map((status) => [status, taskStatusLozenge(status, enT).appearance])).toEqual([
      ["draft", "neutral"],
      ["analysis", "neutral"],
      ["review", "warning"],
      ["changes_requested", "warning"],
      ["ready", "information"],
      ["in_progress", "information"],
      ["in_review", "information"],
      ["done", "success"],
      ["accepted", "success"],
      ["cancelled", "neutral"],
    ]);
    for (const status of Object.keys(STATES)) {
      const look = taskStatusLook(status, enT);
      expect([status, taskStatusLozenge(status, enT).label, taskStatusLozenge(status, enT).icon]).toEqual([status, look.label, look.icon]);
    }
  });

  it("shows an unknown state raw and neutral under lang=en, never mapped", () => {
    expect(taskStatusLozenge("weird", enT)).toEqual({ appearance: "neutral", icon: "unknown", label: "weird", lang: "en" });
    expect(taskStatusLozenge("toString", enT).appearance).toBe("neutral");
  });
});
