import { describe, expect, it } from "vitest";
import { KNOWN_CHECK_VERDICTS } from "../api/types";
import appCss from "../styles/app.css?raw";
import tokensCss from "../styles/tokens.css?raw";
import {
  ASSUMED_STATES,
  assumedLook,
  bindingLabel,
  codeRowLook,
  findingSeverityLook,
  findingSeverityRank,
  idStateLabel,
  isCannotCheck,
  isClean,
  isKnownMode,
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
