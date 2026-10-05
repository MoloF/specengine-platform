import { node, proposal, stamp, type MockProject } from "../build";
import { nodeKinds } from "./kinds";

// ledger-api: an invented payments service, with a node vocabulary disjoint from harbor-sim's.

const SLUG = "ledger-api";
const WINDOW_FILE = "docs/spec/refunds/window.md";
const ENDPOINT_FILE = "docs/spec/api/refunds.md";

const ENDPOINT_TEXT = [
  "## EP-REFUND-CREATE: POST /v1/refunds",
  "",
  "Creates a refund for a captured payment.",
  "",
  "- Body: payment_id, amount (minor units), reason.",
  "- Answers 201 with the refund, 422 when the amount exceeds the captured rest.",
].join("\n");

const ENDPOINT_DIFF = [
  `--- base ${ENDPOINT_FILE}`,
  `+++ proposed ${ENDPOINT_FILE}`,
  "@@ -3,4 +3,6 @@",
  " Creates a refund for a captured payment.",
  " ",
  " - Body: payment_id, amount (minor units), reason.",
  "-- Answers 201 with the refund, 422 when the amount exceeds the captured rest.",
  "+- Header: Idempotency-Key, required (POL-IDEMPOTENCY).",
  "+- Answers 201 with the refund, 200 with the same refund on a repeated key,",
  "+  422 when the amount exceeds the captured rest.",
].join("\n");

export function ledgerApi(now: number): MockProject {
  const at = (minutesAgo: number) => stamp(now, minutesAgo);
  const agent = (role: string, run: string) => ({ type: "agent", role, model: "claude-opus-5-5", run });
  return {
    project: { slug: SLUG, name: "Ledger API" },
    nodeKinds,
    notes: ["2 proposals of an archived task are left out (spec inbox --all lists them)"],
    nodes: [
      node({
        id: "SVC-REFUNDS",
        kind: "service",
        title: "Refund service",
        path: "docs/spec/refunds/README.md",
        line: 1,
        status: "accepted",
        rev: 5,
        text: [
          "---",
          "id: SVC-REFUNDS",
          "kind: service",
          "status: accepted",
          "rev: 5",
          "---",
          "",
          "# Refund service",
          "",
          "Owns refunds of captured card payments; writes one ledger entry per refund.",
        ].join("\n"),
      }),
      node({
        id: "EP-REFUND-CREATE",
        kind: "endpoint",
        title: "POST /v1/refunds",
        path: ENDPOINT_FILE,
        line: 5,
        rev: 2,
        text: ENDPOINT_TEXT,
      }),
      node({
        id: "POL-REFUND-WINDOW",
        kind: "policy",
        title: "Refund window",
        path: WINDOW_FILE,
        line: 7,
        rev: 1,
        text: [
          "## POL-REFUND-WINDOW: Refund window",
          "",
          "A payment is refundable within 30 days of capture; later requests go to support.",
        ].join("\n"),
      }),
      node({
        id: "POL-IDEMPOTENCY",
        kind: "policy",
        title: "Idempotency keys",
        path: "docs/spec/api/idempotency.md",
        line: 3,
        rev: 1,
        text: [
          "## POL-IDEMPOTENCY: Idempotency keys",
          "",
          "Every write endpoint takes an Idempotency-Key header and answers a repeated key",
          "with the first result.",
        ].join("\n"),
      }),
    ],
    proposals: [
      proposal({
        id: "PR-0007",
        project: SLUG,
        kind: "discrepancy",
        severity: "high",
        gap_type: "contradicts",
        task_id: "T-0031",
        target_id: "POL-REFUND-WINDOW",
        target_path: WINDOW_FILE,
        target_ids: ["POL-REFUND-WINDOW"],
        branch: "task/T-0031",
        summary: "Refunds are accepted for 60 days; POL-REFUND-WINDOW allows 30",
        evidence: [
          {
            file: "src/refunds/window.rs",
            qpath: "refunds::window::is_refundable",
            lines: "12-20",
            observed: "REFUND_WINDOW_DAYS = 60",
            documented: "A payment is refundable within 30 days of capture.",
          },
        ],
        options: [
          {
            label: "Code to spec",
            effect: "Set REFUND_WINDOW_DAYS to 30.",
            price: "Support tickets for days 31 to 60 rise; finance agrees.",
          },
          {
            label: "Spec to code",
            effect: "Write 60 days into POL-REFUND-WINDOW.",
            price: "Chargeback exposure grows; needs the risk team's sign-off.",
          },
        ],
        recommendation: 0,
        working_answer: "The service keeps 60 days until you decide.",
        author: agent("rust-developer", "R-0418"),
        created_at: at(25),
      }),
      proposal({
        id: "PR-0008",
        project: SLUG,
        kind: "update",
        severity: "normal",
        task_id: "T-0033",
        target_id: "EP-REFUND-CREATE",
        target_path: ENDPOINT_FILE,
        target_ids: ["EP-REFUND-CREATE", "POL-IDEMPOTENCY"],
        summary: "Require an Idempotency-Key on POST /v1/refunds",
        rationale: "POL-IDEMPOTENCY covers every write endpoint; the refund endpoint never said so.",
        base_text: ENDPOINT_TEXT,
        diff: ENDPOINT_DIFF,
        preview: "applies",
        author: agent("spec-writer", "R-0422"),
        diagnostics: [
          {
            code: "ref-dangling",
            severity: "warning",
            path: ENDPOINT_FILE,
            line: 8,
            subject: "POL-IDEMPOTENCY",
            message: "POL-IDEMPOTENCY is cited inline but not declared in links",
          },
        ],
        created_at: at(5 * 60),
      }),
      proposal({
        id: "PR-0009",
        project: SLUG,
        kind: "question",
        severity: "low",
        target_id: "POL-IDEMPOTENCY",
        target_ids: ["POL-IDEMPOTENCY"],
        summary: "How long are idempotency keys kept?",
        options: [
          { label: "24 hours", effect: "Keys expire after a day.", price: "A client retrying after a day creates a second refund." },
          { label: "7 days", effect: "Keys live for a week.", price: "About 40 MB more in the key store." },
        ],
        recommendation: 1,
        working_answer: "Keys are kept for 7 days.",
        author: agent("requirement-analyst", "R-0399"),
        created_at: at(3 * 24 * 60),
      }),
    ],
  };
}
