import { describe, expect, it } from "vitest";
import { gapLabel, previewLabel, severityLook, severityRank, statusLook, UNRANKED } from "./labels";
import { enT } from "../test/text";
import { ru } from "../i18n/ru";
import { ruT } from "../test/text";
import * as inbox from "./labels";

describe("closed tables (AC-07)", () => {
  it("label known severities and keep an unknown one raw and neutral", () => {
    expect(severityLook("high", enT)).toMatchObject({ label: "High", tone: "severity-high" });
    expect(severityLook("urgent", enT)).toEqual({ label: "urgent", tone: "severity-unknown", icon: "unknown", served: true });
    expect(severityLook(null, enT).tone).toBe("severity-unknown");
    expect(severityRank("urgent")).toBe(UNRANKED);
    expect(severityRank("low")).toBeLessThan(UNRANKED);
    expect(severityRank("toString")).toBe(UNRANKED);
  });

  it("label known states and keep an unknown one raw and neutral", () => {
    expect(statusLook("changes_requested", enT)).toMatchObject({ label: "Changes requested", tone: "proposal-changes-requested" });
    expect(statusLook("escalated", enT)).toEqual({ label: "escalated", tone: "proposal-unknown", icon: "unknown", served: true });
    expect(statusLook("constructor", enT).tone).toBe("proposal-unknown");
  });

  it("give every known value its own icon, so colour is never alone", () => {
    const icons = ["high", "normal", "low", "x"].map((value) => severityLook(value, enT).icon);
    expect(new Set(icons).size).toBe(icons.length);
    const states = ["open", "changes_requested", "approved", "applied", "rejected", "deferred", "superseded", "x"];
    expect(new Set(states.map((value) => statusLook(value, enT).icon)).size).toBe(states.length);
  });

  it("label gap types and previews, raw when unknown, null when absent", () => {
    expect(gapLabel("contradicts", enT)).toBe("Contradicts the spec");
    expect(gapLabel("sideways", enT)).toBe("sideways");
    expect(gapLabel(null, enT)).toBeNull();
    expect(previewLabel("rebases", enT)).toBe("Rebases onto the current text");
    expect(previewLabel("maybe", enT)).toBe("maybe");
    expect(previewLabel(null, enT)).toBeNull();
  });
});

// LG-3 of docs/features/ui-languages.md: under `ruT` every known value's label is ru.ts's value at its key (M9).
describe("the Inbox tables in Russian (LG-3)", () => {
  it("labels each severity, status, gap type and preview from ru.ts, an unknown one raw", () => {
    for (const severity of inbox.KNOWN_SEVERITIES) {
      expect([severity, inbox.severityLook(severity, ruT).label]).toEqual([severity, ru.inbox.severity[severity]]);
    }
    expect(inbox.severityLook(null, ruT).label).toBe(ru.inbox.severity.none);
    expect(inbox.KNOWN_PROPOSAL_STATUSES).toHaveLength(7);
    for (const status of inbox.KNOWN_PROPOSAL_STATUSES) {
      expect([status, inbox.statusLook(status, ruT).label]).toEqual([status, ru.inbox.status[status]]);
    }
    for (const gap of inbox.KNOWN_GAP_TYPES) {
      expect([gap, inbox.gapLabel(gap, ruT)]).toEqual([gap, ru.inbox.gap[gap]]);
    }
    for (const preview of inbox.KNOWN_PREVIEWS) {
      expect([preview, inbox.previewLabel(preview, ruT)]).toEqual([preview, ru.inbox.preview[preview]]);
    }
    expect([inbox.severityLook("urgent", ruT).label, inbox.statusLook("parked", ruT).label, inbox.gapLabel("odd", ruT), inbox.previewLabel("later", ruT)]).toEqual(["urgent", "parked", "odd", "later"]);
  });
});

// DS5 of docs/features/design-sweep.md "Acceptance criteria": a proposal's status as a lozenge, one
// table the Inbox's head and a task's items share; an unknown word raw, neutral, under lang="en".
describe("statusLozenge (DS5 of design-sweep)", () => {
  const boardLabels = import.meta.glob<string>("/src/board/labels.ts", { query: "?raw", import: "default", eager: true })["/src/board/labels.ts"] ?? "";

  it("looks each of the seven up, every one with its own icon", () => {
    expect(inbox.KNOWN_PROPOSAL_STATUSES.map((status) => [status, inbox.statusLozenge(status, enT).appearance])).toEqual([
      ["open", "neutral"],
      ["changes_requested", "warning"],
      ["approved", "information"],
      ["applied", "success"],
      ["rejected", "danger"],
      ["deferred", "neutral"],
      ["superseded", "neutral"],
    ]);
    expect(inbox.statusLozenge("changes_requested", enT)).toEqual({ appearance: "warning", icon: "changes", label: "Changes requested" });
    const icons = inbox.KNOWN_PROPOSAL_STATUSES.map((status) => inbox.statusLozenge(status, enT).icon);
    expect(new Set([...icons, inbox.statusLozenge("weird", enT).icon]).size).toBe(8);
    for (const status of inbox.KNOWN_PROPOSAL_STATUSES) {
      expect([status, inbox.statusLozenge(status, ruT).label, inbox.statusLozenge(status, ruT).lang]).toEqual([status, ru.inbox.status[status], undefined]);
    }
  });

  it("shows a word it does not know raw and neutral under lang=en, never a known one", () => {
    expect(inbox.statusLozenge("weird", enT)).toEqual({ appearance: "neutral", icon: "unknown", label: "weird", lang: "en" });
    expect(inbox.statusLozenge("constructor", ruT)).toEqual({ appearance: "neutral", icon: "unknown", label: "constructor", lang: "en" });
  });

  it("is the Board's item status too: board/labels.ts delegates and holds no table of its own", async () => {
    const board = await import("../board/labels");
    for (const status of [...inbox.KNOWN_PROPOSAL_STATUSES, "weird"]) {
      expect([status, board.itemStatusLook(status, enT)]).toEqual([status, inbox.statusLozenge(status, enT)]);
    }
    // M: a second status table in board/labels.ts.
    expect(boardLabels).toContain("return statusLozenge(status, t);");
    expect(boardLabels).not.toMatch(/changes_requested\s*:/);
  });
});
