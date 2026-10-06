import { describe, expect, it } from "vitest";
import { KNOWN_CHECK_VERDICTS } from "../api/types";
import { findingSeverityLook, findingSeverityRank, isCannotCheck, isClean, isKnownMode, verdictLook } from "./labels";

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
