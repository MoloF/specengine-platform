import { describe, expect, it } from "vitest";
import { gapLabel, previewLabel, severityLook, severityRank, statusLook, UNRANKED } from "./labels";

describe("closed tables (AC-07)", () => {
  it("label known severities and keep an unknown one raw and neutral", () => {
    expect(severityLook("high")).toMatchObject({ label: "High", tone: "severity-high" });
    expect(severityLook("urgent")).toEqual({ label: "urgent", tone: "severity-unknown", icon: "unknown" });
    expect(severityLook(null).tone).toBe("severity-unknown");
    expect(severityRank("urgent")).toBe(UNRANKED);
    expect(severityRank("low")).toBeLessThan(UNRANKED);
    expect(severityRank("toString")).toBe(UNRANKED);
  });

  it("label known states and keep an unknown one raw and neutral", () => {
    expect(statusLook("changes_requested")).toMatchObject({ label: "Changes requested", tone: "proposal-changes-requested" });
    expect(statusLook("escalated")).toEqual({ label: "escalated", tone: "proposal-unknown", icon: "unknown" });
    expect(statusLook("constructor").tone).toBe("proposal-unknown");
  });

  it("give every known value its own icon, so colour is never alone", () => {
    const icons = ["high", "normal", "low", "x"].map((value) => severityLook(value).icon);
    expect(new Set(icons).size).toBe(icons.length);
    const states = ["open", "changes_requested", "approved", "applied", "rejected", "deferred", "superseded", "x"];
    expect(new Set(states.map((value) => statusLook(value).icon)).size).toBe(states.length);
  });

  it("label gap types and previews, raw when unknown, null when absent", () => {
    expect(gapLabel("contradicts")).toBe("Contradicts the spec");
    expect(gapLabel("sideways")).toBe("sideways");
    expect(gapLabel(null)).toBeNull();
    expect(previewLabel("rebases")).toBe("Rebases onto the current text");
    expect(previewLabel("maybe")).toBe("maybe");
    expect(previewLabel(null)).toBeNull();
  });
});
