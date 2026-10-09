import { afterEach, describe, expect, it, vi } from "vitest";
import { ClientError } from "../api/client";
import type { RoundAnswerBody, RoundSheet, StageBody } from "../api/types";
import { MockClient } from "./MockClient";
import { SLOW_MS } from "./scenario";

// U-11 of docs/features/ui-round.md: the mock serves both round routes as the daemon does, in each
// scenario: normal (harbor-sim's open questions and discrepancies in round order, the rest noted;
// answers staged or refused alone), empty, error (503), slow (1.5 s), conflict (the first POST
// refuses one answer as changed since read; the next, on the new sheet, takes it).

const NOW = Date.parse("2026-10-05T12:00:00Z");
const clock = () => NOW;
const AT = "2026-10-05T12:00:00Z";

async function rejection(promise: Promise<unknown>): Promise<ClientError> {
  try {
    await promise;
  } catch (error) {
    if (error instanceof ClientError) {
      return error;
    }
    throw error;
  }
  throw new Error("the call resolved");
}

function readAt(sheet: RoundSheet, id: string): string {
  return sheet.items.find((item) => item.id === id)?.updated_at ?? "not on the sheet";
}

const working = (updatedAt: string): StageBody => ({ decision: "approve", option: null, answer: null, canon: null, note: null, updated_at: updatedAt });
const option = (index: number, updatedAt: string): StageBody => ({ decision: "approve", option: index, answer: null, canon: null, note: null, updated_at: updatedAt });
const own = (text: string, updatedAt: string): StageBody => ({ decision: "approve", option: null, answer: text, canon: null, note: null, updated_at: updatedAt });
const reject = (reason: string, updatedAt: string): StageBody => ({ decision: "reject", reason, updated_at: updatedAt });

afterEach(() => {
  vi.useRealTimers();
});

describe("the normal scenario", () => {
  it("lists harbor-sim's open questions, discrepancies and decisions in round order, notes the change proposals (an interpretation, a decision's update and an amendment among them), lists no deferred one", async () => {
    const sheet = await new MockClient("normal", { now: clock }).getRound("harbor-sim");
    expect(sheet.project).toBe("harbor-sim");
    expect(sheet.items.map((item) => item.id)).toEqual(["PR-0041", "PR-0048", "PR-0050", "PR-0043", "PR-0047", "PR-0044"]);
    expect(sheet.items.map((item) => item.tasks.map((task) => task.id))).toEqual([["T-0107"], [], [], ["T-0112"], [], ["T-0107", "T-0112"]]);
    expect(sheet.more).toBe(0);
    expect(sheet.minutes).toBeGreaterThan(0);
    expect(sheet.notes).toEqual(["4 open change proposal(s) not on this sheet: PR-0042, PR-0049, PR-0051 and PR-0052; spec inbox lists them"]);
    expect(Object.keys(sheet.items[0] ?? {})).toEqual([
      "id",
      "kind",
      "severity",
      "gap_type",
      "summary",
      "working_answer",
      "price_of_other",
      "options",
      "recommendation",
      "evidence",
      "targets",
      "tasks",
      "worktree",
      "branch",
      "author",
      "staged",
      "staged_at",
      "created_at",
      "updated_at",
    ]);
    // A path target: no ID, its document's path and title.
    expect(sheet.items.find((item) => item.id === "PR-0047")?.targets).toEqual([{ id: null, path: "docs/spec/berths/draft-limits.md", title: "Berth limits" }]);
  });

  it("holds the structures the screen offers: options with a recommendation, and a question without options", async () => {
    const sheet = await new MockClient("normal", { now: clock }).getRound("harbor-sim");
    const crane = sheet.items.find((item) => item.id === "PR-0048");
    expect([crane?.options.length, crane?.working_answer !== null, crane?.price_of_other !== null]).toEqual([0, true, true]);
    expect(sheet.items.some((item) => item.options.length > 0 && item.recommendation !== null)).toBe(true);
  });

  it("takes a limit of 1 to 200, the rest noted first; any other refused as the daemon refuses it", async () => {
    const client = new MockClient("normal", { now: clock });
    const two = await client.getRound("harbor-sim", { limit: 2 });
    expect([two.items.map((item) => item.id), two.more, two.notes[0]]).toEqual([
      ["PR-0041", "PR-0048"],
      4,
      "4 more item(s) not on this sheet: spec round new --limit 6",
    ]);
    for (const limit of [0, 201, 1.5]) {
      const error = await rejection(client.getRound("harbor-sim", { limit }));
      expect([error.status, error.message]).toEqual([400, `--limit ${String(limit)}: the limit is 1 to 200`]);
    }
  });

  it("stages each answer refused alone: the rest staged at one clock reading, in request order, the sheet showing them", async () => {
    const client = new MockClient("normal", { now: clock });
    const sheet = await client.getRound("harbor-sim");
    const answers: RoundAnswerBody[] = [
      { id: "PR-0048", stage: own("Two shifts,\nfrom 2027", readAt(sheet, "PR-0048")) },
      { id: "PR-0041", stage: option(1, readAt(sheet, "PR-0041")) },
      { id: "PR-0043", stage: reject("Keep the English name", readAt(sheet, "PR-0043")) },
      { id: "PR-0044", stage: option(7, readAt(sheet, "PR-0044")) },
      { id: "PR-0047", stage: working("2026-01-01T00:00:00Z") },
      { id: "PR-0042", stage: working(AT) },
      { id: "PR-0049", stage: working(AT) },
      { id: "PR-0046", stage: option(0, AT) },
      { id: "PR-9999", stage: working(AT) },
    ];
    const document = await client.stageRoundAnswers("harbor-sim", answers);
    expect(document.answers.map(({ id, outcome, staged_at, reason }) => [id, outcome, staged_at, reason])).toEqual([
      ["PR-0048", "staged", AT, null],
      ["PR-0041", "staged", AT, null],
      ["PR-0043", "staged", AT, null],
      ["PR-0044", "refused", null, "option 7: `PR-0044` has options 0-1"],
      ["PR-0047", "refused", null, `\`PR-0047\` changed since it was read: updated at ${readAt(sheet, "PR-0047")}, the choice was made on the one of 2026-01-01T00:00:00Z; read it again`],
      ["PR-0042", "refused", null, "`PR-0042` is a change proposal (update): `spec approve PR-0042` decides it"],
      ["PR-0049", "refused", null, "`PR-0049` is a change proposal (interpretation): `spec approve PR-0049` decides it"],
      ["PR-0046", "refused", null, "`PR-0046` is deferred"],
      ["PR-9999", "refused", null, "no proposal `PR-9999` in this project's queue"],
    ]);
    expect(document.answers[0]?.stage).toEqual({ decision: "approve", option: null, answer: "Two shifts,\nfrom 2027", canon: null, note: null, span_hash: null });
    const after = await client.getRound("harbor-sim");
    expect(after.items.filter((item) => item.staged !== null).map((item) => [item.id, item.staged_at, item.updated_at])).toEqual([
      ["PR-0041", AT, AT],
      ["PR-0048", AT, AT],
      ["PR-0043", AT, AT],
    ]);
    const again = await client.stageRoundAnswers("harbor-sim", [{ id: "PR-0041", stage: option(0, AT) }]);
    expect(again.answers[0]?.replaced).toBe(AT);
  });

  it("refuses an ID answered twice at each position, an own answer on an item with options, a text over its cap, U+202E", async () => {
    const client = new MockClient("normal", { now: clock });
    const sheet = await client.getRound("harbor-sim");
    const at = (id: string) => readAt(sheet, id);
    const document = await client.stageRoundAnswers("harbor-sim", [
      { id: "PR-0043", stage: reject("one", at("PR-0043")) },
      { id: "PR-0048", stage: own("x".repeat(2049), at("PR-0048")) },
      { id: "PR-0043", stage: reject("two", at("PR-0043")) },
      { id: "PR-0041", stage: own("my own", at("PR-0041")) },
      { id: "PR-0044", stage: reject(`a ${String.fromCodePoint(0x202e)} b`, at("PR-0044")) },
      { id: "PR-0047", stage: reject("  ", at("PR-0047")) },
    ]);
    expect(document.answers.map(({ outcome, reason }) => [outcome, reason])).toEqual([
      ["refused", "answered at positions 1 and 3: give one answer per item"],
      ["refused", "the answer: 2049 bytes; at most 2048"],
      ["refused", "answered at positions 1 and 3: give one answer per item"],
      ["refused", "`PR-0041` takes one of its options by number, 0-1, or a reject with its reason"],
      ["refused", "the reason holds U+202E: a reason never carries it"],
      ["refused", "`PR-0047` needs a reason: the reason is blank"],
    ]);
    expect((await client.getRound("harbor-sim")).items.every((item) => item.staged === null)).toBe(true);
  });

  it("refuses over 200 answers whole: 400, nothing staged", async () => {
    const client = new MockClient("normal", { now: clock });
    const answers = Array.from({ length: 201 }, (_, index): RoundAnswerBody => ({ id: `PR-${String(9000 + index)}`, stage: working(AT) }));
    expect((await rejection(client.stageRoundAnswers("harbor-sim", answers))).status).toBe(400);
  });
});

describe("the other scenarios", () => {
  it("empty: no items, no notes, no reading time", async () => {
    const sheet = await new MockClient("empty", { now: clock }).getRound("harbor-sim");
    expect(sheet).toEqual({ project: "harbor-sim", items: [], more: 0, minutes: 0, notes: [] });
  });

  it("error: the sheet's read rejects with 503 and a message", async () => {
    const error = await rejection(new MockClient("error", { now: clock }).getRound("harbor-sim"));
    expect(error.status).toBe(503);
    expect(error.message.length).toBeGreaterThan(0);
  });

  it("slow: the sheet takes 1.5 s, the POST too", async () => {
    vi.useFakeTimers();
    const client = new MockClient("slow", { now: clock });
    let read = false;
    const pending = client.getRound("harbor-sim").then((sheet) => {
      read = true;
      return sheet;
    });
    await vi.advanceTimersByTimeAsync(SLOW_MS - 1);
    expect(read).toBe(false);
    await vi.advanceTimersByTimeAsync(1);
    expect(read).toBe(true);
    const sheet = await pending;
    let staged = false;
    const post = client.stageRoundAnswers("harbor-sim", [{ id: "PR-0048", stage: working(readAt(sheet, "PR-0048")) }]).then((document) => {
      staged = true;
      return document;
    });
    await vi.advanceTimersByTimeAsync(SLOW_MS - 1);
    expect(staged).toBe(false);
    await vi.advanceTimersByTimeAsync(1);
    expect((await post).answers[0]?.outcome).toBe("staged");
  });

  it("conflict: the first POST refuses its first answer as changed since read, staging the rest; the next, on the new sheet, takes it", async () => {
    const client = new MockClient("conflict", { now: clock });
    const sheet = await client.getRound("harbor-sim");
    const first = await client.stageRoundAnswers("harbor-sim", [
      { id: "PR-0041", stage: option(0, readAt(sheet, "PR-0041")) },
      { id: "PR-0048", stage: working(readAt(sheet, "PR-0048")) },
    ]);
    expect(first.answers.map(({ id, outcome }) => [id, outcome])).toEqual([
      ["PR-0041", "refused"],
      ["PR-0048", "staged"],
    ]);
    expect(first.answers[0]?.reason).toMatch(/^`PR-0041` changed since it was read: updated at 2026-10-05T12:00:00Z, the choice was made on the one of /);
    const now = await client.getRound("harbor-sim");
    const changed = now.items.find((item) => item.id === "PR-0041");
    expect([changed?.staged?.decision, changed?.updated_at]).toEqual(["reject", AT]);
    const second = await client.stageRoundAnswers("harbor-sim", [{ id: "PR-0041", stage: option(0, readAt(now, "PR-0041")) }]);
    expect(second.answers.map(({ outcome, replaced }) => [outcome, replaced])).toEqual([["staged", AT]]);
  });
});
