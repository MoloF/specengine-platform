import { describe, expect, it } from "vitest";
import { KNOWN_CHECK_VERDICTS } from "../api/types";
import appCss from "../styles/app.css?raw";
import tokensCss from "../styles/tokens.css?raw";
import { syncLook } from "../tree/labels";
import {
  ASSUMED_STATES,
  assumedLook,
  bindingLabel,
  codeRowLook,
  DRIFT_CLASSES,
  driftClassLook,
  driftTally,
  findingSeverityLook,
  findingSeverityRank,
  idStateLabel,
  isCannotCheck,
  isClean,
  isKnownMode,
  TESTED_STATES,
  testedLook,
  UNBOUND_REASONS,
  unboundReasonLook,
  verdictLook,
} from "./labels";

// docs/features/ui-health.md AC-03: four verdicts, four labels and icons; cannot-check in the
// cannot-verify role; any other value raw and neutral. Severities and modes likewise.

describe("verdicts (AC-03)", () => {
  it("label the four known verdicts distinctly, each with its own icon, none by its raw text", () => {
    const looks = KNOWN_CHECK_VERDICTS.map((verdict) => verdictLook(verdict));
    expect(looks.map((look) => look.label)).toEqual(["Clean", "Passes with findings", "Fails the check", "Could not check"]);
    expect(new Set(looks.map((look) => look.icon)).size).toBe(4);
    expect(new Set(looks.map((look) => look.tone)).size).toBe(4);
    KNOWN_CHECK_VERDICTS.forEach((verdict, index) => {
      expect(looks[index]?.label).not.toBe(verdict);
    });
    expect(looks.map((look) => look.label).join(" ")).not.toMatch(/block/i);
  });

  it("draw cannot-check in its own role, never clean's", () => {
    const cannot = verdictLook("cannot-check");
    expect(cannot.tone).toBe("check-cannot-check");
    expect(cannot.tone).not.toBe(verdictLook("clean").tone);
    expect(cannot.icon).toBe("cannotCheck");
    expect([isCannotCheck("cannot-check"), isCannotCheck("clean"), isClean("clean"), isClean("observed")]).toEqual([true, false, true, false]);
  });

  it("show any other verdict raw in a neutral badge", () => {
    expect(verdictLook("triage")).toEqual({ label: "triage", tone: "check-unknown", icon: "unknown" });
    expect(verdictLook("Clean").tone).toBe("check-unknown");
    expect(verdictLook("toString").tone).toBe("check-unknown");
  });
});

describe("severities and modes", () => {
  it("label error and warning, rank them first, any other raw and last", () => {
    expect([findingSeverityLook("error").label, findingSeverityLook("warning").label, findingSeverityLook("fatal").label]).toEqual(["Error", "Warning", "fatal"]);
    expect(findingSeverityLook("fatal").tone).toBe("finding-unknown");
    expect([findingSeverityRank("error"), findingSeverityRank("warning"), findingSeverityRank("fatal")]).toEqual([0, 1, 2]);
  });

  it("know the three modes of the canon, nothing else", () => {
    expect(["observe", "enforce", "enforce-introduced", "strict", "constructor"].map(isKnownMode)).toEqual([true, true, true, false, false]);
  });
});

// docs/features/health-code-ui.md "Data": every state word of rows 4 and 5 through one map; an
// unknown word raw, never a failure (UI-2's other half: the parts compare none, policy.test.ts).

describe("rows 4 and 5's words (health-code-ui)", () => {
  it("draw a row measured, not measured (off, not configured) or could not be measured; any other state raw, not measured, no Retry", () => {
    expect(["measured", "off", "not_configured", "cannot"].map(codeRowLook)).toEqual([
      { label: "Measured", measured: true, retry: false },
      { label: "Code layer off", measured: false, retry: false },
      { label: "Not configured", measured: false, retry: false },
      { label: "Could not be measured", measured: false, retry: true },
    ]);
    expect(codeRowLook("stale")).toEqual({ label: "stale", measured: false, retry: false });
    expect(codeRowLook("constructor")).toEqual({ label: "constructor", measured: false, retry: false });
  });

  it("label row 4's eight states distinctly, each with its own tone and icon; a choice and record for a decided state alone, the open items for one in question alone", () => {
    expect(ASSUMED_STATES).toEqual(["pending", "as_assumed", "otherwise", "applied", "rejected", "unknown", "archived", "in_question"]);
    const looks = ASSUMED_STATES.map(assumedLook);
    expect(new Set(looks.map(({ look }) => look.label)).size).toBe(8);
    expect(new Set(looks.map(({ look }) => look.tone)).size).toBe(8);
    expect(new Set(looks.map(({ look }) => look.icon)).size).toBe(8);
    expect(looks.every(({ look }) => look.tone.startsWith("assumed-") && look.tone !== "assumed-unknown")).toBe(true);
    expect(ASSUMED_STATES.filter((state) => assumedLook(state).record)).toEqual(["as_assumed", "otherwise"]);
    expect(ASSUMED_STATES.filter((state) => assumedLook(state).under)).toEqual(["in_question"]);
    expect(looks.map(({ look }) => look.label).join(" ")).not.toMatch(/block/i);
    for (const word of ["superseded_x", "constructor", "toString"]) {
      expect(assumedLook(word)).toEqual({ look: { label: word, tone: "assumed-unknown", icon: "unknown" }, record: false, under: false });
    }
  });

  it("label row 5's four reasons distinctly; cannot_verify in cannot-verify's role and icon; any other raw, neutral", () => {
    expect(UNBOUND_REASONS).toEqual(["homoglyph", "unanchored", "cannot_verify", "relation"]);
    const looks = UNBOUND_REASONS.map(unboundReasonLook);
    expect(new Set(looks.map((look) => look.label)).size).toBe(4);
    expect(new Set(looks.map((look) => look.tone)).size).toBe(4);
    expect(unboundReasonLook("cannot_verify")).toEqual({ label: "Cannot verify", tone: "unbound-cannot-verify", icon: "cannotCheck" });
    expect(unboundReasonLook("shadowed")).toEqual({ label: "shadowed", tone: "unbound-unknown", icon: "unknown" });
    expect(unboundReasonLook("constructor").tone).toBe("unbound-unknown");
  });

  it("word `spec refs`' ID states and bindings; any other raw", () => {
    expect(["resolved", "alias", "ambiguous", "homoglyph", "other_project", "queue", "dangling"].map(idStateLabel)).toEqual([
      "Resolved",
      "Alias",
      "Ambiguous",
      "Look-alike",
      "Other project",
      "Queue item",
      "Dangling",
    ]);
    expect(["item", "module", "ron", "file", "unanchored", "cannot_verify"].map(bindingLabel)).toEqual(["Item", "Module", "RON field", "File", "Unanchored", "Cannot verify"]);
    expect([idStateLabel("foreign"), idStateLabel("constructor"), bindingLabel("macro"), bindingLabel("toString")]).toEqual(["foreign", "constructor", "macro", "toString"]);
  });

  it("give every tone of the two rows a badge rule in app.css and a colour role in tokens.css", () => {
    const tones = [...ASSUMED_STATES.map((state) => assumedLook(state).look.tone), "assumed-unknown", ...UNBOUND_REASONS.map((reason) => unboundReasonLook(reason).tone), "unbound-unknown"];
    for (const tone of tones) {
      expect([tone, appCss.includes(`.tone-${tone} { --badge-color: var(--${tone}); }`), tokensCss.includes(`--${tone}:`)]).toEqual([tone, true, true]);
    }
  });
});

// docs/features/health-drift-ui.md "Data": row 6's class and a `[[tested]]` entry's state through one
// map each, a row's state through the tree's syncLook, never a second table; an unknown word raw
// and neutral (UI-3's other half: the parts compare none and sort nothing, policy.test.ts).

describe("row 6's words (health-drift-ui)", () => {
  it("label verify's three classes distinctly, cannot in cannot-verify's role and icon, never ok's; any other class raw, neutral; none neutral too", () => {
    expect(DRIFT_CLASSES).toEqual(["fix", "check", "cannot"]);
    expect(DRIFT_CLASSES.map(driftClassLook)).toEqual([
      { label: "Fix", tone: "finding-error", icon: "findingError" },
      { label: "Check", tone: "finding-warning", icon: "findingWarning" },
      { label: "Cannot verify", tone: "sync-cannot-verify", icon: "cannotCheck" },
    ]);
    // A cannot row looks like a cannot-verify state, never like an ok one.
    expect(driftClassLook("cannot")).toEqual(syncLook("cannot_verify"));
    expect(driftClassLook("cannot").tone).not.toBe(syncLook("ok").tone);
    expect(driftClassLook("advisory")).toEqual({ label: "advisory", tone: "sync-unknown", icon: "unknown" });
    expect(driftClassLook("constructor").tone).toBe("sync-unknown");
    expect(driftClassLook(null)).toEqual({ label: "No class", tone: "sync-unknown", icon: "unknown" });
  });

  it("tally verify's counts in its summary's order, each class in its look, ok in the tree's In sync; the counts as sent, never summed here", () => {
    const tally = driftTally({ rows: 39, ok: 8, fix: 19, check: 6, cannot: 6, pre_existing: 0, skipped: 2 });
    expect(tally.map(({ key, look, count }) => [key, look.label, look.tone, count])).toEqual([
      ["fix", "Fix", "finding-error", 19],
      ["check", "Check", "finding-warning", 6],
      ["cannot", "Cannot verify", "sync-cannot-verify", 6],
      ["ok", "In sync", "sync-ok", 8],
    ]);
  });

  it("label the four [[tested]] states distinctly, unverifiable in cannot-verify's role, never verified's; any other raw, neutral", () => {
    expect(TESTED_STATES).toEqual(["verified", "stale", "unverifiable", "node_gone"]);
    const looks = TESTED_STATES.map(testedLook);
    expect(looks.map((look) => look.label)).toEqual(["Verified", "Stale", "Unverifiable", "Node gone"]);
    expect(new Set(looks.map((look) => look.tone)).size).toBe(4);
    expect(new Set(looks.map((look) => look.icon)).size).toBe(4);
    expect(testedLook("unverifiable")).toEqual({ label: "Unverifiable", tone: "sync-cannot-verify", icon: "cannotCheck" });
    expect(testedLook("unverifiable").tone).not.toBe(testedLook("verified").tone);
    expect(testedLook("expired")).toEqual({ label: "expired", tone: "sync-unknown", icon: "unknown" });
    expect(testedLook("toString").tone).toBe("sync-unknown");
    expect([...looks, ...DRIFT_CLASSES.map(driftClassLook)].map((look) => look.label).join(" ")).not.toMatch(/block/i);
  });

  it("give every tone of row 6 a badge rule in app.css and a colour role in tokens.css: no new tone", () => {
    const tones = [...DRIFT_CLASSES.map((value) => driftClassLook(value).tone), driftClassLook("advisory").tone, ...TESTED_STATES.map((state) => testedLook(state).tone), testedLook("expired").tone];
    for (const tone of tones) {
      expect([tone, appCss.includes(`.tone-${tone} { --badge-color: var(--${tone}); }`), tokensCss.includes(`--${tone}:`)]).toEqual([tone, true, true]);
    }
  });
});
