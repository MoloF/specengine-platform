import { describe, expect, it, vi } from "vitest";
import { ClientError } from "../api/client";
import { KNOWN_CHECK_VERDICTS, type CheckFinding, type CheckReport } from "../api/types";
import { checkReportOf } from "./check";
import { MockClient } from "./MockClient";

// The mock's `check` (docs/features/ui-health.md "Data", Mock): one builder, no clock; normal,
// empty, cannot-check and large as the spec lists them, `error` and `slow` as every read.

function sortedAsTheCore(findings: readonly CheckFinding[]): boolean {
  return findings.every((finding, index) => {
    const before = findings[index - 1];
    if (before === undefined) {
      return true;
    }
    const a = [before.path, before.line, before.code, before.subject, before.message];
    const b = [finding.path, finding.line, finding.code, finding.subject, finding.message];
    const at = a.findIndex((value, slot) => value !== b[slot]);
    const [x, y] = [a[at], b[at]];
    return at === -1 || (x !== undefined && y !== undefined && x < y);
  });
}

/** The core's counting rule, restated: live debt apart, an expired one an error again. */
function recount(report: CheckReport) {
  const live = (finding: CheckFinding) => finding.debt !== undefined && !finding.debt.expired;
  return {
    errors: report.findings.filter((finding) => finding.severity === "error" && !live(finding)).length,
    warnings: report.findings.filter((finding) => finding.severity === "warning" && !live(finding)).length,
    debt: report.findings.filter(live).length,
    expired: report.findings.filter((finding) => finding.severity === "error" && finding.debt?.expired === true).length,
    stale: report.stale.length,
  };
}

describe("the mock's check", () => {
  it("normal: harbor-sim is the spec's sample, observed under observe, its counts the core's", () => {
    const report = checkReportOf("harbor-sim", "normal");
    expect([report.mode, report.verdict]).toEqual(["observe", "observed"]);
    expect(report.counts).toEqual({ documents: 41, errors: 2, warnings: 0, debt: 1, expired: 0, stale: 1, worst_w_bytes: 61234 });
    expect(report.findings.map((finding) => [finding.code, finding.path, finding.line, finding.subject])).toEqual([
      ["budget", "docs/canon/tides.md", 1, "canon"],
      ["ref-dangling", "docs/spec/berths/mooring.md", 12, "RULE-TIDE-GATE"],
      ["id-width", "docs/spec/cranes.md", 3, "CR-7"],
    ]);
    expect(report.findings[0]?.message).toBe("12950 bytes, over the canon cap of 12288: move detail down a tier; caps are never raised");
    expect(report.stale).toEqual([{ code: "file-name", path: "docs/spec/harbor.md", subject: "", reason: "legacy import", expires: "2026-12-31", line: 7 }]);
    expect(report.cannot_check).toEqual([]);
    expect(Object.keys(report)).toEqual(["mode", "verdict", "counts", "findings", "stale", "cannot_check"]);
  });

  it("normal: ledger-api fails the check under enforce: an expired debt, a fix, a config path", () => {
    const report = checkReportOf("ledger-api", "normal");
    expect(report.mode).toBe("enforce");
    // The third known verdict: a check outcome, labelled "Fails the check" on screen.
    expect(report.verdict).toBe(KNOWN_CHECK_VERDICTS[2]);
    expect(report.counts.expired).toBe(1);
    expect(report.findings.some((finding) => finding.fix !== undefined)).toBe(true);
    expect(report.findings.some((finding) => finding.path === "specengine.toml")).toBe(true);
    expect(report.findings.some((finding) => finding.debt?.expired === true)).toBe(true);
    expect(report.stale).toEqual([]);
  });

  it.each([
    ["harbor-sim", "normal"],
    ["ledger-api", "normal"],
    ["harbor-sim", "large"],
    ["ledger-api", "large"],
  ] as const)("%s in %s: sorted as the core sorts, counted as the core counts, no key set to undefined", (project, scenario) => {
    const report = checkReportOf(project, scenario);
    expect(sortedAsTheCore(report.findings)).toBe(true);
    expect(recount(report)).toEqual({
      errors: report.counts.errors,
      warnings: report.counts.warnings,
      debt: report.counts.debt,
      expired: report.counts.expired,
      stale: report.counts.stale,
    });
    expect(JSON.parse(JSON.stringify(report))).toEqual(report);
    expect("introduced" in report.counts || "new_debt" in report.counts || "new_debt" in report).toBe(false);
  });

  it("empty: both projects clean, no finding, no debt", () => {
    for (const project of ["harbor-sim", "ledger-api"]) {
      const report = checkReportOf(project, "empty");
      expect([report.verdict, report.findings, report.stale, report.cannot_check]).toEqual(["clean", [], [], []]);
    }
  });

  it("cannot-check: two causes each, worst W 0, no finding", () => {
    for (const project of ["harbor-sim", "ledger-api"]) {
      const report = checkReportOf(project, "cannot-check");
      expect(report.verdict).toBe("cannot-check");
      expect(report.cannot_check).toHaveLength(2);
      expect(report.counts.worst_w_bytes).toBe(0);
      expect(report.findings).toEqual([]);
    }
  });

  it("large: harbor-sim holds 3 000 findings over 12 codes, `budget` among them", () => {
    const report = checkReportOf("harbor-sim", "large");
    expect(report.findings).toHaveLength(3000);
    const codes = new Set(report.findings.map((finding) => finding.code));
    expect(codes.size).toBe(12);
    expect(codes.has("budget")).toBe(true);
    expect(new Set(report.findings.map((finding) => JSON.stringify(finding))).size).toBe(3000);
    expect(report.findings.filter((finding) => finding.debt !== undefined).length).toBeGreaterThan(0);
  });

  it("reads no clock: two builds are equal", () => {
    const now = vi.spyOn(Date, "now");
    expect(checkReportOf("harbor-sim", "large")).toEqual(checkReportOf("harbor-sim", "large"));
    expect(now).not.toHaveBeenCalled();
  });
});

describe("MockClient.getCheck", () => {
  it("serves the builder's report, a copy each call", async () => {
    const client = new MockClient("normal");
    const first = await client.getCheck("harbor-sim");
    expect(first).toEqual(checkReportOf("harbor-sim", "normal"));
    first.findings.length = 0;
    expect((await client.getCheck("harbor-sim")).findings).toHaveLength(3);
  });

  it("fails as every read in the error scenario, and for an unknown project", async () => {
    await expect(new MockClient("error").getCheck("harbor-sim")).rejects.toBeInstanceOf(ClientError);
    await expect(new MockClient("normal").getCheck("zeta")).rejects.toMatchObject({ status: 404 });
  });

  it("serves the cannot-check report in that scenario, every other read as normal", async () => {
    const client = new MockClient("cannot-check");
    expect((await client.getCheck("ledger-api")).verdict).toBe("cannot-check");
    expect((await client.getInbox("harbor-sim")).proposals.length).toBe((await new MockClient("normal").getInbox("harbor-sim")).proposals.length);
  });
});
