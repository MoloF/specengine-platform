import type { CheckCounts, CheckFinding, CheckReport, DebtEntry } from "../api/types";
import type { Scenario } from "./scenario";

// The mock's `spec check --json` (docs/features/ui-health.md "Data", Mock): one builder, no clock.
// normal: harbor-sim passes with findings (the spec's sample: a budget, a warning in live debt, a
// dangling reference, a stale baseline entry), ledger-api fails the check in `enforce` (an expired
// debt, a fix, a config path and a warning); empty: clean, no finding; cannot-check: two causes,
// worst W 0; large: harbor-sim 3 000 findings over 12 codes. Findings are sorted as the core sorts
// them, by (path, line, code, subject, message); the counts are the core's rule over them.

const HARBOR = "harbor-sim";
const LEDGER = "ledger-api";

/** A dangling ID written as a reference; the core's message shape (`check/engine.rs`). */
function dangling(key: string, written: string): string {
  return `\`${key}\`: \`${written}\` names no ID of this corpus`;
}

/** A budget overflow as the core words it (`check/engine.rs` `fn budget`). */
function overBudget(bytes: number, slot: string, cap: number): string {
  return `${String(bytes)} bytes, over the ${slot} cap of ${String(cap)}: move detail down a tier; caps are never raised`;
}

/** The core's order of findings: (path, line, code, subject, message), byte order. */
function byCoreOrder(a: CheckFinding, b: CheckFinding): number {
  const left = [a.path, a.line, a.code, a.subject, a.message] as const;
  const right = [b.path, b.line, b.code, b.subject, b.message] as const;
  for (let at = 0; at < left.length; at += 1) {
    const [x, y] = [left[at], right[at]];
    if (x !== undefined && y !== undefined && x !== y) {
      return x < y ? -1 : 1;
    }
  }
  return 0;
}

/** Counts as the core counts (`docs/canon/spec-check.md` "Findings, debt, verdict"): live debt apart, expired debt an error again. */
function countsOf(findings: readonly CheckFinding[], stale: readonly DebtEntry[], documents: number, worstW: number): CheckCounts {
  const live = (finding: CheckFinding) => finding.debt !== undefined && !finding.debt.expired;
  return {
    documents,
    errors: findings.filter((finding) => finding.severity === "error" && !live(finding)).length,
    warnings: findings.filter((finding) => finding.severity === "warning" && !live(finding)).length,
    debt: findings.filter(live).length,
    expired: findings.filter((finding) => finding.severity === "error" && finding.debt?.expired === true).length,
    stale: stale.length,
    worst_w_bytes: worstW,
  };
}

function report(fields: Pick<CheckReport, "mode" | "verdict" | "findings" | "stale">, documents: number, worstW: number): CheckReport {
  const findings = [...fields.findings].sort(byCoreOrder);
  return {
    mode: fields.mode,
    verdict: fields.verdict,
    counts: countsOf(findings, fields.stale, documents, worstW),
    findings,
    stale: fields.stale,
    cannot_check: [],
  };
}

/** harbor-sim's baseline entry that matches no finding: line 7 of its `.spec-debt.toml`. */
const HARBOR_STALE: DebtEntry = { code: "file-name", path: "docs/spec/harbor.md", subject: "", reason: "legacy import", expires: "2026-12-31", line: 7 };

/** The spec's sample (docs/features/ui-health.md "Data"): `observed` under `observe`. */
function harborObserved(): CheckReport {
  return report(
    {
      mode: "observe",
      verdict: "observed",
      findings: [
        { code: "budget", severity: "error", path: "docs/canon/tides.md", line: 1, subject: "canon", message: overBudget(12950, "canon", 12288) },
        {
          code: "id-width",
          severity: "warning",
          path: "docs/spec/cranes.md",
          line: 3,
          subject: "CR-7",
          message: "`CR-7` has 1 digits; `[ids] CR` issues 3",
          debt: { reason: "legacy import", expires: "2026-12-31", expired: false },
        },
        {
          code: "ref-dangling",
          severity: "error",
          path: "docs/spec/berths/mooring.md",
          line: 12,
          subject: "RULE-TIDE-GATE",
          message: dangling("depends_on", "RULE-TIDE-GATE"),
        },
      ],
      stale: [HARBOR_STALE],
    },
    41,
    61234,
  );
}

/** The ID with a Cyrillic capital O (U+041E) in place of the Latin one, escaped here (ADR-0024). */
const LOOKALIKE_ID = "POL-IDEMP\u041eTENCY";

/** ledger-api under `enforce`: errors outside live debt, one of them in expired debt. */
function ledgerFails(): CheckReport {
  return report(
    {
      mode: "enforce",
      verdict: "blocked",
      findings: [
        {
          code: "budget",
          severity: "error",
          path: "docs/README.md",
          line: 1,
          subject: "tier1",
          message: overBudget(10600, "tier1", 10240),
          debt: { reason: "split planned with the v2 docs", expires: "2026-12-31", expired: false },
        },
        {
          code: "mention-dangling",
          severity: "warning",
          path: "docs/spec/README.md",
          line: 14,
          subject: "EP-PAYOUTS",
          message: "`EP-PAYOUTS` is mentioned but names no ID of this corpus",
        },
        {
          code: "homoglyph",
          severity: "error",
          path: "docs/spec/api/idempotency.md",
          line: 7,
          subject: LOOKALIKE_ID,
          message: `\`${LOOKALIKE_ID}\` mixes scripts; IDs are Latin only: \`POL-IDEMPOTENCY\``,
          fix: { span: { start: 214, end: 230 }, text: "POL-IDEMPOTENCY" },
        },
        {
          code: "key-missing",
          severity: "error",
          path: "docs/spec/api/legacy-v0.md",
          line: 1,
          subject: "status",
          message: "key `status` is required for class spec",
          debt: { reason: "legacy v0 API, retired in Q3", expires: "2026-09-30", expired: true },
        },
        {
          code: "ref-dangling",
          severity: "error",
          path: "docs/spec/refunds/window.md",
          line: 9,
          subject: "POL-REFUND-CAP",
          message: dangling("constrains", "POL-REFUND-CAP"),
        },
        {
          code: "generator-path",
          severity: "warning",
          path: "specengine.toml",
          line: 22,
          subject: "docs/index.md",
          message: "`[[generators]]` output `docs/index.md` is outside the walked roots",
        },
      ],
      stale: [],
    },
    12,
    23870,
  );
}

/** Nothing to report: the empty scenario's corpus. */
function clean(mode: "observe" | "enforce"): CheckReport {
  return report({ mode, verdict: "clean", findings: [], stale: [] }, 4, 9180);
}

/** The check could not vouch for the corpus: two causes, no W (`worst_w_bytes` 0). */
function cannotCheck(project: string): CheckReport {
  const harbor = project === HARBOR;
  return {
    mode: harbor ? "observe" : "enforce",
    verdict: "cannot-check",
    counts: { documents: harbor ? 39 : 10, errors: 0, warnings: 0, debt: 0, expired: 0, stale: 0, worst_w_bytes: 0 },
    findings: [],
    stale: [],
    cannot_check: harbor
      ? [
          { path: "docs/spec/cranes.md", message: "cannot read: Permission denied (os error 13)" },
          { path: "docs/spec/locks", message: "directory cannot be listed; its files are unchecked" },
        ]
      : [
          { path: "docs/spec/api", message: "directory cannot be listed; its files are unchecked" },
          { path: "docs/spec/refunds/window.md", message: "cannot read: Input/output error (os error 5)" },
        ],
  };
}

const pad = (value: number) => String(value).padStart(2, "0");

/** The large scenario's eleven codes besides `budget`: six errors, five warnings. */
const LARGE_CODES = [
  ["ref-dangling", "error"],
  ["key-missing", "error"],
  ["status-invalid", "error"],
  ["date-invalid", "error"],
  ["canon-missing", "error"],
  ["id-taken", "error"],
  ["mention-dangling", "warning"],
  ["link-dangling", "warning"],
  ["link-anchor", "warning"],
  ["text-empty", "warning"],
  ["key-empty", "warning"],
] as const;

/** Findings per code of LARGE_CODES; with the eight budget findings, 3 000. */
const PER_CODE = 272;
const LARGE_BUDGETS = 8;

/** A generated mechanic's file (src/mocks/harbor-sim/large.ts): 12 domains of 6 mechanics. */
function generatedFile(seed: number): string {
  const at = seed % 72;
  return `docs/spec/gen/d${pad(Math.floor(at / 6) + 1)}/m${pad((at % 6) + 1)}.md`;
}

function generatedFinding(code: string, severity: "error" | "warning", index: number, seed: number): CheckFinding {
  const subject = `RULE-GEN-${pad(seed % 12)}-${String(index).padStart(3, "0")}`;
  const finding: CheckFinding = {
    code,
    severity,
    path: generatedFile(index * 7 + seed),
    line: 6 + ((index * 3 + seed) % 40),
    subject,
    message: `${code} on \`${subject}\` (generated finding ${String(index + 1)} of ${String(PER_CODE)})`,
  };
  // Every 45th dangling reference is baselined; the first two of them expired.
  if (code === "ref-dangling" && index % 45 === 0) {
    const expired = index < 90;
    finding.debt = { reason: "generated corpus, fixed in bulk", expires: expired ? "2026-06-30" : "2027-03-31", expired };
  }
  return finding;
}

/** harbor-sim under `large`: 3 000 findings over 12 codes, still `observed` under `observe`. */
function harborLarge(): CheckReport {
  const findings: CheckFinding[] = [];
  LARGE_CODES.forEach(([code, severity], seed) => {
    for (let index = 0; index < PER_CODE; index += 1) {
      findings.push(generatedFinding(code, severity, index, seed));
    }
  });
  for (let index = 0; index < LARGE_BUDGETS; index += 1) {
    const slot = index % 2 === 0 ? "canon" : "tier1";
    const cap = slot === "canon" ? 12288 : 10240;
    findings.push({
      code: "budget",
      severity: "error",
      path: `docs/canon/gen-${pad(index + 1)}.md`,
      line: 1,
      subject: slot,
      message: overBudget(cap + 100 * (index + 1), slot, cap),
    });
  }
  return report({ mode: "observe", verdict: "observed", findings, stale: [HARBOR_STALE] }, 3149, 98765);
}

/** The report `spec check --json` gives for `project` in `scenario` (`error` and `slow` act in the client). */
export function checkReportOf(project: string, scenario: Scenario): CheckReport {
  if (scenario === "empty") {
    return clean(project === HARBOR ? "observe" : "enforce");
  }
  if (scenario === "cannot-check") {
    return cannotCheck(project);
  }
  if (project === HARBOR) {
    return scenario === "large" ? harborLarge() : harborObserved();
  }
  if (project === LEDGER) {
    return ledgerFails();
  }
  return clean("enforce");
}
