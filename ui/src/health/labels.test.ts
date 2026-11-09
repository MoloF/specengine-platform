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
import { enT } from "../test/text";
import { ru } from "../i18n/ru";
import { ruT } from "../test/text";
import * as health from "./labels";

// docs/features/ui-health.md AC-03: four verdicts, four labels and icons; cannot-check in the
// cannot-verify role; any other value raw and neutral. Severities and modes likewise.

describe("verdicts (AC-03)", () => {
  it("label the four known verdicts distinctly, each with its own icon, none by its raw text", () => {
    const looks = KNOWN_CHECK_VERDICTS.map((verdict) => verdictLook(verdict, enT));
    expect(looks.map((look) => look.label)).toEqual(["Clean", "Passes with findings", "Fails the check", "Could not check"]);
    expect(new Set(looks.map((look) => look.icon)).size).toBe(4);
    expect(new Set(looks.map((look) => look.tone)).size).toBe(4);
    KNOWN_CHECK_VERDICTS.forEach((verdict, index) => {
      expect(looks[index]?.label).not.toBe(verdict);
    });
    expect(looks.map((look) => look.label).join(" ")).not.toMatch(/block/i);
  });

  it("draw cannot-check in its own role, never clean's", () => {
    const cannot = verdictLook("cannot-check", enT);
    expect(cannot.tone).toBe("check-cannot-check");
    expect(cannot.tone).not.toBe(verdictLook("clean", enT).tone);
    expect(cannot.icon).toBe("cannotCheck");
    expect([isCannotCheck("cannot-check"), isCannotCheck("clean"), isClean("clean"), isClean("observed")]).toEqual([true, false, true, false]);
  });

  it("show any other verdict raw in a neutral badge", () => {
    expect(verdictLook("triage", enT)).toEqual({ label: "triage", tone: "check-unknown", icon: "unknown", served: true });
    expect(verdictLook("Clean", enT).tone).toBe("check-unknown");
    expect(verdictLook("toString", enT).tone).toBe("check-unknown");
  });
});

describe("severities and modes", () => {
  it("label error and warning, rank them first, any other raw and last", () => {
    expect([findingSeverityLook("error", enT).label, findingSeverityLook("warning", enT).label, findingSeverityLook("fatal", enT).label]).toEqual(["Error", "Warning", "fatal"]);
    expect(findingSeverityLook("fatal", enT).tone).toBe("finding-unknown");
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
    expect(["measured", "off", "not_configured", "cannot"].map((value) => codeRowLook(value, enT))).toEqual([
      { label: "Measured", measured: true, retry: false },
      { label: "Code layer off", measured: false, retry: false },
      { label: "Not configured", measured: false, retry: false },
      { label: "Could not be measured", measured: false, retry: true },
    ]);
    expect(codeRowLook("stale", enT)).toEqual({ label: "stale", measured: false, retry: false });
    expect(codeRowLook("constructor", enT)).toEqual({ label: "constructor", measured: false, retry: false });
  });

  it("label row 4's eight states distinctly, each with its own tone and icon; a choice and record for a decided state alone, the open items for one in question alone", () => {
    expect(ASSUMED_STATES).toEqual(["pending", "as_assumed", "otherwise", "applied", "rejected", "unknown", "archived", "in_question"]);
    const looks = ASSUMED_STATES.map((value) => assumedLook(value, enT));
    expect(new Set(looks.map(({ look }) => look.label)).size).toBe(8);
    expect(new Set(looks.map(({ look }) => look.tone)).size).toBe(8);
    expect(new Set(looks.map(({ look }) => look.icon)).size).toBe(8);
    expect(looks.every(({ look }) => look.tone.startsWith("assumed-") && look.tone !== "assumed-unknown")).toBe(true);
    expect(ASSUMED_STATES.filter((state) => assumedLook(state, enT).record)).toEqual(["as_assumed", "otherwise"]);
    expect(ASSUMED_STATES.filter((state) => assumedLook(state, enT).under)).toEqual(["in_question"]);
    expect(looks.map(({ look }) => look.label).join(" ")).not.toMatch(/block/i);
    for (const word of ["superseded_x", "constructor", "toString"]) {
      expect(assumedLook(word, enT)).toEqual({ look: { label: word, tone: "assumed-unknown", icon: "unknown", served: true }, record: false, under: false });
    }
  });

  it("label row 5's four reasons distinctly; cannot_verify in cannot-verify's role and icon; any other raw, neutral", () => {
    expect(UNBOUND_REASONS).toEqual(["homoglyph", "unanchored", "cannot_verify", "relation"]);
    const looks = UNBOUND_REASONS.map((value) => unboundReasonLook(value, enT));
    expect(new Set(looks.map((look) => look.label)).size).toBe(4);
    expect(new Set(looks.map((look) => look.tone)).size).toBe(4);
    expect(unboundReasonLook("cannot_verify", enT)).toEqual({ label: "Cannot verify", tone: "unbound-cannot-verify", icon: "cannotCheck" });
    expect(unboundReasonLook("shadowed", enT)).toEqual({ label: "shadowed", tone: "unbound-unknown", icon: "unknown", served: true });
    expect(unboundReasonLook("constructor", enT).tone).toBe("unbound-unknown");
  });

  it("word `spec refs`' ID states and bindings; any other raw", () => {
    expect(["resolved", "alias", "ambiguous", "homoglyph", "other_project", "queue", "dangling"].map((value) => idStateLabel(value, enT))).toEqual([
      "Resolved",
      "Alias",
      "Ambiguous",
      "Look-alike",
      "Other project",
      "Queue item",
      "Dangling",
    ]);
    expect(["item", "module", "ron", "file", "unanchored", "cannot_verify"].map((value) => bindingLabel(value, enT))).toEqual(["Item", "Module", "RON field", "File", "Unanchored", "Cannot verify"]);
    expect([idStateLabel("foreign", enT), idStateLabel("constructor", enT), bindingLabel("macro", enT), bindingLabel("toString", enT)]).toEqual(["foreign", "constructor", "macro", "toString"]);
  });

  it("give every tone of the two rows a badge rule in app.css and a colour role in tokens.css", () => {
    const tones = [...ASSUMED_STATES.map((state) => assumedLook(state, enT).look.tone), "assumed-unknown", ...UNBOUND_REASONS.map((reason) => unboundReasonLook(reason, enT).tone), "unbound-unknown"];
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
    expect(DRIFT_CLASSES.map((value) => driftClassLook(value, enT))).toEqual([
      { label: "Fix", tone: "finding-error", icon: "findingError" },
      { label: "Check", tone: "finding-warning", icon: "findingWarning" },
      { label: "Cannot verify", tone: "sync-cannot-verify", icon: "cannotCheck" },
    ]);
    // A cannot row looks like a cannot-verify state, never like an ok one.
    expect(driftClassLook("cannot", enT)).toEqual(syncLook("cannot_verify", enT));
    expect(driftClassLook("cannot", enT).tone).not.toBe(syncLook("ok", enT).tone);
    expect(driftClassLook("advisory", enT)).toEqual({ label: "advisory", tone: "sync-unknown", icon: "unknown", served: true });
    expect(driftClassLook("constructor", enT).tone).toBe("sync-unknown");
    expect(driftClassLook(null, enT)).toEqual({ label: "No class", tone: "sync-unknown", icon: "unknown" });
  });

  it("tally verify's counts in its summary's order, each class in its look, ok in the tree's In sync; the counts as sent, never summed here", () => {
    const tally = driftTally({ rows: 39, ok: 8, fix: 19, check: 6, cannot: 6, pre_existing: 0, skipped: 2 }, enT);
    expect(tally.map(({ key, look, count }) => [key, look.label, look.tone, count])).toEqual([
      ["fix", "Fix", "finding-error", 19],
      ["check", "Check", "finding-warning", 6],
      ["cannot", "Cannot verify", "sync-cannot-verify", 6],
      ["ok", "In sync", "sync-ok", 8],
    ]);
  });

  it("label the four [[tested]] states distinctly, unverifiable in cannot-verify's role, never verified's; any other raw, neutral", () => {
    expect(TESTED_STATES).toEqual(["verified", "stale", "unverifiable", "node_gone"]);
    const looks = TESTED_STATES.map((value) => testedLook(value, enT));
    expect(looks.map((look) => look.label)).toEqual(["Verified", "Stale", "Unverifiable", "Node gone"]);
    expect(new Set(looks.map((look) => look.tone)).size).toBe(4);
    expect(new Set(looks.map((look) => look.icon)).size).toBe(4);
    expect(testedLook("unverifiable", enT)).toEqual({ label: "Unverifiable", tone: "sync-cannot-verify", icon: "cannotCheck" });
    expect(testedLook("unverifiable", enT).tone).not.toBe(testedLook("verified", enT).tone);
    expect(testedLook("expired", enT)).toEqual({ label: "expired", tone: "sync-unknown", icon: "unknown", served: true });
    expect(testedLook("toString", enT).tone).toBe("sync-unknown");
    expect([...looks, ...DRIFT_CLASSES.map((value) => driftClassLook(value, enT))].map((look) => look.label).join(" ")).not.toMatch(/block/i);
  });

  it("give every tone of row 6 a badge rule in app.css and a colour role in tokens.css: no new tone", () => {
    const tones = [...DRIFT_CLASSES.map((value) => driftClassLook(value, enT).tone), driftClassLook("advisory", enT).tone, ...TESTED_STATES.map((state) => testedLook(state, enT).tone), testedLook("expired", enT).tone];
    for (const tone of tones) {
      expect([tone, appCss.includes(`.tone-${tone} { --badge-color: var(--${tone}); }`), tokensCss.includes(`--${tone}:`)]).toEqual([tone, true, true]);
    }
  });
});

// LG-3 of docs/features/ui-languages.md: under `ruT` every known value's label is ru.ts's value at its key (M9).
describe("the check's and Health's tables in Russian (LG-3)", () => {
  it("labels each verdict and finding severity from ru.ts", () => {
    const [clean, observed, fails, cannot] = KNOWN_CHECK_VERDICTS;
    expect([clean, observed, fails, cannot].map((verdict) => health.verdictLook(verdict, ruT).label)).toEqual([
      ru.health.verdict.clean,
      ru.health.verdict.observed,
      ru.health.verdict.fails,
      ru.health.verdict.cannotCheck,
    ]);
    expect([health.findingSeverityLook("error", ruT).label, health.findingSeverityLook("warning", ruT).label]).toEqual([ru.health.severity.error, ru.health.severity.warning]);
  });

  it("labels each code row, row 4, row 5, ID state and binding word from ru.ts", () => {
    for (const state of health.CODE_ROW_STATES) {
      expect([state, health.codeRowLook(state, ruT).label]).toEqual([state, ru.health.codeRow[state]]);
    }
    expect(health.measuredWithoutCounts(ruT)).toBe(ru.health.measuredWithoutCounts);
    for (const state of health.ASSUMED_STATES) {
      expect([state, health.assumedLook(state, ruT).look.label]).toEqual([state, ru.health.assumed[state as keyof typeof ru.health.assumed]]);
    }
    for (const reason of health.UNBOUND_REASONS) {
      expect([reason, health.unboundReasonLook(reason, ruT).label]).toEqual([reason, ru.health.unbound[reason as keyof typeof ru.health.unbound]]);
    }
    for (const state of health.ID_STATES) {
      expect([state, health.idStateLabel(state, ruT)]).toEqual([state, ru.health.idState[state]]);
    }
    for (const binding of health.BINDINGS) {
      expect([binding, health.bindingLabel(binding, ruT)]).toEqual([binding, ru.health.binding[binding]]);
    }
  });

  it("labels row 6's classes, its tally, the tree's badges and the tested states from ru.ts, an unknown one raw", () => {
    for (const value of health.DRIFT_CLASSES) {
      expect([value, health.driftClassLook(value, ruT).label]).toEqual([value, ru.health.driftClass[value as keyof typeof ru.health.driftClass]]);
    }
    expect(health.driftClassLook(null, ruT).label).toBe(ru.health.noClass);
    expect(health.driftTally({ rows: 10, ok: 4, fix: 1, check: 2, cannot: 3, pre_existing: 0, skipped: 0 }, ruT).map(({ look }) => look.label)).toEqual([
      ru.health.driftClass.fix,
      ru.health.driftClass.check,
      ru.health.driftClass.cannot,
      ru.tree.sync.ok,
    ]);
    expect(health.driftBadgeLooks({ fix: 0, check: 0, cannot: 0, unlisted: 2 }, ruT).map((look) => look.label)).toEqual([`${ru.health.anotherClass} 2`]);
    for (const state of health.TESTED_STATES) {
      expect([state, health.testedLook(state, ruT).label]).toEqual([state, ru.health.tested[state as keyof typeof ru.health.tested]]);
    }
    expect([health.verdictLook("pending", ruT).label, health.codeRowLook("paused", ruT).label, health.testedLook("odd", ruT).label]).toEqual(["pending", "paused", "odd"]);
  });
});
