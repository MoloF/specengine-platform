import { describe, expect, it } from "vitest";
import stagedSource from "./StagedChoice.tsx?raw";
import { confirmCommand, DECISION_KEYS, PROPOSAL_ID, WORDING } from "./decisions";

// AC-14 of docs/features/decision-staging.md: the command a staged choice offers to copy is fixed
// words and an ID matching ^PR-[0-9]{4,}$, never free text; Accept and Reject stage, the other two
// send nothing; the effects say the daemon stages and a terminal confirms, never that it applies.

describe("the confirming command (AC-14 of decision-staging)", () => {
  it.each([
    ["approve", "PR-0004", "spec approve PR-0004"],
    ["reject", "PR-0004", "spec reject PR-0004"],
    ["approve", "PR-123456", "spec approve PR-123456"],
  ])("%s %s: %s", (decision, id, command) => {
    expect(confirmCommand(decision, id)).toBe(command);
  });

  it.each([
    ["approve", "PR-1"],
    ["approve", "PR-0004 --yes"],
    ["approve", "PR-0004; rm -rf ~"],
    ["approve", "PR-0004\n"],
    ["approve", " PR-0004"],
    ["approve", `${String.fromCodePoint(0x420)}R-0004`],
    ["approve", "PR-٠١٢٣"],
    ["approve", "pr-0004"],
    ["defer", "PR-0004"],
    ["approve --option 1", "PR-0004"],
    ["", "PR-0004"],
  ])("offers none for %j on %j", (decision, id) => {
    expect(confirmCommand(decision, id)).toBeNull();
  });

  it("checks the ID with exactly ^PR-[0-9]{4,}$", () => {
    expect(PROPOSAL_ID.source).toBe("^PR-[0-9]{4,}$");
    expect(PROPOSAL_ID.flags).toBe("");
  });

  it("gives Copy the checked command alone on the card, no other text", () => {
    const copies = [...stagedSource.matchAll(/<CopyButton text=\{([^}]+)\}/g)].map((match) => match[1]);
    expect(copies).toEqual(["command"]);
    expect(stagedSource).toContain("const command = confirmCommand(staged.decision, id);");
  });
});

describe("the four decisions' words (AC-14 of decision-staging)", () => {
  it("stages Accept and Reject; Needs clarification and Defer are not sent", () => {
    expect(DECISION_KEYS.map(([kind]) => [kind, WORDING[kind].staged])).toEqual([
      ["accept", true],
      ["reject", true],
      ["needs_clarification", false],
      ["defer", false],
    ]);
  });

  it("says Accept and Reject stage, confirmed on a terminal; none says the daemon applies", () => {
    for (const kind of ["accept", "reject"] as const) {
      expect([kind, WORDING[kind].effect]).toEqual([kind, expect.stringMatching(/^Stages .* Confirm it on a terminal: spec (approve|reject) with this ID /)]);
    }
    expect(Object.values(WORDING).map((wording) => wording.effect).filter((effect) => /daemon applies|apply_proposal/.test(effect))).toEqual([]);
  });
});
