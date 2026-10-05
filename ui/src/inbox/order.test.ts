import { describe, expect, it } from "vitest";
import { aProposal } from "../test/builders";
import { queueOrder } from "./order";

describe("queueOrder (AC-07)", () => {
  it("sorts by severity, then oldest first, then ID; null and unknown after low", () => {
    const ordered = queueOrder([
      aProposal({ id: "PR-10", severity: "urgent", created_at: "2026-01-01T00:00:00Z" }),
      aProposal({ id: "PR-9", severity: "low", created_at: "2026-10-01T00:00:00Z" }),
      aProposal({ id: "PR-8", severity: null, created_at: "2026-02-01T00:00:00Z" }),
      aProposal({ id: "PR-7", severity: "normal", created_at: "2026-10-02T00:00:00Z" }),
      aProposal({ id: "PR-6", severity: "normal", created_at: "2026-10-01T00:00:00Z" }),
      aProposal({ id: "PR-5", severity: "high", created_at: "2026-10-03T00:00:00Z" }),
      aProposal({ id: "PR-11", severity: "high", created_at: "2026-10-03T00:00:00Z" }),
      aProposal({ id: "PR-2", severity: "high", created_at: "2026-10-03T00:00:00Z" }),
    ]);
    expect(ordered.map((proposal) => proposal.id)).toEqual([
      "PR-2",
      "PR-5",
      "PR-11",
      "PR-6",
      "PR-7",
      "PR-9",
      "PR-10",
      "PR-8",
    ]);
  });

  it("leaves its input untouched", () => {
    const input = [aProposal({ id: "PR-2", severity: "low" }), aProposal({ id: "PR-1", severity: "high" })];
    queueOrder(input);
    expect(input.map((proposal) => proposal.id)).toEqual(["PR-2", "PR-1"]);
  });
});
