import { describe, expect, it } from "vitest";
import { aProposal } from "../test/builders";
import { countFor, namesOf, proposalsFor, targetIndex } from "./matching";

// AC-09 of docs/features/ui-tree-node.md: a target matches a node by its ID, `<stem>/<ID>` or,
// ID-less, its path; `target_ids` first, else `target_id`; every entry counts.

const NODE = { id: "MEC-TIDES", path: "docs/spec/tides/tide-cycle.md" };
const IDLESS = { id: null, path: "docs/spec/berths/draft-limits.md" };

describe("matching inbox items to a node", () => {
  it("names a node by ID and stem/ID, an ID-less one by path", () => {
    expect(namesOf(NODE)).toEqual(["MEC-TIDES", "tide-cycle/MEC-TIDES"]);
    expect(namesOf(IDLESS)).toEqual(["docs/spec/berths/draft-limits.md"]);
  });

  it("matches by ID, by stem/ID, by a second target_ids entry, by path; nothing else", () => {
    const proposals = [
      aProposal({ id: "PR-1", target_id: "MEC-TIDES" }),
      aProposal({ id: "PR-2", target_ids: ["RULE-X", "MEC-TIDES"] }),
      aProposal({ id: "PR-3", target_ids: ["tide-cycle/MEC-TIDES"] }),
      aProposal({ id: "PR-4", target_ids: ["docs/spec/berths/draft-limits.md"] }),
      aProposal({ id: "PR-5", target_ids: ["docs/spec/tides/tide-cycle.md"] }),
      aProposal({ id: "PR-6", target_id: "MEC-TIDES", target_ids: ["RULE-X"] }),
    ];
    expect(proposalsFor(proposals, [NODE]).map((proposal) => proposal.id)).toEqual(["PR-1", "PR-2", "PR-3"]);
    expect(proposalsFor(proposals, [IDLESS]).map((proposal) => proposal.id)).toEqual(["PR-4"]);
  });

  it("counts a proposal once per node, however many of its targets name it", () => {
    const index = targetIndex([
      aProposal({ id: "PR-1", target_ids: ["MEC-TIDES", "tide-cycle/MEC-TIDES"] }),
      aProposal({ id: "PR-2", target_ids: ["MEC-TIDES"] }),
    ]);
    expect(countFor(index, NODE)).toBe(2);
    expect(countFor(index, IDLESS)).toBe(0);
  });
});
