import { describe, expect, it } from "vitest";
import type { TraceCounts } from "../api/types";
import { aFlowNode, aFlowView } from "../test/builders";
import { confirmedLine, countParts, countsText, isNoCard, rollupLook, stageWords } from "./labels";
import { laneTotal, stageAsked, stageKey, withoutStates } from "./timeline";
import { enT } from "../test/text";

// docs/features/trace-views.md "Timeline", "Stage filter", "Flow badges": what is arranged of the
// served values, and nothing counted but what the lane's own bar is told.

const NONE: TraceCounts = { no_card: 0, in_work: 0, suspect: 0, departed: 0, confirmed: 0 };

describe("a rollup's word", () => {
  it("gives each state a label and an icon of its own, never by colour alone", () => {
    const looks = ["confirmed", "in work", "suspect", "departed", "no card"].map((rollup) => rollupLook(rollup, enT));
    expect(looks.map((look) => look.label)).toEqual(["Confirmed", "In work", "Suspect", "Departed", "No card"]);
    expect(new Set(looks.map((look) => look.icon)).size).toBe(5);
    expect(new Set(looks.map((look) => look.tone)).size).toBe(5);
  });

  it("shows a word this build does not know raw and neutral", () => {
    expect(rollupLook("pondering", enT)).toEqual({ label: "pondering", tone: "trace-unknown", icon: "unknown", served: true });
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
});

describe("the lanes", () => {
  it("ask a stage as served, in any script, and `-` for none", () => {
    expect(stageAsked("\u043f\u0435\u0447\u0430\u0442\u044c")).toBe("\u043f\u0435\u0447\u0430\u0442\u044c");
    expect(stageAsked(null)).toBe("-");
    expect(stageWords(null, enT)).toBe("No stage");
  });

  it("key a lane by its served stage: two stages two keys, none its own, a project's word `none` apart from it", () => {
    expect(new Set([stageKey("gather"), stageKey("print"), stageKey(null), stageKey("none")]).size).toBe(4);
  });
});

describe("a read that failed again", () => {
  it("keeps its boxes and says no state and no gap", () => {
    const view = aFlowView([aFlowNode({ id: "A", distance: 0, rollup: "confirmed", counts: { ...NONE, confirmed: 1 }, holds: 2, gaps: 3 })], []);
    expect(withoutStates(view).nodes[0]).toMatchObject({ id: "A", distance: 0, holds: 2, rollup: null, counts: null, gaps: 0 });
    expect(view.nodes[0]?.rollup).toBe("confirmed");
  });
});
