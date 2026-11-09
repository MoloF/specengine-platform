import { describe, expect, it } from "vitest";
import type { TraceCounts, TraceStage } from "../api/types";
import { aFlowNode, aFlowView, aTraceNode } from "../test/builders";
import { confirmedLine, countParts, countsText, isNoCard, rollupLook, stageWords, stepWords } from "./labels";
import { ALL_STAGES, inStage, keptInList, laneTotal, noCardAnywhere, noStageAnywhere, outsideStage, stageAsked, stageInForce, stageKey, stageOptions, stepsInOrder, withoutStates } from "./timeline";
import { enT } from "../test/text";

// docs/features/trace-views.md "Timeline", "Stage filter", "Flow badges": what is arranged of the
// served values, and nothing counted but what the lane's own bar is told.

const NONE: TraceCounts = { no_card: 0, in_work: 0, suspect: 0, departed: 0, confirmed: 0 };

function lane(stage: string | null, counts: Partial<TraceCounts> = {}, distance: number | null = 0): TraceStage {
  return { stage, distance, counts: { ...NONE, ...counts } };
}

describe("a rollup's word", () => {
  it("gives each state a label and an icon of its own, never by colour alone", () => {
    const looks = ["confirmed", "in work", "suspect", "departed", "no card"].map((rollup) => rollupLook(rollup, enT));
    expect(looks.map((look) => look.label)).toEqual(["Confirmed", "In work", "Suspect", "Departed", "No card"]);
    expect(new Set(looks.map((look) => look.icon)).size).toBe(5);
    expect(new Set(looks.map((look) => look.tone)).size).toBe(5);
  });

  it("shows a word this build does not know raw and neutral", () => {
    expect(rollupLook("pondering", enT)).toEqual({ label: "pondering", tone: "trace-unknown", icon: "unknown" });
  });

  it("knows a step with no card, and nothing else, as hollow", () => {
    expect(["no card", "confirmed", "pondering"].map(isNoCard)).toEqual([true, false, false]);
  });
});

describe("served counts", () => {
  it("say a closed group's line, zero parts left out, from what is done to what is missing", () => {
    expect(countsText({ ...NONE, confirmed: 3, suspect: 1, no_card: 2 }, enT)).toBe("3 confirmed, 1 suspect, 2 no card");
    expect(countsText({ ...NONE, confirmed: 7 }, enT)).toBe("7 confirmed");
    expect(countsText(NONE, enT)).toBe("");
  });

  it("mark the no-card part alone as hollow", () => {
    expect(countParts({ ...NONE, confirmed: 1, no_card: 1 }, enT).map((part) => [part.key, part.hollow])).toEqual([
      ["confirmed", false],
      ["no_card", true],
    ]);
  });

  it("score a lane out of all its parts", () => {
    const counts = { ...NONE, confirmed: 2, in_work: 1, no_card: 3 };
    expect(confirmedLine(counts.confirmed, laneTotal(counts), enT)).toBe("2 of 6 confirmed");
  });

  it("tell when no card holds anything: no_card alone may hold steps", () => {
    expect(noCardAnywhere({ ...NONE, no_card: 4 })).toBe(true);
    expect(noCardAnywhere({ ...NONE, no_card: 4, departed: 1 })).toBe(false);
  });
});

describe("the lanes", () => {
  it("ask a stage as served, in any script, and `-` for none", () => {
    expect(stageAsked("\u043f\u0435\u0447\u0430\u0442\u044c")).toBe("\u043f\u0435\u0447\u0430\u0442\u044c");
    expect(stageAsked(null)).toBe("-");
    expect(stageWords(null, enT)).toBe("No stage");
  });

  it("tell no `stage:` anywhere: no lane, or only the lane of none", () => {
    expect(noStageAnywhere([lane(null)])).toBe(true);
    expect(noStageAnywhere([])).toBe(true);
    expect(noStageAnywhere([lane("print")])).toBe(false);
    expect(noStageAnywhere([lane("print"), lane(null)])).toBe(false);
  });

  it("say where a step stands counted from one, never its distance", () => {
    expect([0, 3, null].map((distance) => stepWords(distance, enT))).toEqual(["Step 1", "Step 4", "Step unknown"]);
  });

  it("order a lane's steps by distance, those off the walk last, the same whatever order they came in", () => {
    const rows = [aTraceNode("D", [], { distance: null }), aTraceNode("C", [], { distance: 2 }), aTraceNode("A", [], { distance: 1 }), aTraceNode("B", [], { distance: 1 }), aTraceNode("E", [], { distance: null })];
    const names = (nodes: typeof rows) => stepsInOrder(nodes).map((node) => node.id);
    expect(names(rows)).toEqual(["A", "B", "C", "D", "E"]);
    expect(names([...rows].reverse())).toEqual(["A", "B", "C", "D", "E"]);
  });
});

describe("the stage filter", () => {
  const stages = [lane("gather"), lane("print"), lane(null, {}, null)];

  it("offers All, then the lanes as served, No stage among them", () => {
    expect(stageOptions(stages, enT).map((option) => option.label)).toEqual(["All stages", "gather", "print", "No stage"]);
  });

  it("compares nothing but the served value, and all stands for everything", () => {
    expect(inStage("gather", stageKey("gather"))).toBe(true);
    expect(inStage("Gather", stageKey("gather"))).toBe(false);
    expect(inStage(null, stageKey(null))).toBe(true);
    expect(inStage("gather", stageKey(null))).toBe(false);
    expect(inStage("gather", ALL_STAGES)).toBe(true);
    expect(inStage(null, ALL_STAGES)).toBe(true);
  });

  it("judges a step by its own stage, and a group only if it has one: what a group with none holds may stand in any stage", () => {
    const print = stageKey("print");
    expect(outsideStage({ stage: "gather", holds: 0 }, print)).toBe(true);
    expect(outsideStage({ stage: null, holds: 0 }, print)).toBe(true);
    expect(outsideStage({ stage: "print", holds: 0 }, print)).toBe(false);
    expect(outsideStage({ stage: null, holds: 2 }, print)).toBe(false);
    expect(outsideStage({ stage: null, holds: 2 }, stageKey(null))).toBe(false);
    expect(outsideStage({ stage: "gather", holds: 2 }, print)).toBe(true);
    expect(outsideStage({ stage: "gather", holds: 0 }, ALL_STAGES)).toBe(false);
  });

  it("keeps every group in the List under any filter, its own stage or none: its Open is the way to what it holds", () => {
    const print = stageKey("print");
    expect(keptInList({ stage: null, holds: 2 }, print)).toBe(true);
    expect(keptInList({ stage: "gather", holds: 2 }, print)).toBe(true);
    expect(keptInList({ stage: null, holds: 0 }, print)).toBe(false);
    expect(keptInList({ stage: "print", holds: 0 }, print)).toBe(true);
  });

  it("keeps a word of a project's from meaning all or none", () => {
    expect(inStage("all", stageKey("none"))).toBe(false);
    expect(stageKey("all")).not.toBe(ALL_STAGES);
    expect(stageKey("none")).not.toBe(stageKey(null));
  });

  it("falls back to everything when the stage chosen is no lane now, or the lanes are not read yet", () => {
    expect(stageInForce(stageKey("print"), stages)).toBe(stageKey("print"));
    expect(stageInForce(stageKey("gone"), stages)).toBe(ALL_STAGES);
    expect(stageInForce(stageKey("print"), [])).toBe(ALL_STAGES);
  });
});

describe("a read that failed again", () => {
  it("keeps its boxes and says no state and no gap", () => {
    const view = aFlowView([aFlowNode({ id: "A", distance: 0, rollup: "confirmed", counts: { ...NONE, confirmed: 1 }, holds: 2, gaps: 3 })], []);
    expect(withoutStates(view).nodes[0]).toMatchObject({ id: "A", distance: 0, holds: 2, rollup: null, counts: null, gaps: 0 });
    expect(view.nodes[0]?.rollup).toBe("confirmed");
  });
});
