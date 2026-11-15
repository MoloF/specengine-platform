import { describe, expect, it, vi } from "vitest";
import { ALL_SECTIONS, feedHash, navOf, parseHash } from "../app/routes";
import { enT } from "../test/text";
import { recallSeen, rememberSeen, seenKey } from "./seen";
import { quietBetween, shortRule, subjectOf, typeLabel } from "./words";

// docs/features/auto-feed.md "The UI": Activity at `#/<p>/feed` and `#/<p>/feed/needs-me`, never a tenth
// section; any other segment under it names nothing. The nav lists it second, after Overview, in every
// department (the owner's word of 2026-11-15, over the spec's off-nav).
describe("Activity's routes (auto-feed)", () => {
  it("reads both tabs and writes them back; a third segment, another tab or a page names nothing", () => {
    expect(parseHash("#/alpha/feed")).toEqual({ type: "feed", project: "alpha", tab: "all" });
    expect(parseHash("#/alpha/feed/")).toEqual({ type: "feed", project: "alpha", tab: "all" });
    expect(parseHash("#/alpha/feed/needs-me")).toEqual({ type: "feed", project: "alpha", tab: "needs-me" });
    expect(parseHash("#/alpha/feed?doc=R-1")).toEqual({ type: "feed", project: "alpha", tab: "all" });
    expect(parseHash("#/alpha/feed/other")).toEqual({ type: "not_found" });
    expect(parseHash("#/alpha/feed/needs-me/page")).toEqual({ type: "not_found" });
    expect([feedHash("alpha"), feedHash("alpha", "needs-me"), feedHash("a b")]).toEqual(["#/alpha/feed", "#/alpha/feed/needs-me", "#/a%20b/feed"]);
    expect(ALL_SECTIONS).not.toContain("feed");
    expect(navOf("alpha", [...ALL_SECTIONS, "feed"], enT).map((entry) => entry.key)).toEqual(["overview", "feed", ...ALL_SECTIONS]);
    expect(navOf("alpha", [], enT).map(({ key, label, hash }) => [key, label, hash])).toEqual([
      ["overview", "Overview", "#/alpha"],
      ["feed", "Activity", "#/alpha/feed"],
    ]);
  });
});

// FD5 of docs/features/auto-feed.md "Acceptance criteria": the stored point, per login on a hub; storage
// that refuses reads as none and keeps nothing, never throwing.
describe("the stored point (FD5 of auto-feed)", () => {
  it("is browser-wide locally, the login's on a hub; a whole number or none", () => {
    expect([seenKey("alpha", null), seenKey("alpha", "mira")]).toEqual(["specengine.feed.seen.alpha", "specengine.feed.seen.alpha.mira"]);
    rememberSeen("alpha", "mira", 1207);
    expect([recallSeen("alpha", "mira"), recallSeen("alpha", null)]).toEqual([1207, null]);
    window.localStorage.setItem("specengine.feed.seen.alpha", "12x");
    expect(recallSeen("alpha", null)).toBeNull();
  });

  it("reads none and keeps nothing when storage refuses", () => {
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("denied");
    });
    const write = vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("denied");
    });
    expect(() => {
      rememberSeen("alpha", null, 3);
    }).not.toThrow();
    expect(write).toHaveBeenCalledWith("specengine.feed.seen.alpha", "3");
    expect(recallSeen("alpha", null)).toBeNull();
  });
});

describe("an entry's words (docs/features/auto-feed.md \"The UI\")", () => {
  it("names the subject: the agent task, else the proposal, else the task of the Board, else the line, else none", () => {
    const none = { task: null, proposal: null, card: null, line: null };
    expect(subjectOf({ ...none, task: "T-1", proposal: "PR-1" })).toEqual({ to: "tasks", id: "T-1" });
    expect(subjectOf({ ...none, proposal: "PR-1", card: "C-1" })).toEqual({ to: "inbox", id: "PR-1" });
    expect(subjectOf({ ...none, card: "C-1", line: "812/1" })).toEqual({ to: "board", id: "C-1" });
    expect(subjectOf({ ...none, line: "812/1" })).toEqual({ to: "line", id: "812/1" });
    expect(subjectOf(none)).toBeNull();
  });

  it("labels the known types in the viewer's words and keeps another as stored", () => {
    expect(typeLabel("auto.held", enT)).toEqual({ label: "Held", raw: false });
    expect(typeLabel("proposal.applied", enT)).toEqual({ label: "Applied", raw: false });
    expect(typeLabel("verify.tests", enT)).toEqual({ label: "verify.tests", raw: true });
    expect(typeLabel("toString", enT)).toEqual({ label: "toString", raw: true });
  });

  it("cuts a rule hash to b3: and twelve hex; marks two hours or more as quiet", () => {
    expect(shortRule(`b3:${"a".repeat(64)}`)).toBe(`b3:${"a".repeat(12)}`);
    expect(quietBetween("2026-11-16T01:00:00Z", "2026-11-16T02:59:00Z")).toBeNull();
    expect(quietBetween("2026-11-16T01:00:00Z", "2026-11-16T03:05:00Z")).toEqual({ hours: 2, minutes: 5 });
    expect(quietBetween("not a time", "2026-11-16T03:05:00Z")).toBeNull();
  });
});
