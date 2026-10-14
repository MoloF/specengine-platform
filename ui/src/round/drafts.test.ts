import { describe, expect, it } from "vitest";
import { aRoundItem } from "../test/builders";
import { answerOf, draftOf, offers, stateOf, withDraft, type Draft } from "./drafts";

// U-03, U-04 and U-05 of docs/features/ui-round.md, the drafts: a pick becomes the decision route's
// body (W-1), keys in the stage's order then `updated_at`; nothing is picked by default; a draft
// made on an older version is left out; an own answer or a reason with nothing written is left out;
// a pick the revised item no longer offers is never sent, whatever version it carries.

const OPTIONS = [
  { label: "Code to spec", effect: "Fix the code", price: "One test" },
  { label: "Spec to code", effect: "Edit the rule", price: "A note" },
];

function draft(fields: Partial<Draft>): Draft {
  return { pick: "open", option: null, answer: "", reason: "", updatedAt: "2026-10-01T10:00:00Z", ...fields };
}

describe("a draft's answer (U-04)", () => {
  it("sends an option as option N, every other flag null, then the updated_at shown, in this key order", () => {
    const item = aRoundItem({ id: "PR-0005", kind: "riddle", options: OPTIONS, recommendation: 1 });
    expect(JSON.stringify(answerOf(item, draft({ pick: "option", option: 1 })))).toBe(
      '{"id":"PR-0005","stage":{"decision":"approve","option":1,"answer":null,"canon":null,"editorial":null,"note":null,"updated_at":"2026-10-01T10:00:00Z"}}',
    );
  });

  it("sends the working answer as an approve with every flag null, an own answer as answer (LF kept, trimmed), a reject with its reason", () => {
    const item = aRoundItem({ id: "PR-0003", working_answer: "yes, 1.5 s" });
    expect(answerOf(item, draft({ pick: "working" }))?.stage).toEqual({
      decision: "approve",
      option: null,
      answer: null,
      canon: null,
      editorial: null,
      note: null,
      updated_at: "2026-10-01T10:00:00Z",
    });
    expect(answerOf(item, draft({ pick: "answer", answer: "  no,\n2 s \n" }))?.stage).toEqual({
      decision: "approve",
      option: null,
      answer: "no,\n2 s",
      canon: null,
      editorial: null,
      note: null,
      updated_at: "2026-10-01T10:00:00Z",
    });
    expect(JSON.stringify(answerOf(item, draft({ pick: "reject", reason: "dup" })))).toBe(
      '{"id":"PR-0003","stage":{"decision":"reject","reason":"dup","updated_at":"2026-10-01T10:00:00Z"}}',
    );
  });

  it("sends a text over its cap all the same: the daemon's refusal decides, nothing is cut", () => {
    const item = aRoundItem({ id: "PR-0003" });
    const long = "x".repeat(5000);
    expect(answerOf(item, draft({ pick: "answer", answer: long }))?.stage).toMatchObject({ answer: long });
  });
});

describe("where a draft stands (U-03, U-05)", () => {
  it("starts left open: nothing picked, the recommendation included, nothing sent", () => {
    const item = aRoundItem({ id: "PR-0005", options: OPTIONS, recommendation: 0 });
    expect(draftOf(item)).toEqual(draft({}));
    expect([stateOf(item, undefined), stateOf(item, draftOf(item))]).toEqual(["open", "open"]);
    expect(answerOf(item, undefined)).toBeNull();
    expect(answerOf(item, draftOf(item))).toBeNull();
  });

  it("leaves out a draft made on an older version of its item until picked again", () => {
    const item = aRoundItem({ id: "PR-0005", options: OPTIONS, updated_at: "2026-10-02T08:00:00Z" });
    const old = draft({ pick: "option", option: 0 });
    expect(stateOf(item, old)).toBe("changed");
    expect(answerOf(item, old)).toBeNull();
    const again = { ...old, updatedAt: item.updated_at };
    expect(stateOf(item, again)).toBe("ready");
    expect(answerOf(item, again)?.stage.updated_at).toBe("2026-10-02T08:00:00Z");
  });

  it("leaves out an own answer or a reject with nothing but white space written", () => {
    const item = aRoundItem({ id: "PR-0003" });
    expect(stateOf(item, draft({ pick: "answer", answer: " \n " }))).toBe("unwritten");
    expect(stateOf(item, draft({ pick: "reject", reason: "" }))).toBe("unwritten");
    expect(answerOf(item, draft({ pick: "reject", reason: "" }))).toBeNull();
  });

  it("never sends a pick the item shown no longer offers, re-stamped or not: open on its version, changed on an older one", () => {
    const withOptions = aRoundItem({ id: "PR-0005", options: OPTIONS, working_answer: "w", updated_at: "2026-10-02T08:00:00Z" });
    const withoutOptions = aRoundItem({ id: "PR-0005", working_answer: null, updated_at: "2026-10-02T08:00:00Z" });
    const cases: [ReturnType<typeof aRoundItem>, Partial<Draft>][] = [
      [withOptions, { pick: "option", option: 2 }],
      [withOptions, { pick: "option", option: -1 }],
      [withOptions, { pick: "answer", answer: "mine" }],
      [withOptions, { pick: "working" }],
      [withoutOptions, { pick: "option", option: 0 }],
      [withoutOptions, { pick: "working" }],
    ];
    for (const [item, fields] of cases) {
      const old = draft(fields);
      const kept = draft({ ...fields, updatedAt: item.updated_at });
      expect([fields, offers(item, kept), stateOf(item, old), stateOf(item, kept), answerOf(item, old), answerOf(item, kept)]).toEqual([
        fields,
        false,
        "changed",
        "open",
        null,
        null,
      ]);
    }
    expect(offers(withOptions, draft({ pick: "option", option: 1 }))).toBe(true);
    expect(offers(withoutOptions, draft({ pick: "answer", answer: "mine" }))).toBe(true);
  });

  it("keys drafts by ID: replacing one keeps the others, removing one drops only it", () => {
    const first = withDraft(new Map(), "PR-0003", draft({ pick: "working" }));
    const both = withDraft(first, "PR-0005", draft({ pick: "reject", reason: "dup" }));
    expect([...both.keys()]).toEqual(["PR-0003", "PR-0005"]);
    expect([...withDraft(both, "PR-0003", undefined).keys()]).toEqual(["PR-0005"]);
    expect([...first.keys()]).toEqual(["PR-0003"]);
  });
});
