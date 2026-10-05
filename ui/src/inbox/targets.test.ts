import { describe, expect, it } from "vitest";
import { aProposal } from "../test/builders";
import { sharesTarget, targetsOf } from "./targets";

describe("targets", () => {
  it("reads target_ids, else the one target_id, else none", () => {
    expect(targetsOf(aProposal({ id: "PR-1", target_ids: ["R-1", "R-2"], target_id: "R-9" }))).toEqual(["R-1", "R-2"]);
    expect(targetsOf(aProposal({ id: "PR-2", target_id: "R-9" }))).toEqual(["R-9"]);
    expect(targetsOf(aProposal({ id: "PR-3" }))).toEqual([]);
  });

  it("finds a shared node whichever field names it, never a proposal with itself", () => {
    const many = aProposal({ id: "PR-1", target_ids: ["R-1", "R-2"] });
    const single = aProposal({ id: "PR-2", target_id: "R-2" });
    const elsewhere = aProposal({ id: "PR-3", target_id: "R-7" });
    const none = aProposal({ id: "PR-4" });
    expect(sharesTarget(many, single)).toBe(true);
    expect(sharesTarget(single, many)).toBe(true);
    expect(sharesTarget(single, aProposal({ id: "PR-5", target_id: "R-2" }))).toBe(true);
    expect(sharesTarget(many, elsewhere)).toBe(false);
    expect(sharesTarget(none, none)).toBe(false);
    expect(sharesTarget(many, many)).toBe(false);
  });
});
