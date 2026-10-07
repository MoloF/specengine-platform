import { afterEach, describe, expect, it, vi } from "vitest";
import { endOwnWrite, isOwnStageEvent, outsideSince, startOwnWrite } from "./ownStages";
import { createQueryClient } from "./provider";

// AC-14 of docs/features/decision-staging.md, the outside-change alert: a stage event is this tab's
// own when one of its writes accounts for it, each write for one event, a refused one for none.

const REJECT = { decision: "reject", reason: "dup" } as const;
const APPROVE = { decision: "approve", option: 1, answer: null, canon: null, note: "n" } as const;

afterEach(() => {
  vi.useRealTimers();
});

describe("this tab's own stage writes", () => {
  it("count a stage's event once, by its choice, whatever its span_hash; before or after the answer", () => {
    const queryClient = createQueryClient();
    const write = startOwnWrite(queryClient, "alpha", "PR-0004", APPROVE);
    const stored = { ...APPROVE, span_hash: "b3:00" };
    expect(isOwnStageEvent(queryClient, "alpha", "PR-0004", false, stored)).toBe(true);
    endOwnWrite(queryClient, "alpha", "PR-0004", write, false);
    expect(isOwnStageEvent(queryClient, "alpha", "PR-0004", false, stored)).toBe(false);
    const second = startOwnWrite(queryClient, "alpha", "PR-0004", REJECT);
    endOwnWrite(queryClient, "alpha", "PR-0004", second, false);
    expect(isOwnStageEvent(queryClient, "alpha", "PR-0004", false, REJECT)).toBe(true);
  });

  it("tell another choice, another proposal, another project and another page's cache apart", () => {
    const queryClient = createQueryClient();
    startOwnWrite(queryClient, "alpha", "PR-0004", APPROVE);
    expect(isOwnStageEvent(queryClient, "alpha", "PR-0004", false, { ...APPROVE, note: "other", span_hash: null })).toBe(false);
    expect(isOwnStageEvent(queryClient, "alpha", "PR-0004", false, REJECT)).toBe(false);
    expect(isOwnStageEvent(queryClient, "alpha", "PR-0005", false, { ...APPROVE, span_hash: null })).toBe(false);
    expect(isOwnStageEvent(queryClient, "beta", "PR-0004", false, { ...APPROVE, span_hash: null })).toBe(false);
    expect(isOwnStageEvent(createQueryClient(), "alpha", "PR-0004", false, { ...APPROVE, span_hash: null })).toBe(false);
    expect(isOwnStageEvent(queryClient, "alpha", "PR-0004", true, undefined)).toBe(false);
    expect(isOwnStageEvent(queryClient, "alpha", "PR-0004", false, "not a stage")).toBe(false);
    expect(isOwnStageEvent(queryClient, "alpha", "PR-0004", false, { ...APPROVE, span_hash: null })).toBe(true);
  });

  it("count an unstage's event once; a refused write none; one with no answer still counts", () => {
    const queryClient = createQueryClient();
    const refused = startOwnWrite(queryClient, "alpha", "PR-0004", null);
    endOwnWrite(queryClient, "alpha", "PR-0004", refused, true);
    expect(isOwnStageEvent(queryClient, "alpha", "PR-0004", true, undefined)).toBe(false);
    const unanswered = startOwnWrite(queryClient, "alpha", "PR-0004", null);
    endOwnWrite(queryClient, "alpha", "PR-0004", unanswered, false);
    expect(isOwnStageEvent(queryClient, "alpha", "PR-0004", true, undefined)).toBe(true);
    expect(isOwnStageEvent(queryClient, "alpha", "PR-0004", true, undefined)).toBe(false);
  });

  it("note an event no write accounts for as an outside change, after which an older read no longer says what is staged", () => {
    vi.useFakeTimers();
    vi.setSystemTime(1_000);
    const queryClient = createQueryClient();
    expect(outsideSince(queryClient, "alpha", "PR-0004", 0)).toBe(false);
    const write = startOwnWrite(queryClient, "alpha", "PR-0004", REJECT);
    vi.setSystemTime(2_000);
    expect(isOwnStageEvent(queryClient, "alpha", "PR-0004", false, REJECT)).toBe(true);
    // This tab's own event is no outside change.
    expect(outsideSince(queryClient, "alpha", "PR-0004", 0)).toBe(false);
    endOwnWrite(queryClient, "alpha", "PR-0004", write, false);
    vi.setSystemTime(3_000);
    expect(isOwnStageEvent(queryClient, "alpha", "PR-0004", true, undefined)).toBe(false);
    expect([2_999, 3_000].map((since) => outsideSince(queryClient, "alpha", "PR-0004", since))).toEqual([true, false]);
    expect(outsideSince(queryClient, "alpha", "PR-0005", 0)).toBe(false);
    expect(outsideSince(queryClient, "beta", "PR-0004", 0)).toBe(false);
    expect(outsideSince(createQueryClient(), "alpha", "PR-0004", 0)).toBe(false);
  });

  it("forget an answered write a minute on: an unstage with nothing staged has no event to wait for", () => {
    vi.useFakeTimers();
    const queryClient = createQueryClient();
    const write = startOwnWrite(queryClient, "alpha", "PR-0004", null);
    endOwnWrite(queryClient, "alpha", "PR-0004", write, false);
    vi.advanceTimersByTime(60_000);
    const kept = startOwnWrite(queryClient, "alpha", "PR-0005", null);
    endOwnWrite(queryClient, "alpha", "PR-0005", kept, false);
    vi.advanceTimersByTime(1);
    expect(isOwnStageEvent(queryClient, "alpha", "PR-0004", true, undefined)).toBe(false);
    expect(isOwnStageEvent(queryClient, "alpha", "PR-0005", true, undefined)).toBe(true);
  });
});
